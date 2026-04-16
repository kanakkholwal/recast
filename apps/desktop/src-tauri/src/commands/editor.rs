use std::fs;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose, Engine as _};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, State};

use super::ffmpeg::{
    append_cursor_overlay_to_complex, append_output_filters_to_complex, build_gif_palette_complex,
    build_output_scale_filter, has_audio, probe_video_metadata, resolve_export_profile,
    summarize_ffmpeg_error,
};
use super::system::get_active_output_dir;
use super::types::{AppState, EditorDocument, ExportRequest, VideoMetadata};
use crate::project::reader::ProjectOpenResult;
#[allow(unused_imports)]
use crate::render::cursor_export::{render_cursor_overlay, CursorOverlayRequest};
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};

/// True if the line is part of an FFmpeg `-progress` block (key=value metric
/// lines that FFmpeg emits every `-stats_period` interval). These should be
/// filtered out of the error ring buffer so a successful export's progress
/// stream doesn't push a real FFmpeg error off the tail. The set matches the
/// keys FFmpeg's `print_report()` writes before `progress=continue` / `end`.
fn is_ffmpeg_progress_key_line(line: &str) -> bool {
    const KEYS: &[&str] = &[
        "frame=",
        "fps=",
        "bitrate=",
        "total_size=",
        "out_time_ms=",
        "out_time=",
        "dup_frames=",
        "drop_frames=",
        "speed=",
        "progress=",
    ];
    let trimmed = line.trim_start();
    if trimmed.starts_with("stream_") {
        // e.g. `stream_0_0_q=28.0`
        return true;
    }
    KEYS.iter().any(|k| trimmed.starts_with(k))
}

fn static_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate = cwd.join("..").join("static");
    if candidate.exists() {
        candidate
    } else {
        cwd.join("static")
    }
}

fn open_project_if_needed(path: &Path) -> Result<Option<ProjectOpenResult>, String> {
    if path.extension().and_then(|value| value.to_str()) == Some("recast") {
        crate::project::reader::open_project(path)
            .map(Some)
            .map_err(|e| e.to_string())
    } else {
        Ok(None)
    }
}

fn project_or_media_metadata(path: &Path) -> Result<VideoMetadata, String> {
    if path.extension().and_then(|value| value.to_str()) == Some("recast") {
        let project = crate::project::reader::open_project(path).map_err(|e| e.to_string())?;
        return Ok(VideoMetadata {
            duration: project.metadata.video.duration_ms as f64 / 1000.0,
            width: project.metadata.video.width,
            height: project.metadata.video.height,
            fps: project.metadata.video.fps as f64,
            codec: "h264".into(),
            size_bytes: fs::metadata(path).map(|m| m.len()).unwrap_or_default(),
        });
    }
    probe_video_metadata(path)
}

#[tauri::command]
pub fn get_video_metadata(path: String) -> Result<VideoMetadata, String> {
    project_or_media_metadata(Path::new(&path))
}

#[tauri::command]
pub fn load_editor_document(path: String) -> Result<EditorDocument, String> {
    let input = PathBuf::from(&path);
    if let Some(project) = open_project_if_needed(&input)? {
        let render_state = fs::read_to_string(&project.edits_path)
            .ok()
            .and_then(|content| serde_json::from_str(&content).ok())
            .unwrap_or_else(|| RenderState {
                trim_end: project.metadata.video.duration_ms as f64 / 1000.0,
                ..RenderState::default()
            });

        return Ok(EditorDocument {
            project_path: path,
            media_path: project.recording_path.to_string_lossy().to_string(),
            cursor_path: Some(project.cursor_path.to_string_lossy().to_string()),
            edits_path: Some(project.edits_path.to_string_lossy().to_string()),
            audio_path: project.audio_path.map(|p| p.to_string_lossy().to_string()),
            microphone_path: project
                .microphone_path
                .map(|p| p.to_string_lossy().to_string()),
            camera_path: project.camera_path.map(|p| p.to_string_lossy().to_string()),
            metadata: VideoMetadata {
                duration: project.metadata.video.duration_ms as f64 / 1000.0,
                width: project.metadata.video.width,
                height: project.metadata.video.height,
                fps: project.metadata.video.fps as f64,
                codec: "h264".into(),
                size_bytes: fs::metadata(&input).map(|m| m.len()).unwrap_or_default(),
            },
            render_state,
        });
    }

    let metadata = probe_video_metadata(&input)?;
    Ok(EditorDocument {
        project_path: path.clone(),
        media_path: path,
        cursor_path: None,
        edits_path: None,
        audio_path: None,
        microphone_path: None,
        camera_path: None,
        metadata: metadata.clone(),
        render_state: RenderState {
            trim_end: metadata.duration,
            ..RenderState::default()
        },
    })
}

#[tauri::command]
pub fn generate_thumbnails(path: String, count: u32) -> Result<Vec<String>, String> {
    let input = PathBuf::from(&path);
    let project = open_project_if_needed(&input)?;
    let media_path = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or(input);
    let meta = probe_video_metadata(&media_path)?;
    if meta.duration <= 0.0 || count == 0 {
        return Ok(Vec::new());
    }

    let interval = meta.duration / count as f64;
    // Unique subdir per invocation so concurrent thumbnail requests (e.g. two
    // editor tabs scrubbing at once) don't race on the same `thumb-N.jpg`.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir()
        .join("recast-thumbnails")
        .join(format!("{}-{stamp}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let mut thumbnails = Vec::new();

    let scale_width = if count <= 2 { 480 } else { 240 };

    for index in 0..count {
        let timestamp = index as f64 * interval;
        let thumb_path = temp_dir.join(format!("thumb-{index}.jpg"));
        let result = Command::new(crate::ffmpeg::ffmpeg_path())
            .args([
                "-y",
                "-ss",
                &format!("{timestamp:.2}"),
                "-i",
                &media_path.to_string_lossy(),
                "-vframes",
                "1",
                "-vf",
                &format!("scale={scale_width}:-1"),
                "-q:v",
                "4",
                thumb_path.to_string_lossy().as_ref(),
            ])
            .output();

        if let Ok(output) = result {
            if output.status.success() {
                if let Ok(data) = fs::read(&thumb_path) {
                    thumbnails.push(format!(
                        "data:image/jpeg;base64,{}",
                        general_purpose::STANDARD.encode(data)
                    ));
                }
            }
        }
        let _ = fs::remove_file(&thumb_path);
    }

    // Best-effort removal of the now-empty per-invocation subdir. Ignore
    // failure (parallel invocations or filesystem races can leave stragglers).
    let _ = fs::remove_dir(&temp_dir);

    Ok(thumbnails)
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    request: ExportRequest,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Install a fresh cancellation token for this run. Using a per-run token
    // (rather than a shared flag that we reset-on-entry) closes a race where a
    // cancel click issued between the frontend's IPC kickoff and this command's
    // first line would be stomped by the reset and silently lost.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    *state.export_cancel.lock() = Some(cancel_flag.clone());

    let input_path = PathBuf::from(&request.input_path);
    let project = open_project_if_needed(&input_path)?;
    let source_video = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or_else(|| input_path.clone());
    let metadata = probe_video_metadata(&source_video)?;
    if metadata.width == 0 || metadata.height == 0 {
        return Err("export failed: source video metadata is incomplete".into());
    }
    let graph = RenderGraph::from_state(&request.render_state);
    let (trim_start, trim_end) = graph.trim_range();
    let duration = (trim_end - trim_start).max(0.0);
    // Snapshot the source's full duration to use as a progress-denominator
    // fallback when the render state has no Trim node (duration == 0).
    let source_duration = metadata.duration.max(0.0);
    let profile = resolve_export_profile(&request.quality);
    let output_scale_filter = build_output_scale_filter(profile);
    let output_dir = get_active_output_dir(&state).join("exports");
    let _ = std::fs::create_dir_all(&output_dir);
    let extension = match request.format.as_str() {
        "gif" => "gif",
        "webm" => "webm",
        _ => "mp4",
    };
    // Nanosecond-resolution + PID suffix so back-to-back exports (or two editor
    // windows exporting at once) can't collide on the same second and overwrite
    // each other's output / trigger cleanup of the wrong file on failure.
    let stamp_nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let output_path = output_dir.join(format!(
        "recast_export_{stamp_nanos}_{}.{extension}",
        std::process::id()
    ));

    let export_plan = graph
        .build_export_plan(
            SourceVideoMetadata {
                width: metadata.width,
                height: metadata.height,
            },
            &static_root(),
            1,
        )
        .map_err(|e| e.to_string())?;

    // TEMPORARY: cursor overlay in export is disabled while we diagnose a
    // reproducible hang + output-corruption issue. Decoding a pre-rendered
    // alpha VP9 webm and compositing it at 4K was causing FFmpeg to either
    // stall during the mux trailer or write an unplayable file. Until we have
    // a reliable path (likely a separate stream-copy remux pass), export runs
    // without the cursor overlay — the WebGL preview still shows it correctly.
    let cursor_overlay: Option<crate::render::cursor_export::CursorOverlayResult> = None;

    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        // Progress reporting goes to stderr (pipe:2), not stdout (pipe:1).
        // On Windows with NVENC + a non-trivial filter_complex, FFmpeg's pipe:1
        // progress writes get batched — we've observed 40 s of silence followed
        // by a single burst of lines right before `progress=end`, which made
        // the UI sit on "Preparing…" for the entire encode. Stderr is flushed
        // per progress block on every Windows build we've tested, so routing
        // here gives us real-time updates from the very first GOP.
        // `-stats_period 0.1` forces 100 ms updates.
        "-progress".to_string(),
        "pipe:2".to_string(),
        "-stats_period".to_string(),
        "0.1".to_string(),
    ];
    if trim_start > 0.0 {
        args.extend(["-ss".to_string(), format!("{trim_start:.3}")]);
    }
    if duration > 0.0 {
        args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }
    args.extend(["-i".to_string(), source_video.to_string_lossy().to_string()]);

    for input in &export_plan.extra_inputs {
        args.extend([
            "-loop".to_string(),
            "1".to_string(),
            "-i".to_string(),
            input.to_string_lossy().to_string(),
        ]);
    }

    // Cursor overlay is input index = 1 + export_plan.extra_inputs.len()
    let cursor_input_index = 1 + export_plan.extra_inputs.len();
    let cursor_overlay_path = cursor_overlay.as_ref().map(|o| o.overlay_path.clone());
    if let Some(ref path) = cursor_overlay_path {
        args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
    }

    // Build the final filter_complex string taking cursor overlay into account.
    let (initial_filter_complex, initial_video_map) = (
        export_plan.filter_complex.clone(),
        export_plan.video_map.clone(),
    );
    let (mut filter_complex_after_cursor, mut video_map_after_cursor) =
        if cursor_overlay_path.is_some() {
            let (new_complex, new_map) = append_cursor_overlay_to_complex(
                initial_filter_complex.as_deref(),
                &initial_video_map,
                cursor_input_index,
            );
            (Some(new_complex), new_map)
        } else {
            (initial_filter_complex, initial_video_map)
        };

    // For GIF, always route through filter_complex with a palettegen/paletteuse
    // pipeline. Naive single-pass GIF encoding uses a per-frame 256-colour palette
    // which produces heavy banding and dithered noise. Baking fps + any output
    // scale into the palette chain means we don't need a separate `-vf` or a
    // post-hoc merge step for GIFs.
    let mut output_filters: Vec<String> = Vec::new();
    if request.format == "gif" {
        let (gif_complex, gif_map) = build_gif_palette_complex(
            filter_complex_after_cursor.as_deref(),
            &video_map_after_cursor,
            profile.gif_fps,
            output_scale_filter.as_deref(),
        );
        filter_complex_after_cursor = Some(gif_complex);
        video_map_after_cursor = gif_map;
    } else if let Some(scale_filter) = output_scale_filter {
        output_filters.push(scale_filter);
    }

    if let Some(ref filter_complex) = filter_complex_after_cursor {
        args.extend([
            "-filter_complex".to_string(),
            filter_complex.clone(),
            "-map".to_string(),
            video_map_after_cursor.clone(),
        ]);
    } else {
        args.extend(["-map".to_string(), "0:v:0".to_string()]);
    }

    let has_source_audio = has_audio(&source_video) && request.format != "gif";
    if has_source_audio {
        args.extend(["-map".to_string(), "0:a?".to_string()]);
    }

    if !output_filters.is_empty() && filter_complex_after_cursor.is_none() {
        args.extend(["-vf".to_string(), output_filters.join(",")]);
    }

    if duration <= 0.0 {
        if !export_plan.extra_inputs.is_empty() || cursor_overlay_path.is_some() {
            args.push("-shortest".to_string());
        }
    }

    match request.format.as_str() {
        "gif" => {
            args.extend([
                "-an".to_string(),
                "-loop".to_string(),
                "0".to_string(),
                output_path.to_string_lossy().to_string(),
            ]);
        }
        "webm" => {
            args.extend([
                "-c:v".to_string(),
                "libvpx-vp9".to_string(),
                "-crf".to_string(),
                profile.webm_crf.to_string(),
                "-b:v".to_string(),
                "0".to_string(),
            ]);
            if has_source_audio {
                args.extend(["-c:a".to_string(), "libopus".to_string()]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
        _ => {
            // NOTE: we intentionally do NOT pass `-movflags +faststart` here.
            // Faststart does an in-place moov-atom rewrite at the very end of
            // the mux, and on 4K clips that rewrite can take 10–60+ seconds
            // while stdout stays silent — manifesting as a UI that's stuck in
            // the "Finalizing…" state. Desktop playback (VLC, Windows Media,
            // browsers reading from disk) works fine with moov-at-end. If we
            // later need HTTP-streamable output, add it as a separate optional
            // `-c copy -movflags +faststart` remux pass with its own progress.
            match crate::ffmpeg::preferred_h264_encoder() {
                "h264_nvenc" => {
                    args.extend([
                        "-c:v".to_string(),
                        "h264_nvenc".to_string(),
                        "-preset".to_string(),
                        "p5".to_string(),
                        "-tune".to_string(),
                        "hq".to_string(),
                        "-rc".to_string(),
                        "vbr".to_string(),
                        "-cq".to_string(),
                        profile.mp4_nvenc_cq.to_string(),
                        "-b:v".to_string(),
                        "0".to_string(),
                        "-profile:v".to_string(),
                        "high".to_string(),
                        "-pix_fmt".to_string(),
                        "yuv420p".to_string(),
                    ]);
                }
                _ => {
                    args.extend([
                        "-c:v".to_string(),
                        "libx264".to_string(),
                        "-preset".to_string(),
                        profile.mp4_preset.to_string(),
                        "-crf".to_string(),
                        profile.mp4_crf.to_string(),
                        "-pix_fmt".to_string(),
                        "yuv420p".to_string(),
                        "-threads".to_string(),
                        "0".to_string(),
                    ]);
                }
            }
            if has_source_audio {
                args.extend([
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "192k".to_string(),
                ]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
    }

    if !output_filters.is_empty() && filter_complex_after_cursor.is_some() {
        let (complex_filter, map_label) = append_output_filters_to_complex(
            filter_complex_after_cursor.as_deref().unwrap_or_default(),
            &video_map_after_cursor,
            &output_filters,
        );

        let filter_index = args
            .iter()
            .position(|arg| arg == "-filter_complex")
            .and_then(|index| args.get_mut(index + 1));
        if let Some(slot) = filter_index {
            *slot = complex_filter;
        }

        let map_index = args
            .iter()
            .position(|arg| arg == "-map")
            .and_then(|index| args.get_mut(index + 1));
        if let Some(slot) = map_index {
            *slot = map_label;
        }
    }

    let output_path_str = output_path.to_string_lossy().to_string();
    log::info!("export ffmpeg args: {}", args.join(" "));

    // Spawn FFmpeg in a background thread so the UI stays responsive.
    // Watchdog: if 60s pass without a progress line, kill the child.
    // Clone the handle so we retain one outside the closure for the
    // panic-fallback emit in the match below.
    let app_for_fallback = app.clone();
    let task_result = tokio::task::spawn_blocking(move || {
        let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NO_WINDOW: u32 = 0x08000000;
            command.creation_flags(CREATE_NO_WINDOW);
        }

        let mut child = command
            .spawn()
            .map_err(|e| format!("failed to start ffmpeg: {e}"))?;

        let mut stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ffmpeg stdout pipe not available".to_string())?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| "ffmpeg stderr pipe not available".to_string())?;

        // Shared state consumed by the stderr parser (progress events) and the
        // watchdog (stall detection).
        let last_progress = Arc::new(Mutex::new(Instant::now()));
        let killed_by_timeout = Arc::new(AtomicBool::new(false));
        let killed_by_user = Arc::new(AtomicBool::new(false));

        // Parse stderr line-by-line. Progress blocks (key=value lines) get
        // filtered out; only genuine log output is appended to the 8 KB error
        // ring buffer used for post-mortem in the failure path. `out_time_us=`
        // lines drive the UI `export-progress` emits, and `progress=end`
        // signals the encoder has finished and only the mux trailer remains.
        let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf_writer = stderr_buf.clone();
        let stderr_last_progress = last_progress.clone();
        let stderr_app = app.clone();
        let stderr_thread = std::thread::Builder::new()
            .name("recast-export-stderr".into())
            .spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                let mut finalizing_emitted = false;
                for line in reader.lines().map_while(Result::ok) {
                    // FFmpeg progress blocks are key=value lines terminated by
                    // `progress=continue` (between blocks) or `progress=end`
                    // (final block). Treat all of these as non-log noise.
                    if let Some(time_str) = line.strip_prefix("out_time_us=") {
                        if let Ok(time_us) = time_str.trim().parse::<f64>() {
                            {
                                let mut guard = stderr_last_progress.lock();
                                *guard = Instant::now();
                            }
                            let progress_secs = time_us / 1_000_000.0;
                            let effective_duration = if duration > 0.0 {
                                duration
                            } else {
                                source_duration
                            };
                            let pct = if effective_duration > 0.0 {
                                (progress_secs / effective_duration * 100.0)
                                    .clamp(0.0, 100.0)
                            } else {
                                0.0
                            };
                            let _ = stderr_app.emit("export-progress", pct);
                        }
                        continue;
                    }
                    // `progress=end` means FFmpeg has finished encoding and
                    // is about to write the container trailer / exit. Flip
                    // the UI to finalizing NOW rather than waiting for the
                    // pipes to close — on Windows stderr close can lag the
                    // actual encoder finish by seconds, which manifested as
                    // the bar sitting at 100% with no state change. Also
                    // stamp `last_progress` so the 60 s stall watchdog gives
                    // the trailer write its own fresh budget.
                    if line.trim() == "progress=end" {
                        if !finalizing_emitted {
                            finalizing_emitted = true;
                            let _ = stderr_app.emit("export-progress", 100.0_f64);
                            let _ = stderr_app.emit("export-finalizing", ());
                            log::info!("export: progress=end seen, flipping UI to finalizing");
                        }
                        let mut guard = stderr_last_progress.lock();
                        *guard = Instant::now();
                        continue;
                    }
                    if is_ffmpeg_progress_key_line(&line) {
                        continue;
                    }
                    // Everything else is real log output — append to the ring
                    // buffer so the failure path can surface it to the user.
                    let mut guard = stderr_buf_writer.lock();
                    guard.extend_from_slice(line.as_bytes());
                    guard.push(b'\n');
                    if guard.len() > 8192 {
                        let overflow = guard.len() - 8192;
                        guard.drain(0..overflow);
                    }
                }
                log::info!("export: stderr thread exiting (pipe closed)");
            })
            .map_err(|e| format!("failed to spawn stderr drain thread: {e}"))?;

        // Stdout carries nothing useful now that progress is on stderr, but we
        // still need to drain it — closing or ignoring the pipe can cause
        // FFmpeg to hit EPIPE on any stray write (e.g. `-report`) and abort.
        let stdout_thread = std::thread::Builder::new()
            .name("recast-export-stdout".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match stdout.read(&mut buf) {
                        Ok(0) | Err(_) => break,
                        Ok(_) => {}
                    }
                }
                log::info!("export: stdout thread exiting (pipe closed)");
            })
            .map_err(|e| format!("failed to spawn stdout drain thread: {e}"))?;

        // Spawn the watchdog thread — narrow responsibility: only kill the
        // child if it stops producing progress for >60s (genuine stall) OR if
        // the user-facing cancel flag flips. Previous versions also auto-
        // emitted `export-finalizing` when progress went quiet for 1.5s, but
        // that fired falsely on Windows when FFmpeg's pipe buffering batched
        // progress into multi-second bursts, flipping the UI to "Finalizing"
        // mid-encode and leaving it there. Finalization detection now lives
        // in the reader loop (below) on the more-reliable pct≥95 signal.
        let watchdog_last_progress = last_progress.clone();
        let watchdog_killed = killed_by_timeout.clone();
        let watchdog_cancel_flag = cancel_flag.clone();
        let watchdog_user_kill = killed_by_user.clone();
        let watchdog_stop = Arc::new(AtomicBool::new(false));
        let watchdog_stop_flag = watchdog_stop.clone();
        // Share the child with the watchdog via a mutex so it can call kill().
        let child_handle = Arc::new(Mutex::new(Some(child)));
        let watchdog_child = child_handle.clone();
        let watchdog_thread = std::thread::Builder::new()
            .name("recast-export-watchdog".into())
            .spawn(move || {
                const STALL_TIMEOUT: Duration = Duration::from_secs(60);
                const POLL_INTERVAL: Duration = Duration::from_millis(250);
                while !watchdog_stop_flag.load(Ordering::Acquire) {
                    std::thread::sleep(POLL_INTERVAL);
                    if watchdog_stop_flag.load(Ordering::Acquire) {
                        return;
                    }
                    if watchdog_cancel_flag.load(Ordering::Acquire) {
                        let mut guard = watchdog_child.lock();
                        if let Some(ref mut child) = *guard {
                            log::info!("export cancel: killing ffmpeg process on user request");
                            let _ = child.kill();
                            watchdog_user_kill.store(true, Ordering::Release);
                        }
                        return;
                    }
                    let elapsed = {
                        let guard = watchdog_last_progress.lock();
                        guard.elapsed()
                    };
                    if elapsed > STALL_TIMEOUT {
                        let mut guard = watchdog_child.lock();
                        if let Some(ref mut child) = *guard {
                            log::warn!("export watchdog: killing stalled ffmpeg process (no progress for {:?})", elapsed);
                            let _ = child.kill();
                            watchdog_killed.store(true, Ordering::Release);
                        }
                        return;
                    }
                }
            })
            .map_err(|e| format!("failed to spawn watchdog thread: {e}"))?;

        // Wait for the I/O drain threads to finish. Both unblock when FFmpeg
        // closes its respective pipes, which happens as it's exiting.
        let _ = stdout_thread.join();
        let _ = stderr_thread.join();
        log::info!("export: drain threads joined, pipes closed");

        // Redundant-but-idempotent final emit: if `progress=end` wasn't seen
        // (e.g. FFmpeg was killed before finishing), make sure the UI still
        // gets a finalizing flip before `export-done` arrives so the dialog
        // has a consistent visual sequence.
        let _ = app.emit("export-progress", 100.0_f64);
        let _ = app.emit("export-finalizing", ());

        // Stop the watchdog now that the I/O is done.
        watchdog_stop.store(true, Ordering::Release);
        let _ = watchdog_thread.join();

        // Pull the child back out and wait for its exit status. Stdout has
        // already closed, so FFmpeg should be on its last gasp (trailer write +
        // teardown). A well-behaved exit happens within milliseconds. We still
        // bound the wait with a hard timeout so the UI can never be stuck in
        // "Finalizing…" indefinitely if the process refuses to exit — if it
        // takes longer than POST_CLOSE_TIMEOUT, we force-kill and surface an
        // error rather than leaving the Promise hanging.
        let mut child = {
            let mut guard = child_handle.lock();
            guard.take()
        }
        .ok_or_else(|| "ffmpeg child handle missing".to_string())?;

        const POST_CLOSE_TIMEOUT: Duration = Duration::from_secs(30);
        let wait_deadline = Instant::now() + POST_CLOSE_TIMEOUT;
        let mut forced_exit = false;
        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if Instant::now() >= wait_deadline {
                        log::warn!(
                            "export post-close wait exceeded {:?}; force-killing ffmpeg",
                            POST_CLOSE_TIMEOUT
                        );
                        let _ = child.kill();
                        forced_exit = true;
                        // One final wait after kill to reap the process.
                        break child.wait().map_err(|e| e.to_string())?;
                    }
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(e) => return Err(e.to_string()),
            }
        };

        if forced_exit {
            let _ = std::fs::remove_file(&output_path_str);
            let err_msg = format!(
                "export failed: ffmpeg did not exit within {}s of finishing the encode",
                POST_CLOSE_TIMEOUT.as_secs()
            );
            let _ = app.emit(
                "export-done",
                serde_json::json!({ "kind": "error", "message": err_msg.clone() }),
            );
            return Err(err_msg);
        }

        if killed_by_user.load(Ordering::Acquire) {
            // Clean up the half-written output file so the exports list doesn't
            // show a broken artifact from the aborted run.
            let _ = std::fs::remove_file(&output_path_str);
            let _ = app.emit(
                "export-done",
                serde_json::json!({ "kind": "cancelled" }),
            );
            return Err("export cancelled".to_string());
        }

        if killed_by_timeout.load(Ordering::Acquire) {
            let _ = std::fs::remove_file(&output_path_str);
            let err_msg = "export timed out: ffmpeg produced no progress for 60s".to_string();
            let _ = app.emit(
                "export-done",
                serde_json::json!({ "kind": "error", "message": err_msg }),
            );
            return Err(err_msg);
        }

        if !status.success() {
            let stderr_bytes = stderr_buf.lock().clone();
            let _ = std::fs::remove_file(&output_path_str);
            let err_msg = format!(
                "export failed:\n{}",
                summarize_ffmpeg_error(&stderr_bytes)
            );
            let _ = app.emit(
                "export-done",
                serde_json::json!({ "kind": "error", "message": err_msg.clone() }),
            );
            return Err(err_msg);
        }

        // Log stderr tail even on success so we can diagnose silent warnings
        // (e.g. mux trailer problems) that produce a "valid" exit code but a
        // broken file.
        let stderr_bytes = stderr_buf.lock().clone();
        if !stderr_bytes.is_empty() {
            let tail = String::from_utf8_lossy(&stderr_bytes);
            log::info!("export ffmpeg stderr tail: {tail}");
        }

        // Final 100% ping + an `export-done` event with the result. The
        // frontend uses `export-done` to transition the dialog to the success
        // state immediately — decoupled from the `exportVideo` Promise, which
        // may take an extra beat to resolve through Tauri's IPC layer.
        let _ = app.emit("export-progress", 100.0_f64);
        let _ = app.emit(
            "export-done",
            serde_json::json!({ "kind": "success", "path": &output_path_str }),
        );
        Ok(output_path_str)
    })
    .await;

    // Cleanup must run regardless of whether the task returned Ok/Err or even
    // panicked — otherwise a panic would leak the cursor overlay's temp dir and
    // leave a stale cancel token installed that would poison the next export.
    drop(cursor_overlay);
    *state.export_cancel.lock() = None;

    match task_result {
        Ok(inner) => inner,
        Err(join_err) => {
            // spawn_blocking only errors on panic; surface it so the frontend
            // can show a real failure dialog instead of hanging on the Promise.
            let err_msg = format!("export task failed: {join_err}");
            let _ = app_for_fallback.emit(
                "export-done",
                serde_json::json!({ "kind": "error", "message": err_msg.clone() }),
            );
            Err(err_msg)
        }
    }
}

/// Signal any running export to abort. The watchdog thread polls this flag every
/// ~250ms and kills the ffmpeg child process, which causes `export_video` to
/// return `Err("export cancelled")`. Safe to call when no export is running
/// (the flag just gets reset at the start of the next run).
#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>) -> Result<(), String> {
    if let Some(flag) = state.export_cancel.lock().as_ref() {
        flag.store(true, Ordering::Release);
    }
    // No installed token → no active export. Treat as a no-op rather than
    // an error so double-clicks on Cancel don't surface a confusing toast.
    Ok(())
}

#[tauri::command]
pub fn autosave_project(project_path: String, edits_json: String) -> Result<(), String> {
    crate::project::autosave::save_autosave(Path::new(&project_path), &edits_json)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn clear_autosave(project_path: String) {
    crate::project::autosave::clear_autosave(Path::new(&project_path));
}

#[tauri::command]
pub fn get_recoverable_sessions() -> Vec<crate::project::autosave::AutosaveState> {
    crate::project::autosave::find_recoverable_sessions()
}

/// Analyse a captured cursor track and return the list of moments that would
/// make good auto-focus candidates (mouse-down events + settle-after-motion).
/// Reuses the existing `detect_zoom_triggers` helper — falls back to on-the-fly
/// recomputation if the project was saved before trigger persistence landed.
#[tauri::command]
pub fn suggest_zoom_regions(cursor_path: String) -> Result<Vec<crate::cursor::smoothing::ZoomTrigger>, String> {
    let bytes = fs::read(Path::new(&cursor_path)).map_err(|e| format!("read cursor track: {e}"))?;
    let track: crate::cursor::CursorTrack =
        serde_json::from_slice(&bytes).map_err(|e| format!("parse cursor track: {e}"))?;
    if !track.zoom_triggers.is_empty() {
        return Ok(track.zoom_triggers);
    }
    Ok(crate::cursor::smoothing::detect_zoom_triggers(
        &track.samples,
        &track.clicks,
    ))
}

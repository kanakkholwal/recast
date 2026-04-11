use std::fs;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use base64::{Engine as _, engine::general_purpose};
use parking_lot::Mutex;
use tauri::{AppHandle, Emitter, State};

use super::ffmpeg::{
    append_cursor_overlay_to_complex, append_output_filters_to_complex, build_output_scale_filter,
    has_audio, probe_video_metadata, resolve_export_profile, summarize_ffmpeg_error,
};
#[allow(unused_imports)]
use crate::render::cursor_export::{CursorOverlayRequest, render_cursor_overlay};
use super::system::get_active_output_dir;
use super::types::{AppState, EditorDocument, ExportRequest, VideoMetadata};
use crate::project::reader::ProjectOpenResult;
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};

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
            microphone_path: project.microphone_path.map(|p| p.to_string_lossy().to_string()),
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
    let temp_dir = std::env::temp_dir().join("recast-thumbnails");
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

    Ok(thumbnails)
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    request: ExportRequest,
    state: State<'_, AppState>,
) -> Result<String, String> {
    // Reset the cancellation flag at the start of every run. A stale `true` from a
    // prior cancel would otherwise kill this run immediately.
    state.export_cancel.store(false, Ordering::Release);
    let cancel_flag = state.export_cancel.clone();

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
    let profile = resolve_export_profile(&request.quality);
    let output_scale_filter = build_output_scale_filter(profile);
    let output_dir = get_active_output_dir(&state).join("exports");
    let _ = std::fs::create_dir_all(&output_dir);
    let extension = match request.format.as_str() {
        "gif" => "gif",
        "webm" => "webm",
        _ => "mp4",
    };
    let output_path = output_dir.join(format!(
        "recast_export_{}.{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        extension
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
        // Progress reporting needs to be a global option so FFmpeg 7.x parses
        // it correctly regardless of where the output file sits in the args.
        // `-stats_period 0.1` forces 100ms updates so the UI transitions from
        // "Preparing…" to a determinate progress bar almost immediately.
        "-progress".to_string(),
        "pipe:1".to_string(),
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
    let (filter_complex_after_cursor, video_map_after_cursor) = if cursor_overlay_path.is_some() {
        let (new_complex, new_map) = append_cursor_overlay_to_complex(
            initial_filter_complex.as_deref(),
            &initial_video_map,
            cursor_input_index,
        );
        (Some(new_complex), new_map)
    } else {
        (initial_filter_complex, initial_video_map)
    };

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

    let mut output_filters = Vec::new();
    if request.format == "gif" {
        output_filters.push(format!("fps={}", profile.gif_fps));
    }
    if let Some(scale_filter) = output_scale_filter {
        output_filters.push(scale_filter);
    }
    if !output_filters.is_empty() && filter_complex_after_cursor.is_none() {
        args.extend(["-vf".to_string(), output_filters.join(",")]);
    }

    if !export_plan.extra_inputs.is_empty() || cursor_overlay_path.is_some() {
        args.push("-shortest".to_string());
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
    let result = tokio::task::spawn_blocking(move || {
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

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "ffmpeg stdout pipe not available".to_string())?;
        let mut stderr = child
            .stderr
            .take()
            .ok_or_else(|| "ffmpeg stderr pipe not available".to_string())?;

        // Drain stderr in a separate thread so the pipe buffer can't fill up and
        // deadlock FFmpeg. Keep the last ~8KB for error reporting.
        let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf_writer = stderr_buf.clone();
        let stderr_thread = std::thread::Builder::new()
            .name("recast-export-stderr".into())
            .spawn(move || {
                let mut buf = [0u8; 4096];
                loop {
                    match stderr.read(&mut buf) {
                        Ok(0) => break,
                        Ok(n) => {
                            let mut guard = stderr_buf_writer.lock();
                            guard.extend_from_slice(&buf[..n]);
                            // Cap at 8KB, keeping the tail (most recent output).
                            if guard.len() > 8192 {
                                let overflow = guard.len() - 8192;
                                guard.drain(0..overflow);
                            }
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|e| format!("failed to spawn stderr drain thread: {e}"))?;

        // Watchdog state: last time we saw a progress line.
        let last_progress = Arc::new(Mutex::new(Instant::now()));
        let killed_by_timeout = Arc::new(AtomicBool::new(false));
        let killed_by_user = Arc::new(AtomicBool::new(false));

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

        // Tracks whether we've already emitted `export-finalizing` so we don't
        // send it twice (once from the reader, once from the post-loop fallback).
        let mut finalizing_emitted = false;

        // Read stdout for progress updates.
        let reader = std::io::BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            if let Some(time_str) = line.strip_prefix("out_time_us=") {
                if let Ok(time_us) = time_str.trim().parse::<f64>() {
                    {
                        let mut guard = last_progress.lock();
                        *guard = Instant::now();
                    }
                    let progress_secs = time_us / 1_000_000.0;
                    let pct = if duration > 0.0 {
                        (progress_secs / duration * 100.0).clamp(0.0, 100.0)
                    } else {
                        0.0
                    };
                    let _ = app.emit("export-progress", pct);
                    // When FFmpeg reports we're essentially done encoding,
                    // flip the UI into finalizing mode. Threshold is intentionally
                    // loose (95 vs. 99) because the final out_time_us reported by
                    // FFmpeg is typically one frame period shy of the full duration.
                    if !finalizing_emitted && pct >= 95.0 {
                        finalizing_emitted = true;
                        let _ = app.emit("export-progress", 100.0_f64);
                        let _ = app.emit("export-finalizing", ());
                    }
                }
            }
        }

        // stdout closed → FFmpeg is fully done. Fallback emit in case we
        // never crossed the 95% threshold (e.g. a very short clip where the
        // last progress line landed below 95%).
        if !finalizing_emitted {
            let _ = app.emit("export-progress", 100.0_f64);
            let _ = app.emit("export-finalizing", ());
        }

        // Stop the watchdog and wait for it + the stderr drain to finish.
        watchdog_stop.store(true, Ordering::Release);
        let _ = watchdog_thread.join();
        let _ = stderr_thread.join();

        // Pull the child back out and wait for its exit status.
        let mut child = {
            let mut guard = child_handle.lock();
            guard.take()
        }
        .ok_or_else(|| "ffmpeg child handle missing".to_string())?;
        let status = child.wait().map_err(|e| e.to_string())?;

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
    .await
    .map_err(|e| format!("export task failed: {e}"))?;

    // Explicitly hold `cursor_overlay` alive until the child has finished reading
    // the webm. Dropping it here runs the TempDirGuard that cleans up scratch files.
    drop(cursor_overlay);

    result
}

/// Signal any running export to abort. The watchdog thread polls this flag every
/// ~250ms and kills the ffmpeg child process, which causes `export_video` to
/// return `Err("export cancelled")`. Safe to call when no export is running
/// (the flag just gets reset at the start of the next run).
#[tauri::command]
pub fn cancel_export(state: State<'_, AppState>) -> Result<(), String> {
    state.export_cancel.store(true, Ordering::Release);
    Ok(())
}

#[tauri::command]
pub fn autosave_project(project_path: String, edits_json: String) -> Result<(), String> {
    crate::project::autosave::save_autosave(
        Path::new(&project_path),
        &edits_json,
    )
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

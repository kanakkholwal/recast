use std::fs;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use base64::{engine::general_purpose, Engine as _};
use parking_lot::Mutex;
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::ffmpeg::{
    append_camera_overlay_to_complex, append_cursor_overlay_to_complex,
    append_output_filters_to_complex, build_annotation_blur_complex,
    build_gif_palette_prepass_filter, build_gif_paletteuse_external_complex,
    build_output_scale_filter, has_audio, probe_video_metadata, resolve_export_profile,
    summarize_ffmpeg_error, BlurRegion, CameraOverlayParams, GifFilterOptions,
};
use super::system::get_active_output_dir;
use super::types::{AppState, EditorDocument, ExportRequest, GifSettings, VideoMetadata};
use crate::project::reader::ProjectOpenResult;
#[allow(unused_imports)]
use crate::render::cursor_export::{render_cursor_overlay, CursorOverlayRequest};
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};
use crate::render::mask_export::{render_border_radius_mask, MaskResult};
use crate::render::node_types::{AnnotationKind, AudioSettings};

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

fn parse_ffmpeg_progress_seconds(line: &str) -> Option<f64> {
    if let Some(value) = line
        .strip_prefix("out_time_us=")
        .or_else(|| line.strip_prefix("out_time_ms="))
    {
        return value
            .trim()
            .parse::<f64>()
            .ok()
            .map(|raw| raw / 1_000_000.0);
    }

    let value = line.strip_prefix("out_time=")?.trim();
    let mut parts = value.split(':');
    let hours = parts.next()?.parse::<f64>().ok()?;
    let minutes = parts.next()?.parse::<f64>().ok()?;
    let seconds = parts.next()?.parse::<f64>().ok()?;
    if parts.next().is_some() {
        return None;
    }
    Some(hours * 3600.0 + minutes * 60.0 + seconds)
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

fn completed_export_looks_usable(path: &Path, expected_duration: f64) -> bool {
    if !path.exists() {
        return false;
    }

    let Ok(metadata) = probe_video_metadata(path) else {
        return false;
    };

    if metadata.duration <= 0.0 || metadata.width == 0 || metadata.height == 0 {
        return false;
    }

    if expected_duration <= 0.0 {
        return true;
    }

    let min_duration = if expected_duration > 1.0 {
        (expected_duration - 0.5).max(expected_duration * 0.95)
    } else {
        expected_duration * 0.75
    };

    metadata.duration + 0.05 >= min_duration
}

fn append_audio_to_complex(
    existing: Option<&str>,
    audio_inputs: &[usize],
    settings: &AudioSettings,
    trim_start: f64,
    duration: f64,
) -> Option<(String, String)> {
    if audio_inputs.is_empty() || settings.muted || settings.volume <= 0.0 {
        return None;
    }

    let volume = (settings.volume / 100.0).clamp(0.0, 4.0);
    let mut segments: Vec<String> = existing
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .into_iter()
        .collect();
    let mut labels = Vec::new();

    for (i, input_index) in audio_inputs.iter().enumerate() {
        let label = if audio_inputs.len() == 1 {
            "aout".to_string()
        } else {
            format!("aud{i}")
        };
        let mut filters = Vec::new();
        if duration > 0.0 {
            filters.push(format!(
                "atrim=start={:.3}:duration={:.3}",
                trim_start.max(0.0),
                duration
            ));
        } else if trim_start > 0.0 {
            filters.push(format!("atrim=start={:.3}", trim_start));
        }
        filters.push("asetpts=PTS-STARTPTS".to_string());
        filters.push(format!("volume={volume:.4}"));
        if settings.fade_in > 0.0 {
            let fade = if duration > 0.0 {
                settings.fade_in.min(duration * 0.5)
            } else {
                settings.fade_in
            };
            if fade > 0.0 {
                filters.push(format!("afade=t=in:st=0:d={fade:.3}"));
            }
        }
        if duration > 0.0 && settings.fade_out > 0.0 {
            let fade = settings.fade_out.min(duration * 0.5);
            let start = (duration - fade).max(0.0);
            if fade > 0.0 {
                filters.push(format!("afade=t=out:st={start:.3}:d={fade:.3}"));
            }
        }
        segments.push(format!("[{input_index}:a]{}[{label}]", filters.join(",")));
        labels.push(format!("[{label}]"));
    }

    if audio_inputs.len() > 1 {
        segments.push(format!(
            "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[aout]",
            labels.join(""),
            audio_inputs.len()
        ));
    }

    Some((segments.join(";"), "[aout]".into()))
}

fn append_watermark_to_complex(
    existing: Option<&str>,
    current_video_map: &str,
    watermark_input_index: usize,
    settings: &crate::render::node_types::WatermarkSettings,
    canvas_width: u32,
    _canvas_height: u32,
) -> (String, String) {
    let normalized_current = if current_video_map.starts_with('[') {
        current_video_map.to_string()
    } else {
        format!("[{current_video_map}]")
    };
    let scale_width = ((canvas_width as f64) * (settings.scale / 100.0).clamp(0.02, 1.0))
        .round()
        .max(1.0) as u32;
    let opacity = (settings.opacity / 100.0).clamp(0.0, 1.0);
    let inset = settings.inset.max(0.0).round() as i32;
    let x = match settings.position.as_str() {
        "top-left" | "bottom-left" => inset.to_string(),
        _ => format!("W-w-{inset}"),
    };
    let y = match settings.position.as_str() {
        "top-left" | "top-right" => inset.to_string(),
        _ => format!("H-h-{inset}"),
    };
    let stage = format!(
        "[{watermark_input_index}:v]format=rgba,scale={scale_width}:-1,colorchannelmixer=aa={opacity:.4}[wm];{normalized_current}[wm]overlay=x={x}:y={y}:format=auto[vwm]"
    );
    let complex = match existing {
        Some(existing) if !existing.is_empty() => format!("{existing};{stage}"),
        _ => stage,
    };
    (complex, "[vwm]".into())
}

const EXPORT_STATE_EVENT: &str = "export-state";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExportStateEvent {
    export_id: String,
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    progress: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
}

impl ExportStateEvent {
    fn started(export_id: &str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status: "started",
            progress: None,
            path: None,
            message: None,
        }
    }

    fn progress(export_id: &str, progress: f64) -> Self {
        Self {
            export_id: export_id.to_string(),
            status: "progress",
            progress: Some(progress),
            path: None,
            message: None,
        }
    }

    fn finalizing(export_id: &str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status: "finalizing",
            progress: None,
            path: None,
            message: None,
        }
    }

    fn success(export_id: &str, path: &str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status: "success",
            progress: None,
            path: Some(path.to_string()),
            message: None,
        }
    }

    fn cancelled(export_id: &str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status: "cancelled",
            progress: None,
            path: None,
            message: None,
        }
    }

    fn error(export_id: &str, message: &str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status: "error",
            progress: None,
            path: None,
            message: Some(message.to_string()),
        }
    }
}

fn emit_export_state(app: &AppHandle, event: ExportStateEvent) {
    let _ = app.emit(EXPORT_STATE_EVENT, event);
}

#[tauri::command]
pub async fn get_video_metadata(path: String) -> Result<VideoMetadata, String> {
    // ffprobe spawn off the main thread — see generate_thumbnails for context.
    tauri::async_runtime::spawn_blocking(move || project_or_media_metadata(Path::new(&path)))
        .await
        .map_err(|e| format!("get_video_metadata join error: {e}"))?
}

#[tauri::command]
pub async fn load_editor_document(path: String) -> Result<EditorDocument, String> {
    tauri::async_runtime::spawn_blocking(move || load_editor_document_blocking(path))
        .await
        .map_err(|e| format!("load_editor_document join error: {e}"))?
}

fn load_editor_document_blocking(path: String) -> Result<EditorDocument, String> {
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
pub async fn generate_thumbnails(path: String, count: u32) -> Result<Vec<String>, String> {
    // Sync ffmpeg/ffprobe calls run on Tauri's main thread by default,
    // freezing the UI (clicks/touch/window-drag) for the duration. Move the
    // whole pipeline onto a blocking worker so the event loop stays free —
    // /recasts fires this once per recording in parallel from JS, and the
    // serialised main-thread ffmpeg spawns produced multi-second freezes.
    tauri::async_runtime::spawn_blocking(move || generate_thumbnails_blocking(path, count))
        .await
        .map_err(|e| format!("generate_thumbnails join error: {e}"))?
}

fn generate_thumbnails_blocking(path: String, count: u32) -> Result<Vec<String>, String> {
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
        // A single thumbnail is a poster frame: sample ~25% into the clip
        // rather than at 0s, since the first frame of a screen recording is
        // routinely black/blank. Multi-frame requests (the editor's timeline
        // strip) keep the even spread starting at 0.
        let timestamp = if count == 1 {
            meta.duration * 0.25
        } else {
            index as f64 * interval
        };
        let thumb_path = temp_dir.join(format!("thumb-{index}.jpg"));
        let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
        command.args([
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
        ]);
        crate::ffmpeg::configure_silent_command(&mut command);
        let result = command.output();

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

/// Pass 1 of the 2-pass GIF export. Consumes the source at the GIF's target
/// fps + scale and writes a single palette PNG. The main encode pass then
/// reads that palette as an external input and runs paletteuse on every
/// frame, which streams in real time so the progress bar actually moves.
///
/// Single-pass `palettegen → paletteuse` was stalling the UI: palettegen has
/// to consume every input frame before emitting its one output, so the
/// encoder's `out_time_us` stayed at 0 the entire palette phase and the bar
/// sat at 0% while only the elapsed counter ticked.
fn run_gif_palette_prepass(
    app: &AppHandle,
    export_id: &str,
    source_path: &Path,
    palette_path: &Path,
    trim_start: f64,
    duration: f64,
    source_duration: f64,
    options: GifFilterOptions<'_>,
    output_scale_filter: Option<&str>,
    cancel_flag: Arc<AtomicBool>,
    progress_offset: f64,
    progress_scale: f64,
) -> Result<(), String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
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
    args.extend([
        "-i".to_string(),
        source_path.to_string_lossy().to_string(),
    ]);

    let vf = build_gif_palette_prepass_filter(options, output_scale_filter);
    args.extend([
        "-vf".to_string(),
        vf,
        "-frames:v".to_string(),
        "1".to_string(),
        "-an".to_string(),
        palette_path.to_string_lossy().to_string(),
    ]);

    log::info!("export gif palette pre-pass args: {}", args.join(" "));

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args(&args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);

    let mut child = command
        .spawn()
        .map_err(|e| format!("failed to start ffmpeg palette pre-pass: {e}"))?;
    let mut stdout = child
        .stdout
        .take()
        .ok_or_else(|| "ffmpeg palette stdout pipe missing".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "ffmpeg palette stderr pipe missing".to_string())?;

    let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
    let stderr_buf_writer = stderr_buf.clone();
    let app_for_emit = app.clone();
    let export_id_for_emit = export_id.to_string();
    let effective_duration = if duration > 0.0 { duration } else { source_duration };

    let stderr_thread = std::thread::Builder::new()
        .name("recast-export-palette-stderr".into())
        .spawn(move || {
            let reader = std::io::BufReader::new(stderr);
            let mut last_emitted = -1.0_f64;
            for line in reader.lines().map_while(Result::ok) {
                if let Some(progress_secs) = parse_ffmpeg_progress_seconds(&line) {
                    if effective_duration > 0.0 {
                        let raw_pct =
                            (progress_secs / effective_duration * 100.0).clamp(0.0, 100.0);
                        let scaled = progress_offset + progress_scale * raw_pct;
                        if scaled > last_emitted + 0.5 {
                            last_emitted = scaled;
                            emit_export_state(
                                &app_for_emit,
                                ExportStateEvent::progress(&export_id_for_emit, scaled),
                            );
                        }
                    }
                    continue;
                }
                if line.trim() == "progress=end" || is_ffmpeg_progress_key_line(&line) {
                    continue;
                }
                let mut guard = stderr_buf_writer.lock();
                guard.extend_from_slice(line.as_bytes());
                guard.push(b'\n');
                if guard.len() > 8192 {
                    let overflow = guard.len() - 8192;
                    guard.drain(0..overflow);
                }
            }
        })
        .map_err(|e| format!("failed to spawn palette stderr drain: {e}"))?;

    let stdout_thread = std::thread::Builder::new()
        .name("recast-export-palette-stdout".into())
        .spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match stdout.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(_) => {}
                }
            }
        })
        .map_err(|e| format!("failed to spawn palette stdout drain: {e}"))?;

    // Poll cancel_flag while waiting for the child so a user cancel kills the
    // palette pre-pass mid-run instead of waiting for it to finish first.
    let exit_status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if cancel_flag.load(Ordering::Acquire) {
                    let _ = child.kill();
                    break Err("export cancelled".to_string());
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            Err(e) => break Err(format!("ffmpeg palette wait error: {e}")),
        }
    };

    let _ = stderr_thread.join();
    let _ = stdout_thread.join();

    match exit_status {
        Ok(status) => {
            if !status.success() {
                let stderr_bytes = stderr_buf.lock().clone();
                return Err(format!(
                    "export failed (palette pre-pass):\n{}",
                    summarize_ffmpeg_error(&stderr_bytes)
                ));
            }
            match std::fs::metadata(palette_path) {
                Ok(meta) if meta.len() > 0 => Ok(()),
                Ok(_) => Err("export failed: palette pre-pass wrote empty file".into()),
                Err(e) => Err(format!(
                    "export failed: palette pre-pass output missing: {e}"
                )),
            }
        }
        Err(e) => Err(e),
    }
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    request: ExportRequest,
    state: State<'_, AppState>,
) -> Result<String, String> {
    let export_id = request.export_id.clone();

    // Install a fresh cancellation token for this run, scoped to the export
    // session id that the frontend also uses to filter state events.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .export_cancel
        .lock()
        .insert(export_id.clone(), cancel_flag.clone());
    emit_export_state(&app, ExportStateEvent::started(&export_id));

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
    // Name the export after its source recording, with a Finder/Explorer-style
    // counter suffix (` (1)`, ` (2)`, …) when the same recording is exported
    // more than once — so exports stay searchable and easy to correlate.
    let source_stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Recast_export".to_string());
    let output_path = super::unique_path(&output_dir, &source_stem, extension);

    let asset_cache_dir = app
        .path()
        .app_data_dir()
        .ok()
        .map(|base| base.join("assets"));

    // Border-radius is stored as a 0..50 percentage of the shorter source edge.
    // Generate a single-frame alpha mask at source dimensions; the export plan
    // will alphamerge it onto the (zoomed) source video before background
    // composition so the rounded corners cut through to the background.
    let border_radius_pct = request.render_state.border_radius.clamp(0.0, 50.0);
    let border_radius_px = border_radius_pct / 100.0
        * metadata.width.min(metadata.height) as f64;
    let border_radius_mask: Option<MaskResult> = if border_radius_px > 0.5 {
        render_border_radius_mask(metadata.width, metadata.height, border_radius_px)
            .map_err(|e| format!("border-radius mask render failed: {e}"))?
    } else {
        None
    };
    let border_radius_mask_path = border_radius_mask.as_ref().map(|m| m.path.clone());

    // Canvas geometry feeds the drop-shadow rasteriser, the cursor
    // overlay PNG, and the FFmpeg filter graph. Compute once.
    //
    // Cursor and drop-shadow PNGs are rendered at COMP dims (= source +
    // padding * 2), not the final canvas dims. They're composited at the
    // comp's offset inside the canvas via FFmpeg overlay. Doing it the
    // other way piped a 1984×3528 RGBA stream for a 9:16 of 1080p
    // (~28 MB/frame at 60fps), which stalled the cursor sub-encode.
    let canvas_geom = crate::render::graph::compute_canvas_geometry(
        metadata.width,
        metadata.height,
        request.render_state.padding,
        request.render_state.output_aspect.as_deref(),
    );
    let canvas_width = canvas_geom.canvas_w;
    let canvas_height = canvas_geom.canvas_h;
    let canvas_padding = canvas_geom.padding_px;
    let comp_width = canvas_geom.comp_w;
    let comp_height = canvas_geom.comp_h;

    // Drop-shadow PNG: rasterised once and overlaid on the background by the
    // FFmpeg planner. Skipped when the user has disabled the effect or set
    // opacity to 0 — those gates are also enforced inside
    // `render_drop_shadow_mask`, but checking here saves the canvas-sized
    // allocation.
    let shadow_settings = &request.render_state.shadow;
    let drop_shadow_mask: Option<MaskResult> =
        if shadow_settings.enabled && shadow_settings.opacity > 0.0 {
            crate::render::mask_export::render_drop_shadow_mask(
                crate::render::mask_export::DropShadowRequest {
                    canvas_width: comp_width,
                    canvas_height: comp_height,
                    video_width: metadata.width,
                    video_height: metadata.height,
                    padding: canvas_padding,
                    video_border_radius: border_radius_px,
                    blur: shadow_settings.blur,
                    spread: shadow_settings.spread,
                    offset_y: shadow_settings.offset_y,
                    opacity: shadow_settings.opacity,
                    color: shadow_settings.color.clone(),
                },
            )
            .map_err(|e| format!("drop-shadow mask render failed: {e}"))?
        } else {
            None
        };
    let drop_shadow_mask_path = drop_shadow_mask.as_ref().map(|m| m.path.clone());

    let export_plan = graph
        .build_export_plan_with(
            SourceVideoMetadata {
                width: metadata.width,
                height: metadata.height,
            },
            &static_root(),
            1,
            asset_cache_dir.as_deref(),
            border_radius_mask_path,
            drop_shadow_mask_path,
            canvas_geom,
        )
        .map_err(|e| e.to_string())?;
    let overlay_duration = if duration > 0.0 {
        duration
    } else {
        source_duration
    };
    let needs_overlay = request.render_state.cursor_enabled
        || !request.render_state.annotations.is_empty()
        || (request.render_state.shadow.enabled && request.render_state.shadow.opacity > 0.0);
    let cursor_overlay = if needs_overlay && overlay_duration > 0.0 {
        project
            .as_ref()
            .map(|project| {
                render_cursor_overlay(CursorOverlayRequest {
                    cursor_track_path: project.cursor_path.clone(),
                    canvas_width: comp_width,
                    canvas_height: comp_height,
                    source_width: metadata.width,
                    source_height: metadata.height,
                    padding: canvas_padding,
                    fps: metadata.fps.round().max(1.0) as u32,
                    duration_secs: overlay_duration,
                    trim_start,
                    render_state: request.render_state.clone(),
                })
            })
            .transpose()
            .map_err(|e| e.to_string())?
    } else {
        None
    };

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

    let watermark_path = if request.render_state.watermark_settings.enabled
        && !request
            .render_state
            .watermark_settings
            .image_path
            .trim()
            .is_empty()
    {
        let path = PathBuf::from(request.render_state.watermark_settings.image_path.trim());
        path.exists().then_some(path)
    } else {
        None
    };
    let watermark_input_index = watermark_path
        .as_ref()
        .map(|_| 1 + export_plan.extra_inputs.len() + cursor_overlay_path.is_some() as usize);
    if let Some(ref path) = watermark_path {
        args.extend([
            "-loop".to_string(),
            "1".to_string(),
            "-i".to_string(),
            path.to_string_lossy().to_string(),
        ]);
    }

    //  Camera overlay
    //
    // Composite the project's `camera.mp4` onto the screen video at the
    // bubble's UV-space placement. Coordinates mirror `CameraOverlay.svelte`
    // exactly so preview and export agree to the pixel:
    //   - bubble_w == bubble_h (Phase 1 enforces 1:1 in CSS)
    //   - dimensions derived from `video_w` so the bubble is square in
    //     screen pixels regardless of source aspect
    //   - position offset by `video_x/video_y` so padding doesn't bias the
    //     placement
    //
    // Shape clipping is done via a one-shot rounded-rect alpha mask
    // rendered at bubble dimensions and `alphamerge`d with the camera
    // stream. Square shape skips the mask entirely.
    let camera_overlay_settings = &request.render_state.camera_overlay;
    let camera_path = if camera_overlay_settings.enabled {
        project
            .as_ref()
            .and_then(|p| p.camera_path.clone())
            .filter(|p| p.exists())
    } else {
        None
    };
    let camera_bubble: Option<(PathBuf, u32, u32, u32, u32)> =
        if let Some(ref path) = camera_path {
            let p = &camera_overlay_settings.default_placement;
            // Use video_w as the size base so the bubble is square in
            // screen pixels (matches `aspect-ratio: 1` in the preview).
            let bubble_w = (p.width.clamp(0.02, 1.0) * canvas_geom.video_w as f64)
                .round()
                .max(2.0) as u32;
            let bubble_h = bubble_w;
            // Clamp into the canvas so an out-of-range placement (legacy
            // project, manual JSON edit) still produces a valid overlay.
            let max_x = canvas_geom.canvas_w.saturating_sub(bubble_w);
            let max_y = canvas_geom.canvas_h.saturating_sub(bubble_h);
            let bubble_x = ((canvas_geom.video_x as f64
                + p.x.clamp(0.0, 1.0) * canvas_geom.video_w as f64)
                .round() as u32)
                .min(max_x);
            let bubble_y = ((canvas_geom.video_y as f64
                + p.y.clamp(0.0, 1.0) * canvas_geom.video_h as f64)
                .round() as u32)
                .min(max_y);
            Some((path.clone(), bubble_x, bubble_y, bubble_w, bubble_h))
        } else {
            None
        };

    // Pre-render the rounded-rect mask matching the bubble's shape. Square
    // shape needs no mask (mask_input_index stays None and the filter chain
    // skips the alphamerge stage).
    let camera_mask: Option<MaskResult> = if let Some(&(_, _, _, bw, bh)) = camera_bubble.as_ref() {
        let radius_px = match camera_overlay_settings.shape.as_str() {
            "circle" => bw as f64 / 2.0,
            "square" | "rectangle" => 0.0,
            _ => camera_overlay_settings.corner_radius * bw as f64,
        };
        if radius_px > 0.5 {
            crate::render::mask_export::render_border_radius_mask(bw, bh, radius_px)
                .map_err(|e| format!("camera mask render failed: {e}"))?
        } else {
            None
        }
    } else {
        None
    };
    let camera_mask_path = camera_mask.as_ref().map(|m| m.path.clone());

    let camera_input_index = camera_bubble.as_ref().map(|_| {
        1 + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
            + watermark_path.is_some() as usize
    });
    if let Some((ref path, _, _, _, _)) = camera_bubble {
        args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
    }
    let camera_mask_input_index = camera_mask_path.as_ref().map(|_| {
        1 + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
            + watermark_path.is_some() as usize
            + camera_input_index.is_some() as usize
    });
    if let Some(ref path) = camera_mask_path {
        args.extend([
            "-loop".to_string(),
            "1".to_string(),
            "-i".to_string(),
            path.to_string_lossy().to_string(),
        ]);
    }

    let mut audio_input_indices = Vec::new();
    let source_has_audio = has_audio(&source_video);
    if request.format != "gif" && source_has_audio {
        audio_input_indices.push(0);
    }
    if request.format != "gif" {
        if let Some(project) = project.as_ref() {
            let mut next_audio_input_index = 1
                + export_plan.extra_inputs.len()
                + cursor_overlay_path.is_some() as usize
                + watermark_path.is_some() as usize
                + camera_input_index.is_some() as usize
                + camera_mask_input_index.is_some() as usize;
            for path in [&project.audio_path, &project.microphone_path]
                .into_iter()
                .flatten()
                .filter(|path| path.exists())
            {
                audio_input_indices.push(next_audio_input_index);
                next_audio_input_index += 1;
                args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
            }
        }
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
                canvas_geom.comp_x,
                canvas_geom.comp_y,
            );
            (Some(new_complex), new_map)
        } else {
            (initial_filter_complex, initial_video_map)
        };

    if let Some(watermark_input_index) = watermark_input_index {
        let (new_complex, new_map) = append_watermark_to_complex(
            filter_complex_after_cursor.as_deref(),
            &video_map_after_cursor,
            watermark_input_index,
            &request.render_state.watermark_settings,
            canvas_width,
            canvas_height,
        );
        filter_complex_after_cursor = Some(new_complex);
        video_map_after_cursor = new_map;
    }

    // Camera overlay: composited after the watermark so the speaker bubble
    // sits on top of any branding mark and below the annotation blur (which
    // a user might want to apply over their own face).
    if let (Some(cam_idx), Some((_, bx, by, bw, bh))) =
        (camera_input_index, camera_bubble.as_ref())
    {
        let (new_complex, new_map) = append_camera_overlay_to_complex(
            filter_complex_after_cursor.as_deref(),
            &video_map_after_cursor,
            &CameraOverlayParams {
                camera_input_index: cam_idx,
                mask_input_index: camera_mask_input_index,
                bubble_x: *bx,
                bubble_y: *by,
                bubble_w: *bw,
                bubble_h: *bh,
                mirror: camera_overlay_settings.mirror,
            },
        );
        filter_complex_after_cursor = Some(new_complex);
        video_map_after_cursor = new_map;
    }

    // Annotation blur regions — applied AFTER the cursor overlay so the blur
    // sits over the composited cursor too (same z-order as in the preview),
    // but BEFORE GIF palettization so the palette captures the blurred pixels.
    let blur_regions: Vec<BlurRegion> = request
        .render_state
        .annotations
        .iter()
        .filter(|a| !a.hidden)
        .filter_map(|a| match &a.kind {
            AnnotationKind::Blur {
                x,
                y,
                w,
                h,
                strength,
                variant,
                tint_color,
                ..
            } => {
                // UV → canvas-pixel rect.
                let cx = (x * canvas_width as f64).round() as i32;
                let cy = (y * canvas_height as f64).round() as i32;
                let cw = (w.abs() * canvas_width as f64).round() as i32;
                let ch = (h.abs() * canvas_height as f64).round() as i32;
                if cw < 4 || ch < 4 {
                    return None;
                }
                // Strength 0..1 → kernel radius up to 12% of the shorter edge,
                // clamped at FFmpeg boxblur's hard max of 127. Mirrors
                // ffmpeg.rs::make_blur_region — both paths must agree so the
                // export and editor previews match.
                let max_dim = canvas_width.min(canvas_height) as f64 * 0.12;
                let radius = (strength.clamp(0.0, 1.0) * max_dim)
                    .round()
                    .clamp(1.0, 127.0) as u32;
                let tint_rgb = u32::from_str_radix(tint_color.trim_start_matches('#'), 16)
                    .unwrap_or(0x000000);
                Some(BlurRegion {
                    x: cx,
                    y: cy,
                    w: cw,
                    h: ch,
                    radius,
                    start_secs: a.start - trim_start,
                    end_secs: a.end - trim_start,
                    variant: variant.as_str(),
                    tint_rgb,
                    opacity: a.opacity.clamp(0.0, 1.0),
                    strength: strength.clamp(0.0, 1.0),
                })
            }
            _ => None,
        })
        .collect();
    if !blur_regions.is_empty() {
        let (new_complex, new_map) = build_annotation_blur_complex(
            filter_complex_after_cursor.as_deref(),
            &video_map_after_cursor,
            &blur_regions,
        );
        filter_complex_after_cursor = Some(new_complex);
        video_map_after_cursor = new_map;
    }

    // For GIF, route through a 2-pass pipeline. Pass 1 here (synchronous,
    // before the main spawn_blocking) generates the palette PNG so the main
    // pass can use a paletteuse-only chain. The single-pass alternative
    // (`split→palettegen/paletteuse` in one filter graph) buffers every input
    // frame inside palettegen before emitting the palette, so the encoder's
    // `out_time_us` stays at 0 the entire palette phase — the UI sat at 0%
    // while only the elapsed counter moved. Splitting the passes lets us
    // emit real progress: pre-pass owns 0..40%, main pass owns 40..100%.
    let mut output_filters: Vec<String> = Vec::new();
    let gif_settings: GifSettings = request.gif_settings.clone().unwrap_or_default();
    let mut palette_temp_path: Option<PathBuf> = None;
    let (progress_offset, progress_scale) = if request.format == "gif" {
        let resolved_fps = gif_settings.fps.unwrap_or(profile.gif_fps);
        let gif_max_colors = gif_settings.max_colors();
        // `GifFilterOptions` holds a `&str` for dither, so we can't build the
        // struct here and then move it into a `'static` spawn_blocking closure.
        // Stash the owned String, reconstruct the struct inside each closure.
        let gif_dither_owned: String = gif_settings.dither.clone();

        // Transient 2-pass palette file — unique per run so concurrent exports
        // don't clobber each other's palette.
        let palette_stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let palette_path = output_dir.join(format!(
            "recast_palette_{palette_stamp}_{}.png",
            std::process::id()
        ));

        let app_for_prepass = app.clone();
        let export_id_for_prepass = export_id.clone();
        let source_for_prepass = source_video.clone();
        let palette_for_prepass = palette_path.clone();
        let cancel_for_prepass = cancel_flag.clone();
        let scale_for_prepass = output_scale_filter.clone();
        let dither_for_prepass = gif_dither_owned.clone();
        let prepass_result = tokio::task::spawn_blocking(move || {
            let inner_options = GifFilterOptions {
                fps: resolved_fps,
                max_colors: gif_max_colors,
                dither: dither_for_prepass.as_str(),
            };
            run_gif_palette_prepass(
                &app_for_prepass,
                &export_id_for_prepass,
                &source_for_prepass,
                &palette_for_prepass,
                trim_start,
                duration,
                source_duration,
                inner_options,
                scale_for_prepass.as_deref(),
                cancel_for_prepass,
                0.0,
                0.4,
            )
        })
        .await;

        match prepass_result {
            Ok(Ok(())) => {}
            Ok(Err(err_msg)) => {
                state.export_cancel.lock().remove(&export_id);
                let _ = std::fs::remove_file(&palette_path);
                if cancel_flag.load(Ordering::Acquire) {
                    emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
                    return Err("export cancelled".to_string());
                }
                emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
                return Err(err_msg);
            }
            Err(join_err) => {
                state.export_cancel.lock().remove(&export_id);
                let _ = std::fs::remove_file(&palette_path);
                let err_msg = format!("export task failed (palette pre-pass): {join_err}");
                emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
                return Err(err_msg);
            }
        }

        if cancel_flag.load(Ordering::Acquire) {
            state.export_cancel.lock().remove(&export_id);
            let _ = std::fs::remove_file(&palette_path);
            emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
            return Err("export cancelled".to_string());
        }

        // Wire the palette PNG in as the last FFmpeg input. GIF mode skips
        // audio inputs entirely, so input ordering up to this point is:
        //   0=source, 1..=extra_inputs, [cursor], [watermark]
        // Palette appends after that.
        let palette_input_index = 1
            + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
            + watermark_path.is_some() as usize;
        args.extend([
            "-i".to_string(),
            palette_path.to_string_lossy().to_string(),
        ]);

        let pass2_options = GifFilterOptions {
            fps: resolved_fps,
            max_colors: gif_max_colors,
            dither: gif_dither_owned.as_str(),
        };
        let (gif_complex, gif_map) = build_gif_paletteuse_external_complex(
            filter_complex_after_cursor.as_deref(),
            &video_map_after_cursor,
            palette_input_index,
            pass2_options,
            output_scale_filter.as_deref(),
        );
        filter_complex_after_cursor = Some(gif_complex);
        video_map_after_cursor = gif_map;
        palette_temp_path = Some(palette_path);

        (40.0_f64, 0.6_f64)
    } else {
        if let Some(scale_filter) = output_scale_filter {
            output_filters.push(scale_filter);
        }
        (0.0_f64, 1.0_f64)
    };

    let audio_map = if request.format == "gif" {
        None
    } else {
        append_audio_to_complex(
            filter_complex_after_cursor.as_deref(),
            &audio_input_indices,
            &request.render_state.audio_settings,
            trim_start,
            duration,
        )
        .map(|(new_complex, map)| {
            filter_complex_after_cursor = Some(new_complex);
            map
        })
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

    if let Some(ref audio_map) = audio_map {
        args.extend(["-map".to_string(), audio_map.clone()]);
    }

    if !output_filters.is_empty() && filter_complex_after_cursor.is_none() {
        args.extend(["-vf".to_string(), output_filters.join(",")]);
    }

    // The input-side `-t` above trims the source media, but filtergraph
    // generators such as `color=...` are infinite by default. Add an
    // output-side duration cap so background/composite exports stop after the
    // requested timeline duration instead of encoding forever.
    if duration > 0.0 {
        args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }

    if duration <= 0.0 {
        if !export_plan.extra_inputs.is_empty() || cursor_overlay_path.is_some() {
            args.push("-shortest".to_string());
        }
    }

    match request.format.as_str() {
        "gif" => {
            // Explicit `-c:v gif` + `-f gif` keeps FFmpeg from probing the
            // output container and falling back to an unrelated codec on
            // some Windows builds — we've seen the auto-detect path emit
            // "Could not find tag for codec none" when the filter chain
            // ends in a labelled output rather than the default sink.
            // `-vsync 0` (a.k.a. `-fps_mode passthrough`) honours the
            // exact frame timing produced by the in-graph `fps=` filter
            // instead of FFmpeg's downstream resampler nudging frames
            // around, which previously produced 0-byte GIFs when the
            // composite framerate didn't divide evenly.
            args.extend([
                "-c:v".to_string(),
                "gif".to_string(),
                "-f".to_string(),
                "gif".to_string(),
                "-an".to_string(),
                "-vsync".to_string(),
                "0".to_string(),
                "-loop".to_string(),
                gif_settings.ffmpeg_loop_arg().to_string(),
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
            if audio_map.is_some() {
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
            if audio_map.is_some() {
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
    let export_id_for_task = export_id.clone();
    let export_id_for_fallback = export_id.clone();
    let task_result = tokio::task::spawn_blocking(move || {
        let export_id = export_id_for_task;
        let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
        command
            .args(&args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::ffmpeg::configure_silent_command(&mut command);

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
        let last_progress_secs = Arc::new(Mutex::new(-1.0_f64));
        let killed_by_timeout = Arc::new(AtomicBool::new(false));
        let killed_by_user = Arc::new(AtomicBool::new(false));
        let finalizing_seen = Arc::new(AtomicBool::new(false));
        let near_end_seen = Arc::new(AtomicBool::new(false));
        let progress_end_seen = Arc::new(AtomicBool::new(false));
        // Latched the first time the stderr parser parses a progress block.
        // The watchdog uses this to apply a longer budget during ffmpeg's
        // cold-start window (filter_complex parse, NVENC surface alloc, VP9
        // first-pass init) before falling back to the tighter steady-state
        // timeout once frames start flowing.
        let first_progress_seen = Arc::new(AtomicBool::new(false));

        // Parse stderr line-by-line. Progress blocks (key=value lines) get
        // filtered out; only genuine log output is appended to the 8 KB error
        // ring buffer used for post-mortem in the failure path. `out_time_us=`
        // lines drive the UI `export-progress` emits, and `progress=end`
        // signals the encoder has finished and only the mux trailer remains.
        let stderr_buf: Arc<Mutex<Vec<u8>>> = Arc::new(Mutex::new(Vec::new()));
        let stderr_buf_writer = stderr_buf.clone();
        let stderr_last_progress = last_progress.clone();
        let stderr_last_progress_secs = last_progress_secs.clone();
        let stderr_app = app.clone();
        let stderr_export_id = export_id.clone();
        let stderr_finalizing_seen = finalizing_seen.clone();
        let stderr_near_end_seen = near_end_seen.clone();
        let stderr_progress_end_seen = progress_end_seen.clone();
        let stderr_first_progress_seen = first_progress_seen.clone();
        let encode_started_at = Instant::now();
        let stderr_thread = std::thread::Builder::new()
            .name("recast-export-stderr".into())
            .spawn(move || {
                let reader = std::io::BufReader::new(stderr);
                let mut logged_near_done = false;
                for line in reader.lines().map_while(Result::ok) {
                    // FFmpeg progress blocks are key=value lines terminated by
                    // `progress=continue` (between blocks) or `progress=end`
                    // (final block). Treat all of these as non-log noise.
                    if let Some(progress_secs) = parse_ffmpeg_progress_seconds(&line) {
                        let effective_duration = if duration > 0.0 {
                            duration
                        } else {
                            source_duration
                        };
                        // Watchdog proof-of-life: any parseable progress line
                        // means ffmpeg is alive. Don't gate this on out_time
                        // advancing — on Windows/NVENC we regularly see
                        // back-to-back blocks with unchanged `out_time_us`
                        // while surfaces flush or a GOP is primed, and
                        // waiting for advancement starved the watchdog reset.
                        {
                            let mut guard = stderr_last_progress.lock();
                            *guard = Instant::now();
                        }
                        // First progress line ever → flip the startup-grace
                        // flag and log it so post-mortems can see how long
                        // filter_complex/NVENC warmup took.
                        if !stderr_first_progress_seen.swap(true, Ordering::AcqRel) {
                            log::info!(
                                "export: first progress parsed at T+{}ms",
                                encode_started_at.elapsed().as_millis()
                            );
                        }
                        // UI emit gate: only publish a new pct when out_time
                        // actually advanced. Redundant emits would spam the
                        // progress bar with the same value.
                        let advanced = {
                            let mut last_secs = stderr_last_progress_secs.lock();
                            if progress_secs > *last_secs + 0.01 {
                                *last_secs = progress_secs;
                                true
                            } else {
                                false
                            }
                        };
                        if !advanced {
                            continue;
                        }
                        let pct = if effective_duration > 0.0 {
                            (progress_secs / effective_duration * 100.0).clamp(0.0, 100.0)
                        } else {
                            0.0
                        };
                        if effective_duration > 0.0
                            && (effective_duration - progress_secs).max(0.0) <= 0.25
                        {
                            stderr_near_end_seen.store(true, Ordering::Release);
                        }
                        // Log the moment we cross 99.5% so post-mortems of
                        // "stuck at 99%" reports can locate the gap between
                        // here and the eventual `progress=end` / drain-thread
                        // exit in the captured stderr tail.
                        if !logged_near_done && pct >= 99.5 {
                            logged_near_done = true;
                            log::info!(
                                "export: reached {:.1}% at T+{}ms, awaiting progress=end",
                                pct,
                                encode_started_at.elapsed().as_millis()
                            );
                        }
                        // For 2-pass GIF the pre-pass owns 0..40% and this
                        // pass owns 40..100%; for everything else it's 0..100.
                        // Scaling here (vs. at every progress emit site) keeps
                        // the 100% terminal emits below honest — they always
                        // mean "done", not "60% done because we're in pass 2".
                        let scaled_pct = progress_offset + progress_scale * pct;
                        emit_export_state(
                            &stderr_app,
                            ExportStateEvent::progress(&stderr_export_id, scaled_pct),
                        );
                        continue;
                    }
                    // `progress=end` means FFmpeg has finished encoding and
                    // is about to write the container trailer / exit. Flip
                    // the UI to finalizing NOW rather than waiting for the
                    // pipes to close — on Windows stderr close can lag the
                    // actual encoder finish by seconds, which manifested as
                    // the bar sitting at 100% with no state change. Also
                    // stamp `last_progress` so the watchdog gives the trailer
                    // write its own fresh budget.
                    if line.trim() == "progress=end" {
                        stderr_progress_end_seen.store(true, Ordering::Release);
                        if !stderr_finalizing_seen.swap(true, Ordering::AcqRel) {
                            emit_export_state(
                                &stderr_app,
                                ExportStateEvent::progress(&stderr_export_id, 100.0_f64),
                            );
                            emit_export_state(
                                &stderr_app,
                                ExportStateEvent::finalizing(&stderr_export_id),
                            );
                            log::info!(
                                "export: progress=end seen at T+{}ms, flipping UI to finalizing",
                                encode_started_at.elapsed().as_millis()
                            );
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
                log::info!(
                    "export: stderr thread exiting at T+{}ms (pipe closed)",
                    encode_started_at.elapsed().as_millis()
                );
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
        // mid-encode and leaving it there. Finalization is now reserved for
        // FFmpeg's explicit `progress=end` signal.
        let watchdog_last_progress = last_progress.clone();
        let watchdog_killed = killed_by_timeout.clone();
        let watchdog_cancel_flag = cancel_flag.clone();
        let watchdog_user_kill = killed_by_user.clone();
        let watchdog_near_end_seen = near_end_seen.clone();
        let watchdog_progress_end_seen = progress_end_seen.clone();
        let watchdog_first_progress_seen = first_progress_seen.clone();
        let watchdog_stop = Arc::new(AtomicBool::new(false));
        let watchdog_stop_flag = watchdog_stop.clone();
        // Share the child with the watchdog via a mutex so it can call kill().
        let child_handle = Arc::new(Mutex::new(Some(child)));
        let watchdog_child = child_handle.clone();
        let watchdog_output_path = output_path_str.clone();
        let watchdog_thread = std::thread::Builder::new()
            .name("recast-export-watchdog".into())
            .spawn(move || {
                const ENCODE_TIMEOUT: Duration = Duration::from_secs(60);
                const NEAR_END_TIMEOUT: Duration = Duration::from_secs(20);
                // Startup grace: ffmpeg can take a long time to emit its
                // first progress block when filter_complex parsing, NVENC
                // surface allocation, or VP9 first-pass init runs before
                // the first frame is output. Use a bigger budget until
                // that first progress line arrives, then fall back to
                // ENCODE_TIMEOUT for steady state.
                const FIRST_PROGRESS_TIMEOUT: Duration = Duration::from_secs(120);
                // `FINALIZING_TIMEOUT` is a *no-file-growth* bound, not a
                // wall-clock cap on the finalizing phase. While FFmpeg is
                // legitimately writing the mux trailer the output file grows
                // continuously — we watch for that below and stamp
                // `watchdog_last_progress` on every size increase, so slow-
                // but-productive trailer writes keep us out of the timeout.
                // 60s of *no growth whatsoever* is a real stall.
                const FINALIZING_TIMEOUT: Duration = Duration::from_secs(60);
                const POLL_INTERVAL: Duration = Duration::from_millis(250);
                let mut last_file_size: u64 = 0;
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
                    let in_finalizing = watchdog_progress_end_seen.load(Ordering::Acquire);
                    // File-size growth as a liveness signal. Applies in both
                    // phases: during the encode the output file is already
                    // being written as GOPs complete, and during finalizing
                    // the trailer mux continues to grow the file. If the
                    // file is growing we know ffmpeg is alive and productive,
                    // regardless of whether the stderr progress thread has
                    // been able to refresh the stamp yet.
                    if let Ok(meta) = std::fs::metadata(&watchdog_output_path) {
                        let size = meta.len();
                        if size > last_file_size {
                            last_file_size = size;
                            let mut guard = watchdog_last_progress.lock();
                            *guard = Instant::now();
                        }
                    }
                    let elapsed = {
                        let guard = watchdog_last_progress.lock();
                        guard.elapsed()
                    };
                    let near_end = watchdog_near_end_seen.load(Ordering::Acquire);
                    let first_seen = watchdog_first_progress_seen.load(Ordering::Acquire);
                    let allowed_idle = if in_finalizing {
                        FINALIZING_TIMEOUT
                    } else if near_end {
                        NEAR_END_TIMEOUT
                    } else if !first_seen {
                        FIRST_PROGRESS_TIMEOUT
                    } else {
                        ENCODE_TIMEOUT
                    };
                    if elapsed > allowed_idle {
                        let mut guard = watchdog_child.lock();
                        if let Some(ref mut child) = *guard {
                            let total_elapsed = encode_started_at.elapsed().as_millis();
                            if in_finalizing {
                                log::warn!(
                                    "export watchdog: killing ffmpeg after progress=end at T+{}ms; no exit for {:?}",
                                    total_elapsed,
                                    elapsed
                                );
                            } else if near_end {
                                log::warn!(
                                    "export watchdog: killing ffmpeg near end of encode at T+{}ms; progress stopped for {:?}",
                                    total_elapsed,
                                    elapsed
                                );
                            } else {
                                log::warn!(
                                    "export watchdog: killing stalled ffmpeg at T+{}ms (no progress for {:?})",
                                    total_elapsed,
                                    elapsed
                                );
                            }
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
        log::info!(
            "export: drain threads joined at T+{}ms (pipes closed)",
            encode_started_at.elapsed().as_millis()
        );

        // Redundant-but-idempotent final emit: if `progress=end` wasn't seen
        // (e.g. FFmpeg was killed before finishing), make sure the UI still
        // gets a finalizing flip before `export-done` arrives so the dialog
        // has a consistent visual sequence.
        if !killed_by_user.load(Ordering::Acquire)
            && !killed_by_timeout.load(Ordering::Acquire)
            && !finalizing_seen.swap(true, Ordering::AcqRel)
        {
            emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
            emit_export_state(&app, ExportStateEvent::finalizing(&export_id));
        }

        // Stop the watchdog now that the I/O is done.
        watchdog_stop.store(true, Ordering::Release);
        let _ = watchdog_thread.join();

        let expected_output_duration = if duration > 0.0 {
            duration
        } else {
            source_duration
        };

        // Pipes are closed, which means ffmpeg has finished writing the file.
        // Probe the output NOW and, if it's usable, emit `success` to the UI
        // immediately — we should not make the user watch "Writing video
        // file…" while we wait for the OS to reap the child process. On
        // Windows that reap can legitimately take hundreds of ms to a couple
        // of seconds after stdio close. The reap still happens below, but
        // its only job now is to reap cleanly; its latency no longer blocks
        // the user-visible completion.
        let early_success_emitted = if !killed_by_user.load(Ordering::Acquire)
            && !killed_by_timeout.load(Ordering::Acquire)
            && progress_end_seen.load(Ordering::Acquire)
            && completed_export_looks_usable(
                Path::new(&output_path_str),
                expected_output_duration,
            ) {
            log::info!(
                "export: pipes closed and output probe ok at T+{}ms; emitting success early and reaping child",
                encode_started_at.elapsed().as_millis()
            );
            emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
            emit_export_state(&app, ExportStateEvent::success(&export_id, &output_path_str));
            true
        } else {
            false
        };

        // Pull the child back out and wait for its exit status. Stdout has
        // already closed, so FFmpeg should be on its last gasp (trailer write +
        // teardown). A well-behaved exit happens within milliseconds. We still
        // bound the wait with a hard timeout — if it takes longer than
        // POST_CLOSE_TIMEOUT we force-kill so the ffmpeg process doesn't leak.
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
                            "export post-close wait exceeded {:?} at T+{}ms; force-killing ffmpeg",
                            POST_CLOSE_TIMEOUT,
                            encode_started_at.elapsed().as_millis()
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
        log::info!(
            "export: child exited at T+{}ms (status={:?}, forced_exit={}, early_success_emitted={})",
            encode_started_at.elapsed().as_millis(),
            status.code(),
            forced_exit,
            early_success_emitted
        );

        // If we already told the UI the export succeeded based on the probe
        // of a fully-written file, the reap outcome (clean exit or forced
        // kill) is bookkeeping — the file is good either way. Return Ok so
        // the caller's Promise resolves cleanly.
        if early_success_emitted {
            return Ok(output_path_str);
        }

        if forced_exit {
            let output_path = Path::new(&output_path_str);
            // Force-kill happens only after the I/O drain threads exited
            // (pipes already closed = FFmpeg finished writing) AND we waited
            // POST_CLOSE_TIMEOUT for the process to reap. If `progress_end`
            // was seen, the encoder definitely got through the trailer write
            // before this point — the salvage probe then confirms the file is
            // playable. Without `progress_end` we can't trust the output even
            // if probe succeeds; refuse rather than ship a corrupted file.
            let encode_completed = progress_end_seen.load(Ordering::Acquire);
            if encode_completed
                && completed_export_looks_usable(output_path, expected_output_duration)
            {
                log::warn!(
                    "export: ffmpeg was force-killed after post-close timeout, but progress=end was seen and output looks usable; treating as success"
                );
                emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
                emit_export_state(&app, ExportStateEvent::success(&export_id, &output_path_str));
                return Ok(output_path_str);
            }

            let _ = std::fs::remove_file(output_path);
            let err_msg = format!(
                "export failed: ffmpeg did not exit within {}s of finishing the encode",
                POST_CLOSE_TIMEOUT.as_secs()
            );
            emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
            return Err(err_msg);
        }

        if killed_by_user.load(Ordering::Acquire) {
            // Clean up the half-written output file so the exports list doesn't
            // show a broken artifact from the aborted run.
            let _ = std::fs::remove_file(&output_path_str);
            emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
            return Err("export cancelled".to_string());
        }

        if killed_by_timeout.load(Ordering::Acquire) {
            let output_path = Path::new(&output_path_str);
            // Salvage path: only trust the on-disk file if FFmpeg actually
            // signalled `progress=end` before the watchdog fired. That means
            // the encoder finished writing every frame and we killed it
            // partway through the trailer write — `completed_export_looks_usable`
            // can probe successfully on the partial mux result, but the moov
            // atom may be incomplete. Without `progress=end` we were killed
            // mid-encode and the output is almost certainly truncated;
            // refuse to surface a corrupted file as a successful export.
            let encode_completed = progress_end_seen.load(Ordering::Acquire);
            if encode_completed
                && completed_export_looks_usable(output_path, expected_output_duration)
            {
                log::warn!(
                    "export: watchdog killed ffmpeg after progress=end; output looks usable, treating as success"
                );
                emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
                emit_export_state(&app, ExportStateEvent::success(&export_id, &output_path_str));
                return Ok(output_path_str);
            }

            let _ = std::fs::remove_file(output_path);
            let base_msg = if encode_completed {
                "export failed: ffmpeg reached finalizing but the output file stopped growing for 60s"
            } else if near_end_seen.load(Ordering::Acquire) {
                "export failed: ffmpeg stopped making progress near the end of the encode"
            } else {
                "export timed out: ffmpeg produced no progress for 60s"
            };
            // Surface whatever ffmpeg last said so this error is actionable
            // without needing to re-instrument. The stderr ring buffer holds
            // up to 8 KB; take the final line (or two) to keep the message
            // scannable.
            let stderr_tail = {
                let guard = stderr_buf.lock();
                let text = String::from_utf8_lossy(&guard).into_owned();
                text.lines()
                    .rev()
                    .filter(|l| !l.trim().is_empty())
                    .take(2)
                    .collect::<Vec<_>>()
                    .into_iter()
                    .rev()
                    .collect::<Vec<_>>()
                    .join(" | ")
            };
            let err_msg = if stderr_tail.is_empty() {
                base_msg.to_string()
            } else {
                format!("{base_msg} — last stderr: {stderr_tail}")
            };
            emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
            return Err(err_msg);
        }

        if !status.success() {
            let stderr_bytes = stderr_buf.lock().clone();
            let _ = std::fs::remove_file(&output_path_str);
            let err_msg = format!(
                "export failed:\n{}",
                summarize_ffmpeg_error(&stderr_bytes)
            );
            emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
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

        // On the happy path (status 0 + progress=end observed) we trust
        // FFmpeg's own exit as the integrity signal — spawning ffprobe here
        // just to re-verify what we already know would park the UI in
        // "Finalizing…" for the duration of that probe, which is exactly the
        // hang symptom users hit. Corruption guards remain on the salvage
        // paths above (force-kill, watchdog-kill) where the exit code isn't
        // trustworthy. `_expected_output_duration` kept in scope to make the
        // salvage branches' dependency explicit.
        let _ = expected_output_duration;

        // Final 100% ping + an `export-done` event with the result. The
        // frontend uses `export-done` to transition the dialog to the success
        // state immediately — decoupled from the `exportVideo` Promise, which
        // may take an extra beat to resolve through Tauri's IPC layer.
        emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0_f64));
        emit_export_state(&app, ExportStateEvent::success(&export_id, &output_path_str));
        log::info!(
            "export: success emitted at T+{}ms for {output_path_str}",
            encode_started_at.elapsed().as_millis()
        );
        Ok(output_path_str)
    })
    .await;

    // Cleanup must run regardless of whether the task returned Ok/Err or even
    // panicked — otherwise a panic would leak the cursor overlay's temp dir and
    // leave a stale cancel token installed that would poison the next export.
    drop(cursor_overlay);
    state.export_cancel.lock().remove(&export_id);
    if let Some(p) = palette_temp_path.as_ref() {
        let _ = std::fs::remove_file(p);
    }

    match task_result {
        Ok(inner) => inner,
        Err(join_err) => {
            // spawn_blocking only errors on panic; surface it so the frontend
            // can show a real failure dialog instead of hanging on the Promise.
            let err_msg = format!("export task failed: {join_err}");
            emit_export_state(
                &app_for_fallback,
                ExportStateEvent::error(&export_id_for_fallback, &err_msg),
            );
            Err(err_msg)
        }
    }
}

/// Signal any running export to abort. The watchdog thread polls this flag every
/// ~250ms and kills the ffmpeg child process, which causes `export_video` to
/// return `Err("export cancelled")`. Safe to call when no export is running
/// for the given export session id.
#[tauri::command]
pub fn cancel_export(export_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if let Some(flag) = state.export_cancel.lock().get(&export_id) {
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
pub async fn save_project_edits(project_path: String, edits_json: String) -> Result<u64, String> {
    let path_for_blocking = project_path.clone();
    tokio::task::spawn_blocking(move || {
        crate::project::writer::update_project_edits(Path::new(&path_for_blocking), &edits_json)
    })
    .await
    .map_err(|e| format!("save task panicked: {e}"))?
    .map_err(|e| e.to_string())?;

    // Autosave shadow is now redundant — the on-disk project matches memory.
    crate::project::autosave::clear_autosave(Path::new(&project_path));

    let saved_at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    Ok(saved_at)
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
pub fn suggest_zoom_regions(
    cursor_path: String,
) -> Result<Vec<crate::cursor::smoothing::ZoomTrigger>, String> {
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

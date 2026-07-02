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
    append_output_filters_to_complex, append_subtitles_to_complex, build_annotation_blur_complex,
    build_gif_palette_prepass_filter, build_gif_paletteuse_external_complex,
    build_output_scale_filter, has_audio, probe_video_metadata, resolve_export_profile,
    summarize_ffmpeg_error, BlurRegion, CameraOverlayParams, ExportSpeed, GifFilterOptions,
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

/// Pre-bake a wallpaper/image background to a canvas-sized, blurred PNG once, so
/// the export filter graph doesn't re-scale and re-blur a *static* image on every
/// frame — measured at ~19.5 ms/frame of pure waste on a 120 fps export (the blur
/// of a still image is identical every frame). Uses the exact
/// scale/crop/boxblur the graph would apply, so the composited result is
/// pixel-identical to the per-frame path. Best-effort: returns `None` on any
/// failure and the caller keeps the live per-frame background.
fn prebake_static_background(
    src: &Path,
    canvas_w: u32,
    canvas_h: u32,
    blur: f64,
) -> Option<(PathBuf, crate::render::cursor_export::TempDirGuard)> {
    if canvas_w == 0 || canvas_h == 0 {
        return None;
    }
    // Mirror graph.rs: boxblur sigma is `blur / 8`.
    let sigma = (blur / 8.0).max(0.0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("recast-export-bg-{ts}"));
    std::fs::create_dir_all(&dir).ok()?;
    let guard = crate::render::cursor_export::TempDirGuard::new(dir.clone());
    let out = dir.join("background.png");
    let vf = format!(
        "scale={canvas_w}:{canvas_h}:force_original_aspect_ratio=increase,crop={canvas_w}:{canvas_h},boxblur={sigma}"
    );
    let mut cmd = Command::new(crate::ffmpeg::ffmpeg_path());
    cmd.args(["-y", "-hide_banner", "-loglevel", "error", "-i"])
        .arg(src)
        .args(["-vf", &vf, "-frames:v", "1"])
        .arg(&out);
    crate::ffmpeg::configure_silent_command(&mut cmd);
    match cmd.status() {
        Ok(status) if status.success() && out.exists() => Some((out, guard)),
        _ => None,
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
    /// Human-readable sub-step during the multi-stage prep phase (e.g. "Rendering
    /// cursor layer"), so the UI isn't a blank "Preparing…" while the synchronous
    /// prep passes run before the encode emits real progress.
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
}

impl ExportStateEvent {
    fn base(export_id: &str, status: &'static str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status,
            progress: None,
            path: None,
            message: None,
            detail: None,
        }
    }

    fn started(export_id: &str) -> Self {
        Self::base(export_id, "started")
    }

    /// A named sub-step of the prep phase (before the encode drives real %).
    fn preparing(export_id: &str, detail: &str) -> Self {
        Self {
            detail: Some(detail.to_string()),
            ..Self::base(export_id, "preparing")
        }
    }

    fn progress(export_id: &str, progress: f64) -> Self {
        Self {
            progress: Some(progress),
            ..Self::base(export_id, "progress")
        }
    }

    fn finalizing(export_id: &str) -> Self {
        Self::base(export_id, "finalizing")
    }

    fn success(export_id: &str, path: &str) -> Self {
        Self {
            path: Some(path.to_string()),
            ..Self::base(export_id, "success")
        }
    }

    fn cancelled(export_id: &str) -> Self {
        Self::base(export_id, "cancelled")
    }

    fn error(export_id: &str, message: &str) -> Self {
        Self {
            message: Some(message.to_string()),
            ..Self::base(export_id, "error")
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
            needs_migration: project.needs_migration,
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
        needs_migration: false,
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

    // Thumbnails are identical for a given (file, count) until the recording
    // changes — so reuse a disk-cached strip across editor opens instead of
    // re-running the full FFmpeg decode every time (the dominant "slow load").
    // Keyed by the media file's identity; invalidated automatically when it
    // changes. `count` is the discriminator so the poster (count=1) and the
    // timeline strip don't collide.
    if let Some(cached) =
        crate::cache::get::<Vec<String>>("thumbs", &[media_path.as_path()], count as u64)
    {
        return Ok(cached);
    }

    let meta = probe_video_metadata(&media_path)?;
    if meta.duration <= 0.0 || count == 0 {
        return Ok(Vec::new());
    }

    let scale_width = if count <= 2 { 480 } else { 240 };

    // The single-frame poster path stays a fast `-ss` seek + `-vframes 1`
    // — that's a single decode at the requested timestamp, no full read.
    if count == 1 {
        let timestamp = meta.duration * 0.25;
        let poster = extract_single_thumbnail(&media_path, timestamp, scale_width)
            .map(|jpeg| {
                vec![format!(
                    "data:image/jpeg;base64,{}",
                    general_purpose::STANDARD.encode(jpeg)
                )]
            })
            .unwrap_or_default();
        if !poster.is_empty() {
            crate::cache::put("thumbs", &[media_path.as_path()], count as u64, &poster);
        }
        return Ok(poster);
    }

    // Timeline strip path: collect every thumbnail in ONE FFmpeg invocation
    // using `fps=count/duration` + a sequential output pattern. Previously
    // we spawned `count` separate FFmpeg processes (~200 ms codec init
    // each), which compounded into ~4 s of blocking work on low-end
    // dual-core CPUs before any thumbnail showed up. One spawn one decode
    // pass is dramatically faster — and bumps from O(count × init) to
    // O(decode) total wall time.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir()
        .join("recast-thumbnails")
        .join(format!("{}-{stamp}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    // `fps=count/duration` samples `count` frames evenly across the
    // timeline. `vsync vfr` keeps FFmpeg from duplicating or dropping
    // frames to match a constant output rate — we want exactly the
    // samples the filter produces.
    let fps_expr = format!("{count}/{:.6}", meta.duration.max(0.001));
    let vf = format!("fps={fps_expr},scale={scale_width}:-1");
    let pattern = temp_dir.join("thumb-%04d.jpg");
    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command.args([
        "-y",
        "-i",
        &media_path.to_string_lossy(),
        "-vf",
        &vf,
        "-vsync",
        "vfr",
        "-q:v",
        "4",
        pattern.to_string_lossy().as_ref(),
    ]);
    crate::ffmpeg::configure_silent_command(&mut command);

    let mut thumbnails = Vec::new();
    if command
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        // FFmpeg's image2 muxer numbers from 1 and may produce ±1 frame
        // around the requested `count` depending on rounding — read what's
        // actually there and trim to `count`.
        for index in 1..=count {
            let thumb_path = temp_dir.join(format!("thumb-{index:04}.jpg"));
            if let Ok(data) = fs::read(&thumb_path) {
                thumbnails.push(format!(
                    "data:image/jpeg;base64,{}",
                    general_purpose::STANDARD.encode(data)
                ));
            }
            let _ = fs::remove_file(&thumb_path);
            if thumbnails.len() >= count as usize {
                break;
            }
        }
    }

    // Best-effort *recursive* removal of the per-invocation subdir. `remove_dir`
    // (non-recursive) silently fails when image2 emits ±1 extra frame past
    // `count` or the loop breaks early, leaking the whole dir until the next
    // startup sweep — on a long editor session that's gigabytes of orphaned
    // JPEGs. `remove_dir_all` takes the stragglers with it. Ignore failure
    // (parallel invocations / filesystem races).
    let _ = fs::remove_dir_all(&temp_dir);

    // Persist the strip so the next open of this recording skips the decode.
    // Only cache a complete strip — a partial/failed run shouldn't be pinned.
    if !thumbnails.is_empty() {
        crate::cache::put("thumbs", &[media_path.as_path()], count as u64, &thumbnails);
    }

    Ok(thumbnails)
}

/// Pull a single thumbnail at `timestamp` (seconds). Used for poster
/// frames where the timeline-strip's multi-frame batching would be
/// overkill.
fn extract_single_thumbnail(
    media_path: &Path,
    timestamp: f64,
    scale_width: u32,
) -> Option<Vec<u8>> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir()
        .join("recast-thumbnails")
        .join(format!("{}-{stamp}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let thumb_path = temp_dir.join("thumb.jpg");

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

    let result = command
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|_| fs::read(&thumb_path).ok());
    let _ = fs::remove_file(&thumb_path);
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

/// Single-frame poster encoded as WebP — lighter than JPEG/PNG at equal
/// visual quality. Best-effort: returns `None` if the seek fails or the
/// bundled ffmpeg lacks libwebp. Used by the cloud uploader to give shared
/// recasts a thumbnail.
fn extract_poster_webp(media_path: &Path, timestamp: f64, scale_width: u32) -> Option<Vec<u8>> {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir()
        .join("recast-posters")
        .join(format!("{}-{stamp}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);
    let out_path = temp_dir.join("poster.webp");

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command.args([
        "-y",
        "-ss",
        &format!("{timestamp:.2}"),
        "-i",
        &media_path.to_string_lossy(),
        "-frames:v",
        "1",
        "-vf",
        &format!("scale={scale_width}:-1"),
        "-c:v",
        "libwebp",
        "-quality",
        "82",
        "-compression_level",
        "6",
        out_path.to_string_lossy().as_ref(),
    ]);
    crate::ffmpeg::configure_silent_command(&mut command);

    let result = command
        .output()
        .ok()
        .filter(|o| o.status.success())
        .and_then(|_| fs::read(&out_path).ok());
    let _ = fs::remove_file(&out_path);
    let _ = fs::remove_dir_all(&temp_dir);
    result
}

/// Poster WebP bytes for an exported MP4 (the cloud uploader's source file).
/// Seeks to 25% — the same frame the editor's single-thumbnail path picks.
/// Returns `None` on any failure; callers treat the poster as optional.
pub(crate) fn poster_webp_for_export(path: &str) -> Option<Vec<u8>> {
    let input = PathBuf::from(path);
    let meta = probe_video_metadata(&input).ok()?;
    if meta.duration <= 0.0 {
        return None;
    }
    extract_poster_webp(&input, meta.duration * 0.25, 960)
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
    cut_select: Option<&str>,
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
    args.extend(["-i".to_string(), source_path.to_string_lossy().to_string()]);

    let base_vf = build_gif_palette_prepass_filter(options, output_scale_filter);
    // Drop cut ranges and close the gaps before fps-resample + palettegen, so
    // the palette is built only from kept frames.
    let vf = match cut_select {
        Some(cs) if !cs.is_empty() => format!("{cs},{base_vf}"),
        _ => base_vf,
    };
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
    let effective_duration = if duration > 0.0 {
        duration
    } else {
        source_duration
    };

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

/// Resolve the render state's silence/manual cuts into post-trim stream
/// seconds (the input is seeked by `-ss trim_start`, so the filtergraph's `t`
/// starts at 0 = `trim_start`). Cuts are clamped to the kept `[trim_start,
/// trim_end]` window, sorted, and overlaps merged.
///
/// Note: split/cut editing and silence detection are EXPERIMENTAL, opt-in
/// features on the client. The frontend only includes a cut in `render_state`
/// when its feature is enabled (see `effectiveCuts` in the editor store and
/// `buildExportRenderState`), so when a feature is opted off `render_state.cuts`
/// is empty here and the export matches an un-edited clip. This pipeline applies
/// whatever cuts it is handed; it does not (and cannot) re-check the flags.
/// Two cut edges within this many seconds are treated as the same boundary and
/// merged. Kept in lockstep with `EPS` in the frontend's cut/segment model
/// (apps/desktop/src/lib/timeline/{cuts,segments}.ts) so the previewed edit and
/// the export never disagree on where a segment begins or ends.
const CUT_MERGE_EPS: f64 = 1e-4;

fn collect_export_cuts(
    render_state: &crate::render::graph::RenderState,
    trim_start: f64,
    trim_end: f64,
) -> Vec<(f64, f64)> {
    let mut cuts: Vec<(f64, f64)> = render_state
        .cuts
        .iter()
        .filter_map(|c| {
            let lo = c.start.max(trim_start) - trim_start;
            let hi = c.end.min(trim_end) - trim_start;
            (hi - lo > 0.01).then_some((lo.max(0.0), hi))
        })
        .collect();
    cuts.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
    let mut merged: Vec<(f64, f64)> = Vec::with_capacity(cuts.len());
    for cut in cuts {
        match merged.last_mut() {
            // Adjacency tolerance MUST match the frontend's `normalizeCuts` EPS
            // (1e-4 in apps/desktop/src/lib/timeline/cuts.ts) so the editor's
            // collapsed timeline and this export agree on segment boundaries to
            // the same precision. See cut-parity tests on both sides.
            Some(last) if cut.0 <= last.1 + CUT_MERGE_EPS => last.1 = last.1.max(cut.1),
            _ => merged.push(cut),
        }
    }
    merged
}

/// Build a `select`/`aselect` expression that *keeps* every frame outside the
/// cut ranges: `not(between(t,a,b)+between(t,c,d)+…)`. Single-quoted at the
/// call site so the inner commas survive the filtergraph parser.
fn build_cut_select_expr(cuts: &[(f64, f64)]) -> String {
    let terms: Vec<String> = cuts
        .iter()
        .map(|(a, b)| format!("between(t,{a:.3},{b:.3})"))
        .collect();
    format!("not({})", terms.join("+"))
}

// --- Per-segment speed (Cap-style "edit this cut differently") -------------
//
// Overlays (zoom/cursor/blur) are computed on the continuous post-trim timeline
// and cuts are applied last as a pure frame-drop (see the main pass below), so
// speed slots in at the same tail point as a timing warp — no evaluator needs to
// change. The kept-segment + speed math mirrors the frontend time-map
// (apps/desktop/src/lib/timeline/{segments,segment-speed,time-map}.ts) and is
// parity-tested against the shared speed-parity.json fixture.

/// Clamp mirroring MIN/MAX_SEGMENT_SPEED in segment-speed.ts; a bad value → 1×.
fn clamp_segment_speed(speed: f64) -> f64 {
    if !speed.is_finite() || speed <= 0.0 {
        1.0
    } else {
        speed.clamp(0.25, 4.0)
    }
}

/// A kept segment on the post-trim timeline (t=0 at trim_start) with its speed.
#[derive(Debug, Clone, Copy)]
struct SpeedSegment {
    start: f64,
    end: f64,
    speed: f64,
}

fn make_speed_segment(
    start: f64,
    end: f64,
    segment_speeds: &[crate::render::graph::SegmentSpeed],
    trim_start: f64,
) -> SpeedSegment {
    // Anchors are ORIGINAL-recording seconds; a post-trim segment's original
    // start is `start + trim_start`.
    let anchor = start + trim_start;
    let speed = segment_speeds
        .iter()
        .find(|sp| (sp.start - anchor).abs() <= CUT_MERGE_EPS)
        .map(|sp| clamp_segment_speed(sp.speed))
        .unwrap_or(1.0);
    SpeedSegment { start, end, speed }
}

/// Derive the kept segments on the post-trim timeline with their speeds. `cuts`
/// are the already-collected post-trim cut ranges; `split_points` and the speed
/// anchors are ORIGINAL seconds (shifted by `trim_start`). Mirrors
/// deriveSegments + segment-speed anchoring on the frontend.
fn build_speed_segments(
    duration: f64,
    cuts: &[(f64, f64)],
    split_points: &[f64],
    segment_speeds: &[crate::render::graph::SegmentSpeed],
    trim_start: f64,
) -> Vec<SpeedSegment> {
    // Kept intervals = [0, duration] minus the cuts.
    let mut kept: Vec<(f64, f64)> = Vec::new();
    let mut cursor = 0.0;
    for (cs, ce) in cuts {
        if cs - cursor > CUT_MERGE_EPS {
            kept.push((cursor, *cs));
        }
        cursor = cursor.max(*ce);
    }
    if duration - cursor > CUT_MERGE_EPS {
        kept.push((cursor, duration));
    }
    // Slice each kept interval at the split points strictly inside it.
    let mut segs: Vec<SpeedSegment> = Vec::new();
    for (s, e) in kept {
        let mut inside: Vec<f64> = split_points
            .iter()
            .map(|p| p - trim_start)
            .filter(|p| *p > s + CUT_MERGE_EPS && *p < e - CUT_MERGE_EPS)
            .collect();
        inside.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        let mut from = s;
        for p in inside {
            segs.push(make_speed_segment(from, p, segment_speeds, trim_start));
            from = p;
        }
        segs.push(make_speed_segment(from, e, segment_speeds, trim_start));
    }
    segs
}

/// Any segment off 1× — the guard that keeps the no-speed export path unchanged.
fn has_speed_change(segs: &[SpeedSegment]) -> bool {
    segs.iter().any(|s| (s.speed - 1.0).abs() > CUT_MERGE_EPS)
}

/// Warped output duration — the value the export and the frontend time-map must
/// agree on. The FFmpeg pipeline expresses the same warp via `setpts`/`atempo`;
/// this also drives the output-side `-t` cap so the encode stops at the real
/// post-edit content length (cuts dropped + speed warped), not the raw trimmed
/// span — otherwise the infinite background generators freeze the last frame
/// past content-end. Parity-tested against the frontend time-map.
fn warped_output_duration(segs: &[SpeedSegment]) -> f64 {
    segs.iter().map(|s| (s.end - s.start) / s.speed).sum()
}

/// Output-side `-t` cap (seconds): the real post-edit content length. Non-GIF
/// exports run cuts + per-segment speed through select/setpts, so the stream is
/// the warped duration — capping at the raw trimmed span would freeze the last
/// frame over the infinite background for the cut/sped-away time (and truncate
/// slow-motion, where warped > raw). GIF uses a separate cut/palette path and
/// loops, so it keeps the raw trimmed span.
fn output_duration_cap(format: &str, duration: f64, speed_segments: &[SpeedSegment]) -> f64 {
    if format == "gif" {
        duration
    } else {
        warped_output_duration(speed_segments)
    }
}

/// `setpts` seconds-expression mapping a survivor frame's post-trim source time
/// `T` onto the warped output axis: within segment i it is
/// `offset_i + (T - start_i)/speed_i`, selected by nested `if(lt(T,end_i),…)`
/// with the last segment as the else branch. Caller wraps as `setpts=(EXPR)/TB`.
fn build_speed_setpts_expr(segs: &[SpeedSegment]) -> String {
    fn rec(segs: &[SpeedSegment], i: usize, offset: f64) -> String {
        let s = &segs[i];
        let here = format!("{:.6}+(T-{:.6})/{:.6}", offset, s.start, s.speed);
        if i + 1 >= segs.len() {
            here
        } else {
            let next_offset = offset + (s.end - s.start) / s.speed;
            format!(
                "if(lt(T,{:.6}),{},{})",
                s.end,
                here,
                rec(segs, i + 1, next_offset)
            )
        }
    }
    if segs.is_empty() {
        "T".to_string()
    } else {
        rec(segs, 0, 0.0)
    }
}

/// FFmpeg `atempo` accepts 0.5..=2.0 per instance; chain stages to cover the
/// 0.25..=4.0 speed range (e.g. 4× → "atempo=2.0,atempo=2.0").
fn atempo_chain(speed: f64) -> String {
    let mut remaining = speed;
    let mut stages: Vec<f64> = Vec::new();
    while remaining > 2.0 + 1e-9 {
        stages.push(2.0);
        remaining /= 2.0;
    }
    while remaining < 0.5 - 1e-9 {
        stages.push(0.5);
        remaining /= 0.5;
    }
    stages.push(remaining);
    stages
        .iter()
        .map(|s| format!("atempo={s:.6}"))
        .collect::<Vec<_>>()
        .join(",")
}

/// Per-segment audio retime that matches the video warp: split the kept audio
/// into one branch per segment, `atrim` each to its post-trim range, `atempo` to
/// its speed, then `concat` (audio mode). Replaces the cut-only `aselect`/`asetpts` path when
/// any segment is sped. `amap` is the audio label to consume (e.g. "[0:a:0]").
fn build_speed_audio_filter(amap: &str, segs: &[SpeedSegment]) -> String {
    let n = segs.len();
    let mut parts: Vec<String> = Vec::new();
    let split_labels: Vec<String> = (0..n).map(|i| format!("[aspd{i}]")).collect();
    parts.push(format!("{amap}asplit={n}{}", split_labels.join("")));
    let mut seg_labels: Vec<String> = Vec::new();
    for (i, s) in segs.iter().enumerate() {
        let out = format!("[aseg{i}]");
        parts.push(format!(
            "{}atrim=start={:.6}:end={:.6},asetpts=PTS-STARTPTS,{}{}",
            split_labels[i],
            s.start,
            s.end,
            atempo_chain(s.speed),
            out
        ));
        seg_labels.push(out);
    }
    // FFmpeg has no `aconcat`; audio is concatenated with the `concat` filter
    // configured for audio only (v=0:a=1).
    parts.push(format!("{}concat=n={n}:v=0:a=1[acut]", seg_labels.join("")));
    parts.join(";")
}

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    mut request: ExportRequest,
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
    emit_export_state(
        &app,
        ExportStateEvent::preparing(&export_id, "Preparing export"),
    );

    // Per-stage wall-clock instrumentation (export-perf plan, step 1): attribute
    // the total to prep / cursor pre-render / encode so the pipeline is optimised
    // against measured numbers, not guesses. Emitted at info, correlated with the
    // frontend by `export_id`.
    let export_start = Instant::now();

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
    // Output frame rate (MP4/WebM). Default = source rate, so the export keeps
    // the original smoothness with no resampling. A user selection is clamped to
    // never exceed the source — we only ever downsample, never duplicate frames
    // (which would bloat the file without adding motion). The target drives the
    // composite background rate, the looped-input rate, and the cursor overlay
    // rate so the whole graph runs at one consistent rate (see the 25fps-default
    // judder bug fixed alongside this). GIF ignores it (uses gif_settings.fps).
    let source_fps = if metadata.fps.is_finite() && metadata.fps >= 1.0 {
        metadata.fps
    } else {
        60.0
    };
    let target_fps = request
        .fps
        .filter(|f| f.is_finite() && *f >= 1.0)
        .map(|f| f.min(source_fps))
        .unwrap_or(source_fps);
    // Encoder effort axis, orthogonal to the resolution profile. Defaults to
    // Balanced (historical settings) when absent/unknown.
    let speed = ExportSpeed::from_request(request.speed.as_deref().unwrap_or("balanced"));
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

    // Backend processing trace, correlated with the frontend's `export_started`
    // line by `export_id`. Info level → captured in dev and in diagnostic mode.
    log::info!(
        "export[{}] start: {}x{} dur={:.1}s format={} quality={} speed={} -> {}",
        export_id,
        metadata.width,
        metadata.height,
        duration,
        request.format,
        request.quality,
        request.speed.as_deref().unwrap_or("balanced"),
        output_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default(),
    );

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
    let border_radius_px = border_radius_pct / 100.0 * metadata.width.min(metadata.height) as f64;
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

    // Gradient backgrounds are rasterised to a canvas-sized PNG so the export
    // composites the exact multi-stop, angled gradient the WebGL preview shows.
    // Without this the FFmpeg planner falls back to a single flat color. Held
    // alive until the export finishes (the temp dir auto-cleans on drop).
    let gradient_bg: Option<MaskResult> = if request.render_state.background_type == "gradient" {
        crate::render::mask_export::render_gradient_background(
            &request.render_state.background_value,
            canvas_width,
            canvas_height,
        )
        .map_err(|e| format!("gradient background render failed: {e}"))?
    } else {
        None
    };
    let gradient_bg_path = gradient_bg.as_ref().map(|m| m.path.clone());

    // Pre-bake a static wallpaper/image background once (canvas-sized + blurred)
    // so the filter graph doesn't re-scale/re-blur it on every frame — a static
    // background is identical each frame (measured ~19.5 ms/frame at 120 fps).
    // Point the background at the baked PNG with blur 0; the graph then loops it as
    // a near-no-op, pixel-identical to the per-frame path. Best-effort: on failure
    // the render state is untouched and the live per-frame path runs as before. The
    // guard keeps the PNG alive until the export finishes.
    let _prebaked_bg = if matches!(
        request.render_state.background_type.as_str(),
        "wallpaper" | "image"
    ) {
        crate::render::graph::resolve_background_path(
            &request.render_state.background_value,
            &static_root(),
            asset_cache_dir.as_deref(),
        )
        .and_then(|src| {
            prebake_static_background(
                &src,
                canvas_width,
                canvas_height,
                request.render_state.background_blur,
            )
        })
        .map(|(path, guard)| {
            request.render_state.background_value = path.to_string_lossy().into_owned();
            request.render_state.background_blur = 0.0;
            guard
        })
    } else {
        None
    };
    // Rebuild the graph so the export plan sees the (possibly) pre-baked
    // background. `trim_start`/`trim_end` were already read above and the
    // background swap doesn't affect them.
    let graph = RenderGraph::from_state(&request.render_state);

    // Scene entrance/exit animations on the video layer. Derived on the same
    // post-trim kept-segment windows as speed (cuts + splits) so an animation's
    // window lines up with its clip; the tail cut+speed stage then re-times it,
    // exactly like zoom. `None` when nothing animates → the static overlay path.
    let scene_overlay = if request.render_state.scene_animations.is_empty() {
        None
    } else {
        let scene_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
        let windows: Vec<(f64, f64)> = build_speed_segments(
            duration,
            &scene_cuts,
            &request.render_state.split_points,
            &[],
            trim_start,
        )
        .iter()
        .map(|s| (s.start, s.end))
        .collect();
        crate::render::scene_anim::build_scene_overlay(
            &windows,
            trim_start,
            &request.render_state.scene_animations,
            &canvas_geom,
            metadata.width,
            metadata.height,
        )
    };

    let export_plan = graph
        .build_export_plan_with(
            SourceVideoMetadata {
                width: metadata.width,
                height: metadata.height,
                fps: target_fps,
            },
            &static_root(),
            1,
            asset_cache_dir.as_deref(),
            border_radius_mask_path,
            drop_shadow_mask_path,
            gradient_bg_path,
            canvas_geom,
            scene_overlay.as_ref(),
        )
        .map_err(|e| e.to_string())?;
    let overlay_duration = if duration > 0.0 {
        duration
    } else {
        source_duration
    };
    // Prep (probe + masks + plan) is everything up to here; time the cursor/
    // overlay pre-render separately — it's the plan's prime perf suspect.
    let prep_ms = export_start.elapsed().as_millis();
    let cursor_render_start = Instant::now();
    let needs_overlay = request.render_state.cursor_enabled
        || !request.render_state.annotations.is_empty()
        || (request.render_state.shadow.enabled && request.render_state.shadow.opacity > 0.0);
    // Surface the cursor/annotation pre-render — it's the longest prep sub-step
    // (it renders every output frame before the encode starts), so a plain
    // "Preparing…" here reads as a hang.
    if needs_overlay && overlay_duration > 0.0 {
        emit_export_state(
            &app,
            ExportStateEvent::preparing(&export_id, "Rendering cursor & annotations"),
        );
    }
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
                    fps: target_fps.round().max(1.0) as u32,
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
    let cursor_ms = cursor_render_start.elapsed().as_millis();
    let cursor_ran = cursor_overlay.is_some();
    log::info!(
        "export[{export_id}] timing: prep={prep_ms}ms cursor_overlay={cursor_ms}ms (ran={cursor_ran})"
    );

    // The filter graph is the export's dominant cost (a single-threaded,
    // expression-heavy composite starves the GPU encoder). Parallelise it across
    // cores — pure performance, byte-identical output (every filter still runs).
    let filter_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
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
        "-filter_complex_threads".to_string(),
        filter_threads.to_string(),
        "-filter_threads".to_string(),
        filter_threads.to_string(),
    ];
    if trim_start > 0.0 {
        args.extend(["-ss".to_string(), format!("{trim_start:.3}")]);
    }
    if duration > 0.0 {
        args.extend(["-t".to_string(), format!("{duration:.3}")]);
    }
    args.extend(["-i".to_string(), source_video.to_string_lossy().to_string()]);

    // `-loop 1` on a still image defaults to 25 fps. A wallpaper/gradient/image
    // background is the BASE of the composite overlay, so leaving it at 25 fps
    // would force the whole export to 25 fps (frame-dropping the 60 fps source
    // into judder). Pin every looped image input to the source frame rate.
    let loop_fps = target_fps;
    for input in &export_plan.extra_inputs {
        args.extend([
            "-framerate".to_string(),
            format!("{loop_fps}"),
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
    let camera_bubble: Option<(PathBuf, u32, u32, u32, u32)> = if let Some(ref path) = camera_path {
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
    if let (Some(cam_idx), Some((_, bx, by, bw, bh))) = (camera_input_index, camera_bubble.as_ref())
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
                let tint_rgb =
                    u32::from_str_radix(tint_color.trim_start_matches('#'), 16).unwrap_or(0x000000);
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

    // Burn-in captions (overlay) via libass. The transcript + style ride along
    // in the render-state passthrough; styled into an ASS script and composited
    // here on the trimmed-but-uncut axis, so the cut/speed stage below re-times
    // the burned pixels with the rest. No-op without a transcript; GIF skips it
    // (its paletteuse tail can't take another filter stage).
    if request.burn_captions && request.format != "gif" {
        let transcript: Option<crate::transcription::Transcript> = request
            .render_state
            .passthrough
            .get("transcript")
            .filter(|v| !v.is_null())
            .and_then(|v| serde_json::from_value(v.clone()).ok());
        if let Some(transcript) = transcript.filter(|t| !t.segments.is_empty()) {
            let style: crate::transcription::CaptionStyle = request
                .render_state
                .passthrough
                .get("captionStyle")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            // Embed the preset's font so it renders in the burn instead of a
            // libass fallback. System/generic faces are skipped (libass resolves
            // them); a fetch failure degrades to the fallback, never blocks export.
            let family = crate::transcription::subtitles::first_family(&style.font_family);
            let fontsdir: Option<String> =
                if crate::transcription::subtitles::is_system_family(&family) {
                    None
                } else {
                    match crate::fonts::ensure_caption_font_dir(&app, &family, style.font_weight)
                        .await
                    {
                        Ok(dir) => Some(dir.to_string_lossy().to_string()),
                        Err(e) => {
                            log::warn!("caption font embed ({family}): {e}");
                            None
                        }
                    }
                };
            let ass = crate::transcription::subtitles::to_ass(
                &transcript,
                &style,
                canvas_width,
                canvas_height,
                crate::transcription::subtitles::VideoRectPx {
                    x: canvas_geom.video_x,
                    y: canvas_geom.video_y,
                    w: canvas_geom.video_w,
                    h: canvas_geom.video_h,
                },
                trim_start,
                duration,
                fontsdir.is_some(),
            );
            let ass_path =
                std::env::temp_dir().join(format!("recast-captions-{}.ass", request.export_id));
            match std::fs::write(&ass_path, ass) {
                Ok(()) => {
                    let (new_complex, new_map) = append_subtitles_to_complex(
                        filter_complex_after_cursor.as_deref(),
                        &video_map_after_cursor,
                        &ass_path.to_string_lossy(),
                        fontsdir.as_deref(),
                    );
                    filter_complex_after_cursor = Some(new_complex);
                    video_map_after_cursor = new_map;
                }
                Err(e) => log::warn!("caption burn-in: failed to write ASS script: {e}"),
            }
        }
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

        // Cuts apply to GIF too, but its two-pass palette path runs before the
        // generic (MP4/WebM-only) cut stage below, so inject the same
        // select+setpts into both the palette pre-pass and the main pass.
        let gif_cut_select: Option<String> = {
            let export_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
            (!export_cuts.is_empty()).then(|| {
                format!(
                    "select='{}',setpts=N/FRAME_RATE/TB",
                    build_cut_select_expr(&export_cuts)
                )
            })
        };
        let cut_select_for_prepass = gif_cut_select.clone();
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
                cut_select_for_prepass.as_deref(),
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
        args.extend(["-i".to_string(), palette_path.to_string_lossy().to_string()]);

        // Drop cut ranges before the palette-use stage so removed frames never
        // reach the GIF (the generic cut stage below is MP4/WebM-only).
        if let Some(ref cs) = gif_cut_select {
            let (mut complex, vlabel) = match filter_complex_after_cursor.take() {
                Some(existing) => (existing, video_map_after_cursor.clone()),
                None => ("[0:v]".to_string(), "[0:v]".to_string()),
            };
            if !complex.is_empty() && !complex.ends_with(';') && !vlabel.is_empty() {
                complex.push(';');
            }
            complex.push_str(&vlabel);
            complex.push_str(&format!("{cs}[vgifcut]"));
            filter_complex_after_cursor = Some(complex);
            video_map_after_cursor = "[vgifcut]".to_string();
        }

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

    let mut audio_map = if request.format == "gif" {
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

    // Silence/manual cuts — drop the cut ranges from the middle of the
    // timeline. `select`/`aselect` discard the cut frames and `setpts`/
    // `asetpts` re-stitch the survivors into a gapless stream. This runs at
    // the *end* of the chain: everything upstream (zoom, cursor, blur) was
    // computed on the continuous post-trim timeline and stays correct —
    // select only removes frames, it never reinterprets time. GIF has its own
    // paletteuse tail, so cuts there would need separate handling; skipped.
    let export_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
    // Per-segment speed (Cap-style) warps the survivors' timing on top of the cut
    // drop — same tail-of-chain point, so upstream overlays stay correct. The
    // segments and their warped duration mirror the frontend time-map (parity).
    let speed_segments = build_speed_segments(
        duration,
        &export_cuts,
        &request.render_state.split_points,
        &request.render_state.segment_speeds,
        trim_start,
    );
    let speed_active = has_speed_change(&speed_segments);
    if (!export_cuts.is_empty() || speed_active) && request.format != "gif" {
        let has_cuts = !export_cuts.is_empty();
        let select_expr = build_cut_select_expr(&export_cuts);
        let (mut complex, video_label) = match filter_complex_after_cursor.take() {
            Some(existing) => (existing, video_map_after_cursor.clone()),
            None => {
                // No filtergraph yet: seed one and fold in any pending
                // output-side filters (e.g. a quality downscale) so they
                // aren't lost now that `-vf` no longer applies.
                let mut seed = String::new();
                let prefix = if output_filters.is_empty() {
                    String::new()
                } else {
                    format!("{},", output_filters.join(","))
                };
                output_filters.clear();
                seed.push_str(&format!("[0:v:0]{prefix}"));
                (seed, String::new())
            }
        };
        if !complex.is_empty() && !complex.ends_with(';') && !video_label.is_empty() {
            complex.push(';');
        }
        complex.push_str(&video_label);
        // Drop cut frames (select), then re-time survivors. At 1× this is the
        // uniform CFR re-stamp (unchanged); with speed it's the piecewise warp,
        // and the output `-r` resamples the warped PTS back to CFR (dropping /
        // duplicating frames as the speed demands).
        let select_prefix = if has_cuts {
            format!("select='{select_expr}',")
        } else {
            String::new()
        };
        let setpts = if speed_active {
            // Single-quote the value: the warp expression contains commas
            // (if(lt(T,…),…,…)) that the filtergraph parser would otherwise read
            // as filter separators — same reason `select='…'` is quoted above.
            format!("setpts='({})/TB'", build_speed_setpts_expr(&speed_segments))
        } else {
            "setpts=N/FRAME_RATE/TB".to_string()
        };
        complex.push_str(&format!("{select_prefix}{setpts}[vcut]"));
        video_map_after_cursor = "[vcut]".to_string();
        if let Some(amap) = audio_map.take() {
            if speed_active {
                // Per-segment atrim+atempo+concat keeps audio length matched to
                // the warped video, pitch-preserved (atempo time-stretches).
                complex.push_str(&format!(
                    ";{}",
                    build_speed_audio_filter(&amap, &speed_segments)
                ));
            } else {
                complex.push_str(&format!(
                    ";{amap}aselect='{select_expr}',asetpts=N/SR/TB[acut]"
                ));
            }
            audio_map = Some("[acut]".to_string());
        }
        filter_complex_after_cursor = Some(complex);
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
    //
    // Cap at the REAL post-edit length: cuts drop frames and per-segment speed
    // warps them, so the edited stream is shorter (or, for slow-motion, longer)
    // than the raw trimmed span. Because the background generators are infinite
    // and `overlay` repeats the video's last frame past content-end, capping at
    // the raw span would bake a frozen tail (the cut/sped-away time) onto the
    // end — and would truncate a slowed clip. `warped_output_duration` is the
    // length the select+setpts/atempo stage actually produces. GIF keeps the
    // trimmed span (its cut/palette path differs and it loops).
    let output_cap = output_duration_cap(&request.format, duration, &speed_segments);
    if output_cap > 0.0 {
        args.extend(["-t".to_string(), format!("{output_cap:.3}")]);
    }
    // The real length of the output file — the `-t` cap (cuts dropped + speed
    // warped), not the raw trimmed span. This is the UI progress denominator and
    // the completion-probe target; using the raw span made the bar stall short of
    // (cuts/speed-up) or overshoot past (slow-motion) 100%.
    let expected_output_secs = if output_cap > 0.0 {
        output_cap
    } else {
        source_duration
    };

    if duration <= 0.0 && (!export_plan.extra_inputs.is_empty() || cursor_overlay_path.is_some()) {
        args.push("-shortest".to_string());
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
            // libvpx-vp9 is single-threaded and uses `deadline=best` by
            // default — a combo that turned a 5-min 1080p export into a
            // 30+ min job on a dual-core laptop with the machine pinned
            // at one core. Switching on row-multithreading, letting FFmpeg
            // pick the thread count, and bumping `cpu-used` to 4 with
            // `deadline=good` gives ~4–8× faster encodes at the same CRF
            // with quality loss that's invisible to viewers. `tile-columns`
            // splits the frame for additional parallelism on multi-core
            // machines — log2(2)=1 gives 2 tile columns, a safe default
            // for 1080p+.
            args.extend([
                "-c:v".to_string(),
                "libvpx-vp9".to_string(),
                "-crf".to_string(),
                profile.webm_crf.to_string(),
                "-b:v".to_string(),
                "0".to_string(),
                "-deadline".to_string(),
                "good".to_string(),
                "-cpu-used".to_string(),
                speed.vp9_cpu_used().to_string(),
                "-row-mt".to_string(),
                "1".to_string(),
                "-tile-columns".to_string(),
                "1".to_string(),
                "-threads".to_string(),
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
            // Export-quality codec args. NVENC/AMF/QSV all get hardware
            // rate control tuned for quality (not the lowlatency presets
            // we use for live recording). libx264 stays on the user's
            // chosen profile preset (medium/slow/etc.) because export
            // isn't bound by real-time pacing — slower presets = smaller
            // files at the same quality.
            match crate::ffmpeg::preferred_h264_encoder() {
                "h264_videotoolbox" => {
                    args.extend([
                        "-c:v".to_string(),
                        "h264_videotoolbox".to_string(),
                        "-profile:v".to_string(),
                        "high".to_string(),
                        "-pix_fmt".to_string(),
                        "yuv420p".to_string(),
                        // VideoToolbox uses -q:v (1-100, higher is better) for VBR
                        "-q:v".to_string(),
                        "65".to_string(),
                    ]);
                }
                "h264_nvenc" => {
                    args.extend([
                        "-c:v".to_string(),
                        "h264_nvenc".to_string(),
                        "-preset".to_string(),
                        speed.nvenc_preset().to_string(),
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
                "h264_amf" => {
                    // AMF maps the NVENC `cq` (lower = better, 0..51) to
                    // `qp_i/qp_p` directly. We use the same value range so
                    // the export profiles stay quality-comparable across
                    // GPUs.
                    let qp = profile.mp4_nvenc_cq.to_string();
                    args.extend([
                        "-c:v".to_string(),
                        "h264_amf".to_string(),
                        "-quality".to_string(),
                        speed.amf_quality().to_string(),
                        "-rc".to_string(),
                        "cqp".to_string(),
                        "-qp_i".to_string(),
                        qp.clone(),
                        "-qp_p".to_string(),
                        qp,
                        "-profile:v".to_string(),
                        "high".to_string(),
                        "-pix_fmt".to_string(),
                        "yuv420p".to_string(),
                    ]);
                }
                "h264_qsv" => {
                    args.extend([
                        "-c:v".to_string(),
                        "h264_qsv".to_string(),
                        "-preset".to_string(),
                        speed.qsv_preset().to_string(),
                        "-global_quality".to_string(),
                        profile.mp4_nvenc_cq.to_string(),
                        "-profile:v".to_string(),
                        "high".to_string(),
                        "-pix_fmt".to_string(),
                        "nv12".to_string(),
                    ]);
                }
                _ => {
                    args.extend([
                        "-c:v".to_string(),
                        "libx264".to_string(),
                        "-preset".to_string(),
                        speed
                            .x264_preset()
                            .unwrap_or(profile.mp4_preset)
                            .to_string(),
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

    // Record which encoder/decoder actually ran — the plan's #1 open question
    // (hardware vs the libx264 software fallback). Read off the emitted args so it
    // stays correct across every format/branch. Captured before `args` moves into
    // the encode task below.
    let video_encoder = args
        .iter()
        .position(|a| a == "-c:v")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "unknown".to_string());
    // `-hwaccel auto` may still fall back to software internally, so report the
    // requested mode rather than claiming hardware.
    let decode_mode = args
        .iter()
        .position(|a| a == "-hwaccel")
        .and_then(|i| args.get(i + 1).cloned())
        .map(|v| format!("hwaccel:{v}"))
        .unwrap_or_else(|| "software".to_string());
    log::info!("export[{export_id}] encoder={video_encoder} decode={decode_mode} filter_threads={filter_threads}");

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
                        let effective_duration = expected_output_secs;
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

        let expected_output_duration = expected_output_secs;

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
        Ok(inner) => {
            if inner.is_ok() {
                // One correlated summary line: total wall-clock and the stage
                // breakdown. The encode's own duration is logged inside the task
                // ("child exited at T+…ms" / "success emitted at T+…ms").
                log::info!(
                    "export[{export_id}] timing: total={}ms prep={prep_ms}ms cursor_overlay={cursor_ms}ms (ran={cursor_ran}) encoder={video_encoder} decode={decode_mode}",
                    export_start.elapsed().as_millis()
                );
            }
            inner
        }
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

/// Crash-recovery shadow write, fired on a ~30s timer — async + spawn_blocking
/// so the JSON serialize + atomic file write never stall the UI thread.
#[tauri::command]
pub async fn autosave_project(project_path: String, edits_json: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::project::autosave::save_autosave(Path::new(&project_path), &edits_json)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| format!("autosave task panicked: {e}"))?
}

/// Re-pack a legacy `.recast` as the current format in place (keeps a `.bak`).
/// Heavy zip I/O, so it runs off the main thread.
#[tauri::command]
pub async fn migrate_project(project_path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::project::migrate_project(Path::new(&project_path))
    })
    .await
    .map_err(|e| format!("migrate task panicked: {e}"))?
    .map_err(|e| format!("{e:#}"))
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
pub async fn clear_autosave(project_path: String) {
    let _ = tauri::async_runtime::spawn_blocking(move || {
        crate::project::autosave::clear_autosave(Path::new(&project_path));
    })
    .await;
}

/// Scans the autosave temp dir + parses each shadow file — async so a cluttered
/// recovery dir doesn't block startup on the UI thread.
#[tauri::command]
pub async fn get_recoverable_sessions() -> Vec<crate::project::autosave::AutosaveState> {
    tauri::async_runtime::spawn_blocking(crate::project::autosave::find_recoverable_sessions)
        .await
        .unwrap_or_default()
}

/// Analyse a captured cursor track and return the list of moments that would
/// make good auto-focus candidates (scored, clustered, density-limited).
///
/// Always recomputes via `detect_zoom_triggers` rather than trusting the
/// `zoom_triggers` persisted in the track — clips recorded before a detector
/// improvement would otherwise keep serving stale (often far noisier)
/// suggestions. Detection is cheap (µs over the in-memory track).
#[tauri::command]
pub async fn suggest_zoom_regions(
    cursor_path: String,
) -> Result<Vec<crate::cursor::smoothing::ZoomTrigger>, String> {
    // The cursor track is multi-MB on long recordings; read + parse off-thread.
    tauri::async_runtime::spawn_blocking(move || {
        let bytes =
            fs::read(Path::new(&cursor_path)).map_err(|e| format!("read cursor track: {e}"))?;
        let track: crate::cursor::CursorTrack =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse cursor track: {e}"))?;
        Ok(crate::cursor::smoothing::detect_zoom_triggers(
            &track.samples,
            &track.clicks,
        ))
    })
    .await
    .map_err(|e| format!("suggest task panicked: {e}"))?
}

#[cfg(test)]
mod cut_export_tests {
    use super::{
        atempo_chain, build_cut_select_expr, build_speed_segments, build_speed_setpts_expr,
        clamp_segment_speed, collect_export_cuts, output_duration_cap, warped_output_duration,
        SpeedSegment,
    };
    use crate::render::graph::{CutRange, RenderState, SegmentSpeed};

    fn seg(start: f64, end: f64, speed: f64) -> SpeedSegment {
        SpeedSegment { start, end, speed }
    }

    #[test]
    fn output_cap_uses_warped_length_for_video_and_raw_for_gif() {
        // Two kept segments, the second sped 2× → raw span 8s, warped 4 + 2 = 6s.
        let segs = vec![seg(0.0, 4.0, 1.0), seg(4.0, 8.0, 2.0)];
        // Non-GIF caps at the real content length — this is the frozen-tail fix.
        assert!((output_duration_cap("mp4", 8.0, &segs) - 6.0).abs() < 1e-9);
        assert!((output_duration_cap("webm", 8.0, &segs) - 6.0).abs() < 1e-9);
        // GIF keeps the raw trimmed span (its own cut/palette path).
        assert!((output_duration_cap("gif", 8.0, &segs) - 8.0).abs() < 1e-9);
    }

    #[test]
    fn output_cap_unchanged_without_edits() {
        let segs = vec![seg(0.0, 10.0, 1.0)];
        assert!((output_duration_cap("mp4", 10.0, &segs) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn output_cap_extends_for_slow_motion_so_it_is_not_truncated() {
        // 0.5× → warped 20s > raw 10s; the cap must grow, not clip the slow-mo.
        let segs = vec![seg(0.0, 10.0, 0.5)];
        assert!((output_duration_cap("mp4", 10.0, &segs) - 20.0).abs() < 1e-9);
    }

    #[test]
    fn setpts_expr_edge_cases() {
        // No segments → identity remap.
        assert_eq!(build_speed_setpts_expr(&[]), "T");
        // Single segment → flat affine map, no nested if().
        let one = build_speed_setpts_expr(&[seg(0.0, 4.0, 2.0)]);
        assert_eq!(one, "0.000000+(T-0.000000)/2.000000");
        assert!(!one.contains("if("));
        // Two segments → exactly one branch boundary at the first segment's end.
        let two = build_speed_setpts_expr(&[seg(0.0, 4.0, 1.0), seg(4.0, 8.0, 2.0)]);
        assert_eq!(two.matches("if(lt(T,").count(), 1);
        assert!(two.contains("if(lt(T,4.000000)"));
    }

    #[test]
    fn clamp_segment_speed_guards_bad_values_and_clamps_range() {
        // Non-positive / non-finite collapse to 1× (never 0 → no atempo hang or
        // setpts divide-by-zero downstream).
        assert_eq!(clamp_segment_speed(0.0), 1.0);
        assert_eq!(clamp_segment_speed(-2.0), 1.0);
        assert_eq!(clamp_segment_speed(f64::NAN), 1.0);
        assert_eq!(clamp_segment_speed(f64::INFINITY), 1.0);
        // In-range passes through; out-of-range clamps to [0.25, 4.0].
        assert_eq!(clamp_segment_speed(1.5), 1.5);
        assert_eq!(clamp_segment_speed(0.1), 0.25);
        assert_eq!(clamp_segment_speed(99.0), 4.0);
    }

    fn cut(start: f64, end: f64) -> CutRange {
        CutRange {
            start,
            end,
            extra: Default::default(),
        }
    }

    fn state_with_cuts(cuts: Vec<CutRange>) -> RenderState {
        RenderState {
            cuts,
            ..Default::default()
        }
    }

    #[test]
    fn select_expr_keeps_everything_outside_the_cuts() {
        // The export drops frames where this expression is false. Two cuts →
        // keep = not(in cut A OR in cut B).
        let expr = build_cut_select_expr(&[(1.5, 2.0), (4.0, 5.5)]);
        assert_eq!(expr, "not(between(t,1.500,2.000)+between(t,4.000,5.500))");
    }

    #[test]
    fn select_expr_single_cut() {
        assert_eq!(
            build_cut_select_expr(&[(2.0, 3.0)]),
            "not(between(t,2.000,3.000))"
        );
    }

    #[test]
    fn ripple_delete_in_middle_offsets_into_post_trim_time() {
        // Project trimmed to [10,20]; a ripple-deleted clip at original [12,14]
        // must reach ffmpeg as post-trim [2,4] (the input is seeked by -ss 10).
        let cuts = collect_export_cuts(&state_with_cuts(vec![cut(12.0, 14.0)]), 10.0, 20.0);
        assert_eq!(cuts, vec![(2.0, 4.0)]);
    }

    #[test]
    fn cut_outside_trim_is_dropped_and_straddling_is_clamped() {
        // [0,5] is entirely before the trim → dropped; [8,12] straddles the in
        // point → clamped to [10,12] → post-trim [0,2].
        let cuts = collect_export_cuts(
            &state_with_cuts(vec![cut(0.0, 5.0), cut(8.0, 12.0)]),
            10.0,
            20.0,
        );
        assert_eq!(cuts, vec![(0.0, 2.0)]);
    }

    #[test]
    fn overlapping_cuts_merge() {
        // post-trim [1,3] and [2,5] overlap → single [1,5].
        let cuts = collect_export_cuts(
            &state_with_cuts(vec![cut(11.0, 13.0), cut(12.0, 15.0)]),
            10.0,
            20.0,
        );
        assert_eq!(cuts, vec![(1.0, 5.0)]);
    }

    #[test]
    fn no_cuts_yields_empty() {
        assert!(collect_export_cuts(&state_with_cuts(vec![]), 0.0, 10.0).is_empty());
    }

    #[test]
    fn kept_duration_matches_shared_parity_fixtures() {
        // Anti-drift guard. This loads the SAME json the frontend asserts against
        // (cuts.test.ts → "cut/export parity"). For every case, this export's
        // output duration — (trim length) minus the merged cut spans — must equal
        // `expectedKeptDuration`, which the frontend also matches against its
        // collapsed-timeline length. If the two cut models ever diverge, one of
        // these two tests fails.
        let raw = include_str!("../../../src/lib/timeline/__fixtures__/cut-parity.json");
        let doc: serde_json::Value = serde_json::from_str(raw).expect("valid fixture json");
        let cases = doc["cases"].as_array().expect("cases array");
        for case in cases {
            let name = case["name"].as_str().unwrap_or("?");
            let trim_start = case["trimStart"].as_f64().unwrap();
            let trim_end = case["trimEnd"].as_f64().unwrap();
            let expected = case["expectedKeptDuration"].as_f64().unwrap();
            let cuts: Vec<CutRange> = case["cuts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|pair| {
                    let p = pair.as_array().unwrap();
                    cut(p[0].as_f64().unwrap(), p[1].as_f64().unwrap())
                })
                .collect();

            let merged = collect_export_cuts(&state_with_cuts(cuts), trim_start, trim_end);
            let removed: f64 = merged.iter().map(|(a, b)| b - a).sum();
            let kept = (trim_end - trim_start) - removed;
            assert!(
                (kept - expected).abs() < 1e-6,
                "parity case '{name}': export kept duration {kept} != expected {expected}"
            );
        }
    }

    #[test]
    fn warped_duration_matches_shared_parity_fixtures() {
        // Anti-drift guard for per-segment speed. Loads the SAME json the frontend
        // asserts against (segment-speed.test.ts → "speed parity"). For every case
        // the export's warped output duration must equal the frontend time-map's,
        // or the two speed models have diverged. All cases use trimStart=0.
        let raw = include_str!("../../../src/lib/timeline/__fixtures__/speed-parity.json");
        let doc: serde_json::Value = serde_json::from_str(raw).expect("valid fixture json");
        for case in doc["cases"].as_array().expect("cases array") {
            let name = case["name"].as_str().unwrap_or("?");
            let trim_end = case["trimEnd"].as_f64().unwrap();
            let expected = case["expectedOutputDuration"].as_f64().unwrap();
            let cuts: Vec<CutRange> = case["cuts"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let p = p.as_array().unwrap();
                    cut(p[0].as_f64().unwrap(), p[1].as_f64().unwrap())
                })
                .collect();
            let split_points: Vec<f64> = case["splitPoints"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap())
                .collect();
            let speeds: Vec<SegmentSpeed> = case["segmentSpeeds"]
                .as_array()
                .unwrap()
                .iter()
                .map(|p| {
                    let p = p.as_array().unwrap();
                    SegmentSpeed {
                        start: p[0].as_f64().unwrap(),
                        speed: p[1].as_f64().unwrap(),
                    }
                })
                .collect();

            let merged = collect_export_cuts(&state_with_cuts(cuts), 0.0, trim_end);
            let segs = build_speed_segments(trim_end, &merged, &split_points, &speeds, 0.0);
            let got = warped_output_duration(&segs);
            assert!(
                (got - expected).abs() < 1e-6,
                "speed parity '{name}': warped duration {got} != expected {expected}"
            );
        }
    }

    #[test]
    fn atempo_chain_covers_the_speed_range() {
        assert_eq!(atempo_chain(1.5), "atempo=1.500000");
        assert_eq!(atempo_chain(4.0), "atempo=2.000000,atempo=2.000000");
        assert_eq!(atempo_chain(0.25), "atempo=0.500000,atempo=0.500000");
    }

    #[test]
    fn setpts_expr_warps_a_two_segment_clip() {
        // [0,4]@1x then [4,10]@2x → second segment maps T into half-rate output.
        let segs = build_speed_segments(
            10.0,
            &[],
            &[4.0],
            &[SegmentSpeed {
                start: 4.0,
                speed: 2.0,
            }],
            0.0,
        );
        let expr = build_speed_setpts_expr(&segs);
        assert_eq!(
            expr,
            "if(lt(T,4.000000),0.000000+(T-0.000000)/1.000000,4.000000+(T-4.000000)/2.000000)"
        );
    }
}

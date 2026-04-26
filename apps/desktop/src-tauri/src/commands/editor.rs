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
use crate::render::mask_export::{render_border_radius_mask, MaskResult};
use crate::render::node_types::AudioSettings;

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
        )
        .map_err(|e| e.to_string())?;

    let canvas_padding = {
        let pct = request.render_state.padding.clamp(0.0, 20.0);
        let shorter_edge = metadata.width.min(metadata.height) as f64;
        ((shorter_edge * pct) / 100.0).round() as u32
    };
    let canvas_width = metadata.width + canvas_padding * 2;
    let canvas_height = metadata.height + canvas_padding * 2;
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
                    canvas_width,
                    canvas_height,
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
                + watermark_path.is_some() as usize;
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
                        emit_export_state(
                            &stderr_app,
                            ExportStateEvent::progress(&stderr_export_id, pct),
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

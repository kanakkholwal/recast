use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use base64::{engine::general_purpose, Engine as _};
use tauri::{AppHandle, Manager, State};

use super::error::{AppError, AppResult};
use super::export::cuts_speed::{
    build_cut_select_expr, build_speed_audio_filter, build_speed_segments, build_speed_setpts_expr,
    collect_export_cuts, has_speed_change, output_duration_cap,
};
use super::export::gif::run_gif_palette_prepass;
use super::export::progress::ProgressBand;
use super::export::run::run_encode;
use super::export::state::{emit_export_state, ExportStateEvent};
use super::ffmpeg::{
    append_camera_overlay_to_complex, append_cursor_overlay_to_complex,
    append_output_filters_to_complex, append_subtitles_to_complex, build_annotation_blur_complex,
    build_gif_paletteuse_external_complex, build_output_scale_filter, has_audio,
    probe_video_metadata, resolve_export_profile, BlurRegion, CameraOverlayParams, ExportSpeed,
    GifFilterOptions,
};
use super::system::get_active_output_dir;
use super::types::{AppState, EditorDocument, ExportRequest, GifSettings, VideoMetadata};
use crate::project::reader::ProjectOpenResult;
#[allow(unused_imports)]
use crate::render::cursor_export::{render_cursor_overlay, CursorOverlayRequest};
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};
use crate::render::mask_export::{render_border_radius_mask, MaskResult};
use crate::render::node_types::{AnnotationAnchor, AnnotationKind, AudioSettings};

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

#[tauri::command]
pub async fn get_video_metadata(path: String) -> AppResult<VideoMetadata> {
    // ffprobe spawn off the main thread — see generate_thumbnails for context.
    tauri::async_runtime::spawn_blocking(move || project_or_media_metadata(Path::new(&path)))
        .await
        .map_err(|e| AppError::msg(format!("get_video_metadata join error: {e}")))?
        .map_err(Into::into)
}

#[tauri::command]
pub async fn load_editor_document(path: String) -> AppResult<EditorDocument> {
    tauri::async_runtime::spawn_blocking(move || load_editor_document_blocking(path))
        .await
        .map_err(|e| AppError::msg(format!("load_editor_document join error: {e}")))?
        .map_err(Into::into)
}

fn load_editor_document_blocking(path: String) -> Result<EditorDocument, String> {
    let input = PathBuf::from(&path);
    if let Some(project) = open_project_if_needed(&input)? {
        let default_state = || RenderState {
            trim_end: project.metadata.video.duration_ms as f64 / 1000.0,
            ..RenderState::default()
        };
        // A missing edits.json is a fresh project (expected → defaults). A parse
        // FAILURE, though, would silently discard every edit, so surface it.
        let render_state = match fs::read_to_string(&project.edits_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::error!(
                    "failed to parse edits.json ({}): {e}; loading defaults (edits not applied)",
                    project.edits_path.display()
                );
                default_state()
            }),
            Err(_) => default_state(),
        };

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
pub async fn generate_thumbnails(path: String, count: u32) -> AppResult<Vec<String>> {
    // Sync ffmpeg/ffprobe calls run on Tauri's main thread by default,
    // freezing the UI (clicks/touch/window-drag) for the duration. Move the
    // whole pipeline onto a blocking worker so the event loop stays free —
    // /recasts fires this once per recording in parallel from JS, and the
    // serialised main-thread ffmpeg spawns produced multi-second freezes.
    tauri::async_runtime::spawn_blocking(move || generate_thumbnails_blocking(path, count))
        .await
        .map_err(|e| AppError::msg(format!("generate_thumbnails join error: {e}")))?
        .map_err(Into::into)
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

#[tauri::command]
pub async fn export_video(
    app: AppHandle,
    mut request: ExportRequest,
    state: State<'_, AppState>,
) -> AppResult<String> {
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
            .map_err(|e| AppError::msg(format!("border-radius mask render failed: {e}")))?
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
            .map_err(|e| AppError::msg(format!("drop-shadow mask render failed: {e}")))?
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
        .map_err(|e| AppError::msg(format!("gradient background render failed: {e}")))?
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
        .map_err(AppError::msg)?;
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
            .map_err(AppError::msg)?
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
                .map_err(|e| AppError::msg(format!("camera mask render failed: {e}")))?
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
                radius: corner_frac,
                ..
            } => {
                // UV → canvas-pixel rect, over the annotation's anchor rect:
                // the video region (video anchor, matches preview) or the padded
                // frame (frame anchor). Identical to the old full-canvas mapping
                // when there's no padding. Static either way — FFmpeg can't
                // follow a per-frame zoom, so a zoomed video-anchored blur holds
                // its un-zoomed spot.
                let (rx, ry, rw_ref, rh_ref) = match a.anchor {
                    AnnotationAnchor::Frame => (
                        canvas_geom.comp_x as f64,
                        canvas_geom.comp_y as f64,
                        comp_width as f64,
                        comp_height as f64,
                    ),
                    AnnotationAnchor::Video => (
                        canvas_geom.video_x as f64,
                        canvas_geom.video_y as f64,
                        canvas_geom.video_w as f64,
                        canvas_geom.video_h as f64,
                    ),
                };
                let cx = (rx + x * rw_ref).round() as i32;
                let cy = (ry + y * rh_ref).round() as i32;
                let cw = (w.abs() * rw_ref).round() as i32;
                let ch = (h.abs() * rh_ref).round() as i32;
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
                // Corner radius as a fraction (0..0.5) of the region's shorter
                // side — same basis as the preview's `radius * min(w, h)`.
                let corner_px = corner_frac.clamp(0.0, 0.5) * (cw.min(ch) as f64);
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
                    corner_px,
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
    let progress_band = if request.format == "gif" {
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

        // Cuts AND per-segment speed apply to GIF too, but its two-pass palette
        // path runs before the generic (MP4/WebM-only) cut+speed stage below, so
        // build the same select+setpts warp here and inject it into both the
        // palette pre-pass and the main pass. GIF has no audio, so there's no
        // atempo counterpart; the downstream `fps=` resamples the warped PTS to
        // CFR (dropping/duplicating frames as the speed demands).
        let gif_cut_select: Option<String> = {
            let export_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
            let gif_speed_segments = build_speed_segments(
                duration,
                &export_cuts,
                &request.render_state.split_points,
                &request.render_state.segment_speeds,
                trim_start,
            );
            let gif_speed_active = has_speed_change(&gif_speed_segments);
            let has_cuts = !export_cuts.is_empty();
            (has_cuts || gif_speed_active).then(|| {
                let select_prefix = if has_cuts {
                    format!("select='{}',", build_cut_select_expr(&export_cuts))
                } else {
                    String::new()
                };
                let setpts = if gif_speed_active {
                    // Single-quote: the warp expression contains commas the
                    // filtergraph parser would otherwise read as separators.
                    format!(
                        "setpts='({})/TB'",
                        build_speed_setpts_expr(&gif_speed_segments)
                    )
                } else {
                    "setpts=N/FRAME_RATE/TB".to_string()
                };
                format!("{select_prefix}{setpts}")
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
                ProgressBand {
                    offset: 0.0,
                    scale: 0.4,
                },
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
                    return Err(AppError::from("export cancelled"));
                }
                emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
                return Err(AppError::from(err_msg));
            }
            Err(join_err) => {
                state.export_cancel.lock().remove(&export_id);
                let _ = std::fs::remove_file(&palette_path);
                let err_msg = format!("export task failed (palette pre-pass): {join_err}");
                emit_export_state(&app, ExportStateEvent::error(&export_id, &err_msg));
                return Err(AppError::from(err_msg));
            }
        }

        if cancel_flag.load(Ordering::Acquire) {
            state.export_cancel.lock().remove(&export_id);
            let _ = std::fs::remove_file(&palette_path);
            emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
            return Err(AppError::from("export cancelled"));
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

        ProgressBand {
            offset: 40.0,
            scale: 0.6,
        }
    } else {
        if let Some(scale_filter) = output_scale_filter {
            output_filters.push(scale_filter);
        }
        ProgressBand {
            offset: 0.0,
            scale: 1.0,
        }
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
            // Export-quality codec args. NVENC/AMF/QSV get hardware rate control
            // tuned for quality (not the lowlatency presets used for live
            // recording); libx264 uses the user's chosen profile preset because
            // export isn't bound by real-time pacing. See `encoder::h264`.
            args.extend(crate::encoder::h264::codec_args(
                crate::encoder::h264::H264Encoder::from_ffmpeg_name(
                    crate::ffmpeg::preferred_h264_encoder(),
                ),
                crate::encoder::h264::EncodePurpose::Export(
                    crate::encoder::h264::ExportEncodeParams {
                        nvenc_preset: speed.nvenc_preset(),
                        amf_quality: speed.amf_quality(),
                        qsv_preset: speed.qsv_preset(),
                        x264_preset: speed.x264_preset().unwrap_or(profile.mp4_preset),
                        cq: profile.mp4_nvenc_cq,
                        crf: profile.mp4_crf,
                    },
                ),
            ));
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
        run_encode(
            args,
            app,
            export_id_for_task,
            cancel_flag,
            output_path_str,
            expected_output_secs,
            progress_band,
        )
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
            inner.map_err(Into::into)
        }
        Err(join_err) => {
            // spawn_blocking only errors on panic; surface it so the frontend
            // can show a real failure dialog instead of hanging on the Promise.
            let err_msg = format!("export task failed: {join_err}");
            emit_export_state(
                &app_for_fallback,
                ExportStateEvent::error(&export_id_for_fallback, &err_msg),
            );
            Err(AppError::from(err_msg))
        }
    }
}

/// Signal any running export to abort. The watchdog thread polls this flag every
/// ~250ms and kills the ffmpeg child process, which causes `export_video` to
/// return `Err("export cancelled")`. Safe to call when no export is running
/// for the given export session id.
#[tauri::command]
pub fn cancel_export(export_id: String, state: State<'_, AppState>) -> AppResult<()> {
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
pub async fn autosave_project(project_path: String, edits_json: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::project::autosave::save_autosave(Path::new(&project_path), &edits_json)
            .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| AppError::msg(format!("autosave task panicked: {e}")))?
    .map_err(Into::into)
}

/// Re-pack a legacy `.recast` as the current format in place (keeps a `.bak`).
/// Heavy zip I/O, so it runs off the main thread.
#[tauri::command]
pub async fn migrate_project(project_path: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || {
        crate::project::migrate_project(Path::new(&project_path))
    })
    .await
    .map_err(|e| AppError::msg(format!("migrate task panicked: {e}")))?
    .map_err(AppError::from)
}

#[tauri::command]
pub async fn save_project_edits(project_path: String, edits_json: String) -> AppResult<u64> {
    let path_for_blocking = project_path.clone();
    tokio::task::spawn_blocking(move || {
        crate::project::writer::update_project_edits(Path::new(&path_for_blocking), &edits_json)
    })
    .await
    .map_err(|e| AppError::msg(format!("save task panicked: {e}")))?
    .map_err(AppError::msg)?;

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
) -> AppResult<Vec<crate::cursor::smoothing::ZoomTrigger>> {
    // The cursor track is multi-MB on long recordings; read + parse off-thread.
    tauri::async_runtime::spawn_blocking(move || {
        let bytes =
            fs::read(Path::new(&cursor_path)).map_err(|e| format!("read cursor track: {e}"))?;
        let track: crate::cursor::CursorTrack =
            serde_json::from_slice(&bytes).map_err(|e| format!("parse cursor track: {e}"))?;
        Ok::<_, String>(crate::cursor::smoothing::detect_zoom_triggers(
            &track.samples,
            &track.clicks,
        ))
    })
    .await
    .map_err(|e| AppError::msg(format!("suggest task panicked: {e}")))?
    .map_err(Into::into)
}

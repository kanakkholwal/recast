use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Instant;

use base64::{engine::general_purpose, Engine as _};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{AppError, AppResult};
use super::export::align::{audio_align_filter, camera_input_offset_secs};
use super::export::camera::{build_camera_follow_exprs, camera_bubble_rect, camera_shadow_geom};
use super::export::captions::append_caption_burn_in;
use super::export::codec::append_codec_args;
use super::export::cuts_speed::{
    append_cut_speed_stage, build_speed_segments, collect_export_cuts, has_speed_change,
    resolve_speed_segments, warped_output_duration,
};
use super::export::gif::{run_gif_palette_prepass, run_gif_pass, GifPassError, GifPassParams};
use super::export::progress::ProgressBand;
use super::export::run::{is_ffmpeg_crash_code, parse_ffmpeg_exit_code, run_encode};
use super::export::state::{emit_export_state, ExportStateEvent};
use super::ffmpeg::{
    append_camera_overlay_to_complex, append_cursor_overlay_to_complex,
    append_output_filters_to_complex, build_annotation_blur_complex,
    build_gif_paletteuse_external_complex, build_output_scale_filter, has_audio,
    probe_video_metadata, resolve_export_profile, CameraOverlayAnim, CameraOverlayParams,
    CameraShadowOverlay, ExportSpeed, GifFilterOptions,
};
use super::system::get_active_output_dir;
use super::types::{
    AppState, CameraCapture, EditorDocument, ExportRequest, GifSettings, VideoMetadata,
};
use crate::project::reader::ProjectOpenResult;
use crate::recording::TrackOffsets;
#[allow(unused_imports)]
use crate::render::cursor_export::{render_cursor_overlay, CursorOverlayRequest};
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};
use crate::render::mask_export::MaskResult;
use crate::render::node_types::{AnnotationKind, AudioSettings};

/// Filtergraph length past which we pass it via `-filter_complex_script <file>`
/// rather than inline, to stay under Windows' ~32 KB command-line limit. Well
/// below the limit so the rest of the command line (inputs, codec args) fits.
const FILTER_COMPLEX_SCRIPT_THRESHOLD: usize = 8000;

fn static_root() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let candidate = cwd.join("..").join("static");
    if candidate.exists() {
        candidate
    } else {
        cwd.join("static")
    }
}

/// Pre-bakes a static image background to a canvas-sized blurred PNG once, saving ~19.5 ms/frame of identical re-blur on a 120 fps export.
/// Uses the exact scale, crop and blur the graph would, so the result is pixel-identical; returns `None` on any failure and the caller keeps the per-frame path.
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
            duration: project.metadata.media_duration_secs(),
            width: project.metadata.video.width,
            height: project.metadata.video.height,
            fps: project.metadata.video.fps as f64,
            codec: "h264".into(),
            size_bytes: fs::metadata(path).map(|m| m.len()).unwrap_or_default(),
        });
    }
    probe_video_metadata(path)
}

/// Which capture an export audio input came from, so per-source gain/mute maps
/// to the right FFmpeg input. `Source` = a single file's embedded track (master
/// gain only); `System`/`Mic` = the project's separate WAV captures.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum AudioKind {
    Source,
    System,
    Mic,
}

/// This track's measured lag behind video frame 0, or `None` when it was never
/// measured (pre-offset bundles) or is already muxed into the source video.
fn offset_for(kind: AudioKind, offsets: TrackOffsets) -> Option<i64> {
    match kind {
        AudioKind::Source => None,
        AudioKind::System => offsets.audio_ms,
        AudioKind::Mic => offsets.microphone_ms,
    }
}

/// Effective linear gain for one input: master × its per-source gain, 0 when muted. Mirrors the preview's `effectiveTrackVolume` so preview and export apply the same mix.
fn effective_audio_gain(settings: &AudioSettings, kind: AudioKind) -> f64 {
    let master = (settings.volume / 100.0).clamp(0.0, 4.0);
    let (vol, muted) = match kind {
        AudioKind::Source => return master,
        AudioKind::System => (settings.system_volume, settings.system_muted),
        AudioKind::Mic => (settings.mic_volume, settings.mic_muted),
    };
    if muted {
        0.0
    } else {
        master * (vol / 100.0).clamp(0.0, 4.0)
    }
}

fn append_audio_to_complex(
    existing: Option<&str>,
    audio_inputs: &[(usize, AudioKind)],
    settings: &AudioSettings,
    trim_start: f64,
    duration: f64,
    offsets: TrackOffsets,
) -> Option<(String, String)> {
    if audio_inputs.is_empty() || settings.muted || settings.volume <= 0.0 {
        return None;
    }

    // Drop fully-silenced sources: a muted input left in the amix averages the others back down.
    let live: Vec<(usize, f64, AudioKind)> = audio_inputs
        .iter()
        .map(|&(idx, kind)| (idx, effective_audio_gain(settings, kind), kind))
        .filter(|&(_, gain, _)| gain > 0.0)
        .collect();
    if live.is_empty() {
        return None;
    }

    let mut segments: Vec<String> = existing
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty())
        .into_iter()
        .collect();
    let mut labels = Vec::new();

    for (i, (input_index, gain, kind)) in live.iter().enumerate() {
        let label = if live.len() == 1 {
            "aout".to_string()
        } else {
            format!("aud{i}")
        };
        let mut filters = Vec::new();
        if let Some(align) = audio_align_filter(offset_for(*kind, offsets)) {
            filters.push(align);
        }
        if duration > 0.0 {
            filters.push(format!(
                "atrim=start={:.3}:duration={:.3}",
                trim_start.max(0.0),
                duration
            ));
        } else if trim_start > 0.0 {
            filters.push(format!("atrim=start={trim_start:.3}"));
        }
        filters.push("asetpts=PTS-STARTPTS".to_string());
        filters.push(format!("volume={gain:.4}"));
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

    if live.len() > 1 {
        segments.push(format!(
            "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[aout]",
            labels.join(""),
            live.len()
        ));
    }

    // -14 LUFS / -1 dBTP is the common social target; single-pass, a measured two-pass is later work.
    if settings.normalize_loudness {
        segments.push("[aout]loudnorm=I=-14:TP=-1:LRA=11[aoutn]".to_string());
        return Some((segments.join(";"), "[aoutn]".into()));
    }

    Some((segments.join(";"), "[aout]".into()))
}

/// Mixes output-timeline music clips onto the finished source audio; `clips` pairs each with its ffmpeg input index and `source_audio` is `None` when muted.
/// Each clip is trimmed into its source, gained, faded and delayed onto the output timeline, then amixed; returns the extra filter segments and the final map.
fn build_music_stage(
    clips: &[(usize, &crate::render::node_types::AudioClip)],
    source_audio: Option<&str>,
    output_duration: f64,
) -> Option<(String, String)> {
    if clips.is_empty() {
        return source_audio.map(|s| (String::new(), s.to_string()));
    }
    let mut segments: Vec<String> = Vec::new();
    let mut labels: Vec<String> = Vec::new();
    if let Some(src) = source_audio {
        labels.push(src.to_string());
    }
    for (i, (input_index, clip)) in clips.iter().enumerate() {
        let label = format!("mus{i}");
        let mut f: Vec<String> = Vec::new();
        let start = clip.offset_sec.max(0.0);
        // duration 0 = fill to the end of the output (never past it).
        let play = if clip.duration_sec > 0.0 {
            clip.duration_sec
        } else {
            (output_duration - clip.start_output_sec).max(0.0)
        };
        if play > 0.0 {
            f.push(format!("atrim=start={start:.3}:duration={play:.3}"));
        } else if start > 0.0 {
            f.push(format!("atrim=start={start:.3}"));
        }
        f.push("asetpts=PTS-STARTPTS".to_string());
        let gain = (clip.gain / 100.0).clamp(0.0, 4.0);
        f.push(format!("volume={gain:.4}"));
        if clip.fade_in > 0.0 {
            let fi = if play > 0.0 {
                clip.fade_in.min(play)
            } else {
                clip.fade_in
            };
            f.push(format!("afade=t=in:st=0:d={fi:.3}"));
        }
        if clip.fade_out > 0.0 && play > 0.0 {
            let fo = clip.fade_out.min(play);
            f.push(format!(
                "afade=t=out:st={:.3}:d={fo:.3}",
                (play - fo).max(0.0)
            ));
        }
        // Place on the output timeline (adelay adds leading silence per channel).
        let delay_ms = (clip.start_output_sec.max(0.0) * 1000.0).round() as i64;
        if delay_ms > 0 {
            f.push(format!("adelay={delay_ms}|{delay_ms}"));
        }
        segments.push(format!("[{input_index}:a]{}[{label}]", f.join(",")));
        labels.push(format!("[{label}]"));
    }
    if labels.len() == 1 {
        return Some((segments.join(";"), labels[0].clone()));
    }
    segments.push(format!(
        "{}amix=inputs={}:duration=longest:dropout_transition=0:normalize=0[afinal]",
        labels.join(""),
        labels.len()
    ));
    Some((segments.join(";"), "[afinal]".into()))
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
        let media_duration = project.metadata.media_duration_secs();
        let default_state = || RenderState {
            trim_end: media_duration,
            ..RenderState::default()
        };
        // A missing edits.json is a fresh project, but a parse FAILURE would silently discard every edit.
        let mut render_state: RenderState = match fs::read_to_string(&project.edits_path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|e| {
                log::error!(
                    "failed to parse edits.json ({}): {e}; loading defaults (edits not applied)",
                    project.edits_path.display()
                );
                default_state()
            }),
            Err(_) => default_state(),
        };
        // Projects predating `media_duration_secs` took trim_end from the wall clock and overshoot the encoded file.
        if media_duration > 0.0 && render_state.trim_end > media_duration {
            render_state.trim_end = media_duration;
        }

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
            // `media` is absent on bundles written before it existed, which must not read as 'camera off'.
            camera_capture: match project.metadata.media.as_ref() {
                Some(media) if media.has_camera => CameraCapture::Separate,
                // Asked for but never arrived; older bundles default this false and fall through to `Off`.
                Some(media) if media.camera_requested => CameraCapture::Failed,
                Some(_) => CameraCapture::Off,
                None => CameraCapture::Legacy,
            },
            track_offsets: project
                .metadata
                .media
                .as_ref()
                .map(|m| m.track_offsets)
                .unwrap_or_default(),
            metadata: VideoMetadata {
                duration: media_duration,
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
        // No camera was recorded for a plain video; `Legacy` would claim it is an old Recast bundle.
        camera_capture: CameraCapture::Off,
        track_offsets: Default::default(),
        metadata: metadata.clone(),
        render_state: RenderState {
            trim_end: metadata.duration,
            ..RenderState::default()
        },
        needs_migration: false,
    })
}

/// Read-only timeline summary, mirroring the shape `deriveSegments` and `timeMapFromSegments` produce; shared parity fixtures hold the two to the same precision.
/// One pass over cuts and split points on in-memory state, so a control-socket arm can build it without `spawn_blocking`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectTimeline {
    /// Source-recording duration in seconds (full clip, before trim/cuts).
    pub source_duration: f64,
    /// Trimmed window in original-recording coords (inclusive).
    pub trim_start: f64,
    pub trim_end: f64,
    /// `trim_end - trim_start`, clamped to `>= 0`.
    pub trimmed_duration: f64,
    /// Output-time duration after cuts and per-segment speed warp are applied.
    pub output_duration: f64,
    /// Cuts in original-recording coords, kept verbatim from the render state.
    pub cuts: Vec<TimelineCut>,
    /// Kept segments on the post-trim stream (t=0 at trim_start) with their speed.
    pub kept_segments: Vec<KeptSegment>,
    /// Split points (original-recording coords) the user dropped on the clip.
    pub split_points: Vec<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineCut {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeptSegment {
    pub start: f64,
    pub end: f64,
    pub speed: f64,
}

pub fn derive_project_timeline(
    render_state: &RenderState,
    source_duration: f64,
) -> ProjectTimeline {
    let trim_start = render_state.trim_start.max(0.0);
    let trim_end = render_state.trim_end.max(trim_start);
    let trimmed = (trim_end - trim_start).max(0.0);
    let export_cuts = collect_export_cuts(render_state, trim_start, trim_end);
    let speeds = build_speed_segments(
        trimmed,
        &export_cuts,
        &render_state.split_points,
        &render_state.segment_speeds,
        trim_start,
    );
    let output_duration = warped_output_duration(&speeds);

    ProjectTimeline {
        source_duration,
        trim_start,
        trim_end,
        trimmed_duration: trimmed,
        output_duration,
        cuts: render_state
            .cuts
            .iter()
            .map(|c| TimelineCut {
                start: c.start,
                end: c.end,
            })
            .collect(),
        kept_segments: speeds
            .iter()
            .map(|s| KeptSegment {
                start: s.start,
                end: s.end,
                speed: s.speed,
            })
            .collect(),
        split_points: render_state.split_points.clone(),
    }
}

/// The load, mutate, validate, save cycle every targeted editor verb in `control::dispatch` shares; the mutate closure returns its own result.
/// Spawn-blocking the load and save is fine: both already run on Tauri's blocking pool internally.
pub(crate) fn patch_render_state<F, M>(
    state: &crate::commands::types::AppState,
    app: &tauri::AppHandle,
    path: &str,
    writer_id: &str,
    mutate: F,
) -> Result<M, String>
where
    F: FnOnce(&mut RenderState) -> Result<M, String>,
{
    use crate::commands::types::EditorWriterKind;

    let path_buf = std::path::PathBuf::from(path);
    crate::commands::try_acquire_write(
        state,
        path_buf,
        EditorWriterKind::Agent,
        writer_id.to_string(),
    )
    .map_err(|e| e.to_string())?;

    let doc =
        tauri::async_runtime::block_on(crate::commands::load_editor_document(path.to_string()))
            .map_err(|e| e.to_string())?;

    let mut new_state = doc.render_state;
    let result = mutate(&mut new_state)?;

    if let Err(issues) = validate_render_state(&new_state, doc.metadata.duration) {
        return Err(format!(
            "validation failed: {}",
            serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}"))
        ));
    }

    let edits_json = serde_json::to_string(&new_state).map_err(|e| e.to_string())?;
    tauri::async_runtime::block_on(crate::commands::save_project_edits(
        path.to_string(),
        edits_json,
    ))
    .map_err(|e| e.to_string())?;

    crate::commands::record_activity(state);
    // `commit` persists AND broadcasts the lock, so the GUI learns on the agent's first patch, not the next poll.
    crate::commands::editor_session::commit(state, app);
    let _ = app.emit("editor-state:changed", serde_json::json!({ "path": path }));
    Ok(result)
}

/// Single invariant violation. The agent/CI/UI inspect `reason` to map to a
/// user-facing message; `field` is a dotted path so an editor UI can navigate
/// to the offending control. `reason` is stable: renaming a code is a breaking
/// change for any agent that branches on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub field: String,
    pub reason: String,
}

/// Tolerance for "equal" comparisons in render-state math. Mirrors the
/// `CUT_MERGE_EPS` used by `commands/export/cuts_speed.rs` so a validator and
/// the export agree on what "the same boundary" means.
const VALIDATION_EPS: f64 = 1e-4;

/// The band the export validates a zoom against, and the UI slider offers.
const ZOOM_SCALE_MIN: f64 = 1.0;
const ZOOM_SCALE_MAX: f64 = 3.0;

/// Brings a validated value back into its band. Non-finite takes `fallback`:
/// `f64::clamp` returns NaN for NaN, so clamping alone fixes nothing.
fn repair_into(value: &mut f64, min: f64, max: f64, fallback: f64) -> bool {
    let fixed = if value.is_finite() {
        value.clamp(min, max)
    } else {
        fallback
    };
    if (fixed - *value).abs() > VALIDATION_EPS || !value.is_finite() {
        *value = fixed;
        return true;
    }
    false
}

/// Clamps the trim window and cuts to the real source duration, repairing `trim_end_exceeds_source` in projects saved before the CFR fix.
/// Mutates in place and returns a description per change. Run BEFORE `validate_render_state`.
pub fn repair_render_state(s: &mut RenderState, source_duration: f64) -> Vec<String> {
    let mut repairs = Vec::new();

    // Validated but never repaired, so one recoverable value killed the export.
    let mut fixed_zoom = false;
    for z in s.zoom_regions.iter_mut() {
        // Non-finite becomes 1.0, which is no zoom: the safe reading of nonsense.
        fixed_zoom |= repair_into(&mut z.scale, ZOOM_SCALE_MIN, ZOOM_SCALE_MAX, ZOOM_SCALE_MIN);
        fixed_zoom |= repair_into(&mut z.center_x, 0.0, 1.0, 0.5);
        fixed_zoom |= repair_into(&mut z.center_y, 0.0, 1.0, 0.5);
        fixed_zoom |= repair_into(&mut z.ramp_in, 0.0, f64::MAX, 0.0);
        fixed_zoom |= repair_into(&mut z.ramp_out, 0.0, f64::MAX, 0.0);
    }
    if fixed_zoom {
        repairs.push("Zoom regions clamped back into range".to_string());
    }

    // Frame-level controls; fallbacks are the shipped defaults, so a corrupt value lands on what a fresh project would have used.
    let mut fixed_frame = false;
    fixed_frame |= repair_into(&mut s.border_radius, 0.0, 50.0, 0.0);
    fixed_frame |= repair_into(&mut s.padding, 0.0, 20.0, 0.0);
    fixed_frame |= repair_into(&mut s.cursor_size, 0.0, f64::MAX, 3.0);
    fixed_frame |= repair_into(&mut s.cursor_smoothing, 0.0, 100.0, 50.0);
    fixed_frame |= repair_into(&mut s.cursor_highlight_opacity, 0.0, 100.0, 40.0);
    if fixed_frame {
        repairs.push("Frame and cursor settings clamped back into range".to_string());
    }

    // A speed of zero or less is not slow, it is a division by zero downstream.
    let mut fixed_speed = false;
    for sp in s.segment_speeds.iter_mut() {
        if !sp.speed.is_finite() || sp.speed <= 0.0 {
            sp.speed = 1.0;
            fixed_speed = true;
        }
    }
    if fixed_speed {
        repairs.push("Segment speeds reset to normal".to_string());
    }

    // Annotation geometry; `w`/`h` are unvalidated (negative is legal mid-drag), so only what the export refuses is touched.
    let mut fixed_anno = false;
    for a in s.annotations.iter_mut() {
        fixed_anno |= repair_into(&mut a.opacity, 0.0, 1.0, 1.0);
        match &mut a.kind {
            AnnotationKind::Rect { x, y, radius, .. } => {
                fixed_anno |= repair_into(x, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(y, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(radius, 0.0, 0.5, 0.0);
            }
            AnnotationKind::Ellipse { x, y, .. } | AnnotationKind::Text { x, y, .. } => {
                fixed_anno |= repair_into(x, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(y, 0.0, 1.0, 0.0);
            }
            AnnotationKind::Blur {
                x,
                y,
                strength,
                radius,
                ..
            } => {
                fixed_anno |= repair_into(x, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(y, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(strength, 0.0, 1.0, 1.0);
                fixed_anno |= repair_into(radius, 0.0, 0.5, 0.0);
            }
            AnnotationKind::Image {
                x,
                y,
                opacity,
                radius,
                ..
            } => {
                fixed_anno |= repair_into(x, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(y, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(opacity, 0.0, 1.0, 1.0);
                fixed_anno |= repair_into(radius, 0.0, 0.5, 0.0);
            }
            AnnotationKind::Arrow { x1, y1, x2, y2, .. } => {
                fixed_anno |= repair_into(x1, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(y1, 0.0, 1.0, 0.0);
                fixed_anno |= repair_into(x2, 0.0, 1.0, 1.0);
                fixed_anno |= repair_into(y2, 0.0, 1.0, 1.0);
            }
            AnnotationKind::Unsupported => {}
        }
    }
    if fixed_anno {
        repairs.push("Annotation geometry clamped back into range".to_string());
    }

    if !source_duration.is_finite() || source_duration <= 0.0 {
        return repairs;
    }
    if s.trim_end.is_finite() && s.trim_end > source_duration + VALIDATION_EPS {
        s.trim_end = source_duration;
        repairs.push("Trim end clamped to the video length".to_string());
    }
    if s.trim_start.is_finite() && s.trim_start > source_duration {
        s.trim_start = 0.0;
        repairs.push("Trim start reset into range".to_string());
    }
    let before = s.cuts.len();
    s.cuts.retain(|c| {
        c.start.is_finite() && c.end.is_finite() && c.start < source_duration + VALIDATION_EPS
    });
    let mut clamped_cut = false;
    for c in s.cuts.iter_mut() {
        if c.end > source_duration + VALIDATION_EPS {
            c.end = source_duration;
            clamped_cut = true;
        }
    }
    if before != s.cuts.len() || clamped_cut {
        repairs.push("Cuts past the video end were trimmed".to_string());
    }

    // Clamp each annotation into [trim_start, trim_end] with a forward window; repairs out-of-trim adds.
    let (ts, te) = (s.trim_start, s.trim_end);
    if te > ts + VALIDATION_EPS {
        let mut fixed = false;
        for a in s.annotations.iter_mut() {
            let mut start = a.start.clamp(ts, te);
            let mut end = a.end.clamp(ts, te);
            if end <= start + VALIDATION_EPS {
                end = (start + 2.0).min(te);
                if end <= start + VALIDATION_EPS {
                    start = (te - 2.0).max(ts);
                    end = te;
                }
            }
            if (start - a.start).abs() > VALIDATION_EPS || (end - a.end).abs() > VALIDATION_EPS {
                a.start = start;
                a.end = end;
                fixed = true;
            }
        }
        if fixed {
            repairs.push("Annotation timing repaired into the clip".to_string());
        }
    }

    repairs
}

/// Validate a `RenderState` against the source recording, at every entry point that crosses a trust boundary.
/// Returns ALL violations, not the first, so an agent fixes a document in one pass; pure, so it needs no `spawn_blocking`.
pub fn validate_render_state(
    s: &RenderState,
    source_duration: f64,
) -> Result<(), Vec<ValidationIssue>> {
    let mut issues = Vec::new();

    // Trim window
    if !s.trim_start.is_finite() || s.trim_start < 0.0 {
        issues.push(ValidationIssue {
            field: "trimStart".into(),
            reason: "non_negative".into(),
        });
    }
    if !s.trim_end.is_finite() {
        issues.push(ValidationIssue {
            field: "trimEnd".into(),
            reason: "finite".into(),
        });
    }
    // Only a strict `trim_end < trim_start` is an error: a fresh project holds 0.0 == 0.0 until content loads.
    if s.trim_end < s.trim_start - VALIDATION_EPS {
        issues.push(ValidationIssue {
            field: "trimEnd".into(),
            reason: "trim_end_before_start".into(),
        });
    }
    if s.trim_end > source_duration + VALIDATION_EPS && source_duration > 0.0 {
        issues.push(ValidationIssue {
            field: "trimEnd".into(),
            reason: "trim_end_exceeds_source".into(),
        });
    }

    // Cuts
    for (i, c) in s.cuts.iter().enumerate() {
        if !c.start.is_finite() || !c.end.is_finite() {
            issues.push(ValidationIssue {
                field: format!("cuts/{i}"),
                reason: "finite".into(),
            });
            continue;
        }
        if c.start < 0.0 {
            issues.push(ValidationIssue {
                field: format!("cuts/{i}/start"),
                reason: "non_negative".into(),
            });
        }
        if c.end <= c.start + VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("cuts/{i}/end"),
                reason: "cut_end_before_start".into(),
            });
        }
        // Slop so dragging a cut edge against the trim handle doesn't bounce the validator; export clamps anyway.
        if c.start < s.trim_start - VALIDATION_EPS || c.end > s.trim_end + VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("cuts/{i}"),
                reason: "cut_out_of_trim".into(),
            });
        }
    }
    let mut sorted: Vec<(usize, f64, f64)> = s
        .cuts
        .iter()
        .enumerate()
        .map(|(i, c)| (i, c.start, c.end))
        .collect();
    sorted.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));
    for w in sorted.windows(2) {
        let (_, _, a_e) = w[0];
        let (b_i, b_s, _) = w[1];
        if b_s < a_e - VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("cuts/{b_i}"),
                reason: "cut_overlap".into(),
            });
        }
    }

    // Zoom regions
    for (i, z) in s.zoom_regions.iter().enumerate() {
        if z.start < s.trim_start - VALIDATION_EPS || z.end > s.trim_end + VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("zoomRegions/{i}"),
                reason: "zoom_out_of_trim".into(),
            });
        }
        if z.end <= z.start + VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("zoomRegions/{i}"),
                reason: "zoom_end_before_start".into(),
            });
        }
        if !(ZOOM_SCALE_MIN..=ZOOM_SCALE_MAX).contains(&z.scale) {
            issues.push(ValidationIssue {
                field: format!("zoomRegions/{i}/scale"),
                reason: "scale_out_of_range".into(),
            });
        }
        if !(0.0..=1.0).contains(&z.center_x) || !(0.0..=1.0).contains(&z.center_y) {
            issues.push(ValidationIssue {
                field: format!("zoomRegions/{i}/center"),
                reason: "center_out_of_range".into(),
            });
        }
        if z.ramp_in < 0.0 || z.ramp_out < 0.0 {
            issues.push(ValidationIssue {
                field: format!("zoomRegions/{i}/ramp"),
                reason: "ramp_negative".into(),
            });
        }
    }

    // Annotations — envelope first, then per-kind geometry.
    for (i, a) in s.annotations.iter().enumerate() {
        if a.start < s.trim_start - VALIDATION_EPS || a.end > s.trim_end + VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("annotations/{i}"),
                reason: "annotation_out_of_trim".into(),
            });
        }
        if a.end <= a.start + VALIDATION_EPS {
            issues.push(ValidationIssue {
                field: format!("annotations/{i}"),
                reason: "annotation_end_before_start".into(),
            });
        }
        if !(0.0..=1.0).contains(&a.opacity) {
            issues.push(ValidationIssue {
                field: format!("annotations/{i}/opacity"),
                reason: "opacity_out_of_range".into(),
            });
        }
        match &a.kind {
            AnnotationKind::Rect {
                x,
                y,
                w: _,
                h: _,
                radius,
            } => {
                if !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(y) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/position"),
                        reason: "position_out_of_range".into(),
                    });
                }
                if *radius < 0.0 || *radius > 0.5 {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/radius"),
                        reason: "radius_out_of_range".into(),
                    });
                }
            }
            AnnotationKind::Ellipse { x, y, w: _, h: _ } => {
                if !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(y) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/position"),
                        reason: "position_out_of_range".into(),
                    });
                }
            }
            AnnotationKind::Blur {
                x,
                y,
                w: _,
                h: _,
                strength,
                radius,
                ..
            } => {
                if !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(y) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/position"),
                        reason: "position_out_of_range".into(),
                    });
                }
                if !(0.0..=1.0).contains(strength) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/strength"),
                        reason: "strength_out_of_range".into(),
                    });
                }
                if *radius < 0.0 || *radius > 0.5 {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/radius"),
                        reason: "radius_out_of_range".into(),
                    });
                }
            }
            AnnotationKind::Image {
                x,
                y,
                w: _,
                h: _,
                opacity,
                radius,
                ..
            } => {
                if !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(y) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/position"),
                        reason: "position_out_of_range".into(),
                    });
                }
                if !(0.0..=1.0).contains(opacity) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/opacity"),
                        reason: "opacity_out_of_range".into(),
                    });
                }
                if *radius < 0.0 || *radius > 0.5 {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/radius"),
                        reason: "radius_out_of_range".into(),
                    });
                }
            }
            AnnotationKind::Arrow { x1, y1, x2, y2, .. } => {
                if !(0.0..=1.0).contains(x1)
                    || !(0.0..=1.0).contains(y1)
                    || !(0.0..=1.0).contains(x2)
                    || !(0.0..=1.0).contains(y2)
                {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/points"),
                        reason: "points_out_of_range".into(),
                    });
                }
            }
            AnnotationKind::Text { x, y, .. } => {
                if !(0.0..=1.0).contains(x) || !(0.0..=1.0).contains(y) {
                    issues.push(ValidationIssue {
                        field: format!("annotations/{i}/position"),
                        reason: "position_out_of_range".into(),
                    });
                }
            }
            AnnotationKind::Unsupported => {
                // An Unsupported variant from a forward-compat change has no position to validate. No-op.
            }
        }
    }

    // Frame-level controls
    if !(0.0..=50.0).contains(&s.border_radius) {
        issues.push(ValidationIssue {
            field: "borderRadius".into(),
            reason: "border_radius_out_of_range".into(),
        });
    }
    if !(0.0..=20.0).contains(&s.padding) {
        issues.push(ValidationIssue {
            field: "padding".into(),
            reason: "padding_out_of_range".into(),
        });
    }
    if !s.cursor_size.is_finite() || s.cursor_size < 0.0 {
        issues.push(ValidationIssue {
            field: "cursorSize".into(),
            reason: "cursor_size_negative".into(),
        });
    }
    // Exposed as a 0..100 slider; the historical default is 50.0 and the export reads it as a percent.
    if !s.cursor_smoothing.is_finite() || !(0.0..=100.0).contains(&s.cursor_smoothing) {
        issues.push(ValidationIssue {
            field: "cursorSmoothing".into(),
            reason: "cursor_smoothing_out_of_range".into(),
        });
    }
    if !s.cursor_highlight_opacity.is_finite()
        || !(0.0..=100.0).contains(&s.cursor_highlight_opacity)
    {
        issues.push(ValidationIssue {
            field: "cursorHighlightOpacity".into(),
            reason: "cursor_highlight_opacity_out_of_range".into(),
        });
    }

    // Per-segment speed overrides
    for (i, sp) in s.segment_speeds.iter().enumerate() {
        if !sp.speed.is_finite() || sp.speed <= 0.0 {
            issues.push(ValidationIssue {
                field: format!("segmentSpeeds/{i}/speed"),
                reason: "speed_non_positive".into(),
            });
        }
    }

    if issues.is_empty() {
        Ok(())
    } else {
        Err(issues)
    }
}

#[cfg(test)]
mod project_timeline_tests {
    use super::*;
    use crate::render::graph::{CutRange, RenderState, SegmentSpeed};

    fn s(start: f64, end: f64) -> CutRange {
        CutRange {
            start,
            end,
            extra: serde_json::Map::new(),
        }
    }

    fn sp(start: f64, speed: f64) -> SegmentSpeed {
        SegmentSpeed { start, speed }
    }

    #[test]
    fn empty_state_matches_source_duration() {
        let st = RenderState::default();
        let tl = derive_project_timeline(&st, 30.0);
        assert_eq!(tl.source_duration, 30.0);
        assert_eq!(tl.trimmed_duration, 0.0);
        assert_eq!(tl.output_duration, 0.0);
        assert!(tl.cuts.is_empty());
        assert!(tl.kept_segments.is_empty());
    }

    #[test]
    fn cuts_shorten_output_duration() {
        let st = RenderState {
            trim_end: 20.0,
            cuts: vec![s(5.0, 7.0)],
            ..RenderState::default()
        };
        let tl = derive_project_timeline(&st, 30.0);
        assert_eq!(tl.trimmed_duration, 20.0);
        assert!((tl.output_duration - 18.0).abs() < 1e-6);
        assert_eq!(tl.cuts.len(), 1);
        assert_eq!(tl.cuts[0].start, 5.0);
        assert_eq!(tl.cuts[0].end, 7.0);
    }

    #[test]
    fn split_points_slice_kept_segments_at_1x() {
        let st = RenderState {
            trim_end: 20.0,
            split_points: vec![8.0],
            ..RenderState::default()
        };
        let tl = derive_project_timeline(&st, 30.0);
        assert_eq!(tl.kept_segments.len(), 2);
        assert_eq!(
            tl.kept_segments[0],
            KeptSegment {
                start: 0.0,
                end: 8.0,
                speed: 1.0
            }
        );
        assert_eq!(
            tl.kept_segments[1],
            KeptSegment {
                start: 8.0,
                end: 20.0,
                speed: 1.0
            }
        );
    }

    #[test]
    fn speed_warp_shortens_output_at_2x() {
        // Anchors are ORIGINAL seconds, so a 2x override anchored at t=0 covers the whole clip.
        let st = RenderState {
            trim_end: 10.0,
            segment_speeds: vec![sp(0.0, 2.0)],
            ..RenderState::default()
        };
        let tl = derive_project_timeline(&st, 30.0);
        assert!(
            (tl.output_duration - 5.0).abs() < 1e-6,
            "got {}",
            tl.output_duration
        );
        assert!((tl.kept_segments[0].speed - 2.0).abs() < 1e-6);
    }
}

#[cfg(test)]
mod audio_mix_tests {
    use super::*;
    use crate::render::node_types::AudioSettings;

    #[test]
    fn measured_track_offsets_align_each_source_before_it_is_trimmed() {
        let s = AudioSettings::default();
        let offsets = TrackOffsets {
            audio_ms: Some(240),
            microphone_ms: Some(-180),
            camera_ms: None,
        };
        let (complex, _) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            2.0,
            10.0,
            offsets,
        )
        .expect("audio graph");
        // A late loopback track is padded, an early mic track has its head cut.
        assert!(complex.contains("adelay=240:all=1"), "{complex}");
        assert!(complex.contains("atrim=start=0.180"), "{complex}");
        // Alignment must precede the edit trim: trim_start is video time, so an unaligned track cuts the wrong samples.
        let align = complex.find("adelay=240").expect("align");
        let trim = complex.find("atrim=start=2.000").expect("edit trim");
        assert!(align < trim, "alignment must come first: {complex}");
    }

    #[test]
    fn an_unmeasured_project_keeps_the_pre_offset_graph() {
        let s = AudioSettings::default();
        let (complex, _) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .expect("audio graph");
        assert!(!complex.contains("adelay"), "{complex}");
    }

    #[test]
    fn per_source_gain_reaches_the_graph() {
        let s = AudioSettings {
            system_volume: 100.0,
            mic_volume: 50.0,
            ..AudioSettings::default()
        };
        let (complex, map) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .expect("audio graph");
        assert_eq!(map, "[aout]");
        // System at unity, mic at half — the pre-fix bug applied master to both.
        assert!(complex.contains("[1:a]") && complex.contains("[2:a]"));
        assert!(complex.contains("volume=1.0000"));
        assert!(complex.contains("volume=0.5000"));
        assert!(complex.contains("amix=inputs=2"));
    }

    #[test]
    fn muted_source_is_dropped_from_the_mix() {
        let s = AudioSettings {
            mic_muted: true,
            ..AudioSettings::default()
        };
        let (complex, _map) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .expect("system still audible");
        // Only system survives → single branch, no amix, mic input absent.
        assert!(complex.contains("[1:a]"));
        assert!(!complex.contains("[2:a]"));
        assert!(!complex.contains("amix"));
    }

    #[test]
    fn all_sources_muted_yields_no_audio() {
        let s = AudioSettings {
            system_muted: true,
            mic_muted: true,
            ..AudioSettings::default()
        };
        assert!(append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .is_none());
    }

    #[test]
    fn normalize_appends_loudnorm_to_the_mix() {
        let s = AudioSettings {
            normalize_loudness: true,
            ..AudioSettings::default()
        };
        let (complex, map) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .expect("audio graph");
        assert!(complex.contains("loudnorm=I=-14"));
        assert_eq!(map, "[aoutn]"); // normalize retargets the map to the loudnorm output
                                    // Off by default: no loudnorm, map stays [aout].
        let off = AudioSettings::default();
        let (c2, m2) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System)],
            &off,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .expect("graph");
        assert!(!c2.contains("loudnorm"));
        assert_eq!(m2, "[aout]");
    }

    #[test]
    fn music_stage_mixes_clips_over_source() {
        use crate::render::node_types::{AudioClip, AudioClipSource};
        let clip = AudioClip {
            id: "m1".into(),
            source: AudioClipSource::Local {
                path: "x.mp3".into(),
            },
            role: Default::default(),
            start_output_sec: 2.0,
            offset_sec: 1.0,
            duration_sec: 0.0, // fill
            gain: 50.0,
            muted: false,
            fade_in: 0.5,
            fade_out: 1.0,
            looping: true,
            ducking: false,
        };
        let (seg, map) = build_music_stage(&[(5, &clip)], Some("[aout]"), 10.0).expect("stage");
        assert_eq!(map, "[afinal]");
        assert!(seg.contains("[5:a]"));
        assert!(seg.contains("volume=0.5000"));
        assert!(seg.contains("adelay=2000|2000"));
        assert!(seg.contains("atrim=start=1.000:duration=8.000")); // fill = 10 - 2
        assert!(seg.contains("amix=inputs=2"));
    }

    #[test]
    fn music_only_needs_no_amix() {
        use crate::render::node_types::{AudioClip, AudioClipSource};
        let clip = AudioClip {
            id: "m".into(),
            source: AudioClipSource::Local { path: "x".into() },
            role: Default::default(),
            start_output_sec: 0.0,
            offset_sec: 0.0,
            duration_sec: 0.0,
            gain: 100.0,
            muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
            looping: false,
            ducking: false,
        };
        let (_seg, map) = build_music_stage(&[(3, &clip)], None, 5.0).expect("stage");
        assert_eq!(map, "[mus0]");
    }

    #[test]
    fn audio_clip_deserializes_from_ts_shape() {
        // Guards the TS↔Rust serde names (loop, startOutputSec, providerId, assetPath).
        let local = r#"{"id":"c","source":{"kind":"local","path":"/a.mp3"},"startOutputSec":1.5,"gain":45,"loop":true}"#;
        let c: crate::render::node_types::AudioClip = serde_json::from_str(local).unwrap();
        assert_eq!(c.start_output_sec, 1.5);
        assert!(c.looping);
        assert_eq!(c.source.asset_path(), "/a.mp3");

        let provider = r#"{"id":"c","source":{"kind":"provider","providerId":"up","trackId":"t","assetPath":"/cached.mp3"},"gain":45}"#;
        let p: crate::render::node_types::AudioClip = serde_json::from_str(provider).unwrap();
        assert_eq!(p.source.asset_path(), "/cached.mp3");

        // role: absent → Music (legacy), explicit "voice" round-trips.
        use crate::render::node_types::AudioClipRole;
        assert_eq!(c.role, AudioClipRole::Music);
        let voice = r#"{"id":"v","source":{"kind":"local","path":"/rec.wav"},"role":"voice"}"#;
        let v: crate::render::node_types::AudioClip = serde_json::from_str(voice).unwrap();
        assert_eq!(v.role, AudioClipRole::Voice);
    }

    #[test]
    fn source_kind_uses_master_only() {
        // A per-source gain must not touch an embedded source track.
        let s = AudioSettings {
            volume: 50.0,
            mic_volume: 0.0,
            ..AudioSettings::default()
        };
        let (complex, _) = append_audio_to_complex(
            None,
            &[(0, AudioKind::Source)],
            &s,
            0.0,
            10.0,
            TrackOffsets::default(),
        )
        .expect("source audio");
        assert!(complex.contains("volume=0.5000"));
    }

    #[test]
    fn credits_comment_dedupes_providers_and_skips_local() {
        use crate::render::node_types::{AudioClip, AudioClipSource};
        let provider = |id: &str, attr: Option<&str>| AudioClip {
            id: id.into(),
            source: AudioClipSource::Provider {
                provider_id: "jamendo".into(),
                track_id: id.into(),
                asset_path: format!("/{id}.mp3"),
                attribution: attr.map(str::to_string),
                license: None,
            },
            role: Default::default(),
            start_output_sec: 0.0,
            offset_sec: 0.0,
            duration_sec: 0.0,
            gain: 100.0,
            muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
            looping: false,
            ducking: false,
        };
        let local = AudioClip {
            source: AudioClipSource::Local {
                path: "/x.mp3".into(),
            },
            ..provider("l", None)
        };
        let line = "\"Sunrise\" by Nova (Jamendo)";
        let comment = crate::commands::export::tail::build_credits_comment(&[
            provider("1", Some(line)),
            provider("2", Some(line)), // same line → deduped
            local,                     // local → no credit
        ])
        .expect("comment");
        assert_eq!(comment, format!("Music: {line}"));
        assert!(
            crate::commands::export::tail::build_credits_comment(&[provider("3", None)]).is_none()
        );
    }
}

#[cfg(test)]
mod validate_tests {
    use super::*;
    use crate::render::graph::{CutRange, SegmentSpeed};
    use crate::render::node_types::{Annotation, ZoomRegion};

    fn cut(start: f64, end: f64) -> CutRange {
        CutRange {
            start,
            end,
            extra: serde_json::Map::new(),
        }
    }

    fn sp(start: f64, speed: f64) -> SegmentSpeed {
        SegmentSpeed { start, speed }
    }

    #[test]
    fn default_state_is_valid() {
        let st = RenderState::default();
        eprintln!(
            "default: trim_start={} trim_end={} cursor_smoothing={} cursor_size={} padding={} border_radius={}",
            st.trim_start, st.trim_end, st.cursor_smoothing, st.cursor_size, st.padding, st.border_radius,
        );
        let err = validate_render_state(&st, 30.0);
        if let Err(issues) = &err {
            eprintln!("default state issues: {issues:?}");
        }
        assert!(err.is_ok(), "default state should be valid: {err:?}");
    }

    #[test]
    fn repair_clamps_trim_end_past_source_so_validation_passes() {
        // Pre-fix regression: a wall-clock trim_end past the CFR video (27.102 vs 26.625) is rejected by the validator.
        let mut st = RenderState {
            trim_start: 0.0,
            trim_end: 27.102,
            ..RenderState::default()
        };
        assert_eq!(
            reason(&validate_render_state(&st, 26.625).unwrap_err(), "trimEnd"),
            Some("trim_end_exceeds_source"),
        );
        // Repair clamps to the real duration and reports the change; the same state now validates.
        let repairs = repair_render_state(&mut st, 26.625);
        assert_eq!(st.trim_end, 26.625);
        assert!(!repairs.is_empty());
        assert!(validate_render_state(&st, 26.625).is_ok());
    }

    #[test]
    fn repair_is_a_noop_within_range() {
        let mut st = RenderState {
            trim_start: 0.0,
            trim_end: 20.0,
            ..RenderState::default()
        };
        assert!(repair_render_state(&mut st, 30.0).is_empty());
        assert_eq!(st.trim_end, 20.0);
    }

    #[test]
    fn repair_drops_cuts_past_source() {
        let mut st = RenderState {
            trim_start: 0.0,
            trim_end: 30.0,
            cuts: vec![cut(5.0, 8.0), cut(40.0, 45.0)],
            ..RenderState::default()
        };
        let repairs = repair_render_state(&mut st, 30.0);
        assert_eq!(st.cuts.len(), 1); // the 40..45 cut is entirely past the video
        assert!(repairs.iter().any(|r| r.contains("Cuts")));
    }

    fn anno(start: f64, end: f64) -> Annotation {
        serde_json::from_value(serde_json::json!({
            "id": "a", "start": start, "end": end,
            "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "radius": 0.0 },
        }))
        .unwrap()
    }

    #[test]
    fn repair_fixes_annotation_end_before_start_so_validation_passes() {
        // Reproduces the report: annotations added at or past the trim end got end <= start and failed validation.
        let mut st = RenderState {
            trim_start: 0.0,
            trim_end: 10.0,
            annotations: vec![anno(2.0, 5.0), anno(9.5, 9.5), anno(8.0, 6.0)],
            ..RenderState::default()
        };
        assert_eq!(
            reason(
                &validate_render_state(&st, 10.0).unwrap_err(),
                "annotations/1"
            ),
            Some("annotation_end_before_start"),
        );
        let repairs = repair_render_state(&mut st, 10.0);
        assert!(repairs.iter().any(|r| r.contains("Annotation")));
        // The already-valid annotation is untouched.
        assert_eq!((st.annotations[0].start, st.annotations[0].end), (2.0, 5.0));
        // Every annotation now has a forward window inside the clip.
        for a in &st.annotations {
            assert!(a.end > a.start, "end {} > start {}", a.end, a.start);
            assert!(a.start >= 0.0 && a.end <= 10.0 + VALIDATION_EPS);
        }
        assert!(validate_render_state(&st, 10.0).is_ok());
    }

    fn reason<'a>(issues: &'a [ValidationIssue], field: &str) -> Option<&'a str> {
        issues
            .iter()
            .find(|i| i.field == field)
            .map(|i| i.reason.as_str())
    }

    #[test]
    fn trim_invariants() {
        // trim_end < trim_start
        let st = RenderState {
            trim_start: 10.0,
            trim_end: 5.0,
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(reason(&err, "trimEnd"), Some("trim_end_before_start"));

        // trim_end > source_duration
        let st = RenderState {
            trim_start: 0.0,
            trim_end: 100.0,
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(reason(&err, "trimEnd"), Some("trim_end_exceeds_source"));

        // negative trim_start
        let st = RenderState {
            trim_start: -1.0,
            trim_end: 10.0,
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(reason(&err, "trimStart"), Some("non_negative"));
    }

    #[test]
    fn cut_invalid_when_end_before_start_or_outside_trim() {
        let mut st = RenderState {
            trim_start: 0.0,
            trim_end: 20.0,
            ..RenderState::default()
        };
        st.cuts.push(cut(8.0, 5.0));
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(reason(&err, "cuts/0/end"), Some("cut_end_before_start"));

        st.cuts.clear();
        st.cuts.push(cut(25.0, 28.0));
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(reason(&err, "cuts/0"), Some("cut_out_of_trim"));
    }

    #[test]
    fn overlapping_cuts_are_rejected() {
        let st = RenderState {
            trim_start: 0.0,
            trim_end: 20.0,
            cuts: vec![cut(5.0, 10.0), cut(8.0, 12.0)],
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(reason(&err, "cuts/1"), Some("cut_overlap"));
    }

    #[test]
    fn zoom_scale_must_be_in_range() {
        let st = RenderState {
            trim_end: 10.0,
            zoom_regions: vec![ZoomRegion {
                start: 0.0,
                end: 5.0,
                scale: 5.0,
                ease_in: Default::default(),
                ease_out: Default::default(),
                ramp_in: 0.0,
                ramp_out: 0.0,
                center_x: 0.5,
                center_y: 0.5,
                hidden: false,
                motion_blur: 0.0,
                extra: serde_json::Map::new(),
            }],
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(
            reason(&err, "zoomRegions/0/scale"),
            Some("scale_out_of_range")
        );
    }

    fn zoom(scale: f64, center_x: f64, center_y: f64, ramp_in: f64) -> ZoomRegion {
        ZoomRegion {
            start: 0.0,
            end: 5.0,
            scale,
            ease_in: Default::default(),
            ease_out: Default::default(),
            ramp_in,
            ramp_out: 0.0,
            center_x,
            center_y,
            hidden: false,
            motion_blur: 0.0,
            extra: serde_json::Map::new(),
        }
    }

    fn state_with_zoom(region: ZoomRegion) -> RenderState {
        RenderState {
            trim_end: 10.0,
            zoom_regions: vec![region],
            ..RenderState::default()
        }
    }

    /// The reported failure: `enqueue_export: render state invalid (1 issue):
    /// zoomRegions/0/scale scale_out_of_range`, with no repair to recover it.
    #[test]
    fn an_out_of_range_zoom_scale_is_repaired_rather_than_failing_the_export() {
        let mut st = state_with_zoom(zoom(5.0, 0.5, 0.5, 0.0));
        let repairs = repair_render_state(&mut st, 30.0);

        assert_eq!(st.zoom_regions[0].scale, 3.0);
        assert!(!repairs.is_empty(), "the repair went unreported");
        assert!(
            validate_render_state(&st, 30.0).is_ok(),
            "still invalid after repair"
        );
    }

    /// Every NaN comparison is false, so `!(1.0..=3.0).contains(&NaN)` is true
    /// and a NaN scale reports as out of range. Clamping cannot fix it.
    #[test]
    fn a_non_finite_zoom_scale_becomes_no_zoom() {
        let mut st = state_with_zoom(zoom(f64::NAN, 0.5, 0.5, 0.0));
        repair_render_state(&mut st, 30.0);
        assert_eq!(st.zoom_regions[0].scale, 1.0);
        assert!(validate_render_state(&st, 30.0).is_ok());
    }

    #[test]
    fn a_zoom_scale_below_one_is_lifted_rather_than_left_to_crash_the_crop() {
        let mut st = state_with_zoom(zoom(0.4, 0.5, 0.5, 0.0));
        repair_render_state(&mut st, 30.0);
        assert_eq!(st.zoom_regions[0].scale, 1.0);
    }

    #[test]
    fn an_out_of_range_zoom_centre_is_clamped_and_a_non_finite_one_recentred() {
        let mut st = state_with_zoom(zoom(2.0, 4.0, f64::INFINITY, 0.0));
        repair_render_state(&mut st, 30.0);
        assert_eq!(st.zoom_regions[0].center_x, 1.0);
        assert_eq!(st.zoom_regions[0].center_y, 0.5);
        assert!(validate_render_state(&st, 30.0).is_ok());
    }

    #[test]
    fn a_negative_zoom_ramp_is_zeroed() {
        let mut st = state_with_zoom(zoom(2.0, 0.5, 0.5, -1.0));
        repair_render_state(&mut st, 30.0);
        assert_eq!(st.zoom_regions[0].ramp_in, 0.0);
        assert!(validate_render_state(&st, 30.0).is_ok());
    }

    /// A state already in range must come back untouched and unreported, or
    /// every export would claim it repaired something.
    #[test]
    fn a_zoom_already_in_range_is_left_alone() {
        let mut st = state_with_zoom(zoom(2.0, 0.3, 0.7, 0.25));
        let repairs = repair_render_state(&mut st, 30.0);
        assert_eq!(st.zoom_regions[0].scale, 2.0);
        assert_eq!(st.zoom_regions[0].center_x, 0.3);
        assert_eq!(st.zoom_regions[0].ramp_in, 0.25);
        assert!(
            !repairs.iter().any(|r| r.contains("Zoom")),
            "reported a repair it did not make: {repairs:?}"
        );
    }

    /// The clamp does not depend on the source duration, so it must still run
    /// when the probe gave us nothing.
    #[test]
    fn zoom_is_repaired_even_when_the_source_duration_is_unknown() {
        let mut st = state_with_zoom(zoom(9.0, 0.5, 0.5, 0.0));
        repair_render_state(&mut st, 0.0);
        assert_eq!(st.zoom_regions[0].scale, 3.0);
    }

    /// EVERY value rule violated at once must come back exportable. One state,
    /// not a test per field, so a new rule with no repair fails here.
    #[test]
    fn a_state_violating_every_value_rule_is_repaired_into_something_exportable() {
        let mut st = RenderState {
            trim_end: 10.0,
            border_radius: 900.0,
            padding: -5.0,
            cursor_size: f64::NAN,
            cursor_smoothing: 4000.0,
            cursor_highlight_opacity: -20.0,
            zoom_regions: vec![zoom(f64::INFINITY, -3.0, 9.0, -1.0)],
            annotations: vec![serde_json::from_value(serde_json::json!({
                "id": "a1",
                "kind": { "kind": "blur", "x": -2.0, "y": 5.0, "w": 0.3, "h": 0.2,
                          "strength": 40.0, "radius": 9.0 },
                "start": 0.0, "end": 4.0,
                "opacity": 12.0
            }))
            .expect("annotation fixture")],
            ..RenderState::default()
        };

        let repairs = repair_render_state(&mut st, 30.0);
        assert!(!repairs.is_empty(), "nothing was reported as repaired");
        match validate_render_state(&st, 30.0) {
            Ok(()) => {}
            Err(issues) => panic!("still invalid after repair: {issues:?}"),
        }
    }

    /// The counterpart, and the one that stops the sweep being a blunt reset: a
    /// state already inside every band comes back byte-identical and silent.
    #[test]
    fn a_valid_state_is_neither_changed_nor_reported() {
        let mut st = RenderState {
            trim_end: 10.0,
            border_radius: 12.0,
            padding: 6.0,
            cursor_size: 3.0,
            cursor_smoothing: 50.0,
            cursor_highlight_opacity: 40.0,
            zoom_regions: vec![zoom(2.0, 0.3, 0.7, 0.25)],
            ..RenderState::default()
        };
        let before = st.clone();

        let repairs = repair_render_state(&mut st, 30.0);

        assert!(
            repairs.is_empty(),
            "claimed repairs it did not make: {repairs:?}"
        );
        assert_eq!(st.border_radius, before.border_radius);
        assert_eq!(st.padding, before.padding);
        assert_eq!(st.cursor_smoothing, before.cursor_smoothing);
        assert_eq!(st.zoom_regions[0].scale, before.zoom_regions[0].scale);
        assert_eq!(st.zoom_regions[0].ramp_in, before.zoom_regions[0].ramp_in);
    }

    /// A speed of zero is a division by zero in the time map, not a slow clip.
    #[test]
    fn a_non_positive_segment_speed_is_reset_to_normal() {
        let mut st = RenderState {
            trim_end: 10.0,
            segment_speeds: vec![
                serde_json::from_value(serde_json::json!({
                    "start": 0.0, "end": 2.0, "speed": 0.0
                }))
                .expect("segment fixture"),
                serde_json::from_value(serde_json::json!({
                    "start": 2.0, "end": 4.0, "speed": -3.0
                }))
                .expect("segment fixture"),
            ],
            ..RenderState::default()
        };
        repair_render_state(&mut st, 30.0);
        assert!(st.segment_speeds.iter().all(|s| s.speed == 1.0));
        assert!(validate_render_state(&st, 30.0).is_ok());
    }

    #[test]
    fn border_radius_out_of_range() {
        let st = RenderState {
            trim_end: 10.0,
            border_radius: 60.0,
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(
            reason(&err, "borderRadius"),
            Some("border_radius_out_of_range")
        );
    }

    #[test]
    fn segment_speed_must_be_positive() {
        let st = RenderState {
            trim_end: 10.0,
            segment_speeds: vec![sp(0.0, -1.0)],
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert_eq!(
            reason(&err, "segmentSpeeds/0/speed"),
            Some("speed_non_positive")
        );
    }

    #[test]
    fn validator_collects_all_issues() {
        let st = RenderState {
            trim_start: -1.0,
            trim_end: 100.0,
            border_radius: 100.0,
            ..RenderState::default()
        };
        let err = validate_render_state(&st, 30.0).unwrap_err();
        assert!(err.len() >= 3, "got only {} issues: {err:?}", err.len());
    }
}

#[tauri::command]
pub async fn generate_thumbnails(path: String, count: u32) -> AppResult<Vec<String>> {
    // Sync ffmpeg on Tauri's main thread froze the UI, and /recasts fires this once per recording in parallel.
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

    // Cached per (media identity, count) so reopens skip the decode; count keeps the poster and the strip apart.
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

    // Poster stays a single `-ss` seek + `-vframes 1`: one decode at the timestamp, no full read.
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

    // One FFmpeg invocation via `fps=count/duration`; `count` separate spawns cost ~200 ms of codec init each.
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temp_dir = std::env::temp_dir()
        .join("recast-thumbnails")
        .join(format!("{}-{stamp}", std::process::id()));
    let _ = fs::create_dir_all(&temp_dir);

    // `vsync vfr` stops FFmpeg duplicating or dropping frames to hit a constant rate; we want the filter's samples.
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
        // image2 numbers from 1 and can land one frame either side of `count`, so read what is there and trim.
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

    // Recursive: `remove_dir` leaks the whole dir when image2 emits an extra frame, gigabytes over a long session.
    let _ = fs::remove_dir_all(&temp_dir);

    // Only cache a complete strip; a partial or failed run should not be pinned.
    if !thumbnails.is_empty() {
        crate::cache::put("thumbs", &[media_path.as_path()], count as u64, &thumbnails);
    }

    Ok(thumbnails)
}

/// Pull a single thumbnail at `timestamp` (seconds). Used for poster frames where the timeline-strip's multi-frame batching would be overkill.
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

/// Drops an export's cancellation token when the run ends, however it ends.
struct CancelTokenGuard {
    app: AppHandle,
    export_id: String,
}

impl Drop for CancelTokenGuard {
    fn drop(&mut self) {
        if let Some(state) = self.app.try_state::<AppState>() {
            state.export_cancel.lock().remove(&self.export_id);
        }
    }
}

/// Muxes a browser-rendered video, already composited and warped, with the export's audio; the video is copied and only the audio graph is rebuilt here.
/// Reuses `run_export_job`'s queue, cancel and progress lifecycle and the index-parametric audio helpers, so the browser video can sit at input 0.
pub(crate) async fn run_mux_job(
    app: AppHandle,
    request: ExportRequest,
    browser_video_path: String,
) -> AppResult<String> {
    let state = app.state::<AppState>();
    let export_id = request.export_id.clone();
    let _power = state.power.lease();
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .export_cancel
        .lock()
        .insert(export_id.clone(), cancel_flag.clone());
    let _cancel_token = CancelTokenGuard {
        app: app.clone(),
        export_id: export_id.clone(),
    };
    emit_export_state(&app, ExportStateEvent::started(&export_id));
    emit_export_state(
        &app,
        ExportStateEvent::preparing(&export_id, "Muxing export"),
    );

    let browser_video = PathBuf::from(&browser_video_path);
    if !browser_video.exists() {
        return Err("export failed: browser-rendered video not found".into());
    }
    let input_path = PathBuf::from(&request.input_path);

    // GIF has no audio to mux and the browser video is already warped, so only the 2-pass palette runs.
    if request.format == "gif" {
        return mux_browser_gif(
            &app,
            &request,
            &browser_video,
            &input_path,
            &export_id,
            cancel_flag,
        )
        .await;
    }

    let project = open_project_if_needed(&input_path)?;
    // Per-track capture lag; missing on pre-offset bundles, which align at 0 as they always did.
    let track_offsets = project
        .as_ref()
        .and_then(|p| p.metadata.media.as_ref())
        .map(|m| m.track_offsets)
        .unwrap_or_default();
    let source_video = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or_else(|| input_path.clone());

    let graph = RenderGraph::from_state(&request.render_state);
    let (trim_start, trim_end) = graph.trim_range();
    let duration = (trim_end - trim_start).max(0.0);

    // Input 0 = the browser video; audio inputs follow, indexed for append_audio_to_complex.
    let mut args: Vec<String> = vec![
        "-y".to_string(),
        "-i".to_string(),
        browser_video.to_string_lossy().to_string(),
    ];
    let audio_detached = request
        .render_state
        .music_clips
        .iter()
        .any(|c| c.role == crate::render::node_types::AudioClipRole::Voice);
    let mut audio_input_indices: Vec<(usize, AudioKind)> = Vec::new();
    let mut next_input = 1usize;
    if request.format != "gif" && has_audio(&source_video) && !audio_detached {
        args.extend(["-i".to_string(), source_video.to_string_lossy().to_string()]);
        audio_input_indices.push((next_input, AudioKind::Source));
        next_input += 1;
    }
    if request.format != "gif" {
        if let Some(project) = project.as_ref().filter(|_| !audio_detached) {
            for (path, kind) in [
                (&project.audio_path, AudioKind::System),
                (&project.microphone_path, AudioKind::Mic),
            ] {
                // `exists()` is not enough: a header-only WAV with zero samples aborts the whole graph once amix meets concat.
                let Some(path) = path
                    .as_ref()
                    .filter(|p| crate::audio::wav::wav_has_samples(p))
                else {
                    if let Some(p) = path.as_ref().filter(|p| p.exists()) {
                        log::info!("export: skipping empty audio track {}", p.display());
                    }
                    continue;
                };
                args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
                audio_input_indices.push((next_input, kind));
                next_input += 1;
            }
        }
    }
    let mut music_inputs: Vec<(usize, &crate::render::node_types::AudioClip)> = Vec::new();
    if request.format != "gif" {
        for clip in &request.render_state.music_clips {
            if clip.muted || clip.gain <= 0.0 {
                continue;
            }
            let path = clip.source.asset_path();
            if path.is_empty() || !Path::new(path).exists() {
                continue;
            }
            if clip.looping {
                args.extend(["-stream_loop".to_string(), "-1".to_string()]);
            }
            args.extend(["-i".to_string(), path.to_string()]);
            music_inputs.push((next_input, clip));
            next_input += 1;
        }
    }

    // The browser video is ALREADY warped, so only audio takes the cuts/speed warp before the music mix.
    let mut filter_complex: Option<String> = None;
    let mut audio_map = append_audio_to_complex(
        None,
        &audio_input_indices,
        &request.render_state.audio_settings,
        trim_start,
        duration,
        track_offsets,
    )
    .map(|(complex, map)| {
        filter_complex = Some(complex);
        map
    });
    let export_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
    let speed_segments = resolve_speed_segments(
        request.time_map.as_ref(),
        duration,
        &export_cuts,
        &request.render_state.split_points,
        &request.render_state.segment_speeds,
        trim_start,
    );
    let speed_active = has_speed_change(&speed_segments);
    crate::commands::export::cuts_speed::append_audio_cut_speed(
        &mut filter_complex,
        &mut audio_map,
        &export_cuts,
        &speed_segments,
        speed_active,
    );
    let out_dur = warped_output_duration(&speed_segments);
    if !music_inputs.is_empty() {
        if let Some((seg, map)) = build_music_stage(&music_inputs, audio_map.as_deref(), out_dur) {
            if !seg.is_empty() {
                filter_complex = Some(match filter_complex.take() {
                    Some(fc) => format!("{fc};{seg}"),
                    None => seg,
                });
            }
            audio_map = Some(map);
        }
    }

    let output_dir = get_active_output_dir(&state).join("exports");
    let _ = std::fs::create_dir_all(&output_dir);
    let source_stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Recast_export".to_string());
    let output_path = super::unique_path(&output_dir, &source_stem, "mp4");

    // Windows caps the command line at ~32 KB, and many music or voice clips build a long adelay/amix graph.
    let mut mux_script_path: Option<PathBuf> = None;
    if let Some(ref fc) = filter_complex {
        if fc.len() > FILTER_COMPLEX_SCRIPT_THRESHOLD {
            let path = std::env::temp_dir().join(format!("recast-mux-filtergraph-{export_id}.txt"));
            std::fs::write(&path, fc).map_err(|e| {
                AppError::msg(format!(
                    "failed to write mux filter script {}: {e}",
                    path.display()
                ))
            })?;
            args.extend([
                "-filter_complex_script".to_string(),
                path.to_string_lossy().to_string(),
            ]);
            mux_script_path = Some(path);
        } else {
            args.extend(["-filter_complex".to_string(), fc.clone()]);
        }
    }
    args.extend([
        "-map".to_string(),
        "0:v:0".to_string(),
        "-c:v".to_string(),
        "copy".to_string(),
    ]);
    if let Some(ref amap) = audio_map {
        args.extend([
            "-map".to_string(),
            amap.clone(),
            "-c:a".to_string(),
            "aac".to_string(),
            "-b:a".to_string(),
            "192k".to_string(),
            // Pin the delivered format; see the note in export::codec.
            "-ar".to_string(),
            "48000".to_string(),
            "-ac".to_string(),
            "2".to_string(),
        ]);
    }
    // Without `-shortest` a cut or sped-up edit leaves an audio tail, and a looping music clip never ends.
    if audio_map.is_some() {
        args.push("-shortest".to_string());
    }
    args.extend([
        "-movflags".to_string(),
        "+faststart".to_string(),
        output_path.to_string_lossy().to_string(),
    ]);

    let expected_output_secs = if out_dur > 0.0 { out_dur } else { duration };
    let output_path_str = output_path.to_string_lossy().to_string();
    let progress_band = ProgressBand {
        offset: 0.0,
        scale: 1.0,
    };
    let app_for_task = app.clone();
    let export_id_for_task = export_id.clone();
    let task_result = tokio::task::spawn_blocking(move || {
        run_encode(
            args,
            app_for_task,
            export_id_for_task,
            cancel_flag,
            output_path_str,
            expected_output_secs,
            progress_band,
        )
    })
    .await;

    if let Some(p) = mux_script_path.as_ref() {
        let _ = std::fs::remove_file(p);
    }

    match task_result {
        Ok(Ok(path)) => {
            // The browser video was a pre-encode temp; kept on failure so a retry re-muxes without re-rendering.
            let _ = std::fs::remove_file(&browser_video);
            Ok(path)
        }
        Ok(Err(e)) => Err(AppError::msg(e)),
        Err(join) => Err(AppError::msg(format!("mux task panicked: {join}"))),
    }
}

/// GIF muxing for a browser-rendered export: the browser already composited and
/// warped every frame, so we just run the existing 2-pass palette (pass 1 →
/// palette PNG, pass 2 → paletteuse) on that video at the GIF's fps + scale. No
/// cuts/speed (already baked), no audio. Reuses the classic GIF building blocks.
async fn mux_browser_gif(
    app: &AppHandle,
    request: &ExportRequest,
    browser_video: &Path,
    input_path: &Path,
    export_id: &str,
    cancel_flag: Arc<AtomicBool>,
) -> AppResult<String> {
    let state = app.state::<AppState>();
    let profile = resolve_export_profile(&request.quality);
    let gif_fps_default = profile.gif_fps;
    let output_scale_filter = build_output_scale_filter(profile);
    let gif_settings: GifSettings = request.gif_settings.clone().unwrap_or_default();
    let source_duration = probe_video_metadata(browser_video)?.duration.max(0.0);

    let output_dir = get_active_output_dir(&state).join("exports");
    let _ = std::fs::create_dir_all(&output_dir);
    let source_stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Recast_export".to_string());
    let output_path = super::unique_path(&output_dir, &source_stem, "gif");

    let resolved_fps = gif_settings.fps.unwrap_or(gif_fps_default);
    let max_colors = gif_settings.max_colors();
    let dither = gif_settings.dither.clone();
    let palette_stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let palette_path = output_dir.join(format!(
        "recast_palette_{palette_stamp}_{}.png",
        std::process::id()
    ));

    // Pass 1: palette (own thread; whole file, so trim=0/duration=0).
    let app_p = app.clone();
    let eid_p = export_id.to_string();
    let bv_p = browser_video.to_path_buf();
    let pal_p = palette_path.clone();
    let cancel_p = cancel_flag.clone();
    let scale_p = output_scale_filter.clone();
    let dither_p = dither.clone();
    let prepass = tokio::task::spawn_blocking(move || {
        run_gif_palette_prepass(
            &app_p,
            &eid_p,
            &bv_p,
            &pal_p,
            0.0,
            0.0,
            source_duration,
            GifFilterOptions {
                fps: resolved_fps,
                max_colors,
                dither: dither_p.as_str(),
            },
            scale_p.as_deref(),
            None,
            cancel_p,
            ProgressBand {
                offset: 0.0,
                scale: 0.4,
            },
        )
    })
    .await;
    match prepass {
        Ok(Ok(())) => {}
        Ok(Err(msg)) => {
            let _ = std::fs::remove_file(&palette_path);
            if cancel_flag.load(Ordering::Acquire) {
                emit_export_state(app, ExportStateEvent::cancelled(export_id));
                return Err(AppError::from("export cancelled"));
            }
            emit_export_state(app, ExportStateEvent::error(export_id, &msg));
            return Err(AppError::msg(msg));
        }
        Err(join) => {
            let _ = std::fs::remove_file(&palette_path);
            return Err(AppError::msg(format!("gif palette task panicked: {join}")));
        }
    }

    // Pass 2: paletteuse on the browser video (input 0) + palette (input 1).
    let (complex, video_map) = build_gif_paletteuse_external_complex(
        None,
        "[0:v]",
        1,
        GifFilterOptions {
            fps: resolved_fps,
            max_colors,
            dither: dither.as_str(),
        },
        output_scale_filter.as_deref(),
    );
    let args: Vec<String> = vec![
        "-y".into(),
        "-i".into(),
        browser_video.to_string_lossy().to_string(),
        "-i".into(),
        palette_path.to_string_lossy().to_string(),
        "-filter_complex".into(),
        complex,
        "-map".into(),
        video_map,
        "-an".into(),
        output_path.to_string_lossy().to_string(),
    ];

    let app_e = app.clone();
    let eid_e = export_id.to_string();
    let out_str = output_path.to_string_lossy().to_string();
    let cancel_e = cancel_flag.clone();
    let encode = tokio::task::spawn_blocking(move || {
        run_encode(
            args,
            app_e,
            eid_e,
            cancel_e,
            out_str,
            source_duration,
            ProgressBand {
                offset: 40.0,
                scale: 0.6,
            },
        )
    })
    .await;

    let _ = std::fs::remove_file(&palette_path);
    match encode {
        Ok(Ok(path)) => {
            let _ = std::fs::remove_file(browser_video);
            Ok(path)
        }
        Ok(Err(e)) => Err(AppError::msg(e)),
        Err(join) => Err(AppError::msg(format!("gif encode task panicked: {join}"))),
    }
}

/// Runs one export end to end and emits `export-state` keyed by `request.export_id`; an `Err` containing "cancel" means the user aborted.
/// Not a Tauri command: exports are enqueued, and the serial worker is the only caller. Owns its cancel token so `cancel_export` finds it by id.
pub(crate) async fn run_export_job(
    app: AppHandle,
    mut request: ExportRequest,
) -> AppResult<String> {
    let state = app.state::<AppState>();
    let export_id = request.export_id.clone();

    // RAII: the display/system wake lease is released on every return path, including `?` and cancel.
    let _power = state.power.lease();

    // Scoped to the export session id the frontend also uses to filter state events.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .export_cancel
        .lock()
        .insert(export_id.clone(), cancel_flag.clone());
    // RAII removal on EVERY exit: hand removal stranded a token when prep failed, poisoning the next run on that id.
    let _cancel_token = CancelTokenGuard {
        app: app.clone(),
        export_id: export_id.clone(),
    };
    emit_export_state(&app, ExportStateEvent::started(&export_id));
    emit_export_state(
        &app,
        ExportStateEvent::preparing(&export_id, "Preparing export"),
    );

    // Per-stage wall clock (prep / cursor pre-render / encode), logged at info and correlated by `export_id`.
    let export_start = Instant::now();

    let input_path = PathBuf::from(&request.input_path);
    let project = open_project_if_needed(&input_path)?;
    // Per-track capture lag; missing on pre-offset bundles, which align at 0 as they always did.
    let track_offsets = project
        .as_ref()
        .and_then(|p| p.metadata.media.as_ref())
        .map(|m| m.track_offsets)
        .unwrap_or_default();
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
    // Progress-denominator fallback for a render state with no Trim node (duration == 0).
    let source_duration = metadata.duration.max(0.0);
    let profile = resolve_export_profile(&request.quality);
    // Defaults to the source rate and only ever downsamples; also pins the background, looped-image and cursor rates.
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
    // Encoder effort, orthogonal to the resolution profile; Balanced when absent or unknown.
    let speed = ExportSpeed::from_request(request.speed.as_deref().unwrap_or("balanced"));
    let output_scale_filter = build_output_scale_filter(profile);
    let output_dir = get_active_output_dir(&state).join("exports");
    let _ = std::fs::create_dir_all(&output_dir);
    let extension = match request.format.as_str() {
        "gif" => "gif",
        "webm" => "webm",
        _ => "mp4",
    };
    // Named after the source recording, with an Explorer-style counter suffix on repeat exports.
    let source_stem = input_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Recast_export".to_string());
    let output_path = super::unique_path(&output_dir, &source_stem, extension);

    // Correlated with the frontend's `export_started` by `export_id`; info level, so dev and diagnostic mode keep it.
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

    /// Where the engine render stops when a mux pass still has to run, leaving the
    /// tail of the bar to the encode that follows.
    const ENGINE_RENDER_CEILING: f64 = 70.0;

    /// The caption track and face for an engine export, or `None` when nothing is
    /// to be burned. A font that will not download degrades to the system match
    /// rather than failing an otherwise good export.
    async fn engine_caption_burn_in(
        app: &AppHandle,
        request: &ExportRequest,
    ) -> Option<crate::export_engine::CaptionBurnIn> {
        let track =
            crate::export_engine::burn_in_for(&request.render_state, request.burn_captions)?;
        let style = request.render_state.caption_style.as_ref()?;
        let family = crate::transcription::subtitles::first_family(&style.font_family);
        let font = match crate::transcription::subtitles::is_system_family(&family) {
            true => None,
            false => match crate::fonts::ensure_caption_font_file(app, &family, style.font_weight)
                .await
            {
                Ok(path) => Some(path),
                Err(e) => {
                    log::warn!("engine export: caption font ({family}): {e}");
                    None
                }
            },
        };
        Some(crate::export_engine::CaptionBurnIn { track, font })
    }

    // The engine path, opt in. Everything above still runs (it validates the request and names the output the same way), so the two paths differ only in who renders. MP4 only, no progress/cancel yet, which is why it is not a setting.
    if crate::export_engine::enabled(request.engine_export) {
        // Direct only for an mp4 the engine can finish itself: GIF needs a palette pass, WebM a VP9 encoder, and a platform without an in-process codec needs an audio track, all of which live behind the mux-only path the browser renderer already uses.
        let direct = extension == "mp4" && crate::export_engine::writes_finished_files();
        let render_target = match direct {
            true => output_path.clone(),
            false => std::env::temp_dir().join(format!("recast-engine-{export_id}.mp4")),
        };
        // A stalled bar beats one that rewinds: the mux pass restarts at 0 and the UI keeps the maximum.
        let ceiling = if direct { 100.0 } else { ENGINE_RENDER_CEILING };
        let captions = engine_caption_burn_in(&app, &request).await;
        // Whole percents only: the frontend redraws per event, and 30 a second is not a smoother bar.
        let mut last_pct = -1i64;
        let mut on_frame = |done: u64, total: u64| {
            if cancel_flag.load(Ordering::Acquire) {
                return crate::export_engine::Flow::Cancel;
            }
            let pct = (done as f64 / total.max(1) as f64 * ceiling) as i64;
            if pct != last_pct {
                last_pct = pct;
                emit_export_state(&app, ExportStateEvent::progress(&export_id, pct as f64));
            }
            crate::export_engine::Flow::Continue
        };
        let result = crate::export_engine::export_video(
            &request.render_state,
            &crate::export_engine::ExportSpec {
                input: &source_video,
                output: &render_target,
                fps: (target_fps.round().max(1.0) as u32, 1),
                // Derived from what is actually rendered, which is the only place the quality cap has been applied.
                bitrate: None,
                max_size: profile.max_width.zip(profile.max_height),
                captions: captions.as_ref(),
                // The mux pass owns the music clips and the voice detach, so an intermediate must not carry a second track.
                audio: direct,
                // Where FFmpeg is the codec backend it decodes from a raw pipe, which carries no geometry of its own.
                source: recast_export::SourceInfo {
                    width: metadata.width,
                    height: metadata.height,
                    fps: source_fps,
                },
                ffmpeg: Some(crate::ffmpeg::ffmpeg_path()),
                // The platform decides; the flag exists so the piped path is testable where a native one exists.
                force_ffmpeg: false,
                audio_sources: crate::export_audio::RecordingAudio {
                    video: Some(&source_video),
                    system: project.as_ref().and_then(|p| p.audio_path.as_deref()),
                    microphone: project.as_ref().and_then(|p| p.microphone_path.as_deref()),
                },
            },
            &mut on_frame,
        );
        let frames = match result {
            Ok(frames) => frames,
            Err(crate::export_engine::EngineExportError::Cancelled) => {
                let _ = std::fs::remove_file(&render_target);
                emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
                return Err(AppError::msg("export cancelled"));
            }
            Err(e) => {
                let _ = std::fs::remove_file(&render_target);
                return Err(AppError::msg(format!("engine export failed: {e}")));
            }
        };
        log::info!("export[{export_id}] engine path wrote {frames} frames");
        if !direct {
            // Released before the mux job takes its own for the same export id.
            drop(_cancel_token);
            return run_mux_job(
                app.clone(),
                request,
                render_target.to_string_lossy().into_owned(),
            )
            .await;
        }
        emit_export_state(&app, ExportStateEvent::progress(&export_id, 100.0));
        emit_export_state(
            &app,
            ExportStateEvent::success(&export_id, &output_path.to_string_lossy()),
        );
        return Ok(output_path.to_string_lossy().into_owned());
    }

    let asset_cache_dir = app
        .path()
        .app_data_dir()
        .ok()
        .map(|base| base.join("assets"));

    // `layers` owns the temp-file guards for the static layers, so it must outlive the encode that reads them.
    let layers = crate::commands::export::raster::rasterize_static_layers(
        &mut request,
        metadata.width,
        metadata.height,
        asset_cache_dir.as_deref(),
        &static_root(),
        prebake_static_background,
    )?;
    let canvas_geom = layers.geom;
    let canvas_width = canvas_geom.canvas_w;
    let canvas_height = canvas_geom.canvas_h;
    let canvas_padding = canvas_geom.padding_px;
    let comp_width = canvas_geom.comp_w;
    let comp_height = canvas_geom.comp_h;
    let border_radius_mask_path = layers.border_radius_mask.as_ref().map(|m| m.path.clone());
    let drop_shadow_mask_path = layers.drop_shadow_mask.as_ref().map(|m| m.path.clone());
    let gradient_bg_path = layers.gradient_bg.as_ref().map(|m| m.path.clone());
    // Rebuild so the plan sees a pre-baked background; trim was read above and the swap doesn't affect it.
    let graph = RenderGraph::from_state(&request.render_state);

    // Derived on the same post-trim kept-segment windows as speed, so the tail cut+speed stage re-times them like zoom.
    let scene_overlay = if request.render_state.scene_animations.is_empty() {
        None
    } else {
        let scene_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
        let windows: Vec<(f64, f64)> = resolve_speed_segments(
            request.time_map.as_ref(),
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
    // Prep (probe + masks + plan) ends here; the cursor/overlay pre-render is timed apart as the prime suspect.
    let prep_ms = export_start.elapsed().as_millis();
    let cursor_render_start = Instant::now();
    // Not gated on the drop shadow: it composites as a static PNG, so including it pre-rendered empty frames.
    let needs_overlay =
        request.render_state.cursor_enabled || !request.render_state.annotations.is_empty();
    // The pre-render walks every output frame before the encode, so a plain 'Preparing' here reads as a hang.
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

    // The filtergraph is the dominant cost and single-threaded; parallelising it is byte-identical output.
    let filter_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    let mut args = vec![
        "-hide_banner".to_string(),
        "-loglevel".to_string(),
        "error".to_string(),
        "-y".to_string(),
        // Progress on stderr: Windows NVENC batches pipe:1 writes into one burst before `progress=end`. `-stats_period 0.1` forces 100 ms.
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

    // `-loop 1` defaults to 25 fps and the background is the composite base, so pin looped images to the source rate.
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

    // Camera overlay mirrors CameraOverlay.svelte: square bubble sized off video_w, offset by video_x/y, non-square clipped by an alphamerge mask.
    let camera_overlay_settings = &request.render_state.camera_overlay;
    let camera_path = if camera_overlay_settings.enabled {
        project
            .as_ref()
            .and_then(|p| p.camera_path.clone())
            .filter(|p| p.exists())
    } else {
        None
    };
    let camera_bubble: Option<(PathBuf, u32, u32, u32, u32)> = camera_path.as_ref().map(|path| {
        let (bubble_x, bubble_y, bubble_w, bubble_h) =
            camera_bubble_rect(&camera_overlay_settings.default_placement, &canvas_geom);
        (path.clone(), bubble_x, bubble_y, bubble_w, bubble_h)
    });

    // Square needs no mask: mask_input_index stays None and the chain skips alphamerge.
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

    let camera_input_index = camera_bubble
        .as_ref()
        .map(|_| 1 + export_plan.extra_inputs.len() + cursor_overlay_path.is_some() as usize);
    if let Some((ref path, _, _, _, _)) = camera_bubble {
        // The webcam recorder starts in another webview; shifting the input drops negative-PTS frames as the head trim.
        if let Some(shift) = camera_input_offset_secs(track_offsets.camera_ms) {
            args.extend(["-itsoffset".to_string(), format!("{shift:.3}")]);
        }
        args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
    }
    let camera_mask_input_index = camera_mask_path.as_ref().map(|_| {
        1 + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
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

    // Padded black silhouette scaled and positioned by FFmpeg, mirroring the preview's box-shadow; None at strength 0.
    let camera_shadow: Option<(MaskResult, u32, u32, u32)> =
        if let Some(&(_, _, _, bw, bh)) = camera_bubble.as_ref() {
            match camera_shadow_geom(camera_overlay_settings.shadow, bw) {
                Some(geom) => {
                    let radius_px = match camera_overlay_settings.shape.as_str() {
                        "circle" => bw as f64 / 2.0,
                        "square" | "rectangle" => 0.0,
                        _ => camera_overlay_settings.corner_radius * bw as f64,
                    };
                    crate::render::mask_export::render_camera_shadow(
                        crate::render::mask_export::CameraShadowRequest {
                            bubble_w: bw,
                            bubble_h: bh,
                            corner_radius_px: radius_px,
                            blur_px: geom.blur_px,
                            offset_y: geom.offset_px,
                            opacity: geom.opacity,
                            padding: geom.padding,
                        },
                    )
                    .map_err(|e| AppError::msg(format!("camera shadow render failed: {e}")))?
                    .map(|res| {
                        (
                            res,
                            geom.padding,
                            bw + 2 * geom.padding,
                            bh + 2 * geom.padding,
                        )
                    })
                }
                None => None,
            }
        } else {
            None
        };
    let camera_shadow_path = camera_shadow.as_ref().map(|(m, _, _, _)| m.path.clone());
    let camera_shadow_input_index = camera_shadow_path.as_ref().map(|_| {
        1 + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
            + camera_input_index.is_some() as usize
            + camera_mask_input_index.is_some() as usize
    });
    if let Some(ref path) = camera_shadow_path {
        args.extend([
            "-loop".to_string(),
            "1".to_string(),
            "-i".to_string(),
            path.to_string_lossy().to_string(),
        ]);
    }
    let camera_shadow_overlay = match (camera_shadow_input_index, camera_shadow.as_ref()) {
        (Some(idx), Some((_, padding, cw, ch))) => Some(CameraShadowOverlay {
            input_index: idx,
            padding: *padding,
            canvas_w: *cw,
            canvas_h: *ch,
        }),
        _ => None,
    };

    // Detached audio: the recording's own audio is edited as `voice` clips, so the monolithic tracks are dropped.
    let audio_detached = request
        .render_state
        .music_clips
        .iter()
        .any(|c| c.role == crate::render::node_types::AudioClipRole::Voice);

    let mut audio_input_indices: Vec<(usize, AudioKind)> = Vec::new();
    let source_has_audio = has_audio(&source_video);
    if request.format != "gif" && source_has_audio && !audio_detached {
        audio_input_indices.push((0, AudioKind::Source));
    }
    let mut music_inputs: Vec<(usize, &crate::render::node_types::AudioClip)> = Vec::new();
    if request.format != "gif" {
        let mut next_audio_input_index = 1
            + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
            + camera_input_index.is_some() as usize
            + camera_mask_input_index.is_some() as usize
            + camera_shadow_input_index.is_some() as usize;
        if let Some(project) = project.as_ref().filter(|_| !audio_detached) {
            for (path, kind) in [
                (&project.audio_path, AudioKind::System),
                (&project.microphone_path, AudioKind::Mic),
            ] {
                // See `run_mux_job`: a header-only WAV exists but carries no samples, and feeding one aborts the export.
                let Some(path) = path
                    .as_ref()
                    .filter(|p| crate::audio::wav::wav_has_samples(p))
                else {
                    if let Some(p) = path.as_ref().filter(|p| p.exists()) {
                        log::info!("export: skipping empty audio track {}", p.display());
                    }
                    continue;
                };
                audio_input_indices.push((next_audio_input_index, kind));
                next_audio_input_index += 1;
                args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
            }
        }
        // Looping clips take input-level `-stream_loop -1`; the filter stage trims them to the output length.
        for clip in &request.render_state.music_clips {
            if clip.muted || clip.gain <= 0.0 {
                continue;
            }
            let path = clip.source.asset_path();
            if path.is_empty() || !Path::new(path).exists() {
                continue;
            }
            if clip.looping {
                args.extend(["-stream_loop".to_string(), "-1".to_string()]);
            }
            args.extend(["-i".to_string(), path.to_string()]);
            music_inputs.push((next_audio_input_index, clip));
            next_audio_input_index += 1;
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

    // After the cursor, so the bubble sits above a branding mark and below a blur the user may put over their face.
    if let (Some(cam_idx), Some((_, bx, by, bw, bh))) = (camera_input_index, camera_bubble.as_ref())
    {
        // Mirrors the preview's cameraPlacementAt then applyZoomFollow; None keeps the fixed placement byte-identical.
        let camera_anim = if camera_overlay_settings.zoom_follow
            || !camera_overlay_settings.keyframes.is_empty()
        {
            build_camera_follow_exprs(
                &request.render_state.zoom_regions,
                &camera_overlay_settings.keyframes,
                camera_overlay_settings.keyframe_easing,
                camera_overlay_settings.zoom_follow_easing,
                camera_overlay_settings.zoom_follow_duration,
                &camera_overlay_settings.default_placement,
                camera_overlay_settings.zoom_follow_strength,
                camera_overlay_settings.zoom_follow,
                &canvas_geom,
                trim_start,
                request.render_state.trim_end,
            )
            .map(|(size_expr, x_expr, y_expr)| CameraOverlayAnim {
                size_expr,
                x_expr,
                y_expr,
            })
        } else {
            None
        };
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
                anim: camera_anim,
                shadow: camera_shadow_overlay,
            },
        );
        filter_complex_after_cursor = Some(new_complex);
        video_map_after_cursor = new_map;
    }

    // After the cursor overlay so the blur covers it too, but before GIF palettization so the palette sees blurred pixels.
    let blur_regions = crate::commands::export::blur::blur_regions(
        &request.render_state.annotations,
        &canvas_geom,
        trim_start,
    );
    if !blur_regions.is_empty() {
        let (new_complex, new_map) = build_annotation_blur_complex(
            filter_complex_after_cursor.as_deref(),
            &video_map_after_cursor,
            &blur_regions,
        );
        filter_complex_after_cursor = Some(new_complex);
        video_map_after_cursor = new_map;
    }

    // Burned on the trimmed-but-uncut axis so the cut/speed stage re-times captions with everything else.
    if let Some((new_complex, new_map)) = append_caption_burn_in(
        &app,
        &request,
        canvas_width,
        canvas_height,
        &canvas_geom,
        trim_start,
        duration,
        filter_complex_after_cursor.as_deref(),
        &video_map_after_cursor,
    )
    .await?
    {
        filter_complex_after_cursor = Some(new_complex);
        video_map_after_cursor = new_map;
    }

    // GIF pass 1 emits the palette separately: single-pass palettegen buffers every frame and pins progress at 0%.
    let mut output_filters: Vec<String> = Vec::new();
    let gif_settings: GifSettings = request.gif_settings.clone().unwrap_or_default();
    let mut palette_temp_path: Option<PathBuf> = None;
    let progress_band = if request.format == "gif" {
        let palette_input_index =
            1 + export_plan.extra_inputs.len() + cursor_overlay_path.is_some() as usize;
        let gif_out = run_gif_pass(GifPassParams {
            app: &app,
            export_id: &export_id,
            cancel_flag: cancel_flag.clone(),
            source_video: &source_video,
            output_dir: &output_dir,
            output_scale_filter: output_scale_filter.as_deref(),
            trim_start,
            trim_end,
            duration,
            source_duration,
            render_state: &request.render_state,
            time_map: request.time_map.as_ref(),
            gif_settings: &gif_settings,
            gif_fps: profile.gif_fps,
            palette_input_index,
            filter_complex: filter_complex_after_cursor.take(),
            video_map: video_map_after_cursor.clone(),
        })
        .await;
        match gif_out {
            Ok(out) => {
                args.extend(out.palette_input_args);
                filter_complex_after_cursor = out.filter_complex;
                video_map_after_cursor = out.video_map;
                palette_temp_path = Some(out.palette_temp_path);
                ProgressBand {
                    offset: 40.0,
                    scale: 0.6,
                }
            }
            Err(GifPassError::Cancelled) => {
                emit_export_state(&app, ExportStateEvent::cancelled(&export_id));
                return Err(AppError::from("export cancelled"));
            }
            Err(GifPassError::Failed(msg)) => {
                emit_export_state(&app, ExportStateEvent::error(&export_id, &msg));
                return Err(AppError::from(msg));
            }
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
            track_offsets,
        )
        .map(|(new_complex, map)| {
            filter_complex_after_cursor = Some(new_complex);
            map
        })
    };

    // Cuts run at the END of the chain: select only removes frames, so upstream zoom/cursor/blur stay correct. GIF skipped.
    let export_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
    // Per-segment speed warps the survivors on top of the cut drop; the segments mirror the frontend time map.
    let speed_segments = resolve_speed_segments(
        request.time_map.as_ref(),
        duration,
        &export_cuts,
        &request.render_state.split_points,
        &request.render_state.segment_speeds,
        trim_start,
    );
    let speed_active = has_speed_change(&speed_segments);
    append_cut_speed_stage(
        &mut filter_complex_after_cursor,
        &mut video_map_after_cursor,
        &mut audio_map,
        &mut output_filters,
        &request.format,
        &export_cuts,
        &speed_segments,
        speed_active,
    );

    // After cut/speed, so music clips sit on the same output timeline the viewer sees.
    if !music_inputs.is_empty() {
        let out_dur = warped_output_duration(&speed_segments);
        if let Some((seg, map)) = build_music_stage(&music_inputs, audio_map.as_deref(), out_dur) {
            if !seg.is_empty() {
                filter_complex_after_cursor = Some(match filter_complex_after_cursor.take() {
                    Some(fc) => format!("{fc};{seg}"),
                    None => seg,
                });
            }
            audio_map = Some(map);
        }
    }

    // Merge output-side filters before emitting, so the string handed to FFmpeg is final.
    if !output_filters.is_empty() && filter_complex_after_cursor.is_some() {
        let (complex_filter, map_label) = append_output_filters_to_complex(
            filter_complex_after_cursor.as_deref().unwrap_or_default(),
            &video_map_after_cursor,
            &output_filters,
        );
        filter_complex_after_cursor = Some(complex_filter);
        video_map_after_cursor = map_label;
    }

    // Dense LUT expressions blow Windows' ~32 KB command line (os error 206), so spill to `-filter_complex_script` in temp.
    let mut filter_script_path: Option<PathBuf> = None;
    if let Some(ref filter_complex) = filter_complex_after_cursor {
        if filter_complex.len() > FILTER_COMPLEX_SCRIPT_THRESHOLD {
            let path = std::env::temp_dir().join(format!("recast-filtergraph-{export_id}.txt"));
            std::fs::write(&path, filter_complex).map_err(|e| {
                AppError::msg(format!(
                    "failed to write filter script {}: {e}",
                    path.display()
                ))
            })?;
            args.extend([
                "-filter_complex_script".to_string(),
                path.to_string_lossy().to_string(),
                "-map".to_string(),
                video_map_after_cursor.clone(),
            ]);
            filter_script_path = Some(path);
        } else {
            args.extend([
                "-filter_complex".to_string(),
                filter_complex.clone(),
                "-map".to_string(),
                video_map_after_cursor.clone(),
            ]);
        }
    } else {
        args.extend(["-map".to_string(), "0:v:0".to_string()]);
    }

    if let Some(ref audio_map) = audio_map {
        args.extend(["-map".to_string(), audio_map.clone()]);
    }

    if !output_filters.is_empty() && filter_complex_after_cursor.is_none() {
        args.extend(["-vf".to_string(), output_filters.join(",")]);
    }

    let expected_output_secs = crate::commands::export::tail::append_output_tail(
        &mut args,
        &request,
        duration,
        source_duration,
        &speed_segments,
        !export_plan.extra_inputs.is_empty() || cursor_overlay_path.is_some(),
    )
    .expected_output_secs;

    // Snapshot inputs, filter and maps before the codec tail so a hardware crash can retry with software.
    let base_args = args.clone();
    append_codec_args(
        &mut args,
        &request.format,
        &gif_settings,
        &profile,
        speed,
        audio_map.is_some(),
        &output_path,
        false,
    );

    let output_path_str = output_path.to_string_lossy().to_string();
    // (the full command is logged once inside run_encode at spawn.)

    // Read the encoder off the emitted args so it stays right across formats; captured before `args` moves.
    let video_encoder = args
        .iter()
        .position(|a| a == "-c:v")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "unknown".to_string());
    // Hardware h264 can segfault mid-encode (0xC0000005), so prebuild the same command with software x264 for one retry.
    let sw_retry_args = if ["nvenc", "amf", "qsv", "videotoolbox"]
        .iter()
        .any(|k| video_encoder.contains(k))
    {
        let mut a = base_args;
        append_codec_args(
            &mut a,
            &request.format,
            &gif_settings,
            &profile,
            speed,
            audio_map.is_some(),
            &output_path,
            true,
        );
        Some(a)
    } else {
        None
    };
    // `-hwaccel auto` can still fall back internally, so report the requested mode rather than claiming hardware.
    let decode_mode = args
        .iter()
        .position(|a| a == "-hwaccel")
        .and_then(|i| args.get(i + 1).cloned())
        .map(|v| format!("hwaccel:{v}"))
        .unwrap_or_else(|| "software".to_string());
    log::info!("export[{export_id}] encoder={video_encoder} decode={decode_mode} filter_threads={filter_threads}");

    // Background thread with a 60s no-progress watchdog; the handle is cloned for the panic-fallback emit below.
    let app_for_fallback = app.clone();
    // A CLONE moves into the encode task: `app` must outlive it to remove the cancel token below.
    let app_for_task = app.clone();
    let export_id_for_task = export_id.clone();
    let export_id_for_fallback = export_id.clone();
    // Clones so the software-x264 retry runs inside the same task, reusing the same output and inputs.
    let retry_app = app.clone();
    let retry_export_id = export_id.clone();
    let retry_output = output_path_str.clone();
    let retry_cancel = cancel_flag.clone();
    let task_result = tokio::task::spawn_blocking(move || {
        let first = run_encode(
            args,
            app_for_task,
            export_id_for_task,
            cancel_flag,
            output_path_str,
            expected_output_secs,
            progress_band,
        );
        match first {
            Err(e)
                if sw_retry_args.is_some()
                    && parse_ffmpeg_exit_code(&e).is_some_and(is_ffmpeg_crash_code) =>
            {
                log::warn!(
                    "export[{retry_export_id}]: hardware encoder crashed ({e}); retrying with software x264"
                );
                run_encode(
                    sw_retry_args.unwrap(),
                    retry_app,
                    retry_export_id,
                    retry_cancel,
                    retry_output,
                    expected_output_secs,
                    progress_band,
                )
            }
            other => other,
        }
    })
    .await;

    // The cancel token is RAII-owned by `_cancel_token`; this drop is just the cursor overlay's temp dir.
    drop(cursor_overlay);
    if let Some(p) = palette_temp_path.as_ref() {
        let _ = std::fs::remove_file(p);
    }
    if let Some(p) = filter_script_path.as_ref() {
        let _ = std::fs::remove_file(p);
    }

    match task_result {
        Ok(inner) => {
            if inner.is_ok() {
                // One correlated line: total wall clock plus stage breakdown; the encode logs its own duration inside the task.
                log::info!(
                    "export[{export_id}] timing: total={}ms prep={prep_ms}ms cursor_overlay={cursor_ms}ms (ran={cursor_ran}) encoder={video_encoder} decode={decode_mode}",
                    export_start.elapsed().as_millis()
                );
            }
            inner.map_err(Into::into)
        }
        Err(join_err) => {
            // spawn_blocking only errors on panic; surface it so the frontend shows a failure instead of hanging.
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
    // No installed token means no active export; a no-op keeps a double-clicked Cancel from toasting.
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

/// Scores, clusters and density-limits a captured cursor track into auto-focus candidates.
/// Always recomputes rather than trusting the persisted `zoom_triggers`, or a clip recorded before a detector improvement keeps serving noisier suggestions.
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

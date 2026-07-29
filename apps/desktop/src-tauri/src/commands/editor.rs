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
use super::export::camera::{build_camera_follow_exprs, camera_bubble_rect, camera_shadow_geom};
use super::export::captions::append_caption_burn_in;
use super::export::codec::append_codec_args;
use super::export::cuts_speed::{
    append_cut_speed_stage, build_speed_segments, collect_export_cuts, has_speed_change,
    output_duration_cap, warped_output_duration,
};
use super::export::gif::{run_gif_pass, GifPassError, GifPassParams};
use super::export::progress::ProgressBand;
use super::export::run::run_encode;
use super::export::state::{emit_export_state, ExportStateEvent};
use super::ffmpeg::{
    append_camera_overlay_to_complex, append_cursor_overlay_to_complex,
    append_output_filters_to_complex, build_annotation_blur_complex, build_output_scale_filter,
    has_audio, probe_video_metadata, resolve_export_profile, BlurRegion, CameraOverlayAnim,
    CameraOverlayParams, CameraShadowOverlay, ExportSpeed,
};
use super::system::get_active_output_dir;
use super::types::{AppState, EditorDocument, ExportRequest, GifSettings, VideoMetadata};
use crate::project::reader::ProjectOpenResult;
#[allow(unused_imports)]
use crate::render::cursor_export::{render_cursor_overlay, CursorOverlayRequest};
use crate::render::graph::{RenderGraph, RenderState, SourceVideoMetadata};
use crate::render::mask_export::{render_border_radius_mask, MaskResult};
use crate::render::node_types::{AnnotationAnchor, AnnotationKind, AudioSettings};

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

/// Effective linear gain for one input: master × its per-source gain, 0 when
/// muted. Mirrors the preview's `effectiveTrackVolume` so preview and export
/// apply the same mix.
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
) -> Option<(String, String)> {
    if audio_inputs.is_empty() || settings.muted || settings.volume <= 0.0 {
        return None;
    }

    // Drop fully-silenced sources: leaving a muted input in the amix would let
    // it average the others back down. This is the fix for per-source mute/gain
    // being ignored at export.
    let live: Vec<(usize, f64)> = audio_inputs
        .iter()
        .map(|&(idx, kind)| (idx, effective_audio_gain(settings, kind)))
        .filter(|&(_, gain)| gain > 0.0)
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

    for (i, (input_index, gain)) in live.iter().enumerate() {
        let label = if live.len() == 1 {
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

    // EBU R128 loudness normalize on the final mix: -14 LUFS is the common social
    // target (YouTube/Spotify-ish), -1 dBTP ceiling. Single-pass — a measured
    // two-pass is a later refinement.
    if settings.normalize_loudness {
        segments.push("[aout]loudnorm=I=-14:TP=-1:LRA=11[aoutn]".to_string());
        return Some((segments.join(";"), "[aoutn]".into()));
    }

    Some((segments.join(";"), "[aout]".into()))
}

/// Mix output-timeline music/extra-audio clips onto the finished source audio.
/// `clips` pairs each live clip with its ffmpeg input index; `source_audio` is
/// the edited recording's audio label (None when muted/absent). Each clip is
/// trimmed into its source, gained, faded, and delayed onto the output timeline,
/// then amixed with the source. Returns (extra filter segments, final map).
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

/// Attribution lines the music clips' licenses require (CC-BY), deduped and
/// joined for the output file's `comment` metadata so the credit travels with
/// the exported video. None when nothing needs crediting.
fn build_credits_comment(clips: &[crate::render::node_types::AudioClip]) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for clip in clips {
        if let Some(a) = clip.source.attribution() {
            if !seen.contains(&a) {
                seen.push(a);
            }
        }
    }
    if seen.is_empty() {
        return None;
    }
    Some(format!("Music: {}", seen.join("; ")))
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
        let media_duration = project.metadata.media_duration_secs();
        let default_state = || RenderState {
            trim_end: media_duration,
            ..RenderState::default()
        };
        // A missing edits.json is a fresh project (expected → defaults). A parse
        // FAILURE, though, would silently discard every edit, so surface it.
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
        // Heal projects saved before `media_duration_secs` existed: their
        // `trim_end` came from the wall clock and overshoots the encoded file,
        // which makes `enqueue_export` reject the state the app itself wrote.
        // Clamping costs nothing — there are no frames out there to keep.
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
        metadata: metadata.clone(),
        render_state: RenderState {
            trim_end: metadata.duration,
            ..RenderState::default()
        },
        needs_migration: false,
    })
}

/// Read-only summary of a project's timeline. Mirrors the data shape the
/// frontend's `deriveSegments` + `timeMapFromSegments` produce in
/// `apps/desktop/src/lib/timeline/{segments,time-map}.ts`. The shared parity
/// fixtures already enforce that the Rust side (these helpers) and the JS side
/// agree to the same precision, so an agent that reads this view and then
/// issues a follow-up `editor.*` patch can trust the structure it sees.
///
/// Costs of deriving this are negligible (a single pass over the cuts +
/// split_points on the in-memory render state); safe to call from a control-
/// socket dispatch arm without a `spawn_blocking`.
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

/// Common load → mutate → validate → save cycle for every targeted editor
/// verb in `control::dispatch`. The mutate closure returns its own result
/// (used by `editor.zoom.add` etc. to return the new entry it just
/// pushed).
///
/// Spawn-blocking the load + save calls is fine here: both already run on
/// Tauri's blocking pool (`commands::load_editor_document` and
/// `commands::save_project_edits` internally `spawn_blocking`).
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
    )?;

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
    crate::commands::persist(state, app);
    let _ = app.emit("editor-state:changed", serde_json::json!({ "path": path }));
    Ok(result)
}

/// Apply the agent-supplied `value` at a dotted JSON pointer inside the
/// JSON shape of `RenderState`. Walks the path, replaces the leaf, and
/// reports a structured error if the path doesn't exist.
pub(crate) fn apply_dotted_path_set(
    state: &mut serde_json::Value,
    field: &str,
    value: serde_json::Value,
) -> Result<(), String> {
    let pointer = format!("/{}", field.replace('.', "/"));
    let target = state
        .pointer_mut(&pointer)
        .ok_or_else(|| format!("no field at path '{field}'"))?;
    *target = value;
    Ok(())
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

/// Validate a `RenderState` against the project's source-recording metadata.
///
/// **Runs at every entry point** that crosses a trust boundary: `enqueue_export`,
/// `save_project_edits`, every CLI `editor.*` patch verb, every MCP tool
/// handler. The function returns *all* violations, not the first; an agent
/// iterating on a JSON document needs the complete list to fix in one pass.
///
/// Pure (no I/O, no AppHandle, no State). Costs ~O(n) over the render state's
/// collections; safe to call on a control-socket connection's thread without
/// Clamp a render state's trim window and cuts to the real source duration,
/// repairing the common `trim_end_exceeds_source` case: recordings saved before
/// the wall-clock→CFR fix baked a slightly-too-long `trim_end` into their edits
/// (see project/mod.rs), so exporting an old project fails validation. Pure;
/// mutates `s` in place and returns human-readable descriptions of every change
/// (empty = nothing needed repair). Run BEFORE `validate_render_state`.
pub fn repair_render_state(s: &mut RenderState, source_duration: f64) -> Vec<String> {
    let mut repairs = Vec::new();
    if !(source_duration > 0.0) {
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

    // Annotations: clamp each into [trim_start, trim_end] with a forward window.
    // Repairs the `annotation_end_before_start` / `annotation_out_of_trim` cases —
    // e.g. an annotation added while the playhead was parked past the trim end.
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

/// `spawn_blocking`.
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
    // Only a strict `trim_end < trim_start` is an error: the fresh-project
    // default is `trim_end == trim_start == 0.0` and that's a valid
    // (zero-duration) state the editor holds until first content is loaded.
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
        // Allow a small slop so an end-user dragging a cut edge against the
        // trim handle doesn't bounce on the validator; deeper math already
        // clamps at the export anyway.
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
        if !(1.0..=3.0).contains(&z.scale) {
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
                // Older projects can carry an Unsupported variant after a
                // forward-compat change; carrying it through is fine, but its
                // position never had any value to validate against. No-op.
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
    // `cursor_smoothing` is exposed as a 0..100 slider on the UI; the
    // historical default is 50.0 and the export treats it as a percent.
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
        // Anchors are ORIGINAL seconds; first kept segment starts at trim_start=0,
        // so a 2× override anchored at original t=0 covers the whole clip.
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
    fn per_source_gain_reaches_the_graph() {
        let mut s = AudioSettings::default();
        s.system_volume = 100.0;
        s.mic_volume = 50.0;
        let (complex, map) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
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
        let mut s = AudioSettings::default();
        s.mic_muted = true;
        let (complex, _map) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
        )
        .expect("system still audible");
        // Only system survives → single branch, no amix, mic input absent.
        assert!(complex.contains("[1:a]"));
        assert!(!complex.contains("[2:a]"));
        assert!(!complex.contains("amix"));
    }

    #[test]
    fn all_sources_muted_yields_no_audio() {
        let mut s = AudioSettings::default();
        s.system_muted = true;
        s.mic_muted = true;
        assert!(append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0
        )
        .is_none());
    }

    #[test]
    fn normalize_appends_loudnorm_to_the_mix() {
        let mut s = AudioSettings::default();
        s.normalize_loudness = true;
        let (complex, map) = append_audio_to_complex(
            None,
            &[(1, AudioKind::System), (2, AudioKind::Mic)],
            &s,
            0.0,
            10.0,
        )
        .expect("audio graph");
        assert!(complex.contains("loudnorm=I=-14"));
        assert_eq!(map, "[aoutn]"); // normalize retargets the map to the loudnorm output
                                    // Off by default: no loudnorm, map stays [aout].
        let off = AudioSettings::default();
        let (c2, m2) = append_audio_to_complex(None, &[(1, AudioKind::System)], &off, 0.0, 10.0)
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
        let mut s = AudioSettings::default();
        s.volume = 50.0;
        s.mic_volume = 0.0; // a per-source gain must not touch an embedded source track
        let (complex, _) = append_audio_to_complex(None, &[(0, AudioKind::Source)], &s, 0.0, 10.0)
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
        let comment = build_credits_comment(&[
            provider("1", Some(line)),
            provider("2", Some(line)), // same line → deduped
            local,                     // local → no credit
        ])
        .expect("comment");
        assert_eq!(comment, format!("Music: {line}"));
        assert!(build_credits_comment(&[provider("3", None)]).is_none());
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
        // Pre-fix regression: a wall-clock trim_end past the CFR-encoded video
        // (27.102 session → 26.625 of video) is rejected by the validator.
        let mut st = RenderState {
            trim_start: 0.0,
            trim_end: 27.102,
            ..RenderState::default()
        };
        assert_eq!(
            reason(&validate_render_state(&st, 26.625).unwrap_err(), "trimEnd"),
            Some("trim_end_exceeds_source"),
        );
        // Repair clamps it to the real duration and reports the change; the same
        // state now validates.
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
        // Reproduces the report: annotations added while the playhead was parked
        // at/past the trim end got end <= start (or past trim), failing validation.
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

/// Run one export end to end: build the FFmpeg filter graph from the render
/// state, spawn the encode on a blocking worker, and emit `export-state` events
/// keyed by `request.export_id`. Returns the output path, or an `Err` whose
/// message contains "cancel" when the user aborted.
///
/// This is the single execution path for an export. It is NOT a Tauri command:
/// exports are started by enqueuing them (`commands::export_queue::enqueue_export`),
/// and the serial export worker is this function's only caller (a future CLI
/// export verb would call it too). It still owns its own cancel token in
/// `state.export_cancel` (so `cancel_export` finds it by id) and takes a power
/// lease for the run.
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

/// Mux a browser-rendered video (already composited AND warped to the output
/// timeline) with the export's audio. Video is copied (`-c:v copy`); only the
/// audio graph is (re)built here — the browser owns all compositing (Phase 4).
/// Reuses run_export_job's queue/cancel/progress lifecycle and the shared audio
/// helpers, which are index-parametric so the browser video can sit at input 0.
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
    let project = open_project_if_needed(&input_path)?;
    let source_video = project
        .as_ref()
        .map(|value| value.recording_path.clone())
        .unwrap_or_else(|| input_path.clone());

    let graph = RenderGraph::from_state(&request.render_state);
    let (trim_start, trim_end) = graph.trim_range();
    let duration = (trim_end - trim_start).max(0.0);

    // Input 0 = the browser video (video only); audio inputs follow, indexed so
    // append_audio_to_complex references the right streams.
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
                let Some(path) = path.as_ref().filter(|p| p.exists()) else {
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

    // Audio graph: build the source/system/mic mix, warp it to the output timeline
    // (cuts + speed — the browser video is ALREADY warped, so only audio needs it),
    // then mix in the music clips.
    let mut filter_complex: Option<String> = None;
    let mut audio_map = append_audio_to_complex(
        None,
        &audio_input_indices,
        &request.render_state.audio_settings,
        trim_start,
        duration,
    )
    .map(|(complex, map)| {
        filter_complex = Some(complex);
        map
    });
    let export_cuts = collect_export_cuts(&request.render_state, trim_start, trim_end);
    let speed_segments = build_speed_segments(
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

    if let Some(ref fc) = filter_complex {
        args.extend(["-filter_complex".to_string(), fc.clone()]);
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
        ]);
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

    match task_result {
        Ok(Ok(path)) => {
            // The browser video was a pre-encode temp; the muxed output supersedes
            // it. Kept on failure so a retry can re-mux without re-rendering.
            let _ = std::fs::remove_file(&browser_video);
            Ok(path)
        }
        Ok(Err(e)) => Err(AppError::msg(e)),
        Err(join) => Err(AppError::msg(format!("mux task panicked: {join}"))),
    }
}

pub(crate) async fn run_export_job(
    app: AppHandle,
    mut request: ExportRequest,
) -> AppResult<String> {
    let state = app.state::<AppState>();
    let export_id = request.export_id.clone();

    // Keep display + system awake for the whole export. RAII: released on every
    // return path (success, `?` error, cancel) when this scope ends.
    let _power = state.power.lease();

    // Install a fresh cancellation token for this run, scoped to the export
    // session id that the frontend also uses to filter state events.
    let cancel_flag = Arc::new(AtomicBool::new(false));
    state
        .export_cancel
        .lock()
        .insert(export_id.clone(), cancel_flag.clone());
    // RAII, like the power lease above: the token is removed on EVERY exit,
    // including the `?`s during prep. Hand removal left an entry stranded for
    // the process lifetime whenever prep failed, and a stale token poisons the
    // next export that reuses the id.
    let _cancel_token = CancelTokenGuard {
        app: app.clone(),
        export_id: export_id.clone(),
    };
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
    // NOT gated on the drop shadow: that's composited separately as a static PNG
    // in the graph (`drop_shadow_mask` → `compose_shadow_stage`), so including it
    // here ran the whole per-frame overlay pre-render just to emit empty frames.
    // Annotation glows still qualify via the `annotations` clause.
    let needs_overlay =
        request.render_state.cursor_enabled || !request.render_state.annotations.is_empty();
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
    let camera_bubble: Option<(PathBuf, u32, u32, u32, u32)> = camera_path.as_ref().map(|path| {
        let (bubble_x, bubble_y, bubble_w, bubble_h) =
            camera_bubble_rect(&camera_overlay_settings.default_placement, &canvas_geom);
        (path.clone(), bubble_x, bubble_y, bubble_w, bubble_h)
    });

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

    // Camera drop shadow: a padded black silhouette of the bubble shape, scaled
    // + positioned by FFmpeg to follow the bubble. Mirrors the preview's
    // box-shadow (cameraShadowStyle). `None` when the shadow strength is 0.
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
            + watermark_path.is_some() as usize
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

    // Detached audio: the recording's own audio is now edited as `voice` clips,
    // so the monolithic source/system/mic tracks are dropped (the clips carry it).
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
            + watermark_path.is_some() as usize
            + camera_input_index.is_some() as usize
            + camera_mask_input_index.is_some() as usize
            + camera_shadow_input_index.is_some() as usize;
        if let Some(project) = project.as_ref().filter(|_| !audio_detached) {
            for (path, kind) in [
                (&project.audio_path, AudioKind::System),
                (&project.microphone_path, AudioKind::Mic),
            ] {
                let Some(path) = path.as_ref().filter(|p| p.exists()) else {
                    continue;
                };
                audio_input_indices.push((next_audio_input_index, kind));
                next_audio_input_index += 1;
                args.extend(["-i".to_string(), path.to_string_lossy().to_string()]);
            }
        }
        // Music / extra-audio clips. Looping clips get `-stream_loop -1` (an
        // input-level flag); the filter stage trims them to the output length.
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
        // Per-cut keyframe glide + zoom-follow grow/drift over time (mirrors the
        // preview's cameraPlacementAt ∘ applyZoomFollow). None → the fixed
        // placement, byte-identical to before when there are no keyframes and
        // zoom-follow is off.
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

    // Burn captions into the trimmed-but-uncut axis so the cut/speed stage
    // re-times them with the rest; no-op without a transcript, and GIF skips it.
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
        let palette_input_index = 1
            + export_plan.extra_inputs.len()
            + cursor_overlay_path.is_some() as usize
            + watermark_path.is_some() as usize;
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

    // Mix music/extra-audio clips onto the finished (output-time) audio. Runs
    // after cut/speed so the clips sit on the same output timeline the viewer sees.
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

    // Merge any output-side filters (e.g. the final scale) into the complex graph
    // BEFORE emitting it, so the string handed to FFmpeg is final. (Previously
    // this happened after the args were built, patching the arg slot in place.)
    if !output_filters.is_empty() && filter_complex_after_cursor.is_some() {
        let (complex_filter, map_label) = append_output_filters_to_complex(
            filter_complex_after_cursor.as_deref().unwrap_or_default(),
            &video_map_after_cursor,
            &output_filters,
        );
        filter_complex_after_cursor = Some(complex_filter);
        video_map_after_cursor = map_label;
    }

    // Dense zoom/camera LUT expressions can push the filtergraph past Windows'
    // ~32 KB command-line limit ("The filename or extension is too long",
    // os error 206). Above a threshold, pass it via `-filter_complex_script
    // <file>` (read from disk, no command-line cost) instead of inline. The
    // file is removed after the encode finishes.
    let mut filter_script_path: Option<PathBuf> = None;
    if let Some(ref filter_complex) = filter_complex_after_cursor {
        if filter_complex.len() > FILTER_COMPLEX_SCRIPT_THRESHOLD {
            let path = output_dir.join(format!("recast-filtergraph-{export_id}.txt"));
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

    // CC-BY music requires credit, so bake the attribution into the output's
    // `comment` metadata (skipped for GIF — it carries no audio to credit).
    if request.format != "gif" {
        if let Some(comment) = build_credits_comment(&request.render_state.music_clips) {
            args.extend(["-metadata".to_string(), format!("comment={comment}")]);
        }
    }

    append_codec_args(
        &mut args,
        &request.format,
        &gif_settings,
        &profile,
        speed,
        audio_map.is_some(),
        &output_path,
    );

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
    // Move a CLONE into the encode task, not the original: `state` (derived from
    // `app`) is still needed below to remove the cancel token, so `app` must
    // outlive the task rather than be moved into it.
    let app_for_task = app.clone();
    let export_id_for_task = export_id.clone();
    let export_id_for_fallback = export_id.clone();
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

    // The cancel token is now owned by `_cancel_token` (RAII), so it survives
    // `?` and panics. This drop is just the cursor overlay's temp dir.
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

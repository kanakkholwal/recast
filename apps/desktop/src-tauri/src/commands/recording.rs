use std::fs;
use std::path::PathBuf;

use tauri::State;

use super::ffmpeg::probe_video_metadata;
use super::system::get_active_output_dir;
use super::types::{AppState, RecordingEntry};
use crate::project::writer::{write_project, ProjectWriteRequest};
use crate::project::{ProjectMediaMetadata, ProjectMetadata, ProjectVideoMetadata};
use crate::recording::{CaptureTarget, RecordingOptions, RegionRect};
use crate::render::graph::RenderState;

fn recasts_dir(state: &State<'_, AppState>) -> PathBuf {
    let dir = get_active_output_dir(state).join("recasts");
    let _ = fs::create_dir_all(&dir);
    dir
}

fn exports_dir(state: &State<'_, AppState>) -> PathBuf {
    let dir = get_active_output_dir(state).join("exports");
    let _ = fs::create_dir_all(&dir);
    dir
}

#[tauri::command]
pub fn start_recording(
    target_type: String,
    target_id: u32,
    region: Option<RegionRect>,
    options: Option<RecordingOptions>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let target = if target_type == "region" {
        let rect = region.ok_or_else(|| "region target requires a rect".to_string())?;
        CaptureTarget::resolve_region(rect).map_err(|e| e.to_string())?
    } else {
        CaptureTarget::resolve(&target_type, target_id).map_err(|e| e.to_string())?
    };
    let output_dir = get_active_output_dir(&state);
    state
        .recording_manager
        .start(target, output_dir, options.unwrap_or_default())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn stop_recording(state: State<'_, AppState>) -> Result<String, String> {
    let artifacts = state.recording_manager.stop().map_err(|e| e.to_string())?;
    let dest = recasts_dir(&state);
    let final_path = dest.join(format!("recast_{}.recast", artifacts.started_at_unix_ms));
    let recording_meta = probe_video_metadata(&artifacts.recording_path)?;
    let metadata = ProjectMetadata {
        schema_version: 1,
        created_at_unix_ms: artifacts.started_at_unix_ms,
        capture_target: artifacts.capture_target.clone(),
        stats: artifacts.stats.clone(),
        video: ProjectVideoMetadata {
            width: if recording_meta.width > 0 {
                recording_meta.width
            } else {
                artifacts.capture_target.crop.width
            },
            height: if recording_meta.height > 0 {
                recording_meta.height
            } else {
                artifacts.capture_target.crop.height
            },
            fps: recording_meta.fps.round().max(1.0) as u32,
            duration_ms: artifacts.stats.duration_ms,
        },
        media: Some(ProjectMediaMetadata {
            has_system_audio: true,
            has_microphone: artifacts.microphone_path.is_some(),
            has_camera: artifacts.camera_path.is_some(),
        }),
    };
    let default_render_state = RenderState {
        trim_end: artifacts.stats.duration_ms as f64 / 1000.0,
        ..RenderState::default()
    };
    let project_path = write_project(ProjectWriteRequest {
        output_path: final_path.clone(),
        metadata,
        recording_path: artifacts.recording_path.clone(),
        cursor_path: artifacts.cursor_path.clone(),
        audio_path: artifacts.audio_path.clone(),
        microphone_path: artifacts.microphone_path.clone(),
        camera_path: artifacts.camera_path.clone(),
        edits_json: serde_json::to_string_pretty(&default_render_state)
            .unwrap_or_else(|_| "{}".into()),
    })
    .map_err(|e| e.to_string())?;

    // Clean up temporary session files.
    let _ = fs::remove_file(&artifacts.recording_path);
    let _ = fs::remove_file(&artifacts.cursor_path);
    let _ = fs::remove_file(&artifacts.audio_path);
    if let Some(ref mic_path) = artifacts.microphone_path {
        let _ = fs::remove_file(mic_path);
    }
    if let Some(ref cam_path) = artifacts.camera_path {
        let _ = fs::remove_file(cam_path);
    }

    *state.last_file_path.lock() = Some(project_path.to_string_lossy().to_string());
    Ok(project_path.to_string_lossy().to_string())
}

#[tauri::command]
pub fn list_recasts(state: State<'_, AppState>) -> Result<Vec<RecordingEntry>, String> {
    list_files_by_ext(&recasts_dir(&state), "recast")
}

#[tauri::command]
pub fn list_exports(state: State<'_, AppState>) -> Result<Vec<RecordingEntry>, String> {
    let dir = exports_dir(&state);
    let mut entries = Vec::new();
    for ext in &["mp4", "webm", "gif"] {
        entries.extend(list_files_by_ext(&dir, ext).unwrap_or_default());
    }
    entries.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(entries)
}

fn list_files_by_ext(dir: &PathBuf, ext: &str) -> Result<Vec<RecordingEntry>, String> {
    let mut entries = Vec::new();
    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(_) => return Ok(entries),
    };

    for entry in read.flatten() {
        let path = entry.path();
        let file_ext = path
            .extension()
            .and_then(|v| v.to_str())
            .unwrap_or_default();
        if file_ext != ext {
            continue;
        }
        if let Ok(meta) = entry.metadata() {
            let created = meta
                .modified()
                .unwrap_or(std::time::SystemTime::UNIX_EPOCH)
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            entries.push(RecordingEntry {
                filename: entry.file_name().to_string_lossy().to_string(),
                path: path.to_string_lossy().to_string(),
                size_bytes: meta.len(),
                created,
            });
        }
    }
    entries.sort_by(|a, b| b.created.cmp(&a.created));
    Ok(entries)
}

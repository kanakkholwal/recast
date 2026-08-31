//! IPC commands delivering an OS-opened `.recast` into a fresh editor window.
//! `peek_recast_project` reads only `metadata.json` so the frontend can reject a bad file before navigating.

use std::fs::File;
use std::path::PathBuf;

use tauri::State;
use zip::ZipArchive;

use crate::commands::error::AppResult;
use crate::commands::types::AppState;
use crate::project::ProjectMetadata;
use crate::tray;

#[tauri::command]
pub fn take_pending_open_file(state: State<'_, AppState>) -> Option<String> {
    state
        .pending_open_file
        .lock()
        .take()
        .map(|p| p.to_string_lossy().to_string())
}

/// Whether the app was cold-launched via the jump list "New Recording" task.
/// Drained once by the main window on mount, which then opens the panel.
#[tauri::command]
pub fn take_pending_new_recording(state: State<'_, AppState>) -> bool {
    state
        .pending_new_recording
        .swap(false, std::sync::atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn peek_recast_project(path: String) -> AppResult<ProjectMetadata> {
    Ok(peek_recast_project_inner(&PathBuf::from(&path))?)
}

fn peek_recast_project_inner(path: &std::path::Path) -> anyhow::Result<ProjectMetadata> {
    // File::open distinguishes not-found from permission-denied, which the frontend's toast surfaces verbatim.
    let file = File::open(path)?;
    let mut archive = ZipArchive::new(file)?;
    let mut entry = archive.by_name("metadata.json")?;
    let mut bytes = Vec::with_capacity(2048);
    std::io::Read::read_to_end(&mut entry, &mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}

#[tauri::command]
pub fn is_recording_active() -> bool {
    tray::is_recording_active()
}

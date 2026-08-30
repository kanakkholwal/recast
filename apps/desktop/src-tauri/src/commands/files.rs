//! IPC commands for the file-association path.
//!
//! Three commands cooperate to deliver a `.recast` file the OS handed us
//! into a fresh editor window:
//!
//! * `take_pending_open_file` — drains the path stashed in `AppState` from
//!   the cold-start argv (see `lib.rs::run`). Called by the main window on
//!   mount; subsequent calls return `None`.
//! * `peek_recast_project` — opens the zip and reads ONLY the
//!   `metadata.json` entry (~5ms). Lets the frontend validate before
//!   committing to navigate — bad file ⇒ toast, no broken editor window.
//! * `is_recording_active` — guard so we never spawn an editor window
//!   (which kicks off FFmpeg thumbnail probes) while capture is live.

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

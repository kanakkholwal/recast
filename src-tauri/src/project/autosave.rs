use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::Result;
use serde::{Deserialize, Serialize};

/// Metadata about an autosaved editing session, written alongside the project.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutosaveState {
    /// Path to the original .recast project file being edited.
    pub project_path: String,
    /// Timestamp of the last autosave.
    pub saved_at_unix_ms: u64,
    /// The current render/edit state as JSON.
    pub edits_json: String,
}

/// Directory where autosave state files are stored.
fn autosave_dir() -> PathBuf {
    env::temp_dir().join("recast-autosave")
}

/// Compute a stable filename for the autosave state of a given project.
fn autosave_filename(project_path: &Path) -> String {
    // Use a hash of the absolute path to avoid collisions.
    let path_str = project_path.to_string_lossy();
    let hash = simple_hash(&path_str);
    format!("autosave-{hash:016x}.json")
}

/// Save the current editing state for crash recovery.
/// Called periodically (e.g., every 30 seconds) during editing.
pub fn save_autosave(project_path: &Path, edits_json: &str) -> Result<()> {
    let dir = autosave_dir();
    fs::create_dir_all(&dir)?;

    let state = AutosaveState {
        project_path: project_path.to_string_lossy().to_string(),
        saved_at_unix_ms: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64,
        edits_json: edits_json.to_string(),
    };

    let filename = autosave_filename(project_path);
    let save_path = dir.join(&filename);
    let temp_path = dir.join(format!("{filename}.tmp"));

    // Atomic write: temp file → rename.
    fs::write(&temp_path, serde_json::to_string_pretty(&state)?)?;
    if save_path.exists() {
        let _ = fs::remove_file(&save_path);
    }
    fs::rename(&temp_path, &save_path)?;
    Ok(())
}

/// Remove the autosave state for a project (called after a successful save).
pub fn clear_autosave(project_path: &Path) {
    let dir = autosave_dir();
    let filename = autosave_filename(project_path);
    let save_path = dir.join(filename);
    let _ = fs::remove_file(save_path);
}

/// Check for any autosaved sessions that can be recovered.
/// Returns a list of recoverable sessions.
pub fn find_recoverable_sessions() -> Vec<AutosaveState> {
    let dir = autosave_dir();
    let entries = match fs::read_dir(&dir) {
        Ok(entries) => entries,
        Err(_) => return Vec::new(),
    };

    let mut sessions = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        if let Ok(data) = fs::read_to_string(&path) {
            if let Ok(state) = serde_json::from_str::<AutosaveState>(&data) {
                // Only include sessions whose project file still exists.
                if Path::new(&state.project_path).exists() {
                    sessions.push(state);
                } else {
                    // Project file gone — clean up stale autosave.
                    let _ = fs::remove_file(&path);
                }
            }
        }
    }

    sessions
}

/// Detect and clean up incomplete recording sessions (temp files left behind).
pub fn cleanup_stale_sessions(output_dir: &Path) {
    if let Ok(entries) = fs::read_dir(output_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();

            // Clean up leftover temp files from atomic writes.
            if name.ends_with(".recast.tmp") {
                log::info!("cleaning up stale temp file: {}", path.display());
                let _ = fs::remove_file(&path);
            }

            // Clean up orphaned session files (recording artifacts not packaged).
            if name.contains("recast-session-")
                && (name.ends_with(".recording.mp4")
                    || name.ends_with(".cursor.json")
                    || name.ends_with(".audio.wav"))
            {
                log::info!("cleaning up orphaned session artifact: {}", path.display());
                let _ = fs::remove_file(&path);
            }
        }
    }
}

/// Simple non-cryptographic hash for path → filename mapping.
fn simple_hash(s: &str) -> u64 {
    let mut hash: u64 = 5381;
    for byte in s.bytes() {
        hash = hash.wrapping_mul(33).wrapping_add(byte as u64);
    }
    hash
}

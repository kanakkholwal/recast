//! The backend-owned capture intent: the staged selection for the next recording, mutated by the CLI and read by `rec start`.
//! Every edit broadcasts `capture-intent:changed` so subscribers stay in sync from one source.

use tauri::{AppHandle, Emitter, Manager};

use super::types::{AppState, CaptureIntent};

/// Emitted with the new `CaptureIntent` whenever it changes.
pub const INTENT_CHANGED_EVENT: &str = "capture-intent:changed";

/// A snapshot of the current intent.
pub fn get_intent(app: &AppHandle) -> CaptureIntent {
    app.state::<AppState>().capture_intent.read().clone()
}

/// Apply `f` to the intent under the write lock, then broadcast the new value.
/// Returns the updated snapshot.
pub fn update_intent<F: FnOnce(&mut CaptureIntent)>(app: &AppHandle, f: F) -> CaptureIntent {
    let state = app.state::<AppState>();
    let next = {
        let mut guard = state.capture_intent.write();
        f(&mut guard);
        guard.clone()
    };
    let _ = app.emit(INTENT_CHANGED_EVENT, &next);
    next
}

#[tauri::command]
pub fn get_capture_intent(app: AppHandle) -> CaptureIntent {
    get_intent(&app)
}

#[tauri::command]
pub fn set_capture_intent(app: AppHandle, intent: CaptureIntent) -> CaptureIntent {
    update_intent(&app, |i| *i = intent)
}

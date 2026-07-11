use serde::Serialize;
use tauri::{AppHandle, Emitter};

const EXPORT_STATE_EVENT: &str = "export-state";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ExportStateEvent {
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
    pub(crate) fn base(export_id: &str, status: &'static str) -> Self {
        Self {
            export_id: export_id.to_string(),
            status,
            progress: None,
            path: None,
            message: None,
            detail: None,
        }
    }

    pub(crate) fn started(export_id: &str) -> Self {
        Self::base(export_id, "started")
    }

    /// A named sub-step of the prep phase (before the encode drives real %).
    pub(crate) fn preparing(export_id: &str, detail: &str) -> Self {
        Self {
            detail: Some(detail.to_string()),
            ..Self::base(export_id, "preparing")
        }
    }

    pub(crate) fn progress(export_id: &str, progress: f64) -> Self {
        Self {
            progress: Some(progress),
            ..Self::base(export_id, "progress")
        }
    }

    pub(crate) fn finalizing(export_id: &str) -> Self {
        Self::base(export_id, "finalizing")
    }

    pub(crate) fn success(export_id: &str, path: &str) -> Self {
        Self {
            path: Some(path.to_string()),
            ..Self::base(export_id, "success")
        }
    }

    pub(crate) fn cancelled(export_id: &str) -> Self {
        Self::base(export_id, "cancelled")
    }

    pub(crate) fn error(export_id: &str, message: &str) -> Self {
        Self {
            message: Some(message.to_string()),
            ..Self::base(export_id, "error")
        }
    }
}

pub(crate) fn emit_export_state(app: &AppHandle, event: ExportStateEvent) {
    let _ = app.emit(EXPORT_STATE_EVENT, event);
}

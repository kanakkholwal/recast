use serde::{Deserialize, Serialize};

use crate::recording::{CaptureTarget, RecordingStats};

pub mod autosave;
pub mod reader;
pub mod storage;
pub mod writer;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectMetadata {
    pub schema_version: u32,
    pub created_at_unix_ms: u64,
    pub capture_target: CaptureTarget,
    pub stats: RecordingStats,
    pub video: ProjectVideoMetadata,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectVideoMetadata {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_ms: u64,
}

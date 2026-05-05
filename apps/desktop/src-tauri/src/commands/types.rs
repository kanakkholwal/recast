use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::render::graph::RenderState;

pub const THUMBNAIL_WIDTH: u32 = 320;
pub const THUMBNAIL_HEIGHT: u32 = 180;

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DisplayInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_primary: bool,
    pub thumbnail: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WindowInfo {
    pub id: u32,
    pub pid: u32,
    pub app_name: String,
    pub title: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub is_minimized: bool,
    pub thumbnail: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordingEntry {
    pub filename: String,
    pub path: String,
    pub size_bytes: u64,
    pub created: u64,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct VideoMetadata {
    pub duration: f64,
    pub width: u32,
    pub height: u32,
    pub fps: f64,
    pub codec: String,
    pub size_bytes: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorDocument {
    pub project_path: String,
    pub media_path: String,
    pub cursor_path: Option<String>,
    pub edits_path: Option<String>,
    pub audio_path: Option<String>,
    pub microphone_path: Option<String>,
    pub camera_path: Option<String>,
    pub metadata: VideoMetadata,
    pub render_state: RenderState,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct RecordingStartResult {
    pub warnings: Vec<String>,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct CameraDeviceInfo {
    pub id: String,
    pub name: String,
    pub status: String,
    pub status_message: Option<String>,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CameraValidationResult {
    pub id: String,
    pub name: String,
    pub status: String,
    pub status_message: Option<String>,
    pub probed_at_unix_ms: u64,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct LastSource {
    /// "monitor", "window", or "region"
    pub kind: String,
    pub id: u32,
    pub label: String,
    /// Present for region selections; virtual desktop coords.
    pub region_x: Option<i32>,
    pub region_y: Option<i32>,
    pub region_width: Option<u32>,
    pub region_height: Option<u32>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub output_dir: Option<String>,
    #[serde(default)]
    pub last_source: Option<LastSource>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GifSettings {
    /// Override frame rate. `None` means use the quality profile's `gif_fps`.
    #[serde(default)]
    pub fps: Option<u32>,
    /// "low" | "medium" | "high" — drives palette size + dither bias.
    #[serde(default = "default_gif_quality")]
    pub quality: String,
    /// "infinite" | "once" | a non-negative integer count.
    #[serde(default = "default_gif_loop")]
    pub r#loop: serde_json::Value,
    /// "bayer" | "sierra2" | "none".
    #[serde(default = "default_gif_dither")]
    pub dither: String,
}

fn default_gif_quality() -> String { "medium".into() }
fn default_gif_loop() -> serde_json::Value { serde_json::Value::String("infinite".into()) }
fn default_gif_dither() -> String { "bayer".into() }

impl Default for GifSettings {
    fn default() -> Self {
        Self {
            fps: None,
            quality: default_gif_quality(),
            r#loop: default_gif_loop(),
            dither: default_gif_dither(),
        }
    }
}

impl GifSettings {
    /// Resolve the FFmpeg `-loop` argument. `0` = infinite, `-1` = play once, `n` = play n times.
    pub fn ffmpeg_loop_arg(&self) -> i64 {
        match &self.r#loop {
            serde_json::Value::String(s) if s == "infinite" => 0,
            serde_json::Value::String(s) if s == "once" => -1,
            serde_json::Value::Number(n) => n.as_i64().unwrap_or(0).max(-1),
            _ => 0,
        }
    }

    /// Maximum colours in the generated palette. Caps at 256 (GIF limit).
    pub fn max_colors(&self) -> u32 {
        match self.quality.as_str() {
            "low" => 64,
            "high" => 256,
            _ => 128, // "medium"
        }
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub export_id: String,
    pub input_path: String,
    pub format: String,
    pub quality: String,
    pub render_state: RenderState,
    #[serde(default)]
    pub gif_settings: Option<GifSettings>,
}

#[derive(Clone, Copy)]
pub struct ExportProfile {
    pub max_width: Option<u32>,
    pub max_height: Option<u32>,
    pub mp4_crf: u32,
    pub mp4_preset: &'static str,
    pub mp4_nvenc_cq: u32,
    pub webm_crf: u32,
    pub gif_fps: u32,
}

pub struct AppState {
    pub recording_manager: crate::recording::RecordingManager,
    pub last_file_path: parking_lot::Mutex<Option<String>>,
    pub config: parking_lot::Mutex<AppConfig>,
    /// Per-run cancellation tokens for active exports, keyed by export session id.
    /// `export_video` inserts a fresh `Arc<AtomicBool>` on entry and removes it on
    /// exit; `cancel_export` looks up a specific session and flips only that flag.
    pub export_cancel: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

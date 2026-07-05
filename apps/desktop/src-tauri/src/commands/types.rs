use std::collections::HashMap;
use std::path::PathBuf;
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
    /// Monitor refresh rate in Hz (rounded), e.g. 60 / 120 / 144. The capture
    /// pipeline can't deliver more unique frames per second than this, so the
    /// recording UI uses it to gate the offered frame-rate options. 0 when the
    /// platform couldn't report it (UI then falls back to 60).
    pub refresh_hz: u32,
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
    /// `.recast` only: a legacy v1 bundle the editor must migrate first.
    /// Detected from the ZIP central directory, no extraction.
    pub needs_migration: bool,
}

#[derive(Serialize, Deserialize, Clone, Default)]
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
    /// True when a legacy bundle must be migrated before the editor loads it.
    pub needs_migration: bool,
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

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AppConfig {
    pub output_dir: Option<String>,
    #[serde(default)]
    pub last_source: Option<LastSource>,
    /// When true, closing the main window hides it to the system tray instead
    /// of exiting. The tray's "Quit Recast" item is the canonical exit. Users
    /// who don't want background tray presence can flip this off in Settings.
    #[serde(default = "default_close_to_tray")]
    pub close_to_tray: bool,
    /// When true, the floating recording panel is excluded from screen capture
    /// (Windows `WDA_EXCLUDEFROMCAPTURE`, macOS `NSWindow.sharingType = .none`)
    /// so Recast's own controls don't appear in the recorded video. Default on.
    /// No effect on Linux, which has no per-window exclusion API.
    #[serde(default = "default_hide_panel_from_capture")]
    pub hide_panel_from_capture: bool,
    /// Telemetry consent, mirrored from the frontend `consent.svelte.ts` store
    /// so the native crash reporter (`telemetry.rs`) can read it without IPC.
    ///
    /// `telemetry_product` (behaviour analytics) is strictly opt-in — default
    /// false. `telemetry_errors` (crash reporting) is default opt-in — default
    /// true. `install_id` is the anonymous `distinct_id` shared with JS events.
    #[serde(default)]
    pub telemetry_product: bool,
    #[serde(default = "default_telemetry_errors")]
    pub telemetry_errors: bool,
    #[serde(default)]
    pub install_id: Option<String>,
    /// Self-hosting override for the Recast Cloud API base URL. `None` (the
    /// default) means "use the bundled default endpoint". Set by self-hosters
    /// in Settings → Cloud; validated to an absolute http(s) URL before it's
    /// stored, and the resolver (`auth::cloud_api_url`) falls back to the
    /// default if it's ever absent or malformed.
    #[serde(default)]
    pub cloud_api_url: Option<String>,
    /// Opt-in verbose diagnostic logging. Off by default: release builds log
    /// only warnings/errors. When the user flips this on in Settings →
    /// Diagnostics, the runtime log level drops to Debug so backend processing
    /// and editor-interaction logs (forwarded from the webview) are captured in
    /// the rotating log file for a support bundle. See `apply_log_level`.
    #[serde(default)]
    pub diagnostic_logging: bool,
}

fn default_close_to_tray() -> bool {
    true
}

fn default_hide_panel_from_capture() -> bool {
    true
}

fn default_telemetry_errors() -> bool {
    true
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            output_dir: None,
            last_source: None,
            close_to_tray: true,
            hide_panel_from_capture: true,
            telemetry_product: false,
            telemetry_errors: true,
            install_id: None,
            cloud_api_url: None,
            diagnostic_logging: false,
        }
    }
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

fn default_gif_quality() -> String {
    "medium".into()
}
fn default_gif_loop() -> serde_json::Value {
    serde_json::Value::String("infinite".into())
}
fn default_gif_dither() -> String {
    "bayer".into()
}

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
    /// Encoder *effort* axis ("fast" | "balanced" | "quality"), orthogonal to
    /// `quality` (which is resolution/CRF). Absent/unknown → "balanced", which
    /// reproduces the historical encoder settings exactly.
    #[serde(default)]
    pub speed: Option<String>,
    pub render_state: RenderState,
    #[serde(default)]
    pub gif_settings: Option<GifSettings>,
    /// Output frame rate for MP4/WebM. `None` keeps the source recording's rate
    /// (the quality-preserving default). Only values ≤ source are offered in the
    /// UI, so the export never duplicates frames. GIF ignores this and uses
    /// `gif_settings.fps`.
    #[serde(default)]
    pub fps: Option<f64>,
    /// Burn the generated captions into the video (overlay). The transcript +
    /// style come from the render state's `transcript`/`captionStyle`
    /// passthrough; this is a no-op when there's no transcript. Ignored for GIF.
    #[serde(default)]
    pub burn_captions: bool,
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
    // `Arc` so `stop_recording` can hand an owned handle to a `spawn_blocking`
    // worker. `stop()` joins the encoder/capture threads and (with a paused
    // camera) runs a 30s+ FFmpeg re-encode; doing that on Tauri's main thread
    // froze the macOS WebView until it finished. All other callers reach the
    // manager through `Deref`, so the `Arc` is transparent to them.
    pub recording_manager: Arc<crate::recording::RecordingManager>,
    pub last_file_path: parking_lot::Mutex<Option<String>>,
    /// Read-mostly (output dir, tray pref, telemetry consent, cloud URL are read
    /// on hot/concurrent paths; written only when the user changes a setting), so
    /// a `RwLock` lets concurrent readers proceed without serializing. Writers
    /// must mutate, snapshot, then drop the guard BEFORE calling `save_config` —
    /// never hold the lock across the disk write.
    pub config: parking_lot::RwLock<AppConfig>,
    /// Per-run cancellation tokens for active exports, keyed by export session id.
    /// `export_video` inserts a fresh `Arc<AtomicBool>` on entry and removes it on
    /// exit; `cancel_export` looks up a specific session and flips only that flag.
    pub export_cancel: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// JoinHandle for the in-flight device-authorization poller. `auth_start`
    /// replaces this; `auth_cancel` aborts it. Holding the handle (vs. an
    /// `AbortHandle`) lets us also `await` it later for graceful shutdown if
    /// we ever need it — for cancellation the handle's `abort()` method is
    /// enough. Only one poller can be live at a time: `auth_start` rejects
    /// when this is `Some`.
    pub auth_poller: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    /// `.recast` file path the OS handed us via argv on cold start. The
    /// frontend drains this on main-window mount with `take_pending_open_file`
    /// and routes to a new editor window. `None` after drain — warm-start
    /// opens go through the `app://open-recast` event instead.
    pub pending_open_file: Mutex<Option<PathBuf>>,
}

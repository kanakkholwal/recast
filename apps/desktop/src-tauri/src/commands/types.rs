use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri_plugin_global_shortcut::Shortcut;

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
    /// Birth time (fs creation time) in epoch seconds. Falls back to `modified`
    /// on filesystems/platforms where birth time isn't reported (e.g. ext4).
    /// Used to label the recording date in the library.
    pub created: u64,
    /// Last-modified time in epoch seconds. Drives "what was I last editing"
    /// surfaces like the library's Continue card — birth time can be stale if
    /// the file was only touched (e.g. a thumbnail regen) after recording.
    pub modified: u64,
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
    /// Translucent window backdrop (Win11 Mica/Acrylic, macOS vibrancy). Off by
    /// default; solid on Win10 and unsupported GPUs regardless.
    #[serde(default)]
    pub window_transparency: bool,
    /// Whether `setup()` should attempt to install the `recast` CLI on the
    /// user's PATH on first launch. Default `true` — most users want the CLI
    /// ready to drive Recast from a terminal or an AI agent. The settings
    /// panel exposes an explicit toggle so a user who *removed* it can also
    /// disable the auto-attempt.
    #[serde(default = "default_cli_auto_install")]
    pub cli_auto_install: bool,
    /// Whether we've already attempted the first-launch auto-install. We
    /// only attempt once per app install (per user), so a successful install
    /// is sticky, and an install that errored once doesn't loop forever.
    #[serde(default)]
    pub cli_install_attempted: bool,
}

fn default_cli_auto_install() -> bool {
    true
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
            window_transparency: false,
            cli_auto_install: true,
            cli_install_attempted: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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

/// A subtitle sidecar to write next to a successful export, on the OUTPUT
/// timeline (trim + cuts + speed already applied by the frontend). Written by the
/// export worker after the encode so it survives closing the source editor; the
/// frontend used to write it after the invoke resolved.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptionSidecar {
    /// "vtt" | "srt".
    pub format: String,
    pub transcript: crate::transcription::Transcript,
}

// Serialize as well as Deserialize: the export queue persists the whole request
// (render state included) to a payload file on disk so a queued job can run after
// its editor is closed and survive an app restart. Heavy but self-contained.
#[derive(Serialize, Deserialize, Clone)]
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
    /// Optional subtitle sidecar to write next to the export on success. `None`
    /// when the user chose no sidecar or there is no transcript.
    #[serde(default)]
    pub caption_sidecar: Option<CaptionSidecar>,
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

/// Backend-owned staging config for the next recording. The CLI mutates it
/// (`select`/`set`), `rec start` without explicit flags uses it, and the panel
/// renders it (Phase 3b). Maps directly onto `start_recording`'s arguments.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default, rename_all = "camelCase")]
pub struct CaptureIntent {
    /// "display" | "window" | "region". None until a source is chosen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_type: Option<String>,
    pub target_id: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<crate::recording::RegionRect>,
    pub options: crate::recording::RecordingOptions,
    /// Pre-roll seconds. The countdown is a frontend concern, stored here so the
    /// panel and CLI agree. None inherits the global default.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub countdown: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub active_profile_id: Option<String>,
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
    /// Display/system-sleep inhibitor. Recording holds it across start→stop;
    /// `export_video` takes a scoped lease. See `crate::power`.
    pub power: crate::power::PowerManager,
    /// Set when the app was launched via the jump list's "New Recording" task
    /// (`--new-recording` in argv). The main window drains it on mount and opens
    /// the recording panel. Warm-start launches use the single-instance event.
    pub pending_new_recording: AtomicBool,
    /// Backend-owned selection for the next recording. Mutated by the CLI
    /// (`select`/`set`) and, in Phase 3b, the panel; `capture-intent:changed`
    /// broadcasts edits. RwLock: read at start, written on each tweak.
    pub capture_intent: parking_lot::RwLock<CaptureIntent>,
    /// Backend-owned recording profiles, shared by the panel, profile picker,
    /// and CLI (`recast profile list/use`). Persisted to `recast_profiles.json`;
    /// `recording-profiles:changed` broadcasts edits. See `commands::profiles`.
    pub profiles: parking_lot::RwLock<crate::commands::profiles::ProfilesState>,
    /// False while `profiles` is the ephemeral in-memory seed (no profiles file
    /// yet). The frontend reads this to migrate its `localStorage` profiles into
    /// the backend exactly once; `set_profiles` flips it true.
    pub profiles_initialized: AtomicBool,
    /// Embedded local store (SQLite). Backs the export queue and, later, the
    /// recordings/exports index. See `crate::db`.
    pub db: crate::db::Db,
    /// Wakes the serial export worker whenever the queue changes (enqueue, cancel
    /// of a queued item, retry). The worker `await`s this, then drains all queued
    /// jobs one at a time. See `commands::export_queue`.
    pub export_wake: Arc<tokio::sync::Notify>,
    /// OS-wide hotkeys registered by the global-shortcut plugin in `setup()`.
    /// Stored so the `run` block can unregister each on `RunEvent::Exit` /
    /// `ExitRequested` — otherwise an unclean close (crash, force-kill from
    /// Task Manager, dev/prod coexistence where one instance is killed while
    /// another holds the slot) leaves the OS-level hotkey bound to a dead
    /// process and the next launch logs `HotKey already registered` for the
    /// lifetime of the OS. See `lib.rs::run`.
    pub registered_shortcuts: Mutex<Vec<Shortcut>>,
    /// Per-project editor write-lock. The CLI agent and the GUI user both go
    /// through here so one of them holds the project at a time; the other sees
    /// a structured `editor_locked` error (CLI) or a banner+disabled mutators
    /// (GUI). See `commands::editor_session` for the helpers. Initialised empty
    /// in `setup()`; `commands::editor_session::load_on_startup` may revive a
    /// session from `recast_session.json` if the previous holder's PID is
    /// still alive.
    pub editor_session: parking_lot::RwLock<EditorSession>,
}

/// Who currently holds the editor write-lock, if anyone. Either side (UI,
/// agent) goes through the same `EditorSession::try_acquire_write` API; the
/// discriminator exists so the GUI can render "Agent `claude-…` is editing"
/// specifically vs "you are editing" (the GUI is implicitly its own holder
/// once it opens a project).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EditorWriterKind {
    Ui,
    Agent,
}

/// Editor write-lock state. Lives in `AppState.editor_session`. The mutating
/// helpers in `commands::editor_session` are the only legitimate writers;
/// readers (the GUI's derived `isWriteLockedByAgent`) just take a shared read.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EditorSession {
    /// `None` when idle. Set to the locked project's absolute path while held.
    pub project_path: Option<PathBuf>,
    /// `None` when idle.
    pub writer: Option<EditorWriterKind>,
    /// Free-form identifier: `"ui:<user>"` for the GUI, `"agent:<id>"` (the
    /// agent's session ID) for the CLI. Surfaced verbatim in the
    /// `editor_locked` error so the other side can name the holder.
    pub writer_id: String,
    /// Unix-epoch ms when the lock was acquired.
    pub acquired_at_ms: i64,
    /// Unix-epoch ms of the last mutation under this lock. Bumped by every
    /// `try_acquire_write` / `record_activity`. If `now - last_activity_at_ms
    /// &gt; TTL_MS`, the next acquire reclaims the lock (covers a crashed
    /// holder without leaving the project stranded forever).
    pub last_activity_at_ms: i64,
}

impl EditorSession {
    /// Inactivity window after which the lock is reclaimable. A long
    /// `recast editor patch` over a multi-MB `RenderState` should keep the
    /// activity stamp fresh (`commands::editor_session::record_activity`);
    /// the TTL is the safety net for a crashed CLI/GUI that never released.
    pub const TTL_MS: i64 = 60_000;
}

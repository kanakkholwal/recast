use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};

use super::error::{AppError, AppResult};
use super::ffmpeg::{encode_thumbnail_base64, make_thumbnail};
use serde::Serialize;

use super::types::{
    AppConfig, AppState, CameraDeviceInfo, CameraValidationResult, DisplayInfo, LastSource,
    WindowInfo,
};

fn config_path(app: &AppHandle) -> PathBuf {
    let dir = match app.path().app_data_dir() {
        Ok(dir) => dir,
        Err(e) => {
            // `%TEMP%` is periodically purged, so settings (telemetry consent, install_id) won't survive; log it so resets are diagnosable.
            log::warn!(
                "app_data_dir unavailable ({e}); using temp dir for config — \
                 settings may not persist between sessions"
            );
            env::temp_dir()
        }
    };
    dir.join("recast_config.json")
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = config_path(app);
    match fs::read_to_string(&path) {
        Ok(data) => match serde_json::from_str(&data) {
            Ok(config) => config,
            Err(e) => {
                // A genuine parse failure, not 'no file yet': resetting silently would wipe settings and flip telemetry back on, so back the file up.
                log::warn!(
                    "config at {} is unreadable ({e}); backing up to .bak and \
                     resetting to defaults",
                    path.display()
                );
                let _ = fs::rename(&path, path.with_extension("json.bak"));
                AppConfig::default()
            }
        },
        // First run / no file — the expected case, stay quiet.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => AppConfig::default(),
        Err(e) => {
            log::warn!(
                "failed to read config at {} ({e}); using defaults",
                path.display()
            );
            AppConfig::default()
        }
    }
}

pub(crate) fn save_config(app: &AppHandle, config: &AppConfig) {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::warn!("failed to create config dir {}: {e}", parent.display());
            return;
        }
    }
    let data = match serde_json::to_string_pretty(config) {
        Ok(data) => data,
        Err(e) => {
            log::error!("failed to serialize config: {e}");
            return;
        }
    };
    // Atomic write: `fs::write` truncates first, so a crash mid-write leaves a corrupt file the next launch discards.
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = write_atomic(&tmp, &path, data.as_bytes()) {
        log::warn!("failed to persist config to {}: {e}", path.display());
        let _ = fs::remove_file(&tmp);
    }
}

pub(crate) fn write_atomic(tmp: &Path, dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::File::create(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, dest)
}

/// Temp + rename for derived state written from async code (lockfiles, toggle
/// state). No fsync: these are rebuildable, so the truncate window is the only
/// thing worth closing.
pub(crate) async fn write_replace_async(dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = dest.with_extension("json.tmp");
    if let Err(e) = tokio::fs::write(&tmp, bytes).await {
        let _ = tokio::fs::remove_file(&tmp).await;
        return Err(e);
    }
    match tokio::fs::rename(&tmp, dest).await {
        Ok(()) => Ok(()),
        Err(e) => {
            let _ = tokio::fs::remove_file(&tmp).await;
            Err(e)
        }
    }
}

/// Read a JSON manifest, quarantining an unparseable one to `*.corrupt.json`.
/// Callers write the value straight back, so silently defaulting on a parse
/// error would make one torn write permanently destroy every record it held.
pub(crate) fn read_json_manifest<T: serde::de::DeserializeOwned + Default>(path: &Path) -> T {
    let Ok(data) = fs::read_to_string(path) else {
        return T::default();
    };
    match serde_json::from_str(&data) {
        Ok(value) => value,
        Err(e) => {
            let quarantine = path.with_extension("corrupt.json");
            log::error!(
                "manifest {} is unreadable ({e}); moved to {} and starting empty",
                path.display(),
                quarantine.display()
            );
            let _ = fs::rename(path, &quarantine);
            T::default()
        }
    }
}

/// Atomic counterpart to [`read_json_manifest`].
pub(crate) fn write_json_manifest<T: serde::Serialize>(path: &Path, value: &T) {
    if let Some(parent) = path.parent() {
        if let Err(e) = fs::create_dir_all(parent) {
            log::warn!("failed to create manifest dir {}: {e}", parent.display());
            return;
        }
    }
    let data = match serde_json::to_vec_pretty(value) {
        Ok(data) => data,
        Err(e) => {
            log::error!("failed to serialize manifest {}: {e}", path.display());
            return;
        }
    };
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = write_atomic(&tmp, path, &data) {
        log::warn!("failed to persist manifest {}: {e}", path.display());
        let _ = fs::remove_file(&tmp);
    }
}

pub fn get_active_output_dir(state: &State<'_, AppState>) -> PathBuf {
    let config = state.config.read();
    if let Some(dir) = &config.output_dir {
        PathBuf::from(dir)
    } else {
        env::temp_dir()
    }
}

/// First-run default output location: a `Recast` folder in the OS video
/// directory (Videos on Windows/Linux, Movies on macOS), so recordings land
/// somewhere discoverable and durable rather than the purged temp dir. Falls
/// back to Documents, then Home, then temp if the OS can't report a video dir.
pub fn default_output_dir(app: &AppHandle) -> PathBuf {
    let base = app
        .path()
        .video_dir()
        .or_else(|_| app.path().document_dir())
        .or_else(|_| app.path().home_dir())
        .unwrap_or_else(|_| env::temp_dir());
    base.join("Recast")
}

/// True on Linux + Wayland. xcap's per-source `capture_image()` triggers
/// an `xdg-desktop-portal.ScreenCast` permission dialog *per source* on
/// Wayland — calling it across every monitor/window during the picker hot
/// path raises N consecutive dialogs and can stall the calling thread for
/// seconds while the user dismisses each one. We deliberately skip the
/// thumbnail entirely in that case; the picker remains usable from text
/// labels alone, and we'll revisit this once we wire PipeWire directly
/// (see `apps/desktop/docs/linux-native-recording.md` once written).
fn is_wayland() -> bool {
    cfg!(target_os = "linux") && std::env::var_os("WAYLAND_DISPLAY").is_some()
}

/// A picker thumbnail, or `None` where taking one would be intrusive or fail.
///
/// Skipped under Wayland, where every capture is a portal dialog and a picker
/// full of thumbnails would be a picker full of prompts.
fn capture_thumbnail(target: capturekit::Target) -> Option<String> {
    if is_wayland() {
        return None;
    }
    let shot = crate::capture::thumbnail(target).ok()?;
    encode_thumbnail_base64(&make_thumbnail(&shot))
}

#[tauri::command]
pub fn get_output_dir(state: State<'_, AppState>) -> AppResult<String> {
    Ok(get_active_output_dir(&state).to_string_lossy().to_string())
}

#[tauri::command]
pub fn set_output_dir(app: AppHandle, state: State<'_, AppState>, path: String) -> AppResult<()> {
    if !Path::new(&path).exists() {
        return Err(AppError::from("Directory does not exist"));
    }
    let snapshot = {
        let mut config = state.config.write();
        config.output_dir = Some(path);
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

#[tauri::command]
pub fn get_last_source(state: State<'_, AppState>) -> AppResult<Option<LastSource>> {
    Ok(state.config.read().last_source.clone())
}

#[tauri::command]
pub fn get_close_to_tray(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.config.read().close_to_tray)
}

#[tauri::command]
pub fn set_close_to_tray(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.close_to_tray = enabled;
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

/// Whether `setup()` should attempt to install the `recast` CLI on the
/// user's PATH on first launch. The settings toggle mirrors this — flipping
/// it off stops future auto-installs; the user can still install manually
/// via the Install button beside it.
#[tauri::command]
pub fn get_cli_auto_install(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.config.read().cli_auto_install)
}

#[tauri::command]
pub fn set_cli_auto_install(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.cli_auto_install = enabled;
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

#[tauri::command]
pub fn get_window_transparency(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.config.read().window_transparency)
}

#[tauri::command]
pub fn set_window_transparency(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.window_transparency = enabled;
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

/// Whether the FFmpeg-free GPU writer is enabled. `native_encoder_available`
/// reports whether this machine could honour it, so the UI can disable the
/// toggle rather than offering a switch that silently does nothing.
#[tauri::command]
pub fn get_native_encoder(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.config.read().native_encoder)
}

#[tauri::command]
pub fn set_native_encoder(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.native_encoder = enabled;
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

/// Whether this machine has what the native writer needs. Windows plus a Media
/// Foundation H.264 encoder; anywhere else the answer is a flat no.
#[tauri::command]
pub fn native_encoder_available() -> bool {
    #[cfg(windows)]
    {
        crate::encoder::native::available()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

#[tauri::command]
pub fn get_hide_panel_from_capture(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.config.read().hide_panel_from_capture)
}

/// Async on purpose: after persisting, it reflects the change on a *live*
/// recording panel so the toggle is immediate rather than "next launch". The
/// `set_content_protected` round-trip would deadlock the macOS main thread if
/// this ran as a sync command (those run on the main thread) — see
/// `exclude_window_from_capture`.
#[tauri::command]
pub async fn set_hide_panel_from_capture(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.hide_panel_from_capture = enabled;
        config.clone()
    };
    save_config(&app, &snapshot);
    // `set_content_protected` toggles both directions and the compositor honors it next frame, so a mid-recording flip applies at once.
    if let Some(panel) = app.get_webview_window("recording-panel") {
        panel
            .set_content_protected(enabled)
            .map_err(|e| AppError::msg(format!("panel content-protection toggle failed: {e}")))?;
    }
    Ok(())
}

/// Mirror the frontend telemetry-consent state into `AppConfig` so the native
/// crash reporter (`telemetry.rs`) can read the `errors` flag and attribute
/// crashes to the same anonymous `install_id` as JS events. Called from
/// `consent.svelte.ts` whenever a toggle flips or on first-run dismissal.
#[tauri::command]
pub fn set_telemetry_consent(
    app: AppHandle,
    state: State<'_, AppState>,
    product: bool,
    errors: bool,
    install_id: Option<String>,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.telemetry_product = product;
        config.telemetry_errors = errors;
        if let Some(id) = install_id {
            if !id.is_empty() {
                config.install_id = Some(id);
            }
        }
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

#[tauri::command]
pub fn set_last_source(
    app: AppHandle,
    state: State<'_, AppState>,
    source: LastSource,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.last_source = Some(source);
        config.clone()
    };
    save_config(&app, &snapshot);
    Ok(())
}

/// Apply the runtime log-level filter for the current diagnostic-logging
/// setting. The tauri-plugin-log dispatch is built permissively (Trace), so
/// this `log::set_max_level` is the single gate that decides what actually
/// reaches the rotating file — for the Rust backend AND the webview logs the
/// frontend forwards through the same plugin.
///
/// - off (default) → release builds stay quiet (Warn); debug builds keep Info.
/// - on → Debug everywhere, capturing backend processing + editor-interaction
///   traces for a support bundle.
pub(crate) fn apply_log_level(diagnostic: bool) {
    let level = if diagnostic {
        log::LevelFilter::Debug
    } else if cfg!(debug_assertions) {
        log::LevelFilter::Info
    } else {
        log::LevelFilter::Warn
    };
    log::set_max_level(level);
}

#[tauri::command]
pub fn get_diagnostic_logging(state: State<'_, AppState>) -> AppResult<bool> {
    Ok(state.config.read().diagnostic_logging)
}

/// Toggle verbose diagnostic logging. Persists the choice and re-applies the
/// runtime log level immediately, so a user can enable it, reproduce a bug, and
/// grab the log folder — no restart needed.
#[tauri::command]
pub fn set_diagnostic_logging(
    app: AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> AppResult<()> {
    let snapshot = {
        let mut config = state.config.write();
        config.diagnostic_logging = enabled;
        config.clone()
    };
    save_config(&app, &snapshot);
    apply_log_level(enabled);
    // Logged AFTER raising the level, so the enabled transition is the first line of a fresh diagnostic session.
    log::info!(
        "diagnostic logging {}",
        if enabled { "enabled" } else { "disabled" }
    );
    Ok(())
}

/// Reveal the rotating-log directory in the OS file manager so the user can
/// attach it to a support request. Same dir `tauri_plugin_log` writes to
/// (`app_log_dir`); created if a session hasn't written there yet.
#[tauri::command]
pub fn open_log_dir(app: AppHandle) -> AppResult<String> {
    use tauri::Manager;
    use tauri_plugin_opener::OpenerExt;

    let dir = app
        .path()
        .app_log_dir()
        .map_err(|e| AppError::msg(format!("could not resolve log directory: {e}")))?;
    let _ = std::fs::create_dir_all(&dir);
    let display = dir.to_string_lossy().to_string();
    app.opener()
        .open_path(display.clone(), None::<&str>)
        .map_err(|e| AppError::msg(format!("failed to open log folder: {e}")))?;
    Ok(display)
}

// Enumeration and thumbnails hit the compositor; a sync command would freeze the GTK main thread.
#[tauri::command]
pub async fn get_displays() -> AppResult<Vec<DisplayInfo>> {
    tauri::async_runtime::spawn_blocking(|| -> AppResult<Vec<DisplayInfo>> {
        let displays = capturekit::displays().map_err(|e| e.to_string())?;
        Ok(displays
            .into_iter()
            .map(|display| DisplayInfo {
                id: display.id.0,
                name: display.name,
                x: display.bounds.x,
                y: display.bounds.y,
                width: display.bounds.width,
                height: display.bounds.height,
                is_primary: display.is_primary,
                thumbnail: capture_thumbnail(capturekit::Target::Display(display.id)),
                // The CURRENT rate, not the panel's maximum; 0 if unreported.
                refresh_hz: display.refresh_hz.map_or(0, |hz| hz.round() as u32),
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::msg(format!("get_displays join error: {e}")))?
}

#[tauri::command]
pub async fn get_windows() -> AppResult<Vec<WindowInfo>> {
    tauri::async_runtime::spawn_blocking(|| -> AppResult<Vec<WindowInfo>> {
        let windows = capturekit::windows().map_err(|e| e.to_string())?;
        Ok(windows
            .into_iter()
            .filter(|window| window.is_capturable() && !window.title.is_empty())
            .map(|window| WindowInfo {
                id: window.id.0,
                pid: window.pid,
                app_name: window.app_name,
                title: window.title,
                x: window.bounds.x,
                y: window.bounds.y,
                width: window.bounds.width,
                height: window.bounds.height,
                is_minimized: window.is_minimized,
                thumbnail: capture_thumbnail(capturekit::Target::Window(window.id)),
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::msg(format!("get_windows join error: {e}")))?
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// List available audio input (microphone) devices.
///
/// `async` + `spawn_blocking` for the same reason as `get_camera_devices`:
/// enumeration blocks — Linux spawns `pactl list short sources` and waits on
/// the PulseAudio daemon; macOS spawns FFmpeg on the first (uncached) call;
/// Windows walks the WASAPI endpoint COM API. Tauri runs sync commands on the
/// main thread, which on macOS/Linux also renders the WebView, so a slow audio
/// subsystem would freeze the UI. Push it onto a worker instead.
#[tauri::command]
pub async fn get_audio_devices() -> AppResult<Vec<AudioDeviceInfo>> {
    tauri::async_runtime::spawn_blocking(get_audio_devices_blocking)
        .await
        .map_err(|e| AppError::msg(format!("get_audio_devices join error: {e}")))?
        .map_err(Into::into)
}

/// Every microphone capturekit can name, plus the default row.
///
/// Loopback endpoints are dropped: they are outputs read backwards, and a mic
/// picker offering one records the desktop instead of the speaker.
fn get_audio_devices_blocking() -> Result<Vec<AudioDeviceInfo>, String> {
    if !capturekit::capabilities().audio_device_enumeration {
        return Ok(vec![default_microphone()]);
    }
    let devices = capturekit::audio_devices().map_err(|err| err.to_string())?;
    Ok(microphones(&devices))
}

/// The row for a backend that names no devices, so the picker offers what will
/// actually be captured rather than reading as "no microphone found".
fn default_microphone() -> AudioDeviceInfo {
    AudioDeviceInfo {
        id: "default".to_string(),
        name: "System default".to_string(),
        is_default: true,
    }
}

fn microphones(devices: &[capturekit::AudioDevice]) -> Vec<AudioDeviceInfo> {
    devices
        .iter()
        .filter(|device| device.direction == capturekit::AudioDirection::Input)
        .map(|device| AudioDeviceInfo {
            id: device.id.0.clone(),
            name: if device.name.is_empty() {
                device.id.0.clone()
            } else {
                device.name.clone()
            },
            is_default: device.is_default,
        })
        .collect()
}

/// Mark a Tauri window as excluded from screen capture.
///
/// On Windows this calls `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)`,
/// which tells the OS compositor to render the window to the user but
/// substitute a black box (or skip it entirely on supported APIs) when any
/// process captures the desktop — including DXGI Desktop Duplication, which
/// is what Recast itself uses for screen recording.
///
/// This is the fix for the "I can see my own camera bubble inside the
/// recorded video" bug: the floating webcam preview window we open during
/// recording IS part of the desktop, so without this exclusion DXGI
/// captures its pixels into the screen frame just like any other window.
///
/// Requires Windows 10 v2004+ (build 19041) for `WDA_EXCLUDEFROMCAPTURE`.
/// Older Windows versions silently fall back to `WDA_MONITOR` (renders as
/// a black box rather than excluded entirely) — still better than the
/// preview leaking into the recording.
///
/// Delegates to Tauri/tao's `set_content_protected`, whose per-platform
/// behavior is:
///   - **Windows** — `SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)`,
///     which removes the window entirely from every capture surface (DXGI
///     Desktop Duplication included — the API Recast records with).
///   - **macOS** — `NSWindow.sharingType = .none`. Recast captures the screen
///     through FFmpeg AVFoundation, a compositor-level source that honors
///     `sharingType`, so the window is genuinely absent from the recording.
///     (The macOS-15 ScreenCaptureKit exception that ignores this flag does
///     not apply to an AVFoundation capture.)
///   - **Linux** — compile-time no-op: tao gates the implementation to
///     macOS+Windows (`window.rs`), because neither X11 root `GetImage` nor
///     the PipeWire portal exposes a per-window exclusion primitive. The call
///     is harmless and would start working if tao ever adds a Wayland path.
///
/// Async on purpose: sync Tauri commands run on the macOS main thread, and
/// `set_content_protected` round-trips to the event loop — doing that from the
/// main thread would deadlock. An async command runs off it.
#[tauri::command]
pub async fn exclude_window_from_capture(app: AppHandle, label: String) -> AppResult<()> {
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| AppError::msg(format!("window '{label}' not found")))?;
    window
        .set_content_protected(true)
        .map_err(|e| AppError::msg(format!("content protection failed for '{label}': {e}")))?;
    log::info!("excluded window '{label}' from screen capture");
    Ok(())
}

/// Lock a window's resize to a fixed aspect ratio and cap its width at a
/// fraction of its current monitor.
///
/// On Windows this installs a `WM_SIZING` subclass so the box stays
/// proportional *while dragging* (you can't pull width or height
/// independently) and never exceeds `max_screen_fraction` of the monitor's
/// work-area width. Re-invoke with a new ratio when the aspect changes (e.g.
/// the camera bubble cycling 1:1 → 16:9) — the constraint updates in place.
///
/// No-op on other platforms; callers there keep the JS snap-to-aspect
/// fallback. `min_width_px` and `chrome_px` are in physical pixels (the OS
/// drag rect is too), so callers pass `logical * devicePixelRatio`.
///
/// `chrome_px` is fixed, non-scaling vertical space reserved at the bottom of
/// the window for a control bar that sits *outside* the rounded video — the
/// aspect ratio applies to `height - chrome_px`, so the visible bubble keeps
/// its shape while the window is that much taller. Pass 0 for a video-only
/// window.
#[tauri::command]
pub fn set_window_aspect_ratio(
    app: AppHandle,
    label: String,
    aspect_width: f64,
    aspect_height: f64,
    max_screen_fraction: f64,
    min_width_px: f64,
    chrome_px: f64,
) -> AppResult<()> {
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| AppError::msg(format!("window '{label}' not found")))?;
    let ratio = if aspect_height > 0.0 {
        aspect_width / aspect_height
    } else {
        1.0
    };
    #[cfg(windows)]
    {
        let hwnd = window
            .hwnd()
            .map_err(|e| AppError::msg(format!("hwnd lookup failed for '{label}': {e}")))?;
        crate::window_aspect::apply(
            &app,
            hwnd.0 as isize,
            ratio,
            max_screen_fraction,
            min_width_px.round() as i32,
            chrome_px.round() as i32,
        );
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = (window, ratio, max_screen_fraction, min_width_px, chrome_px);
        Ok(())
    }
}

/// List available camera/video capture devices.
#[tauri::command]
pub async fn get_camera_devices() -> AppResult<Vec<CameraDeviceInfo>> {
    // Enumeration opens every device node, which a slow webcam stalls for ages.
    tauri::async_runtime::spawn_blocking(get_camera_devices_blocking)
        .await
        .map_err(|e| AppError::msg(format!("get_camera_devices join error: {e}")))?
        .map_err(Into::into)
}

fn get_camera_devices_blocking() -> Result<Vec<CameraDeviceInfo>, String> {
    // Id stays the friendly name: it is what a saved profile holds.
    Ok(crate::camera::devices()?
        .into_iter()
        .map(|camera| {
            let (status, status_message) = classify_camera_name(&camera.name);
            CameraDeviceInfo {
                id: camera.name.clone(),
                name: camera.name,
                status,
                status_message,
            }
        })
        .collect())
}

#[tauri::command]
pub async fn validate_camera_source(device_id: String) -> AppResult<CameraValidationResult> {
    tauri::async_runtime::spawn_blocking(move || -> Result<CameraValidationResult, String> {
        let devices = get_camera_devices_blocking()?;
        let probed_at_unix_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        let Some(device) = devices.into_iter().find(|d| d.id == device_id) else {
            return Ok(CameraValidationResult {
                id: device_id.clone(),
                name: device_id,
                status: "error".into(),
                status_message: Some("Camera device is no longer available.".into()),
                probed_at_unix_ms,
            });
        };

        // The deep liveliness probe is Windows-only; AVFoundation and V4L2 have no cheap 'open and grab one frame' equivalent.
        #[cfg(windows)]
        let (status, status_message) = probe_camera_device_health(&device.id)
            .unwrap_or_else(|| (device.status.clone(), device.status_message.clone()));
        #[cfg(not(windows))]
        let (status, status_message) = (device.status.clone(), device.status_message.clone());

        Ok(CameraValidationResult {
            id: device.id,
            name: device.name,
            status,
            status_message,
            probed_at_unix_ms,
        })
    })
    .await
    .map_err(|e| AppError::msg(format!("validate_camera_source join error: {e}")))?
    .map_err(Into::into)
}

fn classify_camera_name(name: &str) -> (String, Option<String>) {
    let normalized = name.to_ascii_lowercase();
    if normalized.contains("droidcam")
        || normalized.contains("epoccam")
        || normalized.contains("nvidia broadcast")
    {
        return (
            "warning".into(),
            Some("Virtual camera source may enumerate but produce no frames.".into()),
        );
    }
    if normalized.contains("obs virtual camera") || normalized.contains("snap camera") {
        return (
            "unknown".into(),
            Some("Virtual camera source requires live validation.".into()),
        );
    }
    ("ready".into(), None)
}

#[cfg(windows)]
fn probe_camera_device_health(device_id: &str) -> Option<(String, Option<String>)> {
    let input = if device_id.starts_with("video=") {
        device_id.to_string()
    } else {
        format!("video={device_id}")
    };

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-f",
        "dshow",
        "-i",
        &input,
        "-frames:v",
        "1",
        "-f",
        "null",
        "-",
    ]);
    crate::ffmpeg::configure_silent_command(&mut command);
    let output = command.output().ok()?;
    let stderr = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();

    if output.status.success() {
        return Some(("ready".into(), None));
    }
    if stderr.contains("device not found")
        || stderr.contains("could not find")
        || stderr.contains("no such file")
    {
        return Some(("error".into(), Some("Camera device was not found.".into())));
    }
    if stderr.contains("busy") || stderr.contains("already in use") {
        return Some((
            "warning".into(),
            Some("Camera appears to be busy or unavailable for capture.".into()),
        ));
    }
    Some((
        "warning".into(),
        Some("Camera probe failed. Preview validation will confirm liveliness.".into()),
    ))
}

#[cfg(test)]
mod microphone_tests {
    use super::{microphones, AudioDeviceInfo};
    use capturekit::{AudioDevice, AudioDeviceId, AudioDirection, AudioFormat, SampleFormat};

    fn device(id: &str, name: &str, direction: AudioDirection, is_default: bool) -> AudioDevice {
        AudioDevice {
            id: AudioDeviceId(id.to_string()),
            name: name.to_string(),
            direction,
            is_default,
            format: AudioFormat::new(48_000, 2, SampleFormat::F32),
        }
    }

    fn listed() -> Vec<AudioDeviceInfo> {
        microphones(&[
            device("speakers", "Speakers", AudioDirection::Loopback, true),
            device("yeti", "Blue Yeti", AudioDirection::Input, true),
            device("nameless", "", AudioDirection::Input, false),
        ])
    }

    /// A loopback endpoint is an output read backwards; offering one in a mic
    /// picker records the desktop instead of the speaker.
    #[test]
    fn loopback_endpoints_are_not_offered_as_microphones() {
        let ids: Vec<_> = listed().into_iter().map(|d| d.id).collect();
        assert_eq!(ids, vec!["yeti", "nameless"]);
    }

    #[test]
    fn the_default_flag_survives_the_filter() {
        let devices = listed();
        assert!(devices[0].is_default);
        assert!(!devices[1].is_default);
    }

    /// PipeWire nodes can have no description; a blank row is unpickable.
    #[test]
    fn a_device_with_no_name_is_listed_under_its_id() {
        assert_eq!(listed()[1].name, "nameless");
    }
}

#[cfg(test)]
mod manifest_tests {
    use super::{read_json_manifest, write_json_manifest};
    use std::collections::HashMap;

    fn scratch(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("recast-manifest-test-{name}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir.join("manifest.json")
    }

    #[test]
    fn round_trips_through_an_atomic_write() {
        let path = scratch("roundtrip");
        let mut map = HashMap::new();
        map.insert("a".to_string(), 1u32);
        write_json_manifest(&path, &map);

        let back: HashMap<String, u32> = read_json_manifest(&path);
        assert_eq!(back, map);
        assert!(
            !path.with_extension("json.tmp").exists(),
            "the temp file must not survive a successful write"
        );
    }

    #[test]
    fn quarantines_a_corrupt_manifest_instead_of_discarding_it() {
        let path = scratch("corrupt");
        std::fs::write(&path, b"{ this is not json").unwrap();

        let back: HashMap<String, u32> = read_json_manifest(&path);

        assert!(back.is_empty(), "an unreadable manifest reads as empty");
        let quarantine = path.with_extension("corrupt.json");
        assert!(
            quarantine.exists(),
            "the corrupt bytes must be preserved for recovery, not dropped"
        );
        assert!(!path.exists(), "the corrupt file is moved, not copied");
    }

    #[test]
    fn a_missing_manifest_is_not_quarantined() {
        let path = scratch("missing");
        let back: HashMap<String, u32> = read_json_manifest(&path);
        assert!(back.is_empty());
        assert!(!path.with_extension("corrupt.json").exists());
    }
}

#[cfg(test)]
mod tests {
    use super::classify_camera_name;

    #[test]
    fn classifies_known_flaky_virtual_cameras_as_warning() {
        let (status, message) = classify_camera_name("NVIDIA Broadcast");
        assert_eq!(status, "warning");
        assert!(message.is_some());
    }

    #[test]
    fn classifies_validation_required_virtual_cameras_as_unknown() {
        let (status, message) = classify_camera_name("OBS Virtual Camera");
        assert_eq!(status, "unknown");
        assert!(message.is_some());
    }

    #[test]
    fn classifies_plain_hardware_camera_as_ready_with_no_message() {
        let (status, message) = classify_camera_name("Integrated Camera");
        assert_eq!(status, "ready");
        assert!(message.is_none());
    }

    #[test]
    fn camera_classification_is_case_insensitive() {
        let (status, _) = classify_camera_name("droidcam source");
        assert_eq!(status, "warning");
    }
}

// Async plus spawn_blocking: the Linux path blocks on a D-Bus round-trip, and sync commands run on the UI thread.
#[tauri::command]
pub async fn open_file_location(path: String) -> AppResult<()> {
    tauri::async_runtime::spawn_blocking(move || open_file_location_blocking(path))
        .await
        .map_err(|e| AppError::msg(format!("open_file_location join error: {e}")))?
        .map_err(Into::into)
}

fn open_file_location_blocking(path: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        Command::new("explorer")
            .args(["/select,", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "macos")]
    {
        // `open -R` is Finder's equivalent of explorer select; a detached spawn, since we never wait on Finder.
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        // No portable reveal: try D-Bus FileManager1 via `gdbus`, then fall back to `xdg-open` on the parent; both best-effort.
        let p = std::path::Path::new(&path);
        let uri = format!("file://{}", p.display());
        let reveal = Command::new("gdbus")
            .args([
                "call",
                "--session",
                "--dest",
                "org.freedesktop.FileManager1",
                "--object-path",
                "/org/freedesktop/FileManager1",
                "--method",
                "org.freedesktop.FileManager1.ShowItems",
                &format!("[\"{uri}\"]"),
                "",
            ])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .stdin(std::process::Stdio::null())
            .status();
        let revealed = matches!(reveal, Ok(s) if s.success());
        if !revealed {
            // Couldn't reveal — open the containing folder.
            let parent = p.parent().unwrap_or_else(|| std::path::Path::new("."));
            let _ = Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Move a file to the OS recycle bin / trash.
/// Validates the path exists and is a file before deleting.
///
/// `trash::delete` is a COM shell round-trip on Windows and a Finder/DBus one
/// elsewhere, so it must not run on the main thread (macOS WKWebView freeze).
#[tauri::command]
pub async fn delete_file(path: String) -> AppResult<()> {
    tokio::task::spawn_blocking(move || {
        let target = std::path::Path::new(&path);
        if !target.exists() {
            return Err(AppError::from("File not found"));
        }
        if !target.is_file() {
            return Err(AppError::from("Path is not a file"));
        }
        trash::delete(target).map_err(|e| AppError::msg(format!("Could not move to trash: {e}")))
    })
    .await
    .map_err(|e| AppError::msg(format!("delete task panicked: {e}")))?
}

/// Rename a file in place (same directory, new filename).
/// Preserves the original extension by default if `new_name` has none.
/// Returns the new absolute path on success.
///
/// Edge cases handled:
/// - empty new name
/// - name containing path separators or illegal chars
/// - target filename already exists (reject, never overwrite)
/// - source file missing
#[tauri::command]
pub fn rename_file(path: String, new_name: String) -> AppResult<String> {
    let src = std::path::PathBuf::from(&path);
    if !src.exists() {
        return Err(AppError::from("File not found"));
    }
    if !src.is_file() {
        return Err(AppError::from("Path is not a file"));
    }

    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err(AppError::from("Name cannot be empty"));
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err(AppError::from("Name cannot contain path separators"));
    }
    // Basic Windows-illegal chars check.
    if trimmed
        .chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err(AppError::from("Name contains illegal characters"));
    }

    // If the user didn't include an extension, preserve the original one.
    let final_name = if std::path::Path::new(trimmed).extension().is_some() {
        trimmed.to_string()
    } else if let Some(orig_ext) = src.extension().and_then(|e| e.to_str()) {
        format!("{trimmed}.{orig_ext}")
    } else {
        trimmed.to_string()
    };

    let parent = src
        .parent()
        .ok_or_else(|| AppError::from("Cannot determine parent directory"))?;
    let dest = parent.join(&final_name);

    if dest == src {
        // No-op rename.
        return Ok(src.to_string_lossy().to_string());
    }
    if dest.exists() {
        return Err(AppError::msg(format!(
            "A file named \"{final_name}\" already exists"
        )));
    }

    std::fs::rename(&src, &dest).map_err(|e| AppError::msg(format!("Rename failed: {e}")))?;
    Ok(dest.to_string_lossy().to_string())
}

/// Probe which video encoders actually initialize on this device (a real
/// 1-frame encode per candidate, not just "compiled in"). Drives the
/// Settings → About "Hardware acceleration" matrix so users can see which
/// GPU encoder their machine supports and which one the recorder picks.
///
/// async + spawn_blocking because each hardware probe spawns FFmpeg and can
/// take a few hundred ms cold — running it inline would freeze the GTK main
/// thread on Linux (same rationale as `get_displays`).
#[tauri::command]
pub async fn probe_video_encoders() -> AppResult<Vec<crate::ffmpeg::EncoderAvailability>> {
    tauri::async_runtime::spawn_blocking(crate::ffmpeg::probe_recordable_encoders)
        .await
        .map_err(|e| AppError::msg(format!("probe_video_encoders join error: {e}")))
}

/// One capture-input capability and whether the *running* build can do it on
/// *this* device. `backend` names the native API actually used (DXGI, PipeWire,
/// AVFoundation, …) so the Settings panel can be specific instead of vague.
/// Why a capability isn't usable — the distinction the UI needs to choose
/// between "not supported on this OS" and "not available yet". `supported`
/// stays as the plain boolean the Settings matrix already keys off; `status`
/// refines the `false` case.
#[derive(Debug, Serialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum CapabilityStatus {
    /// Works on this device right now.
    Supported,
    /// The OS / native APIs genuinely can't do this — no future Recast build
    /// will add it on this platform. UI: "not supported on <os>".
    Unsupported,
    /// We intend to support it but haven't shipped it for this platform yet.
    /// UI: "not available yet". Only emitted by the unknown-platform branch of
    /// `build_capture_capabilities`, so it reads as unused on the three real
    /// targets — kept for the serialized API + the frontend's toast contract.
    #[allow(dead_code)]
    Planned,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCapability {
    /// Stable key the UI keys icons/order off: "screen" | "window" | "region"
    /// | "systemAudio" | "microphone" | "camera" | "cursor".
    pub key: String,
    pub label: String,
    pub supported: bool,
    /// Tri-state refinement of `supported`. When `supported` is true this is
    /// always `Supported`; when false it tells the UI whether to say "not
    /// supported here" (`Unsupported`) or "coming soon" (`Planned`).
    pub status: CapabilityStatus,
    pub backend: String,
    /// Optional caveat — permission requirement, fallback path, OS limitation.
    pub note: Option<String>,
}

/// Capture-support matrix for the current OS. Replaces the old hardcoded
/// "Windows only" banner: every row is computed from the backend the compiled
/// platform actually wires up plus a cheap runtime check, so the panel tells
/// the truth on Windows, macOS, and Linux alike.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CaptureCapabilities {
    /// Raw platform key — "windows" | "macos" | "linux" | "other".
    pub platform: String,
    /// Short name of the active screen-capture backend (the headline).
    pub screen_backend: String,
    pub capabilities: Vec<CaptureCapability>,
}

fn cap(
    key: &str,
    label: &str,
    supported: bool,
    backend: &str,
    note: Option<&str>,
) -> CaptureCapability {
    CaptureCapability {
        key: key.to_string(),
        label: label.to_string(),
        supported,
        status: if supported {
            CapabilityStatus::Supported
        } else {
            CapabilityStatus::Unsupported
        },
        backend: backend.to_string(),
        note: note.map(str::to_string),
    }
}

/// The cursor row, from what capturekit reports rather than from a per-OS
/// guess. Wayland hands the pointer to no application, so a hardcoded `true`
/// there is how a silently empty cursor track ships.
fn cursor_cap(backend: &str) -> CaptureCapability {
    let caps = capturekit::capabilities();
    let note = if !caps.cursor_pointer {
        Some("Your session does not report the pointer to applications, so cursor tracking is off.")
    } else if !caps.cursor_buttons {
        Some("Movement is tracked, but clicks cannot be detected on this session.")
    } else {
        None
    };
    cap(
        "cursor",
        "Cursor tracking",
        caps.cursor_pointer,
        backend,
        note,
    )
}

/// A capability we plan to support but haven't built for this platform yet —
/// distinct from `cap(.., false, ..)`, which marks something the OS can't do
/// at all. Drives the "not available yet" toast rather than "not supported".
/// Only used by the unknown-platform branch, so it's dead on real targets.
#[allow(dead_code)]
fn cap_planned(key: &str, label: &str, backend: &str, note: Option<&str>) -> CaptureCapability {
    CaptureCapability {
        key: key.to_string(),
        label: label.to_string(),
        supported: false,
        status: CapabilityStatus::Planned,
        backend: backend.to_string(),
        note: note.map(str::to_string),
    }
}

/// Build the capture-support matrix for whichever platform this binary was
/// compiled for. Each `#[cfg]` block is the function's tail expression on its
/// target.
///
/// Every row but the cursor describes a capturekit backend; the cursor sampler
/// is the app's own and has not moved yet.
fn build_capture_capabilities() -> CaptureCapabilities {
    #[cfg(windows)]
    {
        let screen_backend = "DXGI Desktop Duplication";
        CaptureCapabilities {
            platform: "windows".into(),
            screen_backend: screen_backend.into(),
            capabilities: vec![
                cap(
                    "screen",
                    "Full-screen recording",
                    true,
                    screen_backend,
                    None,
                ),
                cap(
                    "window",
                    "Window capture",
                    true,
                    "Windows Graphics Capture",
                    Some("Records the window's own surface, so overlapping windows stay out."),
                ),
                cap("region", "Region capture", true, screen_backend, None),
                cap("systemAudio", "System audio", true, "WASAPI loopback", None),
                cap("microphone", "Microphone", true, "WASAPI", None),
                cap(
                    "camera",
                    "Webcam",
                    true,
                    "Media Foundation",
                    Some("DirectShow-only virtual cameras are not listed."),
                ),
                cursor_cap("Win32 GetCursorInfo"),
            ],
        }
    }
    #[cfg(target_os = "macos")]
    {
        // Always present; the grant varies, and the row's note covers that.
        let has_screen = true;
        let screen_backend = "ScreenCaptureKit";
        CaptureCapabilities {
            platform: "macos".into(),
            screen_backend: screen_backend.into(),
            capabilities: vec![
                cap(
                    "screen",
                    "Full-screen recording",
                    has_screen,
                    screen_backend,
                    Some("Requires Screen Recording permission (System Settings → Privacy & Security)."),
                ),
                cap(
                    "window",
                    "Window capture",
                    has_screen,
                    screen_backend,
                    Some("Records the window's own surface, so overlapping windows stay out."),
                ),
                cap("region", "Region capture", has_screen, screen_backend, None),
                cap(
                    "systemAudio",
                    "System audio",
                    has_screen,
                    screen_backend,
                    Some("Carries the Screen Recording grant: macOS taps the output mix through the same stream."),
                ),
                cap(
                    "microphone",
                    "Microphone",
                    true,
                    "AVFoundation",
                    Some("Uses the Microphone permission, separate from Screen Recording."),
                ),
                cap("camera", "Webcam", true, "AVFoundation", None),
                cursor_cap("CoreGraphics"),
            ],
        }
    }
    #[cfg(target_os = "linux")]
    {
        // WAYLAND_DISPLAY first: XWayland sets both, and X11 capture under it is black.
        let wayland = std::env::var_os("WAYLAND_DISPLAY").is_some();
        let x11 = std::env::var_os("DISPLAY").is_some();
        let (screen_backend, screen_note): (&str, Option<&str>) = if wayland {
            (
                "PipeWire (xdg-desktop-portal)",
                Some("Approve the screen-share prompt at the start of each recording."),
            )
        } else if x11 {
            ("X11 (XGetImage)", None)
        } else {
            (
                "None",
                Some("No display server detected — set WAYLAND_DISPLAY or DISPLAY."),
            )
        };
        let audio = capturekit::capabilities().audio_loopback;
        CaptureCapabilities {
            platform: "linux".into(),
            screen_backend: screen_backend.into(),
            capabilities: vec![
                cap(
                    "screen",
                    "Full-screen recording",
                    true,
                    screen_backend,
                    screen_note,
                ),
                cap("window", "Window capture", true, screen_backend, None),
                cap("region", "Region capture", true, screen_backend, None),
                cap(
                    "systemAudio",
                    "System audio",
                    audio,
                    "PipeWire",
                    Some("Reads the default sink's monitor; needs a running PipeWire daemon."),
                ),
                cap("microphone", "Microphone", audio, "PipeWire", None),
                cap("camera", "Webcam", true, "V4L2", None),
                cursor_cap("X11 XQueryPointer"),
            ],
        }
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        // No backend wired yet is a gap in our coverage, not an incapable OS, so mark every row planned, not unsupported.
        let pending = "Not implemented yet";
        CaptureCapabilities {
            platform: "other".into(),
            screen_backend: pending.into(),
            capabilities: vec![
                cap_planned(
                    "screen",
                    "Full-screen recording",
                    pending,
                    Some("Screen capture isn't available for this platform yet."),
                ),
                cap_planned("window", "Window capture", pending, None),
                cap_planned("region", "Region capture", pending, None),
                cap_planned("systemAudio", "System audio", pending, None),
                cap_planned("microphone", "Microphone", pending, None),
                cap_planned("camera", "Webcam", pending, None),
                cap_planned("cursor", "Cursor tracking", pending, None),
            ],
        }
    }
}

/// Report which capture inputs this device's native APIs support. Drives the
/// Settings → "Capture support" panel. async + spawn_blocking because on macOS
/// the first call may spawn the FFmpeg AVFoundation device listing — keeping it
/// off the UI thread matches the other probe commands.
#[tauri::command]
pub async fn capture_capabilities() -> AppResult<CaptureCapabilities> {
    tauri::async_runtime::spawn_blocking(build_capture_capabilities)
        .await
        .map_err(|e| AppError::msg(format!("capture_capabilities join error: {e}")))
}

/// Whether `recast` currently resolves as a bare terminal command.
#[tauri::command]
pub fn cli_install_status() -> crate::path_install::InstallStatus {
    crate::path_install::status()
}

/// Put `recast` on the user's PATH (the in-app "Install command line tool").
///
/// Async + `spawn_blocking`: this copies the whole release binary (tens of MB
/// with ggml + ocr linked) and edits the registry / shell rc. A sync command
/// runs on the main thread, which freezes the macOS WKWebView.
#[tauri::command]
pub async fn install_cli() -> AppResult<String> {
    tokio::task::spawn_blocking(|| crate::path_install::install().map_err(AppError::msg))
        .await
        .map_err(|e| AppError::msg(format!("install task panicked: {e}")))?
}

/// Remove `recast` from the user's PATH.
#[tauri::command]
pub async fn uninstall_cli() -> AppResult<String> {
    tokio::task::spawn_blocking(|| crate::path_install::uninstall().map_err(AppError::msg))
        .await
        .map_err(|e| AppError::msg(format!("uninstall task panicked: {e}")))?
}

#[derive(Debug, Serialize)]
pub struct FfmpegDiagnostics {
    pub ffmpeg_path: String,
    pub ffprobe_path: String,
    pub version: Option<String>,
    pub h264_encoder: String,
    pub encoders_present: Vec<String>,
    pub encoders_missing: Vec<String>,
}

/// Reports the resolved ffmpeg/ffprobe paths, version line, and which of the
/// encoders the export pipeline depends on are actually available. Surfaced
/// to the UI so users can include this in bug reports without needing a CLI.
#[tauri::command]
pub async fn diagnose_ffmpeg() -> AppResult<FfmpegDiagnostics> {
    tauri::async_runtime::spawn_blocking(|| {
        let ffmpeg = crate::ffmpeg::ffmpeg_path().clone();
        let ffprobe = crate::ffmpeg::ffprobe_path().clone();

        let version = {
            let mut cmd = Command::new(&ffmpeg);
            cmd.arg("-version");
            crate::ffmpeg::configure_silent_command(&mut cmd);
            cmd.output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| {
                    String::from_utf8_lossy(&o.stdout)
                        .lines()
                        .next()
                        .map(|s| s.to_string())
                })
        };

        // Critical encoders for our export formats.
        const REQUIRED: &[&str] = &["libx264", "aac", "libvpx-vp9", "libopus"];
        let mut present: Vec<String> = Vec::new();
        let mut missing: Vec<String> = Vec::new();

        let encoders_output = {
            let mut cmd = Command::new(&ffmpeg);
            cmd.args(["-hide_banner", "-encoders"]);
            crate::ffmpeg::configure_silent_command(&mut cmd);
            cmd.output()
        };
        if let Ok(out) = encoders_output {
            let table = String::from_utf8_lossy(&out.stdout);
            for &name in REQUIRED {
                if table.contains(name) {
                    present.push(name.to_string());
                } else {
                    missing.push(name.to_string());
                }
            }
            // Hardware encoders are informational: list what the bundled FFmpeg supports so diagnostics reflect what is selectable.
            for hw in [
                "h264_videotoolbox",
                "h264_nvenc",
                "h264_amf",
                "h264_qsv",
                "hevc_videotoolbox",
                "hevc_nvenc",
                "hevc_amf",
                "hevc_qsv",
            ] {
                if table.contains(hw) {
                    present.push(hw.to_string());
                }
            }
        } else {
            for &name in REQUIRED {
                missing.push(name.to_string());
            }
        }

        Ok(FfmpegDiagnostics {
            ffmpeg_path: ffmpeg.display().to_string(),
            ffprobe_path: ffprobe.display().to_string(),
            version,
            h264_encoder: crate::ffmpeg::preferred_h264_encoder().to_string(),
            encoders_present: present,
            encoders_missing: missing,
        })
    })
    .await
    .map_err(|e| AppError::msg(format!("diagnose_ffmpeg join error: {e}")))?
}

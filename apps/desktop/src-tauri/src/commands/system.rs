use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};
use xcap::{Monitor, Window};

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
            // `%TEMP%` is periodically purged by the OS, so settings (incl.
            // telemetry consent + install_id) won't survive between sessions.
            // Log it so a "my settings keep resetting" report is diagnosable
            // rather than silent.
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
                // A genuine parse failure (partial write, hand-edit, schema
                // drift) — distinct from "no file yet". Silently resetting here
                // would wipe all settings AND flip telemetry_errors back to its
                // default-on, a privacy-relevant regression. Back the bad file
                // up for diagnosis before falling back to defaults.
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
    // Atomic write: a plain `fs::write` truncates-then-writes, so a crash or
    // power loss mid-write leaves a corrupt file that the next launch discards —
    // wiping every setting. Write to a sibling temp file, fsync, then rename
    // over the target (atomic on the same volume).
    let tmp = path.with_extension("json.tmp");
    if let Err(e) = write_atomic(&tmp, &path, data.as_bytes()) {
        log::warn!("failed to persist config to {}: {e}", path.display());
        let _ = fs::remove_file(&tmp);
    }
}

fn write_atomic(tmp: &Path, dest: &Path, bytes: &[u8]) -> std::io::Result<()> {
    use std::io::Write;
    let mut file = fs::File::create(tmp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(tmp, dest)
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

fn capture_monitor_thumbnail(monitor: &Monitor) -> Option<String> {
    if is_wayland() {
        return None;
    }
    let shot = monitor.capture_image().ok()?;
    encode_thumbnail_base64(&make_thumbnail(&shot))
}

fn capture_window_thumbnail(window: &Window) -> Option<String> {
    if is_wayland() {
        return None;
    }
    let shot = window.capture_image().ok()?;
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
    // If the floating panel is currently open, apply the change right away.
    // `set_content_protected` toggles both directions (Windows
    // WDA_EXCLUDEFROMCAPTURE ⇄ WDA_NONE, macOS NSWindow.sharingType none ⇄
    // readOnly) and the compositor honors it on the next captured frame, so a
    // mid-recording flip is reflected immediately. No-op on Linux.
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
///   - off (default) → release builds stay quiet (Warn); debug builds keep Info
///   - on            → Debug everywhere, capturing backend processing +
///                     editor-interaction traces for a support bundle
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
    // Logged AFTER raising the level so the "enabled" transition is the first
    // line in a fresh diagnostic session.
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

// `get_displays` and `get_windows` are async + spawn_blocking because xcap's
// underlying calls (`Monitor::all`, `Window::all`, `capture_image`) can stall
// for hundreds of ms or longer on Linux/Wayland (portal handshake, compositor
// IPC). Tauri runs sync commands directly on the GTK main thread on Linux —
// any blocking work there freezes the entire window: close/minimize/maximize
// stop responding because the WM can't deliver events. Pushing both onto a
// blocking worker keeps the GTK loop free even if xcap hangs.
#[tauri::command]
pub async fn get_displays() -> AppResult<Vec<DisplayInfo>> {
    tauri::async_runtime::spawn_blocking(|| -> AppResult<Vec<DisplayInfo>> {
        let monitors = Monitor::all().map_err(|e| e.to_string())?;
        Ok(monitors
            .iter()
            .map(|monitor| DisplayInfo {
                id: monitor.id().unwrap_or_default(),
                name: monitor.name().unwrap_or_default(),
                x: monitor.x().unwrap_or_default(),
                y: monitor.y().unwrap_or_default(),
                width: monitor.width().unwrap_or_default(),
                height: monitor.height().unwrap_or_default(),
                is_primary: monitor.is_primary().unwrap_or_default(),
                thumbnail: capture_monitor_thumbnail(monitor),
                // The display's current refresh rate, rounded; 0 if unreported.
                // This is the *current* rate, not the panel's max, so a 144 Hz
                // monitor running at 60 correctly reports 60.
                refresh_hz: monitor
                    .frequency()
                    .ok()
                    .filter(|hz| hz.is_finite() && *hz >= 1.0)
                    .map(|hz| hz.round() as u32)
                    .unwrap_or(0),
            })
            .collect())
    })
    .await
    .map_err(|e| AppError::msg(format!("get_displays join error: {e}")))?
}

#[tauri::command]
pub async fn get_windows() -> AppResult<Vec<WindowInfo>> {
    tauri::async_runtime::spawn_blocking(|| -> AppResult<Vec<WindowInfo>> {
        let windows = Window::all().map_err(|e| e.to_string())?;
        // Each xcap accessor hits the compositor/WM. The old filter + map
        // called `.is_minimized()` and `.title()` twice each per window.
        // Snapshot once into a local struct, then filter + map cheaply.
        Ok(windows
            .iter()
            .filter_map(|window| {
                let is_minimized = window.is_minimized().unwrap_or_default();
                let title = window.title().unwrap_or_default();
                if is_minimized || title.is_empty() {
                    return None;
                }
                Some(WindowInfo {
                    id: window.id().unwrap_or_default(),
                    pid: window.pid().unwrap_or_default(),
                    app_name: window.app_name().unwrap_or_default(),
                    title,
                    x: window.x().unwrap_or_default(),
                    y: window.y().unwrap_or_default(),
                    width: window.width().unwrap_or_default(),
                    height: window.height().unwrap_or_default(),
                    is_minimized,
                    thumbnail: capture_window_thumbnail(window),
                })
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

fn get_audio_devices_blocking() -> Result<Vec<AudioDeviceInfo>, String> {
    #[cfg(windows)]
    {
        get_audio_devices_windows()
    }
    #[cfg(target_os = "macos")]
    {
        Ok(get_audio_devices_macos())
    }
    #[cfg(target_os = "linux")]
    {
        Ok(get_audio_devices_linux())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Ok(Vec::new())
    }
}

/// macOS audio input devices via the cached AVFoundation listing in
/// `ffmpeg.rs`. We reuse the already-collected stderr (one probe per
/// process) instead of spawning FFmpeg again — see
/// `cached_avfoundation_devices` for the caching rationale.
///
/// AVFoundation lacks a "system default" concept comparable to WASAPI's
/// `eConsole` endpoint, so `is_default` flags the first device — which
/// is what FFmpeg picks when `-i :0` is passed without an explicit index.
#[cfg(target_os = "macos")]
fn get_audio_devices_macos() -> Vec<AudioDeviceInfo> {
    let stderr = crate::ffmpeg::cached_avfoundation_devices();
    parse_avfoundation_section(stderr, AvfSection::Audio)
        .into_iter()
        .enumerate()
        .map(|(idx, (id, name))| AudioDeviceInfo {
            id,
            name,
            is_default: idx == 0,
        })
        .collect()
}

/// Linux audio input devices via `pactl list short sources`. Returns
/// PulseAudio source names (e.g. `alsa_input.pci-0000_00_1f.3.analog-stereo`)
/// as IDs. `.monitor` sources are filtered out — those are loopback
/// sinks, not microphones, and would confuse a mic picker.
#[cfg(target_os = "linux")]
fn get_audio_devices_linux() -> Vec<AudioDeviceInfo> {
    let output = match std::process::Command::new("pactl")
        .args(["list", "short", "sources"])
        .output()
    {
        Ok(out) if out.status.success() => out,
        _ => return Vec::new(),
    };
    let text = String::from_utf8_lossy(&output.stdout);
    let mut devices = Vec::new();
    for line in text.lines() {
        // Format: `<id>\t<name>\t<driver>\t<sample_spec>\t<state>`
        let mut cols = line.split('\t');
        let _ = cols.next();
        let Some(name) = cols.next() else { continue };
        if name.ends_with(".monitor") {
            continue;
        }
        // First *surviving* (non-monitor) source becomes the default. Using
        // the pre-filter enumerate index here would let a leading `.monitor`
        // row eat the only default slot.
        let is_default = devices.is_empty();
        devices.push(AudioDeviceInfo {
            id: name.to_string(),
            name: name.to_string(),
            is_default,
        });
    }
    devices
}

#[cfg(windows)]
fn get_audio_devices_windows() -> Result<Vec<AudioDeviceInfo>, String> {
    use windows::Win32::Media::Audio::*;
    use windows::Win32::System::Com::*;

    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);

        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|e| format!("failed to create device enumerator: {e}"))?;

        let default_id = enumerator
            .GetDefaultAudioEndpoint(eCapture, eConsole)
            .ok()
            .and_then(|d| d.GetId().ok())
            .map(|pwstr| pwstr.to_string().unwrap_or_default());

        let collection = enumerator
            .EnumAudioEndpoints(eCapture, DEVICE_STATE_ACTIVE)
            .map_err(|e| format!("failed to enumerate audio devices: {e}"))?;

        let count = collection.GetCount().map_err(|e| e.to_string())?;
        let mut devices = Vec::new();

        for i in 0..count {
            let Ok(device) = collection.Item(i) else {
                continue;
            };

            let id = device
                .GetId()
                .ok()
                .and_then(|pwstr| pwstr.to_string().ok())
                .unwrap_or_default();

            // Use device friendly name from endpoint properties.
            let name = get_device_name(&device).unwrap_or_else(|| format!("Microphone {}", i + 1));

            let is_default = default_id.as_deref() == Some(&id);

            devices.push(AudioDeviceInfo {
                id,
                name,
                is_default,
            });
        }

        Ok(devices)
    }
}

/// Extract the friendly name from an audio device using its property store.
#[cfg(windows)]
fn get_device_name(device: &windows::Win32::Media::Audio::IMMDevice) -> Option<String> {
    use windows::core::GUID;
    use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};

    unsafe {
        let store: IPropertyStore = device
            .OpenPropertyStore(windows::Win32::System::Com::STGM(0))
            .ok()?;
        // PKEY_Device_FriendlyName = {a45c254e-df1c-4efd-8020-67d146a850e0}, 14
        let key = PROPERTYKEY {
            fmtid: GUID::from_values(
                0xa45c254e,
                0xdf1c,
                0x4efd,
                [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0],
            ),
            pid: 14,
        };
        let value = store.GetValue(&key).ok()?;
        // The value is a VT_LPWSTR PROPVARIANT. Use its Display/Debug impl.
        let display = format!("{}", value);
        if display.is_empty() || display == "EMPTY" {
            None
        } else {
            Some(display)
        }
    }
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
    // Device enumeration can take a few hundred ms (or several seconds if a
    // webcam is slow to respond on Windows dshow). Tauri runs sync commands
    // on the main thread, which froze the UI; move to a worker.
    tauri::async_runtime::spawn_blocking(get_camera_devices_blocking)
        .await
        .map_err(|e| AppError::msg(format!("get_camera_devices join error: {e}")))?
        .map_err(Into::into)
}

fn get_camera_devices_blocking() -> Result<Vec<CameraDeviceInfo>, String> {
    #[cfg(windows)]
    {
        get_camera_devices_windows()
    }
    #[cfg(target_os = "macos")]
    {
        Ok(get_camera_devices_macos())
    }
    #[cfg(target_os = "linux")]
    {
        Ok(get_camera_devices_linux())
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        Ok(Vec::new())
    }
}

#[cfg(windows)]
fn get_camera_devices_windows() -> Result<Vec<CameraDeviceInfo>, String> {
    // ffmpeg DirectShow listing is the only way to enumerate dshow names
    // that match what the capture pipeline opens.
    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command.args([
        "-hide_banner",
        "-list_devices",
        "true",
        "-f",
        "dshow",
        "-i",
        "dummy",
    ]);
    crate::ffmpeg::configure_silent_command(&mut command);
    let output = command
        .output()
        .map_err(|e| format!("failed to list camera devices: {e}"))?;

    // ffmpeg prints device list to stderr (it "fails" because "dummy" isn't a real input).
    let stderr = String::from_utf8_lossy(&output.stderr);
    Ok(parse_camera_devices(&stderr))
}

/// macOS cameras from the cached AVFoundation listing. The AVFoundation
/// video section includes screen-capture pseudo-devices ("Capture screen 0",
/// "Capture screen 1") that aren't real cameras — filter those so the UI
/// picker doesn't surface them.
#[cfg(target_os = "macos")]
fn get_camera_devices_macos() -> Vec<CameraDeviceInfo> {
    let stderr = crate::ffmpeg::cached_avfoundation_devices();
    parse_avfoundation_section(stderr, AvfSection::Video)
        .into_iter()
        .filter(|(_, name)| !name.to_ascii_lowercase().starts_with("capture screen"))
        .map(|(id, name)| {
            let (status, status_message) = classify_camera_name(&name);
            CameraDeviceInfo {
                id,
                name,
                status,
                status_message,
            }
        })
        .collect()
}

/// Linux cameras via /dev/video* scan + sysfs name lookup. Friendly names
/// live in `/sys/class/video4linux/videoN/name`; the device node path is
/// what the V4L2 input expects, so we return that as the ID.
///
/// V4L2 exposes capture *and* output devices under the same prefix. We
/// only want capture devices (cameras); each video node's `device_caps`
/// in sysfs has the V4L2_CAP_VIDEO_CAPTURE bit (0x00000001) set. Read
/// that bit and skip nodes that don't have it.
#[cfg(target_os = "linux")]
fn get_camera_devices_linux() -> Vec<CameraDeviceInfo> {
    let entries = match std::fs::read_dir("/dev") {
        Ok(r) => r,
        Err(_) => return Vec::new(),
    };
    let mut nodes: Vec<std::path::PathBuf> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if !name.starts_with("video") {
                return None;
            }
            // Only numeric suffixes — skip "video-output" etc. that some drivers expose.
            if !name[5..].chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            Some(entry.path())
        })
        .collect();
    // Sort by node number so /dev/video0 comes first, /dev/video10 after /dev/video2.
    nodes.sort_by_key(|p| {
        p.file_name()
            .and_then(|s| s.to_str())
            .and_then(|s| s[5..].parse::<u32>().ok())
            .unwrap_or(u32::MAX)
    });

    let mut devices = Vec::new();
    for node in nodes {
        let Some(file_name) = node.file_name().and_then(|s| s.to_str()) else {
            continue;
        };
        // Skip non-capture devices (V4L2 output, metadata, etc.) — we only
        // want webcams. The capture-capability bit is V4L2_CAP_VIDEO_CAPTURE.
        let caps_path = format!("/sys/class/video4linux/{file_name}/device_caps");
        if let Ok(caps_text) = std::fs::read_to_string(&caps_path) {
            let caps =
                u32::from_str_radix(caps_text.trim().trim_start_matches("0x"), 16).unwrap_or(0);
            if caps & 0x0000_0001 == 0 {
                continue;
            }
        }
        let name = std::fs::read_to_string(format!("/sys/class/video4linux/{file_name}/name"))
            .map(|s| s.trim().to_string())
            .ok()
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| file_name.to_string());
        let id = node.to_string_lossy().to_string();
        let (status, status_message) = classify_camera_name(&name);
        devices.push(CameraDeviceInfo {
            id,
            name,
            status,
            status_message,
        });
    }
    devices
}

/// AVFoundation listing sections. The `-list_devices true` probe prints
/// two — video first, then audio — each marked with a header line we
/// match on.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy)]
enum AvfSection {
    Video,
    Audio,
}

/// Parse `cached_avfoundation_devices`'s stderr for one section.
///
/// AVFoundation lines look like:
/// ```text
/// [AVFoundation indev @ 0x…] AVFoundation video devices:
/// [AVFoundation indev @ 0x…] [0] FaceTime HD Camera
/// [AVFoundation indev @ 0x…] [1] Capture screen 0
/// [AVFoundation indev @ 0x…] AVFoundation audio devices:
/// [AVFoundation indev @ 0x…] [0] MacBook Air Microphone
/// ```
///
/// Returns `(id, name)` pairs where `id` is the numeric AVFoundation index
/// (matches what the capture pipeline passes to FFmpeg as `-i N:M`) and
/// `name` is the user-readable string after the index.
#[cfg(target_os = "macos")]
fn parse_avfoundation_section(stderr: &str, section: AvfSection) -> Vec<(String, String)> {
    let (in_marker, other_marker) = match section {
        AvfSection::Video => ("AVFoundation video devices", "AVFoundation audio devices"),
        AvfSection::Audio => ("AVFoundation audio devices", "AVFoundation video devices"),
    };
    let mut in_section = false;
    let mut out = Vec::new();
    for line in stderr.lines() {
        if line.contains(in_marker) {
            in_section = true;
            continue;
        }
        if line.contains(other_marker) {
            in_section = false;
            continue;
        }
        if !in_section {
            continue;
        }
        // Each device line has `[N]` (the numeric index in square brackets)
        // somewhere after the `[AVFoundation indev @ …]` prefix. Find the
        // *last* `[` to skip the prefix.
        let Some(idx_open) = line.rfind('[') else {
            continue;
        };
        let Some(idx_close_rel) = line[idx_open + 1..].find(']') else {
            continue;
        };
        let inside = &line[idx_open + 1..idx_open + 1 + idx_close_rel];
        // Must be purely numeric — skip the prefix `[AVFoundation indev @ …]`.
        if inside.is_empty() || !inside.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let name = line[idx_open + 1 + idx_close_rel + 1..].trim().to_string();
        if name.is_empty() {
            continue;
        }
        out.push((inside.to_string(), name));
    }
    out
}

#[cfg(any(windows, test))]
fn parse_camera_devices(stderr: &str) -> Vec<CameraDeviceInfo> {
    let mut devices: Vec<CameraDeviceInfo> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut in_video_section = false;
    for line in stderr.lines() {
        if line.contains("DirectShow video devices") {
            in_video_section = true;
            continue;
        }
        if line.contains("DirectShow audio devices") {
            in_video_section = false;
            continue;
        }
        if line.contains("Alternative name") {
            continue;
        }

        let has_video_tag = line.contains("(video)");
        let has_audio_tag = line.contains("(audio)");
        let is_video_device = has_video_tag || (in_video_section && !has_audio_tag);
        if !is_video_device {
            continue;
        }

        let Some(start) = line.find('"') else {
            continue;
        };
        let Some(end_rel) = line[start + 1..].find('"') else {
            continue;
        };
        let name = line[start + 1..start + 1 + end_rel].trim().to_string();
        if name.is_empty() || !seen.insert(name.clone()) {
            continue;
        }

        let (status, status_message) = classify_camera_name(&name);
        devices.push(CameraDeviceInfo {
            id: name.clone(),
            name,
            status,
            status_message,
        });
    }
    devices
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

        // Deep liveliness probe is Windows-only — it spawns FFmpeg with
        // `-f dshow` against the device ID. On macOS/Linux we trust the
        // enumeration's classification (which already flags known
        // virtual-camera quirks) since AVFoundation / V4L2 don't have a
        // cheap equivalent to dshow's "open and grab one frame" check.
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
mod tests {
    use super::{classify_camera_name, parse_camera_devices};

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

    #[test]
    fn parses_legacy_ffmpeg_camera_list() {
        let stderr = r#"
[dshow @ 0000] DirectShow video devices
[dshow @ 0000]  "Integrated Camera"
[dshow @ 0000]  "NVIDIA Broadcast"
[dshow @ 0000] DirectShow audio devices
"#;
        let devices = parse_camera_devices(stderr);
        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Integrated Camera");
        assert_eq!(devices[0].status, "ready");
        assert_eq!(devices[1].status, "warning");
    }

    #[test]
    fn parses_inline_video_tags_and_dedupes() {
        let stderr = r#"
[dshow @ 0000] "OBS Virtual Camera" (video)
[dshow @ 0000] "OBS Virtual Camera" (video)
[dshow @ 0000] "Microphone" (audio)
"#;
        let devices = parse_camera_devices(stderr);
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].name, "OBS Virtual Camera");
        assert_eq!(devices[0].status, "unknown");
    }
}

// `async` + `spawn_blocking`: the Linux path runs `gdbus ... .status()`, which
// blocks on a D-Bus round-trip (and its timeout) to the file manager. Sync
// commands run on the macOS/Linux UI thread, so that round-trip would briefly
// freeze the window. The Windows/macOS branches only `spawn()` (non-blocking),
// but routing all three through a worker keeps the command uniformly off-thread.
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
        // `open -R` is the Finder equivalent of `explorer /select,` —
        // it opens Finder and highlights the file in its containing
        // folder. Detached spawn; we never wait on Finder.
        Command::new("open")
            .args(["-R", &path])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(target_os = "linux")]
    {
        // No portable "reveal" — the closest cross-DE option is the
        // D-Bus FileManager1 interface, supported by Nautilus, Dolphin,
        // Nemo, Caja, and Thunar. Try that first via `gdbus`, then fall
        // back to opening the parent directory with `xdg-open`. Both
        // paths are best-effort: if neither tool is present we still
        // succeed at the IPC level so the UI doesn't surface a hard
        // failure for what is a quality-of-life shortcut.
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
#[tauri::command]
pub fn delete_file(path: String) -> AppResult<()> {
    let target = std::path::Path::new(&path);
    if !target.exists() {
        return Err(AppError::from("File not found"));
    }
    if !target.is_file() {
        return Err(AppError::from("Path is not a file"));
    }
    trash::delete(target).map_err(|e| AppError::msg(format!("Could not move to trash: {e}")))?;
    Ok(())
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
/// target — the same dispatch pattern as `capture::platform::create_source`.
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
                    Some("Falls back to GDI capture (xcap) if GPU duplication is unavailable."),
                ),
                cap("window", "Window capture", true, screen_backend, None),
                cap("region", "Region capture", true, screen_backend, None),
                cap("systemAudio", "System audio", true, "WASAPI loopback", None),
                cap("microphone", "Microphone", true, "WASAPI", None),
                cap("camera", "Webcam", true, "DirectShow (FFmpeg)", None),
                cap(
                    "cursor",
                    "Cursor tracking",
                    true,
                    "Win32 GetCursorInfo",
                    None,
                ),
            ],
        }
    }
    #[cfg(target_os = "macos")]
    {
        // The AVFoundation device listing is cached after the first probe. A
        // "Capture screen" pseudo-device in it means the bundled FFmpeg has
        // avfoundation support wired — the prerequisite for the native macOS
        // path. (Screen Recording *permission* is enforced at record time,
        // not at listing time, so its presence here only proves the API is
        // reachable, which is exactly the "is it supported" question.)
        let listing = crate::ffmpeg::cached_avfoundation_devices();
        let has_avf = !listing.is_empty();
        let has_screen = listing.to_ascii_lowercase().contains("capture screen");
        let screen_backend = "FFmpeg AVFoundation";
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
                    Some("Captured as a screen region; placement is approximate on Retina displays."),
                ),
                cap("region", "Region capture", has_screen, screen_backend, None),
                cap(
                    "systemAudio",
                    "System audio",
                    has_avf,
                    "ScreenCaptureKit",
                    Some("Falls back to a virtual device (e.g. BlackHole) when the system tap is unavailable."),
                ),
                cap("microphone", "Microphone", has_avf, "AVFoundation", None),
                cap("camera", "Webcam", has_avf, "AVFoundation", None),
                cap("cursor", "Cursor tracking", true, "CoreGraphics", None),
            ],
        }
    }
    #[cfg(target_os = "linux")]
    {
        // Session-type dispatch mirrors `capture::platform::create_source`:
        // prefer Wayland (PipeWire portal) when present, else X11, else the
        // software fallback. WAYLAND_DISPLAY is checked first because XWayland
        // sets both.
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
                "xcap (software)",
                Some("No display server detected — capture falls back to the slow software path."),
            )
        };
        // device_query reads the pointer through X11/xcb; a pure-Wayland
        // session (no XWayland) blocks global pointer reads, so cursor
        // tracking is best-effort there.
        let cursor_note = if wayland && !x11 {
            Some("Limited under Wayland — global cursor position may be unavailable.")
        } else {
            None
        };
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
                    true,
                    "PulseAudio / PipeWire",
                    Some("Uses a PulseAudio monitor source; needs PulseAudio or PipeWire-Pulse."),
                ),
                cap(
                    "microphone",
                    "Microphone",
                    true,
                    "PulseAudio (FFmpeg)",
                    None,
                ),
                cap("camera", "Webcam", true, "V4L2", None),
                cap("cursor", "Cursor tracking", true, "X11 (xcb)", cursor_note),
            ],
        }
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        // Unknown desktop platform: we have no backend wired yet, but this is a
        // gap in our coverage rather than an OS that *can't* — so mark every row
        // `planned` ("not available yet") instead of `unsupported`.
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
            // Hardware encoders are informational, not required. Listing
            // every vendor-specific codec the bundled FFmpeg supports so
            // the diagnostics page reflects what's actually selectable.
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

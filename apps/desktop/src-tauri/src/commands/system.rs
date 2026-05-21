use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use tauri::{AppHandle, Manager, State};
use xcap::{Monitor, Window};

use super::ffmpeg::{encode_thumbnail_base64, make_thumbnail};
use serde::Serialize;

use super::types::{
    AppConfig, AppState, CameraDeviceInfo, CameraValidationResult, DisplayInfo, LastSource,
    WindowInfo,
};

fn config_path(app: &AppHandle) -> PathBuf {
    app.path()
        .app_data_dir()
        .unwrap_or_else(|_| env::temp_dir())
        .join("recast_config.json")
}

pub fn load_config(app: &AppHandle) -> AppConfig {
    let path = config_path(app);
    if let Ok(data) = fs::read_to_string(&path) {
        if let Ok(config) = serde_json::from_str(&data) {
            return config;
        }
    }
    AppConfig::default()
}

pub(crate) fn save_config(app: &AppHandle, config: &AppConfig) {
    let path = config_path(app);
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(data) = serde_json::to_string_pretty(config) {
        let _ = fs::write(path, data);
    }
}

pub fn get_active_output_dir(state: &State<'_, AppState>) -> PathBuf {
    let config = state.config.lock();
    if let Some(dir) = &config.output_dir {
        PathBuf::from(dir)
    } else {
        env::temp_dir()
    }
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
pub fn get_output_dir(state: State<'_, AppState>) -> Result<String, String> {
    Ok(get_active_output_dir(&state).to_string_lossy().to_string())
}

#[tauri::command]
pub fn set_output_dir(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    if !Path::new(&path).exists() {
        return Err("Directory does not exist".into());
    }
    let mut config = state.config.lock();
    config.output_dir = Some(path);
    save_config(&app, &config);
    Ok(())
}

#[tauri::command]
pub fn get_last_source(state: State<'_, AppState>) -> Result<Option<LastSource>, String> {
    Ok(state.config.lock().last_source.clone())
}

#[tauri::command]
pub fn set_last_source(
    app: AppHandle,
    state: State<'_, AppState>,
    source: LastSource,
) -> Result<(), String> {
    let mut config = state.config.lock();
    config.last_source = Some(source);
    save_config(&app, &config);
    Ok(())
}

// `get_displays` and `get_windows` are async + spawn_blocking because xcap's
// underlying calls (`Monitor::all`, `Window::all`, `capture_image`) can stall
// for hundreds of ms or longer on Linux/Wayland (portal handshake, compositor
// IPC). Tauri runs sync commands directly on the GTK main thread on Linux —
// any blocking work there freezes the entire window: close/minimize/maximize
// stop responding because the WM can't deliver events. Pushing both onto a
// blocking worker keeps the GTK loop free even if xcap hangs.
#[tauri::command]
pub async fn get_displays() -> Result<Vec<DisplayInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| -> Result<Vec<DisplayInfo>, String> {
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
            })
            .collect())
    })
    .await
    .map_err(|e| format!("get_displays join error: {e}"))?
}

#[tauri::command]
pub async fn get_windows() -> Result<Vec<WindowInfo>, String> {
    tauri::async_runtime::spawn_blocking(|| -> Result<Vec<WindowInfo>, String> {
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
    .map_err(|e| format!("get_windows join error: {e}"))?
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AudioDeviceInfo {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

/// List available audio input (microphone) devices.
#[tauri::command]
pub fn get_audio_devices() -> Result<Vec<AudioDeviceInfo>, String> {
    #[cfg(windows)]
    {
        get_audio_devices_windows()
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
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
        let display = format!("{}", value.to_string());
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
/// No-op on non-Windows platforms.
#[tauri::command]
pub fn exclude_window_from_capture(app: AppHandle, label: String) -> Result<(), String> {
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("window '{label}' not found"))?;
    #[cfg(windows)]
    {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::WindowsAndMessaging::{
            SetWindowDisplayAffinity, WDA_EXCLUDEFROMCAPTURE,
        };
        let hwnd_raw = window
            .hwnd()
            .map_err(|e| format!("hwnd lookup failed for '{label}': {e}"))?;
        // Tauri's `hwnd()` returns a `windows::Win32::Foundation::HWND`
        // already, but the inner pointer type may differ between Tauri's
        // pinned `windows` version and ours. Reconstruct from the raw
        // pointer to be version-agnostic.
        let hwnd = HWND(hwnd_raw.0 as *mut std::ffi::c_void);
        unsafe {
            SetWindowDisplayAffinity(hwnd, WDA_EXCLUDEFROMCAPTURE)
                .map_err(|e| format!("SetWindowDisplayAffinity failed for '{label}': {e}"))?;
        }
        log::info!("excluded window '{label}' from screen capture");
        Ok(())
    }
    #[cfg(not(windows))]
    {
        // Other platforms have their own per-OS exclusion APIs (macOS:
        // CGSWindowSetSharingState; Linux: no portable equivalent). Phase 1
        // ships Windows-only since the recording pipeline is Windows-only
        // today; revisit if the platform matrix expands.
        let _ = window;
        Ok(())
    }
}

/// List available camera/video capture devices.
#[tauri::command]
pub async fn get_camera_devices() -> Result<Vec<CameraDeviceInfo>, String> {
    // dshow device enumeration spawns ffmpeg and can take a few hundred ms
    // (or several seconds if a webcam is slow to respond). Tauri runs sync
    // commands on the main thread, which froze the UI; move to a worker.
    tauri::async_runtime::spawn_blocking(get_camera_devices_blocking)
        .await
        .map_err(|e| format!("get_camera_devices join error: {e}"))?
}

fn get_camera_devices_blocking() -> Result<Vec<CameraDeviceInfo>, String> {
    // Use ffmpeg to list DirectShow video devices on Windows.
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
    let mut devices: Vec<CameraDeviceInfo> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    // Two output formats to handle:
    //   FFmpeg ≤6.x:   section header "DirectShow video devices" followed by lines
    //                  like `[dshow @ ...]  "Integrated Camera"`
    //   FFmpeg 7.x+:   no section headers, each device tagged inline:
    //                  `[dshow @ ...] "Integrated Camera" (video)`
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

        // Skip the `Alternative name "@device_pnp_..."` lines — those are the
        // raw PnP identifiers, not friendly names.
        if line.contains("Alternative name") {
            continue;
        }

        let has_video_tag = line.contains("(video)");
        let has_audio_tag = line.contains("(audio)");
        // A line is a video device if FFmpeg tagged it as such OR we're in
        // the legacy video section header and it isn't explicitly audio.
        let is_video_device = has_video_tag || (in_video_section && !has_audio_tag);
        if !is_video_device {
            continue;
        }

        // Extract device name between the first pair of double quotes.
        let Some(start) = line.find('"') else {
            continue;
        };
        let Some(end_rel) = line[start + 1..].find('"') else {
            continue;
        };
        let name = line[start + 1..start + 1 + end_rel].trim().to_string();
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.clone()) {
            let (status, status_message) = classify_camera_name(&name);
            devices.push(CameraDeviceInfo {
                id: name.clone(),
                name,
                status,
                status_message,
            });
        }
    }

    Ok(devices)
}

#[allow(dead_code)]
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
pub async fn validate_camera_source(device_id: String) -> Result<CameraValidationResult, String> {
    tauri::async_runtime::spawn_blocking(move || {
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

        let (status, status_message) = probe_camera_device_health(&device.id)
            .unwrap_or_else(|| (device.status.clone(), device.status_message.clone()));

        Ok(CameraValidationResult {
            id: device.id,
            name: device.name,
            status,
            status_message,
            probed_at_unix_ms,
        })
    })
    .await
    .map_err(|e| format!("validate_camera_source join error: {e}"))?
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
    use super::parse_camera_devices;

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

#[tauri::command]
pub fn open_file_location(path: String) -> Result<(), String> {
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
pub fn delete_file(path: String) -> Result<(), String> {
    let target = std::path::Path::new(&path);
    if !target.exists() {
        return Err("File not found".to_string());
    }
    if !target.is_file() {
        return Err("Path is not a file".to_string());
    }
    trash::delete(target).map_err(|e| format!("Could not move to trash: {e}"))?;
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
pub fn rename_file(path: String, new_name: String) -> Result<String, String> {
    let src = std::path::PathBuf::from(&path);
    if !src.exists() {
        return Err("File not found".to_string());
    }
    if !src.is_file() {
        return Err("Path is not a file".to_string());
    }

    let trimmed = new_name.trim();
    if trimmed.is_empty() {
        return Err("Name cannot be empty".to_string());
    }
    if trimmed.contains('/') || trimmed.contains('\\') || trimmed.contains("..") {
        return Err("Name cannot contain path separators".to_string());
    }
    // Basic Windows-illegal chars check.
    if trimmed
        .chars()
        .any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*'))
    {
        return Err("Name contains illegal characters".to_string());
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
        .ok_or_else(|| "Cannot determine parent directory".to_string())?;
    let dest = parent.join(&final_name);

    if dest == src {
        // No-op rename.
        return Ok(src.to_string_lossy().to_string());
    }
    if dest.exists() {
        return Err(format!("A file named \"{final_name}\" already exists"));
    }

    std::fs::rename(&src, &dest).map_err(|e| format!("Rename failed: {e}"))?;
    Ok(dest.to_string_lossy().to_string())
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
pub async fn diagnose_ffmpeg() -> Result<FfmpegDiagnostics, String> {
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
            // Hardware encoder is informational, not required.
            if table.contains("h264_nvenc") {
                present.push("h264_nvenc".to_string());
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
    .map_err(|e| format!("diagnose_ffmpeg join error: {e}"))?
}

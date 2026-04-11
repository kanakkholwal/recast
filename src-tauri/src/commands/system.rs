use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use tauri::{AppHandle, Manager, State};
use xcap::{Monitor, Window};

use super::ffmpeg::{encode_thumbnail_base64, make_thumbnail};
use serde::Serialize;

use super::types::{AppConfig, AppState, DisplayInfo, WindowInfo};

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

fn save_config(app: &AppHandle, config: &AppConfig) {
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

fn capture_monitor_thumbnail(monitor: &Monitor) -> Option<String> {
    let shot = monitor.capture_image().ok()?;
    encode_thumbnail_base64(&make_thumbnail(&shot))
}

fn capture_window_thumbnail(window: &Window) -> Option<String> {
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
pub fn get_displays() -> Result<Vec<DisplayInfo>, String> {
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
}

#[tauri::command]
pub fn get_windows() -> Result<Vec<WindowInfo>, String> {
    let windows = Window::all().map_err(|e| e.to_string())?;
    Ok(windows
        .iter()
        .filter(|window| {
            !window.is_minimized().unwrap_or(false)
                && !window.title().unwrap_or_default().is_empty()
        })
        .map(|window| WindowInfo {
            id: window.id().unwrap_or_default(),
            pid: window.pid().unwrap_or_default(),
            app_name: window.app_name().unwrap_or_default(),
            title: window.title().unwrap_or_default(),
            x: window.x().unwrap_or_default(),
            y: window.y().unwrap_or_default(),
            width: window.width().unwrap_or_default(),
            height: window.height().unwrap_or_default(),
            is_minimized: window.is_minimized().unwrap_or_default(),
            thumbnail: capture_window_thumbnail(window),
        })
        .collect())
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
            let Ok(device) = collection.Item(i) else { continue };

            let id = device
                .GetId()
                .ok()
                .and_then(|pwstr| pwstr.to_string().ok())
                .unwrap_or_default();

            // Use device friendly name from endpoint properties.
            let name = get_device_name(&device)
                .unwrap_or_else(|| format!("Microphone {}", i + 1));

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
    use windows::Win32::UI::Shell::PropertiesSystem::{IPropertyStore, PROPERTYKEY};
    use windows::core::GUID;

    unsafe {
        let store: IPropertyStore = device.OpenPropertyStore(windows::Win32::System::Com::STGM(0)).ok()?;
        // PKEY_Device_FriendlyName = {a45c254e-df1c-4efd-8020-67d146a850e0}, 14
        let key = PROPERTYKEY {
            fmtid: GUID::from_values(
                0xa45c254e, 0xdf1c, 0x4efd, [0x80, 0x20, 0x67, 0xd1, 0x46, 0xa8, 0x50, 0xe0],
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

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CameraDeviceInfo {
    pub id: String,
    pub name: String,
}

/// List available camera/video capture devices.
#[tauri::command]
pub fn get_camera_devices() -> Result<Vec<CameraDeviceInfo>, String> {
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
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
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
        let name = line[start + 1..start + 1 + end_rel].to_string();
        if name.is_empty() {
            continue;
        }
        if seen.insert(name.clone()) {
            devices.push(CameraDeviceInfo {
                id: name.clone(),
                name,
            });
        }
    }

    Ok(devices)
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
    if trimmed.chars().any(|c| matches!(c, '<' | '>' | ':' | '"' | '|' | '?' | '*')) {
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

    let parent = src.parent().ok_or_else(|| "Cannot determine parent directory".to_string())?;
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

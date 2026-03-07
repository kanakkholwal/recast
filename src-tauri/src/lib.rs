use std::io::Cursor;
use std::sync::Mutex;
use std::process::{Command, Child};
use std::env;
use tauri::State;
use serde::Serialize;
use xcap::{Monitor, Window};
use image::codecs::png::PngEncoder;
use image::{ImageEncoder, ColorType};
use base64::{Engine as _, engine::general_purpose};

const THUMBNAIL_WIDTH: u32 = 320;
const THUMBNAIL_HEIGHT: u32 = 180;

#[derive(Serialize, Clone)]
pub struct DisplayInfo {
    id: u32,
    name: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    is_primary: bool,
    thumbnail: Option<String>,
}

#[derive(Serialize, Clone)]
pub struct WindowInfo {
    id: u32,
    pid: u32,
    app_name: String,
    title: String,
    x: i32,
    y: i32,
    width: u32,
    height: u32,
    is_minimized: bool,
    thumbnail: Option<String>,
}

struct AppState {
    recording_process: Mutex<Option<Child>>,
    last_file_path: Mutex<Option<String>>,
}

/// Resize an RgbaImage into a normalized thumbnail, fitting within THUMBNAIL_WIDTH x THUMBNAIL_HEIGHT
/// and centering onto a transparent canvas. Mimics Cap's normalize_thumbnail_dimensions.
fn make_thumbnail(img: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return image::RgbaImage::from_pixel(
            THUMBNAIL_WIDTH,
            THUMBNAIL_HEIGHT,
            image::Rgba([0, 0, 0, 255]),
        );
    }

    let scale = (THUMBNAIL_WIDTH as f32 / w as f32)
        .min(THUMBNAIL_HEIGHT as f32 / h as f32)
        .max(f32::MIN_POSITIVE);

    let scaled_w = (w as f32 * scale).round().clamp(1.0, THUMBNAIL_WIDTH as f32) as u32;
    let scaled_h = (h as f32 * scale).round().clamp(1.0, THUMBNAIL_HEIGHT as f32) as u32;

    let resized = image::imageops::resize(
        img,
        scaled_w.max(1),
        scaled_h.max(1),
        image::imageops::FilterType::Triangle, // fast bilinear — good enough for thumbnails
    );

    let mut canvas = image::RgbaImage::from_pixel(
        THUMBNAIL_WIDTH,
        THUMBNAIL_HEIGHT,
        image::Rgba([10, 10, 12, 255]), // dark background matching the UI
    );

    let offset_x = (THUMBNAIL_WIDTH - scaled_w) / 2;
    let offset_y = (THUMBNAIL_HEIGHT - scaled_h) / 2;
    image::imageops::overlay(&mut canvas, &resized, offset_x as i64, offset_y as i64);
    canvas
}

/// Encode an RgbaImage to a base64 PNG data URI string
fn encode_thumbnail_base64(img: &image::RgbaImage) -> Option<String> {
    let mut png_data = Cursor::new(Vec::new());
    let encoder = PngEncoder::new(&mut png_data);
    encoder
        .write_image(
            img.as_raw(),
            img.width(),
            img.height(),
            ColorType::Rgba8.into(),
        )
        .ok()?;

    let b64 = general_purpose::STANDARD.encode(png_data.into_inner());
    Some(format!("data:image/png;base64,{}", b64))
}

/// Capture a screenshot of a monitor and return it as a base64 data URI
fn capture_monitor_thumbnail(monitor: &Monitor) -> Option<String> {
    let screenshot = monitor.capture_image().ok()?;
    let thumbnail = make_thumbnail(&screenshot);
    encode_thumbnail_base64(&thumbnail)
}

/// Capture a screenshot of a window and return it as a base64 data URI
fn capture_window_thumbnail(window: &Window) -> Option<String> {
    let screenshot = window.capture_image().ok()?;
    let thumbnail = make_thumbnail(&screenshot);
    encode_thumbnail_base64(&thumbnail)
}

#[tauri::command]
fn get_displays() -> Result<Vec<DisplayInfo>, String> {
    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    Ok(monitors.iter().map(|m| {
        let thumbnail = capture_monitor_thumbnail(m);
        DisplayInfo {
            id: m.id().unwrap_or_default(),
            name: m.name().unwrap_or_default(),
            x: m.x().unwrap_or_default(),
            y: m.y().unwrap_or_default(),
            width: m.width().unwrap_or_default(),
            height: m.height().unwrap_or_default(),
            is_primary: m.is_primary().unwrap_or_default(),
            thumbnail,
        }
    }).collect())
}

#[tauri::command]
fn get_windows() -> Result<Vec<WindowInfo>, String> {
    let windows = Window::all().map_err(|e| e.to_string())?;
    Ok(windows.iter().filter(|w| {
        let is_minimized = w.is_minimized().unwrap_or(false);
        let title = w.title().unwrap_or_default();
        !is_minimized && !title.is_empty()
    }).map(|w| {
        let thumbnail = capture_window_thumbnail(w);
        WindowInfo {
            id: w.id().unwrap_or_default(),
            pid: w.pid().unwrap_or_default(),
            app_name: w.app_name().unwrap_or_default(),
            title: w.title().unwrap_or_default(),
            x: w.x().unwrap_or_default(),
            y: w.y().unwrap_or_default(),
            width: w.width().unwrap_or_default(),
            height: w.height().unwrap_or_default(),
            is_minimized: w.is_minimized().unwrap_or_default(),
            thumbnail,
        }
    }).collect())
}

#[tauri::command]
fn start_recording(target_type: String, target_id: u32, state: State<'_, AppState>) -> Result<(), String> {
    let mut process_guard = state.recording_process.lock().unwrap();
    if process_guard.is_some() {
        return Err("Already recording".into());
    }

    let file_path = env::temp_dir().join(format!("trace_recording_{}.mp4", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()));
    let file_path_str = file_path.to_string_lossy().to_string();

    let mut args = vec![
        "-y".to_string(),
        "-f".to_string(), "gdigrab".to_string(),
        "-framerate".to_string(), "30".to_string(),
    ];

    if target_type == "window" {
        let windows = Window::all().map_err(|e| e.to_string())?;
        let window = windows.iter().find(|w| w.id().unwrap_or_default() == target_id)
            .ok_or_else(|| "Window not found".to_string())?;
        let title = window.title().unwrap_or_default();
        args.push("-i".to_string());
        args.push(format!("title={}", title));
    } else {
        let monitors = Monitor::all().map_err(|e| e.to_string())?;
        let monitor = monitors.iter().find(|m| m.id().unwrap_or_default() == target_id)
            .ok_or_else(|| "Display not found".to_string())?;

        let offset_x = monitor.x().unwrap_or_default();
        let offset_y = monitor.y().unwrap_or_default();
        let width = monitor.width().unwrap_or_default();
        let height = monitor.height().unwrap_or_default();

        args.push("-offset_x".to_string());
        args.push(offset_x.to_string());
        args.push("-offset_y".to_string());
        args.push(offset_y.to_string());
        args.push("-video_size".to_string());
        args.push(format!("{}x{}", width, height));
        args.push("-i".to_string());
        args.push("desktop".to_string());
    }

    args.extend(vec![
        "-c:v".to_string(), "libx264".to_string(),
        "-preset".to_string(), "ultrafast".to_string(),
        "-pix_fmt".to_string(), "yuv420p".to_string(),
        file_path_str.clone()
    ]);

    let child = Command::new("ffmpeg")
        .args(&args)
        .spawn()
        .map_err(|e| format!("Failed to start ffmpeg: {}", e))?;

    *process_guard = Some(child);
    *state.last_file_path.lock().unwrap() = Some(file_path_str);

    Ok(())
}

#[tauri::command]
fn stop_recording(state: State<'_, AppState>) -> Result<String, String> {
    let mut process_guard = state.recording_process.lock().unwrap();
    if let Some(mut child) = process_guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    } else {
        return Err("Not recording".into());
    }

    let path = state.last_file_path.lock().unwrap().clone().unwrap_or_default();
    Ok(path)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .manage(AppState {
        recording_process: Mutex::new(None),
        last_file_path: Mutex::new(None),
    })
    .invoke_handler(tauri::generate_handler![get_displays, get_windows, start_recording, stop_recording])
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }
      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}

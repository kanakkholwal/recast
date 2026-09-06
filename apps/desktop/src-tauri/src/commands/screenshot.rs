//! Full-resolution screenshots through capturekit so an agent can see on-screen state and decide what to do next.
//! Display and window shots are headless; the `app` shot goes through the running instance to target its own focused window.

use std::borrow::Cow;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use capturekit::{DisplayId, Rect, Target, WindowId};
use image::RgbaImage;
use serde::Serialize;

use super::ffmpeg::{encode_png_bytes, encode_thumbnail_base64};
use tauri::State;

use super::error::{AppError, AppResult};
use super::system::get_active_output_dir;
use crate::capture::{CaptureTarget, RegionRect};
use crate::AppState;

/// Longest-edge cap for agent-facing shots. Vision models downsample anyway and
/// a native 4K PNG is megabytes of wasted tokens, so bound it unless the caller
/// opts out with a full-resolution request (`max_edge == 0`).
pub const DEFAULT_MAX_EDGE: u32 = 1600;

/// What the caller wants out of a capture. `max_edge == 0` means native
/// resolution; any other value caps the longest side.
pub struct ShotOptions {
    /// Absolute path to write the PNG to. `None` picks a timestamped temp file.
    pub out: Option<PathBuf>,
    /// Longest-edge cap in pixels; `0` disables downscaling.
    pub max_edge: u32,
    /// Also embed a `data:image/png;base64,...` URI in the result.
    pub base64: bool,
}

/// The written screenshot plus the metadata an agent needs to act on it.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Screenshot {
    pub path: String,
    pub width: u32,
    pub height: u32,
    /// The captured surface: `display`, `window`, or `app`.
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base64: Option<String>,
    /// Whether the shot also reached the system clipboard. Absent when no copy
    /// was asked for, `false` when it was asked for and the OS refused.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copied_to_clipboard: Option<bool>,
}

/// Capture a whole monitor by its display id (from `displays list`).
pub fn capture_display(id: u64, opts: &ShotOptions) -> Result<Screenshot, String> {
    let img = crate::capture::grab(Target::Display(DisplayId(id))).map_err(|e| format!("{e:#}"))?;
    finalize(&img, "display", opts)
}

/// Capture one application window by its window id (from `windows list`).
pub fn capture_window(id: u64, opts: &ShotOptions) -> Result<Screenshot, String> {
    let img = crate::capture::grab(Target::Window(WindowId(id))).map_err(|e| format!("{e:#}"))?;
    finalize(&img, "window", opts)
}

/// Captures the region the overlay dragged out; `rect` is physical virtual-desktop pixels, resolved by the same pure functions the recorder uses.
/// The crop happens during acquisition, on the GPU where there is one, so pixels outside the selection are never read back.
pub fn capture_region(rect: RegionRect, opts: &ShotOptions) -> Result<Screenshot, String> {
    finalize(&grab_region_pixels(rect)?, "region", opts)
}

/// The selection's pixels, cropped during acquisition.
fn grab_region_pixels(rect: RegionRect) -> Result<RgbaImage, String> {
    let target = CaptureTarget::resolve_region(rect).map_err(|e| format!("{e:#}"))?;
    crate::capture::grab_region(
        Target::Display(DisplayId(target.display_id)),
        target.crop_relative_to_source(),
    )
    .map_err(|e| format!("{e:#}"))
}

/// Put a shot on the system clipboard, reporting rather than propagating a
/// refusal: the PNG on disk is the artifact, and a clipboard another process is
/// holding open should not turn a good capture into a failed one.
fn copy_to_clipboard(app: &tauri::AppHandle, img: &RgbaImage) -> bool {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    let image = tauri::image::Image::new(img.as_raw(), img.width(), img.height());
    match app.clipboard().write_image(&image) {
        Ok(()) => true,
        Err(err) => {
            log::warn!("screenshot clipboard copy failed: {err}");
            false
        }
    }
}

/// Captures the dragged region at native resolution and copies it to the clipboard, since the user is about to edit and export these pixels.
/// `spawn_blocking` because opening a source and reading a frame blocks, and a sync command runs on the thread WKWebView paints on.
#[tauri::command]
pub async fn capture_region_shot(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    rect: RegionRect,
) -> AppResult<Screenshot> {
    // Read the directory before the await: a state guard cannot cross one.
    let out = get_active_output_dir(&state)
        .join(SCREENSHOT_DIR)
        .join(screenshot_name());
    tauri::async_runtime::spawn_blocking(move || {
        let img = grab_region_pixels(rect)?;
        // Disk first: the durable artifact, and a clipboard write is not worth losing it over.
        let mut shot = finalize(
            &img,
            "region",
            &ShotOptions {
                out: Some(out),
                max_edge: 0,
                base64: false,
            },
        )?;
        shot.copied_to_clipboard = Some(copy_to_clipboard(&app, &img));
        Ok::<_, String>(shot)
    })
    .await
    .map_err(|e| AppError::msg(format!("capture_region_shot join error: {e}")))?
    .map_err(AppError::msg)
}

/// The overlay window's label, so a second trigger focuses the one already up
/// rather than stacking another transparent full-screen window over it.
const OVERLAY_LABEL: &str = "screenshot-region";

/// The recording region picker's window, which the source page listens to.
const PICKER_LABEL: &str = "region-picker";

/// Opens the region overlay across every display, sized in PHYSICAL pixels from capturekit's own enumeration.
/// The primary display's logical size would leave a second monitor unreachable and put an above-or-left origin in negative space no (0, 0) window covers.
pub fn open_region_overlay(app: &tauri::AppHandle) -> Result<(), String> {
    open_overlay(
        app,
        OVERLAY_LABEL,
        "/select-area?mode=screenshot",
        "Capture Area",
    )
}

/// Opens the recording region picker over the whole virtual desktop, sized here rather than in the frontend.
/// `window.screen` reports the primary display in logical points, which left a second monitor unselectable and put an above-or-left origin in uncovered negative space.
#[tauri::command]
pub async fn open_area_picker(app: tauri::AppHandle) -> Result<(), String> {
    open_overlay(&app, PICKER_LABEL, "/select-area", "Select Area")
}

/// Place a borderless overlay across every display, in PHYSICAL pixels.
fn open_overlay(app: &tauri::AppHandle, label: &str, url: &str, title: &str) -> Result<(), String> {
    use tauri::{Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder};

    if let Some(existing) = app.get_webview_window(label) {
        return existing.set_focus().map_err(|e| e.to_string());
    }
    let displays = capturekit::displays().map_err(|e| format!("{e:#}"))?;
    let bounds = crate::capture::virtual_bounds(&displays)
        .ok_or("no displays to put the capture overlay on")?;

    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(title)
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .resizable(false)
        .shadow(false)
        .build()
        .map_err(|e| e.to_string())?;

    // Physical pixels: the builder takes logical points, and that conversion is the Retina half-size bug.
    let placed = window
        .set_position(PhysicalPosition::new(bounds.x, bounds.y))
        .and_then(|()| window.set_size(PhysicalSize::new(bounds.width, bounds.height)))
        .and_then(|()| window.set_focus());
    if let Err(error) = placed {
        // Left alive it holds the label and covers the screen; a retry only focuses it.
        let _ = window.close();
        return Err(error.to_string());
    }
    Ok(())
}

/// Where shots land inside the output directory, beside the recordings rather
/// than mixed in with them.
const SCREENSHOT_DIR: &str = "Screenshots";

/// A filename a person can read in a file manager, and that sorts by when it was taken. Colons are illegal on Windows and awkward everywhere, so the time is dashed.
fn screenshot_name() -> String {
    let now = chrono::Local::now();
    format!("Recast {}.png", now.format("%Y-%m-%d %H-%M-%S"))
}

/// Captures Recast's own UI so an agent can read app state; targets the given label, else the focused window, else the first.
/// Crops the containing monitor during acquisition: no raise or focus steal and portable, at the cost of an overlapping window showing through.
pub fn capture_app_window(
    app: &tauri::AppHandle,
    label: Option<&str>,
    opts: &ShotOptions,
) -> Result<Screenshot, String> {
    let window = resolve_window(app, label)?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;

    let displays = capturekit::displays().map_err(|e| e.to_string())?;
    let frame = Rect::new(pos.x, pos.y, size.width, size.height);
    let display = crate::capture::display_at(&displays, frame.centre())
        .ok_or("could not find the monitor the app window is on")?;

    // A window can hang off its display, and a crop past the frame is refused.
    let region = frame
        .relative_to(&display.bounds)
        .fit_inside(&Rect::from_size(
            display.bounds.width,
            display.bounds.height,
        ))
        .ok_or("the app window is off screen")?;
    let img = crate::capture::grab_region(Target::Display(display.id), Some(region))
        .map_err(|e| format!("{e:#}"))?;
    finalize(&img, "app", opts)
}

/// The webview window an `app` shot targets: the named one, else the focused
/// one, else any window (deterministic first by label).
fn resolve_window(
    app: &tauri::AppHandle,
    label: Option<&str>,
) -> Result<tauri::WebviewWindow, String> {
    use tauri::Manager;
    if let Some(label) = label {
        return app
            .get_webview_window(label)
            .ok_or_else(|| format!("no app window labelled '{label}'"));
    }
    let mut windows: Vec<_> = app.webview_windows().into_iter().collect();
    if windows.is_empty() {
        return Err("the app has no open windows to capture".into());
    }
    windows.sort_by(|a, b| a.0.cmp(&b.0));
    if let Some((_, focused)) = windows
        .iter()
        .find(|(_, w)| w.is_focused().unwrap_or(false))
    {
        return Ok(focused.clone());
    }
    Ok(windows
        .into_iter()
        .next()
        .expect("non-empty checked above")
        .1)
}

/// Downscale (if requested), PNG-encode, write to disk, and build the result.
fn finalize(img: &RgbaImage, kind: &str, opts: &ShotOptions) -> Result<Screenshot, String> {
    let img = downscale(img, opts.max_edge);
    let path = match &opts.out {
        Some(p) => p.clone(),
        None => default_path(kind),
    };
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create {}: {e}", parent.display()))?;
    }
    let bytes = encode_png_bytes(&img).ok_or("failed to PNG-encode the screenshot")?;
    std::fs::write(&path, &bytes).map_err(|e| format!("write {}: {e}", path.display()))?;
    let base64 = if opts.base64 {
        encode_thumbnail_base64(&img)
    } else {
        None
    };
    Ok(Screenshot {
        path: path.to_string_lossy().into_owned(),
        width: img.width(),
        height: img.height(),
        kind: kind.to_string(),
        base64,
        copied_to_clipboard: None,
    })
}

/// Resize so the longest edge is at most `max_edge`. `0`, an already-small
/// image, or a degenerate dimension passes through untouched.
fn downscale(img: &RgbaImage, max_edge: u32) -> Cow<'_, RgbaImage> {
    let (w, h) = (img.width(), img.height());
    let (tw, th) = scaled_dims(w, h, max_edge);
    if (tw, th) == (w, h) {
        return Cow::Borrowed(img);
    }
    Cow::Owned(image::imageops::resize(
        img,
        tw,
        th,
        image::imageops::FilterType::Triangle,
    ))
}

/// Target dimensions after capping the longest edge to `max_edge` (0 = no cap),
/// preserving aspect ratio. Pure so it can be unit-tested without a display.
fn scaled_dims(w: u32, h: u32, max_edge: u32) -> (u32, u32) {
    let longest = w.max(h);
    if max_edge == 0 || longest <= max_edge || w == 0 || h == 0 {
        return (w, h);
    }
    let scale = max_edge as f32 / longest as f32;
    let sw = ((w as f32 * scale).round() as u32).max(1);
    let sh = ((h as f32 * scale).round() as u32).max(1);
    (sw, sh)
}

fn default_path(kind: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or_default();
    std::env::temp_dir().join(format!("recast-shot-{kind}-{ts}.png"))
}

/// Make a relative path absolute against the current working directory. The
/// `app` shot is written by the running app process, whose CWD differs from the
/// agent's, so a relative `--out` must be resolved on the CLI side first.
pub fn absolutize(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        return path;
    }
    std::env::current_dir()
        .map(|cwd| cwd.join(&path))
        .unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_dims_caps_the_longest_edge_and_keeps_aspect() {
        // 3840x2160 capped to 1600 => 1600x900.
        assert_eq!(scaled_dims(3840, 2160, 1600), (1600, 900));
    }

    #[test]
    fn scaled_dims_caps_a_portrait_by_its_height() {
        assert_eq!(scaled_dims(1080, 1920, 960), (540, 960));
    }

    #[test]
    fn scaled_dims_passes_through_when_already_within_the_cap() {
        assert_eq!(scaled_dims(1280, 720, 1600), (1280, 720));
    }

    #[test]
    fn scaled_dims_zero_cap_means_native_resolution() {
        assert_eq!(scaled_dims(3840, 2160, 0), (3840, 2160));
    }

    #[test]
    fn scaled_dims_never_collapses_to_zero() {
        assert_eq!(scaled_dims(2000, 1, 100), (100, 1));
    }

    #[test]
    fn absolutize_leaves_absolute_paths_untouched() {
        let abs = if cfg!(windows) {
            PathBuf::from(r"C:\tmp\shot.png")
        } else {
            PathBuf::from("/tmp/shot.png")
        };
        assert_eq!(absolutize(abs.clone()), abs);
    }
}

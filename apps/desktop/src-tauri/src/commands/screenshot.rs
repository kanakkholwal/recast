//! Full-resolution screenshots for the automation CLI, so an agent driving
//! Recast can see on-screen state and decide when a step is done or what to do
//! next. Reuses xcap (the same backend as the picker thumbnails), writes a PNG,
//! and returns its path plus dimensions. Display/window shots are headless (no
//! running app needed, like the enumeration verbs); the `app` shot goes through
//! the running instance so it can target its own focused window.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use image::RgbaImage;
use serde::Serialize;
use xcap::{Monitor, Window};

use super::ffmpeg::{encode_png_bytes, encode_thumbnail_base64};

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
}

/// Capture a whole monitor by its display id (from `displays list`).
pub fn capture_display(id: u32, opts: &ShotOptions) -> Result<Screenshot, String> {
    let monitor = Monitor::all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|m| m.id().unwrap_or_default() == id)
        .ok_or_else(|| format!("no display with id {id}"))?;
    let img = monitor.capture_image().map_err(|e| e.to_string())?;
    finalize(img, "display", opts)
}

/// Capture one application window by its window id (from `windows list`).
pub fn capture_window(id: u32, opts: &ShotOptions) -> Result<Screenshot, String> {
    let window = Window::all()
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|w| w.id().unwrap_or_default() == id)
        .ok_or_else(|| format!("no window with id {id}"))?;
    let img = window.capture_image().map_err(|e| e.to_string())?;
    finalize(img, "window", opts)
}

/// Capture Recast's own UI so an agent can read the current app state (which
/// screen is up, whether a dialog blocks, an error toast, the live timer).
///
/// Targets the given window label, else the focused Recast window, else the
/// first one. Captures the monitor the window sits on and crops to the window
/// rectangle: portable across all three OSes and non-intrusive (no raise or
/// focus steal). The tradeoff is that another window overlapping ours would show
/// through, but the app window is normally focused and on top when an agent asks.
pub fn capture_app_window(
    app: &tauri::AppHandle,
    label: Option<&str>,
    opts: &ShotOptions,
) -> Result<Screenshot, String> {
    let window = resolve_window(app, label)?;
    let pos = window.outer_position().map_err(|e| e.to_string())?;
    let size = window.outer_size().map_err(|e| e.to_string())?;

    let monitors = Monitor::all().map_err(|e| e.to_string())?;
    let center = (
        pos.x + size.width as i32 / 2,
        pos.y + size.height as i32 / 2,
    );
    let monitor = monitor_for_point(&monitors, center)
        .ok_or("could not find the monitor the app window is on")?;

    let full = monitor.capture_image().map_err(|e| e.to_string())?;
    let mon_origin = (
        monitor.x().unwrap_or_default(),
        monitor.y().unwrap_or_default(),
    );
    let cropped = crop_to_window(&full, mon_origin, pos.into(), (size.width, size.height));
    finalize(cropped, "app", opts)
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
fn finalize(img: RgbaImage, kind: &str, opts: &ShotOptions) -> Result<Screenshot, String> {
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
    })
}

/// Resize so the longest edge is at most `max_edge`. `0`, an already-small
/// image, or a degenerate dimension passes through untouched.
fn downscale(img: RgbaImage, max_edge: u32) -> RgbaImage {
    let (w, h) = (img.width(), img.height());
    let (tw, th) = scaled_dims(w, h, max_edge);
    if (tw, th) == (w, h) {
        return img;
    }
    image::imageops::resize(&img, tw, th, image::imageops::FilterType::Triangle)
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

/// Crop a full-monitor image to a window rectangle expressed in virtual-desktop
/// physical pixels. Clamps to the image so an off-screen or oversized window
/// can't panic the crop.
fn crop_to_window(
    monitor_img: &RgbaImage,
    monitor_origin: (i32, i32),
    window_pos: (i32, i32),
    window_size: (u32, u32),
) -> RgbaImage {
    let (mw, mh) = (monitor_img.width(), monitor_img.height());
    let x = (window_pos.0 - monitor_origin.0).clamp(0, mw as i32) as u32;
    let y = (window_pos.1 - monitor_origin.1).clamp(0, mh as i32) as u32;
    let w = window_size.0.min(mw.saturating_sub(x)).max(1);
    let h = window_size.1.min(mh.saturating_sub(y)).max(1);
    image::imageops::crop_imm(monitor_img, x, y, w, h).to_image()
}

/// The first monitor whose bounds contain `point`, else the primary, else the
/// first available. Pure over the monitor rects so it is unit-testable.
fn monitor_for_point(monitors: &[Monitor], point: (i32, i32)) -> Option<&Monitor> {
    monitors
        .iter()
        .find(|m| {
            rect_contains(
                m.x().unwrap_or_default(),
                m.y().unwrap_or_default(),
                m.width().unwrap_or_default(),
                m.height().unwrap_or_default(),
                point,
            )
        })
        .or_else(|| monitors.iter().find(|m| m.is_primary().unwrap_or(false)))
        .or_else(|| monitors.first())
}

fn rect_contains(x: i32, y: i32, w: u32, h: u32, point: (i32, i32)) -> bool {
    point.0 >= x && point.0 < x + w as i32 && point.1 >= y && point.1 < y + h as i32
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
    fn rect_contains_is_half_open_on_the_far_edge() {
        assert!(rect_contains(0, 0, 100, 100, (0, 0)));
        assert!(rect_contains(0, 0, 100, 100, (99, 99)));
        // The right/bottom edge belongs to the next monitor, not this one.
        assert!(!rect_contains(0, 0, 100, 100, (100, 50)));
        assert!(!rect_contains(0, 0, 100, 100, (50, 100)));
    }

    #[test]
    fn rect_contains_handles_a_negative_origin_monitor() {
        // A left-of-primary monitor at x = -1920.
        assert!(rect_contains(-1920, 0, 1920, 1080, (-1000, 500)));
        assert!(!rect_contains(-1920, 0, 1920, 1080, (10, 500)));
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

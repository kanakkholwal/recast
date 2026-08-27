use core::time::Duration;

use capturekit_core::{
    CaptureError, Display, DisplayId, Rect, Result, Rotation, Window, WindowId,
};
use objc2::rc::Retained;
use objc2_core_graphics::{
    CGDisplayCopyDisplayMode, CGDisplayMode, CGDisplayRotation, CGMainDisplayID,
};
use objc2_foundation::NSError;
use objc2_screen_capture_kit::{SCDisplay, SCShareableContent, SCWindow};

pub(crate) const BACKEND: &str = "screencapturekit";

/// How long to wait for the content list before deciding the daemon is wedged.
const CONTENT_TIMEOUT: Duration = Duration::from_secs(5);

fn failed(message: String) -> CaptureError {
    CaptureError::backend(BACKEND, std::io::Error::other(message))
}

fn error_text(error: *mut NSError) -> String {
    if error.is_null() {
        return "ScreenCaptureKit returned no content and no error".into();
    }
    // SAFETY: non-null and owned by the caller for the length of the handler.
    let error = unsafe { &*error };
    error.localizedDescription().to_string()
}

/// Fetch the shareable content, blocking until ScreenCaptureKit answers.
///
/// **Never call this from the main thread.** ScreenCaptureKit answers on an
/// internal queue, but the first call also triggers the TCC prompt, which needs
/// the main run loop: blocking it here would deadlock the prompt against the
/// wait.
fn shareable_content() -> Result<Retained<SCShareableContent>> {
    let (sender, receiver) = std::sync::mpsc::channel::<core::result::Result<usize, String>>();
    let handler = block2::RcBlock::new(
        move |content: *mut SCShareableContent, error: *mut NSError| {
            let message = if content.is_null() {
                Err(error_text(error))
            } else {
                // Retained here and released by the receiver: the handler's own
                // reference dies with its autorelease pool, which drains long
                // before the waiting thread wakes.
                //
                // SAFETY: non-null, and ScreenCaptureKit guarantees the object
                // is live for the duration of this handler.
                let retained = unsafe { Retained::retain(content) };
                match retained {
                    Some(retained) => Ok(Retained::into_raw(retained) as usize),
                    None => Err("ScreenCaptureKit handed back a dead content list".into()),
                }
            };
            let _ = sender.send(message);
        },
    );

    unsafe { SCShareableContent::getShareableContentWithCompletionHandler(&handler) };

    match receiver.recv_timeout(CONTENT_TIMEOUT) {
        Ok(Ok(pointer)) => {
            // SAFETY: a +1 reference the handler transferred to this thread.
            unsafe { Retained::from_raw(pointer as *mut SCShareableContent) }
                .ok_or_else(|| failed("the content list was released before it arrived".into()))
        }
        Ok(Err(message)) => Err(match message.contains("declined") || message.contains("permission")
        {
            true => CaptureError::PermissionDenied(capturekit_core::PermissionKind::Screen),
            false => failed(message),
        }),
        Err(_) => Err(CaptureError::Timeout(CONTENT_TIMEOUT)),
    }
}

/// The display's real resolution and refresh, which `SCDisplay` reports in
/// points rather than pixels.
fn physical_mode(id: u32) -> Option<(u32, u32, f32)> {
    let mode = CGDisplayCopyDisplayMode(id)?;
    let width = CGDisplayMode::pixel_width(Some(&mode)) as u32;
    let height = CGDisplayMode::pixel_height(Some(&mode)) as u32;
    let refresh = CGDisplayMode::refresh_rate(Some(&mode)) as f32;
    Some((width, height, refresh))
}

fn rotation_of(id: u32) -> Rotation {
    match CGDisplayRotation(id).round() as i32 {
        90 => Rotation::Cw90,
        180 => Rotation::Cw180,
        270 => Rotation::Cw270,
        _ => Rotation::None,
    }
}

fn display_of(sc: &SCDisplay) -> Display {
    let id = unsafe { sc.displayID() };
    let frame = unsafe { sc.frame() };
    let points = (frame.size.width as u32, frame.size.height as u32);
    let (width, height, refresh) = physical_mode(id).unwrap_or((points.0, points.1, 0.0));
    // Backing scale, derived rather than assumed: an external 1x display beside
    // a Retina panel has a different one, and a fixed 2.0 would be wrong on both.
    let scale = if points.0 > 0 {
        width as f32 / points.0 as f32
    } else {
        1.0
    };
    Display {
        id: DisplayId(u64::from(id)),
        name: format!("Display {id}"),
        bounds: Rect::new(
            (frame.origin.x * f64::from(scale)).round() as i32,
            (frame.origin.y * f64::from(scale)).round() as i32,
            width,
            height,
        ),
        scale_factor: scale,
        refresh_hz: (refresh > 1.0).then_some(refresh),
        is_primary: id == CGMainDisplayID(),
        rotation: rotation_of(id),
    }
}

/// The display holding the middle of `bounds`, or the primary one.
fn display_for(bounds: &Rect, displays: &[Display]) -> DisplayId {
    let centre = Rect::new(
        bounds.x + (bounds.width / 2) as i32,
        bounds.y + (bounds.height / 2) as i32,
        1,
        1,
    );
    displays
        .iter()
        .find(|display| display.bounds.intersect(&centre).is_some())
        .or_else(|| displays.iter().find(|display| display.is_primary))
        .map(|display| display.id)
        .unwrap_or(DisplayId(0))
}

fn window_of(sc: &SCWindow, displays: &[Display], scale: f32) -> Window {
    let frame = unsafe { sc.frame() };
    let bounds = Rect::new(
        (frame.origin.x * f64::from(scale)).round() as i32,
        (frame.origin.y * f64::from(scale)).round() as i32,
        (frame.size.width * f64::from(scale)).round() as u32,
        (frame.size.height * f64::from(scale)).round() as u32,
    );
    let on_screen = unsafe { sc.isOnScreen() };
    Window {
        id: WindowId(u64::from(unsafe { sc.windowID() })),
        title: unsafe { sc.title() }
            .map(|title| title.to_string())
            .unwrap_or_default(),
        app_name: unsafe { sc.owningApplication() }
            .map(|app| unsafe { app.applicationName() }.to_string())
            .unwrap_or_default(),
        display: display_for(&bounds, displays),
        bounds,
        // ScreenCaptureKit does not report minimisation directly; a window that
        // is not on screen is one there is nothing to capture from.
        is_minimized: !on_screen,
        is_on_screen: on_screen,
    }
}

pub(crate) fn displays() -> Result<Vec<Display>> {
    let content = shareable_content()?;
    Ok(unsafe { content.displays() }
        .iter()
        .map(|display| display_of(&display))
        .collect())
}

pub(crate) fn windows() -> Result<Vec<Window>> {
    let content = shareable_content()?;
    let displays: Vec<Display> = unsafe { content.displays() }
        .iter()
        .map(|display| display_of(&display))
        .collect();
    // Window frames are in the global point space, so one scale serves them all;
    // the primary display defines it, as the window server does.
    let scale = displays
        .iter()
        .find(|display| display.is_primary)
        .map_or(1.0, |display| display.scale_factor);

    Ok(unsafe { content.windows() }
        .iter()
        .map(|window| window_of(&window, &displays, scale))
        .filter(|window| !window.title.is_empty())
        .collect())
}

/// The `SCDisplay` matching `id`, which a capture filter needs by object.
pub(crate) fn sc_display(id: DisplayId) -> Result<(Retained<SCDisplay>, Display)> {
    let content = shareable_content()?;
    let found = unsafe { content.displays() }
        .iter()
        .find(|display| u64::from(unsafe { display.displayID() }) == id.0)
        .ok_or(CaptureError::NotFound {
            kind: "display",
            id: id.0,
        })?;
    let described = display_of(&found);
    Ok((found, described))
}

/// The `SCWindow` matching `id`.
pub(crate) fn sc_window(id: WindowId) -> Result<(Retained<SCWindow>, Window)> {
    let content = shareable_content()?;
    let displays: Vec<Display> = unsafe { content.displays() }
        .iter()
        .map(|display| display_of(&display))
        .collect();
    let scale = displays
        .iter()
        .find(|display| display.is_primary)
        .map_or(1.0, |display| display.scale_factor);
    let found = unsafe { content.windows() }
        .iter()
        .find(|window| u64::from(unsafe { window.windowID() }) == id.0)
        .ok_or(CaptureError::NotFound {
            kind: "window",
            id: id.0,
        })?;
    let described = window_of(&found, &displays, scale);
    Ok((found, described))
}

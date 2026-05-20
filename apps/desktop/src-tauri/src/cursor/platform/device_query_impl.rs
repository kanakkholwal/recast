//! Cursor sampling for macOS and Linux via the `device_query` crate.
//!
//! Why one file for two platforms: `device_query` is itself the
//! abstraction. On macOS it queries CoreGraphics
//! (`CGEventSource::button_state` + the global mouse location); on Linux
//! it opens an xcb connection against `$DISPLAY` and uses
//! `XQueryPointer`. Wrapping it twice with platform-named files would
//! just be ceremony for identical code.
//!
//! ## Wayland caveat
//!
//! On a Wayland session `device_query` reaches the pointer through
//! XWayland, which is present on every mainstream distro that ships
//! Wayland (GNOME, KDE, Sway with the X11-compat option, etc.). The
//! coordinate space is XWayland's, which usually matches the compositor
//! 1:1 — but under HiDPI / fractional scaling it may differ from the
//! portal-captured frame's coordinate system, causing the editor's
//! stylized cursor to land near (but not exactly on) the user's actual
//! cursor. The screen capture itself uses `CursorMode::Embedded`, so the
//! recording always shows the cursor at the right pixel; only the
//! editor-side stylization is affected.
//!
//! A Wayland-native pointer source (libei or the PipeWire stream's
//! cursor metadata via `CursorMode::Metadata`) is the longer-term fix —
//! tracked in the cross-platform plan.
//!
//! ## Performance
//!
//! `DeviceState::new()` opens the underlying CoreGraphics/xcb handle.
//! At the 125 Hz cursor sample rate, reopening it per call would
//! dominate the cost — so we cache one per thread in a `thread_local!`.
//! The cursor capture spawns a single thread (`recast-cursor`), so this
//! is effectively a one-time setup.
//!
//! ## Behavioural differences vs. the Windows backend
//!
//! - **No cursor visibility.** `device_query` does not surface
//!   `CGCursorIsVisible` / the X11 hide state, so we report
//!   `visible: true` unconditionally. The cursor-capture loop's
//!   frame-bounds check (`cursor::spawn_cursor_capture`) still hides the
//!   cursor when it leaves the recorded area, so the editor behaviour
//!   stays correct for the common case (cursor off-frame).
//! - **No middle-button signal.** The Windows backend tracks left and
//!   right only, so this matches — middle-button clicks won't show in
//!   either backend's track.

use std::cell::RefCell;

use device_query::{DeviceQuery, DeviceState};

use crate::cursor::CursorState;

thread_local! {
    static DEVICE_STATE: RefCell<DeviceState> = RefCell::new(DeviceState::new());
}

pub fn sample_cursor_state() -> Option<CursorState> {
    DEVICE_STATE.with(|cell| {
        let state = cell.borrow();
        let mouse = state.get_mouse();
        // `button_pressed` follows the X11 button numbering even on
        // macOS (the crate normalises): 1 = left, 2 = middle, 3 = right
        // (4/5 are scroll, 6+ are side buttons). Indexes that don't
        // exist on a given platform are absent rather than `false`, so
        // we use `.get()` to coerce a missing entry into "not pressed".
        let left_down = mouse.button_pressed.get(1).copied().unwrap_or(false);
        let right_down = mouse.button_pressed.get(3).copied().unwrap_or(false);
        Some(CursorState {
            x: mouse.coords.0 as i32,
            y: mouse.coords.1 as i32,
            visible: true,
            left_down,
            right_down,
        })
    })
}

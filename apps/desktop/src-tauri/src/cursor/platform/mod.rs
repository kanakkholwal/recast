#[cfg(windows)]
mod windows;

// macOS and Linux share one impl backed by the `device_query` crate,
// which wraps CoreGraphics and xcb respectively. Keeping them on a
// single file avoids two near-identical wrappers and one shared module
// for one shared abstraction.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod device_query_impl;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod fallback;

use super::CursorState;

/// Sample the current cursor position and button state from the OS.
/// Returns `None` if the cursor state cannot be determined on this platform.
pub fn sample_cursor_state() -> Option<CursorState> {
    #[cfg(windows)]
    {
        windows::sample_cursor_state()
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        device_query_impl::sample_cursor_state()
    }
    #[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
    {
        fallback::sample_cursor_state()
    }
}

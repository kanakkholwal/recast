#[cfg(windows)]
mod windows;

#[cfg(not(windows))]
mod fallback;

use super::CursorState;

/// Sample the current cursor position and button state from the OS.
/// Returns `None` if the cursor state cannot be determined on this platform.
pub fn sample_cursor_state() -> Option<CursorState> {
    #[cfg(windows)]
    {
        windows::sample_cursor_state()
    }
    #[cfg(not(windows))]
    {
        fallback::sample_cursor_state()
    }
}

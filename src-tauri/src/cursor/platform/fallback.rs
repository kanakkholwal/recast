use crate::cursor::CursorState;

/// Stub cursor sampling for platforms without native cursor APIs yet.
/// Returns `None` — cursor tracking will produce an empty track.
pub fn sample_cursor_state() -> Option<CursorState> {
    // TODO: Implement for macOS (CGEvent / NSEvent) and Linux (X11/Wayland)
    None
}

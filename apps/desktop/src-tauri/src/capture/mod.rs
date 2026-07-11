pub mod platform;

use std::time::Duration;

use anyhow::Result;

use crate::recording::CaptureTarget;

/// Platform-independent frame capture interface.
///
/// Each platform implements this trait to provide screen frame data
/// as raw pixel buffers (BGRA8 format).
pub trait CaptureSource: Send {
    /// Capture the next frame within the given timeout.
    /// Returns `Ok(None)` if no new frame is available within the timeout.
    /// Returns `Ok(Some(bytes))` with raw BGRA8 pixel data on success.
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<Vec<u8>>>;

    /// Width of the captured frames in pixels.
    fn width(&self) -> u32;

    /// Height of the captured frames in pixels.
    fn height(&self) -> u32;

    /// Hint the recording frame rate. Sources that receive frames faster than
    /// the encoder consumes them — Windows Graphics Capture delivers on every
    /// window repaint — use this to throttle the expensive GPU→CPU readback down
    /// to the encode rate. A per-frame readback maps GPU memory (a GPU stall), so
    /// doing it hundreds of times a second stalls the whole GPU. Default: ignore
    /// (DXGI/xcap only produce a frame when the screen actually changes).
    fn set_target_fps(&mut self, _fps: u32) {}
}

/// Create the best available capture source for the current platform.
pub fn create_capture_source(target: &CaptureTarget) -> Result<Box<dyn CaptureSource>> {
    platform::create_source(target)
}

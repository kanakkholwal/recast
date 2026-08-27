use core::time::Duration;

use capturekit_core::{DirtyRects, Rect, Result, SourceDesc, Timestamp};

/// A frame as the backend holds it, before capturekit copies or wraps it.
pub(crate) struct RawFrame<'a> {
    /// When the source produced it, on the source's own clock.
    pub pts: Timestamp,
    /// Pixels, at the stride the source declared.
    pub bytes: &'a [u8],
    /// Bytes between rows.
    pub stride: u32,
    /// Regions that changed, empty when the backend cannot say.
    #[expect(dead_code, reason = "read by the streaming path, which lands in 5b")]
    pub dirty: DirtyRects,
}

/// What every platform implements, and the only thing the surfaces above it know.
///
/// One trait rather than one per surface is what makes a screenshot and a
/// recording the same acquisition: [`crate::shot`] and the streaming capturer both
/// drive this, so a fix to stale frames, cropping, permissions or colour lands on
/// both at once.
pub(crate) trait ScreenBackend: Send {
    /// What the backend actually negotiated, which is not always what was asked for.
    fn describe(&self) -> &SourceDesc;

    /// The region the backend cropped to during acquisition, if it did.
    ///
    /// Reported rather than inferred from the frame size: a region the same size
    /// as the display but offset would otherwise look like an uncropped frame and
    /// silently skip the host-side fallback.
    fn region(&self) -> Option<Rect> {
        None
    }

    /// Wait for the next frame.
    ///
    /// Push backends serve this from their delivery callback; pull backends poll.
    /// Returns [`capturekit_core::CaptureError::Timeout`] if none arrives.
    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>>;

    /// Release the source. Called on drop, and safe to call twice.
    #[expect(dead_code, reason = "called by the streaming path, which lands in 5b")]
    fn stop(&mut self) -> Result<()>;
}

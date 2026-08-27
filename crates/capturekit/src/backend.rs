use core::time::Duration;

use capturekit_core::{CursorSample, DirtyRects, Rect, Result, SourceDesc, Timestamp};

/// A frame as the backend holds it, before capturekit copies or wraps it.
pub(crate) struct RawFrame<'a> {
    /// When the source produced it, on the source's own clock.
    pub pts: Timestamp,
    /// Pixels, at the stride the source declared.
    pub bytes: &'a [u8],
    /// Bytes between rows.
    pub stride: u32,
    /// Regions that changed, empty when the backend cannot say.
    pub dirty: DirtyRects,
    /// Where the cursor was when this frame was produced, on the same clock.
    ///
    /// `None` from a backend whose `Capabilities::cursor_samples` is false.
    pub cursor: Option<CursorSample>,
}

/// A source of video frames, whatever produced them.
///
/// One trait rather than one per surface is what makes a screenshot, a screen
/// recording and a camera the same acquisition: [`crate::shot`], the streaming
/// capturer and the camera backends all drive this, so a fix to stale frames,
/// cropping, permissions or colour lands on every one of them at once.
pub(crate) trait FrameSource: Send {
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

    /// The cursor image most recently reported, if this backend reports one.
    fn cursor_shape(&self) -> Option<&capturekit_core::CursorShape> {
        None
    }

    /// Wait for the next frame.
    ///
    /// Push backends serve this from their delivery callback; pull backends poll.
    /// Returns [`capturekit_core::CaptureError::Timeout`] if none arrives.
    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>>;

    /// Release the source. Called on drop, and safe to call twice.
    fn stop(&mut self) -> Result<()>;
}

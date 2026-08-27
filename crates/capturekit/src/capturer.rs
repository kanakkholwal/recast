use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use capturekit_core::{
    CaptureError, ColorSpaceRequest, CursorSample, CursorShape, DirtyRects, ExclusionSupport,
    MonotonicClock, Pacing, Rect, Result, SourceDesc, Target, Timestamp, WindowId,
};

use crate::backend::FrameSource;
use crate::image::Image;
use crate::platform::{os, OpenOptions};
use crate::shot::{grab_one, CursorMode, ShotOptions};

/// What a frame handler tells the capture to do next.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Flow {
    /// Keep capturing.
    Continue,
    /// Tear the capture down.
    Stop,
}

/// One frame of a live capture, borrowed from the backend that produced it.
pub struct Frame<'a> {
    pts: Timestamp,
    bytes: &'a [u8],
    stride: u32,
    dirty: DirtyRects,
    cursor: Option<CursorSample>,
    desc: &'a SourceDesc,
}

impl Frame<'_> {
    /// When the source produced the frame, corrected to always move forward.
    #[must_use]
    pub const fn pts(&self) -> Timestamp {
        self.pts
    }

    /// The pixels, at the stride the driver laid them out with.
    #[must_use]
    pub const fn bytes(&self) -> &[u8] {
        self.bytes
    }

    /// Bytes between the start of one row and the next, padding included.
    #[must_use]
    pub const fn stride(&self) -> u32 {
        self.stride
    }

    /// Regions that changed. Empty means the whole frame must be assumed dirty.
    #[must_use]
    pub const fn dirty(&self) -> &DirtyRects {
        &self.dirty
    }

    /// Where the cursor was when this frame was produced.
    ///
    /// On the frame's own clock, not a separate poller's, so it needs no
    /// smoothing to line up with the video. `None` where the platform's
    /// [`crate::Capabilities::cursor_samples`] is false.
    #[must_use]
    pub const fn cursor(&self) -> Option<&CursorSample> {
        self.cursor.as_ref()
    }

    /// What the backend negotiated for this source.
    #[must_use]
    pub const fn desc(&self) -> &SourceDesc {
        self.desc
    }

    /// Copy the frame into an owned image.
    pub fn to_image(&self) -> Result<Image> {
        Image::new(
            self.bytes.to_vec(),
            self.desc.width,
            self.desc.height,
            self.stride,
            self.desc.format,
            self.desc.color_space,
            self.pts,
        )
    }
}

/// Configures a capture before it opens.
///
/// Options that only one platform honours are still typed here and documented as
/// ignored elsewhere, rather than hidden behind a lowest common denominator.
#[derive(Debug, Clone)]
pub struct CapturerBuilder {
    target: Target,
    opts: OpenOptions,
}

impl CapturerBuilder {
    pub(crate) fn new(target: Target) -> Self {
        Self {
            target,
            opts: OpenOptions::default(),
        }
    }

    /// Whether the cursor is composited into frames. All three platforms.
    #[must_use]
    pub fn cursor(mut self, cursor: CursorMode) -> Self {
        self.opts.cursor = cursor;
        self
    }

    /// Crop during acquisition, in the target's own coordinates.
    #[must_use]
    pub fn region(mut self, region: Option<Rect>) -> Self {
        self.opts.region = region;
        self
    }

    /// Frames per second to hold, repeating the last frame when the source
    /// produces nothing. Shorthand for [`CapturerBuilder::pacing`].
    #[must_use]
    pub fn frame_rate(self, fps: u32) -> Self {
        self.pacing(Pacing::Constant { fps })
    }

    /// How the output timeline relates to what the source produced.
    #[must_use]
    pub fn pacing(mut self, pacing: Pacing) -> Self {
        self.opts.pacing = pacing;
        self
    }

    /// Keep these windows out of the capture.
    ///
    /// Not best-effort. If the platform cannot honour the request,
    /// [`CapturerBuilder::build`] fails rather than capturing a window the caller
    /// asked to hide: exclusion is a privacy control, and silently ignoring one
    /// records exactly what the user was promised would be left out. Check
    /// [`crate::capabilities`] first to decide gracefully.
    #[must_use]
    pub fn exclude_windows(mut self, windows: &[WindowId]) -> Self {
        self.opts.exclude = windows.to_vec();
        self
    }

    /// The colour space to deliver, or `Auto` to take what the source reports.
    #[must_use]
    pub fn color_space(mut self, color_space: ColorSpaceRequest) -> Self {
        self.opts.color_space = color_space;
        self
    }

    /// Open the source.
    pub fn build(self) -> Result<Capturer> {
        check_exclusion(&self.opts.exclude)?;
        let backend = os::open(&self.target, &self.opts)?;
        let desc = backend.describe().clone();
        let clock = MonotonicClock::for_frame_rate(desc.frame_rate.unwrap_or(60));
        Ok(Capturer {
            backend,
            desc,
            clock,
        })
    }
}

/// Refuse an exclusion request this platform cannot meet.
fn check_exclusion(requested: &[WindowId]) -> Result<()> {
    if requested.is_empty() {
        return Ok(());
    }
    let capabilities = os::capabilities();
    let detail = match capabilities.exclusion {
        ExclusionSupport::AnyWindow => return Ok(()),
        // Ownership is checked by the backend, which is the only layer that can
        // ask the OS who owns a window.
        ExclusionSupport::OwnWindowsOnly => return Ok(()),
        ExclusionSupport::None => "this session gives a client no say in what the capture contains",
    };
    Err(CaptureError::ExclusionUnsupported {
        backend: capabilities.backend,
        requested: requested.len(),
        detail,
    })
}

/// A live capture, held open.
pub struct Capturer {
    backend: Box<dyn FrameSource>,
    desc: SourceDesc,
    clock: MonotonicClock,
}

impl Capturer {
    /// What the backend actually negotiated.
    #[must_use]
    pub const fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    /// Wait for the next frame.
    ///
    /// Timestamps are forced to advance here and not in the one-shot path: a
    /// stalled source is a pacing problem for a recording, but for a screenshot
    /// it is the signal that the frame is stale.
    pub fn next_frame(&mut self, timeout: Duration) -> Result<Frame<'_>> {
        let raw = self.backend.next_frame(timeout)?;
        let pts = self.clock.admit(raw.pts);
        // The cursor belongs to this frame, so it carries the frame's corrected
        // timestamp rather than the raw one the backend read. Desktop
        // Duplication reports the origin for a frame with no new content, which
        // the clock moves forward; a cursor left behind would then sit on a
        // different timeline from the pixels it was sampled with.
        let cursor = raw.cursor.map(|sample| CursorSample { pts, ..sample });
        Ok(Frame {
            pts,
            bytes: raw.bytes,
            stride: raw.stride,
            dirty: raw.dirty,
            cursor,
            desc: &self.desc,
        })
    }

    /// Take a still from the running capture, with the same warmup and cropping
    /// a standalone [`crate::shot`] would apply.
    pub fn snapshot(&mut self, opts: &ShotOptions) -> Result<Image> {
        grab_one(self.backend.as_mut(), opts, os::now())
    }

    /// The cursor image most recently reported, if the backend reports one.
    ///
    /// Kept off the frame because it changes rarely: a consumer decodes it once
    /// when [`CursorSample::shape_id`] changes rather than on every frame.
    #[must_use]
    pub fn cursor_shape(&self) -> Option<&CursorShape> {
        self.backend.cursor_shape()
    }

    /// How many timestamps the source reported out of order or repeated.
    #[must_use]
    pub const fn timestamp_corrections(&self) -> u64 {
        self.clock.corrections()
    }

    /// Release the source.
    pub fn stop(&mut self) -> Result<()> {
        self.backend.stop()
    }

    /// Run `handler` on a capture thread until it returns [`Flow::Stop`].
    ///
    /// The pull API above is the same stream; this only moves the loop off the
    /// caller's thread for backends that deliver faster than it can consume.
    pub fn start<H>(mut self, timeout: Duration, mut handler: H) -> CaptureHandle
    where
        H: FnMut(Frame<'_>) -> Flow + Send + 'static,
    {
        let stopping = Arc::new(AtomicBool::new(false));
        let flag = Arc::clone(&stopping);
        let join = std::thread::spawn(move || {
            while !flag.load(Ordering::Relaxed) {
                match self.next_frame(timeout) {
                    Ok(frame) => {
                        if handler(frame) == Flow::Stop {
                            break;
                        }
                    }
                    // A timeout is an idle desktop, not a failure.
                    Err(err) if err.is_recoverable() => continue,
                    Err(err) => {
                        let _ = self.stop();
                        return Err(err);
                    }
                }
            }
            self.stop()
        });
        CaptureHandle {
            stopping,
            join: Some(join),
        }
    }
}

/// A running capture thread.
pub struct CaptureHandle {
    stopping: Arc<AtomicBool>,
    join: Option<JoinHandle<Result<()>>>,
}

impl CaptureHandle {
    /// Ask the capture to stop and wait for it, returning what it ended with.
    pub fn stop(mut self) -> Result<()> {
        self.stopping.store(true, Ordering::Relaxed);
        match self.join.take() {
            Some(join) => join.join().unwrap_or(Ok(())),
            None => Ok(()),
        }
    }

    /// Whether the capture thread has already finished.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.join.as_ref().is_some_and(JoinHandle::is_finished)
    }
}

impl Drop for CaptureHandle {
    /// Stops the capture rather than detaching it, so a dropped handle cannot
    /// leave a duplication session holding the desktop open.
    fn drop(&mut self) {
        self.stopping.store(true, Ordering::Relaxed);
        if let Some(join) = self.join.take() {
            let _ = join.join();
        }
    }
}

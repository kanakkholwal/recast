use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Instant;

use capturekit_core::{
    CaptureError, ColorSpaceRequest, CursorSample, CursorShape, DirtyRects, ExclusionSupport,
    MonotonicClock, Pacer, Pacing, Rect, Result, SourceDesc, Target, Timestamp, WindowId,
};

use crate::backend::{FrameSource, RawFrame};
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
    repeat: bool,
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

    /// Whether this frame repeats the previous one to hold the paced rate.
    ///
    /// Under [`Pacing::Constant`] an idle source still owes a frame every slot.
    /// The pixels are the last ones the source produced, so an encoder can emit a
    /// duplicate rather than compressing them again. Always false under
    /// [`Pacing::Passthrough`], which invents nothing.
    #[must_use]
    pub const fn is_repeat(&self) -> bool {
        self.repeat
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

    /// How often frames are worth reading back to host memory, for a caller that
    /// paces itself and so leaves [`Pacing::Passthrough`] in place.
    ///
    /// Windows Graphics Capture delivers on every window repaint, far above any
    /// encode rate, and each readback maps GPU memory. Ignored where frames
    /// arrive in host memory already.
    #[must_use]
    pub fn readback_rate(mut self, fps: u32) -> Self {
        self.opts.readback_rate = Some(fps);
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
            pacer: self.opts.pacing.fps().map(Pacer::new),
            held: Held::default(),
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

/// The frame a paced capture last took from the source, kept so a slot the
/// source produced nothing for can still be filled.
///
/// Constant pacing copies every frame it emits. That is the price of being able
/// to repeat one: a backend's buffer is only valid until the next acquisition
/// unmaps it, so there is nothing left to repeat by the time the gap is known.
#[derive(Default)]
struct Held {
    bytes: Vec<u8>,
    stride: u32,
    dirty: DirtyRects,
    cursor: Option<CursorSample>,
    /// Whether the source produced this since the last slot was filled.
    fresh: bool,
    /// Whether anything has been taken from the source at all.
    filled: bool,
    repeats: u64,
}

impl Held {
    fn store(&mut self, raw: &RawFrame<'_>) {
        self.bytes.clear();
        self.bytes.extend_from_slice(raw.bytes);
        self.stride = raw.stride;
        self.dirty = raw.dirty.clone();
        self.cursor = raw.cursor;
        self.fresh = true;
        self.filled = true;
    }
}

/// A live capture, held open.
pub struct Capturer {
    backend: Box<dyn FrameSource>,
    desc: SourceDesc,
    clock: MonotonicClock,
    /// `None` under [`Pacing::Passthrough`], where the source sets the rate.
    pacer: Option<Pacer>,
    held: Held,
}

impl Capturer {
    /// What the backend actually negotiated.
    #[must_use]
    pub const fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    /// Wait for the next frame.
    ///
    /// Under [`Pacing::Constant`] this returns one frame per slot whatever the
    /// source did, repeating the last one over a gap. Under
    /// [`Pacing::Passthrough`] it returns only what the source produced.
    ///
    /// Timestamps are forced to advance here and not in the one-shot path: a
    /// stalled source is a pacing problem for a recording, but for a screenshot
    /// it is the signal that the frame is stale.
    pub fn next_frame(&mut self, timeout: Duration) -> Result<Frame<'_>> {
        if self.pacer.is_some() {
            let slot = self.wait_for_slot(timeout)?;
            return Ok(self.fill(slot));
        }
        let Self {
            backend,
            desc,
            clock,
            ..
        } = self;
        let raw = backend.next_frame(timeout)?;
        let pts = clock.admit(raw.pts);
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
            desc,
            repeat: false,
        })
    }

    /// Drain the source until a slot falls due, and report which slot.
    ///
    /// A slot is only taken from the pacer once there is a frame to put in it, so
    /// the pacer's count is the number of frames actually handed out and the
    /// timeline never gains a hole the caller was not told about.
    fn wait_for_slot(&mut self, timeout: Duration) -> Result<Timestamp> {
        let started = Instant::now();
        loop {
            if self.held.filled {
                if let Some(slot) = self.pacer.as_mut().and_then(|p| p.next_due(os::now())) {
                    return Ok(slot);
                }
            }
            let elapsed = started.elapsed();
            if elapsed >= timeout {
                return Err(CaptureError::Timeout(timeout));
            }
            let budget = timeout - elapsed;
            // Nothing held yet means nothing to repeat, so the first frame is
            // worth the whole budget however far off the slot is.
            let wait = if self.held.filled {
                self.until_slot(budget)
            } else {
                budget
            };
            let Self { backend, held, .. } = self;
            match backend.next_frame(wait) {
                Ok(raw) => held.store(&raw),
                // An idle source is what pacing exists to cover, so keep waiting
                // for the slot rather than passing a timeout to the caller.
                Err(err) if err.is_recoverable() => {}
                Err(err) => return Err(err),
            }
        }
    }

    /// How long until the next slot, never longer than what is left of `budget`.
    fn until_slot(&self, budget: Duration) -> Duration {
        match self.pacer.as_ref().and_then(Pacer::next_deadline) {
            Some(deadline) => budget.min(deadline.saturating_since(os::now())),
            None => budget,
        }
    }

    /// Put the held frame in `pts`, marking it a repeat if the source has
    /// produced nothing since the last slot.
    fn fill(&mut self, pts: Timestamp) -> Frame<'_> {
        let repeat = !self.held.fresh;
        self.held.fresh = false;
        self.held.repeats += u64::from(repeat);
        let cursor = self
            .held
            .cursor
            .map(|sample| CursorSample { pts, ..sample });
        Frame {
            pts,
            bytes: &self.held.bytes,
            stride: self.held.stride,
            // A repeat carries no damage of its own, and empty already means
            // "assume everything changed"; `is_repeat` is the signal to act on.
            dirty: if repeat {
                DirtyRects::unknown()
            } else {
                self.held.dirty.clone()
            },
            cursor,
            desc: &self.desc,
            repeat,
        }
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

    /// How many paced slots were filled with a repeat of the previous frame.
    ///
    /// A recording of a mostly idle desktop is nearly all repeats and that is
    /// correct. A rate that stays high while the screen is busy is the signal
    /// that the source cannot keep up with the pace it was asked for.
    #[must_use]
    pub const fn repeated_frames(&self) -> u64 {
        self.held.repeats
    }

    /// Slots abandoned because the capture stalled longer than the pacer's
    /// catch-up window, or 0 under [`Pacing::Passthrough`].
    #[must_use]
    pub fn skipped_frames(&self) -> u64 {
        self.pacer.as_ref().map_or(0, Pacer::skipped)
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

#[cfg(test)]
impl Capturer {
    /// Drive a synthetic source, so pacing can be tested without a display.
    pub(crate) fn from_source(backend: Box<dyn FrameSource>, pacing: Pacing) -> Self {
        let desc = backend.describe().clone();
        Self {
            clock: MonotonicClock::for_frame_rate(desc.frame_rate.unwrap_or(60)),
            backend,
            desc,
            pacer: pacing.fps().map(Pacer::new),
            held: Held::default(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::{MockFrame, MockSource};

    const SLOT_MS: u64 = 10;
    const FPS: u32 = (1_000 / SLOT_MS) as u32;
    /// Long enough that a timeout means pacing is broken, not that the box is busy.
    const PATIENT: Duration = Duration::from_secs(2);

    fn paced(frames: usize) -> Capturer {
        let frames = (0..frames)
            .map(|i| MockFrame::new(1_000 + i as i64, 0x10 + i as u8))
            .collect();
        Capturer::from_source(
            Box::new(MockSource::new(4, 4, frames)),
            Pacing::Constant { fps: FPS },
        )
    }

    #[test]
    fn a_slot_the_source_produced_nothing_for_repeats_the_frame_before_it() {
        let mut capturer = paced(1);
        let first = capturer.next_frame(PATIENT).expect("the source frame");
        assert!(!first.is_repeat());
        let fill = capturer.next_frame(PATIENT).expect("a filled slot");
        assert!(fill.is_repeat());
        assert_eq!(fill.bytes()[0], 0x10, "the repeat lost the pixels");
        assert_eq!(capturer.repeated_frames(), 1);
    }

    /// The point of constant pacing: slots land on the grid whatever the source
    /// did, so audio recorded alongside stays in step.
    #[test]
    fn paced_timestamps_sit_exactly_on_the_grid() {
        let mut capturer = paced(1);
        let mut stamps = Vec::new();
        for _ in 0..5 {
            stamps.push(
                capturer
                    .next_frame(PATIENT)
                    .expect("a slot")
                    .pts()
                    .as_nanos(),
            );
        }
        let interval = (SLOT_MS * 1_000_000) as i64;
        // On the grid, not adjacent on it: a stall costs whole slots, never phase.
        for pair in stamps.windows(2) {
            assert!(pair[1] > pair[0], "{stamps:?} did not advance");
        }
        for stamp in &stamps {
            let offset = stamp - stamps[0];
            assert_eq!(offset % interval, 0, "{stamps:?} left the grid");
        }
    }

    /// A source faster than the pace is not forwarded faster than the pace, and
    /// the slot gets the newest frame rather than the oldest one still queued.
    #[test]
    fn a_source_that_outruns_the_pace_is_drained_to_its_newest_frame() {
        let mut capturer = paced(64);
        let started = Instant::now();
        let first = capturer.next_frame(PATIENT).expect("a slot").bytes()[0];
        let second = capturer.next_frame(PATIENT).expect("a slot").bytes()[0];
        assert_eq!(first, 0x10, "the first slot ran ahead of the source");
        assert_eq!(second, 0x10 + 63, "a slot served a superseded frame");
        let owed = Duration::from_millis(SLOT_MS);
        assert!(
            started.elapsed() >= owed,
            "two slots took {:?}, less than the {owed:?} the pace owes",
            started.elapsed()
        );
        assert_eq!(capturer.repeated_frames(), 0, "the source was never idle");
    }

    #[test]
    fn passthrough_pacing_reports_an_idle_source_rather_than_inventing_a_frame() {
        let mut capturer = Capturer::from_source(
            Box::new(MockSource::new(4, 4, vec![MockFrame::new(1_000, 0x10)])),
            Pacing::Passthrough,
        );
        assert!(!capturer
            .next_frame(Duration::from_millis(20))
            .expect("the source frame")
            .is_repeat());
        let Err(err) = capturer.next_frame(Duration::from_millis(20)) else {
            panic!("passthrough invented a frame the source never produced");
        };
        assert!(err.is_recoverable(), "{err}");
        assert_eq!(capturer.repeated_frames(), 0);
    }

    /// A repeat carries no damage of its own. Reusing the previous frame's rects
    /// would tell an encoder that region changed again when nothing did.
    #[test]
    fn a_repeat_reports_unknown_damage_where_the_frame_it_copies_reported_rects() {
        let dirty = Rect::new(1, 1, 2, 2);
        let mut capturer = Capturer::from_source(
            Box::new(
                MockSource::new(4, 4, vec![MockFrame::new(1_000, 0x10)]).reporting_dirty(dirty),
            ),
            Pacing::Constant { fps: FPS },
        );
        assert_eq!(
            capturer
                .next_frame(PATIENT)
                .expect("a frame")
                .dirty()
                .as_slice(),
            [dirty]
        );
        assert!(capturer
            .next_frame(PATIENT)
            .expect("a filled slot")
            .dirty()
            .is_unknown());
    }

    /// The cursor belongs to the slot it is delivered in, not to the frame it was
    /// sampled with, or it drifts behind the pixels over every repeat.
    #[test]
    fn a_repeated_cursor_carries_the_timestamp_of_the_slot_it_fills() {
        let mut capturer = Capturer::from_source(
            Box::new(MockSource::new(4, 4, vec![MockFrame::new(1_000, 0x10)]).with_cursor((7, 9))),
            Pacing::Constant { fps: FPS },
        );
        capturer.next_frame(PATIENT).expect("a frame");
        let fill = capturer.next_frame(PATIENT).expect("a filled slot");
        let pts = fill.pts();
        let cursor = fill.cursor().expect("the cursor came with it");
        assert_eq!(cursor.position, Some((7, 9)));
        assert_eq!(cursor.pts, pts);
    }
}

//! The screen source: capturekit owns DXGI, WGC, ScreenCaptureKit, X11 and the Wayland portal.
//! This file only maps the app's [`CaptureTarget`] onto capturekit's [`Target`] and repacks frames for the encoder.

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use capturekit::{CaptureError, Capturer, DisplayId, Target, WindowId};

use super::{CaptureKind, CaptureTarget};
use super::{CaptureNotice, CaptureSource, CapturedFrame};
use crate::encoder::pack_rows;

/// Whether a window can be captured as its own surface rather than as a crop of the monitor it sits on.
/// Target resolution asks this before sizing a window target, so the resolved `source` always matches what the backend will actually deliver.
pub fn window_capture_supported() -> bool {
    capturekit::capabilities().window_capture
}

/// Where the source should leave its pixels.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum FrameMode {
    /// Read back to host memory, packed for an encoder that reads bytes.
    Host,
    /// Left on the GPU as a shared handle, for an encoder that takes textures.
    Gpu,
}

pub fn create_capture_source(
    target: &CaptureTarget,
    fps: u32,
    mode: FrameMode,
) -> Result<Box<dyn CaptureSource>> {
    Ok(Box::new(CapturekitSource::open(target, fps, mode)?))
}

/// How long to wait between reopen attempts after a loss.
/// The recording repeats its last frame meanwhile, so retrying at the pacer's rate only burns CPU and fills the log; a display returns from a mode change on a human timescale.
const REACQUIRE_INTERVAL: Duration = Duration::from_millis(500);

/// A live capturekit capture, reopened in place when the source is lost.
struct CapturekitSource {
    target: Target,
    fps: u32,
    mode: FrameMode,
    /// `None` between a loss and a successful reopen.
    capturer: Option<Capturer>,
    width: u32,
    height: u32,
    /// When another reopen is worth trying, or `None` once it is hopeless.
    retry_at: Option<Instant>,
    /// Pending notice for the recorder to forward to the user.
    notice: Option<CaptureNotice>,
    /// Whether the user was told frames stopped, so recovery is only announced
    /// to someone who saw the interruption.
    interrupted: bool,
}

/// Opens a capture that produces only what the source produced; pacing stays with the recording loop, which owns the wall-clock contract.
/// `readback_rate` is what a self-pacing caller still owes a push backend: WGC delivers on every repaint, and each readback maps GPU memory.
fn open_capturer(target: &Target, fps: u32, mode: FrameMode) -> Result<Capturer> {
    let mut builder = capturekit::capturer(target.clone())
        .readback_rate(fps)
        .gpu_handles(mode == FrameMode::Gpu);
    if mode == FrameMode::Gpu {
        // capturekit paces constantly by default, and its repeats carry no GPU handle.
        builder = builder.pacing(capturekit::Pacing::Passthrough);
    }
    builder
        .build()
        .with_context(|| format!("failed to open {} capture", target.kind_name()))
}

impl CapturekitSource {
    fn open(target: &CaptureTarget, fps: u32, mode: FrameMode) -> Result<Self> {
        let resolved = resolve(target);
        let capturer = open_capturer(&resolved, fps, mode)?;
        let desc = capturer.describe();
        Ok(Self {
            target: resolved,
            fps,
            mode,
            width: desc.width,
            height: desc.height,
            capturer: Some(capturer),
            retry_at: None,
            notice: None,
            interrupted: false,
        })
    }

    /// Reopen after a loss, at most once per [`REACQUIRE_INTERVAL`].
    /// A display that was unplugged, or a mode change, closes the backend; the recording repeats its last frame meanwhile rather than ending.
    fn reacquire(&mut self) {
        match self.retry_at {
            Some(at) if Instant::now() < at => return,
            None => return,
            Some(_) => {}
        }
        self.retry_at = Some(Instant::now() + REACQUIRE_INTERVAL);

        let capturer = match open_capturer(&self.target, self.fps, self.mode) {
            Ok(capturer) => capturer,
            Err(e) => {
                log::warn!("screen source could not be reopened: {e:#}");
                return;
            }
        };
        let desc = capturer.describe();
        // Fixed at open: a source back at another size cannot reach the encoder.
        if desc.width != self.width || desc.height != self.height {
            log::error!(
                "screen source reopened at {}x{}, but the recording is {}x{};                  the rest of it will repeat the last frame",
                desc.width,
                desc.height,
                self.width,
                self.height
            );
            self.retry_at = None;
            self.note(CaptureNotice::Ended(format!(
                "The display came back at {}x{}, but this recording is {}x{}, so it cannot continue.",
                desc.width, desc.height, self.width, self.height
            )));
            return;
        }
        self.capturer = Some(capturer);
        self.retry_at = None;
        if self.interrupted {
            self.interrupted = false;
            self.note(CaptureNotice::Resumed);
        }
    }

    /// Queue a notice, keeping a terminal one over any later interruption:
    /// once the recording cannot continue, that is the fact worth reporting.
    fn note(&mut self, notice: CaptureNotice) {
        if self.notice.as_ref().is_some_and(CaptureNotice::is_terminal) {
            return;
        }
        if notice.is_terminal() || matches!(notice, CaptureNotice::Interrupted(_)) {
            self.interrupted = true;
        }
        self.notice = Some(notice);
    }
}

impl CaptureSource for CapturekitSource {
    fn capture_next(&mut self, timeout: Duration) -> Result<Option<CapturedFrame>> {
        let Some(capturer) = self.capturer.as_mut() else {
            // Reopened or not, this tick has no frame; the loop repeats its last.
            self.reacquire();
            return Ok(None);
        };
        // The frame borrows the capturer, so it is taken and released first.
        let failure = match capturer.next_frame(timeout) {
            Ok(frame) => {
                let taken = match (self.mode, frame.gpu_handle()) {
                    (FrameMode::Gpu, Some(handle)) => CapturedFrame::Gpu(*handle),
                    // A silent host fallback is how a readback reached this path once.
                    (FrameMode::Gpu, None) => {
                        return Err(anyhow::anyhow!(
                            "the capture answered without a GPU handle although one was asked for"
                        ))
                    }
                    (FrameMode::Host, _) => CapturedFrame::Host(Arc::from(pack_rows(
                        frame.bytes(),
                        frame.stride(),
                        self.width,
                        self.height,
                    ))),
                };
                return Ok(Some(taken));
            }
            Err(e) => e,
        };
        match failure {
            // An idle desktop produces nothing, which is not a failure.
            CaptureError::Timeout(_) => Ok(None),
            other if other.is_recoverable() => {
                log::warn!("screen source lost, reopening: {other}");
                self.note(CaptureNotice::Interrupted(format!(
                    "Screen capture was interrupted ({other}). Trying to resume."
                )));
                self.capturer = None;
                self.retry_at = Some(Instant::now());
                self.reacquire();
                Ok(None)
            }
            other => {
                // Unplugging the display lands here; terminal stops the frame repeat.
                self.note(CaptureNotice::Ended(format!(
                    "Screen capture stopped: {other}. The recording was kept up to this point."
                )));
                self.capturer = None;
                Ok(None)
            }
        }
    }

    fn take_notice(&mut self) -> Option<CaptureNotice> {
        self.notice.take()
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

/// What capturekit should open for a resolved target; the ids ARE capturekit's, since it is the only enumerator.
/// A window it can no longer list falls back to the display the target recorded, which is what a window closed between the picker and the recording looks like.
fn resolve(target: &CaptureTarget) -> Target {
    if target.kind == CaptureKind::Window {
        return Target::Window(WindowId(target.id));
    }
    Target::Display(DisplayId(target.display_id))
}

#[cfg(test)]
mod tests {
    use super::super::CaptureArea;
    use super::*;

    fn target(kind: CaptureKind, id: u64, display_id: u64) -> CaptureTarget {
        CaptureTarget {
            kind,
            id,
            display_id,
            label: "test".into(),
            source: CaptureArea::from_size(1920, 1080),
            crop: CaptureArea::from_size(1920, 1080),
            scale_factor: 1.0,
        }
    }

    #[test]
    fn a_window_target_opens_the_window_itself() {
        let win = target(CaptureKind::Window, 42, 10);
        assert_eq!(resolve(&win), Target::Window(WindowId(42)));
    }

    /// A region is a rectangle of a display that the encoder crops later, so
    /// what gets opened is the display.
    #[test]
    fn a_region_opens_the_display_it_names() {
        let region = target(CaptureKind::Region, 10, 10);
        assert_eq!(resolve(&region), Target::Display(DisplayId(10)));
    }

    /// `id` is the window for a window target, so a display target must read
    /// `display_id` or a window and a display sharing a number would collide.
    #[test]
    fn a_display_target_opens_its_display_never_a_window() {
        let screen = target(CaptureKind::Display, 10, 10);
        assert_eq!(resolve(&screen), Target::Display(DisplayId(10)));
    }

    /// The adapter end to end against the real backend: open the primary display and read a frame of exactly the promised size.
    /// Skipped only where no display is listed, or under the Wayland portal whose display is a placeholder for a human's dialog choice.
    #[test]
    fn the_primary_display_opens_and_delivers_a_frame_of_the_promised_size() {
        if !capturekit::capabilities().display_enumeration {
            return;
        }
        let Ok(displays) = capturekit::displays() else {
            return;
        };
        let Some(primary) = displays.iter().find(|d| d.is_primary) else {
            return;
        };
        let source = target(CaptureKind::Display, primary.id.0, primary.id.0);
        let mut capture = CapturekitSource::open(&source, 60, FrameMode::Host)
            .expect("the primary display opens");
        assert_eq!(capture.width(), primary.bounds.width);
        assert_eq!(capture.height(), primary.bounds.height);

        // An idle desktop answers nothing, so this waits rather than polling once.
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        let frame = loop {
            if std::time::Instant::now() >= deadline {
                panic!("no frame within 5s from a display the platform listed");
            }
            if let Some(frame) = capture
                .capture_next(Duration::from_millis(200))
                .expect("capture did not fail")
            {
                break frame;
            }
        };
        let CapturedFrame::Host(bytes) = frame else {
            panic!("a host-mode source must not answer with a GPU handle");
        };
        assert_eq!(
            bytes.len(),
            capture.width() as usize * capture.height() as usize * 4,
            "a packed frame is exactly width * height * 4"
        );
    }
}

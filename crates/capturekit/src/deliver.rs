use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

#[cfg(target_os = "macos")]
use capturekit_core::AudioFormat;
use capturekit_core::{CaptureError, LostReason, Result, Timestamp};

/// What a video delivery published alongside the pixels.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Delivered {
    pub pts: Timestamp,
    pub stride: u32,
    /// Reported per frame: a stream can renegotiate its size mid-capture, and a description that does not follow makes every consumer read the wrong geometry out of a correct buffer.
    pub width: u32,
    pub height: u32,
}

#[derive(Default)]
struct Held<M> {
    meta: M,
    bytes: Vec<u8>,
    /// Bumped per delivery, so a waiter tells a new frame from the last one and
    /// a spurious wake cannot hand back a frame already returned.
    sequence: u64,
}

/// The handoff from a push backend's delivery thread to its consumer, shared by ScreenCaptureKit, PipeWire and Media Foundation.
/// Newest wins: a source that outruns the consumer overwrites the slot, since an undelivered older buffer is only latency. Generic over metadata.
#[derive(Default)]
pub(crate) struct Slot<M> {
    held: Mutex<Held<M>>,
    arrived: Condvar,
    /// Set when the source stops producing before the consumer stops reading.
    ended: AtomicBool,
}

/// Frames from a compositor or a camera.
pub(crate) type FrameSlot = Slot<Delivered>;

impl<M: Copy + Default> Slot<M> {
    /// Publish a buffer and wake the consumer. Never blocks on the consumer.
    pub(crate) fn publish(&self, meta: M, bytes: &[u8]) {
        let Ok(mut slot) = self.held.lock() else {
            return;
        };
        slot.bytes.clear();
        slot.bytes.extend_from_slice(bytes);
        slot.meta = meta;
        slot.sequence = slot.sequence.wrapping_add(1);
        self.arrived.notify_all();
    }

    /// Reports that no further frame will arrive and wakes whoever is waiting: the device was unplugged, the session closed, or another process took it.
    /// Without it a consumer times out repeatedly on a dead stream, which reads as a slow source rather than a finished one.
    pub(crate) fn end(&self) {
        self.ended.store(true, Ordering::Release);
        self.arrived.notify_all();
    }

    /// Waits for a buffer newer than `seen`; already-published buffers are delivered even after [`Slot::end`], since a finished stream still owes its last one.
    /// Swapped rather than copied, so the caller owns the pixels and the delivery thread gets the old buffer to refill: neither side allocates after the first frame.
    pub(crate) fn take(
        &self,
        timeout: Duration,
        seen: &mut u64,
        buffer: &mut Vec<u8>,
    ) -> Result<M> {
        let mut slot = self.held.lock().map_err(|_| lost())?;
        while slot.sequence == *seen {
            if self.ended.load(Ordering::Acquire) {
                return Err(lost());
            }
            let (next, waited) = self
                .arrived
                .wait_timeout(slot, timeout)
                .map_err(|_| lost())?;
            slot = next;
            if waited.timed_out() && slot.sequence == *seen {
                return Err(CaptureError::Timeout(timeout));
            }
        }
        *seen = slot.sequence;
        core::mem::swap(buffer, &mut slot.bytes);
        Ok(slot.meta)
    }
}

/// A push backend's audio handoff, which ACCUMULATES rather than keeping only the newest: a dropped audio buffer is an unreconstructable hole, not a repeated picture.
/// Samples are kept in contiguous runs ending where one was refused, so a hole is reported on the FOLLOWING run and nothing is spliced across a gap.
#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", feature = "pipewire-audio")
))]
#[derive(Default)]
pub(crate) struct AudioQueue {
    queued: Mutex<Queued>,
    arrived: Condvar,
    ended: AtomicBool,
}

/// One contiguous run of samples.
#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", feature = "pipewire-audio")
))]
struct Run {
    bytes: Vec<u8>,
    /// When the first sample in this run was captured.
    pts: Timestamp,
    /// Whether samples are missing between the previous run and this one.
    broken: bool,
}

#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", feature = "pipewire-audio")
))]
#[derive(Default)]
struct Queued {
    runs: std::collections::VecDeque<Run>,
    total: usize,
    /// Samples were refused, so whatever arrives next begins a new run.
    broke: bool,
    /// Deliveries dropped before they could be queued at all, and why the first
    /// one was. Counted rather than logged per buffer: the delivery callback
    /// runs hundreds of times a second, and a device whose format cannot be read
    /// fails on every one of them.
    dropped: u64,
    reason: Option<String>,
    /// What the delivered samples are actually in. A backend that configures
    /// its own format knows this up front; one that opens a device the system
    /// picked can only learn it from a buffer. PipeWire renegotiates through its
    /// own param callback, so only the AVFoundation input needs this.
    #[cfg(target_os = "macos")]
    format: Option<AudioFormat>,
}

#[cfg(any(
    target_os = "macos",
    all(target_os = "linux", feature = "pipewire-audio")
))]
impl AudioQueue {
    /// Queue samples captured at `pts` and wake the consumer.
    /// Never blocks the delivery thread: a consumer stalled past `capacity` costs the samples that do not fit, and the next run says so.
    pub(crate) fn publish(&self, pts: Timestamp, bytes: &[u8], capacity: usize) {
        let Ok(mut queued) = self.queued.lock() else {
            return;
        };
        if queued.total.saturating_add(bytes.len()) > capacity {
            // Refusing the new samples rather than evicting keeps what is queued intact; the caller asked for those first.
            queued.broke = true;
            return;
        }
        queued.total += bytes.len();
        // A run a refusal already ended can't take more samples: appending would splice them onto the far side of the hole.
        let continues = !queued.broke && !queued.runs.is_empty();
        if continues {
            if let Some(run) = queued.runs.back_mut() {
                run.bytes.extend_from_slice(bytes);
            }
        } else {
            let broken = core::mem::take(&mut queued.broke);
            queued.runs.push_back(Run {
                bytes: bytes.to_vec(),
                pts,
                broken,
            });
        }
        self.arrived.notify_all();
    }

    /// Record the format the delivered samples are in.
    #[cfg(target_os = "macos")]
    pub(crate) fn note_format(&self, format: AudioFormat) {
        if let Ok(mut queued) = self.queued.lock() {
            queued.format = Some(format);
        }
    }

    /// The format of the samples delivered so far, once any have been.
    #[cfg(target_os = "macos")]
    pub(crate) fn format(&self) -> Option<AudioFormat> {
        self.queued.lock().ok()?.format
    }

    /// Records that a delivery could not be queued, and why.
    /// The samples are gone either way; what must not happen is losing them silently, so the gap is marked for the next run and the reason kept for [`Self::take_drops`].
    pub(crate) fn note_dropped(&self, reason: &str) {
        let Ok(mut queued) = self.queued.lock() else {
            return;
        };
        queued.broke = true;
        queued.dropped = queued.dropped.saturating_add(1);
        if queued.reason.is_none() {
            queued.reason = Some(reason.to_string());
        }
    }

    /// [`Self::note_dropped`] with the error that caused it.
    pub(crate) fn note_dropped_with(&self, reason: &str, err: &CaptureError) {
        self.note_dropped(&format!("{reason}: {err}"));
    }

    /// Log anything dropped since the last call, once, with its reason.
    /// The samples are already gone; a track that is quietly short is the thing worth preventing.
    pub(crate) fn report_drops(&self, backend: &str) {
        if let Some((count, reason)) = self.take_drops() {
            log::warn!(
                "{backend}: dropped {count} audio buffer(s) — {reason}. The track is short by that much."
            );
        }
    }

    /// Take what has been dropped since the last ask, as a count and a reason.
    /// `None` when nothing was dropped, so a caller can report only when there is something to report.
    pub(crate) fn take_drops(&self) -> Option<(u64, String)> {
        let mut queued = self.queued.lock().ok()?;
        let reason = queued.reason.take()?;
        let count = core::mem::take(&mut queued.dropped);
        Some((count, reason))
    }

    /// Report that no further samples will arrive, and wake whoever is waiting.
    pub(crate) fn end(&self) {
        self.ended.store(true, Ordering::Release);
        self.arrived.notify_all();
    }

    /// Waits for samples, swapping the oldest contiguous run into `buffer`; the bool says whether samples are missing between the previous run and this one.
    /// Runs already queued are delivered even after [`AudioQueue::end`], since a stream that ends still owes what it captured.
    /// Waits until at least one run is queued, WITHOUT taking it. A caller that
    /// only needs the hardware's format must not consume audio to learn it: the
    /// run it took was dropped and the track started late by that much.
    // Only the macOS mic backend has to ask the hardware for its format before the first run; every other backend is told it on open.
    #[cfg_attr(not(target_os = "macos"), expect(dead_code))]
    pub(crate) fn wait_until_ready(&self, timeout: Duration) -> Result<()> {
        let mut queued = self.queued.lock().map_err(|_| lost())?;
        while queued.runs.is_empty() {
            if self.ended.load(Ordering::Acquire) {
                return Err(lost());
            }
            let (next, waited) = self
                .arrived
                .wait_timeout(queued, timeout)
                .map_err(|_| lost())?;
            queued = next;
            if waited.timed_out() && queued.runs.is_empty() {
                return Err(CaptureError::Timeout(timeout));
            }
        }
        Ok(())
    }

    pub(crate) fn take(
        &self,
        timeout: Duration,
        buffer: &mut Vec<u8>,
    ) -> Result<(Timestamp, bool)> {
        buffer.clear();
        let mut queued = self.queued.lock().map_err(|_| lost())?;
        while queued.runs.is_empty() {
            if self.ended.load(Ordering::Acquire) {
                return Err(lost());
            }
            let (next, waited) = self
                .arrived
                .wait_timeout(queued, timeout)
                .map_err(|_| lost())?;
            queued = next;
            if waited.timed_out() && queued.runs.is_empty() {
                return Err(CaptureError::Timeout(timeout));
            }
        }
        let Some(run) = queued.runs.pop_front() else {
            return Err(CaptureError::Timeout(timeout));
        };
        queued.total = queued.total.saturating_sub(run.bytes.len());
        *buffer = run.bytes;
        Ok((run.pts, run.broken))
    }
}

/// A slot that can be told its source stopped, whatever it carries.
/// One end-of-stream delegate then serves the video and audio slots alike, rather than one per metadata type.
#[cfg(target_os = "macos")]
pub(crate) trait Endable: Send + Sync {
    fn end(&self);
}

#[cfg(target_os = "macos")]
impl<M: Copy + Default + Send> Endable for Slot<M> {
    fn end(&self) {
        Self::end(self);
    }
}

#[cfg(target_os = "macos")]
impl Endable for AudioQueue {
    fn end(&self) {
        Self::end(self);
    }
}

/// A stream that ended, or a slot poisoned by a delivery thread that panicked
/// mid-publish. Neither is a timeout, and neither is fixed by waiting longer.
/// No further samples will arrive: the source ended, or the thread that feeds
/// this one panicked and poisoned the lock. Neither is worth retrying.
fn lost() -> CaptureError {
    CaptureError::Lost(LostReason::Ended)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn meta(pts: i64, width: u32) -> Delivered {
        Delivered {
            pts: Timestamp::from_nanos(pts),
            stride: width * 4,
            width,
            height: 2,
        }
    }

    #[test]
    fn a_published_frame_reaches_the_consumer_with_its_pixels() {
        let slot = FrameSlot::default();
        slot.publish(meta(100, 4), &[0xAB; 32]);
        let mut seen = 0;
        let mut buffer = Vec::new();
        let got = slot
            .take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect("the published frame");
        assert_eq!(got.pts, Timestamp::from_nanos(100));
        assert_eq!(buffer, vec![0xAB; 32]);
    }

    #[test]
    fn a_consumer_that_has_seen_the_only_frame_times_out_rather_than_repeating_it() {
        let slot = FrameSlot::default();
        slot.publish(meta(100, 4), &[0xAB; 32]);
        let mut seen = 0;
        let mut buffer = Vec::new();
        slot.take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect("the first frame");
        let err = slot
            .take(Duration::from_millis(10), &mut seen, &mut buffer)
            .expect_err("nothing new was published");
        assert!(err.is_recoverable(), "{err}");
    }

    /// The property the slot exists for: a source faster than its consumer must
    /// not build a backlog, so the frame that arrives is the newest published.
    #[test]
    fn a_backlog_collapses_to_the_newest_frame() {
        let slot = FrameSlot::default();
        for i in 1..=5u8 {
            slot.publish(meta(i64::from(i), 4), &[i; 32]);
        }
        let mut seen = 0;
        let mut buffer = Vec::new();
        let got = slot
            .take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect("a frame");
        assert_eq!(got.pts, Timestamp::from_nanos(5));
        assert_eq!(buffer[0], 5);
    }

    #[test]
    fn a_consumer_waiting_first_is_woken_by_a_later_delivery() {
        let slot = Arc::new(FrameSlot::default());
        let writer = Arc::clone(&slot);
        let publishing = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            writer.publish(meta(7, 4), &[0x11; 32]);
        });
        let mut seen = 0;
        let mut buffer = Vec::new();
        let got = slot
            .take(Duration::from_secs(2), &mut seen, &mut buffer)
            .expect("the delivery arrived while waiting");
        assert_eq!(got.pts, Timestamp::from_nanos(7));
        publishing.join().expect("the publisher finished");
    }

    /// A dead stream must not present as a slow one: a consumer told "timeout"
    /// retries forever on a source that will never produce again.
    #[test]
    fn a_stream_that_ended_is_reported_as_lost_rather_than_as_a_timeout() {
        let slot = FrameSlot::default();
        slot.end();
        let mut seen = 0;
        let mut buffer = Vec::new();
        let err = slot
            .take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect_err("the stream ended");
        assert!(
            !matches!(err, CaptureError::Timeout(_)),
            "an ended stream reported {err}"
        );
    }

    #[test]
    fn a_frame_published_before_the_end_is_still_delivered() {
        let slot = FrameSlot::default();
        slot.publish(meta(9, 4), &[0x5A; 32]);
        slot.end();
        let mut seen = 0;
        let mut buffer = Vec::new();
        let got = slot
            .take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect("the last frame is still owed");
        assert_eq!(got.pts, Timestamp::from_nanos(9));
        assert_eq!(buffer[0], 0x5A);
    }

    /// Ending wakes the waiter. Without the notify the consumer sits out its
    /// whole timeout before learning the stream is gone.
    #[test]
    fn a_consumer_already_waiting_is_woken_by_the_end_of_the_stream() {
        let slot = Arc::new(FrameSlot::default());
        let ending = Arc::clone(&slot);
        let ender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            ending.end();
        });
        let started = std::time::Instant::now();
        let mut seen = 0;
        let mut buffer = Vec::new();
        let err = slot
            .take(Duration::from_secs(30), &mut seen, &mut buffer)
            .expect_err("the stream ended while waiting");
        assert!(
            !matches!(err, CaptureError::Timeout(_)),
            "woken but reported {err}"
        );
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "the waiter sat out its timeout instead of being woken"
        );
        ender.join().expect("the ender finished");
    }

    /// The buffer goes back to the delivery thread rather than being dropped, so
    /// a steady stream stops allocating after the first frame.
    #[test]
    fn the_consumers_buffer_is_handed_back_for_the_next_delivery() {
        let slot = FrameSlot::default();
        let mut seen = 0;
        let mut buffer = Vec::with_capacity(64);
        slot.publish(meta(1, 4), &[1; 32]);
        slot.take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect("the first frame");
        let recycled = buffer.as_ptr();
        slot.publish(meta(2, 4), &[2; 32]);
        slot.take(Duration::from_millis(50), &mut seen, &mut buffer)
            .expect("the second frame");
        assert_eq!(buffer[0], 2);
        assert_ne!(
            buffer.as_ptr(),
            recycled,
            "the same buffer came back, so nothing was swapped"
        );
    }
}

#[cfg(all(
    test,
    any(
        target_os = "macos",
        all(target_os = "linux", feature = "pipewire-audio")
    )
))]
mod audio_queue_tests {
    use super::*;

    const ROOM: usize = 64;

    fn at(nanos: i64) -> Timestamp {
        Timestamp::from_nanos(nanos)
    }

    #[test]
    fn samples_accumulate_rather_than_the_newest_winning() {
        let queue = AudioQueue::default();
        queue.publish(at(0), &[1, 2], ROOM);
        queue.publish(at(10), &[3, 4], ROOM);
        let mut out = Vec::new();
        let (pts, broken) = queue
            .take(Duration::from_millis(50), &mut out)
            .expect("both buffers");
        assert_eq!(out, vec![1, 2, 3, 4], "samples were dropped");
        assert_eq!(pts, at(0), "the run starts at its first sample");
        assert!(!broken);
    }

    /// The bug this shape exists to prevent: the flag marking a hole was
    /// attached to the run BEFORE it, so a consumer saw an undeclared jump.
    #[test]
    fn a_hole_is_reported_on_the_run_after_it_not_the_one_before() {
        let queue = AudioQueue::default();
        queue.publish(at(0), &[1, 2], ROOM);
        // Refused: too big for what is left.
        queue.publish(at(10), &[9; ROOM], ROOM);
        queue.publish(at(99), &[7, 8], ROOM);

        let mut out = Vec::new();
        let (pts, broken) = queue
            .take(Duration::from_millis(50), &mut out)
            .expect("the run before the hole");
        assert_eq!(out, vec![1, 2]);
        assert_eq!(pts, at(0));
        assert!(!broken, "the run before a hole is not the broken one");

        let (pts, broken) = queue
            .take(Duration::from_millis(50), &mut out)
            .expect("the run after the hole");
        assert_eq!(out, vec![7, 8]);
        assert_eq!(pts, at(99));
        assert!(broken, "the hole was never declared");
    }

    /// Appending across a refusal would splice samples from the far side of a
    /// hole onto the near side, which reads as one continuous run that is not.
    #[test]
    fn samples_after_a_refusal_never_join_the_run_before_it() {
        let queue = AudioQueue::default();
        queue.publish(at(0), &[1], ROOM);
        queue.publish(at(5), &[9; ROOM], ROOM);
        queue.publish(at(50), &[2], ROOM);
        let mut out = Vec::new();
        queue
            .take(Duration::from_millis(50), &mut out)
            .expect("the first run");
        assert_eq!(
            out,
            vec![1],
            "the run swallowed samples from after the hole"
        );
    }

    #[test]
    fn a_refusal_frees_up_again_once_the_consumer_drains() {
        let queue = AudioQueue::default();
        queue.publish(at(0), &[1; ROOM], ROOM);
        queue.publish(at(10), &[2], ROOM);
        let mut out = Vec::new();
        queue
            .take(Duration::from_millis(50), &mut out)
            .expect("the full run");
        assert_eq!(out.len(), ROOM);
        queue.publish(at(20), &[3], ROOM);
        let (_, broken) = queue
            .take(Duration::from_millis(50), &mut out)
            .expect("room again");
        assert_eq!(out, vec![3]);
        assert!(broken, "the refusal while full was never declared");
    }

    #[test]
    fn an_empty_queue_times_out_rather_than_returning_nothing() {
        let queue = AudioQueue::default();
        let mut out = Vec::new();
        let err = queue
            .take(Duration::from_millis(10), &mut out)
            .expect_err("nothing was published");
        assert!(matches!(err, CaptureError::Timeout(_)), "{err}");
    }

    #[test]
    fn samples_queued_before_the_end_are_still_delivered() {
        let queue = AudioQueue::default();
        queue.publish(at(3), &[5, 6], ROOM);
        queue.end();
        let mut out = Vec::new();
        let (pts, _) = queue
            .take(Duration::from_millis(50), &mut out)
            .expect("the stream still owes these");
        assert_eq!(out, vec![5, 6]);
        assert_eq!(pts, at(3));
        let err = queue
            .take(Duration::from_millis(10), &mut out)
            .expect_err("and nothing after");
        assert!(!matches!(err, CaptureError::Timeout(_)), "{err}");
    }
}

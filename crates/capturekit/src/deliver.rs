use core::time::Duration;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex};

use capturekit_core::{CaptureError, LostReason, Result, Timestamp};

/// What a video delivery published alongside the pixels.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Delivered {
    pub pts: Timestamp,
    pub stride: u32,
    /// Reported per frame: a stream can renegotiate its size mid-capture, and a
    /// description that does not follow makes every consumer read the wrong
    /// geometry out of a correct buffer.
    pub width: u32,
    pub height: u32,
}

/// What an audio delivery published alongside the samples.
///
/// macOS and PipeWire push audio; WASAPI is polled, so Windows never builds one.
#[cfg(any(target_os = "macos", all(target_os = "linux", feature = "wayland")))]
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct DeliveredAudio {
    pub pts: Timestamp,
}

#[derive(Default)]
struct Held<M> {
    meta: M,
    bytes: Vec<u8>,
    /// Bumped per delivery, so a waiter tells a new frame from the last one and
    /// a spurious wake cannot hand back a frame already returned.
    sequence: u64,
}

/// The handoff from a push backend's delivery thread to its consumer.
///
/// Shared by every backend the OS calls rather than polls: ScreenCaptureKit,
/// PipeWire and Media Foundation all deliver on a thread of their own, all must
/// not block it, and all want the current buffer rather than a backlog.
///
/// Newest wins. A source that outruns the consumer overwrites the slot instead
/// of queueing, because an undelivered older buffer is only latency.
///
/// Generic over the metadata so audio does not carry a stride and a picture
/// size it has no use for.
#[derive(Default)]
pub(crate) struct Slot<M> {
    held: Mutex<Held<M>>,
    arrived: Condvar,
    /// Set when the source stops producing before the consumer stops reading.
    ended: AtomicBool,
}

/// Frames from a compositor or a camera.
pub(crate) type FrameSlot = Slot<Delivered>;
/// Samples from a system-audio or microphone stream.
#[cfg(any(target_os = "macos", all(target_os = "linux", feature = "wayland")))]
pub(crate) type AudioSlot = Slot<DeliveredAudio>;

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

    /// Report that no further frame will arrive, and wake whoever is waiting.
    ///
    /// The device was unplugged, the compositor closed the session, or another
    /// process took it. Without this a consumer times out over and over on a
    /// stream that will never produce again, which reads as a slow source rather
    /// than a dead one.
    pub(crate) fn end(&self) {
        self.ended.store(true, Ordering::Release);
        self.arrived.notify_all();
    }

    /// Wait for a buffer newer than `seen`, swapping it into `buffer`.
    ///
    /// Buffers already published are delivered even after [`Slot::end`]: a
    /// stream that ends still owes its last one.
    ///
    /// Swapped rather than copied: the caller ends up owning the pixels it
    /// reads and the delivery thread gets the previous buffer back to refill, so
    /// neither side allocates after the first frame and neither can reallocate
    /// under the other.
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

/// A slot that can be told its source stopped, whatever it carries.
///
/// One end-of-stream delegate then serves the video and audio slots alike,
/// rather than one per metadata type.
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

/// A stream that ended, or a slot poisoned by a delivery thread that panicked
/// mid-publish. Neither is a timeout, and neither is fixed by waiting longer.
fn lost() -> CaptureError {
    CaptureError::Lost(LostReason::AccessLost)
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

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;

/// A wall-clock timer that can be paused. `effective_elapsed` reports elapsed
/// time *minus* every interval spent paused, so all capture tracks (video
/// pacer, cursor, audio) stay on one gap-free timeline across pause/resume.
#[derive(Clone)]
pub struct RecordingClock {
    start: Instant,
    paused_total_us: Arc<AtomicU64>,
    paused_since: Arc<Mutex<Option<Instant>>>,
}

impl RecordingClock {
    pub fn new(start: Instant) -> Self {
        Self {
            start,
            paused_total_us: Arc::new(AtomicU64::new(0)),
            paused_since: Arc::new(Mutex::new(None)),
        }
    }

    /// The session origin every track measures its first sample from.
    pub const fn origin(&self) -> Instant {
        self.start
    }

    /// Wall-clock time since start, excluding all paused intervals.
    pub fn effective_elapsed(&self) -> Duration {
        let raw = self.start.elapsed();
        let banked = Duration::from_micros(self.paused_total_us.load(Ordering::Acquire));
        let live = self
            .paused_since
            .lock()
            .map(|since| since.elapsed())
            .unwrap_or_default();
        raw.saturating_sub(banked).saturating_sub(live)
    }

    pub fn is_paused(&self) -> bool {
        self.paused_since.lock().is_some()
    }

    /// Begin a pause interval. Idempotent.
    pub fn pause(&self) {
        let mut slot = self.paused_since.lock();
        if slot.is_none() {
            *slot = Some(Instant::now());
        }
    }

    /// End the current pause interval, banking its duration. Idempotent.
    pub fn resume(&self) {
        let mut slot = self.paused_since.lock();
        if let Some(since) = slot.take() {
            self.paused_total_us
                .fetch_add(since.elapsed().as_micros() as u64, Ordering::AcqRel);
        }
    }
}

/// `first_us` sentinel for "this track has not produced a sample yet". Real
/// offsets are bounded by the recording length, so the max value is free.
const UNSET: u64 = u64::MAX;

/// When a capture track produced its first sample, measured against the shared
/// session origin. Every track (screen, system audio, microphone, camera)
/// starts at its own unpredictable instant, so the *difference* between two
/// tracks' marks is the A/V offset that has to be corrected downstream.
///
/// Cheap to clone and lock-free: capture threads only ever call [`Self::mark`].
#[derive(Clone, Debug)]
pub struct TrackStart {
    origin: Instant,
    first_us: Arc<AtomicU64>,
}

impl TrackStart {
    pub fn new(origin: Instant) -> Self {
        Self {
            origin,
            first_us: Arc::new(AtomicU64::new(UNSET)),
        }
    }

    /// Record *now* as this track's first sample. Only the first call takes
    /// effect, so capture loops can call it unconditionally on every write.
    ///
    /// For a track whose unit is an instant: a video frame, a cursor sample. A
    /// track delivered in buffers must use [`Self::mark_at`] instead.
    pub fn mark(&self) {
        self.mark_at(Instant::now());
    }

    /// Record `at` as this track's first sample.
    ///
    /// Audio arrives in buffers that cover time *before* they are handed over,
    /// so the arrival instant is not the first sample's instant. Marking the
    /// arrival puts the whole track late by one buffer, which on an idle
    /// loopback is capturekit's 100 ms silence chunk: audible lip-sync error.
    pub fn mark_at(&self, at: Instant) {
        if self.first_us.load(Ordering::Relaxed) != UNSET {
            return;
        }
        let elapsed = at.saturating_duration_since(self.origin).as_micros() as u64;
        let _ = self.first_us.compare_exchange(
            UNSET,
            elapsed.min(UNSET - 1),
            Ordering::AcqRel,
            Ordering::Relaxed,
        );
    }

    /// Microseconds from the session origin to this track's first sample, or
    /// `None` when the track never delivered one.
    pub fn elapsed_us(&self) -> Option<u64> {
        match self.first_us.load(Ordering::Acquire) {
            UNSET => None,
            us => Some(us),
        }
    }
}

/// Signed milliseconds `track` starts after `video`. Positive means the track
/// began late and needs padding at its head; negative means it began early and
/// its head must be trimmed. `None` when either track never produced a sample,
/// which downstream reads as "assume aligned".
pub fn offset_ms_from_video(video_first_us: u64, track: &TrackStart) -> Option<i64> {
    let track_us = track.elapsed_us()?;
    Some((track_us as i64 - video_first_us as i64) / 1000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn track_start_is_unset_until_marked() {
        let track = TrackStart::new(Instant::now());
        assert_eq!(track.elapsed_us(), None);
    }

    #[test]
    fn mark_records_a_value_and_later_marks_are_ignored() {
        let track = TrackStart::new(Instant::now());
        track.mark();
        let first = track.elapsed_us().expect("marked");
        std::thread::sleep(Duration::from_millis(5));
        track.mark();
        assert_eq!(track.elapsed_us(), Some(first));
    }

    #[test]
    fn offset_is_positive_when_the_track_starts_after_the_video() {
        let track = TrackStart::new(Instant::now() - Duration::from_millis(900));
        track.mark();
        let ms = offset_ms_from_video(400_000, &track).expect("marked");
        assert!((ms - 500).abs() <= 20, "expected ~500ms, got {ms}");
    }

    #[test]
    fn offset_is_negative_when_the_track_starts_before_the_video() {
        let track = TrackStart::new(Instant::now() - Duration::from_millis(150));
        track.mark();
        let ms = offset_ms_from_video(650_000, &track).expect("marked");
        assert!((ms + 500).abs() <= 20, "expected ~-500ms, got {ms}");
    }

    #[test]
    fn offset_is_none_for_a_track_that_never_produced_a_sample() {
        assert_eq!(
            offset_ms_from_video(0, &TrackStart::new(Instant::now())),
            None
        );
    }

    #[test]
    fn effective_elapsed_excludes_paused_spans() {
        // Measured as "how far did the clock advance across the pause", not as
        // an absolute reading: a loaded CI runner can stretch either sleep.
        let clock = RecordingClock::new(Instant::now());
        std::thread::sleep(Duration::from_millis(20));
        clock.pause();
        let at_pause = clock.effective_elapsed();

        std::thread::sleep(Duration::from_millis(40));
        let while_paused = clock.effective_elapsed();
        assert!(
            while_paused.saturating_sub(at_pause) < SLACK,
            "clock ran while paused: {at_pause:?} -> {while_paused:?}"
        );

        clock.resume();
        let after_resume = clock.effective_elapsed();
        assert!(
            after_resume.saturating_sub(at_pause) < SLACK,
            "paused span leaked into elapsed: {at_pause:?} -> {after_resume:?}"
        );
    }

    /// Room for the scheduler between a `pause`/`resume` call and the reading
    /// beside it. Well under the 40 ms a leaked pause span would show up as.
    const SLACK: Duration = Duration::from_millis(10);

    #[test]
    fn pause_and_resume_are_idempotent() {
        let clock = RecordingClock::new(Instant::now());
        clock.pause();
        clock.pause();
        assert!(clock.is_paused());
        clock.resume();
        clock.resume();
        assert!(!clock.is_paused());
    }
}

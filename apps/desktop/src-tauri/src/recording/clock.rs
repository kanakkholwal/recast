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
    pub fn mark(&self) {
        if self.first_us.load(Ordering::Relaxed) != UNSET {
            return;
        }
        let now = self.origin.elapsed().as_micros() as u64;
        let _ = self.first_us.compare_exchange(
            UNSET,
            now.min(UNSET - 1),
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

/// A camera stamp more than this far ahead of the session start is not from
/// this recording. The preview is told to roll just before `start_recording`,
/// so a genuine lead is well under a second.
const MAX_CAMERA_LEAD_MS: i64 = 10_000;

/// Signed milliseconds the camera track begins after video frame 0, from the
/// preview WebView's own Unix-ms report.
///
/// The camera records in a separate webview that is told to roll BEFORE the
/// capture threads spin up, so its offset is normally negative (it has a head
/// start, which the export trims). `None` when the stamp cannot belong to this
/// session, which downstream reads as "assume aligned" rather than shifting the
/// track by a wrong amount.
pub fn camera_offset_ms(
    reported_unix_ms: u64,
    session_start_unix_ms: u64,
    video_first_us: u64,
    session_duration_ms: u64,
) -> Option<i64> {
    let delta = reported_unix_ms as i64 - session_start_unix_ms as i64;
    let latest = session_duration_ms as i64 + MAX_CAMERA_LEAD_MS;
    if delta < -MAX_CAMERA_LEAD_MS || delta > latest {
        return None;
    }
    Some(delta - (video_first_us / 1000) as i64)
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
    fn a_camera_that_rolled_before_capture_reports_a_negative_offset() {
        // Preview started 300 ms before the session; video frame 0 landed
        // 750 ms in. The camera therefore leads the picture by 1050 ms.
        assert_eq!(
            camera_offset_ms(9_700, 10_000, 750_000, 60_000),
            Some(-1_050)
        );
    }

    #[test]
    fn a_camera_that_rolled_after_capture_reports_a_positive_offset() {
        assert_eq!(camera_offset_ms(11_500, 10_000, 750_000, 60_000), Some(750));
    }

    #[test]
    fn a_stamp_from_an_earlier_session_is_refused() {
        // A leftover report from a previous recording must not shift this one.
        assert_eq!(camera_offset_ms(1_000, 500_000, 0, 60_000), None);
    }

    #[test]
    fn a_stamp_past_the_end_of_the_session_is_refused() {
        assert_eq!(camera_offset_ms(200_000, 10_000, 0, 60_000), None);
    }

    #[test]
    fn a_camera_starting_exactly_with_the_video_is_aligned() {
        assert_eq!(camera_offset_ms(10_750, 10_000, 750_000, 60_000), Some(0));
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

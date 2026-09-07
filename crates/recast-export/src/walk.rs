use recast_time::quantize_secs;

/// Frame times within a tolerance of an exact boundary snap to it. A duration
/// that is 2.0 plus one ulp must not buy a whole extra frame.
const FRAME_EPSILON: f64 = 1e-6;

/// The output frames an export writes. Count-based, never accumulated: adding
/// `1.0 / fps` per frame drifts, which shipped as a frozen tail and as 25 fps.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameWalk {
    /// Numerator and denominator, so 30000/1001 stays exact.
    fps: (u32, u32),
    count: u64,
}

impl FrameWalk {
    /// Frames covering `[0, duration)` at `fps`, so no frame lands on or past
    /// the end. A frame at exactly `duration` is the frozen tail.
    #[must_use]
    pub fn new(duration_secs: f64, fps: (u32, u32)) -> Self {
        let fps = (fps.0.max(1), fps.1.max(1));
        let duration = quantize_secs(duration_secs);
        if !duration.is_finite() || duration <= 0.0 {
            return Self { fps, count: 0 };
        }
        let exact = duration * f64::from(fps.0) / f64::from(fps.1);
        // Snap before rounding up: 2.0s at 30fps is 60 frames, not 61.
        let snapped = if (exact - exact.round()).abs() < FRAME_EPSILON {
            exact.round()
        } else {
            exact.ceil()
        };
        Self {
            fps,
            count: snapped.max(0.0) as u64,
        }
    }

    #[must_use]
    pub fn len(&self) -> u64 {
        self.count
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    #[must_use]
    pub fn fps(&self) -> (u32, u32) {
        self.fps
    }

    /// Output-axis seconds of frame `index`, computed from the index rather
    /// than stepped, so frame 100_000 is as exact as frame 1.
    #[must_use]
    pub fn time_of(&self, index: u64) -> f64 {
        index as f64 * f64::from(self.fps.1) / f64::from(self.fps.0)
    }

    /// 100 ns units, the clock Media Foundation and `recast-mux` both stamp.
    #[must_use]
    pub fn duration_100ns(&self) -> i64 {
        i64::from(self.fps.1) * 10_000_000 / i64::from(self.fps.0)
    }

    /// Presentation time of frame `index` in 100 ns units. Derived from the
    /// index for the same reason as `time_of`: a summed duration drifts.
    #[must_use]
    pub fn timestamp_100ns(&self, index: u64) -> i64 {
        let index = i64::try_from(index).unwrap_or(i64::MAX);
        index.saturating_mul(i64::from(self.fps.1)) * 10_000_000 / i64::from(self.fps.0)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u64, f64)> + '_ {
        (0..self.count).map(|index| (index, self.time_of(index)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_whole_number_of_seconds_yields_exactly_that_many_frames() {
        assert_eq!(FrameWalk::new(2.0, (30, 1)).len(), 60);
        assert_eq!(FrameWalk::new(1.0, (60, 1)).len(), 60);
    }

    /// The frozen tail: a frame at exactly `duration` repeats the last picture
    /// because there is no source left to read, and the file runs long.
    #[test]
    fn no_frame_lands_on_or_past_the_end() {
        let walk = FrameWalk::new(2.0, (30, 1));
        let last = walk.time_of(walk.len() - 1);
        assert!(last < 2.0, "last frame at {last} is at or past the end");
        assert!(walk.time_of(walk.len()) >= 2.0, "a frame was dropped");
    }

    /// 64.1641 * 30000 / 1001 is 1923.0000000000002, so a bare `ceil` writes
    /// 1924 frames and the last repeats past the end. The frozen tail.
    #[test]
    fn a_frame_count_a_float_lands_just_over_does_not_buy_an_extra_frame() {
        let walk = FrameWalk::new(64.1641, (30000, 1001));
        assert_eq!(walk.len(), 1923);
        assert!(walk.time_of(walk.len() - 1) < 64.1641);
    }

    #[test]
    fn quantising_the_duration_absorbs_an_ulp_either_side_of_a_boundary() {
        assert_eq!(FrameWalk::new(2.0 + 1e-12, (30, 1)).len(), 60);
        assert_eq!(FrameWalk::new(2.0 - 1e-12, (30, 1)).len(), 60);
    }

    #[test]
    fn a_partial_frame_at_the_end_is_still_written() {
        // 2.01s at 30fps covers 60 whole frames plus a sliver.
        assert_eq!(FrameWalk::new(2.01, (30, 1)).len(), 61);
    }

    #[test]
    fn a_duration_shorter_than_one_frame_still_writes_one() {
        assert_eq!(FrameWalk::new(0.001, (30, 1)).len(), 1);
    }

    #[test]
    fn nothing_to_render_is_no_frames_rather_than_one_black_one() {
        assert_eq!(FrameWalk::new(0.0, (30, 1)).len(), 0);
        assert_eq!(FrameWalk::new(-1.0, (30, 1)).len(), 0);
        assert!(FrameWalk::new(f64::NAN, (30, 1)).is_empty());
    }

    /// Stepping by `1.0 / fps` drifts. At an hour of 29.97 the accumulated
    /// error is a frame and a half, which is audible as lip sync.
    #[test]
    fn frame_times_do_not_drift_over_an_hour_of_drop_frame() {
        let walk = FrameWalk::new(3600.0, (30000, 1001));
        let last = walk.len() - 1;
        let want = last as f64 * 1001.0 / 30000.0;
        assert!((walk.time_of(last) - want).abs() < 1e-9);

        let mut stepped = 0.0;
        for _ in 0..last {
            stepped += 1001.0 / 30000.0;
        }
        assert!(
            (stepped - walk.time_of(last)).abs() > 1e-9,
            "the accumulating clock did not drift, so this test proves nothing"
        );
    }

    #[test]
    fn drop_frame_timestamps_stay_on_the_hundred_nanosecond_clock() {
        let walk = FrameWalk::new(1.0, (30000, 1001));
        assert_eq!(walk.timestamp_100ns(0), 0);
        assert_eq!(walk.duration_100ns(), 1001 * 10_000_000 / 30000);
        // Frame 30000 of 30000/1001 is exactly 1001 seconds in.
        assert_eq!(walk.timestamp_100ns(30_000), 1001 * 10_000_000);
    }

    #[test]
    fn the_walk_yields_its_own_count_and_times() {
        let walk = FrameWalk::new(0.1, (30, 1));
        let frames: Vec<_> = walk.iter().collect();
        assert_eq!(frames.len() as u64, walk.len());
        assert_eq!(frames[0], (0, 0.0));
        assert!((frames[1].1 - 1.0 / 30.0).abs() < 1e-12);
    }

    #[test]
    fn a_zero_frame_rate_is_treated_as_one_rather_than_dividing_by_zero() {
        let walk = FrameWalk::new(2.0, (0, 0));
        assert_eq!(walk.fps(), (1, 1));
        assert_eq!(walk.len(), 2);
    }
}

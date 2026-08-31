use core::time::Duration;

/// A capture instant in nanoseconds on the source's own monotonic clock.
/// Nanoseconds and `i64` because that is what every backend here reports: QPC ticks, `CMTime`, and PipeWire's `pw_time` all convert without loss.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Timestamp(i64);

impl Timestamp {
    /// The clock's origin.
    pub const ZERO: Self = Self(0);

    /// Build a timestamp from nanoseconds.
    #[must_use]
    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    /// Build a timestamp from microseconds, saturating rather than wrapping.
    #[must_use]
    pub const fn from_micros(micros: i64) -> Self {
        Self(micros.saturating_mul(1_000))
    }

    /// Build a timestamp from a tick count and its frequency, as QPC reports.
    /// Multiplies before dividing, so a 10 MHz clock does not lose the sub-tick remainder the way `ticks / freq * 1e9` would.
    #[must_use]
    pub fn from_ticks(ticks: i64, ticks_per_second: i64) -> Self {
        if ticks_per_second <= 0 {
            return Self::ZERO;
        }
        let seconds = ticks / ticks_per_second;
        let remainder = ticks % ticks_per_second;
        Self(
            seconds.saturating_mul(1_000_000_000)
                + (i128::from(remainder) * 1_000_000_000 / i128::from(ticks_per_second)) as i64,
        )
    }

    /// Nanoseconds since the clock's origin.
    #[must_use]
    pub const fn as_nanos(self) -> i64 {
        self.0
    }

    /// Seconds since the clock's origin, for display and for humans only.
    #[must_use]
    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1e9
    }

    /// Time from `earlier` to `self`, or zero if `self` is not later.
    #[must_use]
    pub const fn saturating_since(self, earlier: Self) -> Duration {
        let delta = self.0.saturating_sub(earlier.0);
        if delta <= 0 {
            Duration::ZERO
        } else {
            Duration::from_nanos(delta as u64)
        }
    }

    /// This instant moved forward by `delta`.
    #[must_use]
    pub const fn saturating_add(self, delta: Duration) -> Self {
        Self(self.0.saturating_add(delta.as_nanos() as i64))
    }
}

/// Forces a source's timestamps to advance.
///
/// Every backend here can repeat or reverse a timestamp: DXGI reports the
/// accumulated frame's time, WGC can deliver two frames in one presentation tick,
/// and PipeWire renegotiates its clock on a format change. A consumer that
/// divides by the delta or sorts on it breaks on all three, so the correction
/// belongs here once rather than in each backend.
#[derive(Debug, Clone)]
pub struct MonotonicClock {
    last: Option<Timestamp>,
    min_step: Duration,
    corrections: u64,
}

impl MonotonicClock {
    /// A clock that nudges a stalled timestamp forward by `min_step`.
    #[must_use]
    pub const fn new(min_step: Duration) -> Self {
        Self {
            last: None,
            min_step,
            corrections: 0,
        }
    }

    /// A clock whose minimum step is one frame at `fps`.
    #[must_use]
    pub fn for_frame_rate(fps: u32) -> Self {
        let nanos = if fps == 0 {
            1_000_000
        } else {
            1_000_000_000 / u64::from(fps)
        };
        Self::new(Duration::from_nanos(nanos))
    }

    /// Admit a raw source timestamp, returning one guaranteed to be later than
    /// every timestamp this clock has returned before.
    pub fn admit(&mut self, raw: Timestamp) -> Timestamp {
        let Some(last) = self.last else {
            self.last = Some(raw);
            return raw;
        };
        let corrected = if raw > last {
            raw
        } else {
            self.corrections += 1;
            last.saturating_add(self.min_step)
        };
        self.last = Some(corrected);
        corrected
    }

    /// How many timestamps had to be corrected, for a backend to log or report.
    #[must_use]
    pub const fn corrections(&self) -> u64 {
        self.corrections
    }

    /// The most recent timestamp handed out.
    #[must_use]
    pub const fn last(&self) -> Option<Timestamp> {
        self.last
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qpc_ticks_convert_without_losing_the_remainder() {
        // 10 MHz, the usual QPC frequency: half a tick must not round to zero.
        let t = Timestamp::from_ticks(3, 10_000_000);
        assert_eq!(t.as_nanos(), 300);
    }

    #[test]
    fn a_zero_frequency_clock_yields_the_origin_rather_than_dividing_by_zero() {
        assert_eq!(Timestamp::from_ticks(1_000, 0), Timestamp::ZERO);
    }

    #[test]
    fn ticks_convert_at_a_frequency_that_does_not_divide_evenly() {
        // An awkward frequency chosen so both the seconds and the remainder contribute.
        let t = Timestamp::from_ticks(3_579_545 * 2 + 1, 3_579_545);
        assert_eq!(t.as_nanos(), 2_000_000_279);
    }

    #[test]
    fn a_repeated_timestamp_is_pushed_forward_by_one_step() {
        let mut clock = MonotonicClock::new(Duration::from_millis(1));
        let t = Timestamp::from_nanos(1_000_000_000);
        assert_eq!(clock.admit(t), t);
        assert_eq!(clock.admit(t), Timestamp::from_nanos(1_001_000_000));
        assert_eq!(clock.corrections(), 1);
    }

    #[test]
    fn a_stalled_source_keeps_advancing_across_repeated_corrections() {
        let mut clock = MonotonicClock::new(Duration::from_millis(1));
        let stuck = Timestamp::from_nanos(1_000_000_000);
        let admitted: Vec<i64> = (0..4).map(|_| clock.admit(stuck).as_nanos()).collect();
        assert_eq!(
            admitted,
            vec![1_000_000_000, 1_001_000_000, 1_002_000_000, 1_003_000_000]
        );
    }

    #[test]
    fn a_backwards_timestamp_never_moves_the_clock_back() {
        let mut clock = MonotonicClock::new(Duration::from_millis(1));
        clock.admit(Timestamp::from_nanos(5_000_000_000));
        let corrected = clock.admit(Timestamp::from_nanos(1_000_000_000));
        assert_eq!(corrected, Timestamp::from_nanos(5_001_000_000));
    }

    #[test]
    fn an_advancing_source_is_passed_through_untouched() {
        let mut clock = MonotonicClock::new(Duration::from_millis(1));
        for nanos in [10, 20, 30, 40] {
            let t = Timestamp::from_nanos(nanos * 1_000_000);
            assert_eq!(clock.admit(t), t);
        }
        assert_eq!(clock.corrections(), 0);
    }

    #[test]
    fn the_first_timestamp_is_never_treated_as_a_correction() {
        let mut clock = MonotonicClock::new(Duration::from_millis(1));
        let t = Timestamp::from_nanos(-5_000);
        assert_eq!(clock.admit(t), t);
        assert_eq!(clock.corrections(), 0);
    }

    #[test]
    fn a_frame_rate_clock_steps_by_one_frame() {
        let clock = MonotonicClock::for_frame_rate(60);
        assert_eq!(clock.min_step, Duration::from_nanos(16_666_666));
    }

    #[test]
    fn a_zero_frame_rate_still_produces_a_usable_step() {
        let clock = MonotonicClock::for_frame_rate(0);
        assert!(clock.min_step > Duration::ZERO);
    }

    #[test]
    fn a_delta_backwards_saturates_to_zero_rather_than_underflowing() {
        let early = Timestamp::from_nanos(10);
        let late = Timestamp::from_nanos(20);
        assert_eq!(late.saturating_since(early), Duration::from_nanos(10));
        assert_eq!(early.saturating_since(late), Duration::ZERO);
    }
}

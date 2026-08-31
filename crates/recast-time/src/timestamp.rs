#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize), serde(transparent))]
pub struct Timestamp(i64);

impl Timestamp {
    pub const ZERO: Self = Self(0);

    pub const fn from_nanos(nanos: i64) -> Self {
        Self(nanos)
    }

    pub const fn as_nanos(self) -> i64 {
        self.0
    }

    pub fn from_secs_f64(secs: f64) -> Self {
        Self((secs * 1e9).round() as i64)
    }

    pub fn as_secs_f64(self) -> f64 {
        self.0 as f64 / 1e9
    }

    pub fn from_micros(micros: i64) -> Self {
        Self(micros.saturating_mul(1_000))
    }

    pub fn as_micros(self) -> i64 {
        self.0 / 1_000
    }

    pub fn from_millis(millis: i64) -> Self {
        Self(millis.saturating_mul(1_000_000))
    }

    pub fn as_millis(self) -> i64 {
        self.0 / 1_000_000
    }

    pub fn saturating_sub(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0))
    }

    pub fn saturating_add(self, other: Self) -> Self {
        Self(self.0.saturating_add(other.0))
    }

    pub fn abs_diff(self, other: Self) -> Self {
        Self(self.0.saturating_sub(other.0).saturating_abs())
    }

    pub fn is_negative(self) -> bool {
        self.0 < 0
    }
}

impl core::ops::Add for Timestamp {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for Timestamp {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

/// Seconds per grid step for authored times. One microsecond: finer than any
/// frame rate resolves, coarse enough that `f64` holds it exactly for the
/// length of a recording, and `format!("{:.6}")` is then a lossless encoding.
pub const TIME_QUANTUM: f64 = 1e-6;

/// Snaps an authored time onto [`TIME_QUANTUM`], stopping arithmetic dust accumulating and making the six-decimal text projection LOSSLESS so an agent's edit round-trips.
/// Non-finite input is returned untouched: clamping here would hide a bug the caller's own validation should reject.
#[must_use]
pub fn quantize_secs(secs: f64) -> f64 {
    if !secs.is_finite() {
        return secs;
    }
    // Divide by 1e6, never multiply by 1e-6: only the division is correctly rounded to the printed decimal.
    (secs * 1e6).round() / 1e6
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantizing_clears_arithmetic_dust() {
        assert_eq!(quantize_secs(0.1 + 0.2), 0.3);
        assert_eq!(quantize_secs(1.9999999999999998), 2.0);
    }

    #[test]
    fn a_quantized_time_survives_six_decimals_exactly() {
        for raw in [0.1 + 0.2, 1.0 / 3.0, 59.94, 12.3456789, -0.7] {
            let q = quantize_secs(raw);
            let text = format!("{q:.6}");
            // NaN on a parse failure fails the assert below; the crate denies `expect`.
            let parsed = text.parse::<f64>().unwrap_or(f64::NAN);
            assert_eq!(parsed, q, "{raw} -> {text} did not round-trip");
        }
    }

    #[test]
    fn quantizing_is_idempotent() {
        let once = quantize_secs(1.0 / 3.0);
        assert_eq!(quantize_secs(once), once);
    }

    #[test]
    fn a_non_finite_time_is_left_for_the_caller_to_reject() {
        assert!(quantize_secs(f64::NAN).is_nan());
        assert_eq!(quantize_secs(f64::INFINITY), f64::INFINITY);
    }

    #[test]
    fn round_trips_seconds() {
        let t = Timestamp::from_secs_f64(1.5);
        assert_eq!(t.as_nanos(), 1_500_000_000);
        assert!((t.as_secs_f64() - 1.5).abs() < 1e-12);
    }

    #[test]
    fn is_signed_so_a_pre_origin_sample_is_representable() {
        let t = Timestamp::from_millis(-40);
        assert!(t.is_negative());
        assert_eq!(t.as_millis(), -40);
    }

    #[test]
    fn ordering_is_by_instant() {
        let mut stamps = [
            Timestamp::from_millis(30),
            Timestamp::from_millis(-10),
            Timestamp::ZERO,
        ];
        stamps.sort();
        assert_eq!(stamps[0].as_millis(), -10);
        assert_eq!(stamps[2].as_millis(), 30);
    }

    #[test]
    fn abs_diff_is_symmetric() {
        let a = Timestamp::from_millis(10);
        let b = Timestamp::from_millis(90);
        assert_eq!(a.abs_diff(b), b.abs_diff(a));
        assert_eq!(a.abs_diff(b).as_millis(), 80);
    }
}

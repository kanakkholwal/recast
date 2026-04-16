use serde::{Deserialize, Serialize};

/// Unit-interval cubic-bezier control points. P0 and P3 are implicit at
/// (0, 0) and (1, 1). x1 / x2 should stay in `[0, 1]`; y1 / y2 can go
/// outside for overshoot curves.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Easing {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
}

impl Default for Easing {
    /// CSS `ease` curve — reasonable default for both ramp-in and ramp-out.
    fn default() -> Self {
        Self {
            x1: 0.25,
            y1: 0.10,
            x2: 0.25,
            y2: 1.00,
        }
    }
}

impl Easing {
    #[allow(dead_code)] // public constant; tests consume it, export may in the future
    pub const LINEAR: Easing = Easing {
        x1: 0.0,
        y1: 0.0,
        x2: 1.0,
        y2: 1.0,
    };

    fn coeffs(c1: f32, c2: f32) -> (f32, f32, f32) {
        let a = 1.0 - 3.0 * c2 + 3.0 * c1;
        let b = 3.0 * c2 - 6.0 * c1;
        let c = 3.0 * c1;
        (a, b, c)
    }

    fn sample_poly(t: f32, a: f32, b: f32, c: f32) -> f32 {
        ((a * t + b) * t + c) * t
    }

    fn sample_deriv(t: f32, a: f32, b: f32, c: f32) -> f32 {
        (3.0 * a * t + 2.0 * b) * t + c
    }

    /// Evaluate `y` for a given `x` in `[0, 1]`. Returns `y(t)` where `t`
    /// is the bezier parameter that produces `x` (solved via Newton-Raphson
    /// with a bisection fallback, same technique as Blink/WebKit).
    pub fn y(&self, x: f32) -> f32 {
        if (self.x1 - self.y1).abs() < f32::EPSILON && (self.x2 - self.y2).abs() < f32::EPSILON {
            return x.clamp(0.0, 1.0);
        }
        if x <= 0.0 {
            return 0.0;
        }
        if x >= 1.0 {
            return 1.0;
        }

        let (ax, bx, cx) = Self::coeffs(self.x1, self.x2);
        let (ay, by, cy) = Self::coeffs(self.y1, self.y2);

        let mut t = x;
        for _ in 0..8 {
            let xt = Self::sample_poly(t, ax, bx, cx) - x;
            if xt.abs() < 1e-6 {
                return Self::sample_poly(t, ay, by, cy);
            }
            let dxt = Self::sample_deriv(t, ax, bx, cx);
            if dxt.abs() < 1e-6 {
                break;
            }
            t -= xt / dxt;
        }

        // Bisection fallback.
        let mut lo = 0.0f32;
        let mut hi = 1.0f32;
        t = x;
        loop {
            let xt = Self::sample_poly(t, ax, bx, cx);
            if (xt - x).abs() < 1e-6 {
                return Self::sample_poly(t, ay, by, cy);
            }
            if x > xt {
                lo = t;
            } else {
                hi = t;
            }
            t = (lo + hi) * 0.5;
            if hi - lo < 1e-7 {
                break;
            }
        }
        Self::sample_poly(t, ay, by, cy)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_is_identity() {
        let e = Easing::LINEAR;
        for i in 0..=10 {
            let x = i as f32 / 10.0;
            assert!((e.y(x) - x).abs() < 1e-4, "linear at {x}");
        }
    }

    #[test]
    fn ease_matches_reference_midpoint() {
        // CSS `ease` at x=0.5 is ≈0.8024 (reference from Chromium test data).
        let y = Easing::default().y(0.5);
        assert!((y - 0.8024).abs() < 5e-3, "ease(0.5)={y} expected ~0.8024");
    }

    #[test]
    fn monotonic_on_standard_curves() {
        let curves = [
            Easing::LINEAR,
            Easing::default(),
            Easing {
                x1: 0.42,
                y1: 0.0,
                x2: 1.0,
                y2: 1.0,
            },
            Easing {
                x1: 0.0,
                y1: 0.0,
                x2: 0.58,
                y2: 1.0,
            },
        ];
        for e in curves {
            let mut prev = e.y(0.0);
            for i in 1..=100 {
                let x = i as f32 / 100.0;
                let y = e.y(x);
                assert!(y + 1e-4 >= prev, "non-monotonic on {e:?} at x={x}");
                prev = y;
            }
        }
    }

    #[test]
    fn bounce_can_overshoot() {
        let bounce = Easing {
            x1: 0.68,
            y1: -0.55,
            x2: 0.27,
            y2: 1.55,
        };
        let mut max_y = f32::NEG_INFINITY;
        let mut min_y = f32::INFINITY;
        for i in 0..=100 {
            let x = i as f32 / 100.0;
            let y = bounce.y(x);
            max_y = max_y.max(y);
            min_y = min_y.min(y);
        }
        assert!(max_y > 1.05, "bounce should overshoot top, got {max_y}");
        assert!(min_y < -0.05, "bounce should overshoot bottom, got {min_y}");
    }
}

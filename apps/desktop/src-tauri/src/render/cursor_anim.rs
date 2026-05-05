//! Pure helpers for cursor animation effects (click bounce, idle sway,
//! motion-blur trail alpha). Kept free of FFmpeg/render-state types so the
//! curves can be unit-tested in isolation.

/// Map a click-bounce sample to a sprite scale multiplier.
///
/// `t_ms` is the signed offset (in ms) from the *nearest* click event:
/// negative means the click hasn't happened yet, positive means it just
/// fired. `duration_ms` is the full bounce window (the user-tunable
/// "Bounce speed" knob — typically 120..400 ms).
///
/// `amplitude` is the raw 0..5 slider value; we treat 1.0 as "Apple-style
/// subtle squash" (~12% size delta) and let larger values exaggerate.
///
/// The curve:
/// - Pre-anticipation: a tiny inward dip (~3% of amplitude) just before the
///   click, so the bounce doesn't feel like it appears from nowhere.
/// - Impact: a hard outward pop at t=0.
/// - Settle: damped sinusoidal decay for the rest of the window.
pub fn click_bounce_scale(t_ms: f64, duration_ms: f64, amplitude: f64) -> f64 {
    if amplitude.abs() < 1e-6 || duration_ms <= 0.0 {
        return 1.0;
    }
    if t_ms.abs() > duration_ms {
        return 1.0;
    }
    // Normalised time in [-1, 1] across the window.
    let n = (t_ms / duration_ms).clamp(-1.0, 1.0);
    // Apple's Materials team uses ~0.12 of the parameter as the visible
    // amplitude; multiplying by amplitude_factor lets the slider's "1×" look
    // like a real macOS bounce while "5×" still has headroom for cinematic
    // squash demos without going non-physical.
    const PER_UNIT_DELTA: f64 = 0.12;
    let amp = amplitude * PER_UNIT_DELTA;

    if n < 0.0 {
        // Anticipation lobe — small inward dip easing toward 0.
        let p = 1.0 + n; // 0 → 1 as we approach the click
        // Smooth ease-in (cubic) keeps the dip subtle.
        let dip = 0.25 * amp * (1.0 - (1.0 - p).powi(3));
        return 1.0 - dip;
    }

    // Post-impact damped oscillation.
    // exp(-4n) decays to ~1.8% of starting amplitude by n=1; cos(2πn·1.5)
    // gives a single overshoot that lands just below 1.0 then settles back.
    let damp = (-4.0 * n).exp();
    let osc = (std::f64::consts::TAU * n * 1.5).cos();
    1.0 + amp * damp * osc
}

/// Add a small sinusoidal wobble (in source pixels) to an idle/slow cursor.
///
/// `amplitude` is the 0..1 slider; we map 1.0 to ±2 source pixels of sway,
/// which reads as "alive" without ever drifting visibly off the click target.
/// `velocity` is current cursor speed in source-px/sec — sway tapers to 0
/// once the cursor is moving fast enough that the wobble would just smear.
pub fn idle_sway_offset(t_ms: f64, amplitude: f64, velocity_px_per_s: f64) -> (f64, f64) {
    if amplitude.abs() < 1e-6 {
        return (0.0, 0.0);
    }
    // Tapered influence: full strength at rest, zero by 600 px/s.
    let velocity_factor = (1.0 - (velocity_px_per_s / 600.0)).clamp(0.0, 1.0);
    let amp_px = amplitude.clamp(0.0, 1.0) * 2.0 * velocity_factor;
    if amp_px < 1e-3 {
        return (0.0, 0.0);
    }
    // Two slightly out-of-phase axes so the path traces a Lissajous-like
    // figure rather than a straight line. Periods are coprime to avoid a
    // visible "loop" point.
    let t_s = t_ms / 1000.0;
    let dx = amp_px * (std::f64::consts::TAU * t_s * 0.7).sin();
    let dy = amp_px * (std::f64::consts::TAU * t_s * 0.9 + 1.2).sin();
    (dx, dy)
}

/// Per-step trail alpha for the motion-blur effect.
///
/// Returns the alpha for the i-th historical position (0 = current frame,
/// `steps - 1` = oldest). Alpha falls off linearly and is scaled by the
/// 0..1 strength slider so MB=0 contributes no visible trail.
pub fn motion_blur_step_alpha(i: usize, steps: usize, strength: f64) -> f64 {
    if strength <= 0.0 || steps == 0 {
        return 0.0;
    }
    let t = (i as f64) / (steps as f64);
    let s = strength.clamp(0.0, 1.0);
    // Quadratic falloff reads more like real motion blur than linear —
    // most of the brightness sits near the current position, the tail
    // dims fast.
    s * (1.0 - t).powi(2) * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounce_zero_amplitude_is_identity() {
        for t in [-200.0, -50.0, 0.0, 50.0, 200.0] {
            assert!((click_bounce_scale(t, 200.0, 0.0) - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn bounce_outside_window_is_identity() {
        assert!((click_bounce_scale(500.0, 200.0, 2.0) - 1.0).abs() < 1e-9);
        assert!((click_bounce_scale(-500.0, 200.0, 2.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn bounce_dip_before_click() {
        let s = click_bounce_scale(-20.0, 200.0, 1.0);
        assert!(s < 1.0, "expected anticipation dip, got {s}");
        // Dip stays small — never more than 5% of the unit amplitude.
        assert!(s > 0.95);
    }

    #[test]
    fn bounce_overshoot_at_impact() {
        let s = click_bounce_scale(0.0, 200.0, 1.0);
        assert!(s > 1.0, "expected outward pop at t=0, got {s}");
        // 1× amplitude maps to ~12% overshoot.
        assert!(s < 1.15);
    }

    #[test]
    fn bounce_settles_back_toward_one() {
        let near_end = click_bounce_scale(190.0, 200.0, 1.0);
        assert!((near_end - 1.0).abs() < 0.02, "expected near-1 settle, got {near_end}");
    }

    #[test]
    fn bounce_amplitude_scales_overshoot() {
        let s1 = click_bounce_scale(0.0, 200.0, 1.0);
        let s5 = click_bounce_scale(0.0, 200.0, 5.0);
        assert!(s5 > s1, "5× should overshoot more than 1× ({s5} vs {s1})");
    }

    #[test]
    fn sway_zero_amplitude_returns_origin() {
        let (dx, dy) = idle_sway_offset(123.0, 0.0, 0.0);
        assert_eq!((dx, dy), (0.0, 0.0));
    }

    #[test]
    fn sway_tapers_with_velocity() {
        let slow = idle_sway_offset(250.0, 1.0, 0.0);
        let fast = idle_sway_offset(250.0, 1.0, 800.0);
        assert!(slow.0.hypot(slow.1) > fast.0.hypot(fast.1));
        // Fully past the velocity cutoff → no sway.
        assert_eq!(fast, (0.0, 0.0));
    }

    #[test]
    fn sway_amplitude_capped_at_two_px() {
        // Worst case: max amplitude, dead stop, peak of both sinusoids.
        let mut peak = 0.0_f64;
        for t in 0..2000 {
            let (dx, dy) = idle_sway_offset(t as f64, 1.0, 0.0);
            peak = peak.max(dx.hypot(dy));
        }
        assert!(peak <= 2.0 * std::f64::consts::SQRT_2 + 1e-6, "peak={peak}");
    }

    #[test]
    fn motion_blur_alpha_falls_off() {
        let a0 = motion_blur_step_alpha(0, 8, 1.0);
        let a4 = motion_blur_step_alpha(4, 8, 1.0);
        let a7 = motion_blur_step_alpha(7, 8, 1.0);
        assert!(a0 > a4 && a4 > a7);
        assert!(a0 <= 0.5 + 1e-9, "head alpha never exceeds 0.5: {a0}");
    }

    #[test]
    fn motion_blur_zero_strength_is_silent() {
        for i in 0..8 {
            assert_eq!(motion_blur_step_alpha(i, 8, 0.0), 0.0);
        }
    }
}

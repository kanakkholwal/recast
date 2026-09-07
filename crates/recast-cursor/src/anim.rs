/// One captured click: rising and falling edge in wall-clock μs, plus the cursor position at the rising edge in SOURCE pixels.
/// Smoothing must NEVER reshape these, or the rendered impact drifts off the click sound and the captured target.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressEvent {
    pub down_us: u64,
    pub up_us: u64,
    pub down_x: f64,
    pub down_y: f64,
    /// Right mouse button initiated this press (vs left). Selects the sprite
    /// slot; does NOT affect the click animation/anchor curves.
    pub right: bool,
    /// Cursor moved past `DRAG_THRESHOLD_PX` (source px) while held — a drag.
    pub dragged: bool,
}

/// Per-frame press state. `visible_alpha` is additive on top of `idle_alpha`, so an upcoming click pulls a hidden cursor back rather than teleporting it in on impact.
/// `pressed_sprite` leads by `PRESS_PREROLL_US`, and `scale` runs anticipation lift, a snap to `1-PUNCH` on the click frame, then bounce and settle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PressFrameState {
    pub pressed_sprite: bool,
    pub visible_alpha: f64,
    pub scale: f64,
    /// Active press was a right-click (mirrors the selected event's `right`).
    pub right: bool,
    /// Active press is a drag (mirrors the selected event's `dragged`).
    pub dragged: bool,
}

impl PressFrameState {
    pub const NONE: Self = Self {
        pressed_sprite: false,
        visible_alpha: 0.0,
        scale: 1.0,
        right: false,
        dragged: false,
    };
}

/// Cursor displacement (source px) during a hold beyond which it's a drag.
const DRAG_THRESHOLD_PX: f64 = 8.0;

const PRESS_MIN_HOLD_US: i64 = 320_000;
const PRESS_LINGER_US: i64 = 320_000;
const PRESS_PREROLL_US: i64 = 320_000;
const PRESS_VIS_RAMP_US: i64 = 180_000;
const PRESS_POSTROLL_US: i64 = 320_000;
const PRESS_ANTICIP_US: i64 = 140_000;
const PRESS_RECOVERY_US: i64 = 380_000;
const PRESS_LIFT: f64 = 0.04;
const PRESS_PUNCH: f64 = 0.16;
const PRESS_BOUNCE: f64 = 0.03;
/// Always-on click-snap half-window in μs — see `click_anchor_at`.
pub const CLICK_SNAP_HALF_US: i64 = 200_000;

#[inline]
fn smooth_step_01(t: f64) -> f64 {
    if t <= 0.0 {
        return 0.0;
    }
    if t >= 1.0 {
        return 1.0;
    }
    t * t * (3.0 - 2.0 * t)
}

/// All click-relative state at a moment, picking the press whose `down_us` is nearest among those whose influence window contains it. `events` MUST be sorted by `down_us`.
/// For sub-300 ms double-clicks that hands the curve to the upcoming click as soon as it is the closer one.
pub fn press_state_at(ts_us: i64, events: &[PressEvent]) -> PressFrameState {
    let mut best: Option<(PressEvent, i64, i64, i64, i64)> = None; // (ev, hold_end, vis_start, vis_end, abs_dt)
    for &ev in events {
        let down = ev.down_us as i64;
        let up = ev.up_us as i64;
        // holdEnd is the later of release plus LINGER and down plus MIN_HOLD, so flash clicks and long holds both dwell visibly.
        let hold_end = (up + PRESS_LINGER_US).max(down + PRESS_MIN_HOLD_US);
        let vis_start = down - PRESS_PREROLL_US - PRESS_VIS_RAMP_US;
        let vis_end = hold_end + PRESS_POSTROLL_US + PRESS_VIS_RAMP_US;
        if ts_us < vis_start {
            break; // events sorted; later ones also fail
        }
        if ts_us > vis_end {
            continue;
        }
        let abs_dt = (ts_us - down).abs();
        match best {
            Some((_, _, _, _, cur_abs_dt)) if abs_dt >= cur_abs_dt => {}
            _ => best = Some((ev, hold_end, vis_start, vis_end, abs_dt)),
        }
    }
    let Some((ev, hold_end, vis_start, vis_end, _)) = best else {
        return PressFrameState::NONE;
    };
    let down = ev.down_us as i64;

    let mut visible_alpha = 1.0;
    if ts_us < down - PRESS_PREROLL_US {
        visible_alpha = smooth_step_01((ts_us - vis_start) as f64 / PRESS_VIS_RAMP_US as f64);
    } else if ts_us > hold_end + PRESS_POSTROLL_US {
        visible_alpha = smooth_step_01((vis_end - ts_us) as f64 / PRESS_VIS_RAMP_US as f64);
    }

    let pressed_sprite = ts_us >= down - PRESS_PREROLL_US && ts_us <= hold_end;

    let mut scale = 1.0;
    let dt = ts_us - down;
    if (-PRESS_ANTICIP_US..0).contains(&dt) {
        let u = (dt + PRESS_ANTICIP_US) as f64 / PRESS_ANTICIP_US as f64;
        scale = 1.0 + PRESS_LIFT * smooth_step_01(u);
    } else if (0..PRESS_RECOVERY_US).contains(&dt) {
        let u = dt as f64 / PRESS_RECOVERY_US as f64;
        if u < 0.6 {
            let v = u / 0.6;
            scale = 1.0 - PRESS_PUNCH + (PRESS_PUNCH + PRESS_BOUNCE) * smooth_step_01(v);
        } else {
            let v = (u - 0.6) / 0.4;
            scale = 1.0 + PRESS_BOUNCE - PRESS_BOUNCE * smooth_step_01(v);
        }
    }

    PressFrameState {
        pressed_sprite,
        visible_alpha,
        scale,
        right: ev.right,
        dragged: ev.dragged,
    }
}

/// Build the press-event list from the cursor track's raw samples.
/// Pairs each rising edge with the next falling edge; an unmatched final
/// rising edge is closed at the last sample's timestamp. Captures the
/// (x, y) at the rising edge for the always-on click-anchor snap.
pub fn build_press_events_from_iter<I>(samples: I) -> Vec<PressEvent>
where
    I: IntoIterator<Item = (u64, f64, f64, bool, bool)>,
{
    let mut events = Vec::new();
    let mut in_press = false;
    let mut down_us = 0_u64;
    let mut down_x = 0.0_f64;
    let mut down_y = 0.0_f64;
    let mut right = false;
    let mut max_disp = 0.0_f64;
    let mut last_ts = 0_u64;
    for (ts, x, y, left, r) in samples {
        last_ts = ts;
        let down = left || r;
        if down && !in_press {
            in_press = true;
            down_us = ts;
            down_x = x;
            down_y = y;
            // Right-click only when right is the sole button down at the edge.
            right = r && !left;
            max_disp = 0.0;
        } else if down && in_press {
            max_disp = max_disp.max((x - down_x).hypot(y - down_y));
        } else if !down && in_press {
            in_press = false;
            events.push(PressEvent {
                down_us,
                up_us: ts,
                down_x,
                down_y,
                right,
                dragged: max_disp > DRAG_THRESHOLD_PX,
            });
        }
    }
    if in_press {
        events.push(PressEvent {
            down_us,
            up_us: last_ts,
            down_x,
            down_y,
            right,
            dragged: max_disp > DRAG_THRESHOLD_PX,
        });
    }
    events
}

/// Active click anchor + cosine falloff weight at `ts_us`. Returns `None`
/// when outside any snap window. `events` MUST be sorted ascending by
/// `down_us`. Picks the closest event when two snap windows overlap so
/// double-clicks each pull the cursor to their respective targets.
pub fn click_anchor_at(ts_us: i64, events: &[PressEvent]) -> Option<(f64, f64, f64)> {
    let mut best: Option<(PressEvent, i64)> = None;
    for &ev in events {
        let down = ev.down_us as i64;
        if ts_us < down - CLICK_SNAP_HALF_US {
            break;
        }
        if ts_us > down + CLICK_SNAP_HALF_US {
            continue;
        }
        let abs_dt = (ts_us - down).abs();
        match best {
            Some((_, cur_abs_dt)) if abs_dt >= cur_abs_dt => {}
            _ => best = Some((ev, abs_dt)),
        }
    }
    let (ev, abs_dt) = best?;
    let weight =
        0.5 + 0.5 * (abs_dt as f64 / CLICK_SNAP_HALF_US as f64 * std::f64::consts::PI).cos();
    Some((ev.down_x, ev.down_y, weight))
}

/// Fade-in / fade-out of the pinned click highlight, in μs.
const HIGHLIGHT_FADE_IN_US: i64 = 40_000;
const HIGHLIGHT_FADE_OUT_US: i64 = 220_000;

/// Click-highlight envelope: the CAPTURED click position and an alpha that rises on impact, holds through the press, then fades. `events` MUST be sorted by `down_us`.
/// Keyed to the raw click, not the smoothed cursor, or the ring rides the lagging position and reads as delayed, off-target feedback.
pub fn click_highlight_at(ts_us: i64, events: &[PressEvent]) -> Option<(f64, f64, f64)> {
    let mut best: Option<PressEvent> = None;
    let mut best_dt = i64::MAX;
    for &ev in events {
        let down = ev.down_us as i64;
        let up = ev.up_us as i64;
        let hold_end = (up + PRESS_LINGER_US).max(down + PRESS_MIN_HOLD_US);
        if ts_us < down {
            continue;
        }
        if ts_us > hold_end + HIGHLIGHT_FADE_OUT_US {
            continue;
        }
        let dt = ts_us - down;
        if dt < best_dt {
            best_dt = dt;
            best = Some(ev);
        }
    }
    let ev = best?;
    let down = ev.down_us as i64;
    let up = ev.up_us as i64;
    let hold_end = (up + PRESS_LINGER_US).max(down + PRESS_MIN_HOLD_US);
    let alpha = if ts_us < down + HIGHLIGHT_FADE_IN_US {
        smooth_step_01((ts_us - down) as f64 / HIGHLIGHT_FADE_IN_US as f64)
    } else if ts_us <= hold_end {
        1.0
    } else {
        smooth_step_01(
            (hold_end + HIGHLIGHT_FADE_OUT_US - ts_us) as f64 / HIGHLIGHT_FADE_OUT_US as f64,
        )
    };
    Some((ev.down_x, ev.down_y, alpha))
}

/// Sprite scale for a click bounce; `t_ms` is signed from the nearest click and `amplitude` treats 1.0 as a subtle ~12% squash.
/// A small inward dip precedes the click so the pop does not appear from nowhere, then a damped sinusoid settles it.
pub fn click_bounce_scale(t_ms: f64, duration_ms: f64, amplitude: f64) -> f64 {
    if amplitude.abs() < 1e-6 || duration_ms <= 0.0 {
        return 1.0;
    }
    if t_ms.abs() > duration_ms {
        return 1.0;
    }
    // Normalised time in [-1, 1] across the window.
    let n = (t_ms / duration_ms).clamp(-1.0, 1.0);
    // About 0.12 of the parameter is the visible amplitude, so the slider's 1x looks like a real bounce and 5x still has headroom.
    const PER_UNIT_DELTA: f64 = 0.12;
    let amp = amplitude * PER_UNIT_DELTA;

    if n < 0.0 {
        // Anticipation lobe — small inward dip easing toward 0.
        let p = 1.0 + n; // 0 → 1 as we approach the click
                         // Smooth ease-in (cubic) keeps the dip subtle.
        let dip = 0.25 * amp * (1.0 - (1.0 - p).powi(3));
        return 1.0 - dip;
    }

    // exp(-4n) decays to ~1.8% by n=1, and the cosine gives one overshoot that settles just below 1.0.
    let damp = (-4.0 * n).exp();
    let osc = (std::f64::consts::TAU * n * 1.5).cos();
    1.0 + amp * damp * osc
}

/// Adds a small sinusoidal wobble in source pixels to an idle or slow cursor; `amplitude` maps 1.0 to about two pixels of sway.
/// It reads as alive without drifting off the click target, and tapers to zero once `velocity` is high enough that the wobble would just smear.
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
    // Two slightly out-of-phase axes trace a Lissajous figure; coprime periods avoid a visible loop point.
    let t_s = t_ms / 1000.0;
    let dx = amp_px * (std::f64::consts::TAU * t_s * 0.7).sin();
    let dy = amp_px * (std::f64::consts::TAU * t_s * 0.9 + 1.2).sin();
    (dx, dy)
}

/// Per-step trail alpha for the motion-blur effect, for the i-th historical position where 0 is the current frame.
/// Alpha falls off linearly and is scaled by the 0..1 strength slider, so a strength of 0 contributes no visible trail.
pub fn motion_blur_step_alpha(i: usize, steps: usize, strength: f64) -> f64 {
    if strength <= 0.0 || steps == 0 {
        return 0.0;
    }
    let t = (i as f64) / (steps as f64);
    let s = strength.clamp(0.0, 1.0);
    // Quadratic falloff reads more like real motion blur than linear: brightness sits near the current position.
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
        assert!(
            (near_end - 1.0).abs() < 0.02,
            "expected near-1 settle, got {near_end}"
        );
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

use serde::{Deserialize, Serialize};

use super::CursorSample;

/// A smoothed cursor position at a specific point in time.
#[derive(Debug, Clone, Copy)]
pub struct SmoothedPosition {
    pub x: f64,
    pub y: f64,
    pub visible: bool,
    pub left_down: bool,
    pub right_down: bool,
}

/// Smoothing configuration derived from the 0–100 user-facing slider.
#[derive(Debug, Clone, Copy)]
pub struct SmoothingConfig {
    /// Exponential smoothing factor (0.0 = no smoothing, 1.0 = maximum smoothing).
    /// Derived from the user slider: `alpha = 1.0 - (slider / 100.0) * 0.95`
    pub alpha: f64,
    /// Whether to use Catmull-Rom interpolation for sub-sample positions.
    pub interpolate: bool,
}

impl SmoothingConfig {
    /// Create a smoothing config from the user-facing slider value (0–100).
    /// 0 = raw positions (no smoothing), 100 = maximum smoothing.
    pub fn from_slider(value: f64) -> Self {
        let clamped = value.clamp(0.0, 100.0);
        // alpha=1.0 means "take all of new value" (no smoothing).
        // alpha=0.05 means "keep 95% of previous" (heavy smoothing).
        let alpha = 1.0 - (clamped / 100.0) * 0.95;
        Self {
            alpha,
            interpolate: clamped > 10.0,
        }
    }
}

/// Apply exponential smoothing to a list of raw cursor samples.
/// Returns a new list of the same length with smoothed x/y positions.
/// Velocity is recomputed from the smoothed positions.
pub fn smooth_samples(samples: &[CursorSample], config: SmoothingConfig) -> Vec<CursorSample> {
    if samples.is_empty() || config.alpha >= 1.0 {
        return samples.to_vec();
    }

    let mut result = Vec::with_capacity(samples.len());
    let mut smooth_x = samples[0].x as f64;
    let mut smooth_y = samples[0].y as f64;

    for (i, sample) in samples.iter().enumerate() {
        smooth_x = config.alpha * sample.x as f64 + (1.0 - config.alpha) * smooth_x;
        smooth_y = config.alpha * sample.y as f64 + (1.0 - config.alpha) * smooth_y;

        let (velocity_x, velocity_y) = if i > 0 {
            let prev: &CursorSample = &result[i - 1];
            let delta_t = ((sample.timestamp_us.saturating_sub(prev.timestamp_us)).max(1)) as f64
                / 1_000_000.0;
            (
                (smooth_x - prev.x as f64) as f32 / delta_t as f32,
                (smooth_y - prev.y as f64) as f32 / delta_t as f32,
            )
        } else {
            (0.0, 0.0)
        };

        result.push(CursorSample {
            timestamp_us: sample.timestamp_us,
            x: smooth_x.round() as i32,
            y: smooth_y.round() as i32,
            velocity_x,
            velocity_y,
            visible: sample.visible,
            left_down: sample.left_down,
            right_down: sample.right_down,
        });
    }

    result
}

/// Get the interpolated cursor position at a specific timestamp using
/// Catmull-Rom spline interpolation over the four nearest samples.
/// Falls back to linear interpolation if fewer than 4 samples surround the time.
pub fn interpolate_at(samples: &[CursorSample], timestamp_us: u64) -> Option<SmoothedPosition> {
    if samples.is_empty() {
        return None;
    }

    // Binary search for the insertion point.
    let idx = samples
        .binary_search_by_key(&timestamp_us, |s| s.timestamp_us)
        .unwrap_or_else(|i| i);

    // Exact match
    if idx < samples.len() && samples[idx].timestamp_us == timestamp_us {
        let s = &samples[idx];
        return Some(SmoothedPosition {
            x: s.x as f64,
            y: s.y as f64,
            visible: s.visible,
            left_down: s.left_down,
            right_down: s.right_down,
        });
    }

    // Before first or after last sample
    if idx == 0 {
        let s = &samples[0];
        return Some(SmoothedPosition {
            x: s.x as f64,
            y: s.y as f64,
            visible: s.visible,
            left_down: s.left_down,
            right_down: s.right_down,
        });
    }
    if idx >= samples.len() {
        let s = samples.last().unwrap();
        return Some(SmoothedPosition {
            x: s.x as f64,
            y: s.y as f64,
            visible: s.visible,
            left_down: s.left_down,
            right_down: s.right_down,
        });
    }

    let s0 = &samples[idx - 1];
    let s1 = &samples[idx];

    // Linear interpolation parameter.
    let range = (s1.timestamp_us - s0.timestamp_us) as f64;
    let t = if range > 0.0 {
        (timestamp_us - s0.timestamp_us) as f64 / range
    } else {
        0.0
    };

    // If we have 4 points around the target, use Catmull-Rom.
    if idx >= 2 && idx + 1 < samples.len() {
        let p0 = &samples[idx - 2];
        let p1 = s0;
        let p2 = s1;
        let p3 = &samples[idx + 1];

        let x = catmull_rom(p0.x as f64, p1.x as f64, p2.x as f64, p3.x as f64, t);
        let y = catmull_rom(p0.y as f64, p1.y as f64, p2.y as f64, p3.y as f64, t);

        return Some(SmoothedPosition {
            x,
            y,
            visible: if t < 0.5 { s0.visible } else { s1.visible },
            left_down: if t < 0.5 { s0.left_down } else { s1.left_down },
            right_down: if t < 0.5 { s0.right_down } else { s1.right_down },
        });
    }

    // Fallback: linear interpolation.
    Some(SmoothedPosition {
        x: s0.x as f64 + (s1.x - s0.x) as f64 * t,
        y: s0.y as f64 + (s1.y - s0.y) as f64 * t,
        visible: if t < 0.5 { s0.visible } else { s1.visible },
        left_down: if t < 0.5 { s0.left_down } else { s1.left_down },
        right_down: if t < 0.5 { s0.right_down } else { s1.right_down },
    })
}

/// Catmull-Rom spline interpolation between p1 and p2.
/// p0 and p3 are the surrounding control points. t is in [0, 1].
fn catmull_rom(p0: f64, p1: f64, p2: f64, p3: f64, t: f64) -> f64 {
    let t2 = t * t;
    let t3 = t2 * t;
    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

// ── Idle detection ──────────────────────────────────────────────────────

/// A period where the cursor was stationary.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IdlePeriod {
    pub start_us: u64,
    pub end_us: u64,
    pub x: i32,
    pub y: i32,
}

/// Detect periods where the cursor stayed within a small radius for
/// longer than `threshold_us` microseconds.
pub fn detect_idle_periods(
    samples: &[CursorSample],
    threshold_us: u64,
    radius_px: f64,
) -> Vec<IdlePeriod> {
    if samples.len() < 2 {
        return Vec::new();
    }

    let mut periods = Vec::new();
    let mut idle_start_idx = 0;
    let mut anchor_x = samples[0].x as f64;
    let mut anchor_y = samples[0].y as f64;

    for i in 1..samples.len() {
        let dx = samples[i].x as f64 - anchor_x;
        let dy = samples[i].y as f64 - anchor_y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance > radius_px {
            // Movement detected — check if the idle period was long enough.
            let duration = samples[i - 1].timestamp_us.saturating_sub(samples[idle_start_idx].timestamp_us);
            if duration >= threshold_us {
                periods.push(IdlePeriod {
                    start_us: samples[idle_start_idx].timestamp_us,
                    end_us: samples[i - 1].timestamp_us,
                    x: anchor_x.round() as i32,
                    y: anchor_y.round() as i32,
                });
            }
            idle_start_idx = i;
            anchor_x = samples[i].x as f64;
            anchor_y = samples[i].y as f64;
        }
    }

    // Check final segment.
    let last = samples.last().unwrap();
    let duration = last.timestamp_us.saturating_sub(samples[idle_start_idx].timestamp_us);
    if duration >= threshold_us {
        periods.push(IdlePeriod {
            start_us: samples[idle_start_idx].timestamp_us,
            end_us: last.timestamp_us,
            x: anchor_x.round() as i32,
            y: anchor_y.round() as i32,
        });
    }

    periods
}

// ── Zoom trigger detection ──────────────────────────────────────────────

/// A suggested zoom region based on cursor activity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomTrigger {
    /// Timestamp of the trigger event.
    pub timestamp_us: u64,
    /// Center of the zoom target.
    pub x: i32,
    pub y: i32,
    /// What caused the trigger.
    pub reason: ZoomTriggerReason,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomTriggerReason {
    /// User clicked — good candidate for a zoom-in.
    Click,
    /// Cursor settled after fast motion — user is focusing on something.
    SettleAfterMotion,
}

/// Detect moments that would be good candidates for auto-zoom.
/// Returns trigger points based on clicks and settle-after-motion patterns.
pub fn detect_zoom_triggers(
    samples: &[CursorSample],
    clicks: &[super::CursorClickEvent],
) -> Vec<ZoomTrigger> {
    let mut triggers = Vec::new();

    // Every click-down is a zoom trigger candidate.
    for click in clicks {
        if click.phase == "down" {
            triggers.push(ZoomTrigger {
                timestamp_us: click.timestamp_us,
                x: click.x,
                y: click.y,
                reason: ZoomTriggerReason::Click,
            });
        }
    }

    // Detect settle-after-motion: high velocity followed by low velocity.
    if samples.len() >= 3 {
        let velocity_threshold = 2000.0; // px/sec — fast motion
        let settle_threshold = 200.0; // px/sec — settled

        for window in samples.windows(3) {
            let prev_speed =
                (window[0].velocity_x.powi(2) + window[0].velocity_y.powi(2)).sqrt();
            let curr_speed =
                (window[1].velocity_x.powi(2) + window[1].velocity_y.powi(2)).sqrt();
            let next_speed =
                (window[2].velocity_x.powi(2) + window[2].velocity_y.powi(2)).sqrt();

            // Was moving fast, now stopped.
            if prev_speed > velocity_threshold
                && curr_speed < settle_threshold
                && next_speed < settle_threshold
            {
                triggers.push(ZoomTrigger {
                    timestamp_us: window[1].timestamp_us,
                    x: window[1].x,
                    y: window[1].y,
                    reason: ZoomTriggerReason::SettleAfterMotion,
                });
            }
        }
    }

    // Sort by timestamp and deduplicate triggers that are too close together.
    triggers.sort_by_key(|t| t.timestamp_us);
    dedup_triggers(&mut triggers, 500_000); // 500ms minimum gap
    triggers
}

fn dedup_triggers(triggers: &mut Vec<ZoomTrigger>, min_gap_us: u64) {
    if triggers.len() < 2 {
        return;
    }
    let mut write = 1;
    for read in 1..triggers.len() {
        if triggers[read].timestamp_us - triggers[write - 1].timestamp_us >= min_gap_us {
            triggers[write] = triggers[read].clone();
            write += 1;
        }
    }
    triggers.truncate(write);
}

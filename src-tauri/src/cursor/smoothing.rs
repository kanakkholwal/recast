use serde::{Deserialize, Serialize};

use super::CursorSample;

// Per-frame cursor smoothing & interpolation now run in the WebGL2 preview
// compositor (src/components/editor/VideoPreview.svelte). Only idle / zoom
// detection — needed at recording-stop time — remains in this module.

// ── Idle detection 

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

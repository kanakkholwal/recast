use serde::{Deserialize, Serialize};

use super::CursorSample;

// Per-frame cursor smoothing & interpolation now run in the WebGL2 preview
// compositor (src/components/editor/VideoPreview.svelte). Only idle / zoom
// detection — needed at recording-stop time — remains in this module.

//  Idle detection

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
            let duration = samples[i - 1]
                .timestamp_us
                .saturating_sub(samples[idle_start_idx].timestamp_us);
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
    let duration = last
        .timestamp_us
        .saturating_sub(samples[idle_start_idx].timestamp_us);
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

//  Zoom trigger detection
//
// A zoom should land where the viewer *needs* to look — a deliberate
// interaction the user lingered on — not on every stray click. The old
// detector emitted one trigger per click-down, so a recording with 40 clicks
// produced ~40 zoom suggestions and the editor felt like it was zooming
// constantly.
//
// The refined pipeline below mirrors what polished screen-recorder tools do:
//
//   1. Cluster — consecutive clicks close in time *and* space are one
//      interaction (double-clicks, drag-selects, typing into a field), so
//      they collapse to a single candidate.
//   2. Score — each candidate gets a confidence in [0,1] from how
//      deliberate it looks: did the cursor travel a long way to get there
//      (a targeted action) and did it dwell afterwards (the user is
//      actually looking at the result)? Drive-by clicks score low and are
//      dropped.
//   3. Settle — "settled after fast motion" only counts when the move
//      covered real ground *and* the cursor then held still for a real
//      beat, not a 3-sample flicker.
//   4. Select — greedily keep the highest-scoring candidates subject to a
//      minimum spacing, a same-spot guard, and an overall density budget
//      (~one zoom per 9 s of footage) so the result reads as intentional.

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
    /// Confidence in [0,1] — how strongly this moment warrants a zoom.
    /// Used to rank candidates when the density budget is exceeded.
    /// `serde(default)` so triggers persisted before scoring landed still
    /// deserialize (they get 0.0 and are recomputed on next analysis).
    #[serde(default)]
    pub score: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ZoomTriggerReason {
    /// User clicked — good candidate for a zoom-in.
    Click,
    /// Cursor settled after fast motion — user is focusing on something.
    SettleAfterMotion,
}

// --- Tunables ----------------------------------------------------------
// Clicks closer than this in time AND space are the same interaction.
const CLICK_CLUSTER_GAP_US: u64 = 1_400_000;
const CLICK_CLUSTER_RADIUS_PX: f64 = 160.0;
// Two kept triggers must be at least this far apart. The placed zoom window
// runs ~3 s (a short lead-in plus a long hold), so this leaves a small gap
// between consecutive zooms instead of letting them stack back-to-back.
const MIN_TRIGGER_GAP_US: u64 = 3_500_000;
// A trigger landing within this radius of an earlier kept trigger (and not
// far apart in time) is "the same place" — a second zoom there adds nothing.
const SAME_SPOT_RADIUS_PX: f64 = 220.0;
// Density budget: at most ~one trigger per this much footage on average.
const TRIGGER_BUDGET_US: u64 = 9_000_000;
// Settle-after-motion gates: the move must cover real ground …
const SETTLE_MIN_TRAVEL_PX: f64 = 320.0;
const FAST_MOTION_PX_S: f64 = 1_500.0;
const SETTLE_SPEED_PX_S: f64 = 260.0;
// … and the cursor must then hold still for a real beat.
const SETTLE_MIN_DWELL_US: u64 = 380_000;
const SETTLE_DWELL_RADIUS_PX: f64 = 90.0;
// A candidate must clear this score to be considered at all.
const MIN_CANDIDATE_SCORE: f32 = 0.40;

/// Internal scored candidate before gap/budget selection.
#[derive(Clone)]
struct Candidate {
    timestamp_us: u64,
    x: i32,
    y: i32,
    reason: ZoomTriggerReason,
    score: f32,
}

fn dist_px(ax: i32, ay: i32, bx: i32, by: i32) -> f64 {
    let dx = (ax - bx) as f64;
    let dy = (ay - by) as f64;
    (dx * dx + dy * dy).sqrt()
}

fn sample_speed(s: &CursorSample) -> f64 {
    ((s.velocity_x as f64).powi(2) + (s.velocity_y as f64).powi(2)).sqrt()
}

/// Index of the sample nearest to `ts` (binary search; samples are sorted).
fn nearest_idx(samples: &[CursorSample], ts: u64) -> Option<usize> {
    if samples.is_empty() {
        return None;
    }
    let mut lo = 0usize;
    let mut hi = samples.len() - 1;
    while lo < hi {
        let mid = (lo + hi) / 2;
        if samples[mid].timestamp_us < ts {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Some(lo)
}

/// How long, starting at `from`, the cursor stays within `radius` of the
/// position at `from`. Returns (last_index, duration_us).
fn dwell_from(samples: &[CursorSample], from: usize, radius: f64) -> (usize, u64) {
    let (ax, ay) = (samples[from].x, samples[from].y);
    let mut end = from;
    for k in (from + 1)..samples.len() {
        if dist_px(samples[k].x, samples[k].y, ax, ay) > radius {
            break;
        }
        end = k;
    }
    let dur = samples[end]
        .timestamp_us
        .saturating_sub(samples[from].timestamp_us);
    (end, dur)
}

/// How far the cursor travelled in the ~600 ms leading up to `ts` — a long
/// approach means the click was a deliberate target acquisition.
fn travel_before(samples: &[CursorSample], ts: u64) -> f64 {
    let Some(end) = nearest_idx(samples, ts) else {
        return 0.0;
    };
    let start_ts = ts.saturating_sub(600_000);
    let Some(start) = nearest_idx(samples, start_ts) else {
        return 0.0;
    };
    if start >= end {
        return 0.0;
    }
    dist_px(
        samples[start].x,
        samples[start].y,
        samples[end].x,
        samples[end].y,
    )
}

/// Collapse click-downs into interaction clusters and score each one.
fn cluster_click_candidates(
    clicks: &[super::CursorClickEvent],
    samples: &[CursorSample],
) -> Vec<Candidate> {
    let mut downs: Vec<&super::CursorClickEvent> =
        clicks.iter().filter(|c| c.phase == "down").collect();
    downs.sort_by_key(|c| c.timestamp_us);

    // Group consecutive downs that are near in both time and space.
    let mut clusters: Vec<Vec<&super::CursorClickEvent>> = Vec::new();
    for c in downs {
        let extend = clusters.last().map_or(false, |cur| {
            let last = cur.last().unwrap();
            c.timestamp_us.saturating_sub(last.timestamp_us) <= CLICK_CLUSTER_GAP_US
                && dist_px(last.x, last.y, c.x, c.y) <= CLICK_CLUSTER_RADIUS_PX
        });
        if extend {
            clusters.last_mut().unwrap().push(c);
        } else {
            clusters.push(vec![c]);
        }
    }

    let mut out = Vec::with_capacity(clusters.len());
    for cluster in clusters {
        let n = cluster.len();
        let first = cluster.first().unwrap();
        let last = cluster.last().unwrap();
        let first_ts = first.timestamp_us;
        let last_ts = last.timestamp_us;
        // Focus the zoom on the *final* click of the interaction — the target
        // the user ended on. A centroid would strand the focus point in the
        // empty space between two clicks, which reads as "off" from where the
        // user actually clicked.
        let (cx, cy) = (last.x, last.y);

        // Base confidence: a click on its own is weak evidence.
        let mut score: f32 = 0.30;
        // A multi-click cluster (double-click, drag-select) is deliberate.
        if n >= 2 {
            score += 0.12;
        }
        // The user travelled a long way to land this click.
        if travel_before(samples, first_ts) > 260.0 {
            score += 0.18;
        }
        // The user then dwelled near the click — they're inspecting a result.
        if let Some(after) = nearest_idx(samples, last_ts) {
            let (_, dwell) = dwell_from(samples, after, CLICK_CLUSTER_RADIUS_PX);
            score += ((dwell as f64 / 1_500_000.0).min(1.0) * 0.30) as f32;
        }

        out.push(Candidate {
            timestamp_us: first_ts,
            x: cx,
            y: cy,
            reason: ZoomTriggerReason::Click,
            score: score.clamp(0.0, 1.0),
        });
    }
    out
}

/// Detect genuine settle-after-motion moments: a sustained fast move that
/// covers real distance and is followed by a real dwell.
fn settle_candidates(samples: &[CursorSample]) -> Vec<Candidate> {
    let mut out = Vec::new();
    if samples.len() < 4 {
        return out;
    }
    let mut i = 1usize;
    while i < samples.len() {
        if sample_speed(&samples[i]) <= FAST_MOTION_PX_S {
            i += 1;
            continue;
        }
        // Fast-motion run starts here; advance through the deceleration
        // until the cursor drops below the settle speed.
        let run_start = i;
        let mut j = i;
        while j < samples.len() && sample_speed(&samples[j]) > SETTLE_SPEED_PX_S {
            j += 1;
        }
        if j >= samples.len() {
            break;
        }
        // Distance covered along the run path.
        let mut travel = 0.0;
        for w in samples[run_start..=j].windows(2) {
            travel += dist_px(w[0].x, w[0].y, w[1].x, w[1].y);
        }
        let (dwell_end, dwell_us) = dwell_from(samples, j, SETTLE_DWELL_RADIUS_PX);

        if travel >= SETTLE_MIN_TRAVEL_PX && dwell_us >= SETTLE_MIN_DWELL_US {
            let mut score: f32 = 0.45;
            score += (((travel - SETTLE_MIN_TRAVEL_PX) / 700.0).clamp(0.0, 1.0) * 0.25) as f32;
            score += (((dwell_us.saturating_sub(SETTLE_MIN_DWELL_US)) as f64 / 1_200_000.0)
                .min(1.0)
                * 0.30) as f32;
            out.push(Candidate {
                timestamp_us: samples[j].timestamp_us,
                x: samples[j].x,
                y: samples[j].y,
                reason: ZoomTriggerReason::SettleAfterMotion,
                score: score.clamp(0.0, 1.0),
            });
            i = dwell_end + 1; // skip past the dwell — don't re-detect it
        } else {
            i = j + 1;
        }
    }
    out
}

/// Detect moments that would be good candidates for auto-zoom.
///
/// See the module-level comment above for the four-stage pipeline. The
/// returned triggers are sorted by timestamp and already pruned to a sane
/// density — the caller can place them all without flooding the timeline.
pub fn detect_zoom_triggers(
    samples: &[CursorSample],
    clicks: &[super::CursorClickEvent],
) -> Vec<ZoomTrigger> {
    // 1 + 2: gather scored candidates from both signals.
    let mut candidates = cluster_click_candidates(clicks, samples);
    candidates.extend(settle_candidates(samples));
    candidates.retain(|c| c.score >= MIN_CANDIDATE_SCORE);
    if candidates.is_empty() {
        return Vec::new();
    }

    // Density budget from the recording span.
    let span = samples
        .last()
        .map(|s| s.timestamp_us)
        .unwrap_or(0)
        .saturating_sub(samples.first().map(|s| s.timestamp_us).unwrap_or(0));
    let budget = ((span / TRIGGER_BUDGET_US) as usize).max(1);

    // 3: greedy selection by score — strongest candidate first — enforcing
    // a minimum gap and a same-spot guard against everything kept so far.
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.timestamp_us.cmp(&b.timestamp_us))
    });
    let mut accepted: Vec<Candidate> = Vec::new();
    for c in candidates {
        if accepted.len() >= budget {
            break;
        }
        let conflict = accepted.iter().any(|a| {
            let dt = a.timestamp_us.abs_diff(c.timestamp_us);
            dt < MIN_TRIGGER_GAP_US
                || (dt < MIN_TRIGGER_GAP_US * 3
                    && dist_px(a.x, a.y, c.x, c.y) < SAME_SPOT_RADIUS_PX)
        });
        if !conflict {
            accepted.push(c);
        }
    }

    // 4: emit in timeline order.
    accepted.sort_by_key(|c| c.timestamp_us);
    accepted
        .into_iter()
        .map(|c| ZoomTrigger {
            timestamp_us: c.timestamp_us,
            x: c.x,
            y: c.y,
            reason: c.reason,
            score: c.score,
        })
        .collect()
}

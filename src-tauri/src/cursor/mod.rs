mod platform;
pub mod smoothing;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use platform::sample_cursor_state;
use smoothing::{IdlePeriod, ZoomTrigger, detect_idle_periods, detect_zoom_triggers};

// ── Data types ──────────────────────────────────────────────────────────

/// Raw cursor position and button state at a single point in time.
#[derive(Debug, Clone, Copy)]
pub struct CursorState {
    pub x: i32,
    pub y: i32,
    pub visible: bool,
    pub left_down: bool,
    pub right_down: bool,
}

/// A timestamped cursor sample with computed velocity.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorSample {
    pub timestamp_us: u64,
    pub x: i32,
    pub y: i32,
    pub velocity_x: f32,
    pub velocity_y: f32,
    pub visible: bool,
    pub left_down: bool,
    pub right_down: bool,
}

/// A click event with duration tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorClickEvent {
    pub timestamp_us: u64,
    pub button: String,
    pub phase: String,
    pub x: i32,
    pub y: i32,
    /// Duration of the click in microseconds (set on "up" events, 0 on "down").
    #[serde(default)]
    pub duration_us: u64,
}

/// Complete cursor recording — samples, clicks, idle periods, and zoom triggers.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct CursorTrack {
    pub samples: Vec<CursorSample>,
    pub clicks: Vec<CursorClickEvent>,
    /// Periods where the cursor was stationary (computed post-capture).
    #[serde(default)]
    pub idle_periods: Vec<IdlePeriod>,
    /// Suggested zoom trigger points (computed post-capture).
    #[serde(default)]
    pub zoom_triggers: Vec<ZoomTrigger>,
}

// ── Capture loop ────────────────────────────────────────────────────────

/// State for tracking click duration during capture.
struct ClickTracker {
    left_down_at: Option<(u64, i32, i32)>, // (timestamp, x, y)
    right_down_at: Option<(u64, i32, i32)>,
}

impl ClickTracker {
    fn new() -> Self {
        Self {
            left_down_at: None,
            right_down_at: None,
        }
    }

    fn update(
        &mut self,
        now_us: u64,
        current: &CursorState,
        prev: &CursorState,
        clicks: &mut Vec<CursorClickEvent>,
    ) {
        // Left button
        if current.left_down && !prev.left_down {
            self.left_down_at = Some((now_us, current.x, current.y));
            clicks.push(CursorClickEvent {
                timestamp_us: now_us,
                button: "left".into(),
                phase: "down".into(),
                x: current.x,
                y: current.y,
                duration_us: 0,
            });
        } else if !current.left_down && prev.left_down {
            let duration = self
                .left_down_at
                .map(|(t, _, _)| now_us.saturating_sub(t))
                .unwrap_or(0);
            clicks.push(CursorClickEvent {
                timestamp_us: now_us,
                button: "left".into(),
                phase: "up".into(),
                x: current.x,
                y: current.y,
                duration_us: duration,
            });
            self.left_down_at = None;
        }

        // Right button
        if current.right_down && !prev.right_down {
            self.right_down_at = Some((now_us, current.x, current.y));
            clicks.push(CursorClickEvent {
                timestamp_us: now_us,
                button: "right".into(),
                phase: "down".into(),
                x: current.x,
                y: current.y,
                duration_us: 0,
            });
        } else if !current.right_down && prev.right_down {
            let duration = self
                .right_down_at
                .map(|(t, _, _)| now_us.saturating_sub(t))
                .unwrap_or(0);
            clicks.push(CursorClickEvent {
                timestamp_us: now_us,
                button: "right".into(),
                phase: "up".into(),
                x: current.x,
                y: current.y,
                duration_us: duration,
            });
            self.right_down_at = None;
        }
    }
}

/// Spawn a thread that samples cursor state at ~125 Hz until the stop flag is set.
/// Post-capture, computes idle periods and zoom triggers.
pub fn spawn_cursor_capture(
    stop_flag: Arc<AtomicBool>,
    clock: Instant,
) -> Result<thread::JoinHandle<CursorTrack>> {
    thread::Builder::new()
        .name("recast-cursor".into())
        .spawn(move || {
            let mut track = CursorTrack::default();
            let mut previous: Option<(CursorState, u64)> = None;
            let mut click_tracker = ClickTracker::new();

            while !stop_flag.load(Ordering::Acquire) {
                let now_us = clock.elapsed().as_micros() as u64;
                if let Some(current) = sample_cursor_state() {
                    let (velocity_x, velocity_y) = previous
                        .map(|(prev, prev_ts)| {
                            let delta_t =
                                ((now_us.saturating_sub(prev_ts)).max(1)) as f32 / 1_000_000.0;
                            (
                                (current.x - prev.x) as f32 / delta_t,
                                (current.y - prev.y) as f32 / delta_t,
                            )
                        })
                        .unwrap_or((0.0, 0.0));

                    // Track clicks with duration.
                    if let Some((prev, _)) = previous {
                        click_tracker.update(now_us, &current, &prev, &mut track.clicks);
                    }

                    track.samples.push(CursorSample {
                        timestamp_us: now_us,
                        x: current.x,
                        y: current.y,
                        velocity_x,
                        velocity_y,
                        visible: current.visible,
                        left_down: current.left_down,
                        right_down: current.right_down,
                    });
                    previous = Some((current, now_us));
                }

                thread::sleep(Duration::from_millis(8));
            }

            // Post-capture analysis: detect idle periods and zoom triggers.
            // Idle: cursor within 5px radius for > 2 seconds.
            track.idle_periods = detect_idle_periods(&track.samples, 2_000_000, 5.0);
            track.zoom_triggers = detect_zoom_triggers(&track.samples, &track.clicks);

            track
        })
        .map_err(Into::into)
}

// ── Serialization ───────────────────────────────────────────────────────

/// Write a cursor track to a JSON file.
pub fn write_cursor_track(path: &Path, track: &CursorTrack) -> Result<()> {
    std::fs::write(path, serde_json::to_vec_pretty(track)?)?;
    Ok(())
}

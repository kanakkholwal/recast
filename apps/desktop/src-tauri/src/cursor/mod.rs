mod platform;
pub mod smoothing;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use crate::recording::RecordingClock;

use platform::sample_cursor_state;
use smoothing::{detect_idle_periods, detect_zoom_triggers, IdlePeriod, ZoomTrigger};

//  Data types

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

//  Capture loop

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

/// Pixel-space rectangle of the recorded frame inside the virtual desktop.
/// `GetCursorPos` returns coordinates in virtual-desktop space (with each
/// monitor offset by its position in the display arrangement); the recorded
/// video is in frame-relative pixel space (0..width, 0..height). Without
/// this conversion, recording a secondary monitor or a cropped region puts
/// every cursor sample outside the [0..frame] range, and the editor
/// renders the cursor at clamped/wrapped positions for the entire video.
#[derive(Debug, Clone, Copy)]
pub struct CursorCaptureFrame {
    /// Top-left of the recorded frame, in physical device pixels (same space
    /// as `width`/`height` and the encoded video).
    pub origin_x: i32,
    pub origin_y: i32,
    /// Recorded frame size in physical pixels.
    pub width: u32,
    pub height: u32,
    /// Multiplier applied to each raw sample before mapping into frame space.
    /// macOS samples the cursor in logical points while the video is physical
    /// pixels, so this is the display's backing scale there; 1.0 on
    /// Windows/Linux, where samples are already physical.
    pub scale: f32,
}

/// Spawn a thread that samples cursor state at 125 Hz until the stop flag
/// is set. Post-capture, computes idle periods and zoom triggers.
///
/// The capture loop:
/// - Uses deadline-based scheduling (not `thread::sleep(8ms)` which drifts
///   under load) so sample cadence stays uniform across long recordings.
///   Falls back to a fresh baseline if we fall more than one period behind,
///   which prevents burst catch-up after a long pause.
/// - Converts cursor coordinates from virtual-desktop space to
///   frame-relative pixel space using `frame.origin_*`. Samples whose
///   cursor lies outside the frame are recorded with `visible = false` so
///   the editor's `if (pos.visible)` gate hides the cursor cleanly when
///   the user moves the mouse off the captured area, instead of rendering
///   it at a clamped edge.
/// - Logs at most one warning when `GetCursorPos` starts failing
///   (rare — mostly UAC / secure-desktop transitions) so silent gaps in
///   the track are observable in the recording log.
pub fn spawn_cursor_capture(
    stop_flag: Arc<AtomicBool>,
    clock: RecordingClock,
    frame: CursorCaptureFrame,
) -> Result<thread::JoinHandle<CursorTrack>> {
    thread::Builder::new()
        .name("recast-cursor".into())
        .spawn(move || {
            let mut track = CursorTrack::default();
            let mut previous: Option<(CursorState, u64)> = None;
            let mut click_tracker = ClickTracker::new();
            let mut platform_failure_logged = false;

            const SAMPLE_PERIOD: Duration = Duration::from_micros(8_000); // 125 Hz
            let start = Instant::now();
            let mut next_tick = start + SAMPLE_PERIOD;
            let frame_w = frame.width as i32;
            let frame_h = frame.height as i32;
            // Lift logical samples into the video's physical pixel space (macOS);
            // 1.0 elsewhere makes this an exact identity.
            let sample_scale = frame.scale as f64;

            while !stop_flag.load(Ordering::Acquire) {
                // While paused, stop sampling. The effective clock is frozen
                // anyway; skipping keeps the track free of a run of
                // identically-timestamped samples.
                if clock.is_paused() {
                    thread::sleep(SAMPLE_PERIOD);
                    next_tick = Instant::now() + SAMPLE_PERIOD;
                    continue;
                }
                let now_us = clock.effective_elapsed().as_micros() as u64;
                match sample_cursor_state() {
                    Some(raw) => {
                        // Map virtual-desktop coords to frame-relative coords.
                        // We keep the math in i32 so cursors that wander off
                        // the captured area produce negative / over-range x/y
                        // — but we record `visible = false` for those so the
                        // editor doesn't draw a cursor outside the frame.
                        let mapped_x =
                            (raw.x as f64 * sample_scale).round() as i32 - frame.origin_x;
                        let mapped_y =
                            (raw.y as f64 * sample_scale).round() as i32 - frame.origin_y;
                        let on_frame = mapped_x >= 0
                            && mapped_y >= 0
                            && mapped_x < frame_w
                            && mapped_y < frame_h;
                        let current = CursorState {
                            x: mapped_x,
                            y: mapped_y,
                            visible: raw.visible && on_frame,
                            left_down: raw.left_down,
                            right_down: raw.right_down,
                        };

                        let (velocity_x, velocity_y) = previous
                            .map(|(prev, prev_ts): (CursorState, u64)| {
                                let delta_t =
                                    ((now_us.saturating_sub(prev_ts)).max(1)) as f32 / 1_000_000.0;
                                (
                                    (current.x - prev.x) as f32 / delta_t,
                                    (current.y - prev.y) as f32 / delta_t,
                                )
                            })
                            .unwrap_or((0.0, 0.0));

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
                    None => {
                        if !platform_failure_logged {
                            log::warn!(
                                "cursor capture: sample_cursor_state() returned None; \
                                 cursor track will have gaps until the platform recovers"
                            );
                            platform_failure_logged = true;
                        }
                    }
                }

                // Deadline-based sleep: target the next tick exactly,
                // independent of how long the sampling itself took.
                let now = Instant::now();
                if next_tick > now {
                    thread::sleep(next_tick - now);
                } else if now > next_tick + SAMPLE_PERIOD {
                    // Fell more than one period behind (system stall, GC,
                    // etc.). Reset the baseline so we don't fire a burst
                    // of catch-up samples on the next recovery.
                    next_tick = now;
                }
                next_tick += SAMPLE_PERIOD;
            }

            // Post-capture analysis: detect idle periods and zoom triggers.
            // Idle: cursor within 5px radius for > 2 seconds.
            track.idle_periods = detect_idle_periods(&track.samples, 2_000_000, 5.0);
            track.zoom_triggers = detect_zoom_triggers(&track.samples, &track.clicks);

            track
        })
        .map_err(Into::into)
}

//  Serialization

/// Write a cursor track to a JSON file.
pub fn write_cursor_track(path: &Path, track: &CursorTrack) -> Result<()> {
    // Temp + fsync + rename: a truncated cursor.json makes the recording's
    // cursor and zoom data unrecoverable.
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(track)?;
    if let Err(e) = crate::commands::system::write_atomic(&tmp, path, &bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

/// Shift every timestamp in the track earlier by `offset_us`, clamping at 0.
///
/// The cursor clock starts at recording start, but the video timeline is
/// frame-count based and its first encoded frame is whatever the DXGI
/// duplication warmup produced first — i.e. video t=0 corresponds to
/// wall-clock `offset_us`, not 0. Left uncorrected, the whole cursor track
/// runs ahead of the video by the warmup, so clicks and the click highlight
/// land that far off from the on-screen action (the ~half-second the user
/// reported). Subtracting the offset re-bases the cursor track onto the video
/// clock. Samples captured during the warmup (before the first frame) clamp to
/// 0 — they have no corresponding video and collapse onto the first frame.
pub fn shift_cursor_track(track: &mut CursorTrack, offset_us: u64) {
    if offset_us == 0 {
        return;
    }
    for s in &mut track.samples {
        s.timestamp_us = s.timestamp_us.saturating_sub(offset_us);
    }
    for c in &mut track.clicks {
        c.timestamp_us = c.timestamp_us.saturating_sub(offset_us);
    }
    for p in &mut track.idle_periods {
        p.start_us = p.start_us.saturating_sub(offset_us);
        p.end_us = p.end_us.saturating_sub(offset_us);
    }
    for z in &mut track.zoom_triggers {
        z.timestamp_us = z.timestamp_us.saturating_sub(offset_us);
    }
}

#[cfg(test)]
mod tests {
    use super::smoothing::ZoomTriggerReason;
    use super::*;

    fn sample(ts: u64) -> CursorSample {
        CursorSample {
            timestamp_us: ts,
            x: 0,
            y: 0,
            velocity_x: 0.0,
            velocity_y: 0.0,
            visible: true,
            left_down: false,
            right_down: false,
        }
    }

    #[test]
    fn shift_by_zero_is_a_noop() {
        let mut track = CursorTrack::default();
        track.samples.push(sample(5_000));
        shift_cursor_track(&mut track, 0);
        assert_eq!(track.samples[0].timestamp_us, 5_000);
    }

    #[test]
    fn shift_subtracts_offset_across_every_track_and_saturates_at_zero() {
        let mut track = CursorTrack {
            samples: vec![sample(1_000), sample(5_000)],
            clicks: vec![CursorClickEvent {
                timestamp_us: 500,
                button: "left".into(),
                phase: "down".into(),
                x: 0,
                y: 0,
                duration_us: 0,
            }],
            idle_periods: vec![IdlePeriod {
                start_us: 1_000,
                end_us: 4_000,
                x: 0,
                y: 0,
            }],
            zoom_triggers: vec![ZoomTrigger {
                timestamp_us: 6_000,
                x: 0,
                y: 0,
                reason: ZoomTriggerReason::Click,
                score: 0.5,
            }],
        };

        shift_cursor_track(&mut track, 2_000);

        // A sample/click before the offset clamps to 0; later ones shift down.
        assert_eq!(track.samples[0].timestamp_us, 0);
        assert_eq!(track.samples[1].timestamp_us, 3_000);
        assert_eq!(track.clicks[0].timestamp_us, 0);
        // Idle windows shift both ends, clamping the start.
        assert_eq!(track.idle_periods[0].start_us, 0);
        assert_eq!(track.idle_periods[0].end_us, 2_000);
        // Zoom triggers shift too.
        assert_eq!(track.zoom_triggers[0].timestamp_us, 4_000);
    }
}

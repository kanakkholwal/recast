pub mod smoothing;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use capturekit::Timestamp;
use serde::{Deserialize, Serialize};

use crate::recording::{RecordingClock, TrackStart};

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
/// The OS reports the pointer in virtual-desktop space while the video is frame-relative, so without this a secondary monitor or region puts every sample outside the frame.
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

impl CursorCaptureFrame {
    /// Open the platform pointer reader for this frame.
    /// The OS access, the button state and the virtual-desktop mapping all live in capturekit now, so all three agree with what the recorder captured.
    fn open_pointer(&self) -> Result<capturekit::PointerCapturer> {
        Ok(capturekit::PointerCapturer::open(
            capturekit::Rect {
                x: self.origin_x,
                y: self.origin_y,
                width: self.width,
                height: self.height,
            },
            f64::from(self.scale),
        )?)
    }
}

/// Samples cursor state at 125 Hz on a deadline schedule until stopped, then derives idle periods and zoom triggers.
/// Stamped in VIDEO time and blocked until the first encoded frame: sampling earlier piled every sample at t=0, teleporting the cursor.
pub fn spawn_cursor_capture(
    stop_flag: Arc<AtomicBool>,
    clock: RecordingClock,
    video_start: TrackStart,
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
            let mut pointer = match frame.open_pointer() {
                Ok(pointer) => pointer,
                Err(error) => {
                    log::warn!("cursor capture: no pointer reader ({error}); track will be empty");
                    return track;
                }
            };
            // Video t=0 is the first encoded frame; the source warms up first.
            let video_zero_us = loop {
                if let Some(us) = video_start.elapsed_us() {
                    break us;
                }
                if stop_flag.load(Ordering::Acquire) {
                    return track;
                }
                thread::sleep(SAMPLE_PERIOD);
            };
            // Paced from the first frame, or the warmup is owed as catch-up ticks.
            let mut next_tick = Instant::now() + SAMPLE_PERIOD;

            while !stop_flag.load(Ordering::Acquire) {
                // While paused the effective clock is frozen, so skipping keeps the track free of identically-timestamped samples.
                if clock.is_paused() {
                    thread::sleep(SAMPLE_PERIOD);
                    next_tick = Instant::now() + SAMPLE_PERIOD;
                    continue;
                }
                let now_us =
                    (clock.effective_elapsed().as_micros() as u64).saturating_sub(video_zero_us);
                match pointer.sample(Timestamp::from_micros(now_us as i64)) {
                    Some(read) => {
                        // Unclamped: a cursor that wanders off must keep moving in the track.
                        let current = CursorState {
                            x: read.offset.0,
                            y: read.offset.1,
                            visible: read.cursor.visible,
                            left_down: read.cursor.buttons.left,
                            right_down: read.cursor.buttons.right,
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

                // Deadline-based sleep: target the next tick exactly, independent of how long the sampling took.
                let now = Instant::now();
                if next_tick > now {
                    thread::sleep(next_tick - now);
                } else if now > next_tick + SAMPLE_PERIOD {
                    // More than a period behind (a system stall), so reset the baseline instead of firing a burst of catch-up samples.
                    next_tick = now;
                }
                next_tick += SAMPLE_PERIOD;
            }

            // Post-capture analysis: idle is the cursor within a 5px radius for over 2 seconds.
            track.idle_periods = detect_idle_periods(&track.samples, 2_000_000, 5.0);
            track.zoom_triggers = detect_zoom_triggers(&track.samples, &track.clicks);

            track
        })
        .map_err(Into::into)
}

//  Serialization

/// Write a cursor track to a JSON file.
pub fn write_cursor_track(path: &Path, track: &CursorTrack) -> Result<()> {
    // Temp, fsync, rename: a truncated cursor.json makes the recording's cursor and zoom data unrecoverable.
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(track)?;
    if let Err(e) = crate::commands::system::write_atomic(&tmp, path, &bytes) {
        let _ = std::fs::remove_file(&tmp);
        return Err(e.into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Run the capture thread against the real pointer, with `video_start`
    /// marked `warmup` after the thread is already sampling.
    fn track_after_warmup(warmup: Duration) -> CursorTrack {
        let frame = CursorCaptureFrame {
            origin_x: -32_768,
            origin_y: -32_768,
            width: 65_536,
            height: 65_536,
            scale: 1.0,
        };
        let stop = Arc::new(AtomicBool::new(false));
        let started = Instant::now();
        let video_start = TrackStart::new(started);
        let handle = spawn_cursor_capture(
            Arc::clone(&stop),
            RecordingClock::new(started),
            video_start.clone(),
            frame,
        )
        .expect("the thread spawns");
        thread::sleep(warmup);
        video_start.mark();
        thread::sleep(Duration::from_millis(400));
        stop.store(true, Ordering::Release);
        handle.join().expect("the thread joins")
    }

    /// End-to-end over the real pointer: the capture thread, capturekit's
    /// reader and the coordinate mapping together. The pieces have unit tests
    /// either side of the boundary, but only this catches the wiring between
    /// them, which is what the migration off `device_query` actually changed.
    #[test]
    #[ignore = "live: needs a real pointer"]
    fn the_capture_thread_fills_a_track_from_the_real_pointer() {
        if !capturekit::capabilities().cursor_pointer {
            return;
        }
        let track = track_after_warmup(Duration::from_millis(0));

        // 400ms at 125Hz is ~50; anything near zero means it never sampled.
        assert!(
            track.samples.len() > 20,
            "only {} samples in 400ms, so the reader is not keeping its rate",
            track.samples.len()
        );
        assert!(
            track
                .samples
                .windows(2)
                .all(|w| w[1].timestamp_us >= w[0].timestamp_us),
            "timestamps must not go backwards"
        );
        assert!(
            track.samples.iter().all(|s| s.visible),
            "a frame covering the desktop contains the pointer, so every sample draws"
        );
    }

    /// The whole point of taking `video_start`: a 300ms capture warmup must not
    /// put the cursor 300ms ahead of the picture. Before this the track began at
    /// recording start and was re-based afterwards by `shift_cursor_track`,
    /// which clamped every warmup sample onto t=0.
    #[test]
    #[ignore = "live: needs a real pointer"]
    fn samples_are_stamped_from_video_zero_not_from_recording_start() {
        if !capturekit::capabilities().cursor_pointer {
            return;
        }
        const WARMUP: Duration = Duration::from_millis(300);
        let track = track_after_warmup(WARMUP);
        let first = track.samples.first().expect("samples were captured");

        // Stamped from recording start, the first sample would sit at ~300_000.
        assert!(
            first.timestamp_us < 50_000,
            "first sample at {}us, so it is still measured from recording start",
            first.timestamp_us
        );
        // And nothing may pile up at zero the way clamping used to leave it.
        let at_zero = track.samples.iter().filter(|s| s.timestamp_us == 0).count();
        assert!(
            at_zero <= 1,
            "{at_zero} samples share t=0, which is the clamping this replaced"
        );
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_queue::ArrayQueue;

use crate::capture::CaptureSource;

#[derive(Clone)]
#[allow(dead_code)]
pub struct VideoFrame {
    pub timestamp_us: u64,
    pub width: u32,
    pub height: u32,
    pub data: Arc<[u8]>,
}

#[derive(Clone, Default)]
pub struct PipelineStats {
    pub captured_frames: Arc<AtomicU64>,
    pub dropped_frames: Arc<AtomicU64>,
    pub encoded_frames: Arc<AtomicU64>,
}

impl PipelineStats {
    pub fn snapshot(&self) -> PipelineSnapshot {
        PipelineSnapshot {
            captured_frames: self.captured_frames.load(Ordering::Relaxed),
            dropped_frames: self.dropped_frames.load(Ordering::Relaxed),
            encoded_frames: self.encoded_frames.load(Ordering::Relaxed),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct PipelineSnapshot {
    pub captured_frames: u64,
    pub dropped_frames: u64,
    pub encoded_frames: u64,
}

#[derive(Clone)]
pub struct RecordingPipeline {
    queue: Arc<ArrayQueue<VideoFrame>>,
    stats: PipelineStats,
}

impl RecordingPipeline {
    pub fn new(capacity: usize) -> Self {
        Self {
            queue: Arc::new(ArrayQueue::new(capacity)),
            stats: PipelineStats::default(),
        }
    }

    pub fn push(&self, frame: VideoFrame) {
        self.stats.captured_frames.fetch_add(1, Ordering::Relaxed);
        if self.queue.push(frame).is_err() {
            // The queue is full — the encoder is falling behind the
            // pacer. Increment the counter and surface the condition in
            // the log so a recording that comes out choppy has a paper
            // trail. Capacity is sized by `RecordingManager::start` based
            // on the capture resolution (≤256 MB BGRA budget), so the
            // queue holds anywhere from ~8 frames at 4K to 180 at 720p.
            //
            // We log loudly on the first drop of each session so the
            // problem is visible the moment it starts, then dampen to
            // once per ~5 s of sustained dropping (every 300th drop at
            // 60 fps) to avoid flooding the log if the encoder stays
            // behind. The atomic `fetch_add` returns the PRE-increment
            // value, so we treat `0` as "this is the first drop".
            let prev = self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
            if prev == 0 {
                log::warn!(
                    "recording pipeline: frame queue saturated — frames are being \
                     dropped. The encoder is not keeping up with the capture rate. \
                     This will surface as choppy / time-compressed playback. Likely \
                     causes: hardware encoder unavailable (libx264 software fallback \
                     at high resolution), disk I/O contention, or CPU pressure from \
                     another app."
                );
            } else if prev % 300 == 299 {
                // prev=299 ⇒ this drop is the 300th; prev=599 ⇒ 600th; …
                log::warn!("recording pipeline: {} frames dropped total", prev + 1);
            }
        }
    }

    pub fn pop(&self) -> Option<VideoFrame> {
        self.queue.pop()
    }

    pub fn is_empty(&self) -> bool {
        self.queue.is_empty()
    }

    pub fn stats(&self) -> PipelineStats {
        self.stats.clone()
    }
}

const STALE_FIRST_WARN: Duration = Duration::from_secs(5);
const STALE_REPEAT_WARN: Duration = Duration::from_secs(30);

fn stale_warning_due(stale_for: Duration, warnings_emitted: u32) -> bool {
    stale_for >= STALE_FIRST_WARN + STALE_REPEAT_WARN * warnings_emitted
}

/// Spawn the capture + frame-pacer loop.
///
/// Why this is a frame pacer, not a "capture as fast as DXGI delivers" loop:
/// the encoder declares the input rate to FFmpeg as `-framerate {fps}`, so
/// every frame we push contributes 1/fps seconds of *video PTS*, regardless
/// of when it was captured in wall-clock time. DXGI Desktop Duplication
/// only delivers a new frame when the desktop actually changes — for a
/// static screen that's < 1 fps. If we'd push frames at DXGI's natural
/// rate, a 10-second recording with little motion would encode as a 1-2
/// second video, while the cursor track (timestamped from a wall-clock
/// `Instant`) still spans 10 s. The editor would then race through the
/// entire cursor performance in the compressed playback duration —
/// exactly the "everything happens in 5 seconds" symptom.
///
/// To lock playback time to wall-clock time, this loop:
/// 1. Holds a single `last_frame` cache (the most recent captured texture).
/// 2. Polls DXGI non-blocking (`AcquireNextFrame(0)`) every iteration and
///    drains any new frames into the cache, so we always emit the freshest
///    pixels available at the tick instant.
/// 3. Emits exactly `target_fps` frames per real-time second to the
///    pipeline using a deadline scheduler. When DXGI has no new frame, we
///    duplicate the cached one — the video shows a still during static
///    desktop, which is correct.
///
/// Result: wall-clock seconds == video PTS seconds == cursor track
/// seconds. Preview and rendered MP4 stay in lockstep with the cursor
/// track regardless of how often the desktop redraws.
pub fn spawn_capture_loop(
    mut source: Box<dyn CaptureSource>,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pause_flag: Arc<std::sync::atomic::AtomicBool>,
    pipeline: RecordingPipeline,
    clock: Instant,
    target_fps: u32,
    // Marked at the FIRST encoded frame: video t=0, which the cursor blocks on.
    video_start: crate::recording::TrackStart,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("recast-capture".into())
        .spawn(move || {
            let fps = target_fps.max(1) as u64;
            // Exact per-tick schedule: tick `k` (counting from the pacer's
            // current anchor) fires at `base + k/fps` seconds, computed in
            // integer nanoseconds so the 1/fps rounding never accumulates.
            // The previous `Duration::from_micros(1_000_000/fps)` truncated
            // (60 fps → 16666 µs instead of 16666.67), making the pacer run
            // ~0.004 % fast — negligible per second, but ~0.14 s of video-vs-
            // wall-clock drift per hour of recording, enough to desync the
            // cursor/audio on a long capture.
            let tick_at = |base: Instant, k: u64| -> Instant {
                base + Duration::from_nanos(k.saturating_mul(1_000_000_000) / fps)
            };

            // Wait for the very first frame so the encoder isn't fed an
            // empty pipeline at t=0. DXGI returns the current desktop
            // immediately on most systems; we still cap the wait to keep
            // a stop request responsive (poll the stop flag every 100 ms).
            //
            // Bound the total wait: a healthy source (DXGI / avfoundation)
            // delivers within a second or two, so a source that produces
            // NOTHING means capture is broken — most commonly a missing macOS
            // Screen Recording grant, where FFmpeg stays alive but emits zero
            // bytes. Without this cap the loop spins forever and `stop()`'s
            // `capture_handle.join()` hangs, so the Stop button looks dead and
            // the user mashes it. Surface an actionable error instead.
            const WARMUP_TIMEOUT: Duration = Duration::from_secs(10);
            let warmup_start = Instant::now();
            let mut last_frame: Arc<[u8]> = loop {
                if stop_flag.load(Ordering::Acquire) {
                    return Ok(());
                }
                if warmup_start.elapsed() >= WARMUP_TIMEOUT {
                    return Err(anyhow::anyhow!(
                        "no frames captured within {}s — the screen source produced \
                         no data. On macOS, grant Screen Recording in System \
                         Settings → Privacy & Security, then record again.",
                        WARMUP_TIMEOUT.as_secs()
                    ));
                }
                match source.capture_next(Duration::from_millis(100))? {
                    Some(bytes) => break Arc::<[u8]>::from(bytes),
                    None => continue,
                }
            };

            // One instant for both, or the cursor's zero and the video's differ.
            let at = Instant::now();
            let first_us = at.saturating_duration_since(clock).as_micros() as u64;
            video_start.mark_at(at);
            pipeline.push(VideoFrame {
                timestamp_us: first_us,
                width: source.width(),
                height: source.height(),
                data: last_frame.clone(),
            });
            // Anchor the exact schedule at the warmup frame. `emitted` counts
            // frames pushed since `pacer_base`; tick `emitted+1` is the next
            // deadline. Both reset on resume so a paused span is excluded
            // without being "caught up" as lag.
            let mut pacer_base = Instant::now();
            let mut emitted: u64 = 0;
            let mut was_paused = false;
            let mut last_fresh_at = Instant::now();
            let mut stale_warnings: u32 = 0;

            while !stop_flag.load(Ordering::Acquire) {
                // While paused, emit nothing — the encoder is frame-count
                // based, so a span with no frames pushed simply doesn't
                // exist in the output video.
                if pause_flag.load(Ordering::Acquire) {
                    was_paused = true;
                    thread::sleep(Duration::from_millis(20));
                    continue;
                }
                if was_paused {
                    // Resuming: restart the exact schedule from now so the
                    // paused span isn't treated as lag and "caught up" with a
                    // burst of frames.
                    pacer_base = Instant::now();
                    emitted = 0;
                    was_paused = false;
                }

                // Non-blocking drain: pull at most a few frames DXGI may
                // have queued between ticks so we emit the freshest pixels.
                // Capped at 4 because the XCap fallback ignores the
                // timeout and does a full synchronous capture every call,
                // returning Some unconditionally — without the cap the
                // loop would never exit on that path.
                const MAX_DRAIN: usize = 4;
                for _ in 0..MAX_DRAIN {
                    match source.capture_next(Duration::from_millis(0)) {
                        Ok(Some(bytes)) => {
                            last_frame = Arc::<[u8]>::from(bytes);
                            last_fresh_at = Instant::now();
                            stale_warnings = 0;
                        }
                        Ok(None) => break,
                        Err(e) => {
                            // Unrecoverable: recoverable losses self-heal inside the
                            // source and surface as Ok(None).
                            log::error!("screen capture source failed: {e}");
                            break;
                        }
                    }
                }

                // A source that keeps returning no frame emits the cached one
                // forever, which used to look like a working recording.
                let stale_for = last_fresh_at.elapsed();
                if stale_warning_due(stale_for, stale_warnings) {
                    stale_warnings += 1;
                    log::warn!(
                        "no fresh screen frame for {}s — the recording is repeating the last frame",
                        stale_for.as_secs()
                    );
                }

                let now = Instant::now();
                let next_tick = tick_at(pacer_base, emitted + 1);
                if now >= next_tick {
                    pipeline.push(VideoFrame {
                        timestamp_us: clock.elapsed().as_micros() as u64,
                        width: source.width(),
                        height: source.height(),
                        data: last_frame.clone(),
                    });
                    emitted += 1;
                    // If a system stall pushed us more than one period
                    // behind, keep emitting one frame per iteration (no
                    // sleep) until we catch up — the loop body is cheap
                    // (Arc clone + queue push) and FFmpeg will absorb the
                    // burst. This preserves video duration after a
                    // hitch instead of leaving a permanent gap.
                    continue;
                }

                // Sleep until the next tick, but cap at 2 ms so we keep
                // draining fresh DXGI frames between ticks rather than
                // emitting a stale cached frame at tick time.
                let until = (next_tick - now).min(Duration::from_micros(2_000));
                thread::sleep(until);
            }
            Ok(())
        })
        .map_err(Into::into)
}

/// A source under the test's control, so the pacer's contract can be checked
/// without a display.
#[cfg(test)]
struct ScriptedSource {
    width: u32,
    height: u32,
    /// Frames handed out before the source goes quiet; `None` never runs dry.
    remaining: Option<usize>,
}

#[cfg(test)]
impl CaptureSource for ScriptedSource {
    fn capture_next(&mut self, _timeout: Duration) -> Result<Option<Vec<u8>>> {
        if let Some(left) = self.remaining.as_mut() {
            if *left == 0 {
                return Ok(None);
            }
            *left -= 1;
        }
        Ok(Some(vec![0u8; (self.width * self.height * 4) as usize]))
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn height(&self) -> u32 {
        self.height
    }
}

#[cfg(test)]
mod pacer_tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    const FPS: u32 = 50;

    /// Runs the real loop for `run` and returns what reached the pipeline.
    fn run_loop(remaining: Option<usize>, run: Duration) -> (PipelineSnapshot, Option<u64>) {
        let source = Box::new(ScriptedSource {
            width: 8,
            height: 8,
            remaining,
        });
        let stop = Arc::new(AtomicBool::new(false));
        let pause = Arc::new(AtomicBool::new(false));
        let pipeline = RecordingPipeline::new(4096);
        let started = Instant::now();
        let video_start = crate::recording::TrackStart::new(started);
        let handle = spawn_capture_loop(
            source,
            stop.clone(),
            pause.clone(),
            pipeline.clone(),
            started,
            FPS,
            video_start.clone(),
        )
        .expect("the capture thread starts");
        thread::sleep(run);
        stop.store(true, Ordering::Release);
        handle.join().expect("the thread joins").expect("no error");
        (pipeline.stats().snapshot(), video_start.elapsed_us())
    }

    /// The wall-clock contract: one real second of recording is `fps` frames of
    /// video, whatever the source did. A pacer that just forwarded frames would
    /// emit hundreds here, and one that never ticked would emit one.
    #[test]
    fn a_source_faster_than_the_pace_is_still_emitted_at_the_pace() {
        let (stats, _) = run_loop(None, Duration::from_millis(400));
        let expected = FPS as u64 * 400 / 1000;
        assert!(
            (expected / 2..=expected * 2).contains(&stats.captured_frames),
            "expected about {expected} frames at {FPS}fps, got {}",
            stats.captured_frames
        );
        assert_eq!(stats.dropped_frames, 0, "the queue was big enough");
    }

    /// An idle desktop produces nothing after its first frame, and the recording
    /// still has to advance: the encoder is frame-count based, so a gap here
    /// would compress the video against the cursor track.
    #[test]
    fn a_source_that_goes_quiet_keeps_the_recording_advancing() {
        let (stats, _) = run_loop(Some(1), Duration::from_millis(300));
        let expected = FPS as u64 * 300 / 1000;
        assert!(
            stats.captured_frames >= expected / 2,
            "a still screen still owes {expected} frames, got {}",
            stats.captured_frames
        );
    }

    /// Video t=0 is the first frame the source produced, not recording start.
    /// The cursor thread blocks on this mark, so leaving it unset would stall
    /// the cursor track entirely rather than merely misplace it.
    #[test]
    fn the_first_frame_records_when_it_arrived() {
        let (stats, offset) = run_loop(None, Duration::from_millis(200));
        assert!(stats.captured_frames > 0);
        let offset = offset.expect("the first frame marks video t=0");
        assert!(
            offset < 200_000,
            "the warmup offset should be the first frame's instant, got {offset}us"
        );
    }

    /// A source that never yields a frame must leave video t=0 UNSET, not 0:
    /// the two used to be the same value, which is why the cursor could not
    /// tell "no frame yet" from "the frame was at zero".
    #[test]
    fn a_source_that_never_delivers_leaves_video_zero_unset() {
        let (_, offset) = run_loop(Some(0), Duration::from_millis(120));
        assert_eq!(offset, None);
    }
}

#[cfg(test)]
mod stale_warning_tests {
    use super::*;

    #[test]
    fn a_healthy_source_never_warns() {
        assert!(!stale_warning_due(Duration::from_millis(16), 0));
        assert!(!stale_warning_due(Duration::from_secs(4), 0));
    }

    #[test]
    fn the_first_warning_lands_at_the_threshold() {
        assert!(stale_warning_due(STALE_FIRST_WARN, 0));
    }

    #[test]
    fn repeats_are_spaced_not_per_tick() {
        assert!(!stale_warning_due(Duration::from_secs(20), 1));
        assert!(stale_warning_due(Duration::from_secs(35), 1));
        assert!(!stale_warning_due(Duration::from_secs(60), 2));
        assert!(stale_warning_due(Duration::from_secs(65), 2));
    }
}

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossbeam_queue::ArrayQueue;

use super::CaptureTarget;
use crate::capture::create_capture_source;

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
            self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
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
    target: CaptureTarget,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pause_flag: Arc<std::sync::atomic::AtomicBool>,
    pipeline: RecordingPipeline,
    clock: Instant,
    target_fps: u32,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("recast-capture".into())
        .spawn(move || {
            let mut source = create_capture_source(&target)?;
            let frame_period = Duration::from_micros(
                1_000_000_u64 / target_fps.max(1) as u64,
            );

            // Wait for the very first frame so the encoder isn't fed an
            // empty pipeline at t=0. DXGI returns the current desktop
            // immediately on most systems; we still cap the wait to keep
            // a stop request responsive (poll the stop flag every 100 ms).
            let mut last_frame: Arc<[u8]> = loop {
                if stop_flag.load(Ordering::Acquire) {
                    return Ok(());
                }
                match source.capture_next(Duration::from_millis(100))? {
                    Some(bytes) => break Arc::<[u8]>::from(bytes),
                    None => continue,
                }
            };

            // Emit the very first frame at t=0 so the video starts on the
            // same wall-clock instant the cursor capture started.
            pipeline.push(VideoFrame {
                timestamp_us: clock.elapsed().as_micros() as u64,
                width: source.width(),
                height: source.height(),
                data: last_frame.clone(),
            });
            let mut next_tick = Instant::now() + frame_period;
            let mut was_paused = false;

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
                    // Resuming: rebase the pacer so the paused span isn't
                    // treated as lag and "caught up" with a burst of frames.
                    next_tick = Instant::now() + frame_period;
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
                        Ok(Some(bytes)) => last_frame = Arc::<[u8]>::from(bytes),
                        // Transient DXGI errors (mode change, etc.) — keep
                        // emitting the cached frame so the timeline doesn't
                        // freeze. The duplication will recover on the next
                        // poll once the desktop is back to a normal state.
                        Ok(None) | Err(_) => break,
                    }
                }

                let now = Instant::now();
                if now >= next_tick {
                    pipeline.push(VideoFrame {
                        timestamp_us: clock.elapsed().as_micros() as u64,
                        width: source.width(),
                        height: source.height(),
                        data: last_frame.clone(),
                    });
                    next_tick += frame_period;
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

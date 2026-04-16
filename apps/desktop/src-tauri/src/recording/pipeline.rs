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

pub fn spawn_capture_loop(
    target: CaptureTarget,
    stop_flag: Arc<std::sync::atomic::AtomicBool>,
    pipeline: RecordingPipeline,
    clock: Instant,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("recast-capture".into())
        .spawn(move || {
            let mut source = create_capture_source(&target)?;
            while !stop_flag.load(Ordering::Acquire) {
                match source.capture_next(Duration::from_millis(16))? {
                    Some(bytes) => {
                        let timestamp_us = clock.elapsed().as_micros() as u64;
                        pipeline.push(VideoFrame {
                            timestamp_us,
                            width: source.width(),
                            height: source.height(),
                            data: Arc::<[u8]>::from(bytes),
                        });
                    }
                    None => thread::sleep(Duration::from_millis(1)),
                }
            }
            Ok(())
        })
        .map_err(Into::into)
}

use core::time::Duration;

use capturekit_core::{
    AudioDesc, AudioDeviceId, AudioDirection, AudioFormat, CaptureError, ColorSpace, CursorSample,
    DirtyRects, LostReason, PixelFormat, Rect, Result, Rotation, SourceDesc, Timestamp,
};

use crate::backend::{AudioSource, FrameSource, RawAudio, RawFrame};

/// One scripted answer from a mock audio device.
#[derive(Debug, Clone)]
pub enum MockAudio {
    /// Samples the device produced.
    Buffer {
        /// The timestamp to report, on the source's clock.
        pts_nanos: i64,
        /// Interleaved samples in the source's format.
        bytes: Vec<u8>,
        /// Whether the backend invented these to cover a gap.
        silence: bool,
        /// Whether the device reported a break before them.
        discontinuous: bool,
    },
    /// The device had nothing within the timeout. Not a failure: a quiet
    /// microphone answers this all day, and a recorder still owes wall time.
    Timeout,
    /// The device is gone. A recorder stops on this rather than retrying.
    Lost(LostReason),
}

impl MockAudio {
    /// `frames` interleaved samples of silence, in `format`.
    #[must_use]
    pub fn silence(pts_nanos: i64, format: AudioFormat, frames: usize) -> Self {
        Self::Buffer {
            pts_nanos,
            bytes: vec![0u8; frames * format.bytes_per_frame()],
            silence: true,
            discontinuous: false,
        }
    }
}

/// An audio device under the test's control.
/// The point is the RECORDER's loop: a timeout arm that fails to keep wall clock, or an error arm that spins instead of stopping, is invisible against a well-behaved real device.
pub struct MockAudioSource {
    desc: AudioDesc,
    script: std::collections::VecDeque<MockAudio>,
    /// Held so a returned buffer can borrow it, since `RawAudio` is a view.
    current: Vec<u8>,
    /// What a script that has run out answers, forever.
    exhausted: MockAudio,
    served: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    stopped: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl MockAudioSource {
    /// A device that answers `script` in order.
    #[must_use]
    pub fn new(format: AudioFormat, script: Vec<MockAudio>) -> Self {
        Self {
            desc: AudioDesc {
                format,
                device: AudioDeviceId("mock".to_string()),
                direction: AudioDirection::Input,
                backend: "mock",
            },
            script: script.into(),
            current: Vec::new(),
            // A quiet device by default: a loop reading past the script is real.
            exhausted: MockAudio::Timeout,
            served: std::sync::Arc::default(),
            stopped: std::sync::Arc::default(),
        }
    }

    /// What to answer once the script runs out. Default is [`MockAudio::Timeout`].
    #[must_use]
    pub fn then(mut self, answer: MockAudio) -> Self {
        self.exhausted = answer;
        self
    }

    /// A live count of the answers handed out.
    /// Shared, because the source is moved into whatever drives it: a loop that stopped reading and one that never started look identical from outside.
    #[must_use]
    pub fn reads(&self) -> std::sync::Arc<std::sync::atomic::AtomicUsize> {
        std::sync::Arc::clone(&self.served)
    }

    /// A live flag set when the capture is released.
    /// Shared for the same reason as [`Self::reads`]: the source is moved into whatever drives it, so it cannot be asked afterwards.
    #[must_use]
    pub fn released(&self) -> std::sync::Arc<std::sync::atomic::AtomicBool> {
        std::sync::Arc::clone(&self.stopped)
    }
}

impl AudioSource for MockAudioSource {
    fn describe(&self) -> &AudioDesc {
        &self.desc
    }

    fn next_buffer(&mut self, timeout: Duration) -> Result<RawAudio<'_>> {
        self.served
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let answer = self
            .script
            .pop_front()
            .unwrap_or_else(|| self.exhausted.clone());
        match answer {
            MockAudio::Buffer {
                pts_nanos,
                bytes,
                silence,
                discontinuous,
            } => {
                self.current = bytes;
                Ok(RawAudio {
                    pts: Timestamp::from_nanos(pts_nanos),
                    bytes: &self.current,
                    silence,
                    discontinuous,
                })
            }
            MockAudio::Timeout => {
                // Answering instantly turns a caller's poll loop into a busy spin.
                std::thread::sleep(timeout);
                Err(CaptureError::Timeout(timeout))
            }
            MockAudio::Lost(reason) => Err(CaptureError::Lost(reason)),
        }
    }

    fn stop(&mut self) -> Result<()> {
        self.stopped
            .store(true, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

/// A frame the mock will hand out, and how it should behave when it does.
#[derive(Debug, Clone)]
pub struct MockFrame {
    /// The timestamp to report. Values below the request time read as stale.
    pub pts: Timestamp,
    /// The value every byte of the frame carries, so a test can tell frames apart.
    pub fill: u8,
}

impl MockFrame {
    /// A frame at `pts` nanoseconds filled with `fill`.
    #[must_use]
    pub const fn new(pts_nanos: i64, fill: u8) -> Self {
        Self {
            pts: Timestamp::from_nanos(pts_nanos),
            fill,
        }
    }
}

/// A synthetic source, for downstream tests and for CI with no display.
/// Scripted rather than generated: a test says exactly which frames arrive, so stale-frame handling, loss and recovery are all reachable without an OS.
#[derive(Debug, Clone)]
pub struct MockSource {
    desc: SourceDesc,
    frames: Vec<MockFrame>,
    failures: Vec<Option<LostReason>>,
    region: Option<Rect>,
    dirty: DirtyRects,
    cursor: Option<CursorSample>,
    buffer: Vec<u8>,
    served: usize,
}

impl MockSource {
    /// A source of `width` by `height` BGRA frames that hands out `frames` in order.
    #[must_use]
    pub fn new(width: u32, height: u32, frames: Vec<MockFrame>) -> Self {
        Self {
            desc: SourceDesc {
                width,
                height,
                format: PixelFormat::Bgra8,
                color_space: ColorSpace::SRGB,
                rotation: Rotation::None,
                scale_factor: 1.0,
                frame_rate: Some(60),
                backend: "mock",
            },
            frames,
            failures: Vec::new(),
            region: None,
            dirty: DirtyRects::unknown(),
            cursor: None,
            buffer: Vec::new(),
            served: 0,
        }
    }

    /// Report `rect` as the only region that changed on every frame.
    #[must_use]
    pub fn reporting_dirty(mut self, rect: Rect) -> Self {
        self.dirty = DirtyRects::from_rects([rect]);
        self
    }

    /// Report a cursor at `position` on every frame.
    #[must_use]
    pub fn with_cursor(mut self, position: (i32, i32)) -> Self {
        self.cursor = Some(CursorSample {
            pts: Timestamp::ZERO,
            position: Some(position),
            visible: true,
            buttons: capturekit_core::CursorButtons::NONE,
            shape_id: 1,
        });
        self
    }

    /// Fail the nth acquisition with `reason` instead of delivering a frame.
    #[must_use]
    pub fn failing_at(mut self, index: usize, reason: LostReason) -> Self {
        if self.failures.len() <= index {
            self.failures.resize(index + 1, None);
        }
        self.failures[index] = Some(reason);
        self
    }

    /// Claim the backend cropped to `region` during acquisition.
    #[must_use]
    pub fn cropping_to(mut self, region: Rect) -> Self {
        self.desc.width = region.width;
        self.desc.height = region.height;
        self.region = Some(region);
        self
    }

    /// How many frames have been handed out.
    #[must_use]
    pub const fn served(&self) -> usize {
        self.served
    }
}

impl FrameSource for MockSource {
    fn describe(&self) -> &SourceDesc {
        &self.desc
    }

    fn region(&self) -> Option<Rect> {
        self.region
    }

    fn next_frame(&mut self, timeout: Duration) -> Result<RawFrame<'_>> {
        if let Some(Some(reason)) = self.failures.get(self.served).copied() {
            self.served += 1;
            return Err(CaptureError::Lost(reason));
        }
        let Some(frame) = self.frames.get(self.served).cloned() else {
            // A real backend blocks for the timeout before admitting it has nothing; answering instantly would pass a busy-wait.
            std::thread::sleep(timeout);
            return Err(CaptureError::Timeout(timeout));
        };
        self.served += 1;

        let stride = self.desc.width * 4;
        self.buffer.clear();
        self.buffer
            .resize((stride * self.desc.height) as usize, frame.fill);
        Ok(RawFrame {
            pts: frame.pts,
            bytes: &self.buffer,
            stride,
            dirty: self.dirty.clone(),
            cursor: self.cursor.map(|sample| CursorSample {
                pts: frame.pts,
                ..sample
            }),
            gpu: None,
        })
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

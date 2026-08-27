use core::time::Duration;

use capturekit_core::{
    CaptureError, ColorSpace, DirtyRects, LostReason, PixelFormat, Rect, Result, Rotation,
    SourceDesc, Timestamp,
};

use crate::backend::{FrameSource, RawFrame};

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
///
/// Scripted rather than generated: a test says exactly which frames arrive, so
/// stale-frame handling, loss and recovery are all reachable without an OS.
#[derive(Debug, Clone)]
pub struct MockSource {
    desc: SourceDesc,
    frames: Vec<MockFrame>,
    failures: Vec<Option<LostReason>>,
    region: Option<Rect>,
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
            buffer: Vec::new(),
            served: 0,
        }
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
        let frame = self
            .frames
            .get(self.served)
            .cloned()
            .ok_or(CaptureError::Timeout(timeout))?;
        self.served += 1;

        let stride = self.desc.width * 4;
        self.buffer.clear();
        self.buffer
            .resize((stride * self.desc.height) as usize, frame.fill);
        Ok(RawFrame {
            pts: frame.pts,
            bytes: &self.buffer,
            stride,
            dirty: DirtyRects::unknown(),
            cursor: None,
        })
    }

    fn stop(&mut self) -> Result<()> {
        Ok(())
    }
}

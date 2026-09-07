use std::path::Path;

use recast_codec_mf::{DecodeError, DecodedFrame, VideoReader};
use recast_compositor::{PlaneData, PlaneLayout, SourceColor, SourcePlanes};

use crate::frames::PictureSource;

/// 100 ns ticks in a second, the clock Media Foundation stamps with.
const TICKS_PER_SECOND: f64 = 10_000_000.0;

/// A recording on disk, decoded on demand. One frame plus a lookahead, so a
/// source slower than the output rate repeats the frame covering the instant.
pub struct VideoPictures {
    reader: VideoReader,
    /// The frame covering the last instant asked for.
    current: Option<DecodedFrame>,
    /// Decoded but not yet due. Needed because the reader cannot peek.
    ahead: Option<DecodedFrame>,
    color: SourceColor,
    ended: bool,
}

impl VideoPictures {
    /// Opens `path` for decoding, tagging every frame with `color`.
    pub fn open(path: &Path, color: SourceColor) -> Result<Self, DecodeError> {
        Ok(Self {
            reader: VideoReader::open(path)?,
            current: None,
            ahead: None,
            color,
            ended: false,
        })
    }

    #[must_use]
    pub fn width(&self) -> u32 {
        self.reader.info().width
    }

    #[must_use]
    pub fn height(&self) -> u32 {
        self.reader.info().height
    }

    /// Rewinds to the keyframe at or before `ticks` and drops what was held.
    fn rewind_to(&mut self, ticks: i64) -> Result<(), DecodeError> {
        self.reader.seek(ticks)?;
        self.current = None;
        self.ahead = None;
        self.ended = false;
        Ok(())
    }

    /// Whether the lookahead is due at `ticks`, decoding one if the slot is
    /// empty. `false` at end of stream, which is what stops the advance loop.
    fn ahead_is_due(&mut self, ticks: i64) -> Result<bool, DecodeError> {
        if self.ahead.is_none() && !self.ended {
            self.ahead = self.reader.next_frame()?;
            self.ended = self.ahead.is_none();
        }
        // The first frame is taken whatever its stamp: time zero needs a picture.
        Ok(match &self.ahead {
            Some(frame) => self.current.is_none() || frame.timestamp <= ticks,
            None => false,
        })
    }
}

impl PictureSource for VideoPictures {
    type Error = DecodeError;

    fn picture_at(&mut self, source_time: f64) -> Result<Option<SourcePlanes<'_>>, Self::Error> {
        let ticks = (source_time.max(0.0) * TICKS_PER_SECOND) as i64;
        let behind = self
            .current
            .as_ref()
            .is_some_and(|frame| ticks < frame.timestamp);
        if behind {
            self.rewind_to(ticks)?;
        }
        while self.ahead_is_due(ticks)? {
            self.current = self.ahead.take();
        }

        let info = self.reader.info();
        let color = self.color;
        Ok(self.current.as_ref().map(|frame| SourcePlanes {
            width: info.width,
            height: info.height,
            layout: PlaneLayout::Nv12,
            color,
            data: PlaneData::Packed(&frame.data),
        }))
    }
}

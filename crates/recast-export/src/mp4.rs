use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{enumerate_encoders, EncodeConfig, EncodeError, H264Encoder};
use recast_compositor::SourceColor;
use recast_mux::avc::annex_b_to_avcc;
use recast_mux::writer::{Mp4Writer, VideoFormat};

use crate::nv12::{rgba_to_nv12, Nv12Error};
use crate::walk::FrameWalk;

/// Why an export could not be written.
#[derive(Debug, thiserror::Error)]
pub enum Mp4Error {
    #[error("no H.264 encoder on this machine")]
    NoEncoder,
    #[error("frame is {width}x{height}, which does not fit an MP4 track header")]
    Oversized { width: u32, height: u32 },
    #[error("converting frame {index}: {error}")]
    Convert { index: u64, error: Nv12Error },
    #[error("encoding frame {index}: {error}")]
    Encode { index: u64, error: EncodeError },
    #[error("the encoder produced no samples")]
    Empty,
}

/// The best encoder that will actually take this configuration. Ranked, not
/// just preferred: a hardware one refuses sizes the software one accepts.
fn open_ranked(config: EncodeConfig) -> Result<H264Encoder, Mp4Error> {
    let candidates = enumerate_encoders();
    let mut last = None;
    for descriptor in ranked(&candidates, VideoCodec::H264) {
        match H264Encoder::open(descriptor, config) {
            Ok(encoder) => return Ok(encoder),
            Err(error) => last = Some(error),
        }
    }
    match last {
        Some(error) => Err(Mp4Error::Encode { index: 0, error }),
        None => Err(Mp4Error::NoEncoder),
    }
}

/// Encodes RGBA frames to H.264 and writes them into an MP4. Constant rate by
/// construction: timescale is the fps numerator, each sample one denominator.
pub struct Mp4Sink {
    encoder: H264Encoder,
    writer: Mp4Writer,
    walk: FrameWalk,
    color: SourceColor,
    nv12: Vec<u8>,
    width: u32,
    height: u32,
    written: u64,
    /// Last presentation stamp the encoder handed back, to catch reordering.
    last_pts: Option<i64>,
    reordered: bool,
}

impl Mp4Sink {
    /// Opens the machine's preferred H.264 encoder for a `width` by `height`
    /// export at the walk's rate.
    pub fn new(
        width: u32,
        height: u32,
        walk: FrameWalk,
        bitrate: u32,
        color: SourceColor,
    ) -> Result<Self, Mp4Error> {
        let (Ok(w), Ok(h)) = (u16::try_from(width), u16::try_from(height)) else {
            return Err(Mp4Error::Oversized { width, height });
        };
        let config = EncodeConfig {
            width,
            height,
            frame_rate: walk.fps(),
            bitrate,
            // Two seconds, so a scrub decodes from nearby rather than from the top.
            keyframe_interval: walk.fps().0.saturating_mul(2) / walk.fps().1.max(1),
        };
        let encoder = open_ranked(config)?;

        Ok(Self {
            encoder,
            writer: Mp4Writer::new(VideoFormat {
                width: w,
                height: h,
                timescale: walk.fps().0,
            }),
            walk,
            color,
            nv12: Vec::new(),
            width,
            height,
            written: 0,
            last_pts: None,
            reordered: false,
        })
    }

    /// Converts, encodes and buffers one frame. An encoder holds frames back to
    /// look ahead, so a call that writes no sample is normal.
    pub fn push(&mut self, index: u64, rgba: &[u8]) -> Result<(), Mp4Error> {
        self.nv12.clear();
        rgba_to_nv12(&mut self.nv12, rgba, self.width, self.height, &self.color)
            .map_err(|error| Mp4Error::Convert { index, error })?;

        let samples = self
            .encoder
            .encode(
                &self.nv12,
                self.walk.timestamp_100ns(index),
                self.walk.duration_100ns(),
            )
            .map_err(|error| Mp4Error::Encode { index, error })?;
        self.drain(samples);
        Ok(())
    }

    /// Flushes the encoder and returns the finished file.
    pub fn finish(mut self) -> Result<Vec<u8>, Mp4Error> {
        let tail = self.encoder.finish().map_err(|error| Mp4Error::Encode {
            index: self.written,
            error,
        })?;
        self.drain(tail);
        self.writer.finish().ok_or(Mp4Error::Empty)
    }

    /// Samples written so far. Lags the frames pushed while the encoder is
    /// holding frames for lookahead, and matches once `finish` has drained it.
    #[must_use]
    pub fn sample_count(&self) -> u64 {
        self.written
    }

    /// Whether the encoder handed back an out-of-order stamp. This writer emits
    /// no composition offsets, so a true here means the timing is untrustworthy.
    #[must_use]
    pub fn saw_reordering(&self) -> bool {
        self.reordered
    }

    fn drain(&mut self, samples: Vec<recast_codec_mf::EncodedSample>) {
        for sample in samples {
            if self.last_pts.is_some_and(|last| sample.timestamp < last) {
                self.reordered = true;
            }
            self.last_pts = Some(sample.timestamp);
            let converted = annex_b_to_avcc(&sample.data);
            self.writer.set_avc_config(converted.config);
            if converted.sample.is_empty() {
                continue;
            }
            self.writer
                .push_sample(&converted.sample, self.walk.fps().1, converted.is_sync);
            self.written += 1;
        }
    }
}

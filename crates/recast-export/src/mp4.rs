use recast_codec::{ranked, VideoCodec};
use recast_codec_mf::{
    enumerate_encoders, AacEncoder, AudioFormat as MfAudioFormat, EncodeConfig, EncodeError,
    H264Encoder,
};
use recast_compositor::SourceColor;
use recast_mux::avc::annex_b_to_avcc;
use recast_mux::writer::{AudioFormat, Mp4Writer, VideoFormat};

use crate::frames::Frame;
use crate::nv12::{Nv12Encoder, Nv12Error};
use crate::walk::FrameWalk;

/// Why an export could not be written.
#[derive(Debug, thiserror::Error)]
pub enum Mp4Error {
    #[error("no H.264 encoder on this machine")]
    NoEncoder,
    #[error("opening an encoder for {width}x{height}: {error}")]
    Open {
        width: u32,
        height: u32,
        error: EncodeError,
    },
    #[error("frame is {width}x{height}, which does not fit an MP4 track header")]
    Oversized { width: u32, height: u32 },
    #[error("converting frame {index}: {error}")]
    Convert { index: u64, error: Nv12Error },
    #[error("encoding frame {index}: {error}")]
    Encode { index: u64, error: EncodeError },
    #[error("the encoder produced no samples")]
    Empty,
    #[error("the encoder reordered samples, which this writer cannot time correctly")]
    Reordered,
    #[error("encoding audio: {0}")]
    Audio(EncodeError),
    #[error("the audio track was already written")]
    AudioTwice,
    #[error("writing the file: {0}")]
    Write(std::io::Error),
}

/// Samples an AAC-LC frame covers. The muxer wants audio durations in samples.
const AAC_FRAME_SAMPLES: u32 = 1024;

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
        Some(error) => Err(Mp4Error::Open {
            width: config.width,
            height: config.height,
            error,
        }),
        None => Err(Mp4Error::NoEncoder),
    }
}

/// Encodes RGBA frames to H.264 and writes them into an MP4. Constant rate by
/// construction: timescale is the fps numerator, each sample one denominator.
pub struct Mp4Sink {
    encoder: H264Encoder,
    writer: Mp4Writer,
    walk: FrameWalk,
    /// Built once: the matrix is constant for the whole export.
    encoder_matrix: Nv12Encoder,
    nv12: Vec<u8>,
    width: u32,
    height: u32,
    written: u64,
    /// Last presentation stamp the encoder handed back, to catch reordering.
    last_pts: Option<i64>,
    reordered: bool,
    audio_samples: u64,
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
            encoder_matrix: Nv12Encoder::new(&color),
            nv12: Vec::new(),
            width,
            height,
            written: 0,
            last_pts: None,
            reordered: false,
            audio_samples: 0,
        })
    }

    /// Converts, encodes and buffers one frame. An encoder holds frames back to
    /// look ahead, so a call that writes no sample is normal.
    pub fn push(&mut self, index: u64, frame: Frame<'_>) -> Result<(), Mp4Error> {
        // Already NV12 means the loop converted on the GPU, and converting again would be both wrong and slow.
        let planes = match frame {
            Frame::Nv12(bytes) => bytes,
            Frame::Rgba(rgba) => {
                self.nv12.clear();
                self.encoder_matrix
                    .convert(&mut self.nv12, rgba, self.width, self.height)
                    .map_err(|error| Mp4Error::Convert { index, error })?;
                &self.nv12
            }
        };

        let samples = self
            .encoder
            .encode(
                planes,
                self.walk.timestamp_100ns(index),
                self.walk.duration_100ns(),
            )
            .map_err(|error| Mp4Error::Encode { index, error })?;
        self.drain(samples);
        Ok(())
    }

    /// Encodes a stereo mix into the file's audio track. Call once, before
    /// `finish`. Chunked, so a long export needs no timeline-sized buffer.
    pub fn push_audio(
        &mut self,
        mixer: &mut recast_audio::Mixer,
        bitrate: u32,
    ) -> Result<(), Mp4Error> {
        if self.audio_samples > 0 {
            return Err(Mp4Error::AudioTwice);
        }
        let format = MfAudioFormat {
            sample_rate: recast_audio::MASTER_RATE,
            channels: recast_audio::MASTER_CHANNELS as u16,
        };
        let mut encoder = AacEncoder::open(format, bitrate).map_err(Mp4Error::Audio)?;

        // From the top: `render_into` continues where it left off, and the ducking envelope makes a mid-stream start silently wrong, not short.
        mixer.reset();
        // Integrated loudness is measured over the whole timeline, so normalisation cannot be chunked; rendered whole ONLY when it is asked for, since that is a timeline-sized buffer.
        let normalized = mixer.master().normalize.then(|| mixer.render_all());
        let chunk_frames = format.sample_rate as usize / 10;
        let mut chunk = vec![0.0f32; chunk_frames * recast_audio::MASTER_CHANNELS];
        let mut rendered: u64 = 0;
        let total = mixer.total_frames();
        while rendered < total {
            let frames = chunk_frames.min((total - rendered) as usize);
            let slice = &mut chunk[..frames * recast_audio::MASTER_CHANNELS];
            match normalized.as_ref() {
                Some(whole) => {
                    let at = rendered as usize * recast_audio::MASTER_CHANNELS;
                    slice.copy_from_slice(&whole[at..at + slice.len()]);
                }
                None => mixer.render_into(slice),
            }
            let timestamp = rendered as i64 * 10_000_000 / i64::from(format.sample_rate);
            let samples = encoder.encode(slice, timestamp).map_err(Mp4Error::Audio)?;
            self.drain_audio(samples);
            rendered += frames as u64;
        }
        let tail = encoder.finish().map_err(Mp4Error::Audio)?;
        self.drain_audio(tail);

        if self.audio_samples > 0 {
            self.writer.set_audio_format(AudioFormat {
                sample_rate: format.sample_rate,
                channels: format.channels,
                config: encoder.config().to_vec(),
            });
        }
        Ok(())
    }

    /// AAC frames written. Zero means the file is video only.
    #[must_use]
    pub fn audio_sample_count(&self) -> u64 {
        self.audio_samples
    }

    fn drain_audio(&mut self, samples: Vec<recast_codec_mf::EncodedSample>) {
        for sample in samples {
            if sample.data.is_empty() {
                continue;
            }
            self.writer
                .push_audio_sample(&sample.data, AAC_FRAME_SAMPLES);
            self.audio_samples += 1;
        }
    }

    /// Flushes the encoder and returns the finished file.
    pub fn finish(mut self) -> Result<Vec<u8>, Mp4Error> {
        self.drain_tail()?;
        self.refuse_reordered()?;
        self.writer.finish().ok_or(Mp4Error::Empty)
    }

    /// Flushes the encoder and streams the finished file into `out`.
    ///
    /// A whole export otherwise sits in memory twice at the moment it is
    /// written: once as the samples and once as the file built from them.
    pub fn finish_into<W: std::io::Write>(mut self, out: &mut W) -> Result<(), Mp4Error> {
        self.drain_tail()?;
        self.refuse_reordered()?;
        match self.writer.finish_into(out) {
            Ok(true) => Ok(()),
            Ok(false) => Err(Mp4Error::Empty),
            Err(error) => Err(Mp4Error::Write(error)),
        }
    }

    /// Hold sample bytes in `dir` rather than in memory. Call before the first
    /// frame; a long export is gigabytes of them.
    pub fn spill_to(&mut self, dir: &std::path::Path) -> Result<(), Mp4Error> {
        self.writer.spill_to(dir).map_err(Mp4Error::Write)
    }

    /// The encoder is asked for no B-pictures, but that request is best effort.
    /// One that reorders anyway would get a file whose frames play out of order,
    /// which is worse than an export that stops and says so.
    fn refuse_reordered(&self) -> Result<(), Mp4Error> {
        match self.reordered {
            true => Err(Mp4Error::Reordered),
            false => Ok(()),
        }
    }

    fn drain_tail(&mut self) -> Result<(), Mp4Error> {
        let tail = self.encoder.finish().map_err(|error| Mp4Error::Encode {
            index: self.written,
            error,
        })?;
        self.drain(tail);
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The encoder is only ASKED for no B-pictures, so the refusal has to stand
    /// on its own: an MFT that reorders anyway must stop the export.
    #[test]
    fn a_reordered_stream_is_refused_rather_than_written_with_wrong_times() {
        let walk = FrameWalk::new(0.1, (30, 1));
        let Ok(mut sink) = Mp4Sink::new(64, 64, walk, 100_000, SourceColor::default()) else {
            eprintln!("skipping: no H.264 encoder on this machine");
            return;
        };
        assert!(sink.refuse_reordered().is_ok(), "a fresh sink is in order");
        sink.reordered = true;
        match sink.finish() {
            Err(Mp4Error::Reordered) => {}
            // Several suites holding MFTs at once can refuse the drain, which is not what this asserts.
            Err(Mp4Error::Encode { .. }) => eprintln!("skipping: the encoder would not drain"),
            other => panic!("a reordered stream was not refused: {other:?}"),
        }
    }
}

use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::Result;

/// How the sample bytes are encoded. The device decides this, and guessing it
/// from the bit depth is wrong: WASAPI hands out 32-bit *integer* PCM as well as
/// 32-bit float, and mislabelling either one is heard as noise, not as a subtle
/// artefact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleFormat {
    Int,
    Float,
}

impl SampleFormat {
    /// WAV `wFormatTag`: 1 = PCM integer, 3 = IEEE float.
    fn tag(self) -> u16 {
        match self {
            Self::Int => 1,
            Self::Float => 3,
        }
    }
}

/// One track's byte layout, kept together so a writer can never be handed a
/// half-updated description of its own samples.
#[derive(Debug, Clone, Copy)]
pub struct WavFormat {
    pub sample_rate: u32,
    pub channels: u16,
    /// Bits per sample *as stored*, i.e. the container width. A 24-in-32 device
    /// is 32 here, with the unused low bits zero.
    pub bits_per_sample: u16,
    pub format: SampleFormat,
}

impl WavFormat {
    pub fn new(
        sample_rate: u32,
        channels: u16,
        bits_per_sample: u16,
        format: SampleFormat,
    ) -> Self {
        Self {
            sample_rate: sample_rate.max(1),
            channels: channels.max(1),
            bits_per_sample: bits_per_sample.max(8),
            format,
        }
    }

    /// 16-bit integer PCM, the format the silence fallback writes.
    pub fn pcm16(sample_rate: u32, channels: u16) -> Self {
        Self::new(sample_rate, channels, 16, SampleFormat::Int)
    }

    pub fn block_align(&self) -> u16 {
        self.channels * (self.bits_per_sample / 8)
    }

    fn byte_rate(&self) -> u32 {
        self.sample_rate * self.block_align() as u32
    }
}

/// RIFF caps every size field at 4 GiB. Past that the header can no longer
/// describe the file, so we stop appending rather than write a length that
/// wraps and makes the whole take unreadable.
const MAX_DATA_BYTES: u64 = u32::MAX as u64 - HEADER_BYTES as u64;
const HEADER_BYTES: usize = 44;

/// Writes a WAV file incrementally. Samples are appended with `write_samples`,
/// and the header is finalized by `finish` (or on drop).
pub struct WavWriter {
    file: File,
    format: WavFormat,
    data_bytes_written: u64,
    truncated: bool,
}

impl WavWriter {
    pub fn new(path: &Path, format: WavFormat) -> Result<Self> {
        let mut file = File::create(path)?;
        file.write_all(&build_wav_header(format, 0))?;
        Ok(Self {
            file,
            format,
            data_bytes_written: 0,
            truncated: false,
        })
    }

    /// Append raw interleaved sample bytes, which must already match
    /// [`WavFormat`]. Silently stops at the RIFF size ceiling.
    pub fn write_samples(&mut self, data: &[u8]) -> Result<()> {
        let room = MAX_DATA_BYTES.saturating_sub(self.data_bytes_written);
        if room == 0 {
            if !self.truncated {
                self.truncated = true;
                log::warn!("WAV reached the 4 GiB RIFF limit; dropping further audio");
            }
            return Ok(());
        }
        let take = data.len().min(room as usize);
        self.file.write_all(&data[..take])?;
        self.data_bytes_written += take as u64;
        Ok(())
    }

    /// Whole samples written so far, per channel.
    pub fn frames_written(&self) -> u64 {
        let align = self.format.block_align() as u64;
        if align == 0 {
            0
        } else {
            self.data_bytes_written / align
        }
    }

    pub fn finish(mut self) -> Result<()> {
        self.patch_header()
    }

    fn patch_header(&mut self) -> Result<()> {
        let header = build_wav_header(self.format, self.data_bytes_written as u32);
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&header)?;
        self.file.flush()?;
        Ok(())
    }
}

impl Drop for WavWriter {
    fn drop(&mut self) {
        let _ = self.patch_header();
    }
}

fn build_wav_header(format: WavFormat, data_len: u32) -> Vec<u8> {
    let mut header = Vec::with_capacity(HEADER_BYTES);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes());
    header.extend_from_slice(&format.format.tag().to_le_bytes());
    header.extend_from_slice(&format.channels.to_le_bytes());
    header.extend_from_slice(&format.sample_rate.to_le_bytes());
    header.extend_from_slice(&format.byte_rate().to_le_bytes());
    header.extend_from_slice(&format.block_align().to_le_bytes());
    header.extend_from_slice(&format.bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    header
}

/// Write a silence WAV. Used when no audio device is reachable, so downstream
/// muxing still has a track of the right length.
pub fn write_silence_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    duration_secs: f64,
) -> Result<()> {
    let format = WavFormat::pcm16(sample_rate, channels);
    let frames = (duration_secs.max(0.0) * format.sample_rate as f64).round() as u64;
    let mut remaining = frames.saturating_mul(format.block_align() as u64);

    let mut writer = WavWriter::new(path, format)?;
    let zeros = vec![0u8; 64 * 1024];
    while remaining > 0 {
        let take = remaining.min(zeros.len() as u64) as usize;
        writer.write_samples(&zeros[..take])?;
        remaining -= take as u64;
    }
    writer.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn header_of(format: WavFormat, data_len: u32) -> Vec<u8> {
        build_wav_header(format, data_len)
    }

    fn u16_at(bytes: &[u8], at: usize) -> u16 {
        u16::from_le_bytes([bytes[at], bytes[at + 1]])
    }

    fn u32_at(bytes: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
    }

    #[test]
    fn header_is_the_canonical_44_bytes() {
        assert_eq!(
            header_of(WavFormat::pcm16(48_000, 2), 0).len(),
            HEADER_BYTES
        );
    }

    #[test]
    fn integer_and_float_get_different_tags_at_the_same_depth() {
        let int32 = WavFormat::new(48_000, 2, 32, SampleFormat::Int);
        let float32 = WavFormat::new(48_000, 2, 32, SampleFormat::Float);
        assert_eq!(u16_at(&header_of(int32, 0), 20), 1);
        assert_eq!(u16_at(&header_of(float32, 0), 20), 3);
    }

    #[test]
    fn block_align_and_byte_rate_follow_the_container_width() {
        // 24-in-32 arrives as a 4-byte container, and describing it as 16-bit is
        // what plays a recording back fast and distorted.
        let f = WavFormat::new(48_000, 2, 32, SampleFormat::Int);
        assert_eq!(f.block_align(), 8);
        let h = header_of(f, 0);
        assert_eq!(u16_at(&h, 32), 8);
        assert_eq!(u32_at(&h, 28), 48_000 * 8);
        assert_eq!(u16_at(&h, 34), 32);
    }

    #[test]
    fn a_degenerate_format_is_clamped_rather_than_dividing_by_zero() {
        let f = WavFormat::new(0, 0, 0, SampleFormat::Int);
        assert_eq!(f.sample_rate, 1);
        assert_eq!(f.channels, 1);
        assert_eq!(f.bits_per_sample, 8);
        assert_eq!(f.block_align(), 1);
    }

    #[test]
    fn silence_wav_has_the_length_it_was_asked_for() {
        let dir = std::env::temp_dir().join(format!("recast-wav-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("silence.wav");
        write_silence_wav(&path, 48_000, 2, 0.5).unwrap();
        let bytes = std::fs::read(&path).unwrap();
        // 0.5s @ 48kHz stereo 16-bit = 24000 frames * 4 bytes.
        assert_eq!(bytes.len(), HEADER_BYTES + 96_000);
        assert_eq!(u32_at(&bytes, 40), 96_000);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn writer_reports_frames_not_bytes() {
        let dir = std::env::temp_dir().join(format!("recast-wav-f-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("frames.wav");
        let mut w =
            WavWriter::new(&path, WavFormat::new(48_000, 2, 32, SampleFormat::Float)).unwrap();
        w.write_samples(&[0u8; 80]).unwrap();
        assert_eq!(w.frames_written(), 10);
        w.finish().unwrap();
        let _ = std::fs::remove_file(&path);
    }
}

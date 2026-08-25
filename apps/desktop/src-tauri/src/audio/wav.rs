use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;
use std::time::Duration;

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

    /// Re-declare the capture rate before finalizing. See
    /// [`measured_sample_rate`] for why the declared rate can be wrong.
    pub fn set_sample_rate(&mut self, sample_rate: u32) {
        self.format.sample_rate = sample_rate.max(1);
    }

    /// The rate currently declared in the header.
    pub fn sample_rate(&self) -> u32 {
        self.format.sample_rate
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

/// Rate difference below which correction is pointless. 0.02% is ~45 ms over a
/// 4-minute take, well inside what a viewer can detect.
const MIN_DRIFT_RATIO: f64 = 0.0002;
/// Above this the measurement is not drift, it is a broken span (a stalled
/// thread, a device reset). Correcting by it would wreck an otherwise fine take.
const MAX_DRIFT_RATIO: f64 = 0.05;

/// The sample rate the device ACTUALLY delivered, when it differs enough from
/// the declared rate to matter.
///
/// A capture device runs on its own crystal. Writing `declared` into the header
/// while the device delivers at a slightly different rate makes the track drift
/// against the video for the whole recording — the picture stays locked to the
/// frame pacer, so the error accumulates instead of cancelling. Re-declaring the
/// measured rate makes the file play back over exactly the wall-clock span it
/// was captured in; the pitch shift is the drift ratio, which at these
/// magnitudes is far below audible.
///
/// `None` means keep the declared rate: too small to matter, or too large to
/// believe.
pub fn measured_sample_rate(frames: u64, span: Duration, declared: u32) -> Option<u32> {
    let secs = span.as_secs_f64();
    if frames == 0 || secs <= 0.0 || declared == 0 {
        return None;
    }
    let actual = frames as f64 / secs;
    let ratio = (actual - declared as f64).abs() / declared as f64;
    if !(MIN_DRIFT_RATIO..=MAX_DRIFT_RATIO).contains(&ratio) {
        return None;
    }
    Some(actual.round() as u32)
}

/// Sample bytes a WAV's header claims, or `None` when the file is unreadable
/// or is not a RIFF/WAVE at all.
///
/// Reads only the 44-byte header, so this is a stat-cost check rather than an
/// ffprobe spawn.
pub fn wav_data_bytes(path: &Path) -> Option<u64> {
    use std::io::Read;
    let mut file = File::open(path).ok()?;
    let mut header = [0u8; HEADER_BYTES];
    file.read_exact(&mut header).ok()?;
    if &header[0..4] != b"RIFF" || &header[8..12] != b"WAVE" || &header[36..40] != b"data" {
        return None;
    }
    Some(u32::from_le_bytes(header[40..44].try_into().ok()?) as u64)
}

/// Whether this track actually carries audio.
///
/// A capture that never received a packet still leaves a valid 44-byte
/// header-only WAV behind. It is not merely useless downstream: feeding a
/// zero-sample input into the export's `amix` alongside a `concat` speed warp
/// makes FFmpeg abort the whole filter graph with "Invalid data found when
/// processing input", so an empty track must never reach it.
pub fn wav_has_samples(path: &Path) -> bool {
    wav_data_bytes(path).is_some_and(|bytes| bytes > 0)
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

    fn temp_dir(tag: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("recast-wav-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn a_device_running_true_needs_no_correction() {
        // 48000 frames in exactly 1s: the declared rate is right.
        assert_eq!(
            measured_sample_rate(48_000, Duration::from_secs(1), 48_000),
            None
        );
    }

    #[test]
    fn a_slow_device_clock_is_measured_and_corrected() {
        // 0.2% slow: 225s of wall clock yielded 225s*47904 frames.
        let frames = (47_904.0f64 * 225.0).round() as u64;
        let rate = measured_sample_rate(frames, Duration::from_secs(225), 48_000);
        assert_eq!(rate, Some(47_904));
    }

    #[test]
    fn a_fast_device_clock_is_corrected_the_other_way() {
        let frames = (48_120.0f64 * 100.0).round() as u64;
        assert_eq!(
            measured_sample_rate(frames, Duration::from_secs(100), 48_000),
            Some(48_120)
        );
    }

    #[test]
    fn drift_too_small_to_hear_is_left_alone() {
        // 0.01% over 225s is ~22ms; re-declaring buys nothing.
        let frames = (48_005.0f64 * 225.0).round() as u64;
        assert_eq!(
            measured_sample_rate(frames, Duration::from_secs(225), 48_000),
            None
        );
    }

    #[test]
    fn a_corrected_track_plays_back_over_the_span_it_was_captured_in() {
        // The property that actually keeps audio locked to the picture: after
        // correction, frames / declared_rate == the wall-clock span. The frame
        // pacer holds the video to that same span exactly.
        for drift in [0.998_f64, 0.9995, 1.0005, 1.002] {
            let declared = 48_000u32;
            let span = Duration::from_secs(200);
            let frames = (declared as f64 * drift * span.as_secs_f64()).round() as u64;
            let rate = measured_sample_rate(frames, span, declared).unwrap_or(declared);
            let playback = frames as f64 / rate as f64;
            assert!(
                (playback - span.as_secs_f64()).abs() < 0.01,
                "drift {drift}: plays {playback:.3}s over a {:.3}s capture",
                span.as_secs_f64()
            );
        }
    }

    #[test]
    fn an_uncorrected_track_would_have_drifted_measurably() {
        // Same 0.2%-slow device WITHOUT correction: what the picture drifts against.
        let span = 200.0_f64;
        let frames = (48_000.0 * 0.998 * span).round() as u64;
        let uncorrected = frames as f64 / 48_000.0;
        assert!(
            span - uncorrected > 0.3,
            "expected >0.3s of drift to justify correcting, got {:.3}s",
            span - uncorrected
        );
    }

    #[test]
    fn an_implausible_span_is_refused_rather_than_applied() {
        // Half the expected frames is a stalled capture, not a drifting crystal.
        assert_eq!(
            measured_sample_rate(24_000, Duration::from_secs(1), 48_000),
            None
        );
        assert_eq!(
            measured_sample_rate(0, Duration::from_secs(10), 48_000),
            None
        );
        assert_eq!(measured_sample_rate(48_000, Duration::ZERO, 48_000), None);
    }

    #[test]
    fn a_re_declared_rate_reaches_the_header() {
        let path = temp_dir("rate").join("rate.wav");
        let mut w = WavWriter::new(&path, WavFormat::pcm16(48_000, 2)).unwrap();
        w.write_samples(&[0u8; 400]).unwrap();
        w.set_sample_rate(47_904);
        w.finish().unwrap();
        let bytes = std::fs::read(&path).unwrap();
        assert_eq!(u32_at(&bytes, 24), 47_904);
        // Byte rate has to follow, or the file describes itself inconsistently.
        assert_eq!(u32_at(&bytes, 28), 47_904 * 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_header_only_wav_reports_no_samples() {
        // Exactly what a capture that never received a packet leaves behind.
        let path = temp_dir("empty").join("empty.wav");
        let writer = WavWriter::new(&path, WavFormat::pcm16(48_000, 2)).unwrap();
        writer.finish().unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().len(), HEADER_BYTES as u64);
        assert_eq!(wav_data_bytes(&path), Some(0));
        assert!(!wav_has_samples(&path));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_wav_with_audio_reports_its_samples() {
        let path = temp_dir("full").join("full.wav");
        write_silence_wav(&path, 48_000, 2, 0.25).unwrap();
        assert_eq!(wav_data_bytes(&path), Some(48_000 / 4 * 4));
        assert!(wav_has_samples(&path));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn a_missing_or_non_wav_file_is_not_treated_as_audio() {
        let dir = temp_dir("bogus");
        assert!(!wav_has_samples(&dir.join("does-not-exist.wav")));
        let junk = dir.join("junk.wav");
        std::fs::write(&junk, b"not a wav file at all, just some bytes here ok").unwrap();
        assert_eq!(wav_data_bytes(&junk), None);
        assert!(!wav_has_samples(&junk));
        let _ = std::fs::remove_file(&junk);
    }

    #[test]
    fn a_file_shorter_than_a_header_is_rejected_rather_than_panicking() {
        let path = temp_dir("short").join("short.wav");
        std::fs::write(&path, b"RIFF").unwrap();
        assert_eq!(wav_data_bytes(&path), None);
        assert!(!wav_has_samples(&path));
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

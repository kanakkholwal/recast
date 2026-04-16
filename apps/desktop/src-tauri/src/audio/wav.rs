use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::Result;

/// Writes a WAV file incrementally. Samples are appended with `write_samples`,
/// and the header is finalized when `finish` is called (or on drop).
pub struct WavWriter {
    file: File,
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    data_bytes_written: u32,
}

impl WavWriter {
    /// Create a new WAV writer. Writes the header placeholder immediately.
    pub fn new(path: &Path, sample_rate: u32, channels: u16, bits_per_sample: u16) -> Result<Self> {
        let mut file = File::create(path)?;
        // Write placeholder header (44 bytes). Will be patched in finish().
        let header = build_wav_header(sample_rate, channels, bits_per_sample, 0);
        file.write_all(&header)?;
        Ok(Self {
            file,
            sample_rate,
            channels,
            bits_per_sample,
            data_bytes_written: 0,
        })
    }

    /// Append raw PCM sample bytes. The data must match the format
    /// (e.g. interleaved i16 LE for 16-bit, f32 LE for 32-bit float).
    pub fn write_samples(&mut self, data: &[u8]) -> Result<()> {
        self.file.write_all(data)?;
        self.data_bytes_written += data.len() as u32;
        Ok(())
    }

    /// Finalize the WAV file by patching the header with the correct data size.
    pub fn finish(mut self) -> Result<()> {
        self.patch_header()
    }

    fn patch_header(&mut self) -> Result<()> {
        let header = build_wav_header(
            self.sample_rate,
            self.channels,
            self.bits_per_sample,
            self.data_bytes_written,
        );
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

/// Build a 44-byte WAV header for PCM data.
fn build_wav_header(
    sample_rate: u32,
    channels: u16,
    bits_per_sample: u16,
    data_len: u32,
) -> Vec<u8> {
    let bytes_per_sample = bits_per_sample / 8;
    let block_align = channels * bytes_per_sample;
    let byte_rate = sample_rate * block_align as u32;
    // audio_format: 1 = PCM integer, 3 = IEEE float
    let audio_format: u16 = if bits_per_sample == 32 { 3 } else { 1 };

    let mut header = Vec::with_capacity(44);
    header.extend_from_slice(b"RIFF");
    header.extend_from_slice(&(36 + data_len).to_le_bytes());
    header.extend_from_slice(b"WAVE");
    header.extend_from_slice(b"fmt ");
    header.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
    header.extend_from_slice(&audio_format.to_le_bytes());
    header.extend_from_slice(&channels.to_le_bytes());
    header.extend_from_slice(&sample_rate.to_le_bytes());
    header.extend_from_slice(&byte_rate.to_le_bytes());
    header.extend_from_slice(&block_align.to_le_bytes());
    header.extend_from_slice(&bits_per_sample.to_le_bytes());
    header.extend_from_slice(b"data");
    header.extend_from_slice(&data_len.to_le_bytes());
    header
}

/// Write a silence WAV file. Used as a fallback when no audio device is available.
pub fn write_silence_wav(
    path: &Path,
    sample_rate: u32,
    channels: u16,
    duration_secs: f64,
) -> Result<()> {
    let bits_per_sample: u16 = 16;
    let bytes_per_sample = bits_per_sample / 8;
    let total_samples = (duration_secs * sample_rate as f64).round() as u32;
    let data_len = total_samples * channels as u32 * bytes_per_sample as u32;

    let mut writer = WavWriter::new(path, sample_rate, channels, bits_per_sample)?;
    // Write silence in chunks to avoid huge allocations
    let chunk_size = 64 * 1024;
    let mut remaining = data_len as usize;
    let zeros = vec![0u8; chunk_size];
    while remaining > 0 {
        let to_write = remaining.min(chunk_size);
        writer.write_samples(&zeros[..to_write])?;
        remaining -= to_write;
    }
    writer.finish()?;
    Ok(())
}

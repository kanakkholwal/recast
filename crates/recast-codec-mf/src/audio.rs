use std::path::Path;

use windows::core::HSTRING;
use windows::Win32::Media::MediaFoundation::*;

use crate::decoder::DecodeError;
use crate::windows_mf::ensure_started;

/// What the reader was asked to produce, which is what it produces: the source
/// reader converts, so these are a request rather than a report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AudioFormat {
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for AudioFormat {
    /// The export master. Everything is resampled to it, so a project mixing a
    /// 44.1 kHz music bed with 48 kHz capture has one rate to reason about.
    fn default() -> Self {
        Self {
            sample_rate: 48_000,
            channels: 2,
        }
    }
}

const AUDIO_STREAM: u32 = MF_SOURCE_READER_FIRST_AUDIO_STREAM.0 as u32;

/// Reads a file's audio as interleaved 32-bit float.
pub struct AudioReader {
    reader: IMFSourceReader,
    format: AudioFormat,
}

impl AudioReader {
    /// `None` from `open` means the file has no audio, which is normal for a
    /// screen recording with the microphone off.
    pub fn open(path: &Path, format: AudioFormat) -> Result<Option<Self>, DecodeError> {
        if !ensure_started() {
            return Err(DecodeError::Unsupported);
        }
        // SAFETY: the URL outlives the call.
        let reader =
            unsafe { MFCreateSourceReaderFromURL(&HSTRING::from(path.as_os_str()), None) }?;

        // SAFETY: stream selection and type negotiation on the reader we own.
        let negotiated = unsafe {
            reader.SetStreamSelection(MF_SOURCE_READER_ALL_STREAMS.0 as u32, false)?;
            if reader.SetStreamSelection(AUDIO_STREAM, true).is_err() {
                return Ok(None);
            }
            let output = MFCreateMediaType()?;
            output.SetGUID(&MF_MT_MAJOR_TYPE, &MFMediaType_Audio)?;
            output.SetGUID(&MF_MT_SUBTYPE, &MFAudioFormat_Float)?;
            output.SetUINT32(&MF_MT_AUDIO_SAMPLES_PER_SECOND, format.sample_rate)?;
            output.SetUINT32(&MF_MT_AUDIO_NUM_CHANNELS, format.channels as u32)?;
            reader.SetCurrentMediaType(AUDIO_STREAM, None, &output)
        };
        if negotiated.is_err() {
            return Ok(None);
        }
        Ok(Some(Self { reader, format }))
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// The next block of interleaved samples, or `None` at end of stream.
    pub fn next_block(&mut self) -> Result<Option<Vec<f32>>, DecodeError> {
        loop {
            let mut flags = 0u32;
            let mut sample = None;
            // SAFETY: the out-params live for the call.
            unsafe {
                self.reader.ReadSample(
                    AUDIO_STREAM,
                    0,
                    None,
                    Some(&mut flags),
                    None,
                    Some(&mut sample),
                )?;
            }
            if flags & MF_SOURCE_READERF_ENDOFSTREAM.0 as u32 != 0 {
                return Ok(None);
            }
            let Some(sample) = sample else { continue };
            return Ok(Some(float_samples(&sample)?));
        }
    }

    /// Everything, interleaved. Convenient for a music bed; a long capture
    /// should be pulled block by block instead.
    pub fn read_all(&mut self) -> Result<Vec<f32>, DecodeError> {
        let mut out = Vec::new();
        while let Some(block) = self.next_block()? {
            out.extend_from_slice(&block);
        }
        Ok(out)
    }
}

fn float_samples(sample: &IMFSample) -> Result<Vec<f32>, DecodeError> {
    // SAFETY: one contiguous buffer, read under a lock, reinterpreted as the float samples the media type asked for.
    unsafe {
        let buffer = sample.ConvertToContiguousBuffer()?;
        let mut start = std::ptr::null_mut();
        let mut length = 0u32;
        buffer.Lock(&mut start, None, Some(&mut length))?;
        let bytes = std::slice::from_raw_parts(start, length as usize);
        // Copied via `from_le_bytes`, not transmuted: the buffer has no alignment guarantee and a misaligned f32 read is UB.
        let samples = bytes
            .as_chunks::<4>()
            .0
            .iter()
            .map(|&c| f32::from_le_bytes(c))
            .collect();
        buffer.Unlock()?;
        Ok(samples)
    }
}

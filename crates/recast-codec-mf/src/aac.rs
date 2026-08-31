use windows::core::GUID;
use windows::Win32::Media::MediaFoundation::*;

use crate::audio::AudioFormat;
use crate::encoder::{EncodeError, EncodedSample};
use crate::windows_mf::{encoder_activates, ensure_started};

/// Bytes of `HEAACWAVEINFO` that sit in front of the `AudioSpecificConfig` in the encoder's user data: payload type, profile level, struct type and two reserved fields.
const HEAAC_HEADER: usize = 12;

/// Encodes interleaved PCM into raw AAC frames.
///
/// Raw, not ADTS: MP4 stores bare frames and describes them with the
/// `AudioSpecificConfig` in `esds`, so an ADTS header per frame would be
/// duplicated metadata the demuxer then has to strip.
pub struct AacEncoder {
    transform: IMFTransform,
    format: AudioFormat,
    /// `AudioSpecificConfig`, which the muxer needs for `esds`.
    config: Vec<u8>,
}

impl AacEncoder {
    /// `bitrate` is in bits per second. The Microsoft encoder only accepts a
    /// handful of rates and picks the nearest it supports.
    pub fn open(format: AudioFormat, bitrate: u32) -> Result<Self, EncodeError> {
        if !ensure_started() {
            return Err(EncodeError::NotFound);
        }
        let activate = encoder_activates(
            MFT_CATEGORY_AUDIO_ENCODER,
            MFMediaType_Audio,
            MFAudioFormat_AAC,
        )
        .into_iter()
        .next()
        .ok_or(EncodeError::NotFound)?;
        // SAFETY: the activate came from MFTEnumEx and is alive here.
        let transform: IMFTransform = unsafe { activate.ActivateObject() }?;

        // Output first, as with the video encoders.
        let output = audio_type(&[
            (MF_MT_MAJOR_TYPE, Value::Guid(MFMediaType_Audio)),
            (MF_MT_SUBTYPE, Value::Guid(MFAudioFormat_AAC)),
            (MF_MT_AUDIO_BITS_PER_SAMPLE, Value::U32(16)),
            (
                MF_MT_AUDIO_SAMPLES_PER_SECOND,
                Value::U32(format.sample_rate),
            ),
            (MF_MT_AUDIO_NUM_CHANNELS, Value::U32(format.channels as u32)),
            (MF_MT_AUDIO_AVG_BYTES_PER_SECOND, Value::U32(bitrate / 8)),
            // 0 is raw AAC; 1 would ask for ADTS.
            (MF_MT_AAC_PAYLOAD_TYPE, Value::U32(0)),
        ])?;
        // SAFETY: stream 0 is the only stream an encoder MFT exposes.
        unsafe { transform.SetOutputType(0, &output, 0) }?;

        let input = audio_type(&[
            (MF_MT_MAJOR_TYPE, Value::Guid(MFMediaType_Audio)),
            (MF_MT_SUBTYPE, Value::Guid(MFAudioFormat_PCM)),
            (MF_MT_AUDIO_BITS_PER_SAMPLE, Value::U32(16)),
            (
                MF_MT_AUDIO_SAMPLES_PER_SECOND,
                Value::U32(format.sample_rate),
            ),
            (MF_MT_AUDIO_NUM_CHANNELS, Value::U32(format.channels as u32)),
        ])?;
        // SAFETY: as above.
        unsafe { transform.SetInputType(0, &input, 0) }?;

        // SAFETY: reading back the type the encoder settled on, which is where the AudioSpecificConfig appears.
        let config = unsafe {
            let settled = transform.GetOutputCurrentType(0)?;
            let mut blob = std::ptr::null_mut();
            let mut length = 0u32;
            match settled.GetAllocatedBlob(&MF_MT_USER_DATA, &mut blob, &mut length) {
                Ok(()) => {
                    let bytes = std::slice::from_raw_parts(blob, length as usize).to_vec();
                    windows::Win32::System::Com::CoTaskMemFree(Some(
                        blob as *const std::ffi::c_void,
                    ));
                    bytes
                        .get(HEAAC_HEADER..)
                        .map(|c| c.to_vec())
                        .unwrap_or_default()
                }
                Err(_) => Vec::new(),
            }
        };

        // SAFETY: the documented start-up message pair.
        unsafe {
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_BEGIN_STREAMING, 0)?;
            transform.ProcessMessage(MFT_MESSAGE_NOTIFY_START_OF_STREAM, 0)?;
        }
        Ok(Self {
            transform,
            format,
            config,
        })
    }

    /// The `AudioSpecificConfig` for `esds`. Empty means the encoder did not report one, and the muxer should refuse rather than write a track no decoder can start.
    pub fn config(&self) -> &[u8] {
        &self.config
    }

    pub fn format(&self) -> AudioFormat {
        self.format
    }

    /// Feeds interleaved float samples. `timestamp` is in 100 ns units.
    pub fn encode(
        &mut self,
        samples: &[f32],
        timestamp: i64,
    ) -> Result<Vec<EncodedSample>, EncodeError> {
        if samples.is_empty() {
            return Ok(Vec::new());
        }
        let pcm = to_pcm16(samples);
        let frames = samples.len() / self.format.channels.max(1) as usize;
        let duration = frames as i64 * 10_000_000 / self.format.sample_rate.max(1) as i64;
        let sample = crate::encoder::memory_sample(&pcm, timestamp, duration)?;
        // SAFETY: the sample lives for the call.
        unsafe { self.transform.ProcessInput(0, &sample, 0) }?;
        self.drain()
    }

    pub fn finish(&mut self) -> Result<Vec<EncodedSample>, EncodeError> {
        // SAFETY: the documented shutdown message pair.
        unsafe {
            self.transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_OF_STREAM, 0)?;
            self.transform
                .ProcessMessage(MFT_MESSAGE_COMMAND_DRAIN, 0)?;
        }
        self.drain()
    }

    fn drain(&mut self) -> Result<Vec<EncodedSample>, EncodeError> {
        let mut out = Vec::new();
        while let Some(sample) = crate::encoder::pull_output(&self.transform)? {
            out.push(sample);
        }
        Ok(out)
    }
}

impl Drop for AacEncoder {
    fn drop(&mut self) {
        // SAFETY: a shutdown message on a live transform.
        unsafe {
            let _ = self
                .transform
                .ProcessMessage(MFT_MESSAGE_NOTIFY_END_STREAMING, 0);
        }
    }
}

/// Float to the signed 16-bit PCM the AAC encoder takes. Clamped, because a mix
/// can exceed unity and wrapping turns a loud passage into noise.
fn to_pcm16(samples: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let clamped = sample.clamp(-1.0, 1.0);
        let value = (clamped * i16::MAX as f32).round() as i16;
        out.extend_from_slice(&value.to_le_bytes());
    }
    out
}

enum Value {
    Guid(GUID),
    U32(u32),
}

fn audio_type(attributes: &[(GUID, Value)]) -> Result<IMFMediaType, windows::core::Error> {
    // SAFETY: setting attributes on a type we just created.
    unsafe {
        let media = MFCreateMediaType()?;
        for (key, value) in attributes {
            match value {
                Value::Guid(v) => media.SetGUID(key, v)?,
                Value::U32(v) => media.SetUINT32(key, *v)?,
            }
        }
        Ok(media)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn float_converts_to_signed_pcm() {
        let bytes = to_pcm16(&[0.0, 1.0, -1.0]);
        assert_eq!(&bytes[0..2], &0i16.to_le_bytes());
        assert_eq!(&bytes[2..4], &i16::MAX.to_le_bytes());
        assert_eq!(&bytes[4..6], &(-i16::MAX).to_le_bytes());
    }

    /// A mix can exceed unity; wrapping would turn a loud passage into noise.
    #[test]
    fn samples_past_full_scale_clamp_rather_than_wrap() {
        let bytes = to_pcm16(&[4.0, -4.0]);
        assert_eq!(&bytes[0..2], &i16::MAX.to_le_bytes());
        assert_eq!(&bytes[2..4], &(-i16::MAX).to_le_bytes());
    }
}

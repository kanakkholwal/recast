use core::time::Duration;

use crate::error::CaptureError;

/// How one audio sample is stored. Samples are always interleaved by channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
#[non_exhaustive]
pub enum SampleFormat {
    /// 16-bit signed integer.
    I16,
    /// 32-bit signed integer.
    I32,
    /// 32-bit float, which every OS mixer here works in natively.
    F32,
}

impl SampleFormat {
    /// Bytes one sample of one channel occupies.
    #[must_use]
    pub const fn bytes(self) -> usize {
        match self {
            Self::I16 => 2,
            Self::I32 | Self::F32 => 4,
        }
    }
}

/// What a device delivers: rate, channel count and sample storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct AudioFormat {
    /// Sample frames per second.
    pub sample_rate: u32,
    /// Interleaved channel count.
    pub channels: u16,
    /// How each sample is stored.
    pub sample_format: SampleFormat,
}

impl AudioFormat {
    /// 48 kHz stereo float, what every OS mixer here runs at internally.
    pub const STEREO_48K: Self = Self {
        sample_rate: 48_000,
        channels: 2,
        sample_format: SampleFormat::F32,
    };

    /// Build a format.
    #[must_use]
    pub const fn new(sample_rate: u32, channels: u16, sample_format: SampleFormat) -> Self {
        Self {
            sample_rate,
            channels,
            sample_format,
        }
    }

    /// Bytes one sample frame occupies, across every channel.
    ///
    /// A *sample frame* is one sample per channel, the unit both the OS APIs and
    /// the timestamps count in. It is unrelated to a video frame.
    #[must_use]
    pub const fn bytes_per_frame(self) -> usize {
        self.channels as usize * self.sample_format.bytes()
    }

    /// Whole sample frames that fit in `bytes`, rounding down.
    #[must_use]
    pub const fn frames_in(self, bytes: usize) -> usize {
        match self.bytes_per_frame() {
            0 => 0,
            per_frame => bytes / per_frame,
        }
    }

    /// Bytes `frames` sample frames occupy.
    #[must_use]
    pub const fn bytes_for(self, frames: usize) -> usize {
        frames * self.bytes_per_frame()
    }

    /// How long `frames` sample frames last.
    #[must_use]
    pub fn duration_of(self, frames: u64) -> Duration {
        match self.sample_rate {
            0 => Duration::ZERO,
            rate => Duration::from_nanos(frames.saturating_mul(1_000_000_000) / u64::from(rate)),
        }
    }

    /// How many sample frames fill `duration`.
    #[must_use]
    pub fn frames_in_duration(self, duration: Duration) -> u64 {
        (duration.as_nanos() as u64).saturating_mul(u64::from(self.sample_rate)) / 1_000_000_000
    }

    /// Check a buffer holds whole sample frames of this format.
    ///
    /// A partial frame means the reader and the device disagree about the channel
    /// count, which shows up as a stutter or swapped channels rather than an
    /// error, so it is worth refusing at the boundary.
    pub fn validate_buffer(self, len: usize) -> Result<(), CaptureError> {
        let per_frame = self.bytes_per_frame();
        if per_frame == 0 {
            return Err(CaptureError::Unsupported {
                backend: "capturekit",
                operation: "read audio with no channels",
            });
        }
        if len % per_frame != 0 {
            return Err(CaptureError::PartialAudioFrame {
                format: self,
                len,
                bytes_per_frame: per_frame,
            });
        }
        Ok(())
    }
}

/// Whether a device is captured as an input or as the system's own output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub enum AudioDirection {
    /// A microphone or line input.
    Input,
    /// What the system is playing, captured back from an output device.
    Loopback,
}

/// A device's stable identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct AudioDeviceId(pub String);

/// An audio device available to capture.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct AudioDevice {
    /// Stable identifier to open the device with.
    pub id: AudioDeviceId,
    /// Human-readable device name.
    pub name: String,
    /// Whether it is captured as an input or as system output.
    pub direction: AudioDirection,
    /// Whether the OS reports this as the default for its direction.
    pub is_default: bool,
    /// The format the device is currently configured for.
    pub format: AudioFormat,
}

/// What an audio backend actually negotiated.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct AudioDesc {
    /// The delivered format, which is not always the one requested.
    pub format: AudioFormat,
    /// Which device is being read.
    pub device: AudioDeviceId,
    /// Whether it is an input or a loopback.
    pub direction: AudioDirection,
    /// Name of the backend serving it, for logs and bug reports.
    pub backend: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stereo_float_frame_is_eight_bytes() {
        assert_eq!(AudioFormat::STEREO_48K.bytes_per_frame(), 8);
    }

    #[test]
    fn sixteen_bit_mono_is_two_bytes_a_frame() {
        let format = AudioFormat::new(44_100, 1, SampleFormat::I16);
        assert_eq!(format.bytes_per_frame(), 2);
        assert_eq!(
            format.frames_in(9),
            4,
            "the odd trailing byte is not a frame"
        );
    }

    #[test]
    fn one_second_at_48k_is_48000_frames() {
        let format = AudioFormat::STEREO_48K;
        assert_eq!(format.frames_in_duration(Duration::from_secs(1)), 48_000);
        assert_eq!(format.duration_of(48_000), Duration::from_secs(1));
    }

    #[test]
    fn duration_and_frame_count_round_trip_at_an_awkward_rate() {
        // 44 100 does not divide a second evenly, which is where a naive
        // conversion loses samples.
        let format = AudioFormat::new(44_100, 2, SampleFormat::I16);
        let frames = format.frames_in_duration(Duration::from_millis(1_000));
        assert_eq!(frames, 44_100);
        assert_eq!(format.duration_of(frames), Duration::from_secs(1));
    }

    #[test]
    fn a_zero_rate_device_reports_no_duration_rather_than_dividing_by_zero() {
        let format = AudioFormat::new(0, 2, SampleFormat::F32);
        assert_eq!(format.duration_of(1_000), Duration::ZERO);
        assert_eq!(format.frames_in_duration(Duration::from_secs(1)), 0);
    }

    #[test]
    fn a_buffer_of_whole_frames_is_accepted() {
        assert!(AudioFormat::STEREO_48K.validate_buffer(8 * 480).is_ok());
        assert!(AudioFormat::STEREO_48K.validate_buffer(0).is_ok());
    }

    #[test]
    fn a_partial_frame_is_refused_rather_than_swapping_the_channels() {
        let err = AudioFormat::STEREO_48K
            .validate_buffer(8 * 480 + 3)
            .expect_err("three trailing bytes are not a stereo frame");
        assert!(matches!(err, CaptureError::PartialAudioFrame { .. }));
    }

    #[test]
    fn a_device_with_no_channels_is_refused() {
        let format = AudioFormat::new(48_000, 0, SampleFormat::F32);
        assert!(format.validate_buffer(16).is_err());
        assert_eq!(format.frames_in(16), 0);
    }

    #[test]
    fn bytes_and_frames_are_inverses() {
        let format = AudioFormat::new(48_000, 6, SampleFormat::I32);
        assert_eq!(format.frames_in(format.bytes_for(1_024)), 1_024);
    }
}

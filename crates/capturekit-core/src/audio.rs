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

/// Tracks where a capture is on its own sample timeline, and how far a device
/// has run ahead of it.
///
/// Every OS capture API here reports a device position that keeps advancing in
/// real time, while buffers arrive only when there is something to deliver. A
/// loopback device with nothing playing delivers NOTHING at all, sometimes for
/// minutes. A consumer that just concatenates what arrives produces a track
/// shorter than the recording, and every sound in it drifts earlier than the
/// picture. The gap has to be filled with real silence, at the right place, and
/// only the device position says where that is.
#[derive(Debug, Clone)]
pub struct AudioTimeline {
    format: AudioFormat,
    next_frame: u64,
}

impl AudioTimeline {
    /// A timeline starting at sample frame zero.
    #[must_use]
    pub const fn new(format: AudioFormat) -> Self {
        Self {
            format,
            next_frame: 0,
        }
    }

    /// The format this timeline counts in.
    #[must_use]
    pub const fn format(&self) -> AudioFormat {
        self.format
    }

    /// The next sample frame this timeline expects to receive.
    #[must_use]
    pub const fn position(&self) -> u64 {
        self.next_frame
    }

    /// Silent sample frames needed before a buffer that starts at
    /// `device_position`.
    ///
    /// Zero when the device is where the timeline expects, or behind it: a
    /// device position that goes backwards is a driver resetting its counter,
    /// and inserting silence for it would push everything after it late.
    #[must_use]
    pub const fn gap_before(&self, device_position: u64) -> u64 {
        device_position.saturating_sub(self.next_frame)
    }

    /// Bytes of silence for a gap of `frames`.
    #[must_use]
    pub fn silence_bytes(&self, frames: u64) -> usize {
        self.format.bytes_for(frames as usize)
    }

    /// Record that `frames` sample frames were delivered.
    pub fn advance(&mut self, frames: u64) {
        self.next_frame = self.next_frame.saturating_add(frames);
    }

    /// Jump to a device position, for a driver that reset its counter.
    pub fn resync(&mut self, device_position: u64) {
        self.next_frame = device_position;
    }

    /// How long the timeline covers so far.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.format.duration_of(self.next_frame)
    }
}

/// Interleave one plane per channel into sample frames.
///
/// CoreAudio and PipeWire both deliver planar audio, one buffer per channel,
/// while capturekit's contract is interleaved. Done here rather than in each
/// backend because a stride mistake produces swapped or stuttering channels,
/// which sounds like a bad device rather than like a bug.
///
/// `out` is cleared and reused, so a backend calling this per buffer allocates
/// once. Returns the sample frames written.
pub fn interleave(
    planes: &[&[u8]],
    format: AudioFormat,
    out: &mut Vec<u8>,
) -> Result<usize, CaptureError> {
    out.clear();
    let sample = format.sample_format.bytes();
    if planes.len() != format.channels as usize || sample == 0 {
        return Err(CaptureError::Unsupported {
            backend: "capturekit",
            operation: "interleave a channel count the format does not describe",
        });
    }
    let Some(first) = planes.first() else {
        return Ok(0);
    };
    for (channel, plane) in planes.iter().enumerate().skip(1) {
        if plane.len() != first.len() {
            return Err(CaptureError::RaggedAudioPlanes {
                channel,
                len: plane.len(),
                expected: first.len(),
            });
        }
    }
    let frames = first.len() / sample;
    out.reserve(frames * planes.len() * sample);
    for frame in 0..frames {
        let at = frame * sample;
        for plane in planes {
            out.extend_from_slice(&plane[at..at + sample]);
        }
    }
    Ok(frames)
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEREO_I16: AudioFormat = AudioFormat::new(48_000, 2, SampleFormat::I16);

    #[test]
    fn two_planes_interleave_into_sample_frames() {
        let left = [0x01, 0x02, 0x03, 0x04];
        let right = [0x11, 0x12, 0x13, 0x14];
        let mut out = Vec::new();
        let frames = interleave(&[&left, &right], STEREO_I16, &mut out).expect("two planes");
        assert_eq!(frames, 2);
        assert_eq!(out, vec![0x01, 0x02, 0x11, 0x12, 0x03, 0x04, 0x13, 0x14]);
    }

    /// The mistake this exists to prevent: taking a whole plane at a time rather
    /// than one sample, which plays the left channel then the right.
    #[test]
    fn a_sample_of_each_channel_alternates_rather_than_whole_planes() {
        let left = [1u8, 1, 2, 2, 3, 3];
        let right = [9u8, 9, 8, 8, 7, 7];
        let mut out = Vec::new();
        interleave(&[&left, &right], STEREO_I16, &mut out).expect("two planes");
        assert_eq!(out, vec![1, 1, 9, 9, 2, 2, 8, 8, 3, 3, 7, 7]);
    }

    #[test]
    fn a_mono_plane_comes_through_unchanged() {
        let mono = [7u8, 7, 8, 8];
        let mut out = Vec::new();
        let frames = interleave(
            &[&mono],
            AudioFormat::new(48_000, 1, SampleFormat::I16),
            &mut out,
        )
        .expect("one plane");
        assert_eq!(frames, 2);
        assert_eq!(out, mono);
    }

    /// A plane shorter than the first means the buffer list was misread. Taking
    /// the shorter length would quietly drop the tail of one channel.
    #[test]
    fn planes_that_do_not_agree_are_refused_rather_than_truncated() {
        let left = [1u8, 1, 2, 2];
        let right = [9u8, 9];
        let mut out = Vec::new();
        let err =
            interleave(&[&left, &right], STEREO_I16, &mut out).expect_err("the planes disagree");
        assert!(
            matches!(err, CaptureError::RaggedAudioPlanes { channel: 1, .. }),
            "{err}"
        );
    }

    #[test]
    fn a_plane_count_the_format_does_not_describe_is_refused() {
        let only = [1u8, 1];
        let mut out = Vec::new();
        assert!(interleave(&[&only], STEREO_I16, &mut out).is_err());
    }

    #[test]
    fn interleaving_reuses_the_buffer_it_is_given() {
        let left = [1u8, 1];
        let right = [2u8, 2];
        let mut out = vec![0xFF; 64];
        interleave(&[&left, &right], STEREO_I16, &mut out).expect("two planes");
        assert_eq!(out, vec![1, 1, 2, 2], "the previous contents were kept");
    }

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
        // 44 100 doesn't divide a second evenly, which is where a naive conversion loses samples.
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
    fn a_timeline_that_keeps_up_reports_no_gap() {
        let mut timeline = AudioTimeline::new(AudioFormat::STEREO_48K);
        timeline.advance(480);
        assert_eq!(timeline.gap_before(480), 0);
        assert_eq!(timeline.position(), 480);
    }

    /// The idle-loopback case: nothing played for a second, so the device ran on
    /// while no buffers arrived. Without the silence the track ends up a second
    /// short and everything after it drifts early.
    #[test]
    fn an_idle_device_leaves_a_gap_measured_in_frames() {
        let mut timeline = AudioTimeline::new(AudioFormat::STEREO_48K);
        timeline.advance(480);
        let gap = timeline.gap_before(48_480);
        assert_eq!(gap, 48_000, "one second of silence is owed");
        assert_eq!(timeline.silence_bytes(gap), 48_000 * 8);
    }

    #[test]
    fn a_device_position_that_goes_backwards_owes_no_silence() {
        let mut timeline = AudioTimeline::new(AudioFormat::STEREO_48K);
        timeline.advance(96_000);
        assert_eq!(
            timeline.gap_before(48_000),
            0,
            "a reset counter must not push the timeline later"
        );
    }

    #[test]
    fn resync_moves_the_timeline_to_the_device() {
        let mut timeline = AudioTimeline::new(AudioFormat::STEREO_48K);
        timeline.advance(1_000);
        timeline.resync(50_000);
        assert_eq!(timeline.position(), 50_000);
        assert_eq!(timeline.gap_before(50_000), 0);
    }

    #[test]
    fn elapsed_follows_the_frames_delivered() {
        let mut timeline = AudioTimeline::new(AudioFormat::STEREO_48K);
        timeline.advance(24_000);
        assert_eq!(timeline.elapsed(), Duration::from_millis(500));
    }

    #[test]
    fn bytes_and_frames_are_inverses() {
        let format = AudioFormat::new(48_000, 6, SampleFormat::I32);
        assert_eq!(format.frames_in(format.bytes_for(1_024)), 1_024);
    }
}

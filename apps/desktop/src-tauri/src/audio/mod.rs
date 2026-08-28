mod session;
mod track;
pub mod wav;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Result;

use crate::audio::session::{Source, TrackSession};
use crate::recording::clock::TrackStart;

/// Configuration for system/loopback audio capture.
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    /// Path to write the WAV output file.
    pub output_path: PathBuf,
    /// When set, capture continues draining the device but stops writing
    /// samples — keeps the WAV gap-free across recording pauses.
    pub pause_flag: Arc<AtomicBool>,
    /// Marked when the first PCM sample is written, so the muxer can align this
    /// track against the video's own start instant.
    pub start: TrackStart,
}

/// Handle to a running system audio capture session.
pub struct AudioCaptureSession(TrackSession);

impl AudioCaptureSession {
    /// Start capturing, or `None` when no loopback device is reachable.
    ///
    /// No platform here is guaranteed one: macOS needs the screen recording
    /// grant, and a Linux host without PipeWire has nothing to read. The caller
    /// writes the silence a `None` needs, because it owns the pause-aware clock
    /// and a silence track measured on wall clock outruns the video by every
    /// paused second.
    pub fn start(config: AudioCaptureConfig) -> Option<Self> {
        match TrackSession::start(
            Source::Loopback,
            config.output_path,
            config.pause_flag,
            config.start,
        ) {
            Ok(session) => Some(Self(session)),
            Err(err) => {
                log::warn!("system-audio loopback unavailable: {err:#}");
                None
            }
        }
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.0.stop()
    }
}

/// Configuration for microphone capture.
#[derive(Debug, Clone)]
pub struct MicrophoneCaptureConfig {
    /// Path to write the WAV output file.
    pub output_path: PathBuf,
    /// Specific device ID to capture from (None = system default microphone).
    pub device_id: Option<String>,
    /// When set, capture continues draining the device but stops writing
    /// samples — keeps the WAV gap-free across recording pauses.
    pub pause_flag: Arc<AtomicBool>,
    /// Marked when the first PCM sample is written; see [`AudioCaptureConfig::start`].
    pub start: TrackStart,
}

/// Handle to a running microphone capture session.
///
/// Unlike the loopback there is no silence fallback: a user who asked for the
/// microphone and has no working one is told, rather than handed a mute track.
pub struct MicrophoneCaptureSession(TrackSession);

impl MicrophoneCaptureSession {
    pub fn start(config: MicrophoneCaptureConfig) -> Result<Self> {
        TrackSession::start(
            Source::Input(config.device_id),
            config.output_path,
            config.pause_flag,
            config.start,
        )
        .map(Self)
    }

    /// Non-fatal quality ceiling on the device that actually opened, if any.
    ///
    /// Read from the negotiated format rather than from a device listing: the
    /// backend picks the endpoint, and enumerating to ask activates every other
    /// one on the machine while the user waits for the recording to start.
    pub fn quality_warning(&self) -> Option<String> {
        let format = self.0.format();
        describe_microphone_quality(format.sample_rate, format.channels)
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.0.stop()
    }
}

/// Where the remedy for a low-rate endpoint lives.
#[cfg(windows)]
const LOW_RATE_REMEDY: &str =
    "Windows has it in communications mode, which caps recording quality. Change it under Sound settings → Recording → Properties → Advanced.";
#[cfg(not(windows))]
const LOW_RATE_REMEDY: &str =
    "That caps recording quality. Pick another input, or raise its rate in your system's sound settings.";

/// Communications-mode endpoints report a 16 kHz mono format, which is a hard
/// ceiling on the take no amount of processing recovers from.
fn describe_microphone_quality(sample_rate: u32, channels: u16) -> Option<String> {
    if sample_rate >= 32_000 {
        return None;
    }
    Some(format!(
        "Your microphone is running at {} kHz{} — {LOW_RATE_REMEDY}",
        sample_rate / 1000,
        if channels <= 1 { " mono" } else { "" }
    ))
}

#[cfg(test)]
mod tests {
    use super::describe_microphone_quality;

    #[test]
    fn a_communications_mode_endpoint_is_reported() {
        let warning = describe_microphone_quality(16_000, 1).expect("16 kHz mono should warn");
        assert!(warning.contains("16 kHz mono"), "{warning}");
    }

    #[test]
    fn a_normal_endpoint_is_silent() {
        assert!(describe_microphone_quality(48_000, 2).is_none());
        assert!(describe_microphone_quality(44_100, 1).is_none());
        assert!(describe_microphone_quality(32_000, 1).is_none());
    }

    #[test]
    fn stereo_low_rate_omits_the_mono_note() {
        let warning = describe_microphone_quality(22_050, 2).expect("22 kHz should warn");
        assert!(!warning.contains("mono"), "{warning}");
    }
}

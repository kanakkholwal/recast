use std::path::PathBuf;
use std::time::Instant;

use anyhow::Result;

use crate::audio::wav::write_silence_wav;
use crate::audio::{AudioCaptureConfig, MicrophoneCaptureConfig};

/// Fallback audio session for non-Windows platforms.
/// Writes a silence WAV file on stop.
pub struct PlatformAudioSession {
    config: AudioCaptureConfig,
    started_at: Instant,
}

impl PlatformAudioSession {
    pub fn start(config: AudioCaptureConfig) -> Result<Self> {
        Ok(Self {
            config,
            started_at: Instant::now(),
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        let duration = self.started_at.elapsed().as_secs_f64();
        write_silence_wav(&self.config.output_path, 48_000, 2, duration)?;
        Ok(self.config.output_path)
    }
}

/// Fallback microphone session for non-Windows platforms.
/// Writes a silence WAV file on stop.
pub struct PlatformMicrophoneSession {
    config: MicrophoneCaptureConfig,
    started_at: Instant,
}

impl PlatformMicrophoneSession {
    pub fn start(config: MicrophoneCaptureConfig) -> Result<Self> {
        let _ = config.device_id.as_deref();
        Ok(Self {
            config,
            started_at: Instant::now(),
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        let duration = self.started_at.elapsed().as_secs_f64();
        write_silence_wav(&self.config.output_path, 48_000, 2, duration)?;
        Ok(self.config.output_path)
    }
}

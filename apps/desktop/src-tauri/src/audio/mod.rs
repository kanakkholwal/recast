mod platform;
pub mod wav;

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use anyhow::Result;

/// Configuration for system/loopback audio capture.
#[derive(Debug, Clone)]
pub struct AudioCaptureConfig {
    /// Path to write the WAV output file.
    pub output_path: PathBuf,
    /// When set, capture continues draining the device but stops writing
    /// samples — keeps the WAV gap-free across recording pauses.
    pub pause_flag: Arc<AtomicBool>,
}

/// Handle to a running system audio capture session.
/// The capture runs on a background thread and writes PCM data to a WAV file.
/// Call `stop()` to finalize the WAV file and get the output path.
pub struct AudioCaptureSession {
    inner: platform::PlatformAudioSession,
}

impl AudioCaptureSession {
    pub fn start(config: AudioCaptureConfig) -> Result<Self> {
        let inner = platform::PlatformAudioSession::start(config)?;
        Ok(Self { inner })
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.inner.stop()
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
}

/// Handle to a running microphone capture session.
/// Captures from a specific microphone device and writes PCM data to a WAV file.
pub struct MicrophoneCaptureSession {
    inner: platform::PlatformMicrophoneSession,
}

impl MicrophoneCaptureSession {
    pub fn start(config: MicrophoneCaptureConfig) -> Result<Self> {
        let inner = platform::PlatformMicrophoneSession::start(config)?;
        Ok(Self { inner })
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.inner.stop()
    }
}

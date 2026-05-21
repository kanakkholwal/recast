#[cfg(windows)]
mod windows;

// macOS + Linux share an FFmpeg-based PCM-piped implementation, gated
// internally by `cfg` for the input-format / device-resolution helpers.
// See `ffmpeg_unix.rs` for the pause-aware streaming design.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod ffmpeg_unix;

// macOS-only ScreenCaptureKit backend for system-audio loopback. The
// FFmpeg path tries SCKit first (it's the only built-in macOS API for
// system audio without a virtual driver) and falls through to
// BlackHole / silence on failure.
#[cfg(target_os = "macos")]
mod macos_sckit;

// Genuine fallback — used only when neither Windows nor Unix paths apply.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod fallback;

#[cfg(windows)]
pub use windows::PlatformAudioSession;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use ffmpeg_unix::PlatformAudioSession;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub use fallback::PlatformAudioSession;

#[cfg(windows)]
pub use windows::PlatformMicrophoneSession;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use ffmpeg_unix::PlatformMicrophoneSession;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub use fallback::PlatformMicrophoneSession;

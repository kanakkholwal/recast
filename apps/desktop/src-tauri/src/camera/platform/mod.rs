#[cfg(windows)]
mod windows;

// macOS and Linux share one FFmpeg-based implementation —
// `avfoundation` and `v4l2` are both FFmpeg input formats, and only the
// device-resolution helpers differ. Two near-identical files would just
// be ceremony.
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod ffmpeg_unix;

// Genuine fallback — kept for any future target we haven't ported yet.
#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
mod fallback;

#[cfg(windows)]
pub use windows::PlatformCameraSession;

#[cfg(any(target_os = "macos", target_os = "linux"))]
pub use ffmpeg_unix::PlatformCameraSession;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
pub use fallback::PlatformCameraSession;

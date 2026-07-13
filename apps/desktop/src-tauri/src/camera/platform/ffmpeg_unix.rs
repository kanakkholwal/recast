//! Camera capture for macOS (AVFoundation) and Linux (V4L2) via FFmpeg.
//!
//! Mirrors the structure of `windows.rs` so the contract upstream of the
//! `PlatformCameraSession` is identical: spawn a thread that owns an
//! FFmpeg subprocess, signal stop with an `AtomicBool`, and validate the
//! produced MP4 before reporting success. Only the input format and
//! device-resolution helpers differ between the two Unix-y platforms,
//! so they share one file with `cfg`-gated sections instead of two
//! near-identical files.

use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use anyhow::{anyhow, Context, Result};

use crate::camera::CameraCaptureConfig;

// FFmpeg input format keyword per OS. Kept as a const so the `Command`
// builder below stays one shape and a future port (FreeBSD's `v4l2` is
// the same name) is a one-line addition.
#[cfg(target_os = "macos")]
const FF_INPUT_FORMAT: &str = "avfoundation";
#[cfg(target_os = "linux")]
const FF_INPUT_FORMAT: &str = "v4l2";

pub struct PlatformCameraSession {
    stop_flag: Arc<AtomicBool>,
    // `Option` so `stop()` can take the handle out (it needs the join result)
    // while `Drop` can still detect "stopped without a clean `stop()`" and tear
    // the capture thread + its FFmpeg child down instead of orphaning them.
    thread_handle: Option<JoinHandle<Result<PathBuf>>>,
}

impl PlatformCameraSession {
    pub fn start(config: CameraCaptureConfig) -> Result<Self> {
        let stop_flag = Arc::new(AtomicBool::new(false));
        let flag_clone = stop_flag.clone();
        let output_path = config.output_path.clone();

        let thread_handle = thread::Builder::new()
            .name("recast-camera".into())
            .spawn(move || camera_capture_thread(config, flag_clone))
            .context("failed to spawn camera capture thread")?;

        log::info!("camera capture started, output: {}", output_path.display());

        Ok(Self {
            stop_flag,
            thread_handle: Some(thread_handle),
        })
    }

    pub fn stop(mut self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::Release);
        match self.thread_handle.take() {
            Some(handle) => handle
                .join()
                .map_err(|_| anyhow!("camera capture thread panicked"))?,
            None => Err(anyhow!("camera session already stopped")),
        }
    }
}

impl Drop for PlatformCameraSession {
    fn drop(&mut self) {
        // Only fires when the session is dropped WITHOUT a clean `stop()` —
        // a panic or early return between start and the caller's `stop()`.
        // Without this the capture thread would spin forever and its FFmpeg
        // child would be orphaned (a stuck webcam light + zombie process).
        if let Some(handle) = self.thread_handle.take() {
            self.stop_flag.store(true, Ordering::Release);
            let _ = handle.join();
        }
    }
}

/// A death within this window of spawn is treated as an unsupported capture
/// mode (webcam rejects the requested size/framerate at startup) rather than a
/// mid-recording failure, so the caller retries a different mode.
const CAMERA_STARTUP_GRACE: Duration = Duration::from_secs(2);

/// Capture modes tried in order. Most webcams support 720p30, so it's first; if
/// a device rejects that exact mode FFmpeg exits at startup, so the second
/// candidate passes no `-video_size`/`-framerate` at all and lets the device
/// pick its native default rather than dropping the camera track. The overlay
/// compositor scales the camera to its placement, so a non-720p native
/// resolution is fine.
fn camera_mode_candidates() -> [&'static [&'static str]; 2] {
    [&["-framerate", "30", "-video_size", "1280x720"], &[]]
}

/// Whether an FFmpeg exit this soon after spawn should be treated as an
/// unsupported capture mode (worth retrying with another mode) rather than a
/// genuine mid-recording failure (which a different mode won't fix).
fn is_startup_failure(elapsed: Duration) -> bool {
    elapsed < CAMERA_STARTUP_GRACE
}

/// Outcome of one FFmpeg capture attempt against a single mode.
enum CameraAttempt {
    /// Captured and validated a usable file.
    Ok(PathBuf),
    /// FFmpeg exited at startup, almost always an unsupported capture mode.
    /// Worth retrying with a different `-video_size`/`-framerate`.
    StartupFailed(String),
    /// A failure a different mode won't fix (device missing, no frames from a
    /// permission denial / in-use camera, corrupt output, mid-recording death).
    Fatal(anyhow::Error),
}

fn camera_capture_thread(
    config: CameraCaptureConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf> {
    let device = resolve_camera_device(&config.device_name)?;
    let input = format_input_arg(&device);

    let mode_candidates = camera_mode_candidates();
    let candidate_count = mode_candidates.len();

    let mut startup_errors = Vec::new();
    for (index, mode_args) in mode_candidates.into_iter().enumerate() {
        if stop_flag.load(Ordering::Acquire) {
            return Err(anyhow!("camera capture stopped before it started"));
        }
        match run_camera_ffmpeg(&config, &input, mode_args, &stop_flag) {
            CameraAttempt::Ok(path) => return Ok(path),
            CameraAttempt::Fatal(e) => return Err(e),
            CameraAttempt::StartupFailed(stderr) => {
                let more = index + 1 < candidate_count;
                log::warn!(
                    "camera mode {mode_args:?} failed at startup ({FF_INPUT_FORMAT}){}: {stderr}",
                    if more {
                        "; retrying with the device's default mode"
                    } else {
                        ""
                    }
                );
                startup_errors.push(stderr);
            }
        }
    }

    Err(anyhow!(
        "camera capture could not start in any supported mode ({FF_INPUT_FORMAT}). {}. Last error: {}",
        permission_hint(),
        startup_errors.pop().unwrap_or_default()
    ))
}

/// Run a single FFmpeg camera capture with the given mode args, from spawn
/// through validation. Kept separate from `camera_capture_thread` so the mode
/// fallback can retry it without duplicating the capture/validation logic.
fn run_camera_ffmpeg(
    config: &CameraCaptureConfig,
    input: &str,
    mode_args: &[&str],
    stop_flag: &Arc<AtomicBool>,
) -> CameraAttempt {
    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command.args(["-y", "-f", FF_INPUT_FORMAT]);
    // AVFoundation requires framerate + size before -i; V4L2 accepts them in
    // the same position, so one ordering serves both. An empty `mode_args` lets
    // the device pick its native default (the fallback attempt).
    command.args(mode_args);
    command
        .args([
            "-i", input, "-c:v", "libx264", "-preset", "veryfast", "-pix_fmt", "yuv420p",
            "-an", // No audio from the camera; mic is captured separately.
        ])
        .arg(config.output_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);
    let mut child = match command.spawn() {
        Ok(child) => child,
        Err(e) => {
            return CameraAttempt::Fatal(anyhow!("failed to start FFmpeg camera capture: {e}"))
        }
    };
    let started = Instant::now();

    // Drain stderr continuously on a side thread. REQUIRED, not just diagnostic:
    // the camera FFmpeg runs for the whole (multi-minute) recording and writes
    // periodic `frame=…` progress to stderr; without a drainer the OS pipe fills
    // (smaller default buffers on macOS/Linux than Windows), FFmpeg blocks, and
    // capture deadlocks mid-recording. See `crate::ffmpeg::StderrTail`.
    let stderr_tail = child.stderr.take().map(crate::ffmpeg::StderrTail::spawn);

    while !stop_flag.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(100));
        if let Ok(Some(status)) = child.try_wait() {
            if status.success() {
                break;
            }
            let stderr = stderr_tail
                .as_ref()
                .map(|t| t.snapshot())
                .unwrap_or_default();
            // A death inside the startup window means the requested mode is
            // unsupported: let the caller try another. A later death is a
            // genuine mid-recording failure that a different mode won't fix.
            if is_startup_failure(started.elapsed()) {
                return CameraAttempt::StartupFailed(stderr);
            }
            return CameraAttempt::Fatal(anyhow!(
                "FFmpeg camera process exited early ({FF_INPUT_FORMAT}): {stderr}"
            ));
        }
    }

    let forced_kill = graceful_stop(&mut child);
    let stderr = stderr_tail.map(|t| t.collect()).unwrap_or_default();

    // A forced kill means FFmpeg didn't finalize within the timeout; the MP4
    // `moov` atom is written last, so a killed multi-minute capture leaves a
    // large-but-truncated, unplayable file the size check below would wave
    // through. Reject it so the camera track is dropped rather than committing a
    // corrupt clip into the .recast project.
    if forced_kill {
        return CameraAttempt::Fatal(anyhow!(
            "camera capture did not finalize within the timeout and was terminated \
             ({FF_INPUT_FORMAT}); dropping the camera track to avoid a corrupt file. {stderr}"
        ));
    }

    // Same MP4 sanity check as the Windows backend: FFmpeg can return 0 and
    // still leave us with a malformed / empty file if the `q` arrived before any
    // frame did, or if the device produced no frames (camera in use by another
    // app, blocked by TCC permission on macOS, etc.). The downstream finalize
    // step would otherwise commit an empty camera track into the .recast project.
    let metadata = match std::fs::metadata(&config.output_path) {
        Ok(metadata) => metadata,
        Err(e) => {
            return CameraAttempt::Fatal(anyhow!(
                "camera output missing: {}: {e}",
                config.output_path.display()
            ))
        }
    };
    if metadata.len() < 1024 {
        return CameraAttempt::Fatal(anyhow!(
            "camera output is too small ({} bytes); capture likely produced no frames. {}",
            metadata.len(),
            permission_hint()
        ));
    }

    log::info!(
        "camera capture finished: {} ({} bytes)",
        config.output_path.display(),
        metadata.len()
    );
    CameraAttempt::Ok(config.output_path.clone())
}

/// Normalise a user-supplied device name (or absence thereof) into a
/// concrete device identifier we can hand to FFmpeg. The JS recording
/// panel can pass "Default" or an empty string before the device picker
/// populates; we treat both as "pick the first available".
fn resolve_camera_device(requested: &Option<String>) -> Result<String> {
    let normalised = match requested.as_deref() {
        None => None,
        Some(s) => {
            let trimmed = s.trim();
            if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("default") {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
    };
    match normalised {
        Some(d) => Ok(d),
        None => first_available_camera().context(
            "no camera device available — check that a webcam is connected, not in use \
             by another app, and that camera access is permitted",
        ),
    }
}

#[cfg(target_os = "macos")]
fn format_input_arg(device: &str) -> String {
    // AVFoundation's input spec is "<video>:<audio>" — we only capture
    // video here, so the audio side stays empty (the trailing colon is
    // required, FFmpeg rejects bare integers).
    format!("{device}:")
}

#[cfg(target_os = "linux")]
fn format_input_arg(device: &str) -> String {
    // V4L2 takes a `/dev/video*` path directly. If a user-supplied
    // device name doesn't look like a path, assume it's an index.
    if device.starts_with("/dev/") {
        device.to_string()
    } else if let Ok(n) = device.parse::<u32>() {
        format!("/dev/video{n}")
    } else {
        device.to_string()
    }
}

#[cfg(target_os = "macos")]
fn first_available_camera() -> Result<String> {
    // Shared cached probe — see `ffmpeg::cached_avfoundation_devices`.
    // Pre-caching: audio loopback detection and the screen-index lookup
    // also call this, so the probe runs once per app launch regardless
    // of which subsystem needs it first.
    let stderr = crate::ffmpeg::cached_avfoundation_devices();
    if stderr.is_empty() {
        return Err(anyhow!(
            "AVFoundation device listing returned no output — \
             ffmpeg may be missing avfoundation support, or the probe failed"
        ));
    }
    // AVFoundation's listing format on stderr:
    //   [AVFoundation indev @ 0x...] AVFoundation video devices:
    //   [AVFoundation indev @ 0x...] [0] FaceTime HD Camera
    //   [AVFoundation indev @ 0x...] [1] Capture screen 0
    //   [AVFoundation indev @ 0x...] AVFoundation audio devices:
    //   ...
    // We want the FIRST entry under "video devices" that is NOT a
    // "Capture screen N" pseudo-device — screens are also listed there
    // but they are not webcams.
    let mut in_video = false;
    for line in stderr.lines() {
        if line.contains("video devices:") {
            in_video = true;
            continue;
        }
        if line.contains("audio devices:") {
            in_video = false;
            continue;
        }
        if !in_video {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if lower.contains("capture screen") {
            // Skip screens — they're not webcams.
            continue;
        }
        if let Some(idx) = avfoundation_index(line) {
            return Ok(idx.to_string());
        }
    }
    Err(anyhow!(
        "no AVFoundation video camera found; ensure a webcam is connected and \
         the app has Camera permission in System Settings → Privacy & Security"
    ))
}

/// Extract the FFmpeg device index from the LAST `[N]` bracket on a
/// listing line. Necessary because the line ALSO begins with a bracket
/// containing the libavformat pointer
/// (`[AVFoundation indev @ 0x600003f5c000]`), which we must skip.
#[cfg(target_os = "macos")]
fn avfoundation_index(line: &str) -> Option<u32> {
    let bytes = line.as_bytes();
    let close = bytes.iter().rposition(|&b| b == b']')?;
    let open = bytes[..close].iter().rposition(|&b| b == b'[')?;
    let inner = std::str::from_utf8(&bytes[open + 1..close]).ok()?;
    inner.trim().parse::<u32>().ok()
}

#[cfg(target_os = "linux")]
fn first_available_camera() -> Result<String> {
    // V4L2 cameras appear as `/dev/video*` nodes. /dev/video0 is the
    // overwhelmingly common case; USB cams may enumerate as 1, 2, ...
    // Pick the lowest-numbered node that exists. Cap at 16 because no
    // realistic system mounts more than a handful.
    for n in 0..16 {
        let path = format!("/dev/video{n}");
        if std::path::Path::new(&path).exists() {
            return Ok(path);
        }
    }
    Err(anyhow!(
        "no V4L2 video device found at /dev/video[0..16]; ensure the webcam driver \
         is loaded and the user is a member of the `video` group"
    ))
}

#[cfg(target_os = "macos")]
fn permission_hint() -> &'static str {
    "On macOS this commonly means Camera permission is not granted — \
     System Settings → Privacy & Security → Camera → enable Recast, \
     then restart the app."
}

#[cfg(target_os = "linux")]
fn permission_hint() -> &'static str {
    "On Linux this commonly means the user is not in the `video` group, \
     or another app is holding the device open."
}

/// Send "q" to FFmpeg's stdin for a graceful MP4 finalize, then escalate
/// to SIGKILL if it doesn't exit within ~5 s. The MP4 muxer needs the
/// graceful path so the `moov` atom gets written; killing first
/// produces an unplayable file.
/// Returns `true` if FFmpeg had to be force-killed (didn't finalize in time) —
/// the caller treats that as a corrupt-output signal.
fn graceful_stop(child: &mut Child) -> bool {
    if let Some(ref mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"q");
        let _ = stdin.flush();
    }
    for _ in 0..50 {
        if let Ok(Some(_)) = child.try_wait() {
            return false;
        }
        thread::sleep(Duration::from_millis(100));
    }
    log::warn!("FFmpeg camera process did not exit gracefully, killing");
    let _ = child.kill();
    let _ = child.wait();
    true
}

/// Runs on the macOS + Linux CI legs (see `.github/workflows/ci-desktop.yml`),
/// which is the only place this `cfg`-gated backend compiles at all.
#[cfg(test)]
mod tests {
    use super::*;

    /// 720p30 stays the first thing we ask for: it's what nearly every webcam
    /// supports and what the overlay was designed around.
    #[test]
    fn preferred_camera_mode_is_720p30() {
        let preferred = camera_mode_candidates()[0];
        assert!(preferred.contains(&"1280x720"));
        assert!(preferred.contains(&"30"));
        assert!(preferred.contains(&"-video_size"));
        assert!(preferred.contains(&"-framerate"));
    }

    /// The fallback must pass NO size/framerate flags so FFmpeg negotiates the
    /// device's native mode. A webcam that doesn't do exactly 720p30 used to
    /// exit at startup and lose the camera track entirely.
    #[test]
    fn fallback_camera_mode_lets_the_device_pick_its_native_format() {
        let fallback = camera_mode_candidates()[1];
        assert!(
            fallback.is_empty(),
            "fallback must not pin a mode, got {fallback:?}"
        );
    }

    /// An FFmpeg exit inside the grace window is a rejected mode (retry another);
    /// past it, the capture really was running, so a different mode won't help.
    #[test]
    fn early_exit_is_classified_as_a_startup_failure() {
        assert!(is_startup_failure(Duration::from_millis(0)));
        assert!(is_startup_failure(Duration::from_millis(200)));
        assert!(is_startup_failure(
            CAMERA_STARTUP_GRACE - Duration::from_millis(1)
        ));
    }

    #[test]
    fn exit_after_the_grace_window_is_a_mid_recording_failure() {
        assert!(!is_startup_failure(CAMERA_STARTUP_GRACE));
        assert!(!is_startup_failure(Duration::from_secs(30)));
    }
}

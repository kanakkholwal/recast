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
use std::time::Duration;

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
    thread_handle: JoinHandle<Result<PathBuf>>,
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
            thread_handle,
        })
    }

    pub fn stop(self) -> Result<PathBuf> {
        self.stop_flag.store(true, Ordering::Release);
        self.thread_handle
            .join()
            .map_err(|_| anyhow!("camera capture thread panicked"))?
    }
}

fn camera_capture_thread(
    config: CameraCaptureConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf> {
    let device = resolve_camera_device(&config.device_name)?;
    let input = format_input_arg(&device);

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args([
            "-y",
            "-f",
            FF_INPUT_FORMAT,
            // AVFoundation requires framerate + size before -i. V4L2
            // accepts them in the same position, so one ordering serves
            // both — keeps the arg vector flat instead of cfg'd.
            "-framerate",
            "30",
            "-video_size",
            "1280x720",
            "-i",
            &input,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-an", // No audio from the camera — mic is captured separately.
        ])
        .arg(config.output_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);
    let mut child = command
        .spawn()
        .context("failed to start FFmpeg camera capture")?;

    while !stop_flag.load(Ordering::Acquire) {
        thread::sleep(Duration::from_millis(100));
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() {
                let stderr = read_child_stderr(&mut child);
                return Err(anyhow!(
                    "FFmpeg camera process exited early ({FF_INPUT_FORMAT}): {stderr}"
                ));
            }
            break;
        }
    }

    graceful_stop(&mut child);

    // Same MP4 sanity check as the Windows backend: FFmpeg can return 0
    // and still leave us with a malformed / empty file if the `q`
    // arrived before any frame did, or if the device produced no frames
    // (camera in use by another app, blocked by TCC permission on
    // macOS, etc.). The downstream finalize step would otherwise commit
    // an empty camera track into the .recast project.
    let metadata = std::fs::metadata(&config.output_path)
        .with_context(|| format!("camera output missing: {}", config.output_path.display()))?;
    if metadata.len() < 1024 {
        return Err(anyhow!(
            "camera output is too small ({} bytes) — capture likely produced no frames. \
             {}",
            metadata.len(),
            permission_hint()
        ));
    }

    log::info!(
        "camera capture finished: {} ({} bytes)",
        config.output_path.display(),
        metadata.len()
    );
    Ok(config.output_path)
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
    let stderr = list_avfoundation_devices()
        .context("failed to invoke ffmpeg for avfoundation device enumeration")?;
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

#[cfg(target_os = "macos")]
fn list_avfoundation_devices() -> Result<String> {
    let output = Command::new(crate::ffmpeg::ffmpeg_path())
        .args([
            "-hide_banner",
            "-f",
            "avfoundation",
            "-list_devices",
            "true",
            "-i",
            "",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()?;
    Ok(String::from_utf8_lossy(&output.stderr).into_owned())
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
fn graceful_stop(child: &mut Child) {
    if let Some(ref mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"q");
        let _ = stdin.flush();
    }
    for _ in 0..50 {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        thread::sleep(Duration::from_millis(100));
    }
    log::warn!("FFmpeg camera process did not exit gracefully, killing");
    let _ = child.kill();
    let _ = child.wait();
}

fn read_child_stderr(child: &mut Child) -> String {
    use std::io::Read;
    let mut stderr_str = String::new();
    if let Some(ref mut stderr) = child.stderr {
        let _ = stderr.read_to_string(&mut stderr_str);
    }
    if stderr_str.len() > 500 {
        stderr_str.truncate(500);
    }
    stderr_str
}

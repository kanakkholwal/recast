use std::io::Write;
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{self, JoinHandle};

use anyhow::{anyhow, Context, Result};

use crate::camera::CameraCaptureConfig;

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

/// Capture camera video via FFmpeg DirectShow input on Windows.
/// Spawns an FFmpeg child process that records from the webcam until stopped.
fn camera_capture_thread(
    config: CameraCaptureConfig,
    stop_flag: Arc<AtomicBool>,
) -> Result<PathBuf> {
    // Resolve the device name. The JS recording panel defaults its
    // `selectedCameraName` state to the literal string "Default" before
    // the device list comes back from `get_camera_devices` — and that
    // string would be passed through here as `video=Default`, which
    // DirectShow rejects (no device is literally named "Default"). When
    // we get an empty / "default" / unspecified name, enumerate real
    // devices and pick the first one, instead of letting FFmpeg fail
    // immediately with a cryptic stderr.
    let resolved_name = match config.device_name.as_deref() {
        None => None,
        Some(s) => {
            let trimmed = s.trim();
            let lower = trimmed.to_ascii_lowercase();
            // Strip the optional "video=" prefix the JS side sometimes adds
            // before comparing, so "video=Default" maps to the same fallback.
            let stripped = lower
                .strip_prefix("video=")
                .map(|s| s.trim())
                .unwrap_or(lower.as_str());
            if stripped.is_empty() || stripped == "default" {
                None
            } else {
                Some(trimmed.to_string())
            }
        }
    };
    let device_name = match resolved_name {
        Some(name) => name,
        None => first_available_camera()
            .context("no camera device available — check that a webcam is connected and not in use by another app")?,
    };

    // Build the DirectShow input specifier.
    let input = if device_name.starts_with("video=") {
        device_name.to_string()
    } else {
        format!("video={device_name}")
    };

    let mut command = Command::new(crate::ffmpeg::ffmpeg_path());
    command
        .args([
            "-y",
            "-f",
            "dshow",
            "-video_size",
            "1280x720",
            "-framerate",
            "30",
            "-i",
            &input,
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-pix_fmt",
            "yuv420p",
            "-an", // No audio from camera
        ])
        .arg(config.output_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    crate::ffmpeg::configure_silent_command(&mut command);
    let mut child = command
        .spawn()
        .context("failed to start FFmpeg camera capture")?;

    // Wait for the stop signal, polling periodically.
    while !stop_flag.load(Ordering::Acquire) {
        thread::sleep(std::time::Duration::from_millis(100));

        // Check if FFmpeg exited unexpectedly.
        if let Ok(Some(status)) = child.try_wait() {
            if !status.success() {
                let stderr = read_child_stderr(&mut child);
                return Err(anyhow!("FFmpeg camera process exited early: {stderr}"));
            }
            break;
        }
    }

    // Gracefully stop FFmpeg by writing "q" to stdin (FFmpeg's quit command).
    graceful_stop(&mut child);

    // Validate the output file actually got written. FFmpeg can exit
    // cleanly (status 0) yet produce a missing or empty MP4 if the
    // `q` arrived before any frame made it through the encoder, or if
    // the device produced no frames in the first place. Without this
    // check, the recording-finalize step would happily write an empty
    // entry into the .recast archive and the editor would later fail
    // to play back the camera track or surface a confusing empty state.
    let metadata = std::fs::metadata(&config.output_path)
        .with_context(|| format!("camera output missing: {}", config.output_path.display()))?;
    // 1 KB is well below any real MP4 (the moov atom alone runs hundreds
    // of bytes for an empty file) but above zero. A real recording is
    // always at least tens of KB.
    if metadata.len() < 1024 {
        return Err(anyhow!(
            "camera output is too small ({} bytes) — capture likely produced no frames",
            metadata.len()
        ));
    }

    log::info!(
        "camera capture finished: {} ({} bytes)",
        config.output_path.display(),
        metadata.len()
    );
    Ok(config.output_path)
}

/// Enumerate DirectShow video devices and return the first one's friendly
/// name. Used as the fallback when the user enabled the camera but didn't
/// pick a specific device.
fn first_available_camera() -> Result<String> {
    let output = Command::new(crate::ffmpeg::ffmpeg_path())
        .args([
            "-hide_banner",
            "-list_devices",
            "true",
            "-f",
            "dshow",
            "-i",
            "dummy",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .stdin(Stdio::null())
        .output()
        .context("failed to invoke ffmpeg for device enumeration")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    // FFmpeg's dshow listing format: lines like
    //   [dshow @ 000001234] "Logitech HD Pro Webcam C920" (video)
    // We want the first quoted name on a line that ends with "(video)".
    for line in stderr.lines() {
        let trimmed = line.trim();
        if !trimmed.ends_with("(video)") {
            continue;
        }
        if let Some(start) = trimmed.find('"') {
            let after_quote = &trimmed[start + 1..];
            if let Some(end) = after_quote.find('"') {
                let name = after_quote[..end].trim();
                if !name.is_empty() {
                    return Ok(name.to_string());
                }
            }
        }
    }
    Err(anyhow!(
        "no DirectShow video devices found; ensure a webcam is connected and accessible"
    ))
}

/// Send "q" to FFmpeg's stdin for graceful shutdown, then wait with timeout.
fn graceful_stop(child: &mut Child) {
    if let Some(ref mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(b"q");
        let _ = stdin.flush();
    }

    // Wait up to 5 seconds for FFmpeg to finalize the MP4.
    for _ in 0..50 {
        if let Ok(Some(_)) = child.try_wait() {
            return;
        }
        thread::sleep(std::time::Duration::from_millis(100));
    }

    // Force kill if it didn't exit gracefully.
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

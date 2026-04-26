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
    let device_name = config.device_name.as_deref().unwrap_or("video=default");

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

    log::info!("camera capture finished: {}", config.output_path.display());
    Ok(config.output_path)
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

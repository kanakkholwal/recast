use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use anyhow::{anyhow, Context, Result};

use crate::recording::{pipeline::RecordingPipeline, CaptureArea};

/// Configuration for the live recording encoder.
#[derive(Clone, Debug)]
pub struct EncoderConfig {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub crop: Option<CaptureArea>,
    pub output_path: PathBuf,
}

fn build_video_filter(crop: Option<CaptureArea>) -> Option<String> {
    crop.map(|area| {
        format!(
            "crop={}:{}:{}:{}",
            area.width,
            area.height,
            area.x.max(0),
            area.y.max(0)
        )
    })
}

/// Spawn the encoder thread. Pulls raw BGRA frames from the pipeline
/// and pipes them to FFmpeg for H.264 encoding.
pub fn spawn_encoder_loop(
    config: EncoderConfig,
    stop_flag: Arc<AtomicBool>,
    pipeline: RecordingPipeline,
) -> Result<thread::JoinHandle<Result<()>>> {
    thread::Builder::new()
        .name("recast-encoder".into())
        .spawn(move || {
            let encoder = crate::ffmpeg::preferred_h264_encoder();
            let mut args = vec![
                "-y".to_string(),
                "-f".to_string(),
                "rawvideo".to_string(),
                "-pixel_format".to_string(),
                "bgra".to_string(),
                "-video_size".to_string(),
                format!("{}x{}", config.width, config.height),
                "-framerate".to_string(),
                config.fps.to_string(),
                "-i".to_string(),
                "-".to_string(),
                "-an".to_string(),
            ];

            if let Some(filter) = build_video_filter(config.crop) {
                args.extend(["-vf".to_string(), filter]);
            }

            match encoder {
                "h264_nvenc" => {
                    args.extend([
                        "-c:v".to_string(),
                        "h264_nvenc".to_string(),
                        "-preset".to_string(),
                        "p5".to_string(),
                        "-tune".to_string(),
                        "ll".to_string(),
                        "-pix_fmt".to_string(),
                        "yuv420p".to_string(),
                    ]);
                }
                _ => {
                    args.extend([
                        "-c:v".to_string(),
                        "libx264".to_string(),
                        "-preset".to_string(),
                        "veryfast".to_string(),
                        "-pix_fmt".to_string(),
                        "yuv420p".to_string(),
                    ]);
                }
            }

            args.push(config.output_path.to_string_lossy().to_string());

            let mut child = Command::new(crate::ffmpeg::ffmpeg_path())
                .args(&args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .with_context(|| "failed to start ffmpeg encoder")?;

            let mut stdin = child
                .stdin
                .take()
                .context("ffmpeg encoder stdin was not available")?;
            let stats = pipeline.stats();

            loop {
                if let Some(frame) = pipeline.pop() {
                    stdin.write_all(&frame.data)?;
                    stats.encoded_frames.fetch_add(1, Ordering::Relaxed);
                    continue;
                }

                if stop_flag.load(Ordering::Acquire) && pipeline.is_empty() {
                    break;
                }

                thread::sleep(Duration::from_millis(2));
            }

            drop(stdin);

            let output = child.wait_with_output()?;
            if !output.status.success() {
                return Err(anyhow!(
                    "ffmpeg encoder failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                ));
            }

            Ok(())
        })
        .map_err(Into::into)
}

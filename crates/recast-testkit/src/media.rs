use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use crate::{audio, timecode};

#[derive(Debug, Clone, Copy)]
pub struct SourceSpec {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub duration_secs: f64,
    pub sample_rate: u32,
    pub channels: u16,
}

impl Default for SourceSpec {
    fn default() -> Self {
        Self {
            width: 640,
            height: 360,
            fps: 30,
            duration_secs: 3.0,
            sample_rate: 48_000,
            channels: 2,
        }
    }
}

impl SourceSpec {
    pub fn frame_count(&self) -> u64 {
        (self.duration_secs * self.fps as f64).round() as u64
    }

    pub fn expected_clicks(&self) -> Vec<f64> {
        (0..self.duration_secs.ceil() as u64)
            .map(|s| s as f64)
            .filter(|s| *s < self.duration_secs)
            .collect()
    }
}

/// Resolution order: `RECAST_FFMPEG`, the repo's bundled sidecar, then PATH.
/// Returns `None` when none of them is runnable, so a harness can skip rather
/// than fail on a machine without FFmpeg.
pub fn ffmpeg_path() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("RECAST_FFMPEG") {
        let path = PathBuf::from(explicit);
        if runnable(&path) {
            return Some(path);
        }
    }
    for candidate in bundled_candidates() {
        if runnable(&candidate) {
            return Some(candidate);
        }
    }
    let bare = PathBuf::from(if cfg!(windows) {
        "ffmpeg.exe"
    } else {
        "ffmpeg"
    });
    runnable(&bare).then_some(bare)
}

fn bundled_candidates() -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../apps/desktop/src-tauri/binaries");
    let suffix = if cfg!(windows) { ".exe" } else { "" };
    let triple = if cfg!(all(windows, target_arch = "x86_64")) {
        "x86_64-pc-windows-msvc"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "aarch64-apple-darwin"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "x86_64-apple-darwin"
    } else {
        "x86_64-unknown-linux-gnu"
    };
    vec![
        root.join(format!("ffmpeg-{triple}{suffix}")),
        root.join(format!("ffmpeg{suffix}")),
    ]
}

fn runnable(path: &Path) -> bool {
    Command::new(path)
        .arg("-version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Encode a synthetic timecode video plus click track to `out`. The video is fed
/// as rawvideo so the frames are exactly what [`timecode::render_frame`] produced.
pub fn write_source(ffmpeg: &Path, spec: SourceSpec, out: &Path) -> Result<(), String> {
    let wav = out.with_extension("source.wav");
    write_wav(
        &wav,
        &audio::click_track(spec.sample_rate, spec.channels, spec.duration_secs),
        spec.sample_rate,
        spec.channels,
    )?;

    let mut child = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-y",
            "-f",
            "rawvideo",
            "-pixel_format",
            "rgba",
            "-video_size",
            &format!("{}x{}", spec.width, spec.height),
            "-framerate",
            &spec.fps.to_string(),
            "-i",
            "-",
            "-i",
            &wav.to_string_lossy(),
            "-c:v",
            "libx264",
            "-preset",
            "veryfast",
            "-crf",
            "12",
            "-pix_fmt",
            "yuv420p",
            "-c:a",
            "aac",
            "-shortest",
            &out.to_string_lossy(),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn ffmpeg: {e}"))?;

    let mut stdin = child.stdin.take().ok_or("ffmpeg stdin unavailable")?;
    for index in 0..spec.frame_count() {
        let frame = timecode::render_frame(spec.width, spec.height, index);
        stdin
            .write_all(&frame)
            .map_err(|e| format!("write frame {index}: {e}"))?;
    }
    drop(stdin);

    let output = child.wait_with_output().map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&wav);
    if !output.status.success() {
        return Err(format!(
            "ffmpeg encode failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(())
}

pub fn read_frames(
    ffmpeg: &Path,
    path: &Path,
    width: u32,
    height: u32,
) -> Result<Vec<Vec<u8>>, String> {
    let output = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &path.to_string_lossy(),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgba",
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let frame_bytes = (width * height * 4) as usize;
    Ok(output
        .stdout
        .chunks(frame_bytes)
        .filter(|c| c.len() == frame_bytes)
        .map(<[u8]>::to_vec)
        .collect())
}

pub fn read_samples(
    ffmpeg: &Path,
    path: &Path,
    sample_rate: u32,
    channels: u16,
) -> Result<Vec<i16>, String> {
    let output = Command::new(ffmpeg)
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-i",
            &path.to_string_lossy(),
            "-f",
            "s16le",
            "-ac",
            &channels.to_string(),
            "-ar",
            &sample_rate.to_string(),
            "-",
        ])
        .stdin(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(output
        .stdout
        .as_chunks::<2>()
        .0
        .iter()
        .map(|&b| i16::from_le_bytes(b))
        .collect())
}

fn write_wav(path: &Path, samples: &[i16], sample_rate: u32, channels: u16) -> Result<(), String> {
    let data_bytes = (samples.len() * 2) as u32;
    let block_align = channels * 2;
    let mut out = Vec::with_capacity(44 + data_bytes as usize);
    out.extend(b"RIFF");
    out.extend((36 + data_bytes).to_le_bytes());
    out.extend(b"WAVEfmt ");
    out.extend(16u32.to_le_bytes());
    out.extend(1u16.to_le_bytes());
    out.extend(channels.to_le_bytes());
    out.extend(sample_rate.to_le_bytes());
    out.extend((sample_rate * block_align as u32).to_le_bytes());
    out.extend(block_align.to_le_bytes());
    out.extend(16u16.to_le_bytes());
    out.extend(b"data");
    out.extend(data_bytes.to_le_bytes());
    for sample in samples {
        out.extend(sample.to_le_bytes());
    }
    std::fs::write(path, out).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_count_follows_duration_and_rate() {
        let spec = SourceSpec {
            fps: 60,
            duration_secs: 2.5,
            ..Default::default()
        };
        assert_eq!(spec.frame_count(), 150);
    }

    #[test]
    fn expected_clicks_stop_before_the_end() {
        let spec = SourceSpec {
            duration_secs: 3.0,
            ..Default::default()
        };
        assert_eq!(spec.expected_clicks(), vec![0.0, 1.0, 2.0]);
    }
}

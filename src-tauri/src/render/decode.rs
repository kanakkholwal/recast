use std::path::Path;
use std::process::Command;

use super::frame::Frame;

/// Decode a single video frame at the given timestamp using FFmpeg.
/// Returns an RGBA Frame.
pub fn decode_frame_at(video_path: &Path, timestamp: f64, max_width: Option<u32>) -> Result<Frame, String> {
    let mut args = vec![
        "-v".to_string(),
        "error".to_string(),
        "-ss".to_string(),
        format!("{timestamp:.3}"),
        "-i".to_string(),
        video_path.to_string_lossy().to_string(),
        "-frames:v".to_string(),
        "1".to_string(),
    ];

    // Optional downscale for preview performance.
    if let Some(max_w) = max_width {
        args.extend([
            "-vf".to_string(),
            format!("scale='min(iw,{max_w})':-1:flags=lanczos"),
        ]);
    }

    args.extend([
        "-pix_fmt".to_string(),
        "rgba".to_string(),
        "-f".to_string(),
        "rawvideo".to_string(),
        "-".to_string(),
    ]);

    let output = Command::new("ffmpeg")
        .args(&args)
        .output()
        .map_err(|e| format!("failed to run ffmpeg: {e}"))?;

    if !output.status.success() || output.stdout.is_empty() {
        return Err(format!(
            "frame decode failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    // We need to determine the dimensions of the decoded frame.
    // Probe the video first.
    let (width, height) = probe_frame_dimensions(video_path, max_width)?;

    let expected_size = (width * height * 4) as usize;
    if output.stdout.len() < expected_size {
        return Err(format!(
            "decoded frame size mismatch: got {} bytes, expected {} ({}x{})",
            output.stdout.len(),
            expected_size,
            width,
            height
        ));
    }

    Ok(Frame {
        width,
        height,
        data: output.stdout[..expected_size].to_vec(),
    })
}

/// Probe video dimensions, accounting for optional max_width scaling.
fn probe_frame_dimensions(video_path: &Path, max_width: Option<u32>) -> Result<(u32, u32), String> {
    let output = Command::new("ffprobe")
        .args([
            "-v", "error",
            "-select_streams", "v:0",
            "-show_entries", "stream=width,height",
            "-of", "csv=p=0:s=x",
            &video_path.to_string_lossy(),
        ])
        .output()
        .map_err(|e| format!("ffprobe failed: {e}"))?;

    let text = String::from_utf8_lossy(&output.stdout);
    let text = text.trim();
    let (w, h) = text
        .split_once('x')
        .ok_or_else(|| format!("unexpected ffprobe output: {text}"))?;

    let w: u32 = w.parse().map_err(|_| format!("bad width: {w}"))?;
    let h: u32 = h.parse().map_err(|_| format!("bad height: {h}"))?;

    if let Some(max_w) = max_width {
        if w > max_w {
            let scale = max_w as f64 / w as f64;
            let scaled_h = ((h as f64 * scale).round() as u32) & !1; // even height
            return Ok((max_w, scaled_h));
        }
    }

    Ok((w, h))
}

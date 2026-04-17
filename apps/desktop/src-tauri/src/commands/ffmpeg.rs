use std::io::Cursor;
use std::path::Path;
use std::process::Command;

use base64::{engine::general_purpose, Engine as _};
use image::codecs::png::PngEncoder;
use image::{ColorType, ImageEncoder};

use super::types::{ExportProfile, VideoMetadata, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};
use crate::ffmpeg::ffprobe_path;

pub fn resolve_export_profile(quality: &str) -> ExportProfile {
    match quality {
        "small" => ExportProfile {
            max_width: Some(1280),
            max_height: Some(720),
            mp4_crf: 28,
            mp4_preset: "veryfast",
            mp4_nvenc_cq: 32,
            webm_crf: 34,
            gif_fps: 12,
        },
        "4k" => ExportProfile {
            max_width: Some(3840),
            max_height: Some(2160),
            mp4_crf: 18,
            mp4_preset: "slow",
            mp4_nvenc_cq: 22,
            webm_crf: 24,
            gif_fps: 18,
        },
        "source" => ExportProfile {
            max_width: None,
            max_height: None,
            mp4_crf: 20,
            mp4_preset: "slow",
            mp4_nvenc_cq: 24,
            webm_crf: 28,
            gif_fps: 18,
        },
        _ => ExportProfile {
            max_width: Some(1920),
            max_height: Some(1080),
            mp4_crf: 22,
            mp4_preset: "medium",
            mp4_nvenc_cq: 26,
            webm_crf: 30,
            gif_fps: 15,
        },
    }
}

pub fn build_output_scale_filter(profile: ExportProfile) -> Option<String> {
    match (profile.max_width, profile.max_height) {
        (Some(max_width), Some(max_height)) => Some(format!(
            "scale=w='min(iw,{max_width})':h='min(ih,{max_height})':force_original_aspect_ratio=decrease:flags=lanczos"
        )),
        _ => None,
    }
}

pub fn append_output_filters_to_complex(
    filter_complex: &str,
    input_label: &str,
    filters: &[String],
) -> (String, String) {
    let final_label = "[vfinal]";
    let normalized_input = if input_label.starts_with('[') {
        input_label.to_string()
    } else {
        format!("[{input_label}]")
    };

    (
        format!(
            "{filter_complex};{normalized_input}{}{final_label}",
            filters.join(",")
        ),
        final_label.to_string(),
    )
}

/// Append a cursor overlay stage to an existing filter_complex string.
/// Takes the current `video_map` label (e.g. "[vout]" or "0:v:0") and the
/// FFmpeg input index of the cursor overlay video, and returns the new
/// filter_complex string + the new video_map label.
pub fn append_cursor_overlay_to_complex(
    filter_complex: Option<&str>,
    current_video_map: &str,
    cursor_input_index: usize,
) -> (String, String) {
    let out_label = "[vcursor]";
    let normalized_current = if current_video_map.starts_with('[') {
        current_video_map.to_string()
    } else {
        format!("[{current_video_map}]")
    };
    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!(
            "{existing};{normalized_current}[{cursor_input_index}:v]overlay=0:0:format=auto{out_label}"
        ),
        _ => format!(
            "{normalized_current}[{cursor_input_index}:v]overlay=0:0:format=auto{out_label}"
        ),
    };
    (new_complex, out_label.to_string())
}

/// Wrap the current video chain in a palettegen/paletteuse pipeline so GIF
/// exports have a stable, dithered palette instead of FFmpeg's naive
/// per-frame 256-colour quantization (which produces heavy banding and noise).
/// Always routes through `filter_complex`: the `split`/labelled-graph needed
/// by palettegen is not expressible in the linear `-vf` form.
///
/// Returns the extended `filter_complex` string and the new output label to
/// pass to `-map`. Any inline scale filter is baked into the `paletteuse` leg
/// so we don't double-sample.
pub fn build_gif_palette_complex(
    filter_complex: Option<&str>,
    input_label: &str,
    fps: u32,
    inline_scale: Option<&str>,
) -> (String, String) {
    let final_label = "[vgif]";
    let normalized_input = if input_label.starts_with('[') {
        input_label.to_string()
    } else {
        format!("[{input_label}]")
    };
    // The `[b]` leg carries the frames through paletteuse. If the caller has
    // a scale filter, apply it here (after fps reduction) so the generated
    // palette matches the pixels that will actually end up in the GIF.
    let scaled_b = match inline_scale {
        Some(scale) if !scale.is_empty() => format!("[_gifb]{scale}[_gifbs]"),
        _ => String::new(),
    };
    let (b_label, b_stage) = if scaled_b.is_empty() {
        ("[_gifb]", String::new())
    } else {
        ("[_gifbs]", format!(";{scaled_b}"))
    };
    let palette_chain = format!(
        "{normalized_input}fps={fps},split[_gifa][_gifb];[_gifa]palettegen=stats_mode=diff[_gifp]{b_stage};{b_label}[_gifp]paletteuse=dither=bayer:bayer_scale=5:diff_mode=rectangle{final_label}"
    );
    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!("{existing};{palette_chain}"),
        _ => palette_chain,
    };
    (new_complex, final_label.to_string())
}

pub fn summarize_ffmpeg_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect();

    if lines.is_empty() {
        "FFmpeg failed without returning a detailed error.".into()
    } else {
        lines
            .iter()
            .rev()
            .take(8)
            .copied()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect::<Vec<_>>()
            .join("\n")
    }
}

pub fn probe_video_metadata(path: &Path) -> Result<VideoMetadata, String> {
    if !path.exists() {
        return Err("File not found".into());
    }

    let size_bytes = std::fs::metadata(path).map(|m| m.len()).unwrap_or_default();
    let path_string = path.to_string_lossy().to_string();
    let mut command = Command::new(ffprobe_path());
    command.args([
        "-v",
        "quiet",
        "-print_format",
        "json",
        "-show_format",
        "-show_streams",
        &path_string,
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output();

    match output {
        Ok(out) if out.status.success() => {
            let parsed: serde_json::Value =
                serde_json::from_slice(&out.stdout).map_err(|e| e.to_string())?;
            let duration = parsed["format"]["duration"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .unwrap_or_default();
            let video_stream = parsed["streams"].as_array().and_then(|streams| {
                streams
                    .iter()
                    .find(|stream| stream["codec_type"].as_str() == Some("video"))
            });

            let (width, height, fps, codec) = if let Some(stream) = video_stream {
                let fps_text = stream["r_frame_rate"].as_str().unwrap_or("30/1");
                let fps = if let Some((num, den)) = fps_text.split_once('/') {
                    let num = num.parse::<f64>().unwrap_or(30.0);
                    let den = den.parse::<f64>().unwrap_or(1.0);
                    if den > 0.0 {
                        num / den
                    } else {
                        30.0
                    }
                } else {
                    fps_text.parse::<f64>().unwrap_or(30.0)
                };

                (
                    stream["width"].as_u64().unwrap_or_default() as u32,
                    stream["height"].as_u64().unwrap_or_default() as u32,
                    fps,
                    stream["codec_name"]
                        .as_str()
                        .unwrap_or("unknown")
                        .to_string(),
                )
            } else {
                (0, 0, 30.0, "unknown".into())
            };

            Ok(VideoMetadata {
                duration,
                width,
                height,
                fps,
                codec,
                size_bytes,
            })
        }
        _ => Ok(VideoMetadata {
            duration: 0.0,
            width: 0,
            height: 0,
            fps: 30.0,
            codec: "unknown".into(),
            size_bytes,
        }),
    }
}

pub fn has_audio(path: &Path) -> bool {
    let mut command = Command::new(ffprobe_path());
    command.args([
        "-v",
        "error",
        "-select_streams",
        "a",
        "-show_entries",
        "stream=index",
        "-of",
        "csv=p=0",
        &path.to_string_lossy(),
    ]);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x08000000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let output = command.output();

    matches!(
        output,
        Ok(result) if result.status.success() && !String::from_utf8_lossy(&result.stdout).trim().is_empty()
    )
}

pub fn make_thumbnail(img: &image::RgbaImage) -> image::RgbaImage {
    let (w, h) = (img.width(), img.height());
    if w == 0 || h == 0 {
        return image::RgbaImage::from_pixel(
            THUMBNAIL_WIDTH,
            THUMBNAIL_HEIGHT,
            image::Rgba([0, 0, 0, 255]),
        );
    }

    let scale = (THUMBNAIL_WIDTH as f32 / w as f32)
        .min(THUMBNAIL_HEIGHT as f32 / h as f32)
        .max(f32::MIN_POSITIVE);
    let scaled_w = (w as f32 * scale)
        .round()
        .clamp(1.0, THUMBNAIL_WIDTH as f32) as u32;
    let scaled_h = (h as f32 * scale)
        .round()
        .clamp(1.0, THUMBNAIL_HEIGHT as f32) as u32;
    let resized = image::imageops::resize(
        img,
        scaled_w,
        scaled_h,
        image::imageops::FilterType::Triangle,
    );
    let mut canvas = image::RgbaImage::from_pixel(
        THUMBNAIL_WIDTH,
        THUMBNAIL_HEIGHT,
        image::Rgba([18, 18, 20, 255]),
    );
    let ox = (THUMBNAIL_WIDTH - scaled_w) / 2;
    let oy = (THUMBNAIL_HEIGHT - scaled_h) / 2;
    image::imageops::overlay(&mut canvas, &resized, ox as i64, oy as i64);
    canvas
}

pub fn encode_thumbnail_base64(img: &image::RgbaImage) -> Option<String> {
    let mut buf = Cursor::new(Vec::new());
    let enc = PngEncoder::new(&mut buf);
    enc.write_image(
        img.as_raw(),
        img.width(),
        img.height(),
        ColorType::Rgba8.into(),
    )
    .ok()?;
    let b64 = general_purpose::STANDARD.encode(buf.into_inner());
    Some(format!("data:image/png;base64,{b64}"))
}

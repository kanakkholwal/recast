//! Format-specific codec + output args for the video export. Appends the encoder
//! flags, audio codec, and output path for GIF / WebM(VP9) / MP4(H.264). Split
//! out of commands/editor.rs's `export_video`.

use std::path::Path;

use crate::commands::ffmpeg::ExportSpeed;
use crate::commands::types::{ExportProfile, GifSettings};

/// Clamp a software-x264 `-preset` so a GPU-less export can't run the sub-realtime
/// `slow`/`slower`/`veryslow` tiers at 4K. Faster presets pass through unchanged.
fn cap_software_x264_preset(preset: &str) -> &str {
    match preset {
        "slow" | "slower" | "veryslow" | "placebo" => "medium",
        other => other,
    }
}

/// Append the codec/output tail to `args` for the requested `format`. Mirrors the
/// former inline `match request.format` in `export_video` verbatim.
#[expect(
    clippy::too_many_arguments,
    reason = "one call site; grouping these into a struct would only move the argument list"
)]
pub(crate) fn append_codec_args(
    args: &mut Vec<String>,
    format: &str,
    gif_settings: &GifSettings,
    profile: &ExportProfile,
    speed: ExportSpeed,
    has_audio_map: bool,
    output_path: &Path,
    // Retry path after a hardware encoder (NVENC/AMF/QSV) crashes mid-encode.
    force_software: bool,
) {
    match format {
        "gif" => {
            // Explicit `-c:v gif` and `-f gif` stop auto-detect emitting 'Could not find tag for codec none'; `-vsync 0` keeps the in-graph `fps=` timing.
            args.extend([
                "-c:v".to_string(),
                "gif".to_string(),
                "-f".to_string(),
                "gif".to_string(),
                "-an".to_string(),
                "-vsync".to_string(),
                "0".to_string(),
                "-loop".to_string(),
                gif_settings.ffmpeg_loop_arg().to_string(),
                output_path.to_string_lossy().to_string(),
            ]);
        }
        "webm" => {
            // libvpx-vp9 defaults to single-threaded `deadline=best`; row-mt, `cpu-used=4`, `deadline=good` and tile-columns give roughly 4-8x at the same CRF.
            args.extend([
                "-c:v".to_string(),
                "libvpx-vp9".to_string(),
                "-crf".to_string(),
                profile.webm_crf.to_string(),
                "-b:v".to_string(),
                "0".to_string(),
                "-deadline".to_string(),
                "good".to_string(),
                "-cpu-used".to_string(),
                speed.vp9_cpu_used().to_string(),
                "-row-mt".to_string(),
                "1".to_string(),
                "-tile-columns".to_string(),
                "1".to_string(),
                "-threads".to_string(),
                "0".to_string(),
            ]);
            if has_audio_map {
                args.extend(["-c:a".to_string(), "libopus".to_string()]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
        _ => {
            // No `+faststart`: its end-of-mux moov rewrite takes 10-60s on 4K with no progress, which reads as a stuck Finalizing.
            let encoder = if force_software {
                crate::encoder::h264::H264Encoder::Libx264
            } else {
                crate::encoder::h264::H264Encoder::from_ffmpeg_name(
                    crate::ffmpeg::preferred_h264_encoder(),
                )
            };
            // Software x264 at `slow`/`slower` on a 4K frame is far below realtime
            // and is what turned a ~40s export into minutes on GPU-less machines.
            // Hardware encoders aren't preset-bound this way, so cap only libx264:
            // slow→medium at the same CRF is ~2x faster for a barely-visible size
            // change. The 4K profile's default is `slow`, so this only bites the
            // pathological software-4K path.
            let chosen_x264_preset = speed.x264_preset().unwrap_or(profile.mp4_preset);
            let x264_preset = if matches!(encoder, crate::encoder::h264::H264Encoder::Libx264) {
                cap_software_x264_preset(chosen_x264_preset)
            } else {
                chosen_x264_preset
            };
            args.extend(crate::encoder::h264::codec_args(
                encoder,
                crate::encoder::h264::EncodePurpose::Export(
                    crate::encoder::h264::ExportEncodeParams {
                        nvenc_preset: speed.nvenc_preset(),
                        amf_quality: speed.amf_quality(),
                        qsv_preset: speed.qsv_preset(),
                        x264_preset,
                        cq: profile.mp4_nvenc_cq,
                        crf: profile.mp4_crf,
                    },
                ),
            ));
            if has_audio_map {
                args.extend([
                    "-c:a".to_string(),
                    "aac".to_string(),
                    "-b:a".to_string(),
                    "192k".to_string(),
                    // Pin the delivered format. Without this the output takes
                    // whichever source survived the mix, so a session with only
                    // a 16 kHz mono headset mic shipped a 16 kHz mono export.
                    "-ar".to_string(),
                    "48000".to_string(),
                    "-ac".to_string(),
                    "2".to_string(),
                ]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
    }
}

#[cfg(test)]
mod preset_cap_tests {
    use super::cap_software_x264_preset;

    #[test]
    fn caps_sub_realtime_presets_to_medium() {
        for p in ["slow", "slower", "veryslow", "placebo"] {
            assert_eq!(cap_software_x264_preset(p), "medium", "{p} should cap");
        }
    }

    #[test]
    fn leaves_fast_presets_untouched() {
        for p in ["ultrafast", "veryfast", "faster", "fast", "medium"] {
            assert_eq!(cap_software_x264_preset(p), p, "{p} should pass through");
        }
    }
}

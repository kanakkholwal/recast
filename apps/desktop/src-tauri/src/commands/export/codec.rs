//! Format-specific codec + output args for the video export. Appends the encoder
//! flags, audio codec, and output path for GIF / WebM(VP9) / MP4(H.264). Split
//! out of commands/editor.rs's `export_video`.

use std::path::Path;

use crate::commands::ffmpeg::ExportSpeed;
use crate::commands::types::{ExportProfile, GifSettings};

/// Append the codec/output tail to `args` for the requested `format`. Mirrors the
/// former inline `match request.format` in `export_video` verbatim.
pub(crate) fn append_codec_args(
    args: &mut Vec<String>,
    format: &str,
    gif_settings: &GifSettings,
    profile: &ExportProfile,
    speed: ExportSpeed,
    has_audio_map: bool,
    output_path: &Path,
) {
    match format {
        "gif" => {
            // Explicit `-c:v gif` + `-f gif` keeps FFmpeg from probing the
            // output container and falling back to an unrelated codec on
            // some Windows builds — we've seen the auto-detect path emit
            // "Could not find tag for codec none" when the filter chain
            // ends in a labelled output rather than the default sink.
            // `-vsync 0` (a.k.a. `-fps_mode passthrough`) honours the
            // exact frame timing produced by the in-graph `fps=` filter
            // instead of FFmpeg's downstream resampler nudging frames
            // around, which previously produced 0-byte GIFs when the
            // composite framerate didn't divide evenly.
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
            // libvpx-vp9 is single-threaded and uses `deadline=best` by
            // default — a combo that turned a 5-min 1080p export into a
            // 30+ min job on a dual-core laptop with the machine pinned
            // at one core. Switching on row-multithreading, letting FFmpeg
            // pick the thread count, and bumping `cpu-used` to 4 with
            // `deadline=good` gives ~4–8× faster encodes at the same CRF
            // with quality loss that's invisible to viewers. `tile-columns`
            // splits the frame for additional parallelism on multi-core
            // machines — log2(2)=1 gives 2 tile columns, a safe default
            // for 1080p+.
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
            // NOTE: we intentionally do NOT pass `-movflags +faststart` here.
            // Faststart does an in-place moov-atom rewrite at the very end of
            // the mux, and on 4K clips that rewrite can take 10–60+ seconds
            // while stdout stays silent — manifesting as a UI that's stuck in
            // the "Finalizing…" state. Desktop playback (VLC, Windows Media,
            // browsers reading from disk) works fine with moov-at-end. If we
            // later need HTTP-streamable output, add it as a separate optional
            // `-c copy -movflags +faststart` remux pass with its own progress.
            // Export-quality codec args. NVENC/AMF/QSV get hardware rate control
            // tuned for quality (not the lowlatency presets used for live
            // recording); libx264 uses the user's chosen profile preset because
            // export isn't bound by real-time pacing. See `encoder::h264`.
            args.extend(crate::encoder::h264::codec_args(
                crate::encoder::h264::H264Encoder::from_ffmpeg_name(
                    crate::ffmpeg::preferred_h264_encoder(),
                ),
                crate::encoder::h264::EncodePurpose::Export(
                    crate::encoder::h264::ExportEncodeParams {
                        nvenc_preset: speed.nvenc_preset(),
                        amf_quality: speed.amf_quality(),
                        qsv_preset: speed.qsv_preset(),
                        x264_preset: speed.x264_preset().unwrap_or(profile.mp4_preset),
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
                ]);
            } else {
                args.push("-an".to_string());
            }
            args.push(output_path.to_string_lossy().to_string());
        }
    }
}

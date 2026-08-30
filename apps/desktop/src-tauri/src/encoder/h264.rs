//! FFmpeg H.264 codec-argument construction.
//!
//! Single source of truth for *how* to invoke each H.264 encoder family.
//! Detecting which family is available lives in
//! [`crate::ffmpeg::preferred_h264_encoder`]; this module only turns a chosen
//! family plus an [`EncodePurpose`] into the `-c:v …` CLI args.
//!
//! Every path stays 8-bit 4:2:0 (`yuv420p`, or `nv12` for QSV): the editor
//! previews the raw H.264 in a WebView `<video>` element whose decoder only
//! supports up to High profile 4:2:0, so a 4:4:4 master would not play back.

use super::RecordingQuality;

/// An H.264 encoder family Recast can drive via FFmpeg.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum H264Encoder {
    /// NVIDIA NVENC.
    Nvenc,
    /// AMD AMF.
    Amf,
    /// Intel Quick Sync.
    Qsv,
    /// Apple VideoToolbox (macOS).
    VideoToolbox,
    /// libx264 software fallback — always available.
    Libx264,
}

impl H264Encoder {
    /// Map an FFmpeg codec name (as returned by
    /// [`crate::ffmpeg::preferred_h264_encoder`]) to a family. Any unrecognized
    /// name falls back to [`H264Encoder::Libx264`], the always-present software
    /// encoder — preserving the historical `_ =>` fallback arm.
    pub fn from_ffmpeg_name(name: &str) -> Self {
        match name {
            "h264_nvenc" => Self::Nvenc,
            "h264_amf" => Self::Amf,
            "h264_qsv" => Self::Qsv,
            "h264_videotoolbox" => Self::VideoToolbox,
            _ => Self::Libx264,
        }
    }

    /// The `-c:v` codec name FFmpeg expects for this family.
    pub fn ffmpeg_name(self) -> &'static str {
        match self {
            Self::Nvenc => "h264_nvenc",
            Self::Amf => "h264_amf",
            Self::Qsv => "h264_qsv",
            Self::VideoToolbox => "h264_videotoolbox",
            Self::Libx264 => "libx264",
        }
    }

    /// Whether this is a GPU/hardware encoder (vs the libx264 software path).
    /// Hardware encoders can sustain a higher quality tier during live capture
    /// without dropping frames, so `"auto"` quality defaults them up.
    pub fn is_hardware(self) -> bool {
        !matches!(self, Self::Libx264)
    }
}

/// Resolved per-family knobs for a final export. The caller (which owns the
/// export `Speed`/`QualityProfile` types) resolves them; this module only owns
/// how they map onto each encoder's args. Borrowed — no allocation.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct ExportEncodeParams<'a> {
    /// NVENC `-preset` (e.g. `p5`).
    pub nvenc_preset: &'a str,
    /// AMF `-quality` (e.g. `balanced`).
    pub amf_quality: &'a str,
    /// QSV `-preset` (e.g. `medium`).
    pub qsv_preset: &'a str,
    /// libx264 `-preset`, already resolved (speed override or profile default).
    pub x264_preset: &'a str,
    /// Constant-quality target shared by NVENC `-cq`, AMF `-qp_i/-qp_p`, and QSV
    /// `-global_quality` (the profile's `mp4_nvenc_cq`).
    pub cq: u32,
    /// libx264 `-crf` (the profile's `mp4_crf`).
    pub crf: u32,
}

/// Why we're encoding — selects the latency/quality trade-off for the args.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EncodePurpose<'a> {
    /// Live screen capture: low-latency, must sustain real time. Carries the
    /// user-facing capture quality tier.
    RealtimeCapture(RecordingQuality),
    /// Final render: quality-first (no real-time constraint), adds
    /// `-profile:v high`. Carries the resolved per-family knobs.
    Export(ExportEncodeParams<'a>),
}

/// Build the codec + rate-control args (from `-c:v` onward) for `encoder` at the
/// given [`EncodePurpose`].
///
/// Guarantees 8-bit 4:2:0 output for every family (see the module docs for why).
pub fn codec_args(encoder: H264Encoder, purpose: EncodePurpose<'_>) -> Vec<String> {
    match purpose {
        EncodePurpose::RealtimeCapture(quality) => realtime_capture_args(encoder, quality),
        EncodePurpose::Export(params) => export_args(encoder, params),
    }
}

/// Live-capture args. `Balanced` reproduces the pre-refactor recorder
/// byte-for-byte (fast preset + low-latency tune, no explicit rate control) so
/// existing recordings are unchanged; `High`/`Pristine` trade real-time
/// headroom for fidelity via an explicit near-visually-lossless quality target.
/// libx264 stays on `ultrafast` for `Balanced` so a weak, GPU-less CPU doesn't
/// drop frames during capture.
fn realtime_capture_args(encoder: H264Encoder, quality: RecordingQuality) -> Vec<String> {
    use H264Encoder::*;
    use RecordingQuality::*;
    // Args AFTER `-c:v <name>`: the codec name is emitted once below via `ffmpeg_name()`.
    let tail: &[&str] = match (encoder, quality) {
        (VideoToolbox, Balanced) => &["-realtime", "1", "-pix_fmt", "yuv420p"],
        (VideoToolbox, High) => &["-realtime", "1", "-b:v", "10M", "-pix_fmt", "yuv420p"],
        (VideoToolbox, Pristine) => &["-realtime", "1", "-b:v", "20M", "-pix_fmt", "yuv420p"],
        // NVIDIA NVENC — `cq` is constant-quality (lower = better, 0..51).
        (Nvenc, Balanced) => &["-preset", "p5", "-tune", "ll", "-pix_fmt", "yuv420p"],
        (Nvenc, High) => &[
            "-preset", "p6", "-tune", "hq", "-rc", "vbr", "-cq", "21", "-b:v", "0", "-pix_fmt",
            "yuv420p",
        ],
        (Nvenc, Pristine) => &[
            "-preset", "p7", "-tune", "hq", "-rc", "vbr", "-cq", "16", "-b:v", "0", "-pix_fmt",
            "yuv420p",
        ],
        // AMD AMF — `qp_i/qp_p` mirror the NVENC cq range.
        (Amf, Balanced) => &[
            "-quality",
            "speed",
            "-usage",
            "lowlatency",
            "-pix_fmt",
            "yuv420p",
        ],
        (Amf, High) => &[
            "-quality",
            "balanced",
            "-usage",
            "transcoding",
            "-rc",
            "cqp",
            "-qp_i",
            "21",
            "-qp_p",
            "21",
            "-pix_fmt",
            "yuv420p",
        ],
        (Amf, Pristine) => &[
            "-quality",
            "quality",
            "-usage",
            "transcoding",
            "-rc",
            "cqp",
            "-qp_i",
            "16",
            "-qp_p",
            "16",
            "-pix_fmt",
            "yuv420p",
        ],
        // Quick Sync: `global_quality` is its constant-quality knob, and QSV takes `nv12`, not `yuv420p`.
        (Qsv, Balanced) => &["-preset", "veryfast", "-pix_fmt", "nv12"],
        (Qsv, High) => &[
            "-preset",
            "medium",
            "-global_quality",
            "21",
            "-pix_fmt",
            "nv12",
        ],
        (Qsv, Pristine) => &[
            "-preset",
            "slow",
            "-global_quality",
            "16",
            "-pix_fmt",
            "nv12",
        ],
        // libx264 fallback: Balanced keeps zerolatency and ultrafast so weak CPUs don't drop; higher tiers lower CRF.
        (Libx264, Balanced) => &[
            "-preset",
            "ultrafast",
            "-tune",
            "zerolatency",
            "-pix_fmt",
            "yuv420p",
        ],
        (Libx264, High) => &["-preset", "veryfast", "-crf", "20", "-pix_fmt", "yuv420p"],
        (Libx264, Pristine) => &["-preset", "faster", "-crf", "16", "-pix_fmt", "yuv420p"],
    };
    with_codec(encoder, tail)
}

/// Prefix the codec selector (`-c:v <name>`) onto a tail of tier-specific args,
/// so the `-c:v` name is sourced once from [`H264Encoder::ffmpeg_name`] rather
/// than restated in every match arm.
fn with_codec(encoder: H264Encoder, tail: &[&str]) -> Vec<String> {
    let mut out = Vec::with_capacity(tail.len() + 2);
    out.push("-c:v".to_string());
    out.push(encoder.ffmpeg_name().to_string());
    out.extend(tail.iter().map(|s| s.to_string()));
    out
}

/// Final-export args. Quality-first (no real-time pacing): hardware encoders use
/// quality-tuned rate control at `params.cq`, libx264 uses the user's chosen
/// preset + `params.crf`. Every family adds `-profile:v high` and stays 4:2:0.
fn export_args(encoder: H264Encoder, params: ExportEncodeParams<'_>) -> Vec<String> {
    use H264Encoder::*;
    let cq = params.cq.to_string();
    let crf = params.crf.to_string();
    let mut out = vec!["-c:v".to_string(), encoder.ffmpeg_name().to_string()];
    match encoder {
        VideoToolbox => out
            .extend(["-profile:v", "high", "-pix_fmt", "yuv420p", "-b:v", "15M"].map(String::from)),
        Nvenc => out.extend(
            [
                "-preset",
                params.nvenc_preset,
                "-tune",
                "hq",
                "-rc",
                "vbr",
                "-cq",
                &cq,
                "-b:v",
                "0",
                "-profile:v",
                "high",
                "-pix_fmt",
                "yuv420p",
            ]
            .map(String::from),
        ),
        Amf => out.extend(
            [
                "-quality",
                params.amf_quality,
                "-rc",
                "cqp",
                "-qp_i",
                &cq,
                "-qp_p",
                &cq,
                "-profile:v",
                "high",
                "-pix_fmt",
                "yuv420p",
            ]
            .map(String::from),
        ),
        Qsv => out.extend(
            [
                "-preset",
                params.qsv_preset,
                "-global_quality",
                &cq,
                "-profile:v",
                "high",
                "-pix_fmt",
                "nv12",
            ]
            .map(String::from),
        ),
        Libx264 => out.extend(
            [
                "-preset",
                params.x264_preset,
                "-crf",
                &crf,
                "-pix_fmt",
                "yuv420p",
                "-threads",
                "0",
            ]
            .map(String::from),
        ),
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    mod from_ffmpeg_name {
        use super::*;

        #[test]
        fn maps_each_known_codec_name() {
            assert_eq!(
                H264Encoder::from_ffmpeg_name("h264_nvenc"),
                H264Encoder::Nvenc
            );
            assert_eq!(H264Encoder::from_ffmpeg_name("h264_amf"), H264Encoder::Amf);
            assert_eq!(H264Encoder::from_ffmpeg_name("h264_qsv"), H264Encoder::Qsv);
            assert_eq!(
                H264Encoder::from_ffmpeg_name("h264_videotoolbox"),
                H264Encoder::VideoToolbox
            );
        }

        #[test]
        fn unknown_name_falls_back_to_libx264() {
            assert_eq!(
                H264Encoder::from_ffmpeg_name("something_else"),
                H264Encoder::Libx264
            );
        }

        #[test]
        fn round_trips_through_ffmpeg_name() {
            for enc in [
                H264Encoder::Nvenc,
                H264Encoder::Amf,
                H264Encoder::Qsv,
                H264Encoder::VideoToolbox,
                H264Encoder::Libx264,
            ] {
                assert_eq!(H264Encoder::from_ffmpeg_name(enc.ffmpeg_name()), enc);
            }
        }
    }

    mod is_hardware {
        use super::*;

        #[test]
        fn libx264_is_software() {
            assert!(!H264Encoder::Libx264.is_hardware());
        }

        #[test]
        fn every_gpu_family_is_hardware() {
            for enc in [
                H264Encoder::Nvenc,
                H264Encoder::Amf,
                H264Encoder::Qsv,
                H264Encoder::VideoToolbox,
            ] {
                assert!(enc.is_hardware(), "{enc:?} should be hardware");
            }
        }
    }

    mod codec_args {
        use super::*;

        fn realtime(name: &str, quality: RecordingQuality) -> Vec<String> {
            codec_args(
                H264Encoder::from_ffmpeg_name(name),
                EncodePurpose::RealtimeCapture(quality),
            )
        }

        // Regression guard: the default tier must stay byte-identical to the pre-refactor recorder args.
        #[test]
        fn balanced_tier_reproduces_historical_args_exactly() {
            assert_eq!(
                realtime("h264_nvenc", RecordingQuality::Balanced),
                [
                    "-c:v",
                    "h264_nvenc",
                    "-preset",
                    "p5",
                    "-tune",
                    "ll",
                    "-pix_fmt",
                    "yuv420p"
                ]
            );
            assert_eq!(
                realtime("h264_amf", RecordingQuality::Balanced),
                [
                    "-c:v",
                    "h264_amf",
                    "-quality",
                    "speed",
                    "-usage",
                    "lowlatency",
                    "-pix_fmt",
                    "yuv420p"
                ]
            );
            assert_eq!(
                realtime("h264_qsv", RecordingQuality::Balanced),
                ["-c:v", "h264_qsv", "-preset", "veryfast", "-pix_fmt", "nv12"]
            );
            assert_eq!(
                realtime("libx264", RecordingQuality::Balanced),
                [
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-tune",
                    "zerolatency",
                    "-pix_fmt",
                    "yuv420p"
                ]
            );
        }

        #[test]
        fn unknown_encoder_uses_the_libx264_software_args() {
            assert_eq!(
                realtime("something_else", RecordingQuality::Balanced),
                [
                    "-c:v",
                    "libx264",
                    "-preset",
                    "ultrafast",
                    "-tune",
                    "zerolatency",
                    "-pix_fmt",
                    "yuv420p"
                ]
            );
        }

        #[test]
        fn higher_tiers_stay_420_and_add_quality_rate_control() {
            for enc in [
                "h264_videotoolbox",
                "h264_nvenc",
                "h264_amf",
                "h264_qsv",
                "libx264",
            ] {
                for q in [RecordingQuality::High, RecordingQuality::Pristine] {
                    let args = realtime(enc, q);
                    // Never emit a 4:4:4 pixel format: the editor preview can't decode it.
                    assert!(
                        !args.iter().any(|a| a.contains("444")),
                        "{enc}/{q:?} must stay 4:2:0, got {args:?}"
                    );
                    // Must carry an explicit quality target, so it is higher quality than Balanced.
                    assert!(
                        args.iter().any(|a| matches!(
                            a.as_str(),
                            "-cq" | "-qp_i" | "-global_quality" | "-crf" | "-b:v"
                        )),
                        "{enc}/{q:?} must set an explicit quality target, got {args:?}"
                    );
                }
            }
        }
    }

    mod export {
        use super::*;

        // Representative resolved knobs: the test asserts arg structure and order, not the real profile values.
        fn params() -> ExportEncodeParams<'static> {
            ExportEncodeParams {
                nvenc_preset: "p5",
                amf_quality: "balanced",
                qsv_preset: "medium",
                x264_preset: "slow",
                cq: 24,
                crf: 20,
            }
        }

        fn export(name: &str) -> Vec<String> {
            codec_args(
                H264Encoder::from_ffmpeg_name(name),
                EncodePurpose::Export(params()),
            )
        }

        // Regression guard: byte-identical to the pre-refactor inline export match, so exported files don't change.
        #[test]
        fn reproduces_historical_export_args_exactly() {
            assert_eq!(
                export("h264_videotoolbox"),
                [
                    "-c:v",
                    "h264_videotoolbox",
                    "-profile:v",
                    "high",
                    "-pix_fmt",
                    "yuv420p",
                    "-b:v",
                    "15M"
                ]
            );
            assert_eq!(
                export("h264_nvenc"),
                [
                    "-c:v",
                    "h264_nvenc",
                    "-preset",
                    "p5",
                    "-tune",
                    "hq",
                    "-rc",
                    "vbr",
                    "-cq",
                    "24",
                    "-b:v",
                    "0",
                    "-profile:v",
                    "high",
                    "-pix_fmt",
                    "yuv420p"
                ]
            );
            assert_eq!(
                export("h264_amf"),
                [
                    "-c:v",
                    "h264_amf",
                    "-quality",
                    "balanced",
                    "-rc",
                    "cqp",
                    "-qp_i",
                    "24",
                    "-qp_p",
                    "24",
                    "-profile:v",
                    "high",
                    "-pix_fmt",
                    "yuv420p"
                ]
            );
            assert_eq!(
                export("h264_qsv"),
                [
                    "-c:v",
                    "h264_qsv",
                    "-preset",
                    "medium",
                    "-global_quality",
                    "24",
                    "-profile:v",
                    "high",
                    "-pix_fmt",
                    "nv12"
                ]
            );
            assert_eq!(
                export("libx264"),
                [
                    "-c:v", "libx264", "-preset", "slow", "-crf", "20", "-pix_fmt", "yuv420p",
                    "-threads", "0"
                ]
            );
        }

        #[test]
        fn every_family_sets_profile_high_and_stays_420() {
            for name in ["h264_videotoolbox", "h264_nvenc", "h264_amf", "h264_qsv"] {
                let args = export(name);
                assert!(
                    args.windows(2).any(|w| w == ["-profile:v", "high"]),
                    "{name} export must set -profile:v high, got {args:?}"
                );
                assert!(
                    !args.iter().any(|a| a.contains("444")),
                    "{name} export must stay 4:2:0, got {args:?}"
                );
            }
        }
    }
}

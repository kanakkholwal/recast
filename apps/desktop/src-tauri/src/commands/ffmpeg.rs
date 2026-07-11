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

/// Encoder *effort* axis, orthogonal to the resolution/quality `ExportProfile`.
///
/// This picks how hard each codec works for the SAME quality target — the
/// CRF / cq values from `ExportProfile` are left untouched. It only moves the
/// preset / cpu-used knobs, trading encode time against file size and a small
/// amount of fidelity. `Balanced` reproduces the historical settings exactly,
/// so existing exports are unchanged unless the user opts into Fast/Quality.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ExportSpeed {
    Fast,
    Balanced,
    Quality,
}

impl ExportSpeed {
    pub fn from_request(speed: &str) -> Self {
        match speed {
            "fast" => Self::Fast,
            "quality" => Self::Quality,
            _ => Self::Balanced,
        }
    }

    /// NVENC `-preset` p-level (p1 fastest … p7 slowest).
    pub fn nvenc_preset(self) -> &'static str {
        match self {
            Self::Fast => "p3",
            Self::Balanced => "p5",
            Self::Quality => "p7",
        }
    }

    /// AMD AMF `-quality` mode.
    pub fn amf_quality(self) -> &'static str {
        match self {
            Self::Fast => "speed",
            Self::Balanced | Self::Quality => "quality",
        }
    }

    /// Intel QSV `-preset`.
    pub fn qsv_preset(self) -> &'static str {
        match self {
            Self::Fast => "faster",
            Self::Balanced => "slower",
            Self::Quality => "veryslow",
        }
    }

    /// libvpx-vp9 `-cpu-used` (0 best … 8 fastest).
    pub fn vp9_cpu_used(self) -> &'static str {
        match self {
            Self::Fast => "6",
            Self::Balanced => "4",
            Self::Quality => "2",
        }
    }

    /// libx264 `-preset`. `Balanced` defers to the resolution profile's own
    /// preset (returns `None`); Fast/Quality override it.
    pub fn x264_preset(self) -> Option<&'static str> {
        match self {
            Self::Fast => Some("veryfast"),
            Self::Balanced => None,
            Self::Quality => Some("slow"),
        }
    }
}

pub fn build_output_scale_filter(profile: ExportProfile) -> Option<String> {
    // libx264 + yuv420p (the chroma subsampling for our MP4 output) requires
    // even width AND even height. Without enforcement, fitting an arbitrary
    // source (e.g. 1599×962) into a preset bound (1280×720) preserves the
    // aspect ratio and produces 1195×720 — the encoder fails to open with
    // "width not divisible by 2".
    //
    // FFmpeg 4.4+ exposes `force_divisible_by=2` on the scale filter, but
    // the version of FFmpeg we bundle isn't pinned, so we use the
    // version-agnostic trick: chain a second scale with `trunc(.../2)*2`.
    // It's effectively a no-op when the previous stage produced even
    // dimensions, and snaps down by one pixel when it didn't. `neighbor`
    // sampling on the second pass keeps it cheap (no resample math) and
    // avoids any smear on the ±1px adjustment.
    match (profile.max_width, profile.max_height) {
        (Some(max_width), Some(max_height)) => Some(format!(
            "scale=w='min(iw,{max_width})':h='min(ih,{max_height})':force_original_aspect_ratio=decrease:flags=lanczos,\
             scale=w='trunc(iw/2)*2':h='trunc(ih/2)*2':flags=neighbor"
        )),
        _ => Some(
            // "source" quality has no resize, but window/region captures can
            // still be odd-dimensioned. Apply only the even-dim snap.
            "scale=w='trunc(iw/2)*2':h='trunc(ih/2)*2':flags=neighbor".to_string(),
        ),
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
    overlay_x: u32,
    overlay_y: u32,
) -> (String, String) {
    let out_label = "[vcursor]";
    let normalized_current = if current_video_map.starts_with('[') {
        current_video_map.to_string()
    } else {
        format!("[{current_video_map}]")
    };
    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!(
            "{existing};{normalized_current}[{cursor_input_index}:v]overlay={overlay_x}:{overlay_y}:format=auto{out_label}"
        ),
        _ => format!(
            "{normalized_current}[{cursor_input_index}:v]overlay={overlay_x}:{overlay_y}:format=auto{out_label}"
        ),
    };
    (new_complex, out_label.to_string())
}

/// Append an `ass` (libass) subtitle burn-in stage. `ass_path` is a local .ass
/// file; libass renders the styled captions straight into the frame. Injected
/// before the cut/speed stage so the later select/setpts re-times the burned
/// pixels along with everything else.
pub fn append_subtitles_to_complex(
    filter_complex: Option<&str>,
    current_video_map: &str,
    ass_path: &str,
    fontsdir: Option<&str>,
) -> (String, String) {
    let out_label = "[vcap]";
    let normalized_current = if current_video_map.starts_with('[') {
        current_video_map.to_string()
    } else {
        format!("[{current_video_map}]")
    };
    // Escape the filename for FFmpeg's TWO-level filtergraph parse:
    //   1. Single quotes protect the value at the filterchain level (so it isn't
    //      split on `,`/`;`/`[`).
    //   2. The drive colon must ALSO survive the filter-OPTION parse, which
    //      splits options on `:` — single quotes alone don't stop that (hence the
    //      original "No option name near '/Users/...'"). Backslash-escape it so
    //      that level unescapes `\:` back to `:` for libass.
    // Forward slashes work for libass and dodge backslash-escaping of separators.
    let escape = |p: &str| p.replace('\\', "/").replace(':', "\\:");
    let safe = escape(ass_path);
    // `fontsdir` points libass at a directory of TTFs so a preset's branded font
    // actually renders in the burn (otherwise libass falls back to a system face).
    let fonts = fontsdir
        .map(|d| format!(":fontsdir='{}'", escape(d)))
        .unwrap_or_default();
    let stage = format!("{normalized_current}ass=filename='{safe}'{fonts}{out_label}");
    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!("{existing};{stage}"),
        _ => stage,
    };
    (new_complex, out_label.to_string())
}

/// Parameters for `append_camera_overlay_to_complex`.
///
/// All pixel values are in **canvas pixels** (= source + padding × 2 with
/// any letterbox bars), matching the coordinate space of every other overlay
/// in the export filter graph.
#[derive(Debug, Clone, Copy)]
pub struct CameraOverlayParams {
    /// FFmpeg input index of the camera.mp4 stream.
    pub camera_input_index: usize,
    /// FFmpeg input index of the rounded-rect alpha mask, or `None` when
    /// the user picked the square shape (mask not needed).
    pub mask_input_index: Option<usize>,
    /// Top-left of the bubble in canvas pixels.
    pub bubble_x: u32,
    pub bubble_y: u32,
    /// Bubble dimensions in canvas pixels. Phase 1 enforces 1:1 in CSS, so
    /// width == height — but the function takes both for forward
    /// compatibility with non-square shapes.
    pub bubble_w: u32,
    pub bubble_h: u32,
    /// Horizontally flip the camera so the rendered bubble matches what the
    /// user saw in the recording-time webcam preview (Phase 1 default: on).
    pub mirror: bool,
}

/// Append a camera-overlay stage to an existing filter_complex string.
///
/// Filter chain emitted (with mask):
/// ```text
///   [N:v] hflip?, scale=W:H, format=yuva420p     → [cam_pre]
///   [M:v] format=gray                            → [cam_mask]
///   [cam_pre][cam_mask] alphamerge               → [cam_shaped]
///   [main][cam_shaped] overlay=X:Y               → [vcamera]
/// ```
/// Without a mask (square shape) the `format`/`alphamerge` stage is skipped
/// — the camera is overlaid as a flat rectangle.
pub fn append_camera_overlay_to_complex(
    filter_complex: Option<&str>,
    current_video_map: &str,
    params: &CameraOverlayParams,
) -> (String, String) {
    let out_label = "[vcamera]";
    let normalized_current = if current_video_map.starts_with('[') {
        current_video_map.to_string()
    } else {
        format!("[{current_video_map}]")
    };

    let CameraOverlayParams {
        camera_input_index: cam,
        mask_input_index,
        bubble_x: bx,
        bubble_y: by,
        bubble_w: bw,
        bubble_h: bh,
        mirror,
    } = *params;
    let hflip = if mirror { "hflip," } else { "" };

    // Use unique labels (`vcam_pre`, `vcam_mask`, `vcam_shaped`) so this
    // stage can compose with cursor / watermark / blur stages without label
    // collisions.
    let cam_chain = match mask_input_index {
        Some(mask_idx) => format!(
            "[{cam}:v]{hflip}scale={bw}:{bh},format=yuva420p[vcam_pre];\
             [{mask_idx}:v]format=gray[vcam_mask];\
             [vcam_pre][vcam_mask]alphamerge[vcam_shaped]"
        ),
        None => format!("[{cam}:v]{hflip}scale={bw}:{bh}[vcam_shaped]"),
    };

    let overlay =
        format!("{normalized_current}[vcam_shaped]overlay={bx}:{by}:format=auto{out_label}");

    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!("{existing};{cam_chain};{overlay}"),
        _ => format!("{cam_chain};{overlay}"),
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
/// Per-export GIF tuning passed in from the editor UI. Mirrors `GifSettings`
/// on the JS side but expressed as primitive Rust types so the filter builder
/// stays free of `serde_json::Value` parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GifFilterOptions<'a> {
    /// Output frame rate. Caller resolves overrides vs. quality profile defaults.
    pub fps: u32,
    /// 1..=256. Capped to GIF's maximum palette size.
    pub max_colors: u32,
    /// "bayer" | "sierra2" | "none".
    pub dither: &'a str,
}

impl<'a> Default for GifFilterOptions<'a> {
    fn default() -> Self {
        Self {
            fps: 15,
            max_colors: 128,
            dither: "bayer",
        }
    }
}

/// Pass-2 chain for the 2-pass GIF export. Pass 1 ran palettegen separately
/// and wrote `palette.png`; the caller wires that file in as a regular FFmpeg
/// input at `palette_input_index`, and this builder emits a paletteuse-only
/// stage referencing it.
///
/// Single-pass `palettegen→paletteuse` was stalling the UI: palettegen has to
/// consume every input frame before emitting its one palette frame, so the
/// encoder's `out_time_us` stays at 0 for the entire palette phase and the
/// progress bar never moved off 0%. With the palette pre-baked, paletteuse is
/// a per-frame lookup — frames stream out in real time and progress advances.
pub fn build_gif_paletteuse_external_complex(
    filter_complex: Option<&str>,
    input_label: &str,
    palette_input_index: usize,
    options: GifFilterOptions<'_>,
    inline_scale: Option<&str>,
) -> (String, String) {
    let final_label = "[vgif]";
    let normalized_input = if input_label.starts_with('[') {
        input_label.to_string()
    } else {
        format!("[{input_label}]")
    };
    let scale_clause = match inline_scale {
        Some(s) if !s.is_empty() => format!(",{s}"),
        _ => String::new(),
    };
    let dither_clause = match options.dither {
        "none" => "dither=none".to_string(),
        "sierra2" => "dither=sierra2".to_string(),
        _ => "dither=bayer:bayer_scale=5".to_string(),
    };
    let fps = options.fps.max(1);
    // Pin input chain to GIF fps + output scale, then pair with the pre-baked
    // palette via paletteuse. Two filter stages because paletteuse takes two
    // input pads and we need a labelled intermediate to feed it.
    let chain = format!(
        "{normalized_input}fps={fps}{scale_clause}[_gifv];[_gifv][{palette_input_index}:v]paletteuse={dither_clause}:diff_mode=rectangle{final_label}"
    );
    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!("{existing};{chain}"),
        _ => chain,
    };
    (new_complex, final_label.to_string())
}

/// Build the `-vf` filter for the GIF palette pre-pass (pass 1 of the 2-pass
/// GIF export). Drives a standalone FFmpeg invocation that consumes the source
/// at the GIF target fps + scale and writes the resulting palette to a single
/// PNG. Kept separate from the pipeline filter_complex because the pre-pass
/// only needs a flat `-vf` chain — no overlay inputs, no labelled pads.
pub fn build_gif_palette_prepass_filter(
    options: GifFilterOptions<'_>,
    inline_scale: Option<&str>,
) -> String {
    let scale_clause = match inline_scale {
        Some(s) if !s.is_empty() => format!(",{s}"),
        _ => String::new(),
    };
    let max_colors = options.max_colors.clamp(2, 256);
    let fps = options.fps.max(1);
    format!("fps={fps}{scale_clause},palettegen=max_colors={max_colors}:stats_mode=diff")
}

#[cfg(test)]
mod gif_tests {
    use super::*;

    //  pre-pass `-vf` builder (pass 1: source → palette PNG)

    #[test]
    fn prepass_filter_includes_fps_and_palettegen() {
        let vf = build_gif_palette_prepass_filter(
            GifFilterOptions {
                fps: 12,
                max_colors: 128,
                dither: "bayer",
            },
            None,
        );
        assert!(vf.starts_with("fps=12"), "got: {vf}");
        assert!(vf.contains("palettegen"));
        assert!(vf.contains("max_colors=128"));
        assert!(vf.contains("stats_mode=diff"));
    }

    #[test]
    fn prepass_filter_bakes_scale_before_palettegen() {
        let vf = build_gif_palette_prepass_filter(
            GifFilterOptions {
                fps: 18,
                max_colors: 256,
                dither: "bayer",
            },
            Some("scale=w=720:h=-1"),
        );
        let scale_idx = vf.find("scale=").expect("scale present");
        let pg_idx = vf.find("palettegen").expect("palettegen present");
        assert!(
            scale_idx < pg_idx,
            "scale must come before palettegen: {vf}"
        );
    }

    #[test]
    fn prepass_filter_clamps_max_colors_and_fps() {
        let vf = build_gif_palette_prepass_filter(
            GifFilterOptions {
                fps: 0,
                max_colors: 9999,
                dither: "bayer",
            },
            None,
        );
        assert!(vf.contains("fps=1"), "got: {vf}");
        assert!(vf.contains("max_colors=256"), "got: {vf}");

        let vf = build_gif_palette_prepass_filter(
            GifFilterOptions {
                fps: 15,
                max_colors: 1,
                dither: "bayer",
            },
            None,
        );
        assert!(vf.contains("max_colors=2"), "got: {vf}");
    }

    //  pass-2 paletteuse-only chain (palette wired in as external input)

    #[test]
    fn paletteuse_chain_references_palette_input_index() {
        let (complex, label) = build_gif_paletteuse_external_complex(
            None,
            "vout",
            3,
            GifFilterOptions {
                fps: 12,
                max_colors: 128,
                dither: "bayer",
            },
            None,
        );
        assert_eq!(label, "[vgif]");
        assert!(complex.starts_with("[vout]fps=12"), "got: {complex}");
        assert!(complex.contains("[3:v]paletteuse"), "got: {complex}");
        assert!(complex.contains("dither=bayer:bayer_scale=5"));
        assert!(complex.contains("[vgif]"));
        assert!(
            !complex.contains("palettegen"),
            "pass 2 must not regenerate palette: {complex}"
        );
    }

    #[test]
    fn paletteuse_chain_appends_to_existing_filter_complex() {
        let (complex, _) = build_gif_paletteuse_external_complex(
            Some("[0:v]hflip[vout]"),
            "[vout]",
            5,
            GifFilterOptions::default(),
            None,
        );
        assert!(complex.starts_with("[0:v]hflip[vout];"));
        assert!(complex.contains("[vout]fps=15"));
        assert!(complex.contains("[5:v]paletteuse"));
    }

    #[test]
    fn paletteuse_chain_bakes_inline_scale_before_paletteuse() {
        let (complex, _) = build_gif_paletteuse_external_complex(
            None,
            "vout",
            2,
            GifFilterOptions {
                fps: 18,
                max_colors: 256,
                dither: "bayer",
            },
            Some("scale=w=720:h=-1"),
        );
        let scale_idx = complex.find("scale=").expect("scale present");
        let pu_idx = complex.find("paletteuse").expect("paletteuse present");
        assert!(
            scale_idx < pu_idx,
            "scale must come before paletteuse: {complex}"
        );
    }

    #[test]
    fn paletteuse_chain_sierra2_emits_bare_dither_arg() {
        let (complex, _) = build_gif_paletteuse_external_complex(
            None,
            "vout",
            1,
            GifFilterOptions {
                fps: 15,
                max_colors: 128,
                dither: "sierra2",
            },
            None,
        );
        assert!(complex.contains("dither=sierra2"), "got: {complex}");
        assert!(!complex.contains("bayer_scale"));
    }

    #[test]
    fn paletteuse_chain_dither_none_disables_dither() {
        let (complex, _) = build_gif_paletteuse_external_complex(
            None,
            "vout",
            1,
            GifFilterOptions {
                fps: 15,
                max_colors: 128,
                dither: "none",
            },
            None,
        );
        assert!(complex.contains("dither=none"));
    }

    #[test]
    fn paletteuse_chain_unknown_dither_falls_back_to_bayer() {
        let (complex, _) = build_gif_paletteuse_external_complex(
            None,
            "vout",
            1,
            GifFilterOptions {
                fps: 15,
                max_colors: 128,
                dither: "wat",
            },
            None,
        );
        assert!(complex.contains("dither=bayer:bayer_scale=5"));
    }

    #[test]
    fn paletteuse_chain_fps_zero_clamps_to_one() {
        let (complex, _) = build_gif_paletteuse_external_complex(
            None,
            "vout",
            1,
            GifFilterOptions {
                fps: 0,
                max_colors: 128,
                dither: "bayer",
            },
            None,
        );
        assert!(complex.contains("fps=1"), "got: {complex}");
    }
}

#[cfg(test)]
mod blur_tests {
    use super::*;

    fn region_with(variant: &'static str, start: f64, end: f64) -> BlurRegion<'static> {
        BlurRegion {
            x: 100,
            y: 80,
            w: 320,
            h: 180,
            radius: 12,
            start_secs: start,
            end_secs: end,
            variant,
            tint_rgb: 0xff00aa,
            opacity: 1.0,
            strength: 1.0,
            corner_px: 0.0,
        }
    }

    #[test]
    fn rounded_blur_adds_a_geq_alpha_mask() {
        let regs = [BlurRegion {
            corner_px: 20.0,
            ..region_with("glass", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "0:v", &regs);
        assert!(
            chain.contains("format=yuva420p") && chain.contains("geq="),
            "rounded blur should add an alpha mask: {chain}"
        );
    }

    #[test]
    fn square_blur_has_no_geq_mask() {
        let regs = [region_with("glass", 0.0, 1.0)]; // corner_px = 0.0
        let (chain, _) = build_annotation_blur_complex(None, "0:v", &regs);
        assert!(
            !chain.contains("geq="),
            "square blur should not add a geq mask: {chain}"
        );
    }

    #[test]
    fn empty_regions_returns_input_unchanged() {
        let (chain, label) = build_annotation_blur_complex(Some("[0:v]hflip[v]"), "[v]", &[]);
        assert_eq!(chain, "[0:v]hflip[v]");
        assert_eq!(label, "[v]");
    }

    #[test]
    fn single_region_emits_split_crop_overlay() {
        // strength low enough to skip the high-strength glass redaction wash.
        let regs = [BlurRegion {
            strength: 0.4,
            ..region_with("glass", 1.0, 3.5)
        }];
        let (chain, label) = build_annotation_blur_complex(None, "vout", &regs);
        // Split appears first to fork main/source streams.
        assert!(
            chain.contains("split[blur_main_0][blur_src_0]"),
            "chain: {chain}"
        );
        // Crop dimensions are baked from the region rect.
        assert!(chain.contains("crop=320:180:100:80"));
        // Box blur radius matches the input.
        assert!(chain.contains("boxblur=luma_radius=12"));
        // Glass variant has no drawbox tint.
        assert!(!chain.contains("drawbox"));
        // Overlay is gated by the enable window with the right times.
        assert!(chain.contains("enable='between(t\\,1.0000\\,3.5000)'"));
        assert_eq!(label, "[vblur]");
    }

    #[test]
    fn white_and_black_variants_emit_drawbox() {
        // strength=1.0 (test default) → base_alpha = 0.95.
        for (variant, expected_color) in &[("white", "white@0.950"), ("black", "black@0.950")] {
            let regs = [region_with(variant, 0.0, 2.0)];
            let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
            assert!(chain.contains("drawbox"), "missing drawbox for {variant}");
            assert!(
                chain.contains(expected_color),
                "{variant} should embed {expected_color} got: {chain}"
            );
        }
    }

    #[test]
    fn color_variant_emits_hex_drawbox() {
        let regs = [BlurRegion {
            tint_rgb: 0x3b82f6,
            ..region_with("color", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(chain.contains("0x3b82f6@"), "chain: {chain}");
    }

    #[test]
    fn opacity_scales_drawbox_alpha() {
        let regs = [BlurRegion {
            opacity: 0.5,
            ..region_with("white", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        // strength=1.0 (default) → 0.95 base; opacity 0.5 → 0.475.
        assert!(chain.contains("white@0.475"), "chain: {chain}");
    }

    #[test]
    fn unknown_variant_treated_as_glass() {
        // strength low enough that the glass redaction wash doesn't kick in.
        let regs = [BlurRegion {
            strength: 0.4,
            ..region_with("alien", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(!chain.contains("drawbox"), "chain: {chain}");
    }

    #[test]
    fn radius_zero_clamps_to_one() {
        let regs = [BlurRegion {
            radius: 0,
            ..region_with("glass", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(chain.contains("boxblur=luma_radius=1"));
    }

    #[test]
    fn radius_huge_clamps_to_127() {
        // Large region so boxblur's hard ceiling (127) — not the region size —
        // is the binding limit.
        let regs = [BlurRegion {
            radius: 9999,
            w: 1024,
            h: 720,
            ..region_with("glass", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(chain.contains("boxblur=luma_radius=127"));
    }

    /// Regression for the "Invalid luma_param radius value 84 ... must be <= 81"
    /// export crash: a radius larger than the (small) region's plane must clamp
    /// down, per-plane — chroma is half-size under 4:2:0 so it caps even lower.
    #[test]
    fn radius_clamps_to_region_plane_size() {
        let regs = [BlurRegion {
            radius: 84,
            w: 164,
            h: 164,
            ..region_with("glass", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        // luma plane 164 → max (164-1)/2 = 81; chroma plane 82 → (82-1)/2 = 40.
        assert!(chain.contains("boxblur=luma_radius=81:"), "chain: {chain}");
        assert!(chain.contains("chroma_radius=40:"), "chain: {chain}");
    }

    /// First integer following `key` in `chain` (e.g. "luma_radius=").
    fn radius_after(chain: &str, key: &str) -> i32 {
        let start = chain
            .find(key)
            .unwrap_or_else(|| panic!("{key} missing: {chain}"))
            + key.len();
        let rest = &chain[start..];
        let end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        rest[..end].parse().expect("digits")
    }

    /// Combinatorial guard (the dynamic version of the regression above): across
    /// region sizes × requested radii × variants, EVERY emitted boxblur radius
    /// must satisfy FFmpeg's per-plane constraint `2*r + 1 <= plane_dim` — chroma
    /// is half-size under 4:2:0. No combination may produce an invalid filter.
    #[test]
    fn blur_radius_valid_for_every_region_size_and_strength() {
        let sizes = [
            (4, 4),
            (8, 8),
            (64, 40),
            (164, 164),
            (320, 180),
            (1920, 1080),
        ];
        let radii = [0u32, 1, 12, 40, 84, 127, 9999];
        let variants = ["glass", "white", "black", "color", "unknown"];
        let mut cases = 0;
        for &(w, h) in &sizes {
            for &r in &radii {
                for v in variants {
                    let regs = [BlurRegion {
                        w,
                        h,
                        radius: r,
                        ..region_with(v, 0.0, 1.0)
                    }];
                    let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
                    let lr = radius_after(&chain, "luma_radius=");
                    let cr = radius_after(&chain, "chroma_radius=");
                    assert!(
                        2 * lr + 1 <= w.min(h),
                        "luma radius {lr} invalid for {w}x{h} (r={r}, {v}): {chain}"
                    );
                    assert!(
                        2 * cr + 1 <= (w / 2).min(h / 2),
                        "chroma radius {cr} invalid for {w}x{h} (r={r}, {v}): {chain}"
                    );
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, sizes.len() * radii.len() * variants.len());
    }

    #[test]
    fn multiple_regions_chain_through_intermediate_labels() {
        let regs = [
            region_with("glass", 0.0, 2.0),
            region_with("white", 2.0, 4.0),
            region_with("color", 4.0, 6.0),
        ];
        let (chain, label) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(
            chain.contains("[blur_step_0]"),
            "first step label missing: {chain}"
        );
        assert!(
            chain.contains("[blur_step_1]"),
            "second step label missing: {chain}"
        );
        // Last region's overlay output is the final label.
        assert_eq!(label, "[vblur]");
        assert!(chain.contains("[vblur]"));
        // All three enable windows are present.
        assert!(chain.contains("0.0000\\,2.0000"));
        assert!(chain.contains("2.0000\\,4.0000"));
        assert!(chain.contains("4.0000\\,6.0000"));
    }

    #[test]
    fn appends_to_existing_filter_complex() {
        let regs = [region_with("glass", 0.0, 1.0)];
        let (chain, _) = build_annotation_blur_complex(Some("[0:v]hflip[v]"), "[v]", &regs);
        assert!(chain.starts_with("[0:v]hflip[v];"), "chain: {chain}");
    }

    #[test]
    fn end_clamped_above_start() {
        // Pathological project state: end < start. Filter should still emit
        // a valid enable expression with end = start (so no exception, just
        // a zero-length window).
        let regs = [region_with("glass", 5.0, 1.0)];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(
            chain.contains("between(t\\,5.0000\\,5.0000)"),
            "chain: {chain}"
        );
    }

    #[test]
    fn negative_coords_clamped_to_zero() {
        let regs = [BlurRegion {
            x: -50,
            y: -10,
            ..region_with("glass", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(chain.contains("crop=320:180:0:0"));
        assert!(chain.contains("overlay=x=0:y=0"));
    }
}

#[cfg(test)]
mod export_profile_tests {
    //! Combinatorial coverage of the export-option axes that drive codec
    //! settings: `quality` (resolution/CRF profile) and `speed` (encoder
    //! effort). Each value — including unknown ones, which must fall back —
    //! produces a sane, in-range config and a well-formed scale filter.
    use super::*;

    const QUALITIES: &[&str] = &["small", "hd", "4k", "source", "weird-unknown"];
    const SPEEDS: &[&str] = &["fast", "balanced", "quality", "nonsense"];

    #[test]
    fn every_quality_profile_is_sane() {
        for &quality in QUALITIES {
            let p = resolve_export_profile(quality);
            // CRF/CQ within each codec's accepted range.
            assert!(
                (1..=51).contains(&p.mp4_crf),
                "{quality} mp4_crf={}",
                p.mp4_crf
            );
            assert!(
                (1..=51).contains(&p.mp4_nvenc_cq),
                "{quality} cq={}",
                p.mp4_nvenc_cq
            );
            assert!(
                (1..=63).contains(&p.webm_crf),
                "{quality} webm_crf={}",
                p.webm_crf
            );
            assert!(
                p.gif_fps > 0 && p.gif_fps <= 50,
                "{quality} gif_fps={}",
                p.gif_fps
            );
            // Resolution bounds are present-or-absent together; if present, sane.
            assert_eq!(
                p.max_width.is_some(),
                p.max_height.is_some(),
                "{quality}: width/height bound parity"
            );
            if let (Some(w), Some(h)) = (p.max_width, p.max_height) {
                assert!(w >= 320 && h >= 240, "{quality}: bounds {w}x{h} too small");
            }
            assert!(!p.mp4_preset.is_empty(), "{quality}: empty x264 preset");
            // The output scale filter always ends with the even-dimension snap
            // (libx264 + yuv420p needs even w/h); bounded profiles additionally
            // fit-within their max box. The unbounded "source" profile omits the
            // fit step — that's correct, not a bug.
            let bounded = p.max_width.is_some();
            if let Some(f) = build_output_scale_filter(p) {
                assert!(
                    f.contains("trunc(iw/2)*2"),
                    "{quality}: missing even snap: {f}"
                );
                if bounded {
                    assert!(
                        f.contains("force_original_aspect_ratio"),
                        "{quality}: bounded profile missing fit step: {f}"
                    );
                }
                assert_eq!(
                    f.matches('(').count(),
                    f.matches(')').count(),
                    "{quality}: {f}"
                );
            }
        }
    }

    #[test]
    fn every_speed_maps_to_valid_codec_presets() {
        for &speed in SPEEDS {
            let s = ExportSpeed::from_request(speed);
            // NVENC preset is p1..p7.
            let nv = s.nvenc_preset();
            let level = nv.strip_prefix('p').and_then(|n| n.parse::<u8>().ok());
            assert!(
                matches!(level, Some(1..=7)),
                "{speed}: bad nvenc preset {nv}"
            );
            // VP9 cpu-used is 0..8.
            let vp9 = s.vp9_cpu_used().parse::<u8>().ok();
            assert!(matches!(vp9, Some(0..=8)), "{speed}: bad vp9 cpu-used");
            assert!(!s.amf_quality().is_empty() && !s.qsv_preset().is_empty());
            // x264: Balanced defers to the profile preset (None); others override.
            match s {
                ExportSpeed::Balanced => assert!(s.x264_preset().is_none()),
                _ => assert!(s.x264_preset().is_some()),
            }
        }
    }

    #[test]
    fn unknown_values_fall_back_to_balanced_and_hd() {
        assert_eq!(ExportSpeed::from_request("???"), ExportSpeed::Balanced);
        let hd = resolve_export_profile("hd");
        let unknown = resolve_export_profile("???");
        assert_eq!(hd.mp4_crf, unknown.mp4_crf);
        assert_eq!(hd.max_width, unknown.max_width);
        assert_eq!(hd.gif_fps, unknown.gif_fps);
    }
}

#[cfg(test)]
mod export_retention_tests {
    //! End-to-end-style tests: verify that an `Annotation` carrying a `Blur`
    //! kind survives the full pipeline from JSON → `RenderState` → filter
    //! chain assembly, with the right region geometry preserved at every
    //! step. Mirrors what `export_video` does on the live path, without
    //! actually invoking ffmpeg (so the test stays hermetic and fast).
    use super::*;
    use crate::render::graph::RenderState;
    use crate::render::node_types::AnnotationKind;

    fn build_render_state_json(annotations_json: &str) -> RenderState {
        let json = format!(
            r##"{{
                "trimStart": 0.0,
                "trimEnd": 10.0,
                "backgroundType": "color",
                "backgroundValue": "#000",
                "backgroundBlur": 0.0,
                "padding": 0.0,
                "borderRadius": 0.0,
                "cursorEnabled": false,
                "cursorSize": 1.0,
                "cursorSmoothing": 0.0,
                "cursorHighlightClicks": false,
                "cursorHighlightColor": "#3b82f6",
                "cursorHighlightOpacity": 0.0,
                "cursorHideWhenIdle": false,
                "cursorIdleTimeout": 0.0,
                "zoomRegions": [],
                "annotations": {annotations_json}
            }}"##
        );
        serde_json::from_str(&json).expect("RenderState parses")
    }

    fn make_blur_region<'a>(
        annos: &'a [crate::render::node_types::Annotation],
        canvas_w: u32,
        canvas_h: u32,
        trim_start: f64,
    ) -> Vec<BlurRegion<'a>> {
        annos
            .iter()
            .filter(|a| !a.hidden)
            .filter_map(|a| match &a.kind {
                AnnotationKind::Blur {
                    x,
                    y,
                    w,
                    h,
                    strength,
                    variant,
                    tint_color,
                    ..
                } => {
                    let cx = (x * canvas_w as f64).round() as i32;
                    let cy = (y * canvas_h as f64).round() as i32;
                    let cw = (w.abs() * canvas_w as f64).round() as i32;
                    let ch = (h.abs() * canvas_h as f64).round() as i32;
                    if cw < 4 || ch < 4 {
                        return None;
                    }
                    // 12% of the short edge gives a redaction-grade cap:
                    // 1080p → ~130, clamped to FFmpeg boxblur's hard max of
                    // 127. The previous 5% cap left text readable.
                    let max_dim = canvas_w.min(canvas_h) as f64 * 0.12;
                    let radius = (strength.clamp(0.0, 1.0) * max_dim)
                        .round()
                        .clamp(1.0, 127.0) as u32;
                    let tint_rgb =
                        u32::from_str_radix(tint_color.trim_start_matches('#'), 16).unwrap_or(0);
                    Some(BlurRegion {
                        x: cx,
                        y: cy,
                        w: cw,
                        h: ch,
                        radius,
                        start_secs: a.start - trim_start,
                        end_secs: a.end - trim_start,
                        variant: variant.as_str(),
                        tint_rgb,
                        opacity: a.opacity.clamp(0.0, 1.0),
                        strength: strength.clamp(0.0, 1.0),
                        corner_px: 0.0,
                    })
                }
                _ => None,
            })
            .collect()
    }

    #[test]
    fn blur_annotation_round_trips_into_filter_chain() {
        let annotations = r##"[{
            "id": "blur-a",
            "start": 1.0,
            "end": 4.0,
            "rampIn": 0.0,
            "rampOut": 0.0,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "kind": {
                "kind": "blur",
                "x": 0.25, "y": 0.25, "w": 0.5, "h": 0.5,
                "strength": 1.0,
                "variant": "white",
                "tintColor": "#ffffff",
                "radius": 0.0
            }
        }]"##;

        let render_state = build_render_state_json(annotations);
        let regions = make_blur_region(&render_state.annotations, 1920, 1080, 0.0);

        // Region survives JSON → struct → filter region with correct geometry.
        assert_eq!(regions.len(), 1);
        let r = &regions[0];
        assert_eq!(r.x, 480, "0.25 * 1920");
        assert_eq!(r.y, 270, "0.25 * 1080");
        assert_eq!(r.w, 960);
        assert_eq!(r.h, 540);
        // strength=1.0 + 1080 short edge → 12% = 129.6 → clamped to 127.
        assert!(r.radius == 127, "radius={}", r.radius);
        assert!((r.start_secs - 1.0).abs() < 1e-9);
        assert!((r.end_secs - 4.0).abs() < 1e-9);

        let (chain, label) = build_annotation_blur_complex(None, "vmain", &regions);
        assert_eq!(label, "[vblur]");
        assert!(chain.contains("crop=960:540:480:270"), "chain: {chain}");
        assert!(chain.contains("white@"), "white tint missing");
        assert!(chain.contains("between(t\\,1.0000\\,4.0000)"));
    }

    #[test]
    fn hidden_blur_annotations_are_skipped_at_export() {
        let annotations = r##"[{
            "id": "blur-hidden",
            "start": 0.0, "end": 2.0,
            "rampIn": 0.0, "rampOut": 0.0,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "hidden": true,
            "kind": {
                "kind": "blur",
                "x": 0.0, "y": 0.0, "w": 0.5, "h": 0.5,
                "strength": 0.5,
                "variant": "glass",
                "tintColor": "#000000",
                "radius": 0.0
            }
        }]"##;
        let render_state = build_render_state_json(annotations);
        let regions = make_blur_region(&render_state.annotations, 1920, 1080, 0.0);
        assert!(
            regions.is_empty(),
            "hidden annotations must not generate filter regions"
        );
    }

    #[test]
    fn trim_start_is_subtracted_from_blur_window() {
        let annotations = r##"[{
            "id": "b",
            "start": 5.0, "end": 7.0,
            "rampIn": 0.0, "rampOut": 0.0,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "kind": {
                "kind": "blur",
                "x": 0.0, "y": 0.0, "w": 0.5, "h": 0.5,
                "strength": 0.2,
                "variant": "glass",
                "tintColor": "#000000",
                "radius": 0.0
            }
        }]"##;
        let render_state = build_render_state_json(annotations);
        let regions = make_blur_region(&render_state.annotations, 1280, 720, 3.0);
        // Project start=5, trim_start=3 → output window starts at 2s.
        assert_eq!(regions.len(), 1);
        assert!((regions[0].start_secs - 2.0).abs() < 1e-9);
        assert!((regions[0].end_secs - 4.0).abs() < 1e-9);
    }

    #[test]
    fn microscopic_blur_regions_are_dropped() {
        // 0.001 of a 1920px canvas = ~2px → below the 4px floor.
        let annotations = r##"[{
            "id": "tiny",
            "start": 0.0, "end": 1.0,
            "rampIn": 0.0, "rampOut": 0.0,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "kind": {
                "kind": "blur",
                "x": 0.0, "y": 0.0, "w": 0.001, "h": 0.5,
                "strength": 0.5,
                "variant": "glass",
                "tintColor": "#000000",
                "radius": 0.0
            }
        }]"##;
        let render_state = build_render_state_json(annotations);
        let regions = make_blur_region(&render_state.annotations, 1920, 1080, 0.0);
        assert!(regions.is_empty(), "sub-4px region should be filtered");
    }

    #[test]
    fn mixed_annotations_only_blur_kinds_become_filter_regions() {
        let annotations = r##"[
            {
                "id": "rect-1",
                "start": 0.0, "end": 1.0,
                "rampIn": 0.0, "rampOut": 0.0,
                "stroke": { "color": "transparent", "width": 0 },
                "fill": "transparent",
                "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "radius": 0.0 }
            },
            {
                "id": "blur-1",
                "start": 0.5, "end": 2.0,
                "rampIn": 0.0, "rampOut": 0.0,
                "stroke": { "color": "transparent", "width": 0 },
                "fill": "transparent",
                "kind": {
                    "kind": "blur",
                    "x": 0.3, "y": 0.3, "w": 0.3, "h": 0.3,
                    "strength": 0.5,
                    "variant": "color",
                    "tintColor": "#3b82f6",
                    "radius": 0.0
                }
            },
            {
                "id": "ellipse-1",
                "start": 0.0, "end": 1.0,
                "rampIn": 0.0, "rampOut": 0.0,
                "stroke": { "color": "transparent", "width": 0 },
                "fill": "transparent",
                "kind": { "kind": "ellipse", "x": 0.5, "y": 0.5, "w": 0.2, "h": 0.2 }
            }
        ]"##;
        let render_state = build_render_state_json(annotations);
        // Three annotations parsed.
        assert_eq!(render_state.annotations.len(), 3);
        // Only one becomes a blur filter region.
        let regions = make_blur_region(&render_state.annotations, 1920, 1080, 0.0);
        assert_eq!(regions.len(), 1);
        assert_eq!(regions[0].variant, "color");
        assert_eq!(regions[0].tint_rgb, 0x3b82f6);
    }
}

#[cfg(test)]
mod blur_serde_tests {
    use crate::render::node_types::{Annotation, AnnotationKind};

    #[test]
    fn blur_kind_round_trips_through_json() {
        let json = r##"{
            "id": "blur-1",
            "start": 1.0,
            "end": 3.0,
            "rampIn": 0.2,
            "rampOut": 0.2,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "kind": {
                "kind": "blur",
                "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.25,
                "strength": 0.7,
                "variant": "white",
                "tintColor": "#3b82f6",
                "radius": 0.04
            }
        }"##;
        let parsed: Annotation = serde_json::from_str(json).expect("blur parses");
        match parsed.kind {
            AnnotationKind::Blur {
                x,
                y,
                w,
                h,
                strength,
                variant,
                tint_color,
                radius,
            } => {
                assert!((x - 0.1).abs() < 1e-9);
                assert!((y - 0.2).abs() < 1e-9);
                assert!((w - 0.3).abs() < 1e-9);
                assert!((h - 0.25).abs() < 1e-9);
                assert!((strength - 0.7).abs() < 1e-9);
                assert_eq!(variant, "white");
                assert_eq!(tint_color, "#3b82f6");
                assert!((radius - 0.04).abs() < 1e-9);
            }
            other => panic!("expected Blur, got {other:?}"),
        }
    }

    #[test]
    fn blur_uses_defaults_when_fields_missing() {
        let json = r##"{
            "id": "blur-2",
            "start": 0.0,
            "end": 1.0,
            "rampIn": 0.2,
            "rampOut": 0.2,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "kind": { "kind": "blur", "x": 0.0, "y": 0.0, "w": 0.5, "h": 0.5 }
        }"##;
        let parsed: Annotation = serde_json::from_str(json).expect("blur parses with defaults");
        match parsed.kind {
            AnnotationKind::Blur {
                strength,
                variant,
                tint_color,
                radius,
                ..
            } => {
                assert!((strength - 0.5).abs() < 1e-9);
                assert_eq!(variant, "glass");
                assert_eq!(tint_color, "#000000");
                assert!((radius - 0.0).abs() < 1e-9);
            }
            _ => panic!("expected Blur"),
        }
    }

    #[test]
    fn unknown_kind_falls_back_to_unsupported_not_blur() {
        let json = r##"{
            "id": "x",
            "start": 0.0, "end": 1.0,
            "rampIn": 0.2, "rampOut": 0.2,
            "stroke": { "color": "transparent", "width": 0 },
            "fill": "transparent",
            "kind": { "kind": "totally-fake" }
        }"##;
        let parsed: Annotation = serde_json::from_str(json).expect("parses");
        assert!(matches!(parsed.kind, AnnotationKind::Unsupported));
    }
}

#[cfg(test)]
mod gif_settings_tests {
    use super::super::types::GifSettings;
    use serde_json::json;

    #[test]
    fn loop_infinite_to_zero() {
        let s = GifSettings {
            fps: None,
            quality: "medium".into(),
            r#loop: json!("infinite"),
            dither: "bayer".into(),
        };
        assert_eq!(s.ffmpeg_loop_arg(), 0);
    }

    #[test]
    fn loop_once_to_minus_one() {
        let s = GifSettings {
            fps: None,
            quality: "medium".into(),
            r#loop: json!("once"),
            dither: "bayer".into(),
        };
        assert_eq!(s.ffmpeg_loop_arg(), -1);
    }

    #[test]
    fn loop_numeric_passthrough() {
        let s = GifSettings {
            fps: None,
            quality: "medium".into(),
            r#loop: json!(3),
            dither: "bayer".into(),
        };
        assert_eq!(s.ffmpeg_loop_arg(), 3);
    }

    #[test]
    fn loop_negative_clamped_to_minus_one() {
        let s = GifSettings {
            fps: None,
            quality: "medium".into(),
            r#loop: json!(-5),
            dither: "bayer".into(),
        };
        assert_eq!(s.ffmpeg_loop_arg(), -1);
    }

    #[test]
    fn quality_to_max_colors() {
        let mut s = GifSettings::default();
        s.quality = "low".into();
        assert_eq!(s.max_colors(), 64);
        s.quality = "medium".into();
        assert_eq!(s.max_colors(), 128);
        s.quality = "high".into();
        assert_eq!(s.max_colors(), 256);
        s.quality = "garbage".into();
        assert_eq!(s.max_colors(), 128);
    }
}

/// One blur region as understood by the FFmpeg filter graph builder.
/// All coordinates are in source-video pixels (not UV) — the caller
/// (`build_annotation_blur_complex`) maps from the annotation's UV rect.
#[derive(Debug, Clone, PartialEq)]
pub struct BlurRegion<'a> {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    /// Box-blur kernel radius in pixels (1..=64).
    pub radius: u32,
    /// Timeline-time window when the blur is visible.
    pub start_secs: f64,
    pub end_secs: f64,
    /// "glass" | "white" | "black" | "color".
    pub variant: &'a str,
    /// 0xRRGGBB packed; only consulted when `variant == "color"`.
    pub tint_rgb: u32,
    /// 0..=1 master opacity baked into the colour overlay.
    pub opacity: f64,
    /// 0..=1 — the original blur strength. The tint pass scales its alpha
    /// by this so high strength → near-opaque box (true redaction). The
    /// preview applies the same scaling.
    pub strength: f64,
    /// Corner radius in pixels (0 = square). Rounds the blurred region via a
    /// per-pixel alpha mask so the corners show the sharp video, matching the
    /// preview. 0 keeps the cheap rectangular path.
    pub corner_px: f64,
}

/// A `,format=yuva420p,geq=…` snippet that sets the alpha plane to a
/// rounded-rect mask (opaque inside, transparent in the cut corners of radius
/// `r` px) while copying the colour planes. `ox`/`oy` measure how far a pixel
/// lies outside the inner rect (the box shrunk by `r`); if that offset is past
/// the corner arc the pixel is transparent. Commas are escaped `\,` like the
/// `enable=` expression so the filtergraph parser hands geq clean args.
fn rounded_alpha_filter(r: f64) -> String {
    let r = format!("{r:.2}");
    format!(
        ",format=yuva420p,geq=lum='p(X\\,Y)':cb='p(X\\,Y)':cr='p(X\\,Y)':a='255*(1-gt(pow(max(0\\,max({r}-X\\,X-(W-1-{r})))\\,2)+pow(max(0\\,max({r}-Y\\,Y-(H-1-{r})))\\,2)\\,{r}*{r}))'",
        r = r
    )
}

/// Build a filter_complex chain that crops each `BlurRegion` out of the
/// current video, runs `boxblur` on it, and `overlay`s the result back
/// onto the main video — gated by an `enable=between(t,…)` expression so
/// the blur is only visible during the annotation's lifetime.
///
/// The function is deterministic and pure: callers can unit-test it in
/// isolation. Returns the new filter_complex string and the resulting
/// video map label.
pub fn build_annotation_blur_complex(
    filter_complex: Option<&str>,
    input_label: &str,
    regions: &[BlurRegion<'_>],
) -> (String, String) {
    if regions.is_empty() {
        return (
            filter_complex.unwrap_or("").to_string(),
            input_label.to_string(),
        );
    }

    let normalized_input = if input_label.starts_with('[') {
        input_label.to_string()
    } else {
        format!("[{input_label}]")
    };

    // Each region produces three nodes:
    //   [in] split  → [main_i][src_i]
    //   [src_i] crop=… , boxblur=… , (optional)drawbox=color  → [blur_i]
    //   [main_i][blur_i] overlay=x:y:enable='between(t,start,end)' → [in_{i+1}]
    let mut lines: Vec<String> = Vec::new();
    let mut current_in = normalized_input;

    for (i, region) in regions.iter().enumerate() {
        let main_label = format!("[blur_main_{i}]");
        let src_label = format!("[blur_src_{i}]");
        let out_label = if i + 1 == regions.len() {
            "[vblur]".to_string()
        } else {
            format!("[blur_step_{i}]")
        };
        let blur_label = format!("[blur_done_{i}]");

        // Split the current input. FFmpeg's split takes labels directly,
        // no `=` between filter name and outputs.
        lines.push(format!("{current_in}split{main_label}{src_label}"));

        // Crop + box-blur the source copy.
        //
        // boxblur rejects a radius larger than `(min(plane_w, plane_h) - 1) / 2`
        // for EACH plane, and under 4:2:0 the chroma plane is half-size, so it
        // caps lower than luma. We therefore clamp each radius to its OWN plane:
        // a small blur region can't request a radius bigger than itself. This
        // was the "Invalid luma_param radius value 84 ... must be <= 81" export
        // crash — a heavy blur on a small region. (127 is boxblur's hard ceiling;
        // luma_power=3 stacks three passes so even a region-limited radius still
        // obliterates detail for redaction. Luma stays as strong as the region
        // allows; chroma may be softer without weakening the redaction.)
        let w = region.w.max(2);
        let h = region.h.max(2);
        let x = region.x.max(0);
        let y = region.y.max(0);
        // FFmpeg requires `2*radius + 1 <= plane_dim`, i.e. radius <=
        // (dim-1)/2 per plane. A region too small to support even radius 1
        // resolves to 0 (a valid boxblur no-op) rather than an invalid literal.
        let requested = region.radius.clamp(1, 127) as i32;
        let luma_max = (w.min(h) - 1) / 2;
        let chroma_max = ((w / 2).min(h / 2) - 1) / 2;
        let luma_r = requested.min(luma_max);
        let chroma_r = requested.min(chroma_max);
        let mut tail = format!(
            "{src_label}crop={w}:{h}:{x}:{y},boxblur=luma_radius={luma_r}:luma_power=3:chroma_radius={chroma_r}:chroma_power=3"
        );

        // Tint variants overlay a translucent solid colour over the
        // already-blurred crop using `drawbox` with `t=fill`. `glass`
        // skips the tint pass entirely.
        // Tint alpha tracks strength: 0.15 → 0.95 across the slider, so the
        // strength control doubles as a redaction dial. Master opacity still
        // multiplies on top. Mirrors the preview side in BlurAnnotationLayer.
        let opacity = region.opacity.clamp(0.0, 1.0);
        let strength = region.strength.clamp(0.0, 1.0);
        let base_alpha = 0.15 + 0.80 * strength;
        let tint_rgba = match region.variant {
            "white" => Some(format!("white@{:.3}", base_alpha * opacity)),
            "black" => Some(format!("black@{:.3}", base_alpha * opacity)),
            "color" => {
                let r = ((region.tint_rgb >> 16) & 0xff) as u8;
                let g = ((region.tint_rgb >> 8) & 0xff) as u8;
                let b = (region.tint_rgb & 0xff) as u8;
                Some(format!(
                    "0x{r:02x}{g:02x}{b:02x}@{:.3}",
                    base_alpha * opacity
                ))
            }
            // glass: pile a faint grey wash on past strength=0.6 so the
            // glass variant also redacts when pushed hard.
            _ if strength > 0.6 => Some(format!("gray@{:.3}", ((strength - 0.6) * 0.6) * opacity)),
            _ => None,
        };
        if let Some(rgba) = tint_rgba {
            tail.push_str(&format!(",drawbox=x=0:y=0:w=iw:h=ih:color={rgba}:t=fill"));
        }
        // Rounded corners: give the blurred crop a rounded-rect alpha so the
        // overlay leaves the sharp video showing in the cut corners (matches the
        // preview). Clamp to half the shorter side (fully rounded).
        let corner = region.corner_px.min((w.min(h) as f64) / 2.0);
        if corner >= 1.0 {
            tail.push_str(&rounded_alpha_filter(corner));
        }
        tail.push_str(&blur_label);
        lines.push(tail);

        // Overlay the blurred crop back onto the main copy at the
        // region's position, gated on the enable window.
        let enable = format!(
            "between(t\\,{start:.4}\\,{end:.4})",
            start = region.start_secs.max(0.0),
            end = region.end_secs.max(region.start_secs.max(0.0)),
        );
        lines.push(format!(
            "{main_label}{blur_label}overlay=x={x}:y={y}:enable='{enable}'{out_label}",
            x = region.x.max(0),
            y = region.y.max(0),
        ));

        current_in = out_label;
    }

    let chain = lines.join(";");
    let combined = match filter_complex {
        Some(existing) if !existing.is_empty() => format!("{existing};{chain}"),
        _ => chain,
    };
    (combined, current_in)
}

/// Lines that should NOT count as part of the error context. We pipe
/// progress to stderr (`-progress pipe:2 -stats_period 0.1`) so it
/// streams in tens of times per second; without filtering, the last few
/// stderr lines are always progress noise and the real diagnostic gets
/// evicted.
fn is_progress_line(line: &str) -> bool {
    const PROGRESS_KEYS: &[&str] = &[
        "frame=",
        "fps=",
        "stream_",
        "bitrate=",
        "total_size=",
        "out_time_us=",
        "out_time_ms=",
        "out_time=",
        "dup_frames=",
        "drop_frames=",
        "speed=",
        "progress=",
    ];
    PROGRESS_KEYS.iter().any(|k| line.starts_with(k))
}

pub fn summarize_ffmpeg_error(stderr: &[u8]) -> String {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !is_progress_line(line))
        .collect();

    if lines.is_empty() {
        "FFmpeg failed without returning a detailed error.".into()
    } else {
        lines
            .iter()
            .rev()
            .take(12)
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

    // ffprobe is spawned on every editor open (100–500 ms cold) and again
    // during thumbnail generation. The result is immutable for a given file,
    // so serve it from the file-identity disk cache when available.
    if let Some(cached) = crate::cache::get::<VideoMetadata>("probe", &[path], 0) {
        return Ok(cached);
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
    crate::ffmpeg::configure_silent_command(&mut command);
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

            let meta = VideoMetadata {
                duration,
                width,
                height,
                fps,
                codec,
                size_bytes,
            };
            // Only the successful probe is cached — the zeroed fallback below
            // represents a probe failure and must not be pinned.
            crate::cache::put("probe", &[path], 0, &meta);
            Ok(meta)
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
    crate::ffmpeg::configure_silent_command(&mut command);
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

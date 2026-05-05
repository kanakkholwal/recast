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
        Self { fps: 15, max_colors: 128, dither: "bayer" }
    }
}

pub fn build_gif_palette_complex(
    filter_complex: Option<&str>,
    input_label: &str,
    options: GifFilterOptions<'_>,
    inline_scale: Option<&str>,
) -> (String, String) {
    let final_label = "[vgif]";
    let normalized_input = if input_label.starts_with('[') {
        input_label.to_string()
    } else {
        format!("[{input_label}]")
    };
    // Bake fps + scale into a single chain step BEFORE the split. That way
    // palettegen and paletteuse both consume the same downsampled frames —
    // generating a palette on full-res input and applying it to scaled
    // output (the previous shape) wastes palette slots on detail the GIF
    // can never show, and produces the visible "muddy palette" artefact.
    // Single linear chain is also far more forgiving of FFmpeg's
    // filter_complex parser quirks across versions.
    let scale_clause = match inline_scale {
        Some(s) if !s.is_empty() => format!(",{s}"),
        _ => String::new(),
    };
    // Clamp + render the dither argument. `none` is FFmpeg's literal disable,
    // bayer takes a `bayer_scale`, sierra2 ships without further knobs.
    let dither_clause = match options.dither {
        "none" => "dither=none".to_string(),
        "sierra2" => "dither=sierra2".to_string(),
        _ => "dither=bayer:bayer_scale=5".to_string(),
    };
    let max_colors = options.max_colors.clamp(2, 256);
    let fps = options.fps.max(1);
    let palette_chain = format!(
        "{normalized_input}fps={fps}{scale_clause},split[_gifa][_gifb];[_gifa]palettegen=max_colors={max_colors}:stats_mode=diff[_gifp];[_gifb][_gifp]paletteuse={dither_clause}:diff_mode=rectangle{final_label}"
    );
    let new_complex = match filter_complex {
        Some(existing) if !existing.is_empty() => format!("{existing};{palette_chain}"),
        _ => palette_chain,
    };
    (new_complex, final_label.to_string())
}

#[cfg(test)]
mod gif_tests {
    use super::*;

    #[test]
    fn includes_fps_and_default_palette() {
        let (complex, label) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 12, max_colors: 128, dither: "bayer" },
            None,
        );
        assert_eq!(label, "[vgif]");
        assert!(complex.starts_with("[vout]fps=12"), "got: {complex}");
        assert!(complex.contains("max_colors=128"));
        assert!(complex.contains("dither=bayer:bayer_scale=5"));
        assert!(complex.contains("paletteuse"));
        assert!(complex.contains("[vgif]"));
    }

    #[test]
    fn appends_to_existing_filter_complex() {
        let (complex, _) = build_gif_palette_complex(
            Some("[0:v]hflip[vout]"),
            "[vout]",
            GifFilterOptions::default(),
            None,
        );
        assert!(complex.starts_with("[0:v]hflip[vout];"));
        assert!(complex.contains("[vout]fps=15"));
    }

    #[test]
    fn bakes_inline_scale_before_split() {
        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 18, max_colors: 256, dither: "bayer" },
            Some("scale=w=720:h=-1"),
        );
        let split_idx = complex.find("split").expect("split present");
        let scale_idx = complex.find("scale=").expect("scale present");
        assert!(scale_idx < split_idx, "scale must come before split: {complex}");
    }

    #[test]
    fn sierra2_emits_bare_dither_arg() {
        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 15, max_colors: 128, dither: "sierra2" },
            None,
        );
        assert!(complex.contains("dither=sierra2"), "got: {complex}");
        assert!(!complex.contains("bayer_scale"));
    }

    #[test]
    fn dither_none_disables_dither() {
        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 15, max_colors: 128, dither: "none" },
            None,
        );
        assert!(complex.contains("dither=none"));
    }

    #[test]
    fn unknown_dither_falls_back_to_bayer() {
        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 15, max_colors: 128, dither: "wat" },
            None,
        );
        assert!(complex.contains("dither=bayer:bayer_scale=5"));
    }

    #[test]
    fn fps_zero_clamps_to_one() {
        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 0, max_colors: 128, dither: "bayer" },
            None,
        );
        assert!(complex.contains("fps=1"), "got: {complex}");
    }

    #[test]
    fn max_colors_clamped_to_gif_palette() {
        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 15, max_colors: 9999, dither: "bayer" },
            None,
        );
        assert!(complex.contains("max_colors=256"));

        let (complex, _) = build_gif_palette_complex(
            None,
            "vout",
            GifFilterOptions { fps: 15, max_colors: 1, dither: "bayer" },
            None,
        );
        assert!(complex.contains("max_colors=2"));
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
        }
    }

    #[test]
    fn empty_regions_returns_input_unchanged() {
        let (chain, label) = build_annotation_blur_complex(Some("[0:v]hflip[v]"), "[v]", &[]);
        assert_eq!(chain, "[0:v]hflip[v]");
        assert_eq!(label, "[v]");
    }

    #[test]
    fn single_region_emits_split_crop_overlay() {
        let regs = [region_with("glass", 1.0, 3.5)];
        let (chain, label) = build_annotation_blur_complex(None, "vout", &regs);
        // Split appears first to fork main/source streams.
        assert!(chain.contains("split[blur_main_0][blur_src_0]"), "chain: {chain}");
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
        for (variant, expected_color) in &[("white", "white@0.300"), ("black", "black@0.300")] {
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
        let regs = [BlurRegion { tint_rgb: 0x3b82f6, ..region_with("color", 0.0, 1.0) }];
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
        // 0.30 * 0.5 = 0.150
        assert!(chain.contains("white@0.150"), "chain: {chain}");
    }

    #[test]
    fn unknown_variant_treated_as_glass() {
        let regs = [region_with("alien", 0.0, 1.0)];
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
    fn radius_huge_clamps_to_64() {
        let regs = [BlurRegion {
            radius: 9999,
            ..region_with("glass", 0.0, 1.0)
        }];
        let (chain, _) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(chain.contains("boxblur=luma_radius=64"));
    }

    #[test]
    fn multiple_regions_chain_through_intermediate_labels() {
        let regs = [
            region_with("glass", 0.0, 2.0),
            region_with("white", 2.0, 4.0),
            region_with("color", 4.0, 6.0),
        ];
        let (chain, label) = build_annotation_blur_complex(None, "vout", &regs);
        assert!(chain.contains("[blur_step_0]"), "first step label missing: {chain}");
        assert!(chain.contains("[blur_step_1]"), "second step label missing: {chain}");
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
        assert!(chain.contains("between(t\\,5.0000\\,5.0000)"), "chain: {chain}");
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

    fn make_blur_region<'a>(annos: &'a [crate::render::node_types::Annotation], canvas_w: u32, canvas_h: u32, trim_start: f64) -> Vec<BlurRegion<'a>> {
        annos
            .iter()
            .filter(|a| !a.hidden)
            .filter_map(|a| match &a.kind {
                AnnotationKind::Blur {
                    x, y, w, h, strength, variant, tint_color, ..
                } => {
                    let cx = (x * canvas_w as f64).round() as i32;
                    let cy = (y * canvas_h as f64).round() as i32;
                    let cw = (w.abs() * canvas_w as f64).round() as i32;
                    let ch = (h.abs() * canvas_h as f64).round() as i32;
                    if cw < 4 || ch < 4 { return None; }
                    let max_dim = canvas_w.min(canvas_h) as f64 * 0.05;
                    let radius = (strength.clamp(0.0, 1.0) * max_dim).round().max(1.0) as u32;
                    let tint_rgb = u32::from_str_radix(tint_color.trim_start_matches('#'), 16).unwrap_or(0);
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
        // strength=1.0 + 1080 short edge → 5% = 54 → clamped to 64.
        assert!(r.radius >= 54 && r.radius <= 64, "radius={}", r.radius);
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
        assert!(regions.is_empty(), "hidden annotations must not generate filter regions");
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
            AnnotationKind::Blur { x, y, w, h, strength, variant, tint_color, radius } => {
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
            AnnotationKind::Blur { strength, variant, tint_color, radius, .. } => {
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
        let s = GifSettings { fps: None, quality: "medium".into(), r#loop: json!("infinite"), dither: "bayer".into() };
        assert_eq!(s.ffmpeg_loop_arg(), 0);
    }

    #[test]
    fn loop_once_to_minus_one() {
        let s = GifSettings { fps: None, quality: "medium".into(), r#loop: json!("once"), dither: "bayer".into() };
        assert_eq!(s.ffmpeg_loop_arg(), -1);
    }

    #[test]
    fn loop_numeric_passthrough() {
        let s = GifSettings { fps: None, quality: "medium".into(), r#loop: json!(3), dither: "bayer".into() };
        assert_eq!(s.ffmpeg_loop_arg(), 3);
    }

    #[test]
    fn loop_negative_clamped_to_minus_one() {
        let s = GifSettings { fps: None, quality: "medium".into(), r#loop: json!(-5), dither: "bayer".into() };
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

        // Crop + box-blur the source copy. Clamp radius into FFmpeg's
        // accepted range (1..127) and ensure at least 1 to keep the
        // filter literal.
        let radius = region.radius.clamp(1, 64);
        let mut tail = format!(
            "{src_label}crop={w}:{h}:{x}:{y},boxblur=luma_radius={r}:luma_power=2:chroma_radius={r}:chroma_power=2",
            w = region.w.max(2),
            h = region.h.max(2),
            x = region.x.max(0),
            y = region.y.max(0),
            r = radius,
        );

        // Tint variants overlay a translucent solid colour over the
        // already-blurred crop using `drawbox` with `t=fill`. `glass`
        // skips the tint pass entirely.
        let opacity = region.opacity.clamp(0.0, 1.0);
        let tint_rgba = match region.variant {
            "white" => Some(format!("white@{:.3}", 0.30 * opacity)),
            "black" => Some(format!("black@{:.3}", 0.30 * opacity)),
            "color" => {
                let r = ((region.tint_rgb >> 16) & 0xff) as u8;
                let g = ((region.tint_rgb >> 8) & 0xff) as u8;
                let b = (region.tint_rgb & 0xff) as u8;
                Some(format!("0x{r:02x}{g:02x}{b:02x}@{:.3}", 0.30 * opacity))
            }
            _ => None, // "glass" or unknown → no tint
        };
        if let Some(rgba) = tint_rgba {
            tail.push_str(&format!(
                ",drawbox=x=0:y=0:w=iw:h=ih:color={rgba}:t=fill"
            ));
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

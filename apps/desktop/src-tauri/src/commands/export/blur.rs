//! Annotation blur regions for the export filter graph.
//!
//! Split out of `run_export_job`. Pure: UV rects from the render state mapped
//! to canvas pixels. Touches no FFmpeg inputs, so it cannot disturb the
//! input-index arithmetic elsewhere in the graph.

use crate::commands::ffmpeg::BlurRegion;
use crate::render::graph::CanvasGeometry;
use crate::render::node_types::{Annotation, AnnotationAnchor, AnnotationKind};

/// Visible blur annotations as canvas-pixel regions, on the trimmed-but-uncut
/// axis (the cut/speed stage re-times them afterwards).
pub(crate) fn blur_regions<'a>(
    annotations: &'a [Annotation],
    geom: &CanvasGeometry,
    trim_start: f64,
) -> Vec<BlurRegion<'a>> {
    annotations
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
                radius: corner_frac,
                ..
            } => {
                // UV to canvas pixels over the annotation's anchor rect; static either way, since FFmpeg can't follow a per-frame zoom.
                let (rx, ry, rw_ref, rh_ref) = match a.anchor {
                    AnnotationAnchor::Frame => (
                        geom.comp_x as f64,
                        geom.comp_y as f64,
                        geom.comp_w as f64,
                        geom.comp_h as f64,
                    ),
                    AnnotationAnchor::Video => (
                        geom.video_x as f64,
                        geom.video_y as f64,
                        geom.video_w as f64,
                        geom.video_h as f64,
                    ),
                };
                let cx = (rx + x * rw_ref).round() as i32;
                let cy = (ry + y * rh_ref).round() as i32;
                let cw = (w.abs() * rw_ref).round() as i32;
                let ch = (h.abs() * rh_ref).round() as i32;
                if cw < 4 || ch < 4 {
                    return None;
                }
                // Strength maps to a radius up to 12% of the shorter edge, clamped at boxblur's 127; must agree with ffmpeg.rs::make_blur_region.
                let max_dim = geom.canvas_w.min(geom.canvas_h) as f64 * 0.12;
                let radius = (strength.clamp(0.0, 1.0) * max_dim)
                    .round()
                    .clamp(1.0, 127.0) as u32;
                let tint_rgb =
                    u32::from_str_radix(tint_color.trim_start_matches('#'), 16).unwrap_or(0x000000);
                // Corner radius as a fraction of the region's shorter side, the same basis as the preview's radius times min(w, h).
                let corner_px = corner_frac.clamp(0.0, 0.5) * (cw.min(ch) as f64);
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
                    corner_px,
                })
            }
            _ => None,
        })
        .collect()
}

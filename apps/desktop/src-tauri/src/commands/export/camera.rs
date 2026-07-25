//! Camera-bubble overlay geometry for the video export.

use crate::render::graph::{build_time_lut_expr, CanvasGeometry};
use crate::render::node_types::{CameraPlacement, ZoomRegion};

/// Max video-UV drift per unit of `(scale-1)*strength`. MUST match the preview's
/// `DRIFT_MAX` in `camera-overlay.logic.ts` so export == preview.
const CAMERA_DRIFT_MAX: f64 = 0.18;

/// Pixel rect `(x, y, w, h)` of the camera bubble, from its UV-space `placement`
/// and the canvas geometry. The bubble is square in screen pixels (`w == h`,
/// sized off `video_w` to match the preview's `aspect-ratio: 1`), and clamped
/// into the canvas so an out-of-range placement (legacy project / hand-edited
/// JSON) still yields a valid overlay.
pub(crate) fn camera_bubble_rect(
    placement: &CameraPlacement,
    geom: &CanvasGeometry,
) -> (u32, u32, u32, u32) {
    let bubble_w = (placement.width.clamp(0.02, 1.0) * geom.video_w as f64)
        .round()
        .max(2.0) as u32;
    let bubble_h = bubble_w;
    let max_x = geom.canvas_w.saturating_sub(bubble_w);
    let max_y = geom.canvas_h.saturating_sub(bubble_h);
    let bubble_x = ((geom.video_x as f64 + placement.x.clamp(0.0, 1.0) * geom.video_w as f64)
        .round() as u32)
        .min(max_x);
    let bubble_y = ((geom.video_y as f64 + placement.y.clamp(0.0, 1.0) * geom.video_h as f64)
        .round() as u32)
        .min(max_y);
    (bubble_x, bubble_y, bubble_w, bubble_h)
}

/// Effective camera placement under the zoom-follow effect — the exact mirror of
/// `applyZoomFollow` in `camera-overlay.logic.ts` (grow + drift away from the
/// zoom focus, square-preserving, clamped on-screen), so preview and export
/// agree. Identity at rest (`scale≈1`) or zero strength.
pub(crate) fn camera_follow_placement(
    base: &CameraPlacement,
    scale: f64,
    cx: f64,
    cy: f64,
    strength: f64,
) -> CameraPlacement {
    let k = strength.clamp(0.0, 1.0);
    if k <= 0.0 || scale <= 1.0001 {
        return base.clone();
    }
    let amount = (scale - 1.0) * k;
    let width = (base.width * (1.0 + amount)).min(1.0);
    let height = (base.height * (1.0 + amount)).min(1.0);
    let bcx = base.x + base.width / 2.0;
    let bcy = base.y + base.height / 2.0;
    let mut dx = bcx - cx;
    let mut dy = bcy - cy;
    let len = (dx * dx + dy * dy).sqrt();
    let drift = amount * CAMERA_DRIFT_MAX;
    if len > 1e-4 {
        dx = dx / len * drift;
        dy = dy / len * drift;
    } else {
        dx = 0.0;
        dy = 0.0;
    }
    CameraPlacement {
        x: (bcx + dx - width / 2.0).clamp(0.0, 1.0 - width),
        y: (bcy + dy - height / 2.0).clamp(0.0, 1.0 - height),
        width,
        height,
    }
}

/// Time-varying camera bubble geometry for export: `(size_expr, x_expr, y_expr)`
/// in output-stream `t`, following the zoom regions via `camera_follow_placement`
/// (size drives both w and h — the bubble stays square). `None` when no zoom
/// region is active, so the caller falls back to the fixed overlay. Sampled at
/// 20 Hz per region and collinear-merged, mirroring the main zoom LUT.
pub(crate) fn build_camera_follow_exprs(
    regions: &[ZoomRegion],
    base: &CameraPlacement,
    strength: f64,
    geom: &CanvasGeometry,
    trim_start: f64,
) -> Option<(String, String, String)> {
    // Default (outside every zoom region) = the base bubble rect in pixels.
    let (bx0, by0, bw0, _) = camera_bubble_rect(base, geom);
    let mut w_s: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut x_s: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut y_s: Vec<Vec<(f64, f64)>> = Vec::new();
    for region in regions {
        if region.hidden || region.end <= trim_start {
            continue;
        }
        let effective_start = region.start.max(trim_start);
        let duration = (region.end - effective_start).max(0.0);
        if duration <= 0.0 {
            continue;
        }
        let samples = ((duration * 20.0).ceil() as usize).clamp(8, 200);
        let step = duration / samples as f64;
        let (cx, cy) = (
            region.center_x.clamp(0.0, 1.0),
            region.center_y.clamp(0.0, 1.0),
        );
        let mut wv = Vec::with_capacity(samples + 1);
        let mut xv = Vec::with_capacity(samples + 1);
        let mut yv = Vec::with_capacity(samples + 1);
        for i in 0..=samples {
            let timeline_t = effective_start + step * i as f64;
            let output_t = timeline_t - trim_start;
            let scale = region.scale_at(timeline_t).max(1.0);
            let eff = camera_follow_placement(base, scale, cx, cy, strength);
            let (ex, ey, ew, _) = camera_bubble_rect(&eff, geom);
            wv.push((output_t, ew as f64));
            xv.push((output_t, ex as f64));
            yv.push((output_t, ey as f64));
        }
        w_s.push(wv);
        x_s.push(xv);
        y_s.push(yv);
    }
    if w_s.is_empty() {
        return None;
    }
    Some((
        build_time_lut_expr(&w_s, bw0 as f64),
        build_time_lut_expr(&x_s, bx0 as f64),
        build_time_lut_expr(&y_s, by0 as f64),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // A 1920x1080 video at (40, 60) inside a 2000x1200 canvas.
    fn geom() -> CanvasGeometry {
        CanvasGeometry {
            canvas_w: 2000,
            canvas_h: 1200,
            video_x: 40,
            video_y: 60,
            video_w: 1920,
            video_h: 1080,
            padding_px: 40,
            comp_x: 40,
            comp_y: 60,
            comp_w: 1920,
            comp_h: 1080,
        }
    }

    fn placement(x: f64, y: f64, width: f64) -> CameraPlacement {
        CameraPlacement {
            x,
            y,
            width,
            height: width,
        }
    }

    #[test]
    fn bubble_is_square_and_sized_off_video_width() {
        let (_, _, w, h) = camera_bubble_rect(&placement(0.0, 0.0, 0.2), &geom());
        assert_eq!((w, h), (384, 384)); // 0.2 * 1920
    }

    #[test]
    fn placement_is_clamped_into_the_canvas() {
        // x sits within bounds; y would place the bubble past the bottom edge, so
        // it clamps to canvas_h - h.
        let (x, y, w, h) = camera_bubble_rect(&placement(0.8, 0.8, 0.2), &geom());
        assert_eq!((w, h), (384, 384));
        assert_eq!(x, 1576); // 40 + 0.8*1920 = 1576, within max_x 1616
        assert_eq!(y, 816); // 60 + 0.8*1080 = 924, clamped to canvas_h - h = 816
    }

    #[test]
    fn width_is_clamped_to_a_minimum_fraction() {
        // width 0 clamps up to the 0.02 floor, not to 0.
        let (_, _, w, _) = camera_bubble_rect(&placement(0.0, 0.0, 0.0), &geom());
        assert_eq!(w, 38); // 0.02 * 1920 = 38.4 → 38
    }

    // Mirrors camera-overlay.logic.test.ts (applyZoomFollow) so preview == export.
    #[test]
    fn follow_is_identity_at_rest_or_zero_strength() {
        let base = placement(0.72, 0.08, 0.22);
        assert_eq!(camera_follow_placement(&base, 1.0, 0.5, 0.5, 0.6), base);
        assert_eq!(camera_follow_placement(&base, 1.8, 0.5, 0.5, 0.0), base);
    }

    #[test]
    fn follow_grows_with_the_zoom() {
        // grow = 1 + (1.5-1)*1 = 1.5 → 0.22 * 1.5 = 0.33
        let r = camera_follow_placement(&placement(0.72, 0.08, 0.22), 1.5, 0.2, 0.8, 1.0);
        assert!((r.width - 0.33).abs() < 1e-9, "width {}", r.width);
        assert!((r.width - r.height).abs() < 1e-9);
    }

    #[test]
    fn follow_drifts_away_from_the_focus() {
        let mid = placement(0.4, 0.4, 0.15);
        let r = camera_follow_placement(&mid, 1.3, 0.1, 0.1, 1.0);
        let before = ((mid.x + mid.width / 2.0) - 0.1).hypot((mid.y + mid.height / 2.0) - 0.1);
        let after = ((r.x + r.width / 2.0) - 0.1).hypot((r.y + r.height / 2.0) - 0.1);
        assert!(after > before, "expected drift away: {after} > {before}");
    }

    #[test]
    fn follow_exprs_default_to_base_outside_regions_and_vary_inside() {
        use crate::render::node_types::ZoomRegion;
        let base = placement(0.72, 0.08, 0.2);
        let region = ZoomRegion {
            start: 1.0,
            end: 3.0,
            scale: 1.8,
            ease_in: Default::default(),
            ease_out: Default::default(),
            ramp_in: 0.4,
            ramp_out: 0.4,
            center_x: 0.2,
            center_y: 0.2,
            hidden: false,
            motion_blur: 0.0,
            extra: Default::default(),
        };
        let (w, x, y) =
            build_camera_follow_exprs(&[region], &base, 0.6, &geom(), 0.0).expect("exprs");
        // Base pixels: bw0 = 0.2*1920 = 384, bx0 = 40 + 0.72*1920 = 1422.
        assert!(w.contains("384"), "size expr defaults to base width: {w}");
        assert!(x.contains("1422"), "x expr defaults to base x: {x}");
        // Time-gated terms fire inside the region.
        assert!(w.contains("if(gte(t,"), "size expr is time-varying: {w}");
        assert!(!y.is_empty());
    }
}

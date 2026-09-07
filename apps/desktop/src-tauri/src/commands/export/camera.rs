//! Camera-bubble overlay geometry for the video export.

use crate::render::easing::Easing;
use crate::render::graph::{build_time_lut_expr, CanvasGeometry};
use crate::render::node_types::{CameraKeyframe, CameraPlacement, ZoomRegion};

// Placement lives in `recast_compositor::camera`; this module keeps only what is FFmpeg's: the shadow PNG padding and the sampled LUT exprs.
pub(crate) use recast_compositor::camera::{
    activation as region_camera_activation, follow_placement as camera_follow_placement,
    placement_at as camera_placement_at,
};
use recast_compositor::camera::{SHADOW_BLUR_FRACTION, SHADOW_MAX_OPACITY, SHADOW_OFFSET_FRACTION};

/// The compositor takes borrowed regions, so a scene that owns them can lend a
/// subset without copying; this path owns the whole list.
fn camera_follow_scale_at(
    regions: &[ZoomRegion],
    t: f64,
    duration: f64,
    easing: Easing,
) -> (f64, f64, f64) {
    let borrowed: Vec<&ZoomRegion> = regions.iter().collect();
    recast_compositor::camera::follow_scale_at(&borrowed, t, duration, easing)
}

/// Resolved drop-shadow geometry in canvas pixels for a base bubble of width
/// `bubble_w`. `None` when the shadow is invisible (`strength ≤ 0`). `padding`
/// is the transparent margin baked into the pre-rendered shadow PNG so the blur
/// and downward offset have room to spread past the silhouette.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraShadowGeom {
    pub blur_px: f64,
    pub offset_px: f64,
    pub opacity: f64,
    pub padding: u32,
}

pub(crate) fn camera_shadow_geom(strength: f64, bubble_w: u32) -> Option<CameraShadowGeom> {
    let s = strength.clamp(0.0, 1.0);
    if s <= 0.0 || bubble_w == 0 {
        return None;
    }
    let bw = bubble_w as f64;
    let blur_px = SHADOW_BLUR_FRACTION * s * bw;
    let offset_px = SHADOW_OFFSET_FRACTION * s * bw;
    let opacity = SHADOW_MAX_OPACITY * s;
    // Bottom clearance must cover the blur spread (about 2x) plus the downward offset, with a couple of px for rounding.
    let padding = (blur_px * 2.0 + offset_px + 2.0).ceil().max(1.0) as u32;
    Some(CameraShadowGeom {
        blur_px,
        offset_px,
        opacity,
        padding,
    })
}

/// The bubble rect in whole pixels, which is what the filtergraph's `overlay`
/// and `crop` take.
pub(crate) fn camera_bubble_rect(
    placement: &CameraPlacement,
    geom: &CanvasGeometry,
) -> (u32, u32, u32, u32) {
    let rect = recast_compositor::camera::bubble_rect(placement, *geom);
    (rect.x as u32, rect.y as u32, rect.w as u32, rect.h as u32)
}

/// Time-varying camera geometry as `(size_expr, x_expr, y_expr)` in output `t`, composing the keyframe glide with zoom-follow; `None` when static and unfollowed.
/// Sampled at 20 Hz and collinear-merged like the main zoom LUT, with times mapped original to output as `- trim_start`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn build_camera_follow_exprs(
    regions: &[ZoomRegion],
    keyframes: &[CameraKeyframe],
    easing: Easing,
    follow_easing: Easing,
    follow_duration: f64,
    base: &CameraPlacement,
    strength: f64,
    zoom_follow: bool,
    geom: &CanvasGeometry,
    trim_start: f64,
    trim_end: f64,
) -> Option<(String, String, String)> {
    // Default (outside every sampled window) = the base bubble rect in pixels.
    let (bx0, by0, bw0, _) = camera_bubble_rect(base, geom);
    // The bubble is square in pixels, so its UV height is width * aspect.
    let aspect = if geom.video_h > 0 {
        geom.video_w as f64 / geom.video_h as f64
    } else {
        1.0
    };

    if keyframes.is_empty() {
        // Follow-only: the proven per-zoom-region sampling on a static base.
        if !zoom_follow {
            return None;
        }
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
                let a =
                    region_camera_activation(region, timeline_t, follow_duration, follow_easing);
                let scale = 1.0 + a * (region.scale.max(1.0) - 1.0);
                let eff = camera_follow_placement(base, scale, cx, cy, strength, aspect);
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
        return Some((
            build_time_lut_expr(&w_s, bw0 as f64),
            build_time_lut_expr(&x_s, bx0 as f64),
            build_time_lut_expr(&y_s, by0 as f64),
        ));
    }

    // A keyframed base glides across the WHOLE timeline, so sample uniformly and compose the follow where it applies.
    let duration = (trim_end - trim_start).max(0.0);
    if duration <= 0.0 {
        return None;
    }
    let samples = ((duration * 20.0).ceil() as usize).clamp(2, 20_000);
    let step = duration / samples as f64;
    let mut wv = Vec::with_capacity(samples + 1);
    let mut xv = Vec::with_capacity(samples + 1);
    let mut yv = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        let original_t = trim_start + step * i as f64;
        let output_t = original_t - trim_start;
        let base_t = camera_placement_at(base, keyframes, original_t, easing);
        let eff = if zoom_follow {
            let (scale, cx, cy) =
                camera_follow_scale_at(regions, original_t, follow_duration, follow_easing);
            camera_follow_placement(&base_t, scale, cx, cy, strength, aspect)
        } else {
            base_t
        };
        let (ex, ey, ew, _) = camera_bubble_rect(&eff, geom);
        wv.push((output_t, ew as f64));
        xv.push((output_t, ex as f64));
        yv.push((output_t, ey as f64));
    }
    Some((
        build_time_lut_expr(&[wv], bw0 as f64),
        build_time_lut_expr(&[xv], bx0 as f64),
        build_time_lut_expr(&[yv], by0 as f64),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// D-2, on the FFmpeg path. The bubble must drift along the SCREEN-SPACE
    /// away-from-focus direction; normalising the UV pair pulls it toward
    /// vertical on a 16:9 frame. The compositor and the preview were both
    /// fixed for this; the graph is the export users actually get.
    #[test]
    fn zoom_follow_drifts_along_the_screen_direction_not_the_uv_one() {
        let aspect = 16.0 / 9.0;
        let base = CameraPlacement {
            x: 0.425,
            y: 0.267,
            width: 0.15,
            height: 0.15 * aspect,
        };
        let (fx, fy) = (0.3, 0.3);
        let out = camera_follow_placement(&base, 1.5, fx, fy, 1.0, aspect);

        let base_h = base.width * aspect;
        let (bcx, bcy) = (base.x + base.width / 2.0, base.y + base_h / 2.0);
        let away = ((bcx - fx) * aspect, bcy - fy);
        let drift = (
            (out.x + out.width / 2.0 - bcx) * aspect,
            out.y + out.height / 2.0 - bcy,
        );
        let mag = away.0.hypot(away.1) * drift.0.hypot(drift.1);
        assert!(mag > 1e-9, "a vector was degenerate: {away:?} {drift:?}");
        let cross = (away.0 * drift.1 - away.1 * drift.0) / mag;
        assert!(
            cross.abs() < 1e-3,
            "drift off the screen-away direction by {:.2} deg",
            cross.asin().to_degrees()
        );
        assert!(
            away.0 * drift.0 + away.1 * drift.1 > 0.0,
            "the bubble drifted toward the focus, not away"
        );
        // D-2 was about magnitude: a fraction of frame HEIGHT whichever way the bubble ran, where normalising the UV pair drifted up to `aspect` times too far.
        let want = (1.5 - 1.0) * 1.0 * 0.18;
        let got = drift.0.hypot(drift.1);
        assert!(
            (got - want).abs() < 1e-6,
            "drifted {got} of frame height, want {want}"
        );
    }

    /// One rule, one implementation. The compositor is the authority: the
    /// preview and the engine export both draw from it.
    #[test]
    fn the_follow_placement_matches_the_compositors() {
        let aspect = 16.0 / 9.0;
        let base = CameraPlacement {
            x: 0.425,
            y: 0.267,
            width: 0.15,
            height: 0.15 * aspect,
        };
        for (scale, cx, cy, strength) in [
            (1.5, 0.3, 0.3, 1.0),
            (2.0, 0.8, 0.2, 0.5),
            (1.2, 0.5, 0.5, 1.0),
            (3.0, 0.1, 0.9, 0.25),
            (1.0, 0.3, 0.3, 1.0),
        ] {
            let ours = camera_follow_placement(&base, scale, cx, cy, strength, aspect);
            let theirs =
                recast_compositor::camera::follow_placement(&base, scale, cx, cy, strength, aspect);
            assert!(
                (ours.x - theirs.x).abs() < 1e-9
                    && (ours.y - theirs.y).abs() < 1e-9
                    && (ours.width - theirs.width).abs() < 1e-9
                    && (ours.height - theirs.height).abs() < 1e-9,
                "scale {scale} focus ({cx},{cy}) strength {strength}: {ours:?} vs {theirs:?}"
            );
        }
    }

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
        // x sits within bounds; y would put the bubble past the bottom edge, so it clamps to canvas_h minus h.
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
        assert_eq!(
            camera_follow_placement(&base, 1.0, 0.5, 0.5, 0.6, 1.0),
            base
        );
        assert_eq!(
            camera_follow_placement(&base, 1.8, 0.5, 0.5, 0.0, 1.0),
            base
        );
    }

    #[test]
    fn follow_grows_with_the_zoom() {
        // grow = 1 + (1.5-1)*1 = 1.5 → 0.22 * 1.5 = 0.33 (aspect 1 → height == width)
        let r = camera_follow_placement(&placement(0.72, 0.08, 0.22), 1.5, 0.2, 0.8, 1.0, 1.0);
        assert!((r.width - 0.33).abs() < 1e-9, "width {}", r.width);
        assert!((r.width - r.height).abs() < 1e-9);
    }

    #[test]
    fn follow_height_is_width_times_aspect_on_a_wide_video() {
        // Mirrors camera-overlay.logic.test.ts: square in pixels, so height is width times aspect on a 16:9 frame.
        let aspect = 16.0 / 9.0;
        let r = camera_follow_placement(&placement(0.4, 0.4, 0.15), 1.5, 0.1, 0.1, 1.0, aspect);
        assert!((r.width - 0.225).abs() < 1e-9, "width {}", r.width); // 0.15 * 1.5
        assert!(
            (r.height - r.width * aspect).abs() < 1e-9,
            "height {}",
            r.height
        );
    }

    #[test]
    fn follow_drifts_away_from_the_focus() {
        let mid = placement(0.4, 0.4, 0.15);
        let r = camera_follow_placement(&mid, 1.3, 0.1, 0.1, 1.0, 1.0);
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
        let (w, x, y) = build_camera_follow_exprs(
            &[region],
            &[],
            Easing::default(),
            Easing::LINEAR,
            0.4,
            &base,
            0.6,
            true,
            &geom(),
            0.0,
            3.0,
        )
        .expect("exprs");
        // Base pixels: bw0 = 0.2*1920 = 384, bx0 = 40 + 0.72*1920 = 1422.
        assert!(w.contains("384"), "size expr defaults to base width: {w}");
        assert!(x.contains("1422"), "x expr defaults to base x: {x}");
        // Time-gated terms fire inside the region.
        assert!(w.contains("if(gte(t,"), "size expr is time-varying: {w}");
        assert!(!y.is_empty());
    }

    // Mirrors camera-overlay.logic.test.ts (cameraPlacementAt) so preview == export.
    #[test]
    fn placement_at_holds_and_glides_between_keyframes() {
        let kf = |at: f64, x: f64| CameraKeyframe {
            at_sec: at,
            placement: placement(x, 0.1, 0.2),
        };
        let base = placement(0.5, 0.1, 0.2);
        let kfs = [kf(1.0, 0.1), kf(3.0, 0.7)];
        let lin = Easing::LINEAR;
        // No keyframes → static base.
        assert_eq!(camera_placement_at(&base, &[], 2.0, lin), base);
        // Holds outside the range.
        assert!((camera_placement_at(&base, &kfs, 0.0, lin).x - 0.1).abs() < 1e-6);
        assert!((camera_placement_at(&base, &kfs, 5.0, lin).x - 0.7).abs() < 1e-6);
        // Linear easing → midpoint x = 0.4, quarter x = 0.25.
        assert!((camera_placement_at(&base, &kfs, 2.0, lin).x - 0.4).abs() < 1e-6);
        assert!((camera_placement_at(&base, &kfs, 1.5, lin).x - 0.25).abs() < 1e-6);
    }

    #[test]
    fn shadow_geom_scales_with_strength_and_bubble_width() {
        assert!(camera_shadow_geom(0.0, 400).is_none());
        assert!(camera_shadow_geom(0.5, 0).is_none());
        let g = camera_shadow_geom(0.5, 400).expect("visible");
        // Fractions MUST equal the preview's CAMERA_SHADOW_* (0.14/0.05/0.6).
        assert!((g.blur_px - 0.14 * 0.5 * 400.0).abs() < 1e-9); // 28
        assert!((g.offset_px - 0.05 * 0.5 * 400.0).abs() < 1e-9); // 10
        assert!((g.opacity - 0.6 * 0.5).abs() < 1e-9); // 0.3
        assert!(g.padding >= (g.blur_px * 2.0 + g.offset_px).ceil() as u32);
    }

    // Mirrors camera-overlay.logic.test.ts (cameraFollowScaleAt) so preview == export.
    #[test]
    fn follow_scale_ramps_on_its_own_duration_and_easing() {
        use crate::render::node_types::ZoomRegion;
        let region = ZoomRegion {
            start: 0.0,
            end: 10.0,
            scale: 2.0,
            ease_in: Default::default(),
            ease_out: Default::default(),
            ramp_in: 0.4,
            ramp_out: 0.4,
            center_x: 0.3,
            center_y: 0.7,
            hidden: false,
            motion_blur: 0.0,
            extra: Default::default(),
        };
        let lin = Easing::LINEAR;
        let rs = std::slice::from_ref(&region);
        // Outside the region → identity.
        assert_eq!(camera_follow_scale_at(rs, -1.0, 1.0, lin).0, 1.0);
        // Duration 1s linear: halfway through ramp-in gives activation 0.5, so scale 1.5 and focus at the region centre.
        let (s, cx, cy) = camera_follow_scale_at(rs, 0.5, 1.0, lin);
        assert!((s - 1.5).abs() < 1e-6, "scale {s}");
        assert!((cx - 0.3).abs() < 1e-9 && (cy - 0.7).abs() < 1e-9);
        // Mid-hold (both ramps saturated) → full grow to peak.
        assert!((camera_follow_scale_at(rs, 5.0, 1.0, lin).0 - 2.0).abs() < 1e-6);
    }

    #[test]
    fn keyframed_exprs_are_time_varying_without_any_zoom() {
        let base = placement(0.72, 0.08, 0.2);
        let kfs = [
            CameraKeyframe {
                at_sec: 0.0,
                placement: placement(0.1, 0.08, 0.2),
            },
            CameraKeyframe {
                at_sec: 2.0,
                placement: placement(0.7, 0.08, 0.2),
            },
        ];
        // zoom_follow off, no regions — the base still glides via keyframes.
        let (_, x, _) = build_camera_follow_exprs(
            &[],
            &kfs,
            Easing::LINEAR,
            Easing::LINEAR,
            0.4,
            &base,
            0.6,
            false,
            &geom(),
            0.0,
            2.0,
        )
        .expect("keyframed exprs");
        assert!(x.contains("if(gte(t,"), "x expr is time-varying: {x}");
    }
}

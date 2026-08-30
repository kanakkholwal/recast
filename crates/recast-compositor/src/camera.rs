use recast_scene::v1::nodes::{CameraKeyframe, CameraOverlaySettings, CameraPlacement, ZoomRegion};
use recast_scene::v1::Easing;

use crate::eval::{Affine2, DestRect, ShadowParams};
use crate::geometry::CanvasGeometry;

const DRIFT_MAX: f64 = 0.18;
/// Fractions of the base bubble width. Locked to `CAMERA_SHADOW_*` in
/// `camera-overlay.logic.ts`, which sizes in `cqmin`.
const SHADOW_BLUR_FRACTION: f64 = 0.14;
const SHADOW_OFFSET_FRACTION: f64 = 0.05;
const SHADOW_MAX_OPACITY: f64 = 0.6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BubbleParams {
    pub dest: DestRect,
    /// Fraction of the shorter bubble edge, so 0.5 is a circle.
    pub corner_radius: f32,
    pub transform: Affine2,
}

/// Square in screen PIXELS, sized off `video_w` to match the preview's
/// `aspect-ratio: 1`, and clamped into the canvas so a hand-edited placement
/// still yields a valid overlay.
pub fn bubble_rect(placement: &CameraPlacement, geometry: CanvasGeometry) -> DestRect {
    let size = (placement.width.clamp(0.02, 1.0) * geometry.video_w as f64)
        .round()
        .max(2.0);
    let max_x = (geometry.canvas_w as f64 - size).max(0.0);
    let max_y = (geometry.canvas_h as f64 - size).max(0.0);
    DestRect {
        x: ((geometry.video_x as f64 + placement.x.clamp(0.0, 1.0) * geometry.video_w as f64)
            .round()
            .min(max_x)) as f32,
        y: ((geometry.video_y as f64 + placement.y.clamp(0.0, 1.0) * geometry.video_h as f64)
            .round()
            .min(max_y)) as f32,
        w: size as f32,
        h: size as f32,
    }
}

fn lerp_placement(a: &CameraPlacement, b: &CameraPlacement, e: f64) -> CameraPlacement {
    CameraPlacement {
        x: a.x + (b.x - a.x) * e,
        y: a.y + (b.y - a.y) * e,
        width: a.width + (b.width - a.width) * e,
        height: a.height + (b.height - a.height) * e,
    }
}

/// The eased glide between the keyframes bracketing `t`. Held at the first and
/// last keyframe outside their span.
pub fn placement_at(
    base: &CameraPlacement,
    keyframes: &[CameraKeyframe],
    t: f64,
    easing: Easing,
) -> CameraPlacement {
    let Some(first) = keyframes.first() else {
        return base.clone();
    };
    if keyframes.len() == 1 || t <= first.at_sec {
        return first.placement.clone();
    }
    let Some(last) = keyframes.last() else {
        return base.clone();
    };
    if t >= last.at_sec {
        return last.placement.clone();
    }
    for pair in keyframes.windows(2) {
        if t >= pair[0].at_sec && t < pair[1].at_sec {
            let span = (pair[1].at_sec - pair[0].at_sec).max(1e-6);
            let phase = ((t - pair[0].at_sec) / span).clamp(0.0, 1.0);
            return lerp_placement(
                &pair[0].placement,
                &pair[1].placement,
                easing.y(phase as f32) as f64,
            );
        }
    }
    last.placement.clone()
}

/// Ramps over the CAMERA's own duration and easing, not the zoom's, so the
/// bubble can grow slower than the zoom it is reacting to.
fn activation(region: &ZoomRegion, t: f64, duration: f64, easing: Easing) -> f64 {
    if region.hidden || t <= region.start || t >= region.end {
        return 0.0;
    }
    let d = duration.max(1e-3);
    let rising = easing.y((((t - region.start) / d).clamp(0.0, 1.0)) as f32) as f64;
    let falling = easing.y((((region.end - t) / d).clamp(0.0, 1.0)) as f32) as f64;
    rising.min(falling)
}

fn follow_scale_at(
    regions: &[&ZoomRegion],
    t: f64,
    duration: f64,
    easing: Easing,
) -> (f64, f64, f64) {
    for region in regions {
        if region.hidden || t <= region.start || t >= region.end {
            continue;
        }
        let a = activation(region, t, duration, easing);
        return (
            1.0 + a * (region.scale.max(1.0) - 1.0),
            region.center_x.clamp(0.0, 1.0),
            region.center_y.clamp(0.0, 1.0),
        );
    }
    (1.0, 0.5, 0.5)
}

/// Grow and drift away from the zoom focus, clamped on-screen. The bubble is
/// square in pixels, so its UV height is `width * aspect` and is derived here
/// rather than read from `base.height`, which is wrong on a non-square frame.
pub fn follow_placement(
    base: &CameraPlacement,
    scale: f64,
    cx: f64,
    cy: f64,
    strength: f64,
    aspect: f64,
) -> CameraPlacement {
    let k = strength.clamp(0.0, 1.0);
    if k <= 0.0 || scale <= 1.0001 {
        return base.clone();
    }
    let base_h = (base.width * aspect).min(1.0);
    let amount = (scale - 1.0) * k;
    let width = (base.width * (1.0 + amount)).min(1.0);
    let height = (width * aspect).min(1.0);
    let bcx = base.x + base.width / 2.0;
    let bcy = base.y + base_h / 2.0;

    // The away-from-focus direction is a SCREEN-SPACE one, but `bcx-cx` / `bcy-cy`
    // are UV, and one UV-x unit is `video_w` px while one UV-y unit is `video_h`
    // px (aspect = video_w/video_h). Normalising the UV pair treats them as the
    // same unit, so the drift angle is wrong on a non-square frame (D-2).
    // Normalise in pixels (video_h as the unit), then take the drift back to UV
    // per axis. Drift magnitude stays a fraction of the frame height.
    let drift = amount * DRIFT_MAX;
    let (px, py) = ((bcx - cx) * aspect, bcy - cy);
    let len = (px * px + py * py).sqrt();
    let (dx, dy) = if len > 1e-4 {
        (px / len * drift / aspect, py / len * drift)
    } else {
        (0.0, 0.0)
    };

    // Redundant with `bubble_rect`'s own clamp, kept because this mirrors `applyZoomFollow`, which clamps here too.
    CameraPlacement {
        x: (bcx + dx - width / 2.0).clamp(0.0, 1.0 - width),
        y: (bcy + dy - height / 2.0).clamp(0.0, 1.0 - height),
        width,
        height,
    }
}

fn corner_radius_for(settings: &CameraOverlaySettings) -> f32 {
    match settings.shape.as_str() {
        "circle" => 0.5,
        "square" => 0.0,
        _ => settings.corner_radius.clamp(0.0, 0.5) as f32,
    }
}

/// Mirrors horizontally by flipping the source affine, so a mirrored bubble
/// costs nothing beyond the uniform it already uploads.
fn bubble_transform(settings: &CameraOverlaySettings) -> Affine2 {
    if settings.mirror {
        Affine2 {
            sx: -1.0,
            shx: 0.0,
            tx: 1.0,
            shy: 0.0,
            sy: 1.0,
            ty: 0.0,
        }
    } else {
        Affine2::IDENTITY
    }
}

/// Evaluated directly at `source_time`. The FFmpeg path had to sample this at
/// 20 Hz, collinear-merge the samples and emit three expression LUTs; none of
/// that machinery has a successor.
pub fn bubble_params(
    settings: &CameraOverlaySettings,
    regions: &[&ZoomRegion],
    source_time: f64,
    geometry: CanvasGeometry,
) -> Option<BubbleParams> {
    if !settings.enabled {
        return None;
    }
    let aspect = if geometry.video_h > 0 {
        geometry.video_w as f64 / geometry.video_h as f64
    } else {
        1.0
    };

    let base = placement_at(
        &settings.default_placement,
        &settings.keyframes,
        source_time,
        settings.keyframe_easing,
    );
    let placement = if settings.zoom_follow {
        let (scale, cx, cy) = follow_scale_at(
            regions,
            source_time,
            settings.zoom_follow_duration,
            settings.zoom_follow_easing,
        );
        follow_placement(&base, scale, cx, cy, settings.zoom_follow_strength, aspect)
    } else {
        base
    };

    Some(BubbleParams {
        dest: bubble_rect(&placement, geometry),
        corner_radius: corner_radius_for(settings),
        transform: bubble_transform(settings),
    })
}

pub fn bubble_shadow(
    settings: &CameraOverlaySettings,
    bubble: &BubbleParams,
) -> Option<ShadowParams> {
    let strength = settings.shadow.clamp(0.0, 1.0);
    if strength <= 0.0 || bubble.dest.w <= 0.0 {
        return None;
    }
    let width = bubble.dest.w as f64;
    Some(ShadowParams {
        color: recast_color::Srgba::opaque(0, 0, 0),
        opacity: (SHADOW_MAX_OPACITY * strength) as f32,
        blur_px: (SHADOW_BLUR_FRACTION * strength * width).max(0.5) as f32,
        spread_px: 0.0,
        offset_y_px: (SHADOW_OFFSET_FRACTION * strength * width) as f32,
        center_x: bubble.dest.x + bubble.dest.w / 2.0,
        center_y: bubble.dest.y + bubble.dest.h / 2.0,
        half_w: bubble.dest.w / 2.0,
        half_h: bubble.dest.h / 2.0,
        radius_px: bubble.corner_radius * bubble.dest.w.min(bubble.dest.h),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn geometry() -> CanvasGeometry {
        crate::geometry::canvas_geometry(1000, 500, 0.0, None)
    }

    fn settings(json: &str) -> CameraOverlaySettings {
        serde_json::from_str(json).expect("camera json")
    }

    fn enabled(extra: &str) -> CameraOverlaySettings {
        settings(&format!(
            r#"{{"enabled": true, {extra} "shape": "rounded"}}"#
        ))
    }

    fn zoom(json: &str) -> ZoomRegion {
        serde_json::from_str(json).expect("zoom json")
    }

    #[test]
    fn a_disabled_camera_produces_nothing() {
        let s = settings(r#"{"enabled": false}"#);
        assert!(bubble_params(&s, &[], 0.0, geometry()).is_none());
    }

    #[test]
    fn the_bubble_is_square_in_pixels_even_on_a_wide_frame() {
        let s = enabled(r#""defaultPlacement": {"x":0.1,"y":0.1,"width":0.2,"height":0.2},"#);
        let b = bubble_params(&s, &[], 0.0, geometry()).expect("bubble");
        assert_eq!(b.dest.w, b.dest.h);
        assert_eq!(b.dest.w, 200.0);
    }

    #[test]
    fn a_placement_past_the_canvas_edge_is_clamped_back_on_screen() {
        let s = enabled(r#""defaultPlacement": {"x":0.95,"y":0.95,"width":0.2,"height":0.2},"#);
        let b = bubble_params(&s, &[], 0.0, geometry()).expect("bubble");
        assert!(b.dest.x + b.dest.w <= geometry().canvas_w as f32);
        assert!(b.dest.y + b.dest.h <= geometry().canvas_h as f32);
    }

    #[test]
    fn the_shape_decides_the_corner_radius() {
        assert_eq!(
            bubble_params(
                &settings(r#"{"enabled": true, "shape": "circle"}"#),
                &[],
                0.0,
                geometry()
            )
            .expect("bubble")
            .corner_radius,
            0.5
        );
        assert_eq!(
            bubble_params(
                &settings(r#"{"enabled": true, "shape": "square"}"#),
                &[],
                0.0,
                geometry()
            )
            .expect("bubble")
            .corner_radius,
            0.0
        );
    }

    #[test]
    fn mirroring_flips_the_source_affine_rather_than_the_rect() {
        let mirrored = bubble_params(
            &settings(r#"{"enabled": true, "mirror": true}"#),
            &[],
            0.0,
            geometry(),
        )
        .expect("bubble");
        let plain = bubble_params(
            &settings(r#"{"enabled": true, "mirror": false}"#),
            &[],
            0.0,
            geometry(),
        )
        .expect("bubble");
        assert_eq!(mirrored.dest, plain.dest);
        assert_eq!(mirrored.transform.sx, -1.0);
        assert_eq!(mirrored.transform.tx, 1.0);
        assert!(plain.transform.is_identity());
    }

    #[test]
    fn keyframes_glide_the_base_between_their_positions() {
        let s = enabled(
            r#""keyframes": [
                {"atSec": 0.0, "placement": {"x":0.0,"y":0.0,"width":0.2,"height":0.2}},
                {"atSec": 10.0, "placement": {"x":0.8,"y":0.0,"width":0.2,"height":0.2}}],"#,
        );
        let at = |t: f64| {
            bubble_params(&s, &[], t, geometry())
                .expect("bubble")
                .dest
                .x
        };
        assert!(at(0.0) < at(5.0));
        assert!(at(5.0) < at(10.0));
    }

    #[test]
    fn a_keyframed_base_holds_outside_the_first_and_last_keyframe() {
        let s = enabled(
            r#""keyframes": [
                {"atSec": 2.0, "placement": {"x":0.1,"y":0.0,"width":0.2,"height":0.2}},
                {"atSec": 4.0, "placement": {"x":0.5,"y":0.0,"width":0.2,"height":0.2}}],"#,
        );
        let at = |t: f64| {
            bubble_params(&s, &[], t, geometry())
                .expect("bubble")
                .dest
                .x
        };
        assert_eq!(at(0.0), at(2.0));
        assert_eq!(at(9.0), at(4.0));
    }

    #[test]
    fn zoom_follow_grows_the_bubble_while_a_region_is_active() {
        let s = enabled(
            r#""defaultPlacement": {"x":0.5,"y":0.3,"width":0.15,"height":0.15},
               "zoomFollow": true, "zoomFollowStrength": 1.0,"#,
        );
        let region = zoom(r#"{"start":2.0,"end":8.0,"scale":2.0,"centerX":0.2,"centerY":0.5}"#);
        let regions = [&region];
        let resting = bubble_params(&s, &regions, 0.0, geometry()).expect("bubble");
        let grown = bubble_params(&s, &regions, 5.0, geometry()).expect("bubble");
        assert!(
            grown.dest.w > resting.dest.w,
            "{} did not grow past {}",
            grown.dest.w,
            resting.dest.w
        );
    }

    #[test]
    fn zoom_follow_drifts_the_bubble_away_from_the_zoom_focus() {
        let s = enabled(
            r#""defaultPlacement": {"x":0.5,"y":0.3,"width":0.15,"height":0.15},
               "zoomFollow": true, "zoomFollowStrength": 1.0,"#,
        );
        let region = zoom(r#"{"start":2.0,"end":8.0,"scale":2.0,"centerX":0.1,"centerY":0.5}"#);
        let regions = [&region];
        let resting = bubble_params(&s, &regions, 0.0, geometry()).expect("bubble");
        let drifted = bubble_params(&s, &regions, 5.0, geometry()).expect("bubble");
        let resting_centre = resting.dest.x + resting.dest.w / 2.0;
        let drifted_centre = drifted.dest.x + drifted.dest.w / 2.0;
        assert!(
            drifted_centre > resting_centre,
            "the bubble drifted toward the focus, not away: {drifted_centre} vs {resting_centre}"
        );
    }

    /// D-2: the bubble must drift along the SCREEN-SPACE away-from-focus
    /// direction, not the UV one. On 16:9 the two differ, and the old code
    /// normalised the UV pair, pulling the drift toward vertical. Asserts the
    /// drift is collinear with the screen-space away vector on a non-square frame.
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
        let out = follow_placement(&base, 1.5, fx, fy, 1.0, aspect);

        let base_h = base.width * aspect;
        let (bcx, bcy) = (base.x + base.width / 2.0, base.y + base_h / 2.0);
        // Both vectors in screen pixels (video_h as the unit).
        let away = ((bcx - fx) * aspect, bcy - fy);
        let drift = (
            (out.x + out.width / 2.0 - bcx) * aspect,
            out.y + out.height / 2.0 - bcy,
        );
        let mag = away.0.hypot(away.1) * drift.0.hypot(drift.1);
        assert!(
            mag > 1e-9,
            "a vector was degenerate: away {away:?} drift {drift:?}"
        );
        // Collinear: the normalised cross product is ~0, and pointing the same way.
        let cross = (away.0 * drift.1 - away.1 * drift.0) / mag;
        let dot = away.0 * drift.0 + away.1 * drift.1;
        assert!(
            cross.abs() < 1e-3,
            "drift off the screen-away direction by {:.2} deg",
            cross.asin().to_degrees()
        );
        assert!(dot > 0.0, "the bubble drifted toward the focus, not away");
    }

    #[test]
    fn zoom_follow_off_leaves_the_bubble_alone() {
        let s = enabled(
            r#""defaultPlacement": {"x":0.7,"y":0.1,"width":0.2,"height":0.2},
               "zoomFollow": false,"#,
        );
        let region = zoom(r#"{"start":2.0,"end":8.0,"scale":2.0,"centerX":0.2,"centerY":0.5}"#);
        let regions = [&region];
        assert_eq!(
            bubble_params(&s, &regions, 0.0, geometry())
                .expect("bubble")
                .dest,
            bubble_params(&s, &regions, 5.0, geometry())
                .expect("bubble")
                .dest
        );
    }

    #[test]
    fn a_hidden_zoom_region_does_not_move_the_camera() {
        let s = enabled(r#""zoomFollow": true, "zoomFollowStrength": 1.0,"#);
        let region = zoom(
            r#"{"start":2.0,"end":8.0,"scale":2.0,"centerX":0.2,"centerY":0.5,"hidden":true}"#,
        );
        let regions = [&region];
        assert_eq!(
            bubble_params(&s, &regions, 0.0, geometry())
                .expect("bubble")
                .dest,
            bubble_params(&s, &regions, 5.0, geometry())
                .expect("bubble")
                .dest
        );
    }

    #[test]
    fn a_zero_strength_shadow_is_omitted() {
        let s = enabled(r#""shadow": 0.0,"#);
        let b = bubble_params(&s, &[], 0.0, geometry()).expect("bubble");
        assert!(bubble_shadow(&s, &b).is_none());
    }

    #[test]
    fn shadow_blur_offset_and_opacity_all_scale_with_strength() {
        let s = enabled(r#""shadow": 0.5,"#);
        let b = bubble_params(&s, &[], 0.0, geometry()).expect("bubble");
        let shadow = bubble_shadow(&s, &b).expect("shadow");
        let full = enabled(r#""shadow": 1.0,"#);
        let full_shadow = bubble_shadow(&full, &b).expect("shadow");
        assert!(full_shadow.blur_px > shadow.blur_px);
        assert!(full_shadow.offset_y_px > shadow.offset_y_px);
        assert!(full_shadow.opacity > shadow.opacity);
        assert!(full_shadow.opacity <= SHADOW_MAX_OPACITY as f32);
    }
}

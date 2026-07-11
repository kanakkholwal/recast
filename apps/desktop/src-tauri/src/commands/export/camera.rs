//! Camera-bubble overlay geometry for the video export.

use crate::render::graph::CanvasGeometry;
use crate::render::node_types::CameraPlacement;

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
}

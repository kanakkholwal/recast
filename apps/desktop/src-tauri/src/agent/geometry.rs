//! Tier 2 `check`: what the frame would show. Static geometry in frame fractions, evaluated once per element rather
//! than per sampled time, because none of these boxes move except the camera, whose default placement is what is read.

use recast_time::TimeMap;
use serde_json::Value;

use super::check::{finding, plain, Finding, Severity};
use crate::render::graph::RenderState;

/// A box in frame fractions: x, y, width, height.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frac(pub f64, pub f64, pub f64, pub f64);

impl Frac {
    fn right(self) -> f64 {
        self.0 + self.2
    }
    fn bottom(self) -> f64 {
        self.1 + self.3
    }
    fn overlaps(self, other: Frac) -> bool {
        self.0 < other.right()
            && other.0 < self.right()
            && self.1 < other.bottom()
            && other.1 < self.bottom()
    }
    /// How much of this box lies inside the frame, 0..1 of its own area.
    fn visible_share(self) -> f64 {
        let w = self.right().min(1.0) - self.0.max(0.0);
        let h = self.bottom().min(1.0) - self.1.max(0.0);
        if self.2 <= 0.0 || self.3 <= 0.0 || w <= 0.0 || h <= 0.0 {
            return 0.0;
        }
        ((w * h) / (self.2 * self.3)).min(1.0)
    }
}

/// The box an annotation occupies, read generically so every kind with `x/y/width/height` counts; arrows use their two ends.
pub fn annotation_box(kind: &Value) -> Option<Frac> {
    let num = |k: &str| kind.get(k).and_then(Value::as_f64);
    if let (Some(x), Some(y), Some(x2), Some(y2)) = (num("x"), num("y"), num("x2"), num("y2")) {
        return Some(Frac(x.min(x2), y.min(y2), (x2 - x).abs(), (y2 - y).abs()));
    }
    let (x, y) = (num("x")?, num("y")?);
    let (w, h) = (num("width").unwrap_or(0.0), num("height").unwrap_or(0.0));
    let (x, w) = if w < 0.0 { (x + w, -w) } else { (x, w) };
    let (y, h) = if h < 0.0 { (y + h, -h) } else { (y, h) };
    Some(Frac(x, y, w, h))
}

/// The band captions draw into, from their position and offset: a strip about 15% high at the top or bottom.
pub fn caption_band(position: &str, offset_pct: f64) -> Frac {
    let inset = (offset_pct / 100.0).clamp(0.0, 0.5);
    match position {
        "top" => Frac(0.15, inset, 0.7, 0.15),
        _ => Frac(0.15, 1.0 - inset - 0.15, 0.7, 0.15),
    }
}

/// The part of the source a zoom shows: centre ± half the inverse scale, before the compositor clamps it.
pub fn zoom_view(center_x: f64, center_y: f64, scale: f64) -> Frac {
    let half = 0.5 / scale.max(1.0);
    Frac(center_x - half, center_y - half, 2.0 * half, 2.0 * half)
}

pub fn push_geometry(state: &RenderState, map: &TimeMap, out: &mut Vec<Finding>) {
    for (i, a) in state.annotations.iter().enumerate() {
        if a.hidden {
            continue;
        }
        let Some(kind) = serde_json::to_value(&a.kind).ok() else {
            continue;
        };
        let Some(b) = annotation_box(&kind) else {
            continue;
        };
        let field = format!("annotations/{i}");
        let share = b.visible_share();
        if share == 0.0 {
            out.push(finding(
                "annotation_off_frame",
                Severity::Error,
                field,
                "lies entirely outside the frame",
                map,
                a.start,
                a.end,
            ));
        } else if share < 0.5 {
            out.push(finding(
                "annotation_clipped",
                Severity::Warning,
                field,
                "more than half of it is outside the frame",
                map,
                a.start,
                a.end,
            ));
        }
    }
    for (i, z) in state.zoom_regions.iter().enumerate() {
        if z.hidden {
            continue;
        }
        let v = zoom_view(z.center_x, z.center_y, z.scale);
        if v.0 < -1e-6 || v.1 < -1e-6 || v.right() > 1.0 + 1e-6 || v.bottom() > 1.0 + 1e-6 {
            out.push(finding(
                "zoom_target_clamped",
                Severity::Warning,
                format!("zoomRegions/{i}"),
                "the centre at this scale reaches past the frame, so the view is clamped and drifts from the target",
                map,
                z.start,
                z.end,
            ));
        }
    }
    let cam = &state.camera_overlay;
    let caps = state.caption_style.as_ref().filter(|c| c.enabled);
    if let (true, Some(c)) = (cam.enabled, caps) {
        let p = &cam.default_placement;
        let bubble = Frac(p.x, p.y, p.width, p.height);
        if bubble.overlaps(caption_band(&c.position, c.offset_pct)) {
            out.push(plain(
                "caption_under_camera",
                Severity::Warning,
                "captionStyle/position",
                "the caption band and the camera bubble share the same corner of the frame; move one",
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn boxes_report_their_visible_share_and_arrows_use_their_ends() {
        let inside =
            annotation_box(&json!({"x": 0.1, "y": 0.1, "width": 0.2, "height": 0.2})).unwrap();
        assert!((inside.visible_share() - 1.0).abs() < 1e-9);
        assert_eq!(
            annotation_box(&json!({"x": 1.2, "y": 0.1, "width": 0.2, "height": 0.2}))
                .unwrap()
                .visible_share(),
            0.0
        );
        let half = annotation_box(&json!({"x": 0.9, "y": 0.0, "width": 0.2, "height": 0.2}))
            .unwrap()
            .visible_share();
        assert!((half - 0.5).abs() < 1e-9);
        let flipped =
            annotation_box(&json!({"x": 0.5, "y": 0.5, "width": -0.2, "height": -0.2})).unwrap();
        assert_eq!(flipped, Frac(0.3, 0.3, 0.2, 0.2));
        let arrow = annotation_box(&json!({"x": 0.8, "y": 0.8, "x2": 0.2, "y2": 0.3})).unwrap();
        assert!(
            (arrow.0 - 0.2).abs() < 1e-9 && (arrow.2 - 0.6).abs() < 1e-9,
            "{arrow:?}"
        );
        assert!(annotation_box(&json!({"strength": 0.5})).is_none());
    }

    #[test]
    fn a_zoom_whose_view_leaves_the_frame_is_the_one_the_compositor_clamps() {
        let v = zoom_view(0.5, 0.5, 2.0);
        assert_eq!(v, Frac(0.25, 0.25, 0.5, 0.5));
        let edge = zoom_view(0.9, 0.5, 2.0);
        assert!(edge.right() > 1.0);
        assert_eq!(
            zoom_view(0.5, 0.5, 0.5),
            Frac(0.0, 0.0, 1.0, 1.0),
            "scale below 1 is treated as 1"
        );
    }

    #[test]
    fn the_caption_band_meets_a_bottom_corner_bubble_but_not_a_top_one() {
        let bottom_right = Frac(0.72, 0.72, 0.22, 0.22);
        assert!(bottom_right.overlaps(caption_band("bottom", 5.0)));
        assert!(!bottom_right.overlaps(caption_band("top", 5.0)));
        let top_right = Frac(0.72, 0.08, 0.22, 0.22);
        assert!(!top_right.overlaps(caption_band("bottom", 5.0)));
        assert!(top_right.overlaps(caption_band("top", 5.0)));
    }
}

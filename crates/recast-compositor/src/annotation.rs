use recast_color::{parse_css_color, Srgba};
use recast_scene::v1::nodes::{Annotation, AnnotationAnchor, AnnotationKind};

use crate::eval::{Affine2, DestRect};
use crate::geometry::CanvasGeometry;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnnotationShape {
    Rect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
    },
    Ellipse {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
    Arrow {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        head: f32,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnnotationParams {
    pub shape: AnnotationShape,
    pub fill: Srgba,
    pub stroke: Srgba,
    pub stroke_width: f32,
    pub alpha: f32,
}

/// Where a UV point lands on the canvas. `Frame` pins to the output frame and
/// ignores zoom; `Video` rides the card, so it tracks the zoom affine.
fn uv_to_canvas(
    uv: (f64, f64),
    anchor: AnnotationAnchor,
    geometry: CanvasGeometry,
    dest: DestRect,
    transform: Affine2,
) -> (f32, f32) {
    match anchor {
        AnnotationAnchor::Frame => (
            (uv.0 * geometry.canvas_w as f64) as f32,
            (uv.1 * geometry.canvas_h as f64) as f32,
        ),
        AnnotationAnchor::Video => {
            let sx = if transform.sx.abs() < f32::EPSILON {
                1.0
            } else {
                transform.sx
            };
            let sy = if transform.sy.abs() < f32::EPSILON {
                1.0
            } else {
                transform.sy
            };
            let local_x = (uv.0 as f32 - transform.tx) / sx;
            let local_y = (uv.1 as f32 - transform.ty) / sy;
            (dest.x + local_x * dest.w, dest.y + local_y * dest.h)
        }
    }
}

/// Scale a UV length onto the canvas. Uses the shorter edge so a stroke keeps
/// the same visual weight on a portrait canvas as on a landscape one.
fn uv_scale(
    anchor: AnnotationAnchor,
    geometry: CanvasGeometry,
    dest: DestRect,
    transform: Affine2,
) -> f32 {
    match anchor {
        AnnotationAnchor::Frame => geometry.canvas_w.min(geometry.canvas_h) as f32,
        AnnotationAnchor::Video => {
            let sx = if transform.sx.abs() < f32::EPSILON {
                1.0
            } else {
                transform.sx
            };
            dest.w.min(dest.h) / sx
        }
    }
}

/// Split-ramp alpha on the ORIGINAL axis, multiplied by the master opacity.
/// Each ramp caps at half the annotation's span, matching the zoom evaluator.
pub fn annotation_alpha(annotation: &Annotation, source_time: f64) -> f64 {
    if source_time < annotation.start || source_time > annotation.end {
        return 0.0;
    }
    let duration = (annotation.end - annotation.start).max(0.0);
    let ramp_in = annotation.ramp_in.max(0.0).min(duration * 0.5);
    let ramp_out = annotation.ramp_out.max(0.0).min(duration * 0.5);
    let hold_start = annotation.start + ramp_in;
    let hold_end = annotation.end - ramp_out;

    let raw = if ramp_in > 0.0 && source_time < hold_start {
        let phase = ((source_time - annotation.start) / ramp_in).clamp(0.0, 1.0);
        annotation.ease_in.y(phase as f32) as f64
    } else if ramp_out > 0.0 && source_time > hold_end {
        let phase = ((annotation.end - source_time) / ramp_out).clamp(0.0, 1.0);
        annotation.ease_out.y(phase as f32) as f64
    } else {
        1.0
    };
    raw * annotation.opacity.clamp(0.0, 1.0)
}

/// `None` for a kind this pass cannot draw yet (image, text and blur all need
/// something outside the shape shader: an uploaded asset, a shaped font run, or
/// a copy of what is underneath).
pub fn annotation_params(
    annotation: &Annotation,
    source_time: f64,
    geometry: CanvasGeometry,
    dest: DestRect,
    transform: Affine2,
) -> Option<AnnotationParams> {
    if annotation.hidden {
        return None;
    }
    let alpha = annotation_alpha(annotation, source_time);
    if alpha <= 0.0 {
        return None;
    }

    let to_canvas =
        |x: f64, y: f64| uv_to_canvas((x, y), annotation.anchor, geometry, dest, transform);
    let scale = uv_scale(annotation.anchor, geometry, dest, transform);

    let shape = match &annotation.kind {
        AnnotationKind::Rect { x, y, w, h, radius } => {
            // Width and height go negative while the user drags; the UI flips
            // the rect on release, and the renderer must not wait for that.
            let (left, top) = (x.min(x + w), y.min(y + h));
            let (px, py) = to_canvas(left, top);
            let (fx, fy) = to_canvas(left + w.abs(), top + h.abs());
            AnnotationShape::Rect {
                x: px,
                y: py,
                w: fx - px,
                h: fy - py,
                radius: (*radius as f32) * scale,
            }
        }
        AnnotationKind::Ellipse { x, y, w, h } => {
            let (left, top) = (x.min(x + w), y.min(y + h));
            let (px, py) = to_canvas(left, top);
            let (fx, fy) = to_canvas(left + w.abs(), top + h.abs());
            AnnotationShape::Ellipse {
                cx: (px + fx) * 0.5,
                cy: (py + fy) * 0.5,
                rx: (fx - px) * 0.5,
                ry: (fy - py) * 0.5,
            }
        }
        AnnotationKind::Arrow {
            x1,
            y1,
            x2,
            y2,
            head_size,
        } => {
            let (ax, ay) = to_canvas(*x1, *y1);
            let (bx, by) = to_canvas(*x2, *y2);
            AnnotationShape::Arrow {
                x1: ax,
                y1: ay,
                x2: bx,
                y2: by,
                head: head_size.clamp(0.05, 0.4) as f32,
            }
        }
        _ => return None,
    };

    Some(AnnotationParams {
        shape,
        fill: parse_css_color(&annotation.fill).unwrap_or(recast_color::TRANSPARENT),
        stroke: parse_css_color(&annotation.stroke.color).unwrap_or(recast_color::TRANSPARENT),
        stroke_width: (annotation.stroke.width.max(0.0) as f32) * scale,
        alpha: alpha as f32,
    })
}

/// Hidden dropped, the rest sorted by `(z_index, insertion order)`. A stable
/// sort is what makes equal z values keep the order the editor shows.
pub fn sorted_visible(annotations: &[&Annotation]) -> Vec<usize> {
    let mut indexed: Vec<(usize, &&Annotation)> = annotations
        .iter()
        .enumerate()
        .filter(|(_, a)| !a.hidden)
        .collect();
    indexed.sort_by(|(ai, a), (bi, b)| a.z_index.cmp(&b.z_index).then(ai.cmp(bi)));
    indexed.into_iter().map(|(index, _)| index).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn annotation(json: &str) -> Annotation {
        serde_json::from_str(json).expect("annotation json")
    }

    fn rect_json(extra: &str) -> String {
        format!(
            r#"{{"id":"a1","start":0.0,"end":10.0,{extra}
                "kind":{{"kind":"rect","x":0.2,"y":0.3,"w":0.4,"h":0.2}}}}"#
        )
    }

    fn geometry() -> CanvasGeometry {
        crate::geometry::canvas_geometry(1000, 500, 0.0, None)
    }

    fn dest() -> DestRect {
        DestRect {
            x: 0.0,
            y: 0.0,
            w: 1000.0,
            h: 500.0,
        }
    }

    fn params(a: &Annotation, t: f64) -> Option<AnnotationParams> {
        annotation_params(a, t, geometry(), dest(), Affine2::IDENTITY)
    }

    #[test]
    fn a_rect_maps_its_uv_onto_the_canvas() {
        let a = annotation(&rect_json(""));
        let p = params(&a, 5.0).expect("params");
        assert_eq!(
            p.shape,
            AnnotationShape::Rect {
                x: 200.0,
                y: 150.0,
                w: 400.0,
                h: 100.0,
                radius: 0.0,
            }
        );
    }

    #[test]
    fn a_rect_dragged_backwards_is_normalised_rather_than_disappearing() {
        let a = annotation(
            r#"{"id":"a1","start":0.0,"end":10.0,
                "kind":{"kind":"rect","x":0.6,"y":0.5,"w":-0.4,"h":-0.2}}"#,
        );
        let p = params(&a, 5.0).expect("params");
        assert_eq!(
            p.shape,
            AnnotationShape::Rect {
                x: 200.0,
                y: 150.0,
                w: 400.0,
                h: 100.0,
                radius: 0.0,
            }
        );
    }

    #[test]
    fn an_annotation_outside_its_window_produces_nothing() {
        let a = annotation(&rect_json(""));
        assert!(params(&a, -1.0).is_none());
        assert!(params(&a, 11.0).is_none());
    }

    /// With ramps present the phase clamp happens to return 0 outside the
    /// window anyway. With ramps at zero it does not, so the window guard is the
    /// only thing standing between an expired annotation and a full-opacity draw.
    #[test]
    fn an_annotation_with_no_ramps_still_stops_at_its_window_edges() {
        let a = annotation(&rect_json(r#""rampIn":0.0,"rampOut":0.0,"#));
        assert_eq!(annotation_alpha(&a, -0.1), 0.0);
        assert_eq!(annotation_alpha(&a, 10.1), 0.0);
        assert_eq!(annotation_alpha(&a, 5.0), 1.0);
    }

    #[test]
    fn a_hidden_annotation_produces_nothing() {
        let a = annotation(&rect_json(r#""hidden":true,"#));
        assert!(params(&a, 5.0).is_none());
    }

    #[test]
    fn the_ramps_fade_in_and_out_and_hold_at_full() {
        let a = annotation(&rect_json(r#""rampIn":2.0,"rampOut":2.0,"#));
        assert!(annotation_alpha(&a, 0.0) < 0.01);
        assert!(annotation_alpha(&a, 1.0) > 0.0 && annotation_alpha(&a, 1.0) < 1.0);
        assert!((annotation_alpha(&a, 5.0) - 1.0).abs() < 1e-6);
        assert!(annotation_alpha(&a, 9.0) < 1.0);
    }

    #[test]
    fn each_ramp_caps_at_half_the_span_so_they_cannot_overlap() {
        let a = annotation(&rect_json(r#""rampIn":50.0,"rampOut":50.0,"#));
        assert!((annotation_alpha(&a, 5.0) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn the_master_opacity_multiplies_the_ramp() {
        let a = annotation(&rect_json(r#""opacity":0.5,"#));
        assert!((annotation_alpha(&a, 5.0) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn a_frame_anchored_annotation_ignores_the_zoom() {
        let a = annotation(&rect_json(r#""anchor":"frame","#));
        let zoomed = annotation_params(&a, 5.0, geometry(), dest(), Affine2::zoom(2.0, 0.5, 0.5))
            .expect("params");
        let plain = params(&a, 5.0).expect("params");
        assert_eq!(zoomed.shape, plain.shape);
    }

    #[test]
    fn a_video_anchored_annotation_rides_the_zoom() {
        let a = annotation(&rect_json(""));
        let zoomed = annotation_params(&a, 5.0, geometry(), dest(), Affine2::zoom(2.0, 0.5, 0.5))
            .expect("params");
        let plain = params(&a, 5.0).expect("params");
        assert_ne!(zoomed.shape, plain.shape);
        match zoomed.shape {
            AnnotationShape::Rect { w, .. } => assert!((w - 800.0).abs() < 1e-3, "width {w}"),
            other => panic!("expected a rect, got {other:?}"),
        }
    }

    #[test]
    fn an_ellipse_becomes_a_centre_and_radii() {
        let a = annotation(
            r#"{"id":"a1","start":0.0,"end":10.0,
                "kind":{"kind":"ellipse","x":0.2,"y":0.2,"w":0.4,"h":0.4}}"#,
        );
        match params(&a, 5.0).expect("params").shape {
            AnnotationShape::Ellipse { cx, cy, rx, ry } => {
                assert_eq!((cx, cy), (400.0, 200.0));
                assert_eq!((rx, ry), (200.0, 100.0));
            }
            other => panic!("expected an ellipse, got {other:?}"),
        }
    }

    #[test]
    fn an_arrow_head_is_clamped_to_the_supported_range() {
        let a = annotation(
            r#"{"id":"a1","start":0.0,"end":10.0,
                "kind":{"kind":"arrow","x1":0.1,"y1":0.1,"x2":0.9,"y2":0.9,"headSize":5.0}}"#,
        );
        match params(&a, 5.0).expect("params").shape {
            AnnotationShape::Arrow { head, .. } => assert_eq!(head, 0.4),
            other => panic!("expected an arrow, got {other:?}"),
        }
    }

    #[test]
    fn image_text_and_blur_kinds_are_reported_as_undrawable_rather_than_drawn_wrong() {
        for kind in [
            r#"{"kind":"image","x":0.1,"y":0.1,"w":0.2,"h":0.2,"path":"a.png"}"#,
            r#"{"kind":"text","x":0.1,"y":0.1,"w":0.2,"h":0.2,"content":"hi"}"#,
            r#"{"kind":"blur","x":0.1,"y":0.1,"w":0.2,"h":0.2}"#,
        ] {
            let a = annotation(&format!(
                r#"{{"id":"a1","start":0.0,"end":10.0,"kind":{kind}}}"#
            ));
            assert!(params(&a, 5.0).is_none(), "{kind} should not draw yet");
        }
    }

    #[test]
    fn a_transparent_fill_parses_to_zero_alpha_rather_than_failing() {
        let a = annotation(&rect_json(r#""fill":"transparent","#));
        assert_eq!(params(&a, 5.0).expect("params").fill.a, 0);
    }

    #[test]
    fn z_order_is_stable_within_equal_indices() {
        let a = annotation(&rect_json(r#""zIndex":1,"#));
        let b = annotation(&rect_json(r#""zIndex":0,"#));
        let c = annotation(&rect_json(r#""zIndex":1,"#));
        let hidden = annotation(&rect_json(r#""zIndex":0,"hidden":true,"#));
        let all = vec![&a, &b, &c, &hidden];
        assert_eq!(sorted_visible(&all), vec![1, 0, 2]);
    }
}

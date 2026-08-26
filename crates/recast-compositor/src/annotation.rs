use recast_color::{parse_css_color, Srgba};
use recast_scene::v1::nodes::{Annotation, AnnotationAnchor, AnnotationKind};

use crate::eval::{Affine2, DestRect};
use crate::geometry::CanvasGeometry;

#[derive(Debug, Clone, PartialEq)]
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
    /// Privacy blur over whatever is already composited underneath. `sigma_px`
    /// is a Gaussian standard deviation, matching what CSS `blur()` means in
    /// `paintBlur`, which the preview and the browser export share.
    Blur {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        sigma_px: f32,
        /// Wash laid over the blurred pixels. Transparent for a plain glass
        /// blur below the redaction threshold.
        tint: Srgba,
    },
    /// A decoded asset stretched over the rect. `path` addresses the upload,
    /// matching the host's own per-path image cache, so two annotations sharing
    /// a file share one texture.
    Image {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        radius: f32,
        opacity: f32,
        path: Box<str>,
    },
}

#[derive(Debug, Clone, PartialEq)]
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

/// Gaussian sigma for a blur annotation, in canvas pixels. The 0.12 factor and
/// the shorter-edge reference come from `paintBlur`, so the three renderers
/// agree on how strong a given strength looks.
fn blur_sigma_px(strength: f64, geometry: CanvasGeometry) -> f32 {
    let shorter = geometry.canvas_w.min(geometry.canvas_h) as f64;
    (strength.clamp(0.0, 1.0) * 0.12 * shorter) as f32
}

/// Mirrors `blurTint`. `glass` stays clear until strength passes 0.6, past which
/// a grey wash turns it into a real redaction.
fn blur_tint(variant: &str, tint_color: &str, strength: f64, opacity: f32) -> Srgba {
    let s = strength.clamp(0.0, 1.0);
    let o = opacity.clamp(0.0, 1.0) as f64;
    let alpha = (0.15 + 0.8 * s) * o;
    let with = |color: Srgba, alpha: f64| Srgba {
        a: (alpha.clamp(0.0, 1.0) * 255.0).round() as u8,
        ..color
    };
    match variant {
        "white" => with(Srgba::opaque(255, 255, 255), alpha),
        "black" => with(Srgba::opaque(0, 0, 0), alpha),
        // An unparseable colour draws no wash rather than a black one.
        "color" => match parse_css_color(tint_color) {
            Some(color) => with(color, alpha),
            None => recast_color::TRANSPARENT,
        },
        _ if s > 0.6 => with(Srgba::opaque(128, 128, 128), (s - 0.6) * 0.6 * o),
        _ => recast_color::TRANSPARENT,
    }
}

/// `None` for a kind this pass cannot draw. Only text is left: it reaches the
/// export pre-rasterised as an `Image`, so the compositor never sees a glyph.
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
            let (left, top) = (x.min(x + w), y.min(y + h));
            let (px, py) = to_canvas(left, top);
            let (fx, fy) = to_canvas(left + w.abs(), top + h.abs());
            let (bw, bh) = (fx - px, fy - py);
            AnnotationShape::Blur {
                x: px,
                y: py,
                w: bw,
                h: bh,
                radius: (*radius as f32) * bw.abs().min(bh.abs()),
                sigma_px: blur_sigma_px(*strength, geometry),
                tint: blur_tint(variant, tint_color, *strength, alpha as f32),
            }
        }
        AnnotationKind::Image {
            x,
            y,
            w,
            h,
            path,
            opacity,
            radius,
        } => {
            if path.is_empty() {
                return None;
            }
            let (left, top) = (x.min(x + w), y.min(y + h));
            let (px, py) = to_canvas(left, top);
            let (fx, fy) = to_canvas(left + w.abs(), top + h.abs());
            let (bw, bh) = (fx - px, fy - py);
            AnnotationShape::Image {
                x: px,
                y: py,
                w: bw,
                h: bh,
                radius: (*radius as f32) * bw.abs().min(bh.abs()),
                opacity: opacity.clamp(0.0, 1.0) as f32,
                path: path.as_str().into(),
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

    fn blur(extra: &str) -> Annotation {
        annotation(&format!(
            r#"{{"id":"b1","start":0.0,"end":10.0,
                "kind":{{"kind":"blur","x":0.2,"y":0.3,"w":0.4,"h":0.2{extra}}}}}"#
        ))
    }

    /// Sigma is a fraction of the SHORTER canvas edge, the same reference
    /// `paintBlur` uses, so a given strength reads the same in all three
    /// renderers instead of drifting with the aspect.
    #[test]
    fn blur_strength_becomes_a_sigma_on_the_shorter_edge() {
        match params(&blur(r#","strength":0.5"#), 5.0)
            .expect("params")
            .shape
        {
            // The fixture canvas is 1000x500.
            AnnotationShape::Blur { sigma_px, .. } => {
                assert!((sigma_px - 30.0).abs() < 1e-3, "sigma {sigma_px}")
            }
            other => panic!("expected a blur, got {other:?}"),
        }
    }

    /// The corner radius is a fraction of the RECT's shorter side, not the
    /// canvas: a rounded redaction should keep its shape at any size.
    #[test]
    fn the_blur_corner_radius_follows_the_rect() {
        match params(&blur(r#","radius":0.25"#), 5.0)
            .expect("params")
            .shape
        {
            AnnotationShape::Blur { w, h, radius, .. } => {
                assert_eq!((w, h), (400.0, 100.0));
                assert!((radius - 25.0).abs() < 1e-3, "radius {radius}");
            }
            other => panic!("expected a blur, got {other:?}"),
        }
    }

    /// Glass is a clear blur until the strength slider is pushed into redaction
    /// territory, where a grey wash starts building.
    #[test]
    fn glass_stays_clear_until_the_redaction_threshold() {
        let tint = |extra: &str| match params(&blur(extra), 5.0).expect("params").shape {
            AnnotationShape::Blur { tint, .. } => tint,
            other => panic!("expected a blur, got {other:?}"),
        };
        assert_eq!(tint(r#","strength":0.6"#).a, 0);
        assert!(tint(r#","strength":1.0"#).a > 0);
    }

    #[test]
    fn a_white_wash_scales_its_alpha_with_strength() {
        let tint = |extra: &str| match params(&blur(extra), 5.0).expect("params").shape {
            AnnotationShape::Blur { tint, .. } => tint,
            other => panic!("expected a blur, got {other:?}"),
        };
        let low = tint(r#","variant":"white","strength":0.0"#);
        let high = tint(r#","variant":"white","strength":1.0"#);
        assert_eq!((low.r, low.g, low.b), (255, 255, 255));
        assert_eq!(low.a, 38, "0.15 of 255");
        assert_eq!(high.a, 242, "0.95 of 255");
    }

    /// An unparseable tint draws no wash. Falling back to black would redact
    /// the region the user asked to merely blur.
    #[test]
    fn an_unparseable_tint_colour_washes_nothing() {
        match params(
            &blur(r#","variant":"color","tintColor":"not-a-colour","strength":1.0"#),
            5.0,
        )
        .expect("params")
        .shape
        {
            AnnotationShape::Blur { tint, .. } => assert_eq!(tint.a, 0),
            other => panic!("expected a blur, got {other:?}"),
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
    fn a_text_kind_is_reported_as_undrawable_rather_than_drawn_wrong() {
        for kind in [
            r#"{"kind":"text","x":0.1,"y":0.1,"w":0.2,"h":0.2,"content":"hi"}"#,
            // An image with no path yet: the project is mid-load, not corrupt.
            r#"{"kind":"image","x":0.1,"y":0.1,"w":0.2,"h":0.2}"#,
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

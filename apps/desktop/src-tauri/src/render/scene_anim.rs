//! Per-segment scene animations for export — the video-layer entrance/exit
//! transform (slide / scale), mirrored from the frontend
//! (apps/desktop/src/lib/scenes/{segment-anim,eval}.ts).
//!
//! Like zoom, the animation is evaluated on the continuous post-trim timeline and
//! the tail cut+speed stage re-times it, so nothing here is cut- or speed-aware.
//! For each kept segment we sample the eased transform, merge it into a few linear
//! pieces, and emit FFmpeg `if(gte(t,a)*lt(t,b),ramp,default)` flat-sum expressions
//! — the SAME machinery as the zoom LUT (`fmt_term` / `wrap_flat_sum`), driving the
//! per-frame `overlay=x:y` position and an `eval=frame` `scale` on the video layer.
//!
//! Phase 1 covers the geometric channels (slide, scale, shrink, pop). Opacity
//! (fade) is preview-only for now — FFmpeg has no per-frame alpha expression on
//! `overlay`, so `fade` produces no geometric change here and is skipped.

use super::graph::{fmt_term, wrap_flat_sum, CanvasGeometry};

const SAMPLE_HZ: f64 = 20.0;
const MIN_ANIM_MS: f64 = 100.0;
const MAX_ANIM_MS: f64 = 2000.0;
// Mirror the per-kind defaults in segment-anim.ts — keep in lockstep.
const DEFAULT_SLIDE: f64 = 0.6;
const DEFAULT_SCALE_DELTA: f64 = 0.3;
const DEFAULT_POP_DELTA: f64 = 0.35;
const DEFAULT_ROTATE_DEG: f64 = 15.0;
const ANCHOR_EPS: f64 = 1e-4;
// Mirrors scenes/eval.ts: shorter segments stay static and each ramp caps to this fraction, so fragments never wobble.
const MIN_ANIMATABLE_SEC: f64 = 0.2;
const MAX_SIDE_FRACTION: f64 = 0.4;

/// One side (entrance or exit) of a segment's animation. Mirrors
/// `SceneAnimSpec` on the frontend.
pub use recast_scene::v1::{SceneAnimSpec, SegmentAnim};

/// The video-layer overlay expressions for the whole timeline. `x_expr`/`y_expr`
/// drive `overlay=x:y`; `scale_expr`, when present, drives an `eval=frame` scale
/// on the video layer (about its centre — the overlay expressions already fold in
/// the recentre offset).
#[derive(Debug, Clone)]
pub struct SceneOverlay {
    pub x_expr: String,
    pub y_expr: String,
    pub scale_expr: Option<String>,
    /// Rotation in DEGREES over output `t` (the caller converts to radians).
    pub rotate_expr: Option<String>,
    /// Opacity 0..1 over `geq`'s time var `T`, multiplied into the layer's alpha
    /// plane (fade to background). `None` when nothing fades.
    pub opacity_expr: Option<String>,
}

#[derive(Clone, Copy)]
struct Tf {
    tx: f64,
    ty: f64,
    scale: f64,
    rotate: f64,
    opacity: f64,
}

fn identity() -> Tf {
    Tf {
        tx: 0.0,
        ty: 0.0,
        scale: 1.0,
        rotate: 0.0,
        opacity: 1.0,
    }
}

fn clamp_anim_ms(ms: f64) -> f64 {
    if !ms.is_finite() {
        500.0
    } else {
        ms.clamp(MIN_ANIM_MS, MAX_ANIM_MS)
    }
}

/// Transform for a spec at presence `p` (1 = resting/identity, 0 = fully animated
/// away; may overshoot for bouncy easings). Mirrors `presenceTransform`.
fn presence(spec: &SceneAnimSpec, p: f64) -> Tf {
    let mut t = identity();
    match spec.kind.as_str() {
        "fade" => t.opacity = p.clamp(0.0, 1.0),
        "slide" => {
            let d = spec.intensity.unwrap_or(DEFAULT_SLIDE);
            let off = (1.0 - p) * d;
            match spec.dir.as_deref().unwrap_or("left") {
                "right" => t.tx = off,
                "up" => t.ty = -off,
                "down" => t.ty = off,
                _ => t.tx = -off, // "left" and any unknown
            }
        }
        "scale" | "pop" => {
            let amt = spec.intensity.unwrap_or(if spec.kind == "pop" {
                DEFAULT_POP_DELTA
            } else {
                DEFAULT_SCALE_DELTA
            });
            let start_scale = 1.0 - amt;
            t.scale = start_scale + (1.0 - start_scale) * p;
        }
        "shrink" => {
            let amt = spec.intensity.unwrap_or(DEFAULT_SCALE_DELTA);
            let start_scale = 1.0 + amt;
            t.scale = start_scale + (1.0 - start_scale) * p;
        }
        "rotate" => {
            let deg = spec.intensity.unwrap_or(DEFAULT_ROTATE_DEG);
            t.rotate = (1.0 - p) * deg;
        }
        // Unknown kinds are a no-op (identity).
        _ => {}
    }
    t
}

/// The transform at time `t` within a segment's window `[start, end]`. Mirrors
/// `evalSegmentTransform`: entrance eases 0→1 over `in.durationMs`, exit eases
/// 1→0 over `out.durationMs`, hold between is identity. Each side caps to
/// `MAX_SIDE_FRACTION` of the window and segments shorter than `MIN_ANIMATABLE_SEC`
/// stay static — the anti-wobble guards.
fn eval_segment(anim: &SegmentAnim, t: f64, start: f64, end: f64) -> Tf {
    let win = (end - start).max(0.0);
    if win < MIN_ANIMATABLE_SEC {
        return identity();
    }
    let max_side = win * MAX_SIDE_FRACTION;
    if let Some(a) = &anim.anim_in {
        let d = (clamp_anim_ms(a.duration_ms) / 1000.0).min(max_side);
        if d > 0.0 && t < start + d {
            let phase = ((t - start) / d).clamp(0.0, 1.0);
            return presence(a, a.easing.y(phase as f32) as f64);
        }
    }
    if let Some(a) = &anim.anim_out {
        let d = (clamp_anim_ms(a.duration_ms) / 1000.0).min(max_side);
        if d > 0.0 && t > end - d {
            let phase = ((end - t) / d).clamp(0.0, 1.0);
            return presence(a, a.easing.y(phase as f32) as f64);
        }
    }
    identity()
}

/// Greedy collinear merge of `(t, value)` samples into linear pieces
/// `(ta, va, tb, vb)` — same technique as the zoom LUT's `merge_scale_segments`.
fn merge_linear(samples: &[(f64, f64)], tol: f64) -> Vec<(f64, f64, f64, f64)> {
    let mut out = Vec::new();
    let mut run: Option<(f64, f64, f64, f64)> = None;
    for w in samples.windows(2) {
        let (ta, va) = w[0];
        let (tb, vb) = w[1];
        if tb <= ta {
            continue;
        }
        match run {
            Some((ra, rva, _, _)) => {
                let span = tb - ra;
                let pred = if span > 1e-9 {
                    rva + (vb - rva) * (ta - ra) / span
                } else {
                    va
                };
                if (pred - va).abs() <= tol {
                    run = Some((ra, rva, tb, vb));
                } else {
                    out.push((ra, rva, ta, va));
                    run = Some((ta, va, tb, vb));
                }
            }
            None => run = Some((ta, va, tb, vb)),
        }
    }
    if let Some(r) = run {
        out.push(r);
    }
    out
}

/// Build one flat-sum FFmpeg expression from per-segment samples of a channel.
/// `var` is the filter's time variable (`t` for overlay/scale, `T` for `geq`).
fn build_channel_expr(seg_samples: &[Vec<(f64, f64)>], default_val: f64, var: &str) -> String {
    let tol = 0.002_f64;
    let mut terms = Vec::new();
    for samples in seg_samples {
        for (ta, va, tb, vb) in merge_linear(samples, tol) {
            if let Some(term) = fmt_term(ta, va, tb, vb, default_val, var) {
                terms.push(term);
            }
        }
    }
    // wrap_flat_sum's default is the literal fallback outside every window.
    let default_str = if (default_val - 1.0).abs() < 1e-9 {
        "1"
    } else {
        "0"
    };
    wrap_flat_sum(default_str, terms)
}

/// Build the video-layer overlay expressions for a set of kept segment windows
/// (post-trim seconds, `t=0` at trim_start) and their animations. Returns `None`
/// when nothing animates geometrically, so the caller keeps the static overlay.
pub fn build_scene_overlay(
    windows: &[(f64, f64)],
    trim_start: f64,
    anims: &[SegmentAnim],
    canvas: &CanvasGeometry,
    source_w: u32,
    source_h: u32,
) -> Option<SceneOverlay> {
    if anims.is_empty() {
        return None;
    }

    let mut tx_seg: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut ty_seg: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut sc_seg: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut rot_seg: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut op_seg: Vec<Vec<(f64, f64)>> = Vec::new();
    let mut any_translate = false;
    let mut any_scale = false;
    let mut any_rotate = false;
    let mut any_opacity = false;

    for &(s, e) in windows {
        let anchor = s + trim_start;
        let anim = match anims
            .iter()
            .find(|a| (a.start - anchor).abs() <= ANCHOR_EPS)
        {
            Some(a) => a,
            None => continue,
        };
        let start = s.max(0.0);
        let dur = (e - start).max(0.0);
        if dur <= 0.0 {
            continue;
        }
        let samples = ((dur * SAMPLE_HZ).ceil() as usize).clamp(2, 200);
        let step = dur / samples as f64;
        let mut txs = Vec::with_capacity(samples + 1);
        let mut tys = Vec::with_capacity(samples + 1);
        let mut scs = Vec::with_capacity(samples + 1);
        let mut rots = Vec::with_capacity(samples + 1);
        let mut ops = Vec::with_capacity(samples + 1);
        for i in 0..=samples {
            let t = start + step * i as f64;
            let tf = eval_segment(anim, t, s, e);
            txs.push((t, tf.tx));
            tys.push((t, tf.ty));
            scs.push((t, tf.scale));
            rots.push((t, tf.rotate));
            ops.push((t, tf.opacity));
            if tf.tx.abs() > 1e-6 || tf.ty.abs() > 1e-6 {
                any_translate = true;
            }
            if (tf.scale - 1.0).abs() > 1e-6 {
                any_scale = true;
            }
            if tf.rotate.abs() > 1e-6 {
                any_rotate = true;
            }
            if (tf.opacity - 1.0).abs() > 1e-6 {
                any_opacity = true;
            }
        }
        tx_seg.push(txs);
        ty_seg.push(tys);
        sc_seg.push(scs);
        rot_seg.push(rots);
        op_seg.push(ops);
    }

    if !any_translate && !any_scale && !any_rotate && !any_opacity {
        return None;
    }

    let cw = canvas.canvas_w as f64;
    let ch = canvas.canvas_h as f64;
    let iw = source_w as f64;
    let ih = source_h as f64;
    let vx = canvas.video_x as f64;
    let vy = canvas.video_y as f64;

    let tx_expr = build_channel_expr(&tx_seg, 0.0, "t");
    let ty_expr = build_channel_expr(&ty_seg, 0.0, "t");
    let scale_expr = build_channel_expr(&sc_seg, 1.0, "t");
    let rotate_expr = build_channel_expr(&rot_seg, 0.0, "t");
    // geq evaluates its expression with the uppercase time variable `T`.
    let opacity_expr = build_channel_expr(&op_seg, 1.0, "T");

    // Overlay top-left is the video origin plus translate, minus the recentre for a scale about the video's own centre.
    let x_expr = format!("{vx}+({tx_expr})*{cw}-{iw}*(({scale_expr})-1)/2");
    let y_expr = format!("{vy}+({ty_expr})*{ch}-{ih}*(({scale_expr})-1)/2");

    Some(SceneOverlay {
        x_expr,
        y_expr,
        scale_expr: if any_scale { Some(scale_expr) } else { None },
        rotate_expr: if any_rotate { Some(rotate_expr) } else { None },
        opacity_expr: if any_opacity {
            Some(opacity_expr)
        } else {
            None
        },
    })
}

#[cfg(test)]
mod tests {
    use serde::Deserialize;

    use super::super::easing::Easing;
    use super::*;

    // The fixture stores easing as a 4-element array, so re-map it and check Rust against the exact same TS cases.
    #[derive(Deserialize)]
    struct RawSpec {
        kind: String,
        #[serde(rename = "durationMs")]
        duration_ms: f64,
        easing: [f32; 4],
        #[serde(default)]
        dir: Option<String>,
        #[serde(default)]
        intensity: Option<f64>,
    }
    impl RawSpec {
        fn into_spec(self) -> SceneAnimSpec {
            SceneAnimSpec {
                kind: self.kind,
                duration_ms: self.duration_ms,
                easing: Easing {
                    x1: self.easing[0],
                    y1: self.easing[1],
                    x2: self.easing[2],
                    y2: self.easing[3],
                },
                dir: self.dir,
                intensity: self.intensity,
            }
        }
    }

    #[test]
    fn geometric_transform_matches_shared_parity_fixture() {
        // The same file scenes/eval.test.ts asserts against, proving the Rust and TS evaluators agree.
        let raw = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../packages/editor/src/lib/scenes/__fixtures__/scene-parity.json"
        ));
        // Parse loosely: easing is an array in the fixture.
        let json: serde_json::Value = serde_json::from_str(raw).unwrap();
        for case in json["cases"].as_array().unwrap() {
            let name = case["name"].as_str().unwrap();
            let window = &case["window"];
            let start = window[0].as_f64().unwrap();
            let end = window[1].as_f64().unwrap();
            let to_spec = |v: &serde_json::Value| -> Option<SceneAnimSpec> {
                if v.is_null() {
                    None
                } else {
                    let raw: RawSpec = serde_json::from_value(v.clone()).unwrap();
                    Some(raw.into_spec())
                }
            };
            let anim = SegmentAnim {
                start,
                anim_in: to_spec(&case["in"]),
                anim_out: to_spec(&case["out"]),
            };
            for s in case["samples"].as_array().unwrap() {
                let t = s["t"].as_f64().unwrap();
                let tf = eval_segment(&anim, t, start, end);
                let ex_tx = s["translateX"].as_f64().unwrap();
                let ex_ty = s["translateY"].as_f64().unwrap();
                let ex_sc = s["scale"].as_f64().unwrap();
                let ex_rot = s["rotate"].as_f64().unwrap_or(0.0);
                let ex_op = s["opacity"].as_f64().unwrap();
                assert!(
                    (tf.opacity - ex_op).abs() < 1e-6,
                    "{name} @t={t}: opacity {} != {ex_op}",
                    tf.opacity
                );
                assert!(
                    (tf.tx - ex_tx).abs() < 1e-6,
                    "{name} @t={t}: tx {} != {ex_tx}",
                    tf.tx
                );
                assert!(
                    (tf.ty - ex_ty).abs() < 1e-6,
                    "{name} @t={t}: ty {} != {ex_ty}",
                    tf.ty
                );
                assert!(
                    (tf.scale - ex_sc).abs() < 1e-6,
                    "{name} @t={t}: scale {} != {ex_sc}",
                    tf.scale
                );
                assert!(
                    (tf.rotate - ex_rot).abs() < 1e-6,
                    "{name} @t={t}: rotate {} != {ex_rot}",
                    tf.rotate
                );
            }
        }
    }

    #[test]
    fn fade_produces_opacity_expr_only() {
        let canvas = CanvasGeometry {
            canvas_w: 1920,
            canvas_h: 1080,
            video_x: 0,
            video_y: 0,
            video_w: 1920,
            video_h: 1080,
            padding_px: 0,
            comp_x: 0,
            comp_y: 0,
            comp_w: 1920,
            comp_h: 1080,
        };
        let anims = vec![SegmentAnim {
            start: 0.0,
            anim_in: Some(SceneAnimSpec {
                kind: "fade".into(),
                duration_ms: 500.0,
                easing: Easing::LINEAR,
                dir: None,
                intensity: None,
            }),
            anim_out: None,
        }];
        let ov = build_scene_overlay(&[(0.0, 4.0)], 0.0, &anims, &canvas, 1920, 1080)
            .expect("fade is an active animation");
        // Fade drives only alpha, with no scale or rotate stages, and its LUT uses geq's uppercase time variable.
        assert!(ov.scale_expr.is_none());
        assert!(ov.rotate_expr.is_none());
        let op = ov.opacity_expr.expect("opacity expr");
        assert!(op.contains("gte(T,") && !op.contains("gte(t,"));
    }

    #[test]
    fn tiny_segment_is_guarded_to_no_overlay() {
        // A sub-MIN_ANIMATABLE_SEC window must produce no overlay expressions, so the export stays on the static path.
        let canvas = CanvasGeometry {
            canvas_w: 1920,
            canvas_h: 1080,
            video_x: 0,
            video_y: 0,
            video_w: 1920,
            video_h: 1080,
            padding_px: 0,
            comp_x: 0,
            comp_y: 0,
            comp_w: 1920,
            comp_h: 1080,
        };
        let anims = vec![SegmentAnim {
            start: 0.0,
            anim_in: Some(SceneAnimSpec {
                kind: "slide".into(),
                duration_ms: 500.0,
                easing: Easing::LINEAR,
                dir: Some("left".into()),
                intensity: None,
            }),
            anim_out: None,
        }];
        assert!(build_scene_overlay(&[(0.0, 0.15)], 0.0, &anims, &canvas, 1920, 1080).is_none());
    }

    #[test]
    fn empty_anims_produce_no_overlay() {
        let canvas = CanvasGeometry {
            canvas_w: 1920,
            canvas_h: 1080,
            video_x: 0,
            video_y: 0,
            video_w: 1920,
            video_h: 1080,
            padding_px: 0,
            comp_x: 0,
            comp_y: 0,
            comp_w: 1920,
            comp_h: 1080,
        };
        assert!(build_scene_overlay(&[(0.0, 5.0)], 0.0, &[], &canvas, 1920, 1080).is_none());
    }
}

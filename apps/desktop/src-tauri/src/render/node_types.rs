use serde::{Deserialize, Serialize};

use crate::render::easing::Easing;

fn default_ramp_duration() -> f64 {
    0.35
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrimNode {
    pub start: f64,
    pub end: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackgroundNode {
    pub background_type: String,
    pub value: String,
    pub blur: f64,
    pub padding: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CursorNode {
    pub enabled: bool,
    pub size: f64,
    pub smoothing: f64,
    pub highlight_clicks: bool,
    pub highlight_color: String,
    pub highlight_opacity: f64,
    pub hide_when_idle: bool,
    pub idle_timeout: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomRegion {
    pub start: f64,
    pub end: f64,
    pub scale: f64,
    /// Curve for the `start → start + ramp_in` window. Missing in legacy
    /// projects; serde default falls back to CSS `ease`.
    #[serde(default)]
    pub ease_in: Easing,
    /// Curve for the `end - ramp_out → end` window.
    #[serde(default)]
    pub ease_out: Easing,
    /// Seconds the zoom takes to reach full scale from the region's start.
    #[serde(default = "default_ramp_duration")]
    pub ramp_in: f64,
    /// Seconds the zoom takes to fall back to 1.0 before the region's end.
    #[serde(default = "default_ramp_duration")]
    pub ramp_out: f64,
}

impl ZoomRegion {
    /// Usable ramp durations for this region: never exceed half the region's
    /// length each, so a short region still has a hold phase (even if it's a
    /// single instant). Handles negative / zero durations by clamping to 0.
    pub fn clamped_ramps(&self) -> (f64, f64) {
        let duration = (self.end - self.start).max(0.0);
        let half = duration * 0.5;
        let ramp_in = self.ramp_in.max(0.0).min(half);
        let ramp_out = self.ramp_out.max(0.0).min(half);
        (ramp_in, ramp_out)
    }

    /// Eased scale at time `t` (seconds on the project timeline). Returns
    /// 1.0 outside the region, `self.scale` during the hold, and a bezier-
    /// shaped ramp in/out of the scale on the two edges.
    pub fn scale_at(&self, t: f64) -> f64 {
        if t <= self.start || t >= self.end {
            return 1.0;
        }
        let (ramp_in, ramp_out) = self.clamped_ramps();
        let hold_start = self.start + ramp_in;
        let hold_end = self.end - ramp_out;
        let target = self.scale;
        let (curve, phase) = if t < hold_start {
            let phase = if ramp_in > 0.0 {
                ((t - self.start) / ramp_in).clamp(0.0, 1.0)
            } else {
                1.0
            };
            (self.ease_in, phase)
        } else if t > hold_end {
            let phase = if ramp_out > 0.0 {
                ((self.end - t) / ramp_out).clamp(0.0, 1.0)
            } else {
                1.0
            };
            (self.ease_out, phase)
        } else {
            return target;
        };
        1.0 + (target - 1.0) * curve.y(phase as f32) as f64
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ZoomNode {
    pub regions: Vec<ZoomRegion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RenderNode {
    Trim(TrimNode),
    Background(BackgroundNode),
    Cursor(CursorNode),
    Zoom(ZoomNode),
}

// ── Annotations ────────────────────────────────────────────────────────
//
// Phase 1 ships `rect` and `ellipse`. `kind` is a tagged union so future
// arrow/polygon/text/image variants slot in without breaking serialisation
// of existing projects. All positions are in video UV space (0..1) so they
// track zoom/crop without re-projection.

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationStroke {
    /// Stroke width in UV space (width=0.004 ≈ 2 px at 1080p).
    pub width: f64,
    /// CSS colour string. `"transparent"` disables stroke.
    pub color: String,
}

impl Default for AnnotationStroke {
    fn default() -> Self {
        Self {
            width: 0.004,
            color: "#3b82f6".into(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum AnnotationKind {
    Rect {
        /// UV top-left corner.
        x: f64,
        y: f64,
        /// UV width / height. Can be negative while the user drags — UI flips.
        w: f64,
        h: f64,
        /// Corner radius in UV space. 0 = sharp.
        #[serde(default)]
        radius: f64,
    },
    Ellipse {
        /// UV top-left of the bounding box.
        x: f64,
        y: f64,
        w: f64,
        h: f64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Annotation {
    pub id: String,
    /// Seconds on the project timeline when the annotation starts fading in.
    pub start: f64,
    /// Seconds when the annotation finishes fading out.
    pub end: f64,
    /// Seconds of fade-in. Clamped to half the region's duration by the
    /// evaluator, same split-ramp semantics as Focus.
    #[serde(default = "default_anno_ramp")]
    pub ramp_in: f64,
    #[serde(default = "default_anno_ramp")]
    pub ramp_out: f64,
    #[serde(default)]
    pub ease_in: Easing,
    #[serde(default)]
    pub ease_out: Easing,
    /// Optional stroke applied to all shape kinds.
    #[serde(default)]
    pub stroke: AnnotationStroke,
    /// CSS fill colour (with alpha via rgba(...) / #rrggbbaa). `"transparent"` disables fill.
    #[serde(default = "default_anno_fill")]
    pub fill: String,
    pub kind: AnnotationKind,
}

fn default_anno_ramp() -> f64 {
    0.20
}

fn default_anno_fill() -> String {
    "rgba(59,130,246,0.20)".into()
}

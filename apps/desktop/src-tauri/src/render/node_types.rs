use serde::{Deserialize, Serialize};

use crate::render::easing::Easing;

fn default_ramp_duration() -> f64 {
    0.35
}

fn default_zoom_center() -> f64 {
    0.5
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShadowSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub blur: f64,
    #[serde(default)]
    pub spread: f64,
    #[serde(default)]
    pub offset_y: f64,
    #[serde(default)]
    pub opacity: f64,
    #[serde(default = "default_shadow_color")]
    pub color: String,
}

impl Default for ShadowSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            blur: 40.0,
            spread: 0.0,
            offset_y: 24.0,
            opacity: 40.0,
            color: default_shadow_color(),
        }
    }
}

fn default_shadow_color() -> String {
    "#000000".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioSettings {
    #[serde(default = "default_audio_volume")]
    pub volume: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: default_audio_volume(),
            muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
        }
    }
}

fn default_audio_volume() -> f64 {
    100.0
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatermarkSettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub image_path: String,
    #[serde(default)]
    pub image_src: String,
    #[serde(default = "default_watermark_opacity")]
    pub opacity: f64,
    #[serde(default = "default_watermark_scale")]
    pub scale: f64,
    #[serde(default = "default_watermark_position")]
    pub position: String,
    #[serde(default = "default_watermark_inset")]
    pub inset: f64,
}

impl Default for WatermarkSettings {
    fn default() -> Self {
        Self {
            enabled: false,
            image_path: String::new(),
            image_src: String::new(),
            opacity: default_watermark_opacity(),
            scale: default_watermark_scale(),
            position: default_watermark_position(),
            inset: default_watermark_inset(),
        }
    }
}

fn default_watermark_opacity() -> f64 {
    70.0
}

fn default_watermark_scale() -> f64 {
    18.0
}

fn default_watermark_position() -> String {
    "bottom-right".into()
}

fn default_watermark_inset() -> f64 {
    24.0
}

fn default_camera_shape() -> String {
    "rounded".into()
}

fn default_camera_animation_preset() -> String {
    "soft".into()
}

fn default_camera_motion_source() -> String {
    "manual".into()
}

fn default_camera_mirror() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraPlacement {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl Default for CameraPlacement {
    fn default() -> Self {
        Self {
            x: 0.72,
            y: 0.08,
            width: 0.22,
            height: 0.22,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraMotionSegment {
    pub start: f64,
    pub end: f64,
    pub from_x: f64,
    pub from_y: f64,
    pub from_width: f64,
    pub from_height: f64,
    pub to_x: f64,
    pub to_y: f64,
    pub to_width: f64,
    pub to_height: f64,
    #[serde(default)]
    pub ease_in: Easing,
    #[serde(default)]
    pub ease_out: Easing,
    #[serde(default = "default_camera_motion_source")]
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraOverlaySettings {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_camera_mirror")]
    pub mirror: bool,
    #[serde(default = "default_camera_shape")]
    pub shape: String,
    #[serde(default = "default_camera_corner_radius")]
    pub corner_radius: f64,
    #[serde(default = "default_camera_animation_preset")]
    pub animation_preset: String,
    #[serde(default)]
    pub default_placement: CameraPlacement,
    #[serde(default)]
    pub motion_segments: Vec<CameraMotionSegment>,
}

fn default_camera_corner_radius() -> f64 {
    0.16
}

impl Default for CameraOverlaySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mirror: default_camera_mirror(),
            shape: default_camera_shape(),
            corner_radius: default_camera_corner_radius(),
            animation_preset: default_camera_animation_preset(),
            default_placement: CameraPlacement::default(),
            motion_segments: Vec::new(),
        }
    }
}

impl CameraOverlaySettings {
    #[allow(dead_code)]
    pub fn placement_at(&self, t: f64) -> CameraPlacement {
        let mut current = self.default_placement.clone();
        for segment in &self.motion_segments {
            if t <= segment.start {
                break;
            }
            if t >= segment.end {
                current = CameraPlacement {
                    x: segment.to_x,
                    y: segment.to_y,
                    width: segment.to_width,
                    height: segment.to_height,
                };
                continue;
            }

            let duration = (segment.end - segment.start).max(1e-6);
            let phase = ((t - segment.start) / duration).clamp(0.0, 1.0);
            let eased = segment.ease_in.y(phase as f32) as f64;
            return CameraPlacement {
                x: segment.from_x + (segment.to_x - segment.from_x) * eased,
                y: segment.from_y + (segment.to_y - segment.from_y) * eased,
                width: segment.from_width + (segment.to_width - segment.from_width) * eased,
                height: segment.from_height + (segment.to_height - segment.from_height) * eased,
            };
        }
        current
    }
}

#[cfg(test)]
mod tests {
    use super::{CameraMotionSegment, CameraOverlaySettings, CameraPlacement};

    #[test]
    fn camera_overlay_uses_default_placement_before_motion() {
        let overlay = CameraOverlaySettings::default();
        assert_eq!(overlay.placement_at(0.0), CameraPlacement::default());
    }

    #[test]
    fn camera_overlay_interpolates_inside_motion_segment() {
        let mut overlay = CameraOverlaySettings::default();
        overlay.motion_segments.push(CameraMotionSegment {
            start: 0.0,
            end: 2.0,
            from_x: 0.1,
            from_y: 0.2,
            from_width: 0.2,
            from_height: 0.2,
            to_x: 0.5,
            to_y: 0.6,
            to_width: 0.3,
            to_height: 0.3,
            ease_in: Default::default(),
            ease_out: Default::default(),
            source: "live-recorded".into(),
        });

        let at_mid = overlay.placement_at(1.0);
        assert!(at_mid.x > 0.1 && at_mid.x < 0.5);
        assert!(at_mid.y > 0.2 && at_mid.y < 0.6);
        assert!(at_mid.width > 0.2 && at_mid.width < 0.3);
    }

    #[test]
    fn camera_overlay_uses_last_segment_after_motion() {
        let mut overlay = CameraOverlaySettings::default();
        overlay.motion_segments.push(CameraMotionSegment {
            start: 0.0,
            end: 1.0,
            from_x: 0.1,
            from_y: 0.2,
            from_width: 0.2,
            from_height: 0.2,
            to_x: 0.4,
            to_y: 0.5,
            to_width: 0.25,
            to_height: 0.25,
            ease_in: Default::default(),
            ease_out: Default::default(),
            source: "manual".into(),
        });

        let after = overlay.placement_at(3.0);
        assert_eq!(after.x, 0.4);
        assert_eq!(after.y, 0.5);
        assert_eq!(after.width, 0.25);
    }
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
    /// Frame padding as percent of the shorter source edge.
    pub padding: f64,
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
    /// UV-space focus centre X. 0.5 reproduces legacy center-crop behaviour.
    #[serde(default = "default_zoom_center")]
    pub center_x: f64,
    /// UV-space focus centre Y.
    #[serde(default = "default_zoom_center")]
    pub center_y: f64,
    /// Preview motion-blur strength 0..1.
    ///
    /// **Preview-only by design** — the WebGL preview applies a radial 7-tap
    /// blur whose direction tracks the per-frame zoom velocity. FFmpeg has
    /// no faithful equivalent: `tmix` is direction-agnostic temporal
    /// averaging that ghosts every frame (not just transitions); `boxblur`/
    /// `gblur` only accept a static sigma set at filter init time. Shipping
    /// `tmix` would over-blur every frame and look worse than the
    /// no-motion-blur baseline, so the export silently ignores this field.
    /// The slider remains useful for preview iteration; users who want
    /// smoother export motion should tune `easeIn`/`easeOut` instead.
    #[serde(default)]
    pub motion_blur: f64,
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

//  Annotations
//
// Phase 1 ships `rect` and `ellipse`. `kind` is a tagged union so future
// arrow/polygon/text/image variants slot in without breaking serialisation
// of existing projects. All positions are in video UV space (0..1) so they
// track zoom/crop without re-projection.

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationStrokeStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationStroke {
    /// Stroke width in UV space (width=0.004 ≈ 2 px at 1080p).
    pub width: f64,
    /// CSS colour string. `"transparent"` disables stroke.
    pub color: String,
    /// Stroke pattern. Defaults to `Solid` so v1 projects keep loading
    /// without their stored stroke object growing a new field.
    #[serde(default)]
    pub style: AnnotationStrokeStyle,
}

impl Default for AnnotationStroke {
    fn default() -> Self {
        Self {
            width: 0.004,
            color: "#3b82f6".into(),
            style: AnnotationStrokeStyle::Solid,
        }
    }
}

/// Optional preview-only glow / soft shadow. Stored on the wire in v2 so the
/// preview and any future Rust glow renderer agree on the shape, but the
/// current Rust pipeline ignores it (the editor surfaces a "preview only"
/// banner so the user knows export drops the glow).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationGlow {
    pub color: String,
    /// Blur radius in UV (≈ 0..0.05).
    pub blur: f64,
    pub opacity: f64,
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
    /// Stroke-only directional callout. The head is drawn at (x2, y2).
    Arrow {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        /// Head length as a fraction of line length, clamped 0.05..0.4.
        #[serde(default = "default_arrow_head_size")]
        head_size: f64,
    },
    /// PNG/JPG overlay composited at the UV rect. Used both for the user's
    /// Image tool and as the export substitute for text annotations after
    /// the WebView rasterizes them at export prep.
    Image {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        path: String,
        #[serde(default = "default_image_opacity")]
        opacity: f64,
    },
    /// Privacy/focus blur applied to the live frame underneath the rect.
    /// `strength` (0..1) drives a separable box-blur radius; `variant`
    /// chooses optional tint colour applied over the blurred pixels.
    Blur {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default = "default_blur_strength")]
        strength: f64,
        #[serde(default = "default_blur_variant")]
        variant: String,
        #[serde(default = "default_blur_tint", rename = "tintColor")]
        tint_color: String,
        #[serde(default)]
        radius: f64,
    },
    /// Unknown / unsupported variant. Deserialization fallback so the export
    /// pipeline doesn't fail if the JS side sends a kind Rust can't render
    /// (e.g. `text` annotations that weren't pre-rasterized to PNG). Skipped
    /// silently in the draw loop.
    #[serde(other)]
    Unsupported,
}

fn default_blur_strength() -> f64 {
    0.5
}
fn default_blur_variant() -> String {
    "glass".into()
}
fn default_blur_tint() -> String {
    "#000000".into()
}

fn default_arrow_head_size() -> f64 {
    0.15
}
fn default_image_opacity() -> f64 {
    1.0
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

    // v2 envelope — every field defaulted so v1 projects keep loading. Order
    // matches the TS `Annotation` interface in `editor-store.svelte.ts`.
    /// User-renamed label. Falls back to a kind-derived label in the UI.
    #[serde(default)]
    pub name: Option<String>,
    /// Stacking order; higher draws later (on top). v1 projects start at 0.
    #[serde(default)]
    pub z_index: i32,
    /// When true the canvas overlay ignores pointer hits.
    #[serde(default)]
    pub locked: bool,
    /// When true the renderer skips the annotation entirely.
    #[serde(default)]
    pub hidden: bool,
    /// Master opacity (0..1) multiplied with the split-ramp evaluator output.
    #[serde(default = "default_opacity_unit")]
    pub opacity: f64,
    /// Optional preview-only glow. Carried on the wire so future Rust passes
    /// don't have to bump the version again, but the current draw path ignores it.
    #[serde(default)]
    pub glow: Option<AnnotationGlow>,
}

fn default_anno_ramp() -> f64 {
    0.20
}

fn default_anno_fill() -> String {
    "rgba(59,130,246,0.20)".into()
}

fn default_opacity_unit() -> f64 {
    1.0
}

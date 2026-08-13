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
    // Per-source gains/mutes, layered under the master. Defaulted so projects
    // saved before these existed deserialize to unity (no silent gain change).
    #[serde(default = "default_audio_volume")]
    pub system_volume: f64,
    #[serde(default)]
    pub system_muted: bool,
    #[serde(default = "default_audio_volume")]
    pub mic_volume: f64,
    #[serde(default)]
    pub mic_muted: bool,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    /// EBU R128 loudness normalize on the final mix (export only). Default off so
    /// existing projects export byte-identical.
    #[serde(default)]
    pub normalize_loudness: bool,
}

impl Default for AudioSettings {
    fn default() -> Self {
        Self {
            volume: default_audio_volume(),
            muted: false,
            system_volume: default_audio_volume(),
            system_muted: false,
            mic_volume: default_audio_volume(),
            mic_muted: false,
            fade_in: 0.0,
            fade_out: 0.0,
            normalize_loudness: false,
        }
    }
}

fn default_audio_volume() -> f64 {
    100.0
}

/// Where an {@link AudioClip}'s audio comes from. Mirrors `AudioClipSource` in
/// `src/lib/audio/music.ts`. Internally tagged on `kind` to match the TS union.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum AudioClipSource {
    Local {
        path: String,
    },
    #[serde(rename_all = "camelCase")]
    Provider {
        #[serde(default)]
        provider_id: String,
        #[serde(default)]
        track_id: String,
        asset_path: String,
        #[serde(default)]
        attribution: Option<String>,
        #[serde(default)]
        license: Option<String>,
    },
}

impl AudioClipSource {
    /// The local file to decode/encode, whichever source kind it is.
    pub fn asset_path(&self) -> &str {
        match self {
            AudioClipSource::Local { path } => path,
            AudioClipSource::Provider { asset_path, .. } => asset_path,
        }
    }

    /// Credit line for licenses that require attribution (CC-BY); None for local.
    pub fn attribution(&self) -> Option<&str> {
        match self {
            AudioClipSource::Provider {
                attribution: Some(a),
                ..
            } if !a.trim().is_empty() => Some(a.trim()),
            _ => None,
        }
    }
}

/// `voice` = the recording's own detached audio; `music` = anything added on top.
/// Mirrors `AudioClipRole` in `src/lib/audio/music.ts`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum AudioClipRole {
    #[default]
    Music,
    Voice,
}

/// Music / extra-audio laid on the OUTPUT timeline. Mirrors `AudioClip` in
/// `src/lib/audio/music.ts`; every field defaulted for forward-compat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioClip {
    pub id: String,
    pub source: AudioClipSource,
    #[serde(default)]
    pub role: AudioClipRole,
    #[serde(default)]
    pub start_output_sec: f64,
    #[serde(default)]
    pub offset_sec: f64,
    #[serde(default)]
    pub duration_sec: f64,
    #[serde(default = "default_audio_volume")]
    pub gain: f64,
    #[serde(default)]
    pub muted: bool,
    #[serde(default)]
    pub fade_in: f64,
    #[serde(default)]
    pub fade_out: f64,
    #[serde(default, rename = "loop")]
    pub looping: bool,
    #[serde(default)]
    pub ducking: bool,
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

/// A camera position pinned at an original-recording time. The effective base
/// placement glides (eased) between consecutive keyframes — the per-cut motion.
/// Mirrors the TS `CameraKeyframe`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CameraKeyframe {
    pub at_sec: f64,
    pub placement: CameraPlacement,
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
    #[serde(default = "default_camera_zoom_follow")]
    pub zoom_follow: bool,
    #[serde(default = "default_camera_zoom_follow_strength")]
    pub zoom_follow_strength: f64,
    /// Seconds the grow/shrink takes to ramp in/out (its own transition timing).
    #[serde(default = "default_camera_zoom_follow_duration")]
    pub zoom_follow_duration: f64,
    /// Easing for the grow/shrink transition.
    #[serde(default = "default_camera_keyframe_easing")]
    pub zoom_follow_easing: Easing,
    #[serde(default)]
    pub default_placement: CameraPlacement,
    /// Camera moves recorded live. Write-only: `loadRenderState` folds them into
    /// `keyframes` (the model preview and export both read) and clears this.
    #[serde(default)]
    pub motion_segments: Vec<CameraMotionSegment>,
    /// Per-cut position keyframes (original-time). Empty → static default_placement.
    #[serde(default)]
    pub keyframes: Vec<CameraKeyframe>,
    /// Easing for the glide between keyframes.
    #[serde(default = "default_camera_keyframe_easing")]
    pub keyframe_easing: Easing,
    /// Drop-shadow strength 0..1 (0 = none). Scales blur + offset + opacity.
    #[serde(default = "default_camera_shadow")]
    pub shadow: f64,
}

fn default_camera_keyframe_easing() -> Easing {
    Easing {
        x1: 0.42,
        y1: 0.0,
        x2: 0.58,
        y2: 1.0,
    }
}

fn default_camera_shadow() -> f64 {
    0.35
}

fn default_camera_corner_radius() -> f64 {
    0.16
}

fn default_camera_zoom_follow() -> bool {
    true
}

fn default_camera_zoom_follow_strength() -> f64 {
    0.6
}

fn default_camera_zoom_follow_duration() -> f64 {
    0.4
}

impl Default for CameraOverlaySettings {
    fn default() -> Self {
        Self {
            enabled: false,
            mirror: default_camera_mirror(),
            shape: default_camera_shape(),
            corner_radius: default_camera_corner_radius(),
            animation_preset: default_camera_animation_preset(),
            zoom_follow: default_camera_zoom_follow(),
            zoom_follow_strength: default_camera_zoom_follow_strength(),
            zoom_follow_duration: default_camera_zoom_follow_duration(),
            zoom_follow_easing: default_camera_keyframe_easing(),
            default_placement: CameraPlacement::default(),
            motion_segments: Vec::new(),
            keyframes: Vec::new(),
            keyframe_easing: default_camera_keyframe_easing(),
            shadow: default_camera_shadow(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Annotation, AnnotationAnchor, AnnotationKind, CameraMotionSegment, CameraOverlaySettings,
        CameraPlacement,
    };

    // Guards the IPC contract: the frontend sends camelCase keys, and the export
    // pipeline must read `anchor` + the image `radius`/`stroke` it sends. A key
    // mismatch here would silently drop the feature at export (as `segmentAnims`
    // once did), so these assert the exact wire shape survives deserialization.
    #[test]
    fn annotation_anchor_and_image_controls_survive_frontend_json() {
        let raw = r##"{
            "id": "a1",
            "start": 1.0,
            "end": 3.0,
            "anchor": "frame",
            "stroke": { "width": 0.006, "color": "#ff0000", "style": "solid" },
            "kind": {
                "kind": "image",
                "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.4,
                "path": "logo.png", "opacity": 0.9, "radius": 0.25
            }
        }"##;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        assert_eq!(a.anchor, AnnotationAnchor::Frame);
        assert!((a.stroke.width - 0.006).abs() < 1e-9);
        assert_eq!(a.stroke.color, "#ff0000");
        match a.kind {
            AnnotationKind::Image {
                radius,
                opacity,
                path,
                ..
            } => {
                assert!((radius - 0.25).abs() < 1e-9);
                assert!((opacity - 0.9).abs() < 1e-9);
                assert_eq!(path, "logo.png");
            }
            other => panic!("expected image kind, got {other:?}"),
        }
    }

    #[test]
    fn annotation_anchor_defaults_to_video_when_absent() {
        // Older projects / omitted field must default to Video, never fail.
        let raw = r#"{
            "id": "a2", "start": 0.0, "end": 1.0,
            "kind": { "kind": "rect", "x": 0.0, "y": 0.0, "w": 0.5, "h": 0.5, "radius": 0.0 }
        }"#;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        assert_eq!(a.anchor, AnnotationAnchor::Video);
    }

    #[test]
    fn annotation_anchor_serializes_to_lowercase() {
        // Round-trip: save/load + preview all key off "video"/"frame".
        assert_eq!(
            serde_json::to_value(AnnotationAnchor::Frame).unwrap(),
            serde_json::json!("frame")
        );
        assert_eq!(
            serde_json::to_value(AnnotationAnchor::Video).unwrap(),
            serde_json::json!("video")
        );
    }

    // Rust has no text renderer (text reaches export pre-rasterized as an
    // image), but a saved text annotation must survive the typed load
    // round-trip (deserialize→reserialize) or every field is lost — it would
    // otherwise fall through to `Unsupported` and reload as a broken kind.
    #[test]
    fn text_annotation_round_trips_and_keeps_its_kind() {
        let raw = r##"{
            "id": "t1", "start": 0.0, "end": 2.0,
            "kind": {
                "kind": "text", "x": 0.1, "y": 0.2, "w": 0.3, "h": 0.1,
                "content": "Hello", "fontFamily": "Inter", "fontSize": 0.05,
                "fontWeight": 600, "color": "#ffffff", "align": "center",
                "lineHeight": 1.2
            }
        }"##;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        match &a.kind {
            AnnotationKind::Text {
                content,
                font_family,
                line_height,
                ..
            } => {
                assert_eq!(content, "Hello");
                assert_eq!(font_family, "Inter");
                assert!((line_height - 1.2).abs() < 1e-9);
            }
            other => panic!("expected text kind, got {other:?}"),
        }
        // Re-serialize: the tag must stay "text" (not "unsupported") and the
        // camelCase field keys must be preserved for the JS load map.
        let v = serde_json::to_value(&a).unwrap();
        assert_eq!(v["kind"]["kind"], "text");
        assert_eq!(v["kind"]["content"], "Hello");
        assert_eq!(v["kind"]["fontFamily"], "Inter");
    }

    // The frontend sends `headSize`; without a per-variant rename_all it would
    // never map and the export would silently use the default arrowhead size.
    #[test]
    fn arrow_head_size_maps_from_camelcase() {
        let raw = r#"{"id":"a","start":0.0,"end":1.0,
            "kind":{"kind":"arrow","x1":0.1,"y1":0.1,"x2":0.5,"y2":0.5,"headSize":0.3}}"#;
        let a: Annotation = serde_json::from_str(raw).unwrap();
        match a.kind {
            AnnotationKind::Arrow { head_size, .. } => {
                assert!(
                    (head_size - 0.3).abs() < 1e-9,
                    "headSize should map, got {head_size}"
                );
            }
            other => panic!("expected arrow, got {other:?}"),
        }
        assert_eq!(serde_json::to_value(&a).unwrap()["kind"]["headSize"], 0.3);
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
    /// Non-destructive mute: when true the region is excluded from the export
    /// (and the preview), but kept in the project file. Absent in older
    /// projects → visible.
    #[serde(default)]
    pub hidden: bool,
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
    /// JS-side fields (`id`, `source`) the export doesn't read but must
    /// round-trip — without this they'd be dropped when the load path
    /// re-serializes the render state back to the editor.
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
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

/// Optional glow / soft shadow. Rendered in export for rect, ellipse, image,
/// and (rasterized) text via `draw_shape_shadow`/`draw_image_shadow`; arrow
/// glow is still preview-only.
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
    // `rename_all` on the ENUM only renames variant NAMES, not fields inside a
    // variant — so a multi-word field needs its own rename_all (or explicit
    // rename) or the frontend's `headSize` never maps and export silently uses
    // the default (and it's lost on reload).
    #[serde(rename_all = "camelCase")]
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
        /// Absolute file path or `data:` URL. Defaulted so a corrupt/partial
        /// project missing the key skips this one annotation (empty path fails
        /// to decode → skipped) instead of aborting the whole export deserialize.
        #[serde(default)]
        path: String,
        #[serde(default = "default_image_opacity")]
        opacity: f64,
        /// Corner radius as a fraction of the shorter side (0..0.5). 0 = sharp.
        #[serde(default)]
        radius: f64,
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
    /// Text overlay. Only needs to ROUND-TRIP through save/load — the export
    /// receives text pre-rasterized as an `Image` (rasterize-text.ts), so the
    /// draw loop skips this variant. Without it, a saved text annotation
    /// deserializes to `Unsupported` on reload and loses every field (data loss).
    /// Fields mirror the TS `text` kind. `rename_all` HERE (on the variant) is
    /// required — the enum-level `rename_all` only covers variant names, so
    /// without this `fontFamily`/`fontSize`/etc. wouldn't map.
    #[serde(rename_all = "camelCase")]
    Text {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        #[serde(default)]
        content: String,
        #[serde(default)]
        font_family: String,
        #[serde(default)]
        font_size: f64,
        #[serde(default)]
        font_weight: f64,
        #[serde(default)]
        color: String,
        #[serde(default)]
        align: String,
        #[serde(default)]
        line_height: f64,
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
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
    /// Optional glow / soft shadow. Rendered in export for rect/ellipse
    /// (`draw_shape_shadow`) and image/text-as-image (`draw_image_shadow`);
    /// arrow glow is preview-only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub glow: Option<AnnotationGlow>,
    /// What the annotation is pinned to. `Video` (default) tracks the zoomed
    /// video content; `Frame` pins it to the output frame (no zoom).
    #[serde(default)]
    pub anchor: AnnotationAnchor,
}

/// Coordinate space an annotation is anchored to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub enum AnnotationAnchor {
    #[default]
    Video,
    Frame,
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

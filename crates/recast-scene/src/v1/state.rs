use serde::{Deserialize, Serialize};

use super::nodes::{
    Annotation, AudioClip, AudioSettings, CameraOverlaySettings, ShadowSettings, ZoomRegion,
};

fn default_bounce_speed_ms() -> f64 {
    220.0
}

// Defaults mirror the editor's cursor-smoothing presets in editor-store.svelte.ts (true, 80 ms).
fn default_snap_to_clicks() -> bool {
    true
}
/// Lane master switches default on: a project saved before they existed had
/// every lane applying.
fn enabled() -> bool {
    true
}
fn default_snap_window_ms() -> f64 {
    80.0
}

/// A removed range on the timeline (a silence cut or a manual cut), in
/// original-recording seconds. The export drops these via `select`/`aselect`.
/// Unknown JS-side fields (`id`, `source`) round-trip through `extra`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CutRange {
    pub start: f64,
    pub end: f64,
    #[serde(flatten, default)]
    pub extra: serde_json::Map<String, serde_json::Value>,
}

/// Per-segment speed override, anchored to a kept segment's ORIGINAL start time
/// (see apps/desktop/src/lib/timeline/segment-speed.ts). A segment with no entry
/// plays at 1×. Read by the export pipeline to warp the kept stream's timing.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentSpeed {
    pub start: f64,
    pub speed: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderState {
    pub trim_start: f64,
    pub trim_end: f64,
    pub background_type: String,
    pub background_value: String,
    pub background_blur: f64,
    /// Frame padding as percent of the shorter source edge (0..20).
    pub padding: f64,
    /// Final-canvas aspect: "source" (default) or one of the preset labels ("16:9", "9:16", "1:1", "1.91:1"). Anything we don't recognise falls back to source-matched.
    #[serde(default)]
    pub output_aspect: Option<String>,
    /// Corner rounding as a percentage (0..50) of the shorter video edge.
    #[serde(default)]
    pub border_radius: f64,
    pub cursor_enabled: bool,
    pub cursor_size: f64,
    pub cursor_smoothing: f64,
    /// Anchor the smoothed path to exact click x/y inside the snap window so
    /// presses stay pixel-perfect. Mirrors `cursorSnapToClicks` in the editor;
    /// must be read here so the export's smoothing matches the preview's.
    #[serde(default = "default_snap_to_clicks")]
    pub cursor_snap_to_clicks: bool,
    /// Half-width (ms) of the cosine click-snap ramp. Mirrors
    /// `cursorSnapWindowMs`.
    #[serde(default = "default_snap_window_ms")]
    pub cursor_snap_window_ms: f64,
    pub cursor_highlight_clicks: bool,
    pub cursor_highlight_color: String,
    pub cursor_highlight_opacity: f64,
    pub cursor_hide_when_idle: bool,
    pub cursor_idle_timeout: f64,
    /// Motion-blur strength (0..1). Drives a velocity-proportional alpha trail
    /// in the export compositor (0 = no trail).
    #[serde(default)]
    pub cursor_motion_blur: f64,
    /// Click-bounce amplitude (0..5). Modulates the cursor sprite scale around
    /// each mouse-down event for a satisfying "press" feel.
    #[serde(default)]
    pub cursor_click_bounce: f64,
    /// Bounce/squash duration in milliseconds.
    #[serde(default = "default_bounce_speed_ms")]
    pub cursor_bounce_speed_ms: f64,
    /// Idle sway amplitude (0..1). Adds a subtle sinusoidal wobble during
    /// slow-motion sections so cursors don't feel mechanically rigid.
    #[serde(default)]
    pub cursor_sway: f64,
    pub zoom_regions: Vec<ZoomRegion>,
    /// The zoom lane's master switch. Off leaves the regions authored but
    /// stops them applying, which is what the editor preview does.
    #[serde(default = "enabled")]
    pub focus_enabled: bool,
    /// The annotation lane's master switch, same shape as `focus_enabled`.
    #[serde(default = "enabled")]
    pub annotations_enabled: bool,
    /// Caption look. Absent in a project with no captions; `enabled` on the
    /// style itself is the lane switch, so there is no separate flag.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub caption_style: Option<recast_captions::CaptionStyle>,
    /// User-accepted silence/manual cuts removed from the timeline.
    #[serde(default)]
    pub cuts: Vec<CutRange>,
    /// Split markers (original-recording seconds) dividing the kept clip into addressable segments. Editor-only on their own; here they bound the segments that `segment_speeds` retimes.
    #[serde(default)]
    pub split_points: Vec<f64>,
    /// Per-segment speed overrides (empty = every segment plays at 1×).
    #[serde(default)]
    pub segment_speeds: Vec<SegmentSpeed>,
    /// Per-segment entrance and exit transforms on the video layer, anchored to a segment's original start; empty means every segment is static.
    /// The frontend serialises these under `segmentAnims`, and the key must match or the export silently drops every animation to passthrough.
    #[serde(rename = "segmentAnims", default)]
    pub scene_animations: Vec<super::anim::SegmentAnim>,
    /// Annotation overlays (rect/ellipse/arrow/image/blur; text arrives
    /// pre-rasterized as image). Composited in export by the cursor-overlay
    /// pass (`render/cursor_export.rs`) and the FFmpeg blur filter.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Drop shadow cast by the video rect, rendered in both the preview and the export.
    /// The export rasterises it once as a canvas-sized PNG, baking blur, spread, offset and colour in, so the filter chain needs no expression evaluation.
    #[serde(default)]
    pub shadow: ShadowSettings,
    #[serde(default)]
    pub audio_settings: AudioSettings,
    /// Music / extra-audio clips on the output timeline (mixed in at export).
    #[serde(default)]
    pub music_clips: Vec<AudioClip>,
    #[serde(default)]
    pub camera_overlay: CameraOverlaySettings,
    // Populated by the JS export trigger for a non-dot style; the soft-dot path is unchanged when these are None.
    #[serde(default)]
    pub cursor_sprite_rest: Option<String>,
    #[serde(default)]
    pub cursor_sprite_press: Option<String>,
    #[serde(default)]
    pub cursor_sprite_right_press: Option<String>,
    #[serde(default)]
    pub cursor_sprite_drag: Option<String>,
    #[serde(default)]
    pub cursor_sprite_hotspot_rest: Option<[f64; 2]>,
    #[serde(default)]
    pub cursor_sprite_hotspot_press: Option<[f64; 2]>,
    #[serde(default)]
    pub cursor_sprite_hotspot_right_press: Option<[f64; 2]>,
    #[serde(default)]
    pub cursor_sprite_hotspot_drag: Option<[f64; 2]>,
    #[serde(default)]
    pub cursor_sprite_size_px: Option<f64>,
    /// Reshapes the interpolation parameter between two captured samples. Was a
    /// passthrough key, so the export ignored it while the preview applied it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cursor_motion_easing: Option<crate::v1::easing::Easing>,
    /// Catch-all for JS-only settings Rust never reads, slurped by `#[serde(flatten)]` and re-emitted on serialisation.
    /// Without it every undeclared field would be dropped on reopen, resetting the user's tweaks to defaults.
    #[serde(flatten, default)]
    pub passthrough: serde_json::Map<String, serde_json::Value>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            trim_start: 0.0,
            trim_end: 0.0,
            focus_enabled: true,
            annotations_enabled: true,
            caption_style: None,
            background_type: "color".into(),
            background_value: "#111111".into(),
            background_blur: 0.0,
            padding: 0.0,
            output_aspect: None,
            border_radius: 0.0,
            cursor_enabled: true,
            cursor_size: 3.0,
            cursor_smoothing: 50.0,
            cursor_snap_to_clicks: default_snap_to_clicks(),
            cursor_snap_window_ms: default_snap_window_ms(),
            cursor_highlight_clicks: true,
            cursor_highlight_color: "#3b82f6".into(),
            cursor_highlight_opacity: 40.0,
            cursor_hide_when_idle: false,
            cursor_idle_timeout: 3.0,
            cursor_motion_blur: 0.0,
            cursor_click_bounce: 0.0,
            cursor_bounce_speed_ms: default_bounce_speed_ms(),
            cursor_sway: 0.0,
            zoom_regions: Vec::new(),
            cuts: Vec::new(),
            split_points: Vec::new(),
            segment_speeds: Vec::new(),
            scene_animations: Vec::new(),
            annotations: Vec::new(),
            shadow: ShadowSettings::default(),
            audio_settings: AudioSettings::default(),
            music_clips: Vec::new(),
            camera_overlay: CameraOverlaySettings::default(),
            cursor_sprite_rest: None,
            cursor_sprite_press: None,
            cursor_sprite_right_press: None,
            cursor_sprite_drag: None,
            cursor_sprite_hotspot_rest: None,
            cursor_sprite_hotspot_press: None,
            cursor_sprite_hotspot_right_press: None,
            cursor_sprite_hotspot_drag: None,
            cursor_sprite_size_px: None,
            cursor_motion_easing: None,
            passthrough: serde_json::Map::new(),
        }
    }
}

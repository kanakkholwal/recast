use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::node_types::{
    Annotation, AudioSettings, BackgroundNode, CameraOverlaySettings, CursorNode, RenderNode,
    ShadowSettings, TrimNode, WatermarkSettings, ZoomNode, ZoomRegion,
};

fn default_bounce_speed_ms() -> f64 {
    220.0
}

// Defaults mirror the editor's cursor-smoothing presets (see
// editor-store.svelte.ts: snapToClicks/snapWindowMs default true / 80 ms).
fn default_snap_to_clicks() -> bool {
    true
}
fn default_snap_window_ms() -> f64 {
    80.0
}

/// A removed range on the timeline (a silence cut or a manual cut), in
/// original-recording seconds. The export drops these via `select`/`aselect`.
/// Unknown JS-side fields (`id`, `source`) round-trip through `extra`.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SegmentSpeed {
    pub start: f64,
    pub speed: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenderState {
    pub trim_start: f64,
    pub trim_end: f64,
    pub background_type: String,
    pub background_value: String,
    pub background_blur: f64,
    /// Frame padding as percent of the shorter source edge (0..20).
    pub padding: f64,
    /// Final-canvas aspect: "source" (default) or one of the preset
    /// labels ("16:9", "9:16", "1:1", "1.91:1"). Anything we don't
    /// recognise falls back to source-matched.
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
    /// User-accepted silence/manual cuts removed from the timeline.
    #[serde(default)]
    pub cuts: Vec<CutRange>,
    /// Split markers (original-recording seconds) dividing the kept clip into
    /// addressable segments. Editor-only on their own; here they bound the
    /// segments that `segment_speeds` retimes.
    #[serde(default)]
    pub split_points: Vec<f64>,
    /// Per-segment speed overrides (empty = every segment plays at 1×).
    #[serde(default)]
    pub segment_speeds: Vec<SegmentSpeed>,
    /// Per-segment scene animations — entrance/exit transforms on the video
    /// layer, anchored to a segment's original start (empty = every segment
    /// static). Read by the export to build the video-layer overlay LUT. The
    /// frontend serialises these under `segmentAnims` (see the editor store); the
    /// key must match or the export silently drops every animation to passthrough.
    #[serde(rename = "segmentAnims", default)]
    pub scene_animations: Vec<crate::render::scene_anim::SegmentAnim>,
    /// Annotation overlays (rect/ellipse/arrow/image/blur; text arrives
    /// pre-rasterized as image). Composited in export by the cursor-overlay
    /// pass (`render/cursor_export.rs`) and the FFmpeg blur filter.
    #[serde(default)]
    pub annotations: Vec<Annotation>,
    /// Drop shadow cast by the video rect.
    ///
    /// Rendered in both the WebGL preview and the export. On export, the
    /// shadow is rasterised once as a canvas-sized RGBA PNG by
    /// `render::mask_export::render_drop_shadow_mask` and overlaid onto the
    /// background by `build_export_plan_with` before the video composite.
    /// This bakes `blur`, `spread`, `offset_y`, `opacity`, and `color` into
    /// the static PNG — no time-varying parameters are involved, so the
    /// FFmpeg filter chain stays free of expression evaluation here.
    #[serde(default)]
    pub shadow: ShadowSettings,
    #[serde(default)]
    pub audio_settings: AudioSettings,
    /// Music / extra-audio clips on the output timeline (mixed in at export).
    #[serde(default)]
    pub music_clips: Vec<crate::render::node_types::AudioClip>,
    #[serde(default)]
    pub watermark_settings: WatermarkSettings,
    #[serde(default)]
    pub camera_overlay: CameraOverlaySettings,
    // Hybrid-raster cursor sprite. Populated by the JS export trigger
    // when the active style is non-`dot`; the soft-dot path is unchanged
    // when these are `None`.
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
    /// Catch-all for any JS-only settings (e.g. `cursorStyle`,
    /// `layoutMode`, `lastAppliedPresetId`, `cursorMotionEasing`,
    /// `cursorSnapToClicks`, `cursorSnapWindowMs`, `autoZoomEnabled`,
    /// `autoZoomApplied`) that JS owns but Rust never reads. The Rust
    /// load path deserialises `edits.json` through this struct and then
    /// re-serialises it back to JS — without this catch-all every field
    /// not declared above would be silently dropped on a project reopen,
    /// resetting the user's tweaks to defaults. `#[serde(flatten)]` slurps
    /// all unrecognised keys at this level into the map and emits them
    /// on serialisation, so JS-only settings round-trip cleanly without
    /// every new editor toggle needing a mirror Rust field.
    #[serde(flatten, default)]
    pub passthrough: serde_json::Map<String, serde_json::Value>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            trim_start: 0.0,
            trim_end: 0.0,
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
            watermark_settings: WatermarkSettings::default(),
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
            passthrough: serde_json::Map::new(),
        }
    }
}

/// Final-canvas geometry, mirroring `lib/canvas-geometry.ts` exactly. The
/// preview and the export must agree on the same numbers — if they
/// diverge the rendered file won't match what the user previews.
#[derive(Debug, Clone, Copy)]
pub struct CanvasGeometry {
    pub canvas_w: u32,
    pub canvas_h: u32,
    pub video_x: u32,
    pub video_y: u32,
    pub video_w: u32,
    pub video_h: u32,
    pub padding_px: u32,
    pub comp_x: u32,
    pub comp_y: u32,
    pub comp_w: u32,
    pub comp_h: u32,
}

/// Parse the OutputAspect tag into a width/height ratio. `None` keeps
/// the canvas aligned to source dims (the v1 default).
fn parse_aspect_ratio(label: Option<&str>) -> Option<f64> {
    match label.unwrap_or("source") {
        "16:9" => Some(16.0 / 9.0),
        "9:16" => Some(9.0 / 16.0),
        "1:1" => Some(1.0),
        "1.91:1" => Some(1.91),
        _ => None,
    }
}

pub fn compute_canvas_geometry(
    src_w: u32,
    src_h: u32,
    padding_pct: f64,
    output_aspect: Option<&str>,
) -> CanvasGeometry {
    let pct = padding_pct.clamp(0.0, 20.0);
    let shorter = src_w.min(src_h) as f64;
    let padding_px = ((shorter * pct) / 100.0).round() as u32;

    let comp_w = src_w + padding_px * 2;
    let comp_h = src_h + padding_px * 2;

    let mut canvas_w = comp_w;
    let mut canvas_h = comp_h;
    if let Some(target) = parse_aspect_ratio(output_aspect) {
        if comp_w > 0 && comp_h > 0 {
            let comp_aspect = comp_w as f64 / comp_h as f64;
            if comp_aspect > target {
                // Comp is wider than target → extend HEIGHT.
                canvas_h = ((comp_w as f64) / target).round() as u32;
            } else if comp_aspect < target {
                // Comp is narrower → extend WIDTH.
                canvas_w = ((comp_h as f64) * target).round() as u32;
            }
        }
    }

    // Even alignment so H.264 / pad filter behave.
    canvas_w = (canvas_w + 1) & !1;
    canvas_h = (canvas_h + 1) & !1;

    let comp_x = canvas_w.saturating_sub(comp_w) / 2;
    let comp_y = canvas_h.saturating_sub(comp_h) / 2;
    let video_x = comp_x + padding_px;
    let video_y = comp_y + padding_px;

    CanvasGeometry {
        canvas_w,
        canvas_h,
        video_x,
        video_y,
        video_w: src_w,
        video_h: src_h,
        padding_px,
        comp_x,
        comp_y,
        comp_w,
        comp_h,
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SourceVideoMetadata {
    pub width: u32,
    pub height: u32,
    /// Source frame rate. The generated background source (`color=`) MUST be
    /// pinned to this — otherwise FFmpeg defaults the generator to 25 fps and,
    /// because the background is the BASE of the composite `overlay`, the whole
    /// export inherits 25 fps. A 60 fps recording then gets frame-dropped to 25,
    /// which judders every motion (most visibly under a zoom). See
    /// `build_color_background_filter`.
    pub fps: f64,
}

#[derive(Debug, Clone)]
pub struct ExportPlan {
    pub extra_inputs: Vec<PathBuf>,
    pub filter_complex: Option<String>,
    pub video_map: String,
}

#[derive(Debug, Clone)]
pub struct RenderGraph {
    pub nodes: Vec<RenderNode>,
}

impl RenderGraph {
    pub fn from_state(state: &RenderState) -> Self {
        Self {
            nodes: vec![
                RenderNode::Trim(TrimNode {
                    start: state.trim_start,
                    end: state.trim_end,
                }),
                RenderNode::Background(BackgroundNode {
                    background_type: state.background_type.clone(),
                    value: state.background_value.clone(),
                    blur: state.background_blur,
                    padding: state.padding.max(0.0),
                }),
                RenderNode::Cursor(CursorNode {
                    enabled: state.cursor_enabled,
                    size: state.cursor_size,
                    smoothing: state.cursor_smoothing,
                    highlight_clicks: state.cursor_highlight_clicks,
                    highlight_color: state.cursor_highlight_color.clone(),
                    highlight_opacity: state.cursor_highlight_opacity,
                    hide_when_idle: state.cursor_hide_when_idle,
                    idle_timeout: state.cursor_idle_timeout,
                }),
                RenderNode::Zoom(ZoomNode {
                    regions: state.zoom_regions.clone(),
                }),
            ],
        }
    }

    pub fn trim_range(&self) -> (f64, f64) {
        self.nodes
            .iter()
            .find_map(|node| match node {
                RenderNode::Trim(trim) => Some((trim.start, trim.end)),
                _ => None,
            })
            .unwrap_or((0.0, 0.0))
    }

    #[allow(clippy::too_many_arguments)] // render-plan builder: many independent knobs
    pub fn build_export_plan_with(
        &self,
        source: SourceVideoMetadata,
        static_root: &Path,
        background_input_index: usize,
        asset_cache_dir: Option<&Path>,
        border_radius_mask: Option<PathBuf>,
        drop_shadow_mask: Option<PathBuf>,
        gradient_image: Option<PathBuf>,
        canvas: CanvasGeometry,
        scene: Option<&crate::render::scene_anim::SceneOverlay>,
    ) -> Result<ExportPlan> {
        let background = self.nodes.iter().find_map(|node| match node {
            RenderNode::Background(background) => Some(background),
            _ => None,
        });
        let zoom = self.nodes.iter().find_map(|node| match node {
            RenderNode::Zoom(zoom) => Some(zoom),
            _ => None,
        });

        // Canvas geometry is computed by the caller so the same value
        // feeds the cursor overlay PNG and drop-shadow PNG. video_x/y
        // already include any letterbox offset from an aspect preset.
        let canvas_width = canvas.canvas_w;
        let canvas_height = canvas.canvas_h;
        let video_x = canvas.video_x;
        let video_y = canvas.video_y;
        let _ = background.map(|n| n.padding); // ack — read through canvas now
                                               // Zoom region times are stored in PROJECT-timeline seconds, but the
                                               // FFmpeg expression evaluator's `t` is OUTPUT-stream time, which is
                                               // reset to 0 by the input-side `-ss <trim_start>` we emit in
                                               // `export_video`. If we don't subtract the trim offset here, the LUT
                                               // fires at timeline-t inside the output stream — which, with any
                                               // trim, is past the output's end, so the zoom never visibly applies.
                                               // Without trim the offset is 0 and the behaviour is unchanged.
        let trim_start = self.trim_range().0.max(0.0);
        let zoom_filter = zoom
            .map(|node| build_zoom_filter(node, source, trim_start))
            .filter(|value: &String| !value.is_empty());

        // The mask, when present, occupies the first extra_input slot so its
        // input index is deterministic (= background_input_index). The
        // background image (if any) shifts to the next slot.
        let mut extra_inputs: Vec<PathBuf> = Vec::new();
        let mask_input_index = border_radius_mask.as_ref().map(|_| background_input_index);
        if let Some(path) = border_radius_mask {
            extra_inputs.push(path);
        }
        let bg_image_input_index = background_input_index + extra_inputs.len();
        // Drop-shadow PNG slot is reserved up front so its index is known
        // before the bg image is conditionally pushed; the actual push (if
        // any) happens below, AFTER the bg image, so existing
        // `cursor_input_index = 1 + extra_inputs.len()` math stays correct.
        // `shadow_input_index` is `None` when the caller didn't supply a
        // shadow PNG; the filter chain below treats that as "no shadow stage".
        let mut shadow_input_index: Option<usize> = None;

        // Build the chain that produces the source-video label `[video0]`.
        // When neither zoom nor mask are present, the source can be referenced
        // directly as `[0:v]` (saves a filter pass).
        //
        // For the mask paths we MUST normalize pixel formats: alphamerge
        // expects the main input to already carry an alpha plane (yuva420p)
        // and the mask input to be a single-plane gray image. Without these
        // explicit `format=` conversions FFmpeg tends to negotiate yuv420p
        // (no alpha) on the main input, at which point alphamerge silently
        // outputs a fully-transparent stream — the visual symptom is a black
        // background showing through with only the cursor overlay visible.
        let mut prelude_segments: Vec<String> = Vec::new();
        let mut video_label: String = match (zoom_filter.as_ref(), mask_input_index) {
            (None, None) => "[0:v]".into(),
            (Some(zoom_filter), None) => {
                prelude_segments.push(format!("[0:v]{zoom_filter}[video0]"));
                "[video0]".into()
            }
            (None, Some(mask_idx)) => {
                prelude_segments.push(format!(
                    "[0:v]format=yuva420p[video0pre];[{mask_idx}:v]format=gray[mask0];[video0pre][mask0]alphamerge[video0]"
                ));
                "[video0]".into()
            }
            (Some(zoom_filter), Some(mask_idx)) => {
                prelude_segments.push(format!(
                    "[0:v]{zoom_filter},format=yuva420p[video0pre];[{mask_idx}:v]format=gray[mask0];[video0pre][mask0]alphamerge[video0]"
                ));
                "[video0]".into()
            }
        };

        // Scene entrance/exit animation on the video layer only. `scale_expr`, when
        // present, resizes the layer per frame (about its centre — the overlay
        // position folds in the recentre); the overlay `x/y` expressions below then
        // reposition it. Absent → the static overlay path is byte-identical to the
        // no-animation output. Mirrors scenes/eval.ts; see render::scene_anim.
        if let Some(scale_expr) = scene.and_then(|s| s.scale_expr.as_ref()) {
            prelude_segments.push(format!(
                "{video_label}scale=w='iw*({scale_expr})':h='ih*({scale_expr})':eval=frame[videoScene]"
            ));
            video_label = "[videoScene]".into();
        }
        // Rotation spins the card about its centre; `c=none` leaves the exposed
        // corners transparent (needs alpha), keeping the frame size so the overlay
        // recentre math above is unaffected. `a` re-evaluates per frame.
        if let Some(rot_expr) = scene.and_then(|s| s.rotate_expr.as_ref()) {
            prelude_segments.push(format!(
                "{video_label}format=yuva420p,rotate=a='({rot_expr})*PI/180':c=none:ow=iw:oh=ih[videoSceneRot]"
            ));
            video_label = "[videoSceneRot]".into();
        }
        // Fade to background: multiply the layer's alpha plane by the opacity LUT
        // so the background shows through (overlay does the blend). `geq`'s time
        // variable is `T`; colour planes pass through untouched. geq is per-pixel
        // (offline-only) and runs only when a fade exists, so non-fade exports are
        // unaffected. Composes with the rounded-corner/rotate alpha already present.
        if let Some(op_expr) = scene.and_then(|s| s.opacity_expr.as_ref()) {
            prelude_segments.push(format!(
                "{video_label}format=yuva420p,geq=lum='p(X,Y)':cb='p(X,Y)':cr='p(X,Y)':a='p(X,Y)*({op_expr})'[videoSceneFade]"
            ));
            video_label = "[videoSceneFade]".into();
        }
        // The overlay position: expression-driven when animating, else the static
        // `video_x:video_y` (identical to the pre-scene output).
        let overlay_pos = match scene {
            Some(s) => format!("x='{}':y='{}'", s.x_expr, s.y_expr),
            None => format!("{video_x}:{video_y}"),
        };

        // Resolve the wallpaper/image bg path up-front (without pushing yet)
        // so we know whether a bg-image input slot will be allocated; that
        // determines the shadow-input slot index, which is then baked into
        // the filter strings before any extra_inputs are pushed.
        let resolved_bg_image = match background {
            Some(bg) if matches!(bg.background_type.as_str(), "wallpaper" | "image") => {
                resolve_background_path(&bg.value, static_root, asset_cache_dir)
            }
            // Gradients are pre-rasterised to a PNG by the caller and composited
            // exactly like an image — so the export matches the WebGL preview
            // instead of collapsing to a flat fallback color.
            Some(bg) if bg.background_type == "gradient" => gradient_image.clone(),
            _ => None,
        };
        let will_push_bg_image = resolved_bg_image.is_some();
        if drop_shadow_mask.is_some() {
            shadow_input_index =
                Some(background_input_index + extra_inputs.len() + will_push_bg_image as usize);
        }

        let filter_complex = match background {
            Some(background)
                if matches!(
                    background.background_type.as_str(),
                    "wallpaper" | "image" | "gradient"
                ) =>
            {
                if resolved_bg_image.is_some() {
                    let mut segments = prelude_segments.clone();
                    // Gradients render edge-to-edge and must NOT be blurred (the
                    // preview shader doesn't blur them); blur is an image-only
                    // finishing control.
                    let blur_sigma = if background.background_type == "gradient" {
                        0.0
                    } else {
                        (background.blur / 8.0).max(0.0)
                    };
                    segments.push(format!(
                        "[{bg_image_input_index}:v]scale={canvas_width}:{canvas_height}:force_original_aspect_ratio=increase,crop={canvas_width}:{canvas_height},boxblur={blur_sigma}[bg0]"
                    ));
                    let bg_label = compose_shadow_stage(
                        &mut segments,
                        shadow_input_index,
                        canvas.comp_x,
                        canvas.comp_y,
                    );
                    segments.push(format!(
                        "{bg_label}{video_label}overlay={overlay_pos}[vout]"
                    ));
                    Some(segments.join(";"))
                } else {
                    build_color_background_filter(
                        background,
                        prelude_segments.clone(),
                        &video_label,
                        canvas_width,
                        canvas_height,
                        &overlay_pos,
                        canvas.comp_x,
                        canvas.comp_y,
                        shadow_input_index,
                        source.fps,
                    )
                }
            }
            Some(background) => build_color_background_filter(
                background,
                prelude_segments.clone(),
                &video_label,
                canvas_width,
                canvas_height,
                &overlay_pos,
                canvas.comp_x,
                canvas.comp_y,
                shadow_input_index,
                source.fps,
            ),
            None => {
                if prelude_segments.is_empty() {
                    None
                } else {
                    // Source is `[video0]`; surface it as `[vout]` so the
                    // outer pipeline always maps a labelled stream.
                    let mut segments = prelude_segments.clone();
                    segments.push(format!("{video_label}null[vout]"));
                    Some(segments.join(";"))
                }
            }
        };

        // Now that filter strings are built (and reference the eventual
        // shadow input index), push the actual extra inputs in the
        // committed order: bg_image then drop_shadow.
        if let Some(path) = resolved_bg_image {
            extra_inputs.push(path);
        }
        if let Some(path) = drop_shadow_mask {
            extra_inputs.push(path);
        }

        let requires_map = filter_complex.is_some();

        Ok(ExportPlan {
            extra_inputs,
            filter_complex,
            video_map: if requires_map {
                "[vout]".into()
            } else {
                "0:v:0".into()
            },
        })
    }
}

#[allow(clippy::too_many_arguments)] // filter builder: many independent geometry/color inputs
fn build_color_background_filter(
    background: &BackgroundNode,
    prelude_segments: Vec<String>,
    video_label: &str,
    canvas_width: u32,
    canvas_height: u32,
    overlay_pos: &str,
    shadow_overlay_x: u32,
    shadow_overlay_y: u32,
    shadow_input_index: Option<usize>,
    fps: f64,
) -> Option<String> {
    let color = match background.background_type.as_str() {
        "color" => normalize_color(&background.value),
        "gradient" => gradient_fallback_color(&background.value),
        _ => "#111111".into(),
    };

    // Pin the generator to the source frame rate. Without `:r=` FFmpeg defaults
    // `color=` to 25 fps; since this is the BASE of the composite `overlay`, the
    // entire export would inherit 25 fps and frame-drop a 60 fps recording into
    // a juddery mess (very visible under a zoom). Fall back to 60 for a bogus
    // metadata value rather than emitting an invalid rate.
    let rate = if fps.is_finite() && fps >= 1.0 {
        fps
    } else {
        60.0
    };
    let mut segments = prelude_segments;
    segments.push(format!(
        "color=c={color}:s={canvas_width}x{canvas_height}:r={rate}[bg0]"
    ));
    let bg_label = compose_shadow_stage(
        &mut segments,
        shadow_input_index,
        shadow_overlay_x,
        shadow_overlay_y,
    );
    segments.push(format!(
        "{bg_label}{video_label}overlay={overlay_pos}[vout]"
    ));
    Some(segments.join(";"))
}

/// When a drop-shadow PNG is supplied, append the two extra filter segments
/// that overlay it on top of the freshly-emitted `[bg0]` stage and produce
/// the `[bg]` label the video composite consumes. Returns the label that the
/// next stage should use as its background — `[bg]` when shadow is present,
/// `[bg0]` otherwise (the latter is a label rename, no extra filter pass).
///
/// The shadow PNG is sized to comp dims (= source + padding × 2), not the
/// final canvas. We overlay it at the comp's (x, y) offset inside the
/// canvas so an aspect-changing preset still drops the shadow under the
/// source video and not into the letterbox bars.
fn compose_shadow_stage(
    segments: &mut Vec<String>,
    shadow_input_index: Option<usize>,
    overlay_x: u32,
    overlay_y: u32,
) -> &'static str {
    match shadow_input_index {
        Some(idx) => {
            // `format=rgba` normalises the shadow input — the PNG already
            // carries an alpha plane, but ffmpeg sometimes negotiates a
            // non-alpha pixel format on the decoder side which would make
            // the overlay opaque.
            segments.push(format!("[{idx}:v]format=rgba[shadow]"));
            segments.push(format!("[bg0][shadow]overlay={overlay_x}:{overlay_y}[bg]"));
            "[bg]"
        }
        None => "[bg0]",
    }
}

fn build_zoom_filter(node: &ZoomNode, source: SourceVideoMetadata, time_offset: f64) -> String {
    if node.regions.is_empty() {
        return String::new();
    }

    // Pre-sample each region's curve. FFmpeg's expression evaluator can't
    // call our Rust bezier solver, but a dense piecewise-linear LUT at 20 Hz
    // is visually indistinguishable from the real curve.
    //
    // `time_offset` (= trim_start) shifts the LUT so its t-values are in
    // OUTPUT-stream coordinates rather than project-timeline coordinates;
    // see `build_export_plan_with` for the rationale.
    //
    // Filter shape — IMPORTANT:
    //   `scale=w='iw*Z(t)':h='ih*Z(t)':eval=frame, crop=W:H:x='X(t)':y='Y(t)'`
    //
    // We deliberately do NOT use the more obvious `crop=w='iw/Z':h='ih/Z',
    // scale=W:H` form, because **ffmpeg's `crop` filter evaluates `w` and
    // `h` only ONCE at filter init**, where `t = 0`. With the LUT default
    // returning `iw`/`ih` outside any region, that one-time evaluation
    // resolves to the source dimensions and the crop is a fixed identity for
    // the whole export — zoom never visibly applies. `scale=eval=frame`
    // re-evaluates per frame, and `crop` with literal `w/h` (the constant
    // source dimensions) doesn't hit the init-only limitation; its `x` and
    // `y` are evaluated per frame regardless. This was the actual root cause
    // of "zoom is missing in exported videos" — verified by pixel-diffing
    // FFmpeg outputs of both filter shapes against an identity baseline.
    // Skip hidden regions (non-destructive mute) and regions whose entire
    // timeline window precedes `trim_start` (their LUT entries would all
    // have negative output-t and never fire).
    let visible: Vec<&ZoomRegion> = node
        .regions
        .iter()
        .filter(|region| !region.hidden && region.end > time_offset)
        .collect();
    let samples_per_region: Vec<Vec<ZoomSample>> = disjoint_zoom_windows(&visible, time_offset)
        .into_iter()
        .map(|(idx, start, end)| sample_region(visible[idx], source, time_offset, (start, end)))
        .collect();

    // If filtering left us with nothing, skip the prelude entirely.
    if samples_per_region.iter().all(|s| s.is_empty()) {
        return String::new();
    }

    // Three time-varying expressions, ALL built on the SAME merged scale
    // breakpoints (see `build_zoom_exprs`):
    //   z_expr — multiplicative zoom factor, default 1.0 outside regions.
    //   x_expr — crop top-left X in POST-SCALE absolute pixels, default 0.
    //   y_expr — crop top-left Y in POST-SCALE absolute pixels, default 0.
    //
    // The crop origin is `cx*iw*(Z-1)` — the exact inverse of the preview's
    // focus-pinned affine (`content_uv = (screen_uv - c)/scale + c`). Critically
    // the crop LUT is derived from the SAME piecewise segments as the scale LUT,
    // not merged independently, so `x` and `z` agree on `Z` at every t. Merging
    // them separately (the previous bug) let their breakpoints/round-off diverge;
    // since the implied focus is `x / (iw*(Z-1))`, that disagreement is divided
    // by `(Z-1)` and blew up near the ramp ends (Z≈1) into a visible focus slide
    // — EXPORT-only, because the preview computes the affine exactly per frame.
    let iw = source.width as f64;
    let ih = source.height as f64;
    let (z_expr, x_expr, y_expr) = build_zoom_exprs(&samples_per_region, iw, ih);

    format!(
        "scale=w='iw*({z_expr})':h='ih*({z_expr})':eval=frame,\
         crop={}:{}:x='{x_expr}':y='{y_expr}'",
        source.width, source.height
    )
}

#[derive(Debug, Clone, Copy)]
struct ZoomSample {
    t: f64,            // output-stream time (post-trim) at this sample
    scale_factor: f64, // multiplicative zoom factor (>= 1.0)
    center_x: f64,     // focus centre X in UV space (0..1), constant per region
    center_y: f64,     // focus centre Y in UV space (0..1), constant per region
}

/// Disjoint `(region_index, start, end)` windows in which each region actually
/// applies. Only one zoom can apply at a time, but the filter graph SUMS every
/// region's term (`wrap_flat_sum`), so overlapping regions would stack their
/// zoom — two 1.8x regions rendering as 2.6x while the preview shows 1.8x.
/// A later-starting region takes over, matching `activeZoomIndex` in
/// `src/lib/zoom/resolve.ts`; the earlier region resumes after it ends.
fn disjoint_zoom_windows(regions: &[&ZoomRegion], time_offset: f64) -> Vec<(usize, f64, f64)> {
    let mut out = Vec::new();
    for (i, r) in regions.iter().enumerate() {
        // Blockers are the regions that outrank this one at a shared instant:
        // a later start, or an equal start later in the list (the tie-break).
        let mut blockers: Vec<(f64, f64)> = regions
            .iter()
            .enumerate()
            .filter(|(j, o)| {
                *j != i && (o.start > r.start || (o.start == r.start && *j > i)) && o.end > o.start
            })
            .map(|(_, o)| (o.start, o.end))
            .collect();
        blockers.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

        let mut cursor = r.start.max(time_offset);
        for (bs, be) in blockers {
            if be <= cursor {
                continue;
            }
            if bs >= r.end {
                break;
            }
            if bs > cursor {
                out.push((i, cursor, bs.min(r.end)));
            }
            cursor = cursor.max(be);
            if cursor >= r.end {
                break;
            }
        }
        if cursor < r.end {
            out.push((i, cursor, r.end));
        }
    }
    out.retain(|(_, a, b)| b - a > 1e-6);
    out
}

fn sample_region(
    region: &ZoomRegion,
    // Source dimensions are no longer needed: the crop origin is derived from
    // the crop filter's own `in_w`/`in_h` at render time, not pre-computed in
    // absolute pixels here. Kept in the signature for call-site symmetry.
    _source: SourceVideoMetadata,
    time_offset: f64,
    window: (f64, f64),
) -> Vec<ZoomSample> {
    // Clamp the sampling window to the post-trim portion of the region.
    // `region.scale_at` still receives the true timeline t, so the eased
    // ramp curve is sampled correctly.
    let effective_start = window.0.max(region.start).max(time_offset);
    let window_end = window.1.min(region.end);
    let duration = (window_end - effective_start).max(0.0);
    let samples = ((duration * 20.0).ceil() as usize).clamp(8, 200);
    let step = if samples > 0 {
        duration / samples as f64
    } else {
        0.0
    };
    // Each sample carries the eased scale plus the region's CONSTANT focus
    // centre (only the scale eases). The crop origin is NOT precomputed here as
    // absolute pixels: it's derived later in `build_zoom_exprs` from the same
    // merged scale segments, so the crop and scale LUTs share breakpoints and
    // can't disagree on `Z` (the cause of the export-only focus drift). The
    // centre must match the preview's focus-pinned affine
    // (`content_uv = (screen_uv - c)/scale + c`) and the cursor overlay, which
    // uses the same affine forward transform.
    let fx_target = region.center_x.clamp(0.0, 1.0);
    let fy_target = region.center_y.clamp(0.0, 1.0);
    let mut out = Vec::with_capacity(samples + 1);
    for i in 0..=samples {
        // `timeline_t` drives `scale_at`; `output_t` is what we emit into
        // the FFmpeg LUT (t inside the filter is post-trim output time).
        let timeline_t = effective_start + step * i as f64;
        let output_t = timeline_t - time_offset;
        let scale = region.scale_at(timeline_t).max(1.0);
        out.push(ZoomSample {
            t: output_t,
            scale_factor: scale,
            center_x: fx_target,
            center_y: fy_target,
        });
    }
    out
}

/// FFmpeg's `av_expr_parse` fails ("Cannot allocate memory") on an over-long
/// expression — a recording with many auto-zoom regions once produced ~120
/// terms and broke export at filter-init time. We keep each of the three
/// expressions (z/x/y) under this many flat-sum terms.
const MAX_TERMS_PER_EXPR: usize = 48;

/// One collinear-merged line segment over output time: value goes linearly from
/// `(ta, va)` to `(tb, vb)`.
type Segment = (f64, f64, f64, f64);

/// Build the three time-varying FFmpeg expressions for the zoom — `(z, x, y)`:
///   z — multiplicative zoom factor, default 1.0 outside every region.
///   x — crop top-left X in post-scale pixels, default 0.
///   y — crop top-left Y in post-scale pixels, default 0.
///
/// All three are emitted from the SAME merged scale breakpoints. The crop
/// origin is the exact inverse of the preview's focus-pinned affine —
/// `crop_x = cx*iw*(Z-1)`, `crop_y = cy*ih*(Z-1)` — evaluated at the very same
/// segment endpoints as `Z`. Because `crop_x` is an affine function of `Z` and
/// `Z` is linear within each segment, `crop_x` is exactly linear over that same
/// segment, so the crop LUT and scale LUT can never disagree on `Z` at any `t`.
/// (Merging them independently was the old bug: their breakpoints diverged and
/// the implied focus `x/(iw*(Z-1))` blew up near the ramp ends where `Z≈1`,
/// producing the export-only focus slide.)
///
/// Emitted as a FLAT SUM (`default + if(window,Δ,0) + …`) rather than nested
/// `if`s, because FFmpeg's evaluator has a recursion-depth limit; at most one
/// window fires per `t` (regions don't overlap; segments abut as half-open
/// windows) so the sum equals the active segment's value or the default.
fn build_zoom_exprs(
    samples_per_region: &[Vec<ZoomSample>],
    iw: f64,
    ih: f64,
) -> (String, String, String) {
    // Merge the eased scale into the fewest linear segments per region, then
    // coarsen (double the tolerance) until the total fits the parser budget.
    // The hold phase collapses to one segment for free; a smooth ramp collapses
    // from ~8 samples to a few.
    let base_tol = 0.0035_f64;
    let mut tol = base_tol;
    let mut segs = merge_scale_segments(samples_per_region, tol);
    while segment_count(&segs) > MAX_TERMS_PER_EXPR && tol < base_tol * 256.0 {
        tol *= 2.0;
        segs = merge_scale_segments(samples_per_region, tol);
    }

    let mut z_terms = Vec::new();
    let mut x_terms = Vec::new();
    let mut y_terms = Vec::new();
    for (region_idx, region_segs) in segs.iter().enumerate() {
        // Focus centre is constant per region — read it off any sample.
        let (cx, cy) = samples_per_region
            .get(region_idx)
            .and_then(|s| s.first())
            .map(|s| (s.center_x, s.center_y))
            .unwrap_or((0.5, 0.5));
        for &(ta, za, tb, zb) in region_segs {
            if let Some(t) = fmt_term(ta, za, tb, zb, 1.0, "t") {
                z_terms.push(t);
            }
            // crop_x = cx*iw*(Z-1); linear in t over the same segment as Z.
            if let Some(t) = fmt_term(ta, cx * iw * (za - 1.0), tb, cx * iw * (zb - 1.0), 0.0, "t")
            {
                x_terms.push(t);
            }
            if let Some(t) = fmt_term(ta, cy * ih * (za - 1.0), tb, cy * ih * (zb - 1.0), 0.0, "t")
            {
                y_terms.push(t);
            }
        }
    }

    (
        wrap_flat_sum("1", z_terms),
        wrap_flat_sum("0", x_terms),
        wrap_flat_sum("0", y_terms),
    )
}

pub(crate) fn wrap_flat_sum(default: &str, terms: Vec<String>) -> String {
    if terms.is_empty() {
        default.to_string()
    } else {
        format!("({}+{})", default, terms.join("+"))
    }
}

fn segment_count(segs: &[Vec<Segment>]) -> usize {
    segs.iter().map(|s| s.len()).sum()
}

/// Greedy collinear merge of each region's samples on `scale_factor`: keep
/// extending the current line to the next sample while dropping the intermediate
/// breakpoint stays within `tol` of the extended line.
fn merge_scale_segments(samples_per_region: &[Vec<ZoomSample>], tol: f64) -> Vec<Vec<Segment>> {
    samples_per_region
        .iter()
        .map(|samples| {
            let mut segments: Vec<Segment> = Vec::new();
            let mut run: Option<Segment> = None;
            for pair in samples.windows(2) {
                let (a, b) = (&pair[0], &pair[1]);
                if b.t <= a.t {
                    continue;
                }
                let (va, vb) = (a.scale_factor, b.scale_factor);
                match run {
                    Some((ra, rva, _rb, _rvb)) => {
                        let span = b.t - ra;
                        let pred_at_a = if span > 1e-9 {
                            rva + (vb - rva) * (a.t - ra) / span
                        } else {
                            va
                        };
                        if (pred_at_a - va).abs() <= tol {
                            run = Some((ra, rva, b.t, vb));
                        } else {
                            segments.push((ra, rva, a.t, va));
                            run = Some((a.t, va, b.t, vb));
                        }
                    }
                    None => run = Some((a.t, va, b.t, vb)),
                }
            }
            if let Some(r) = run {
                segments.push(r);
            }
            segments
        })
        .collect()
}

/// Format one segment as a flat-sum term over the half-open window `[ta, tb)`,
/// contributing `value - default`. `var` is the filter's time variable — `t` for
/// overlay/scale/crop, `T` for `geq`. Returns `None` for a constant segment that
/// equals the default (nothing to add).
pub(crate) fn fmt_term(
    ta: f64,
    va: f64,
    tb: f64,
    vb: f64,
    default_val: f64,
    var: &str,
) -> Option<String> {
    if (va - vb).abs() < 1e-6 {
        let offset = va - default_val;
        if offset.abs() < 1e-6 {
            return None;
        }
        Some(format!(
            "if(gte({var},{ta:.4})*lt({var},{tb:.4}),{offset:.4},0)"
        ))
    } else {
        // Guard a degenerate (zero-width) window: without it the ramp's
        // `(t-ta)/dt` divides by zero and bakes `inf` into the filter expression,
        // breaking the whole zoom graph at init.
        let dt = (tb - ta).max(1e-6);
        let dv = vb - va;
        let offset_a = va - default_val;
        Some(format!(
            "if(gte({var},{ta:.4})*lt({var},{tb:.4}),({offset_a:.4}+{dv:.6}*({var}-{ta:.4})/{dt:.4}),0)"
        ))
    }
}

/// Generic collinear merge over per-region `(output_t, value)` samples — the
/// same greedy line-fit as `merge_scale_segments`, but on arbitrary values (used
/// by the camera zoom-follow LUTs).
fn merge_value_segments(samples_per_region: &[Vec<(f64, f64)>], tol: f64) -> Vec<Vec<Segment>> {
    samples_per_region
        .iter()
        .map(|samples| {
            let mut segments: Vec<Segment> = Vec::new();
            let mut run: Option<Segment> = None;
            for pair in samples.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                if b.0 <= a.0 {
                    continue;
                }
                let (va, vb) = (a.1, b.1);
                match run {
                    Some((ra, rva, _rb, _rvb)) => {
                        let span = b.0 - ra;
                        let pred_at_a = if span > 1e-9 {
                            rva + (vb - rva) * (a.0 - ra) / span
                        } else {
                            va
                        };
                        if (pred_at_a - va).abs() <= tol {
                            run = Some((ra, rva, b.0, vb));
                        } else {
                            segments.push((ra, rva, a.0, va));
                            run = Some((a.0, va, b.0, vb));
                        }
                    }
                    None => run = Some((a.0, va, b.0, vb)),
                }
            }
            if let Some(r) = run {
                segments.push(r);
            }
            segments
        })
        .collect()
}

/// Build a time-varying FFmpeg expression (flat-sum of windowed linear terms) in
/// `t` from per-region `(output_t, value)` samples, collinear-merged (coarsening
/// the tolerance until it fits the parser budget). `default` is the value outside
/// every region. Mirrors `build_zoom_exprs` for the camera zoom-follow LUTs.
pub(crate) fn build_time_lut_expr(samples_per_region: &[Vec<(f64, f64)>], default: f64) -> String {
    let base_tol = 0.5_f64; // pixels
    let mut tol = base_tol;
    let mut segs = merge_value_segments(samples_per_region, tol);
    while segment_count(&segs) > MAX_TERMS_PER_EXPR && tol < base_tol * 512.0 {
        tol *= 2.0;
        segs = merge_value_segments(samples_per_region, tol);
    }
    let mut terms = Vec::new();
    for region_segs in &segs {
        for &(ta, va, tb, vb) in region_segs {
            if let Some(term) = fmt_term(ta, va, tb, vb, default, "t") {
                terms.push(term);
            }
        }
    }
    wrap_flat_sum(&format!("{default:.4}"), terms)
}

pub(crate) fn resolve_background_path(
    value: &str,
    static_root: &Path,
    asset_cache_dir: Option<&Path>,
) -> Option<PathBuf> {
    if value.is_empty() {
        return None;
    }

    // External-asset scheme: `asset:<id>` resolves against the downloaded
    // manifest cache in the app data dir. Read manifest.lock.json there.
    if let Some(id) = value.strip_prefix("asset:") {
        if let Some(dir) = asset_cache_dir {
            let lock = dir.join("manifest.lock.json");
            if let Ok(bytes) = std::fs::read(&lock) {
                if let Ok(manifest) =
                    serde_json::from_slice::<crate::commands::assets::Manifest>(&bytes)
                {
                    if let Some(entry) = manifest.assets.iter().find(|a| a.id == id) {
                        let path = dir.join(&entry.filename);
                        if path.exists() {
                            return Some(path);
                        }
                    }
                }
            }
        }
        return None;
    }

    // Frontend wallpapers are served from `/backgrounds/wallpapers/...` — map
    // those back to `static/backgrounds/wallpapers/...` on disk. Also handle the
    // legacy `/wallpapers/...` prefix for any stored projects.
    if let Some(rest) = value.strip_prefix("/backgrounds/wallpapers/") {
        let resolved = static_root
            .join("backgrounds")
            .join("wallpapers")
            .join(rest);
        if resolved.exists() {
            return Some(resolved);
        }
    }
    if let Some(rest) = value.strip_prefix("/wallpapers/") {
        let resolved = static_root.join("wallpapers").join(rest);
        if resolved.exists() {
            return Some(resolved);
        }
        // Also try backgrounds/wallpapers/ as a fallback.
        let alt = static_root
            .join("backgrounds")
            .join("wallpapers")
            .join(rest);
        if alt.exists() {
            return Some(alt);
        }
    }
    // Any other `/`-rooted path is treated as relative to static_root.
    if let Some(rest) = value.strip_prefix('/') {
        let resolved = static_root.join(rest);
        if resolved.exists() {
            return Some(resolved);
        }
    }

    if let Some(decoded_path) = decode_background_uri(value) {
        if decoded_path.exists() {
            return Some(decoded_path);
        }
    }

    let as_path = PathBuf::from(value);
    if as_path.exists() {
        Some(as_path)
    } else {
        None
    }
}

fn decode_background_uri(value: &str) -> Option<PathBuf> {
    const PREFIXES: [&str; 4] = [
        "asset://localhost/",
        "http://asset.localhost/",
        "https://asset.localhost/",
        "file:///",
    ];

    for prefix in PREFIXES {
        if let Some(rest) = value.strip_prefix(prefix) {
            let decoded = percent_decode(rest);
            let normalized = if decoded.starts_with('/') && decoded.as_bytes().get(2) == Some(&b':')
            {
                decoded[1..].to_string()
            } else {
                decoded
            };
            return Some(PathBuf::from(normalized));
        }
    }

    None
}

fn percent_decode(value: &str) -> String {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = std::str::from_utf8(&bytes[index + 1..index + 3]) {
                if let Ok(byte) = u8::from_str_radix(hex, 16) {
                    decoded.push(byte);
                    index += 3;
                    continue;
                }
            }
        }

        decoded.push(bytes[index]);
        index += 1;
    }

    String::from_utf8_lossy(&decoded).into_owned()
}

fn normalize_color(value: &str) -> String {
    if value.trim().is_empty() {
        "#111111".into()
    } else {
        value.trim().to_string()
    }
}

fn gradient_fallback_color(value: &str) -> String {
    value
        .split(|c: char| c == ',' || c.is_whitespace())
        .find(|token| token.starts_with('#'))
        .map(|token| token.trim_matches(')').to_string())
        .unwrap_or_else(|| "#111111".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::node_types::ZoomRegion;

    fn region(start: f64, end: f64, scale: f64) -> ZoomRegion {
        ZoomRegion {
            start,
            end,
            scale,
            ease_in: Default::default(),
            ease_out: Default::default(),
            ramp_in: 0.5,
            ramp_out: 0.5,
            center_x: 0.5,
            center_y: 0.5,
            motion_blur: 0.0,
            hidden: false,
            extra: Default::default(),
        }
    }

    #[test]
    fn parse_aspect_ratio_maps_known_labels() {
        assert!((parse_aspect_ratio(Some("16:9")).unwrap() - 16.0 / 9.0).abs() < 1e-9);
        assert!((parse_aspect_ratio(Some("9:16")).unwrap() - 9.0 / 16.0).abs() < 1e-9);
        assert_eq!(parse_aspect_ratio(Some("1:1")), Some(1.0));
        assert!((parse_aspect_ratio(Some("1.91:1")).unwrap() - 1.91).abs() < 1e-9);
        // 16:9 and 9:16 must not be confused — landscape is wider than tall.
        assert!(parse_aspect_ratio(Some("16:9")).unwrap() > 1.0);
        assert!(parse_aspect_ratio(Some("9:16")).unwrap() < 1.0);
        // Unknown / source / absent → None (keep source dims).
        assert_eq!(parse_aspect_ratio(Some("4:3")), None);
        assert_eq!(parse_aspect_ratio(Some("source")), None);
        assert_eq!(parse_aspect_ratio(None), None);
    }

    #[test]
    fn normalize_color_trims_and_falls_back() {
        assert_eq!(normalize_color("  #ffcc00  "), "#ffcc00");
        assert_eq!(normalize_color(""), "#111111");
        assert_eq!(normalize_color("   "), "#111111");
    }

    #[test]
    fn gradient_fallback_color_picks_first_hex() {
        assert_eq!(
            gradient_fallback_color("linear-gradient(90deg, #3b82f6, #9333ea)"),
            "#3b82f6"
        );
        // No hex token → the neutral default.
        assert_eq!(gradient_fallback_color("rgb(255,0,0)"), "#111111");
        assert_eq!(gradient_fallback_color(""), "#111111");
    }

    #[test]
    fn fmt_term_handles_degenerate_zero_width_window() {
        // tb == ta with differing values previously divided by zero → `inf`.
        let term = fmt_term(2.0, 1.0, 2.0, 1.5, 1.0, "t").expect("ramp term");
        assert!(!term.contains("inf"));
        assert!(!term.contains("NaN"));
    }

    #[test]
    fn fmt_term_drops_constant_at_default() {
        // Constant segment equal to the default contributes nothing.
        assert_eq!(fmt_term(0.0, 1.0, 1.0, 1.0, 1.0, "t"), None);
    }

    #[test]
    fn scene_anims_use_the_frontend_segment_anims_key() {
        // Regression: the frontend serialises scene animations as `segmentAnims`,
        // but the Rust field deserialised `sceneAnimations`, so every animation was
        // silently dropped into passthrough and never reached the export graph
        // (perfect in preview, absent in export). The key must round-trip.
        let value = serde_json::to_value(RenderState::default()).unwrap();
        assert!(
            value.get("segmentAnims").is_some(),
            "must emit the frontend key"
        );
        assert!(value.get("sceneAnimations").is_none());

        // A frontend-shaped payload must populate the typed field the export reads.
        let mut payload = value;
        payload["segmentAnims"] = serde_json::json!([{
            "start": 0.0,
            "in": { "kind": "slide", "durationMs": 500, "easing": { "x1": 0, "y1": 0, "x2": 1, "y2": 1 }, "dir": "left" }
        }]);
        let rs: RenderState = serde_json::from_value(payload).unwrap();
        assert_eq!(rs.scene_animations.len(), 1);
        assert_eq!(
            rs.scene_animations[0].anim_in.as_ref().unwrap().kind,
            "slide"
        );
    }

    #[test]
    fn scene_overlay_injects_video_layer_stages() {
        // The scene overlay must reach the graph: a scale/rotate/opacity overlay
        // produces the per-frame video-layer stages and an expression-driven
        // overlay position (not the static one).
        let state = RenderState {
            trim_start: 0.0,
            trim_end: 10.0,
            ..RenderState::default()
        };
        let scene = crate::render::scene_anim::SceneOverlay {
            x_expr: "100".into(),
            y_expr: "50".into(),
            scale_expr: Some("(1.1)".into()),
            rotate_expr: Some("(15)".into()),
            opacity_expr: Some("(1)".into()),
        };
        let plan = RenderGraph::from_state(&state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: 1920,
                    height: 1080,
                    fps: 60.0,
                },
                Path::new("."),
                1,
                None,
                None,
                None,
                None,
                test_canvas(),
                Some(&scene),
            )
            .expect("plan");
        let fc = plan.filter_complex.expect("filter graph");
        assert!(
            fc.contains("scale=w='iw*((1.1))"),
            "scene scale stage missing: {fc}"
        );
        assert!(
            fc.contains("rotate=a='((15))*PI/180'"),
            "scene rotate stage missing: {fc}"
        );
        assert!(
            fc.contains("geq=lum='p(X,Y)'"),
            "scene fade stage missing: {fc}"
        );
        assert!(
            fc.contains("overlay=x='100':y='50'"),
            "expression overlay missing: {fc}"
        );
    }

    fn render_state_with_zoom(
        trim_start: f64,
        trim_end: f64,
        regions: Vec<ZoomRegion>,
    ) -> RenderState {
        RenderState {
            trim_start,
            trim_end,
            zoom_regions: regions,
            ..RenderState::default()
        }
    }

    fn test_canvas() -> CanvasGeometry {
        compute_canvas_geometry(1920, 1080, 0.0, None)
    }

    fn export_plan(state: &RenderState) -> ExportPlan {
        RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: 1920,
                    height: 1080,
                    fps: 60.0,
                },
                Path::new("."),
                1,
                None,
                None,
                None,
                None,
                test_canvas(),
                None,
            )
            .expect("plan")
    }

    fn export_plan_with_shadow(state: &RenderState, shadow_path: PathBuf) -> ExportPlan {
        RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: 1920,
                    height: 1080,
                    fps: 60.0,
                },
                Path::new("."),
                1,
                None,
                None,
                Some(shadow_path),
                None,
                test_canvas(),
                None,
            )
            .expect("plan")
    }

    /// Without trim, the LUT t-values are timeline = output, and the filter
    /// must include `between(t,1.0,...)` segments because the zoom region
    /// starts at timeline 1.0.
    /// The filter MUST be a `scale=eval=frame` + fixed-size `crop` chain.
    /// The previous `crop=w='<expr>':h='<expr>'` form silently never fired
    /// because ffmpeg's `crop` evaluates `w`/`h` only ONCE at filter init,
    /// where `t = 0`; that was the actual root cause of "zoom missing in
    /// exported videos". This test asserts the new shape directly.
    #[test]
    fn zoom_filter_uses_scale_eval_frame_not_crop_wh_lut() {
        let state = render_state_with_zoom(0.0, 5.0, vec![region(1.0, 4.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan
            .filter_complex
            .expect("filter_complex must exist when zoom present");
        // Must use scale with eval=frame so width/height re-evaluate per frame.
        assert!(
            fc.contains("scale=w='iw*(") && fc.contains(":eval=frame"),
            "zoom must scale via eval=frame: {fc}"
        );
        // Crop must have LITERAL fixed w/h (=source dims) — anything inside
        // `crop=w='<expr>'` would hit the init-only evaluation bug again.
        assert!(
            fc.contains("crop=1920:1080:"),
            "crop must use fixed source dimensions, not LUT-driven w/h: {fc}"
        );
        // LUT must reference output-stream time at the region start.
        assert!(
            fc.contains("gte(t,1.0000)"),
            "expected output-t LUT entry at 1.0000: {fc}"
        );
    }

    /// With trim_start = 2.0, the FFmpeg `t` is OUTPUT-stream time. A region
    /// at timeline [3.0, 5.0] must appear in the LUT at output [1.0, 3.0].
    /// Pre-fix, this assertion failed: the LUT had `between(t,3.0000,...)`
    /// which never fires because the output never reaches t=3 (the visible
    /// duration is 5 - 2 = 3 s, but scrubbing/preview seeing zoom at
    /// timeline 3 expects it at output 1).
    #[test]
    fn zoom_filter_shifts_lut_by_trim_start() {
        let state = render_state_with_zoom(2.0, 5.0, vec![region(3.0, 5.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan
            .filter_complex
            .expect("filter_complex must exist when zoom present");
        assert!(
            fc.contains("gte(t,1.0000)"),
            "LUT must be shifted to output-time (start at output t=1.0): {fc}"
        );
        assert!(
            !fc.contains("gte(t,3.0000)"),
            "stale timeline-t LUT entry at 3.0000 must NOT be present: {fc}"
        );
    }

    /// A zoom region whose entire timeline range precedes trim_start used
    /// to produce a LUT whose t-values were negative — harmless to FFmpeg
    /// (`between(t, -2.0, -1.0)` simply never fires) but a waste of filter
    /// string. Now we prune those regions entirely, so the planner doesn't
    /// emit a zoom prelude at all in this case. The test still verifies
    /// "doesn't panic" and that the rest of the plan is intact.
    #[test]
    fn zoom_region_entirely_before_trim_does_not_panic() {
        let state = render_state_with_zoom(5.0, 10.0, vec![region(1.0, 3.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex still emitted");
        // Pruned zoom must leave NO scale/crop prelude.
        assert!(
            !fc.contains("scale=w='iw*("),
            "pruned zoom should leave no scale prelude: {fc}"
        );
        assert!(fc.contains("[vout]"), "rest of plan intact: {fc}");
    }

    /// Plan must always include the zoom prelude when regions exist, even
    /// with the default color background — this was the agent's mis-diagnosis
    /// originally, but verifying it locks in the contract.
    #[test]
    fn zoom_filter_present_with_default_background() {
        let state = render_state_with_zoom(0.0, 5.0, vec![region(1.0, 4.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(
            fc.contains("[video0]"),
            "zoom prelude must label its output [video0]: {fc}"
        );
        assert_eq!(plan.video_map, "[vout]");
    }

    /// Auto-zoom typically produces 3-6 regions. Each must contribute
    /// segments to the LUT, and a sample at each region's start should be
    /// represented.
    #[test]
    fn multiple_zoom_regions_all_appear_in_lut() {
        let state = render_state_with_zoom(
            0.0,
            10.0,
            vec![
                region(1.0, 2.0, 1.4),
                region(3.0, 4.5, 1.6),
                region(6.0, 8.0, 1.5),
            ],
        );
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(fc.contains("gte(t,1.0000)"), "first region missing: {fc}");
        assert!(fc.contains("gte(t,3.0000)"), "second region missing: {fc}");
        assert!(fc.contains("gte(t,6.0000)"), "third region missing: {fc}");
    }

    /// Overlapping regions used to each contribute a term to the summed zoom
    /// expression, so two 1.8x regions rendered as 2.6x while the preview (which
    /// picks a single region) showed 1.8x. Windows must be disjoint.
    #[test]
    fn overlapping_zoom_regions_do_not_stack() {
        let a = region(0.0, 6.0, 1.8);
        let b = region(4.0, 10.0, 1.8);
        let windows = disjoint_zoom_windows(&[&a, &b], 0.0);
        for (i, s0, e0) in &windows {
            for (j, s1, e1) in &windows {
                if std::ptr::eq(i, j) && (s0 - s1).abs() < f64::EPSILON {
                    continue;
                }
                let overlap = e0.min(*e1) - s0.max(*s1);
                assert!(
                    overlap <= 1e-6,
                    "windows overlap: ({i},{s0},{e0}) vs ({j},{s1},{e1})"
                );
            }
        }
    }

    /// A later-starting region takes over, then the enclosing one resumes —
    /// the same rule as `activeZoomIndex` in src/lib/zoom/resolve.ts.
    #[test]
    fn nested_zoom_region_wins_then_outer_resumes() {
        let outer = region(0.0, 10.0, 1.5);
        let inner = region(4.0, 6.0, 2.0);
        let mut windows = disjoint_zoom_windows(&[&outer, &inner], 0.0);
        windows.sort_by(|x, y| x.1.partial_cmp(&y.1).unwrap());
        assert_eq!(
            windows.len(),
            3,
            "expected outer/inner/outer, got {windows:?}"
        );
        assert_eq!(windows[0].0, 0);
        assert!((windows[0].2 - 4.0).abs() < 1e-9, "{windows:?}");
        assert_eq!(windows[1].0, 1);
        assert!((windows[1].1 - 4.0).abs() < 1e-9 && (windows[1].2 - 6.0).abs() < 1e-9);
        assert_eq!(windows[2].0, 0);
        assert!((windows[2].1 - 6.0).abs() < 1e-9 && (windows[2].2 - 10.0).abs() < 1e-9);
    }

    /// Non-overlapping regions must be left exactly as authored.
    #[test]
    fn disjoint_zoom_regions_are_untouched() {
        let a = region(1.0, 2.0, 1.4);
        let b = region(3.0, 4.5, 1.6);
        let windows = disjoint_zoom_windows(&[&a, &b], 0.0);
        assert_eq!(windows.len(), 2, "{windows:?}");
        assert!((windows[0].1 - 1.0).abs() < 1e-9 && (windows[0].2 - 2.0).abs() < 1e-9);
        assert!((windows[1].1 - 3.0).abs() < 1e-9 && (windows[1].2 - 4.5).abs() < 1e-9);
    }

    /// The generated `color=` background MUST carry an explicit `:r=<fps>`.
    /// Without it FFmpeg defaults the generator to 25 fps, and since it's the
    /// base of the composite overlay the whole export drops to 25 fps —
    /// frame-dropping a 60 fps recording into juddery motion (the export-only
    /// "shake", very visible under a zoom).
    #[test]
    fn color_background_pins_source_framerate() {
        let state = RenderState {
            background_type: "color".into(),
            background_value: "#111111".into(),
            zoom_regions: vec![region(1.0, 4.0, 1.6)],
            ..RenderState::default()
        };
        let plan = RenderGraph::from_state(&state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: 1920,
                    height: 1080,
                    fps: 60.0,
                },
                Path::new("."),
                1,
                None,
                None,
                None,
                None,
                test_canvas(),
                None,
            )
            .expect("plan");
        let fc = plan.filter_complex.expect("filter_complex");
        assert!(
            fc.contains("color=") && fc.contains(":r=60"),
            "color background must pin the source fps (got: {fc})"
        );
    }

    /// A hidden region is a non-destructive mute: it must contribute NOTHING to
    /// the export filter (no scale/crop prelude when it's the only region).
    #[test]
    fn hidden_zoom_region_is_excluded_from_export() {
        let mut r = region(1.0, 4.0, 1.6);
        r.hidden = true;
        let state = render_state_with_zoom(0.0, 5.0, vec![r]);
        let plan = export_plan(&state);
        if let Some(fc) = plan.filter_complex {
            assert!(
                !fc.contains("eval=frame"),
                "hidden region must produce no zoom prelude: {fc}"
            );
        }
    }

    /// One region's worth of samples: ease-in (1.0→peak), hold, ease-out, at
    /// 20 Hz — the shape auto-zoom produces. Used to stress the expression cap.
    fn smooth_region_samples(t0: f64, peak: f64) -> Vec<ZoomSample> {
        let mut v = Vec::new();
        let mut push = |t: f64, z: f64| {
            v.push(ZoomSample {
                t,
                scale_factor: z,
                center_x: 0.5,
                center_y: 0.5,
            });
        };
        let smoothstep = |f: f64| f * f * (3.0 - 2.0 * f);
        for i in 0..=8 {
            let f = i as f64 / 8.0;
            push(t0 + 0.05 * i as f64, 1.0 + (peak - 1.0) * smoothstep(f));
        }
        for i in 0..=20 {
            push(t0 + 0.4 + 0.1 * i as f64, peak);
        }
        for i in 0..=8 {
            let f = i as f64 / 8.0;
            push(
                t0 + 2.4 + 0.05 * i as f64,
                peak - (peak - 1.0) * smoothstep(f),
            );
        }
        v
    }

    /// Regression for the "Cannot allocate memory" export failure: many
    /// auto-zoom regions must NOT blow past FFmpeg's expression parser limit.
    #[test]
    fn zoom_expression_stays_under_parser_budget_with_many_regions() {
        let regions: Vec<Vec<ZoomSample>> = (0..16)
            .map(|k| smooth_region_samples(k as f64 * 3.0, 1.6))
            .collect();
        let (z, x, y) = build_zoom_exprs(&regions, 1920.0, 1080.0);
        for (name, expr) in [("z", &z), ("x", &x), ("y", &y)] {
            let term_count = expr.matches("if(").count();
            assert!(
                term_count <= MAX_TERMS_PER_EXPR,
                "{name} expression must stay under the {MAX_TERMS_PER_EXPR}-term budget, got {term_count}"
            );
        }
    }

    /// Collinear merge compacts a dense ramp but keeps its start anchor and the
    /// flat-sum-over-default shape.
    #[test]
    fn collinear_merge_compacts_ramp_but_keeps_anchor() {
        let region = smooth_region_samples(1.0, 1.5);
        let (z, _x, _y) = build_zoom_exprs(&[region], 1920.0, 1080.0);
        assert!(z.starts_with("(1+"), "flat sum over default: {z}");
        assert!(z.contains("gte(t,1.0000)"), "ramp must start at t0: {z}");
        // The 21-sample hold (offset = peak − default = 0.5) collapses to one term.
        assert_eq!(
            z.matches(",0.5000,0)").count(),
            1,
            "hold phase must collapse to a single term: {z}"
        );
        // ...and the whole thing is well under the 36 raw sample windows.
        assert!(
            z.matches("if(").count() < 20,
            "merge should compact the curve: {z}"
        );
    }

    /// The x/y crop LUTs MUST share the scale LUT's time breakpoints, so the
    /// crop can never disagree with the scale on `Z` (their independent merge was
    /// the export-only focus-drift bug). Every `gte(t,…)` window in `z` must also
    /// appear in `x` and `y`, and vice versa.
    #[test]
    fn crop_lut_shares_scale_breakpoints() {
        let mut r = region(1.0, 4.0, 1.6);
        r.center_x = 0.8;
        r.center_y = 0.3;
        let samples = vec![sample_region(
            &r,
            SourceVideoMetadata {
                width: 1920,
                height: 1080,
                fps: 60.0,
            },
            0.0,
            (r.start, r.end),
        )];
        let (z, x, y) = build_zoom_exprs(&samples, 1920.0, 1080.0);
        // Off-centre focus must actually move the crop (not identically 0).
        assert!(
            x != "0" && y != "0",
            "off-centre crop must be non-trivial: x={x} y={y}"
        );
        let windows = |e: &str| -> Vec<String> {
            e.match_indices("gte(t,")
                .map(|(i, _)| {
                    let rest = &e[i + 6..];
                    let end = rest.find(')').unwrap_or(rest.len());
                    rest[..end].to_string()
                })
                .collect::<std::collections::BTreeSet<_>>()
                .into_iter()
                .collect()
        };
        assert_eq!(windows(&z), windows(&x), "x must share z's breakpoints");
        assert_eq!(windows(&z), windows(&y), "y must share z's breakpoints");
    }

    fn plan_with(
        state: &RenderState,
        border_mask: Option<PathBuf>,
        shadow_mask: Option<PathBuf>,
    ) -> ExportPlan {
        RenderGraph::from_state(state)
            .build_export_plan_with(
                SourceVideoMetadata {
                    width: 1920,
                    height: 1080,
                    fps: 60.0,
                },
                Path::new("."),
                1,
                None,
                border_mask,
                shadow_mask,
                None,
                test_canvas(),
                None,
            )
            .expect("plan")
    }

    /// Pull out the single-quoted FFmpeg expressions (`w='…'`, `x='…'`, …).
    fn quoted_exprs(fc: &str) -> Vec<&str> {
        let mut out = Vec::new();
        let mut rest = fc;
        while let Some(i) = rest.find('\'') {
            let after = &rest[i + 1..];
            match after.find('\'') {
                Some(j) => {
                    out.push(&after[..j]);
                    rest = &after[j + 1..];
                }
                None => break,
            }
        }
        out
    }

    fn assert_graph_valid(fc: &str, ctx: &str) {
        assert_eq!(
            fc.matches('(').count(),
            fc.matches(')').count(),
            "unbalanced parens [{ctx}]: {fc}"
        );
        assert_eq!(
            fc.matches('[').count(),
            fc.matches(']').count(),
            "unbalanced brackets [{ctx}]: {fc}"
        );
        for expr in quoted_exprs(fc) {
            assert!(
                !expr.contains("NaN") && !expr.contains("inf"),
                "non-finite value in expression [{ctx}]: {expr}"
            );
            assert!(
                expr.matches("if(").count() <= MAX_TERMS_PER_EXPR,
                "expression over parser budget [{ctx}]: {expr}"
            );
        }
    }

    /// Combinatorial guard over the render-side export options — background
    /// type, padding, border-radius + drop-shadow masks, and zoom-region count
    /// (incl. the many-region case that broke FFmpeg's parser). EVERY
    /// combination must yield a structurally valid filter graph.
    #[test]
    fn export_filter_graph_valid_across_render_option_matrix() {
        let bg_types = ["color", "gradient", "image", "wallpaper"];
        let paddings = [0.0_f64, 8.0];
        let zoom_counts = [0usize, 1, 8];
        let mask_opts = [false, true];
        let mut cases = 0;

        for bg in bg_types {
            for &pad in &paddings {
                for &zc in &zoom_counts {
                    for &masks in &mask_opts {
                        let regions = (0..zc)
                            .map(|i| region(1.0 + i as f64 * 3.0, 2.5 + i as f64 * 3.0, 1.6))
                            .collect();
                        let state = RenderState {
                            background_type: bg.to_string(),
                            background_value: if bg == "color" {
                                "#202020".into()
                            } else {
                                "linear-gradient(180deg,#ffffff,#000000)".into()
                            },
                            padding: pad,
                            border_radius: if masks { 25.0 } else { 0.0 },
                            zoom_regions: regions,
                            ..RenderState::default()
                        };
                        let (bm, sm) = if masks {
                            (
                                Some(PathBuf::from("border.png")),
                                Some(PathBuf::from("shadow.png")),
                            )
                        } else {
                            (None, None)
                        };
                        let plan = plan_with(&state, bm, sm);
                        if let Some(fc) = plan.filter_complex.as_deref() {
                            let ctx = format!("bg={bg} pad={pad} zoom={zc} masks={masks}");
                            assert_graph_valid(fc, &ctx);
                            assert!(!plan.video_map.is_empty(), "empty video_map [{ctx}]");
                        }
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(
            cases,
            bg_types.len() * paddings.len() * zoom_counts.len() * mask_opts.len()
        );
    }

    /// Region partially overlapping `trim_start` (e.g. region [1, 4],
    /// trim_start = 2.0): the LUT must NOT contain segments before the
    /// trim; samples should start at the post-trim portion (output t ≥ 0).
    #[test]
    fn zoom_region_partially_before_trim_is_clamped() {
        let state = render_state_with_zoom(2.0, 6.0, vec![region(1.0, 4.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        // First segment should start at output t = 0 (corresponding to
        // timeline t = 2.0, the clamped effective_start).
        assert!(
            fc.contains("gte(t,0.0000)"),
            "clamped LUT must start at output t=0: {fc}"
        );
        // No stale pre-trim segment should appear.
        assert!(
            !fc.contains("gte(t,-1.0000)"),
            "negative-t segment should be pruned by clamping: {fc}"
        );
    }

    /// Region whose entire timeline range is before trim_start should not
    /// contribute ANY segments to the LUT (and previously emitted dead
    /// `between(t, negative, negative)` calls).
    #[test]
    fn fully_pre_trim_zoom_region_is_dropped() {
        let state = render_state_with_zoom(
            5.0,
            10.0,
            vec![
                region(1.0, 3.0, 1.5), // entirely before trim
                region(6.0, 8.0, 1.5), // post-trim, should fire
            ],
        );
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        // Note: in this state, region [1,3] is pre-trim and dropped, only
        // region [6,8] survives — its post-trim start is output_t = 1.0.
        assert!(
            fc.contains("gte(t,1.0000)"),
            "post-trim region present: {fc}"
        );
        // Pre-trim region's first sample would have been at output_t = -4.0.
        assert!(
            !fc.contains("-4.0000"),
            "pre-trim region must not contribute LUT entries: {fc}"
        );
    }

    /// When ALL regions are pre-trim, the prelude should not exist at all
    /// (since `build_zoom_filter` returns empty, the `.filter(!is_empty)`
    /// drops the prelude, and with default color bg + no other prelude,
    /// the plan still has a filter_complex but no zoom in it).
    #[test]
    fn all_pre_trim_zoom_regions_yields_no_zoom_prelude() {
        let state = render_state_with_zoom(5.0, 10.0, vec![region(1.0, 3.0, 1.5)]);
        let plan = export_plan(&state);
        let fc = plan
            .filter_complex
            .expect("color bg still produces a complex");
        assert!(
            !fc.contains("scale=w='iw*("),
            "no zoom prelude expected when all regions are pre-trim: {fc}"
        );
    }

    /// Drop shadow path injects a `[N:v]format=rgba[shadow]` stage and
    /// composes it onto the bg before the video overlay. The shadow input
    /// index lands AFTER the bg-image slot (when present) — for the
    /// default color-bg case, that's index 1 (only extra input).
    #[test]
    fn drop_shadow_inserts_overlay_stage_with_color_bg() {
        let state = RenderState::default();
        let plan = export_plan_with_shadow(&state, PathBuf::from("/tmp/fake_shadow.png"));
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(
            fc.contains("[1:v]format=rgba[shadow]"),
            "shadow input stage missing: {fc}"
        );
        assert!(
            fc.contains("[bg0][shadow]overlay=0:0[bg]"),
            "shadow composite stage missing: {fc}"
        );
        assert!(
            fc.contains("[bg]") && fc.contains("overlay=0:0[vout]"),
            "video should still composite onto the shadowed bg: {fc}"
        );
        assert_eq!(
            plan.extra_inputs.len(),
            1,
            "shadow PNG appended to extra_inputs"
        );
    }

    /// Without shadow, the extra `[bg0]` rename should NOT cost a real
    /// filter pass — the planner just labels the color stage `[bg0]` and
    /// the video composite reads from `[bg0]` directly. Quick sanity test
    /// that no `format=rgba[shadow]` ever leaks in.
    #[test]
    fn no_shadow_means_no_shadow_overlay_stage() {
        let state = RenderState::default();
        let plan = export_plan(&state);
        let fc = plan.filter_complex.expect("filter_complex must exist");
        assert!(
            !fc.contains("[shadow]"),
            "shadow stage must not appear when no shadow PNG was supplied: {fc}"
        );
    }
}

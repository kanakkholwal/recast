use recast_color::{Gradient, Srgba};
use recast_cursor::{CursorPlacement, CursorSettings};
use recast_scene::v1::nodes::{ShadowSettings, ZoomRegion};
use recast_scene::v1::SegmentAnim;
use recast_scene::{Effect, LayerId, LayerSource, Scene};
use recast_time::{output_to_original, Segment, TimeMap};

use crate::annotation::{annotation_params, sorted_visible, AnnotationParams};
use crate::camera::{bubble_params, bubble_shadow};
use crate::geometry::{canvas_geometry, CanvasGeometry};

/// A 2x3 row-major affine, applied to normalised source UVs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Affine2 {
    pub sx: f32,
    pub shx: f32,
    pub tx: f32,
    pub shy: f32,
    pub sy: f32,
    pub ty: f32,
}

impl Affine2 {
    pub const IDENTITY: Self = Self {
        sx: 1.0,
        shx: 0.0,
        tx: 0.0,
        shy: 0.0,
        sy: 1.0,
        ty: 0.0,
    };

    /// A centred zoom: samples a `1/scale`-sized window of the source around
    /// `(center_x, center_y)`, clamped so the window never leaves the frame.
    pub fn zoom(scale: f32, center_x: f32, center_y: f32) -> Self {
        Self::zoom_ramp(scale, scale, center_x, center_y)
    }

    /// A zoom at a point on its ramp: the window is exactly `1/scale`, and its CENTRE travels straight from the frame centre toward the destination as the ramp runs.
    /// The destination is clamped ONCE against `final_scale`, never per frame, or x and y leave the bound at different times and the pan arcs (preview defect D-1).
    pub fn zoom_ramp(scale: f32, final_scale: f32, center_x: f32, center_y: f32) -> Self {
        let scale = scale.max(1.0);
        let final_scale = final_scale.max(scale);
        let window = 1.0 / scale;
        // Destination centre, clamped against the FINAL window only.
        let half_final = 0.5 / final_scale;
        let dcx = center_x.clamp(half_final, 1.0 - half_final);
        let dcy = center_y.clamp(half_final, 1.0 - half_final);
        // Straight-line progress: 0 at scale 1 (frame centre), 1 at final scale.
        let progress = if final_scale > 1.0 {
            ((scale - 1.0) / (final_scale - 1.0)).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let cx = 0.5 + progress * (dcx - 0.5);
        let cy = 0.5 + progress * (dcy - 0.5);
        let half = window * 0.5;
        Self {
            sx: window,
            shx: 0.0,
            tx: cx - half,
            shy: 0.0,
            sy: window,
            ty: cy - half,
        }
    }

    pub fn apply(&self, u: f32, v: f32) -> (f32, f32) {
        (
            self.sx * u + self.shx * v + self.tx,
            self.shy * u + self.sy * v + self.ty,
        )
    }

    pub fn is_identity(&self) -> bool {
        *self == Self::IDENTITY
    }

    /// The transform maps a DESTINATION uv to the SOURCE uv it samples, so
    /// placing a known source point on the canvas needs the inverse. `None` for
    /// a degenerate transform, which would put the point nowhere real.
    pub fn invert(&self) -> Option<Self> {
        let det = self.sx * self.sy - self.shx * self.shy;
        if det.abs() < 1e-12 {
            return None;
        }
        let inv = 1.0 / det;
        Some(Self {
            sx: self.sy * inv,
            shx: -self.shx * inv,
            tx: (self.shx * self.ty - self.sy * self.tx) * inv,
            shy: -self.shy * inv,
            sy: self.sx * inv,
            ty: (self.shy * self.tx - self.sx * self.ty) * inv,
        })
    }
}

/// Which sprite the pointer is showing. The host uploads whichever slots it
/// has; a slot with no sprite falls back to the dot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorSlot {
    Rest,
    Press,
    RightPress,
    Drag,
}

impl CursorSlot {
    pub fn index(self) -> usize {
        match self {
            Self::Rest => 0,
            Self::Press => 1,
            Self::RightPress => 2,
            Self::Drag => 3,
        }
    }
}

/// The pointer in canvas pixels, ready to draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CursorDraw {
    pub x: f32,
    pub y: f32,
    pub alpha: f32,
    pub slot: CursorSlot,
    /// Sprite edge in canvas pixels, press scale already applied.
    pub sprite_px: f32,
    /// Dot radius in canvas pixels, for when the slot has no sprite.
    pub dot_radius_px: f32,
    pub highlight: Option<HighlightDraw>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HighlightDraw {
    pub x: f32,
    pub y: f32,
    pub radius_px: f32,
    pub color: Srgba,
    pub alpha: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundParams {
    Solid(Srgba),
    Gradient(Box<Gradient>),
    Asset { kind: String, value: String },
}

/// The card's destination rect in canvas pixels, after scene animation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DestRect {
    pub x: f32,
    pub y: f32,
    pub w: f32,
    pub h: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShadowParams {
    pub color: recast_color::Srgba,
    /// 0..1, already divided out of the authored 0..100.
    pub opacity: f32,
    pub blur_px: f32,
    pub spread_px: f32,
    pub offset_y_px: f32,
    pub center_x: f32,
    pub center_y: f32,
    pub half_w: f32,
    pub half_h: f32,
    pub radius_px: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerParams {
    pub id: LayerId,
    pub visible: bool,
    pub opacity: f32,
    pub transform: Affine2,
    /// Where the card lands on the canvas, after scene animation.
    pub dest: DestRect,
    /// Radians, about the card centre.
    pub rotate: f32,
    /// Fraction of the shorter video edge, 0..0.5.
    pub corner_radius: f32,
    pub blur: f32,
    /// Authored 0..1 strength of the dolly blur.
    pub motion_blur: f32,
    /// Zoom focus in source UV, which is the centre the radial blur streaks from.
    pub zoom_center: [f32; 2],
    /// Signed rate of change of the zoom scale, in scale units per output second. Drives motion blur, which must fire during a ramp and not during the hold.
    pub zoom_velocity: f32,
    /// Crop the source to the card's aspect instead of stretching to it. The
    /// screen layer never needs it (its card matches the source); a camera
    /// bubble always does, since a 16:9 sensor lands in a square.
    pub cover_fit: bool,
    /// Painted immediately before this layer. A shadow belongs to its layer:
    /// drawing them all up front put the bubble's under the opaque screen card.
    pub shadow: Option<ShadowParams>,
    /// Whether this layer samples a decoded picture. Cursor and annotation
    /// layers are evaluated as cards but drawn by their own passes, so only
    /// the screen and the camera oblige the host to bind a texture.
    pub needs_texture: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameParams {
    pub geometry: CanvasGeometry,
    pub background: BackgroundParams,
    pub background_blur: f32,
    /// Where the pointer sits this frame, in SOURCE uv before the zoom. `None`
    /// when there is no cursor layer, no track, or the layer is disabled.
    pub cursor: Option<CursorPlacement>,
    /// The same pointer, resolved onto the canvas. Computed once here so the
    /// sprite pass and the host overlay cannot disagree about where it is.
    pub cursor_draw: Option<CursorDraw>,
    pub layers: Vec<LayerParams>,
    /// In draw order: z-index, then insertion order.
    pub annotations: Vec<AnnotationParams>,
    /// Where `output_time` lands on the original recording axis.
    pub source_time: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceGeometry {
    pub width: u32,
    pub height: u32,
}

/// Central difference step for the zoom-velocity estimate. Half a frame at
/// 60 fps: small enough to track a ramp, large enough to survive f64 noise.
const VELOCITY_DT: f64 = 1.0 / 120.0;

pub struct Evaluator {
    time_map: TimeMap,
    segments: Vec<Segment>,
    geometry: CanvasGeometry,
    source: SourceGeometry,
}

impl Evaluator {
    pub fn new(scene: &Scene, source: SourceGeometry) -> Self {
        Self::with_time_map(scene, source, None)
    }

    /// `time_map` replaces the one derived from the scene. The editor resolves
    /// the axis itself (cut lanes and flags it owns can drop a cut the scene
    /// still carries), so the preview hands its map over rather than letting
    /// two authorities disagree about what output time means.
    pub fn with_time_map(scene: &Scene, source: SourceGeometry, time_map: Option<TimeMap>) -> Self {
        Self {
            time_map: time_map.unwrap_or_else(|| scene.timeline.time_map()),
            segments: scene.timeline.segments(),
            geometry: canvas_geometry(
                source.width,
                source.height,
                scene.output.padding,
                scene.output.aspect.as_deref(),
            ),
            source,
        }
    }

    pub fn geometry(&self) -> CanvasGeometry {
        self.geometry
    }

    pub fn output_duration(&self) -> f64 {
        self.time_map.output_duration
    }

    /// The axis in force, so a caller that has to project a time itself (a
    /// caption's chunk start, say) uses the same one the frames do.
    pub fn time_map(&self) -> Option<&TimeMap> {
        Some(&self.time_map)
    }

    /// `output_time` is gapless output-timeline seconds. Everything else in the
    /// scene is authored on the original recording axis, so the projection
    /// happens here, once, and every effect composes with cuts and speed for free.
    pub fn evaluate(&self, scene: &Scene, output_time: f64) -> FrameParams {
        let source_time = output_to_original(&self.time_map, output_time);

        let cursor = self.cursor_placement(scene, source_time);
        let focus = scene.flags.focus;
        let mut background = BackgroundParams::Solid(Srgba::opaque(0x11, 0x11, 0x11));
        let mut background_blur = 0.0f32;
        let mut layers = Vec::with_capacity(scene.layers.len());
        let mut card = (
            DestRect {
                x: 0.0,
                y: 0.0,
                w: 0.0,
                h: 0.0,
            },
            Affine2::IDENTITY,
        );

        for layer in &scene.layers {
            match &layer.source {
                LayerSource::Solid { color } => {
                    background = BackgroundParams::Solid(*color);
                    background_blur = blur_of(layer);
                }
                LayerSource::Gradient { gradient } => {
                    background = BackgroundParams::Gradient(Box::new(gradient.clone()));
                    background_blur = blur_of(layer);
                }
                LayerSource::Asset { kind, value } => {
                    background = BackgroundParams::Asset {
                        kind: kind.clone(),
                        value: value.clone(),
                    };
                    background_blur = blur_of(layer);
                }
                LayerSource::Camera(settings) => {
                    let mut params = self.layer_params(layer, source_time, output_time, focus);
                    params.cover_fit = true;
                    let follow: Vec<&ZoomRegion> = match focus {
                        true => scene.zoom_regions(),
                        false => Vec::new(),
                    };
                    if let Some(bubble) =
                        bubble_params(settings, &follow, source_time, self.geometry)
                    {
                        params.dest = bubble.dest;
                        params.corner_radius = bubble.corner_radius;
                        params.transform = bubble.transform;
                        if params.visible {
                            params.shadow = bubble_shadow(settings, &bubble);
                        }
                    } else {
                        params.visible = false;
                    }
                    layers.push(params);
                }
                _ => {
                    let mut params = self.layer_params(layer, source_time, output_time, focus);
                    if matches!(layer.source, LayerSource::Screen) {
                        card = (params.dest, params.transform);
                        if params.visible {
                            params.shadow = shadow_params(layer, &params, self.geometry);
                        }
                    }
                    layers.push(params);
                }
            }
        }

        FrameParams {
            geometry: self.geometry,
            background,
            background_blur,
            cursor_draw: cursor.and_then(|c| self.cursor_draw(scene, c, card.0, card.1)),
            cursor,
            annotations: match scene.flags.annotations {
                true => self.annotations(scene, source_time, card.0, card.1),
                false => Vec::new(),
            },
            layers,
            source_time,
        }
    }

    /// The pointer for this frame. Every curve lives in `recast-cursor`, which the TypeScript preview asserts against the same fixture, so preview and export cannot drift.
    fn cursor_placement(&self, scene: &Scene, source_time: f64) -> Option<CursorPlacement> {
        let track = scene.cursor_track.as_ref()?;
        let layer = scene
            .layers
            .iter()
            .find(|l| matches!(l.source, LayerSource::Cursor(_)))?;
        if layer.hidden {
            return None;
        }
        let LayerSource::Cursor(spec) = &layer.source else {
            return None;
        };

        let settings = CursorSettings {
            hide_when_idle: spec.hide_when_idle,
            idle_timeout: spec.idle_timeout,
            highlight_clicks: spec.highlight_clicks,
            highlight_opacity: spec.highlight_opacity,
        };
        let ts_us = (source_time * 1_000_000.0).round() as i64;
        let source = (self.source.width, self.source.height);
        match spec.motion_easing {
            Some(easing) => track.resolve(ts_us, source, settings, |t| easing.y(t as f32) as f64),
            None => track.resolve(ts_us, source, settings, |t| t),
        }
    }

    /// Projects the pointer onto the canvas: the INVERSE of the card transform
    /// (which maps a destination uv to the source uv it samples) and then the
    /// card rect. Doing it anywhere else would be a second evaluator to keep in
    /// step with the shader.
    fn cursor_draw(
        &self,
        scene: &Scene,
        cursor: CursorPlacement,
        dest: DestRect,
        transform: Affine2,
    ) -> Option<CursorDraw> {
        let inverse = transform.invert()?;
        let place = |x: f64, y: f64| {
            let (u, v) = inverse.apply(x as f32, y as f32);
            (dest.x + u * dest.w, dest.y + v * dest.h)
        };

        let spec = scene.layers.iter().find_map(|l| match &l.source {
            LayerSource::Cursor(spec) => Some(spec),
            _ => None,
        })?;
        // Canvas pixels per source pixel, the same scale padding uses.
        let sx = dest.w / self.source.width.max(1) as f32;

        let (x, y) = place(cursor.x, cursor.y);
        let slot = match (cursor.pressed, cursor.dragging, cursor.right) {
            (true, true, _) => CursorSlot::Drag,
            (true, false, true) => CursorSlot::RightPress,
            (true, false, false) => CursorSlot::Press,
            (false, _, _) => CursorSlot::Rest,
        };
        let size = spec.size as f32;

        Some(CursorDraw {
            x,
            y,
            alpha: cursor.alpha as f32,
            slot,
            sprite_px: size * 16.0 * sx * cursor.scale as f32,
            dot_radius_px: (size * 2.0 * sx * cursor.scale as f32).max(2.0),
            highlight: cursor.highlight.map(|h| {
                let (hx, hy) = place(h.x, h.y);
                HighlightDraw {
                    x: hx,
                    y: hy,
                    // Three times the dot, matching the ring the preview drew.
                    radius_px: (size * 6.0 * sx).max(6.0),
                    color: parse_hex_or_blue(&spec.highlight_color),
                    alpha: h.alpha as f32,
                }
            }),
        })
    }

    fn annotations(
        &self,
        scene: &Scene,
        source_time: f64,
        dest: DestRect,
        transform: Affine2,
    ) -> Vec<AnnotationParams> {
        let all = scene.annotations();
        sorted_visible(&all)
            .into_iter()
            .filter_map(|index| {
                annotation_params(all[index], source_time, self.geometry, dest, transform)
            })
            .collect()
    }

    fn layer_params(
        &self,
        layer: &recast_scene::Layer,
        source_time: f64,
        output_time: f64,
        focus: bool,
    ) -> LayerParams {
        let zooms: Vec<&ZoomRegion> = match focus {
            true => layer
                .effects
                .iter()
                .filter_map(|e| match e {
                    Effect::Zoom(z) => Some(&**z),
                    _ => None,
                })
                .collect(),
            false => Vec::new(),
        };

        let zoom = active_zoom(&zooms, source_time);
        let transform = match zoom {
            Some(region) => Affine2::zoom_ramp(
                region.scale_at(source_time) as f32,
                region.scale as f32,
                region.center_x as f32,
                region.center_y as f32,
            ),
            None => Affine2::IDENTITY,
        };
        let (motion_blur, zoom_center) = match zoom {
            Some(region) => (
                region.motion_blur.clamp(0.0, 1.0) as f32,
                [region.center_x as f32, region.center_y as f32],
            ),
            None => (0.0, [0.5, 0.5]),
        };

        let anim = self.scene_anim(layer, source_time);
        let (dest, rotate) = self.place(anim);

        LayerParams {
            id: layer.id,
            visible: !layer.hidden,
            opacity: layer.opacity as f32 * anim.opacity as f32,
            transform,
            dest,
            rotate,
            corner_radius: corner_radius_of(layer),
            blur: blur_of(layer),
            motion_blur,
            zoom_center,
            zoom_velocity: self.zoom_velocity(&zooms, output_time),
            cover_fit: false,
            shadow: None,
            needs_texture: matches!(layer.source, LayerSource::Screen | LayerSource::Camera(_)),
        }
    }

    /// The entrance/exit transform for whichever segment contains `source_time`,
    /// or identity. Anchored to the segment's ORIGINAL start, so a cut or a trim
    /// that orphans an anchor drops the animation instead of misplacing it.
    fn scene_anim(&self, layer: &recast_scene::Layer, source_time: f64) -> AnimTransform {
        let anims: Vec<&SegmentAnim> = layer
            .effects
            .iter()
            .filter_map(|e| match e {
                Effect::SceneAnim(a) => Some(&**a),
                _ => None,
            })
            .collect();
        if anims.is_empty() {
            return AnimTransform::IDENTITY;
        }
        let Some(segment) = self
            .segments
            .iter()
            .find(|s| source_time >= s.start - ANCHOR_EPS && source_time < s.end)
        else {
            return AnimTransform::IDENTITY;
        };
        let Some(anim) = anims
            .iter()
            .find(|a| (a.start - segment.start).abs() <= ANCHOR_EPS)
        else {
            return AnimTransform::IDENTITY;
        };
        eval_segment_anim(anim, source_time, segment.start, segment.end)
    }

    /// Translation is a fraction of the CANVAS, scale is about the card's own
    /// centre. Both match the expressions the FFmpeg overlay path builds.
    fn place(&self, anim: AnimTransform) -> (DestRect, f32) {
        let g = self.geometry;
        let (w, h) = (g.video_w as f64, g.video_h as f64);
        let scaled_w = w * anim.scale;
        let scaled_h = h * anim.scale;
        let dest = DestRect {
            x: (g.video_x as f64 + anim.tx * g.canvas_w as f64 - (scaled_w - w) / 2.0) as f32,
            y: (g.video_y as f64 + anim.ty * g.canvas_h as f64 - (scaled_h - h) / 2.0) as f32,
            w: scaled_w as f32,
            h: scaled_h as f32,
        };
        (dest, anim.rotate.to_radians() as f32)
    }

    /// Differentiated on the OUTPUT axis, so a sped-up segment blurs harder for
    /// the same authored ramp, which is what the viewer actually sees.
    fn zoom_velocity(&self, zooms: &[&ZoomRegion], output_time: f64) -> f32 {
        if zooms.is_empty() {
            return 0.0;
        }
        let sample = |t: f64| {
            let source = output_to_original(&self.time_map, t);
            active_zoom(zooms, source)
                .map(|r| r.scale_at(source))
                .unwrap_or(1.0)
        };
        let before = sample(output_time - VELOCITY_DT);
        let after = sample(output_time + VELOCITY_DT);
        ((after - before) / (2.0 * VELOCITY_DT)) as f32
    }
}

const ANCHOR_EPS: f64 = 1e-4;
const MIN_ANIM_MS: f64 = 100.0;
const MAX_ANIM_MS: f64 = 2000.0;
const DEFAULT_ANIM_MS: f64 = 500.0;
const DEFAULT_SLIDE: f64 = 0.6;
const DEFAULT_SCALE_DELTA: f64 = 0.3;
const DEFAULT_POP_DELTA: f64 = 0.35;
const DEFAULT_ROTATE_DEG: f64 = 15.0;
/// Anti-wobble guards: a segment shorter than this stays static, and each ramp
/// caps to this fraction of the window, so aggressive silence cuts cannot leave
/// a fragment in a permanent in-to-out oscillation.
const MIN_ANIMATABLE_SEC: f64 = 0.2;
const MAX_SIDE_FRACTION: f64 = 0.4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimTransform {
    pub tx: f64,
    pub ty: f64,
    pub scale: f64,
    pub rotate: f64,
    pub opacity: f64,
}

impl AnimTransform {
    pub const IDENTITY: Self = Self {
        tx: 0.0,
        ty: 0.0,
        scale: 1.0,
        rotate: 0.0,
        opacity: 1.0,
    };
}

fn clamp_anim_ms(ms: f64) -> f64 {
    if ms.is_finite() {
        ms.clamp(MIN_ANIM_MS, MAX_ANIM_MS)
    } else {
        DEFAULT_ANIM_MS
    }
}

/// `presence` 1 = resting, 0 = fully animated away. A bouncy easing may push it
/// past either end, which is the overshoot that makes the motion read as physical.
fn presence(spec: &recast_scene::v1::SceneAnimSpec, p: f64) -> AnimTransform {
    let mut t = AnimTransform::IDENTITY;
    match spec.kind.as_str() {
        "fade" => t.opacity = p.clamp(0.0, 1.0),
        "slide" => {
            let d = spec.intensity.unwrap_or(DEFAULT_SLIDE);
            let off = (1.0 - p) * d;
            match spec.dir.as_deref().unwrap_or("left") {
                "right" => t.tx = off,
                "up" => t.ty = -off,
                "down" => t.ty = off,
                _ => t.tx = -off,
            }
        }
        "scale" | "pop" => {
            let amount = spec.intensity.unwrap_or(if spec.kind == "pop" {
                DEFAULT_POP_DELTA
            } else {
                DEFAULT_SCALE_DELTA
            });
            let start = 1.0 - amount;
            t.scale = start + (1.0 - start) * p;
        }
        "shrink" => {
            let amount = spec.intensity.unwrap_or(DEFAULT_SCALE_DELTA);
            let start = 1.0 + amount;
            t.scale = start + (1.0 - start) * p;
        }
        "rotate" => t.rotate = (1.0 - p) * spec.intensity.unwrap_or(DEFAULT_ROTATE_DEG),
        _ => {}
    }
    t
}

fn eval_segment_anim(anim: &SegmentAnim, t: f64, start: f64, end: f64) -> AnimTransform {
    let window = (end - start).max(0.0);
    if window < MIN_ANIMATABLE_SEC {
        return AnimTransform::IDENTITY;
    }
    let max_side = window * MAX_SIDE_FRACTION;
    if let Some(spec) = &anim.anim_in {
        let d = (clamp_anim_ms(spec.duration_ms) / 1000.0).min(max_side);
        if d > 0.0 && t < start + d {
            let phase = ((t - start) / d).clamp(0.0, 1.0);
            return presence(spec, spec.easing.y(phase as f32) as f64);
        }
    }
    if let Some(spec) = &anim.anim_out {
        let d = (clamp_anim_ms(spec.duration_ms) / 1000.0).min(max_side);
        if d > 0.0 && t > end - d {
            let phase = ((end - t) / d).clamp(0.0, 1.0);
            return presence(spec, spec.easing.y(phase as f32) as f64);
        }
    }
    AnimTransform::IDENTITY
}

/// The shadow is cast by the card's rect, so it follows scene animation. Mirrors
/// `render_drop_shadow_mask`: SDF against the spread-expanded rect, coverage
/// smoothstepped over the blur distance.
fn shadow_params(
    layer: &recast_scene::Layer,
    params: &LayerParams,
    geometry: CanvasGeometry,
) -> Option<ShadowParams> {
    let settings: &ShadowSettings = layer.effects.iter().find_map(|e| match e {
        Effect::DropShadow(s) => Some(&**s),
        _ => None,
    })?;
    if !settings.enabled || settings.opacity <= 0.0 {
        return None;
    }
    if geometry.canvas_w == 0 || geometry.canvas_h == 0 {
        return None;
    }

    let half_w = params.dest.w as f64 / 2.0;
    let half_h = params.dest.h as f64 / 2.0;
    let spread = settings.spread.max(0.0);
    let radius_px = (params.corner_radius as f64 * params.dest.w.min(params.dest.h) as f64
        + spread * 0.5)
        .min((half_w + spread).min(half_h + spread))
        .max(0.0);

    Some(ShadowParams {
        color: recast_color::parse_css_color(&settings.color).unwrap_or(Srgba::opaque(0, 0, 0)),
        opacity: (settings.opacity / 100.0).clamp(0.0, 1.0) as f32,
        blur_px: settings.blur.max(0.5) as f32,
        spread_px: spread as f32,
        offset_y_px: settings.offset_y as f32,
        center_x: params.dest.x + half_w as f32,
        center_y: params.dest.y + half_h as f32,
        half_w: half_w as f32,
        half_h: half_h as f32,
        radius_px: radius_px as f32,
    })
}

/// The tightest zoom in force: exactly one region applies at any instant, so overlaps never stack. Ties keep the later start.
/// Picking by SCALE, not latest start, is what stops an overlap flickering; latest-start-wins snapped 2.00x to 1.03x in a frame as the incoming ramp restarts at 1.
fn active_zoom<'a>(regions: &[&'a ZoomRegion], t: f64) -> Option<&'a ZoomRegion> {
    let mut best: Option<(&ZoomRegion, f64)> = None;
    for region in regions {
        if region.hidden || t <= region.start || t >= region.end {
            continue;
        }
        let scale = region.scale_at(t);
        match best {
            Some((current, best_scale))
                if best_scale > scale || (best_scale == scale && region.start < current.start) => {}
            _ => best = Some((region, scale)),
        }
    }
    best.map(|(region, _)| region)
}

/// The picker always writes a hex string, so a parse failure means corrupt
/// state rather than a colour the user chose.
fn parse_hex_or_blue(value: &str) -> Srgba {
    recast_color::parse_hex(value).unwrap_or(Srgba::opaque(0x3b, 0x82, 0xf6))
}

fn blur_of(layer: &recast_scene::Layer) -> f32 {
    layer
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::Blur { amount } => Some(*amount as f32),
            _ => None,
        })
        .unwrap_or(0.0)
}

fn corner_radius_of(layer: &recast_scene::Layer) -> f32 {
    layer
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::CornerRadius { percent } => Some((*percent / 100.0) as f32),
            _ => None,
        })
        .unwrap_or(0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_scene::migrate::to_scene;
    use recast_scene::v1::RenderState;

    const BASE: &str = r##"{
        "trimStart": 0.0, "trimEnd": 10.0,
        "backgroundType": "color", "backgroundValue": "#0f172a", "backgroundBlur": 0.0,
        "padding": 0.0, "cursorEnabled": false, "cursorSize": 3.0, "cursorSmoothing": 50.0,
        "cursorHighlightClicks": true, "cursorHighlightColor": "#3b82f6",
        "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
        "zoomRegions": []
    }"##;

    /// `extra` is a comma-terminated object fragment merged over `BASE`. Merging
    /// rather than splicing, because serde rejects a duplicate key outright.
    fn state(extra: &str) -> RenderState {
        let mut base: serde_json::Value = serde_json::from_str(BASE).expect("base json");
        let fragment = extra.trim().trim_end_matches(',');
        if !fragment.is_empty() {
            let overrides: serde_json::Value =
                serde_json::from_str(&format!("{{{fragment}}}")).expect("override json");
            let (Some(base), Some(overrides)) = (base.as_object_mut(), overrides.as_object())
            else {
                panic!("fixtures must be JSON objects");
            };
            for (key, value) in overrides {
                base.insert(key.clone(), value.clone());
            }
        }
        serde_json::from_value(base).expect("render state")
    }

    fn scene_with(extra: &str) -> recast_scene::Scene {
        to_scene(&state(extra))
    }

    fn source() -> SourceGeometry {
        SourceGeometry {
            width: 1920,
            height: 1080,
        }
    }

    fn screen_layer(params: &FrameParams) -> &LayerParams {
        params.layers.first().expect("a screen layer")
    }

    /// A cursor layer is evaluated as a card so its zoom and animation resolve
    /// with everything else, but its pixels come from the sprite pass. Marking
    /// it as needing a texture makes an export refuse a scene it renders fine.
    #[test]
    fn only_the_screen_and_camera_layers_need_a_bound_texture() {
        let scene = scene_with(r#""cursorEnabled": true,"#);
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 1.0);
        let sources: Vec<(bool, bool)> = scene
            .layers
            .iter()
            .filter_map(|l| {
                let params = params.layers.iter().find(|p| p.id == l.id)?;
                let picture = matches!(
                    l.source,
                    recast_scene::LayerSource::Screen | recast_scene::LayerSource::Camera(_)
                );
                Some((picture, params.needs_texture))
            })
            .collect();
        assert!(!sources.is_empty(), "the fixture has no evaluated layers");
        assert!(
            sources.iter().all(|(picture, needs)| picture == needs),
            "{sources:?}"
        );
    }

    #[test]
    fn an_unzoomed_frame_is_the_identity_transform() {
        let scene = scene_with("");
        let ev = Evaluator::new(&scene, source());
        let params = ev.evaluate(&scene, 1.0);
        assert!(screen_layer(&params).transform.is_identity());
    }

    #[test]
    fn a_zoom_samples_a_smaller_window_of_the_source() {
        let scene = scene_with(
            r#""zoomRegions": [{"start":1.0,"end":5.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let t = screen_layer(&ev.evaluate(&scene, 3.0)).transform;
        assert!((t.sx - 0.5).abs() < 1e-6);
        assert!((t.sy - 0.5).abs() < 1e-6);
        assert!((t.tx - 0.25).abs() < 1e-6);
    }

    #[test]
    fn the_zoom_window_is_clamped_so_it_never_leaves_the_frame() {
        let t = Affine2::zoom(2.0, 0.0, 1.0);
        let (u0, v0) = t.apply(0.0, 0.0);
        let (u1, v1) = t.apply(1.0, 1.0);
        assert!(u0 >= -1e-6 && v0 >= -1e-6, "{u0} {v0}");
        assert!(u1 <= 1.0 + 1e-6 && v1 <= 1.0 + 1e-6, "{u1} {v1}");
    }

    #[test]
    fn a_scale_below_one_is_not_a_zoom_out() {
        assert!(Affine2::zoom(0.4, 0.5, 0.5).is_identity());
    }

    #[test]
    fn the_ramp_reaches_full_scale_and_holds() {
        let scene = scene_with(
            r#""zoomRegions": [{"start":1.0,"end":5.0,"scale":2.0,"rampIn":1.0,"rampOut":1.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let at = |t: f64| 1.0 / screen_layer(&ev.evaluate(&scene, t)).transform.sx;
        assert!((at(3.0) - 2.0).abs() < 1e-4, "hold: {}", at(3.0));
        assert!(at(1.2) > 1.0 && at(1.2) < 2.0, "ramp in: {}", at(1.2));
        assert!(at(4.8) > 1.0 && at(4.8) < 2.0, "ramp out: {}", at(4.8));
    }

    /// The FFmpeg path collapsed to a flat 1.0 at 40 regions because the
    /// piecewise LUT had to fit av_expr_parse's 48-term budget. There is no
    /// budget here: each region is evaluated directly.
    #[test]
    fn forty_zoom_regions_still_zoom() {
        let regions: Vec<String> = (0..40)
            .map(|i| {
                let start = i as f64 * 0.2;
                format!(
                    r#"{{"start":{start},"end":{},"scale":1.9,"rampIn":0.05,"rampOut":0.05,"centerX":0.5,"centerY":0.5}}"#,
                    start + 0.15
                )
            })
            .collect();
        let scene = scene_with(&format!(r#""zoomRegions": [{}],"#, regions.join(",")));
        let ev = Evaluator::new(&scene, source());

        let mid = screen_layer(&ev.evaluate(&scene, 0.075)).transform;
        assert!(
            (1.0 / mid.sx - 1.9).abs() < 1e-3,
            "40 regions collapsed the zoom to {}",
            1.0 / mid.sx
        );
    }

    /// D-1: an off-centre zoom must pan STRAIGHT toward its target as it ramps.

    #[test]
    fn an_off_centre_zoom_pans_along_a_straight_line() {
        let scene = scene_with(
            r#""zoomRegions": [{"start":1.0,"end":6.0,"scale":4.0,"rampIn":2.0,"rampOut":0.0,"centerX":0.8,"centerY":0.65}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        // Window centre in source uv.
        let centre = |t: f64| {
            let tr = screen_layer(&ev.evaluate(&scene, t)).transform;
            (tr.tx + tr.sx * 0.5, tr.ty + tr.sy * 0.5)
        };
        let (x0, y0) = centre(1.001);
        let mut ratio: Option<f32> = None;
        let mut t = 1.2;
        while t < 3.0 {
            let (x, y) = centre(t);
            let (dx, dy) = (x - x0, y - y0);
            if dx.abs() > 1e-4 && dy.abs() > 1e-4 {
                let r = dx / dy;
                if let Some(r0) = ratio {
                    assert!(
                        (r - r0).abs() < 2e-3,
                        "pan curved: ratio {r0} -> {r} at t {t:.2}"
                    );
                }
                ratio = Some(r);
            }
            t += 0.1;
        }
        assert!(ratio.is_some(), "the ramp never moved the centre");
        let (xh, yh) = centre(4.0);
        assert!(
            (xh - 0.8).abs() < 1e-3 && (yh - 0.65).abs() < 1e-3,
            "hold centre landed at ({xh}, {yh}), not the authored target"
        );
    }

    #[test]
    fn the_latest_starting_region_wins_an_overlap() {
        let scene = scene_with(
            r#""zoomRegions": [
                {"start":0.0,"end":10.0,"scale":1.5,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5},
                {"start":4.0,"end":6.0,"scale":3.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}
            ],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let inner = 1.0 / screen_layer(&ev.evaluate(&scene, 5.0)).transform.sx;
        let outer = 1.0 / screen_layer(&ev.evaluate(&scene, 1.0)).transform.sx;
        assert!((inner - 3.0).abs() < 1e-4, "nested region lost: {inner}");
        assert!((outer - 1.5).abs() < 1e-4, "outer region lost: {outer}");
    }

    /// FFmpeg SUMMED overlapping regions: 1.5 + 3.0 previewed as one and exported as the other. Nothing can sum here, since exactly one region is selected per instant.
    #[test]
    fn overlapping_regions_never_sum() {
        let scene = scene_with(
            r#""zoomRegions": [
                {"start":0.0,"end":10.0,"scale":1.5,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5},
                {"start":4.0,"end":6.0,"scale":3.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}
            ],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let scale = 1.0 / screen_layer(&ev.evaluate(&scene, 5.0)).transform.sx;
        assert!(scale <= 3.0 + 1e-4, "regions stacked to {scale}");
    }

    /// The visible defect: latest-start-wins handed over to the incoming region
    /// at its own ramp start, so an overlap snapped 2.00x to 1.03x in one frame
    /// and everything riding the zoom (the card, a video-anchored annotation,
    /// the camera bubble) flickered with it.
    #[test]
    fn an_overlap_hands_over_without_a_step() {
        let scene = scene_with(
            r#""zoomRegions": [
                {"start":1.0,"end":8.0,"scale":2.0,"rampIn":0.5,"rampOut":0.5,"centerX":0.5,"centerY":0.5},
                {"start":5.0,"end":12.0,"scale":2.5,"rampIn":0.5,"rampOut":0.5,"centerX":0.5,"centerY":0.5}
            ],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let step = 1.0 / 60.0;
        // A ramp covers 1.0 of scale in 0.5s, so one 60 fps frame moves it about 0.06; more than that is a jump.
        let budget = 0.08;
        let mut previous: Option<f32> = None;
        let mut t = 0.5;
        while t < 12.5 {
            let sx = screen_layer(&ev.evaluate(&scene, t)).transform.sx;
            if let Some(previous) = previous {
                assert!(
                    (sx - previous).abs() <= budget,
                    "zoom stepped by {} at t {t:.3} ({previous} -> {sx})",
                    (sx - previous).abs()
                );
            }
            previous = Some(sx);
            t += step;
        }
    }

    /// The tightest region wins, so a wide zoom cannot cancel a tight one it
    /// happens to start after.
    #[test]
    fn the_tighter_of_two_overlapping_zooms_is_the_one_in_force() {
        let scene = scene_with(
            r#""zoomRegions": [
                {"start":0.0,"end":10.0,"scale":3.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5},
                {"start":4.0,"end":6.0,"scale":1.2,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}
            ],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let scale = 1.0 / screen_layer(&ev.evaluate(&scene, 5.0)).transform.sx;
        assert!(
            (scale - 3.0).abs() < 1e-4,
            "the looser region won, at {scale}"
        );
    }

    /// The editor gates ALL zoom on the focus lane's switch. Without the same
    /// gate the engine zooms where the editor shows none.
    #[test]
    fn the_focus_switch_turns_every_zoom_off() {
        let on = scene_with(
            r#""zoomRegions": [{"start":1.0,"end":5.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        let off = scene_with(
            r#""focusEnabled": false, "zoomRegions": [{"start":1.0,"end":5.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        assert!(!off.flags.focus, "the fixture must turn focus off");
        let at = |scene: &Scene| {
            Evaluator::new(scene, source())
                .evaluate(scene, 3.0)
                .layers
                .iter()
                .find(|l| l.id == scene.screen_layer().expect("screen").id)
                .expect("screen params")
                .transform
        };
        assert_ne!(at(&on), Affine2::IDENTITY, "the fixture must zoom");
        assert_eq!(at(&off), Affine2::IDENTITY);
        // The regions stay authored, so turning the lane back on restores them.
        assert_eq!(off.zoom_regions().len(), 1);
    }

    #[test]
    fn the_annotation_switch_draws_none_of_them() {
        let json = r#""annotations": [{"id":"a1","start":0.0,"end":9.0,"kind":{"kind":"rect","x":0.2,"y":0.3,"w":0.4,"h":0.2}}],"#;
        let on = scene_with(json);
        let off = scene_with(&(r#""annotationsEnabled": false, "#.to_string() + json));
        assert!(
            !Evaluator::new(&on, source())
                .evaluate(&on, 1.0)
                .annotations
                .is_empty(),
            "the fixture must draw one"
        );
        assert!(Evaluator::new(&off, source())
            .evaluate(&off, 1.0)
            .annotations
            .is_empty());
    }

    #[test]
    fn a_hidden_region_does_not_apply() {
        let scene = scene_with(
            r#""zoomRegions": [{"start":1.0,"end":5.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5,"hidden":true}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        assert!(screen_layer(&ev.evaluate(&scene, 3.0))
            .transform
            .is_identity());
    }

    #[test]
    fn zoom_velocity_fires_on_a_ramp_and_not_on_the_hold() {
        let scene = scene_with(
            r#""zoomRegions": [{"start":1.0,"end":5.0,"scale":2.0,"rampIn":1.0,"rampOut":1.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let ramping = screen_layer(&ev.evaluate(&scene, 1.5)).zoom_velocity;
        let holding = screen_layer(&ev.evaluate(&scene, 3.0)).zoom_velocity;
        let falling = screen_layer(&ev.evaluate(&scene, 4.5)).zoom_velocity;
        assert!(ramping > 0.1, "ramp in velocity {ramping}");
        assert!(holding.abs() < 1e-3, "hold velocity {holding}");
        assert!(falling < -0.1, "ramp out velocity {falling}");
    }

    /// Zoom is authored on the ORIGINAL axis and rendered on the OUTPUT axis.
    /// Cuts and speed shift the two apart, and the projection is the only thing
    /// that has to know: the zoom evaluator is unchanged.
    #[test]
    fn a_cut_before_a_zoom_shifts_it_on_the_output_axis() {
        let scene = scene_with(
            r#""cuts": [{"start":1.0,"end":3.0}],
               "zoomRegions": [{"start":5.0,"end":7.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let at = |t: f64| 1.0 / screen_layer(&ev.evaluate(&scene, t)).transform.sx;
        assert!(
            (at(4.0) - 2.0).abs() < 1e-4,
            "zoom moved left by the cut: {}",
            at(4.0)
        );
        assert!(
            (at(6.5) - 1.0).abs() < 1e-4,
            "zoom outstayed the cut: {}",
            at(6.5)
        );
    }

    #[test]
    fn a_sped_segment_compresses_a_zoom_on_the_output_axis() {
        let scene = scene_with(
            r#""splitPoints": [4.0], "segmentSpeeds": [{"start":4.0,"speed":2.0}],
               "zoomRegions": [{"start":6.0,"end":8.0,"scale":2.0,"rampIn":0.0,"rampOut":0.0,"centerX":0.5,"centerY":0.5}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let at = |t: f64| 1.0 / screen_layer(&ev.evaluate(&scene, t)).transform.sx;
        assert!(
            (at(5.5) - 2.0).abs() < 1e-4,
            "zoom at output 5.5: {}",
            at(5.5)
        );
        assert!(
            (at(4.5) - 1.0).abs() < 1e-4,
            "zoom started early: {}",
            at(4.5)
        );
    }

    /// Every shadow in the frame, in draw order, for tests that assert on them.
    fn shadows_of(params: &FrameParams) -> Vec<ShadowParams> {
        params.layers.iter().filter_map(|l| l.shadow).collect()
    }

    const SHADOW: &str = r##""shadow": {"enabled": true, "blur": 40.0, "spread": 4.0,
        "offsetY": 24.0, "opacity": 50.0, "color": "#000000"},"##;

    #[test]
    fn a_disabled_shadow_produces_no_pass() {
        let scene = scene_with("");
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 0.0);
        assert!(shadows_of(&params).is_empty());
    }

    #[test]
    fn a_zero_opacity_shadow_produces_no_pass() {
        let scene = scene_with(
            r##""shadow": {"enabled": true, "blur": 40.0, "opacity": 0.0, "color": "#000000"},"##,
        );
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 0.0);
        assert!(shadows_of(&params).is_empty());
    }

    #[test]
    fn an_enabled_shadow_is_centred_on_the_card_and_normalises_its_opacity() {
        let scene = scene_with(&format!(r#"{SHADOW} "padding": 10.0,"#));
        let ev = Evaluator::new(&scene, source());
        let params = ev.evaluate(&scene, 0.0);
        let shadow = shadows_of(&params)[0];
        let g = ev.geometry();

        assert!((shadow.center_x - (g.video_x as f32 + g.video_w as f32 / 2.0)).abs() < 1e-3);
        assert!((shadow.opacity - 0.5).abs() < 1e-6);
        assert_eq!(shadow.offset_y_px, 24.0);
        assert_eq!(shadow.spread_px, 4.0);
    }

    /// The old rasteriser floored blur at 0.5 because a zero-width smoothstep is
    /// a division by zero in the shader.
    #[test]
    fn a_zero_blur_shadow_is_floored_rather_than_dividing_by_zero() {
        let scene = scene_with(
            r##""shadow": {"enabled": true, "blur": 0.0, "opacity": 50.0, "color": "#000000"},"##,
        );
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 0.0);
        let shadow = shadows_of(&params)[0];
        assert!(shadow.blur_px >= 0.5);
    }

    #[test]
    fn the_shadow_radius_degrades_to_a_full_ellipse_rather_than_inverting() {
        let scene = scene_with(&format!(r#"{SHADOW} "borderRadius": 50.0,"#));
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 0.0);
        let shadow = shadows_of(&params)[0];
        assert!(shadow.radius_px <= shadow.half_h + shadow.spread_px + 1e-3);
        assert!(shadow.radius_px >= 0.0);
    }

    #[test]
    fn an_unanimated_card_sits_exactly_on_the_video_rect() {
        let scene = scene_with(r#""padding": 10.0,"#);
        let ev = Evaluator::new(&scene, source());
        let g = ev.geometry();
        let dest = screen_layer(&ev.evaluate(&scene, 0.0)).dest;
        assert_eq!(dest.x, g.video_x as f32);
        assert_eq!(dest.y, g.video_y as f32);
        assert_eq!(dest.w, g.video_w as f32);
        assert_eq!(dest.h, g.video_h as f32);
    }

    #[test]
    fn a_slide_entrance_offsets_the_card_and_settles_back() {
        let scene = scene_with(
            r#""segmentAnims": [{"start": 0.0, "in": {"kind": "slide", "durationMs": 500.0, "dir": "left"}}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let g = ev.geometry();
        let entering = screen_layer(&ev.evaluate(&scene, 0.0)).dest;
        let settled = screen_layer(&ev.evaluate(&scene, 5.0)).dest;

        assert!(
            entering.x < g.video_x as f32,
            "slide-from-left did not offset: {}",
            entering.x
        );
        assert_eq!(settled.x, g.video_x as f32);
    }

    #[test]
    fn a_scale_entrance_grows_about_the_card_centre() {
        let scene = scene_with(
            r#""segmentAnims": [{"start": 0.0, "in": {"kind": "scale", "durationMs": 500.0}}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let g = ev.geometry();
        let entering = screen_layer(&ev.evaluate(&scene, 0.0)).dest;

        assert!(
            entering.w < g.video_w as f32,
            "did not scale: {}",
            entering.w
        );
        let centre = entering.x + entering.w / 2.0;
        assert!(
            (centre - (g.video_x as f32 + g.video_w as f32 / 2.0)).abs() < 1e-3,
            "the card drifted off centre while scaling: {centre}"
        );
    }

    #[test]
    fn a_fade_entrance_drives_opacity_not_geometry() {
        let scene = scene_with(
            r#""segmentAnims": [{"start": 0.0, "in": {"kind": "fade", "durationMs": 500.0}}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let g = ev.geometry();
        let params = ev.evaluate(&scene, 0.0);
        let entering = screen_layer(&params);
        assert!(entering.opacity < 0.05, "opacity {}", entering.opacity);
        assert_eq!(entering.dest.x, g.video_x as f32);
    }

    #[test]
    fn a_rotate_entrance_produces_radians() {
        let scene = scene_with(
            r#""segmentAnims": [{"start": 0.0, "in": {"kind": "rotate", "durationMs": 500.0}}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let entering = screen_layer(&ev.evaluate(&scene, 0.0)).rotate;
        assert!(
            (entering.abs() - DEFAULT_ROTATE_DEG.to_radians() as f32).abs() < 1e-3,
            "rotate was {entering}"
        );
    }

    /// A silence cut can leave a fragment shorter than the two ramps combined,
    /// which without this guard sits in a permanent in-to-out oscillation.
    #[test]
    fn a_segment_shorter_than_the_guard_stays_static() {
        let scene = scene_with(
            r#""trimEnd": 0.1,
               "segmentAnims": [{"start": 0.0, "in": {"kind": "slide", "durationMs": 500.0}}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let g = ev.geometry();
        assert_eq!(
            screen_layer(&ev.evaluate(&scene, 0.0)).dest.x,
            g.video_x as f32
        );
    }

    #[test]
    fn an_animation_whose_anchor_no_longer_matches_a_segment_is_dropped() {
        let scene = scene_with(
            r#""segmentAnims": [{"start": 7.5, "in": {"kind": "slide", "durationMs": 500.0}}],"#,
        );
        let ev = Evaluator::new(&scene, source());
        let g = ev.geometry();
        assert_eq!(
            screen_layer(&ev.evaluate(&scene, 0.0)).dest.x,
            g.video_x as f32
        );
    }

    #[test]
    fn each_ramp_caps_to_a_fraction_of_its_segment() {
        let short = scene_with(
            r#""trimEnd": 1.0,
               "segmentAnims": [{"start": 0.0, "in": {"kind": "fade", "durationMs": 2000.0}}],"#,
        );
        let ev = Evaluator::new(&short, source());
        // The ramp caps at 40% of a 1 s window, so 0.5 s in it has finished.
        assert_eq!(screen_layer(&ev.evaluate(&short, 0.5)).opacity, 1.0);
    }

    #[test]
    fn the_background_comes_through_typed_rather_than_as_a_string() {
        let scene = scene_with("");
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 0.0);
        assert_eq!(
            params.background,
            BackgroundParams::Solid(Srgba::opaque(0x0f, 0x17, 0x2a))
        );
    }

    #[test]
    fn geometry_follows_the_scene_output_spec() {
        let scene = scene_with(r#""padding": 10.0, "outputAspect": "9:16","#);
        let ev = Evaluator::new(&scene, source());
        assert_eq!(
            ev.geometry(),
            crate::geometry::canvas_geometry(1920, 1080, 10.0, Some("9:16"))
        );
    }

    #[test]
    fn a_hidden_layer_is_reported_but_marked_invisible() {
        let scene = scene_with("");
        let params = Evaluator::new(&scene, source()).evaluate(&scene, 0.0);
        let cursor = params
            .layers
            .iter()
            .find(|l| !l.visible)
            .expect("the disabled cursor layer is still present");
        assert!(!cursor.visible);
    }
}

#[cfg(test)]
mod cursor_tests {
    use super::*;
    use recast_cursor::{CursorSample, CursorTrack};
    use recast_scene::migrate::to_scene;
    use recast_scene::v1::RenderState;

    const BASE: &str = r##"{
        "trimStart": 0.0, "trimEnd": 10.0,
        "backgroundType": "color", "backgroundValue": "#0f172a", "backgroundBlur": 0.0,
        "padding": 0.0, "cursorEnabled": true, "cursorSize": 3.0, "cursorSmoothing": 0.0,
        "cursorHighlightClicks": false, "cursorHighlightColor": "#3b82f6",
        "cursorHighlightOpacity": 40.0, "cursorHideWhenIdle": false, "cursorIdleTimeout": 3.0,
        "zoomRegions": []
    }"##;

    fn source() -> SourceGeometry {
        SourceGeometry {
            width: 1000,
            height: 500,
        }
    }

    fn sample(us: u64, x: f64, y: f64) -> CursorSample {
        CursorSample {
            timestamp_us: us,
            x,
            y,
            visible: true,
            left_down: false,
            right_down: false,
        }
    }

    fn track() -> CursorTrack {
        CursorTrack::new(
            vec![
                sample(0, 0.0, 0.0),
                sample(1_000_000, 500.0, 250.0),
                sample(2_000_000, 1000.0, 500.0),
            ],
            Vec::new(),
        )
    }

    fn scene(extra: &str, with_track: bool) -> Scene {
        let mut base: serde_json::Value = serde_json::from_str(BASE).expect("base json");
        let fragment = extra.trim().trim_end_matches(',');
        if !fragment.is_empty() {
            let overrides: serde_json::Value =
                serde_json::from_str(&format!("{{{fragment}}}")).expect("override json");
            let (Some(base), Some(overrides)) = (base.as_object_mut(), overrides.as_object())
            else {
                panic!("fixtures must be JSON objects");
            };
            for (key, value) in overrides {
                base.insert(key.clone(), value.clone());
            }
        }
        let state: RenderState = serde_json::from_value(base).expect("render state");
        let mut scene = to_scene(&state);
        if with_track {
            scene.cursor_track = Some(track());
        }
        scene
    }

    fn placement(scene: &Scene, output_time: f64) -> Option<CursorPlacement> {
        Evaluator::new(scene, source())
            .evaluate(scene, output_time)
            .cursor
    }

    #[test]
    fn a_scene_with_no_track_has_no_cursor_to_place() {
        assert!(placement(&scene("", false), 0.5).is_none());
    }

    #[test]
    fn a_disabled_cursor_layer_places_nothing_even_with_a_track() {
        assert!(placement(&scene(r#""cursorEnabled": false,"#, true), 0.5).is_none());
    }

    #[test]
    fn the_placement_is_source_uv_so_it_survives_a_canvas_resize() {
        let placed = placement(&scene("", true), 1.0).expect("a placement");
        assert!((placed.x - 0.5).abs() < 1e-9, "x was {}", placed.x);
        assert!((placed.y - 0.5).abs() < 1e-9, "y was {}", placed.y);
    }

    /// The track is recorded on the ORIGINAL axis, so a cut before the playhead
    /// must shift which sample the output time lands on. Sampling at output time
    /// would leave the pointer behind the picture after every cut.
    #[test]
    fn a_cut_shifts_the_cursor_onto_the_original_axis_with_the_picture() {
        let cut = r#""cuts": [{ "start": 0.25, "end": 0.75 }],"#;
        let with_cut = placement(&scene(cut, true), 0.5).expect("a placement");
        let without = placement(&scene("", true), 0.5).expect("a placement");
        assert!((without.x - 0.25).abs() < 1e-6, "x was {}", without.x);
        assert!((with_cut.x - 0.5).abs() < 1e-6, "x was {}", with_cut.x);
    }

    /// Cursor motion easing used to be a passthrough key, so the export ignored
    /// it while the preview applied it.
    #[test]
    fn the_motion_easing_reshapes_the_path_between_two_captured_samples() {
        let eased = r#""cursorMotionEasing": { "x1": 0.9, "y1": 0.0, "x2": 1.0, "y2": 0.1 },"#;
        let linear = placement(&scene("", true), 0.5).expect("a placement");
        let curved = placement(&scene(eased, true), 0.5).expect("a placement");
        assert!(
            curved.x < linear.x - 0.05,
            "eased {} should lag linear {}",
            curved.x,
            linear.x
        );
    }
}

#[cfg(test)]
mod affine_tests {
    use super::*;

    fn close(got: (f32, f32), want: (f32, f32)) {
        assert!(
            (got.0 - want.0).abs() < 1e-5 && (got.1 - want.1).abs() < 1e-5,
            "got {got:?}, want {want:?}"
        );
    }

    #[test]
    fn inverting_the_identity_changes_nothing() {
        let inv = Affine2::IDENTITY.invert().expect("invertible");
        close(inv.apply(0.3, 0.7), (0.3, 0.7));
    }

    /// The zoom maps destination uv to the source uv it samples, so the inverse
    /// is what places a known source point (the cursor) on the canvas.
    #[test]
    fn the_inverse_round_trips_a_zoom_back_to_where_it_started() {
        let zoom = Affine2::zoom(2.5, 0.4, 0.6);
        let inv = zoom.invert().expect("invertible");
        for point in [(0.0, 0.0), (0.5, 0.5), (1.0, 1.0), (0.2, 0.9)] {
            let (u, v) = zoom.apply(point.0, point.1);
            close(inv.apply(u, v), point);
        }
    }

    /// A 2x zoom centred on the frame puts the frame centre at the canvas centre
    /// and pushes everything else outward by the scale.
    #[test]
    fn a_centred_zoom_pushes_a_source_point_outward_by_the_scale() {
        let inv = Affine2::zoom(2.0, 0.5, 0.5).invert().expect("invertible");
        close(inv.apply(0.5, 0.5), (0.5, 0.5));
        close(inv.apply(0.375, 0.5), (0.25, 0.5));
    }

    #[test]
    fn a_degenerate_transform_reports_no_inverse_rather_than_infinities() {
        let flat = Affine2 {
            sx: 0.0,
            shx: 0.0,
            tx: 0.0,
            shy: 0.0,
            sy: 0.0,
            ty: 0.0,
        };
        assert!(flat.invert().is_none());
    }
}

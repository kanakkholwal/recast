use recast_color::{Gradient, Srgba};
use recast_scene::v1::nodes::ZoomRegion;
use recast_scene::{Effect, LayerId, LayerSource, Scene};
use recast_time::{output_to_original, TimeMap};

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
        let scale = scale.max(1.0);
        let window = 1.0 / scale;
        let half = window * 0.5;
        let cx = center_x.clamp(half, 1.0 - half);
        let cy = center_y.clamp(half, 1.0 - half);
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
}

#[derive(Debug, Clone, PartialEq)]
pub enum BackgroundParams {
    Solid(Srgba),
    Gradient(Box<Gradient>),
    Asset { kind: String, value: String },
}

#[derive(Debug, Clone, PartialEq)]
pub struct LayerParams {
    pub id: LayerId,
    pub visible: bool,
    pub opacity: f32,
    pub transform: Affine2,
    /// Fraction of the shorter video edge, 0..0.5.
    pub corner_radius: f32,
    pub blur: f32,
    /// Signed rate of change of the zoom scale, in scale units per output
    /// second. Drives motion blur, which must fire during a ramp and not during
    /// the hold.
    pub zoom_velocity: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameParams {
    pub geometry: CanvasGeometry,
    pub background: BackgroundParams,
    pub background_blur: f32,
    pub layers: Vec<LayerParams>,
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
    geometry: CanvasGeometry,
}

impl Evaluator {
    pub fn new(scene: &Scene, source: SourceGeometry) -> Self {
        Self {
            time_map: scene.timeline.time_map(),
            geometry: canvas_geometry(
                source.width,
                source.height,
                scene.output.padding,
                scene.output.aspect.as_deref(),
            ),
        }
    }

    pub fn geometry(&self) -> CanvasGeometry {
        self.geometry
    }

    pub fn output_duration(&self) -> f64 {
        self.time_map.output_duration
    }

    /// `output_time` is gapless output-timeline seconds. Everything else in the
    /// scene is authored on the original recording axis, so the projection
    /// happens here, once, and every effect composes with cuts and speed for free.
    pub fn evaluate(&self, scene: &Scene, output_time: f64) -> FrameParams {
        let source_time = output_to_original(&self.time_map, output_time);

        let mut background = BackgroundParams::Solid(Srgba::opaque(0x11, 0x11, 0x11));
        let mut background_blur = 0.0f32;
        let mut layers = Vec::with_capacity(scene.layers.len());

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
                _ => layers.push(self.layer_params(layer, source_time, output_time)),
            }
        }

        FrameParams {
            geometry: self.geometry,
            background,
            background_blur,
            layers,
            source_time,
        }
    }

    fn layer_params(
        &self,
        layer: &recast_scene::Layer,
        source_time: f64,
        output_time: f64,
    ) -> LayerParams {
        let zooms: Vec<&ZoomRegion> = layer
            .effects
            .iter()
            .filter_map(|e| match e {
                Effect::Zoom(z) => Some(&**z),
                _ => None,
            })
            .collect();

        let zoom = active_zoom(&zooms, source_time);
        let transform = match zoom {
            Some(region) => Affine2::zoom(
                region.scale_at(source_time) as f32,
                region.center_x as f32,
                region.center_y as f32,
            ),
            None => Affine2::IDENTITY,
        };

        LayerParams {
            id: layer.id,
            visible: !layer.hidden,
            opacity: layer.opacity as f32,
            transform,
            corner_radius: corner_radius_of(layer),
            blur: blur_of(layer),
            zoom_velocity: self.zoom_velocity(&zooms, output_time),
        }
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

/// The latest-STARTING region containing `t` wins, ties to the later entry.
/// Matches `activeZoomIndex` in the editor: a short region nested inside a long
/// one is the more specific intent, and the rule must not depend on array order.
fn active_zoom<'a>(regions: &[&'a ZoomRegion], t: f64) -> Option<&'a ZoomRegion> {
    let mut best: Option<&ZoomRegion> = None;
    for region in regions {
        if region.hidden || t <= region.start || t >= region.end {
            continue;
        }
        match best {
            Some(current) if region.start < current.start => {}
            _ => best = Some(region),
        }
    }
    best
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

    /// FFmpeg SUMMED overlapping regions: 1.5 + 3.0 previewed as one and
    /// exported as the other. Nothing can sum here, since exactly one region
    /// is selected per instant.
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

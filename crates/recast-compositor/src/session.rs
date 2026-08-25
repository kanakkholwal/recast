use recast_cursor::CursorTrack;
use recast_gpu::{GpuContext, GpuError};
use recast_scene::{LayerId, LayerSource, Scene};
use recast_time::TimeMap;

use crate::eval::{Evaluator, FrameParams, SourceGeometry};
use crate::render::{Compositor, FrameInputs, LayerInput, RenderStats};

/// Owns the scene, its evaluator and the compositor for one preview or export
/// surface. The FFI layers marshal into this; they hold no logic of their own.
pub struct Session {
    scene: Scene,
    source: SourceGeometry,
    /// The host's resolved output axis, when it sent one. See
    /// `Evaluator::with_time_map`.
    time_map: Option<TimeMap>,
    evaluator: Evaluator,
    compositor: Compositor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutputSize {
    pub width: u32,
    pub height: u32,
}

impl Session {
    pub fn new(ctx: &GpuContext, scene: Scene, source: SourceGeometry) -> Result<Self, GpuError> {
        let evaluator = Evaluator::new(&scene, source);
        Ok(Self {
            scene,
            source,
            time_map: None,
            evaluator,
            compositor: Compositor::new(ctx)?,
        })
    }

    /// Rebuilds the evaluator, because a scene edit can move the time map and
    /// the canvas geometry. Cheap: no GPU resources are touched.
    pub fn set_scene(&mut self, mut scene: Scene) {
        // The pointer path arrives on its own channel, so a scene edit that does
        // not carry one must not drop the one already loaded.
        if scene.cursor_track.is_none() {
            scene.cursor_track = self.scene.cursor_track.take();
        }
        self.evaluator = Evaluator::with_time_map(&scene, self.source, self.time_map.clone());
        self.scene = scene;
    }

    /// The evaluator reads the track off the scene per frame, so no rebuild.
    pub fn set_cursor_track(&mut self, track: Option<CursorTrack>) {
        self.scene.cursor_track = track;
    }

    pub fn set_source(&mut self, source: SourceGeometry) {
        self.source = source;
        self.rebuild();
    }

    /// The axis the host resolved. `None` goes back to deriving it from the
    /// scene. Survives a later `set_scene`, like the cursor track.
    pub fn set_time_map(&mut self, time_map: Option<TimeMap>) {
        self.time_map = time_map;
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.evaluator = Evaluator::with_time_map(&self.scene, self.source, self.time_map.clone());
    }

    pub fn scene(&self) -> &Scene {
        &self.scene
    }

    pub fn output_size(&self) -> OutputSize {
        let g = self.evaluator.geometry();
        OutputSize {
            width: g.canvas_w,
            height: g.canvas_h,
        }
    }

    pub fn output_duration(&self) -> f64 {
        self.evaluator.output_duration()
    }

    pub fn evaluate(&self, output_time: f64) -> FrameParams {
        self.evaluator.evaluate(&self.scene, output_time)
    }

    /// The layer a decoded screen frame should be handed to. The FFI needs this
    /// because JS has no other way to name the screen layer.
    pub fn screen_layer(&self) -> Option<LayerId> {
        self.scene.screen_layer().map(|l| l.id)
    }

    pub fn camera_layer(&self) -> Option<LayerId> {
        self.scene
            .layers
            .iter()
            .find(|l| matches!(l.source, LayerSource::Camera(_)))
            .map(|l| l.id)
    }

    pub fn render(
        &mut self,
        output_time: f64,
        inputs: &FrameInputs<'_>,
        target: &wgpu::TextureView,
    ) -> RenderStats {
        let params = self.evaluator.evaluate(&self.scene, output_time);
        self.compositor.render(&params, inputs, target)
    }

    /// Convenience for the harnesses and the CLI: renders into a fresh output
    /// texture sized to the scene.
    pub fn render_to_texture(
        &mut self,
        output_time: f64,
        inputs: &FrameInputs<'_>,
    ) -> (wgpu::Texture, RenderStats) {
        let size = self.output_size();
        let target = self.compositor.output_texture(size.width, size.height);
        let stats = self.render(
            output_time,
            inputs,
            &target.create_view(&Default::default()),
        );
        (target, stats)
    }
}

/// Builds the inputs map from a single screen frame, which is the common case
/// for the preview before the camera stream is wired.
pub fn screen_only<'a>(session: &Session, input: LayerInput<'a>) -> FrameInputs<'a> {
    let mut inputs = FrameInputs::new();
    if let Some(id) = session.screen_layer() {
        inputs.set(id, input);
    }
    inputs
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

    fn scene(extra: &str) -> Scene {
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
        to_scene(&state)
    }

    fn track() -> CursorTrack {
        CursorTrack::new(
            vec![recast_cursor::CursorSample {
                timestamp_us: 0,
                x: 320.0,
                y: 180.0,
                visible: true,
                left_down: false,
                right_down: false,
            }],
            Vec::new(),
        )
    }

    fn context() -> Option<GpuContext> {
        recast_gpu::GpuContext::new_blocking(recast_gpu::GpuOptions {
            require_hardware: false,
            ..Default::default()
        })
        .ok()
    }

    fn source() -> SourceGeometry {
        SourceGeometry {
            width: 640,
            height: 360,
        }
    }

    #[test]
    fn the_output_size_follows_the_scene() {
        let Some(ctx) = context() else { return };
        let session = Session::new(&ctx, scene(r#""padding": 10.0,"#), source()).expect("session");
        assert_eq!(
            session.output_size(),
            OutputSize {
                width: 712,
                height: 432
            }
        );
    }

    /// A scene edit can move the canvas and the time map, so the evaluator has
    /// to be rebuilt. Keeping the old one is the bug this pins.
    #[test]
    fn replacing_the_scene_rebuilds_the_geometry_and_the_duration() {
        let Some(ctx) = context() else { return };
        let mut session = Session::new(&ctx, scene(""), source()).expect("session");
        let before = session.output_size();
        assert!((session.output_duration() - 10.0).abs() < 1e-6);

        session.set_scene(scene(r#""padding": 10.0, "trimEnd": 4.0,"#));
        assert_ne!(session.output_size(), before);
        assert!((session.output_duration() - 4.0).abs() < 1e-6);
    }

    /// The track is uploaded on its own channel and the editor pushes a fresh
    /// scene on any store write, so a scene replace that drops it makes the
    /// pointer vanish mid-session.
    #[test]
    fn replacing_the_scene_keeps_the_cursor_track() {
        let Some(ctx) = context() else { return };
        let mut session =
            Session::new(&ctx, scene(r#""cursorEnabled": true,"#), source()).expect("session");
        session.set_cursor_track(Some(track()));
        assert!(session.evaluate(0.0).cursor.is_some());

        session.set_scene(scene(r#""cursorEnabled": true, "padding": 5.0,"#));
        assert!(session.evaluate(0.0).cursor.is_some());
    }

    /// Clearing has to survive too, or the pointer comes back on the next edit.
    #[test]
    fn clearing_the_cursor_track_is_not_undone_by_the_next_scene() {
        let Some(ctx) = context() else { return };
        let mut session =
            Session::new(&ctx, scene(r#""cursorEnabled": true,"#), source()).expect("session");
        session.set_cursor_track(Some(track()));
        session.set_cursor_track(None);
        assert!(session.evaluate(0.0).cursor.is_none());

        session.set_scene(scene(r#""cursorEnabled": true, "padding": 5.0,"#));
        assert!(session.evaluate(0.0).cursor.is_none());
    }

    fn host_map(output_duration: f64) -> TimeMap {
        TimeMap {
            spans: vec![recast_time::MappedSpan {
                orig_start: 0.0,
                orig_end: output_duration,
                speed: 1.0,
                out_start: 0.0,
                out_end: output_duration,
            }],
            output_duration,
        }
    }

    /// The editor drops cuts its own lane flags disable, so the scene can carry
    /// a cut the host's axis does not. Two authorities for what output time
    /// means puts every effect at the wrong instant.
    #[test]
    fn the_hosts_time_map_wins_over_the_one_derived_from_the_scene() {
        let Some(ctx) = context() else { return };
        let mut session = Session::new(
            &ctx,
            scene(r#""cuts": [{"start": 2.0, "end": 4.0}],"#),
            source(),
        )
        .expect("session");
        assert!(
            (session.output_duration() - 8.0).abs() < 1e-6,
            "the fixture must cut"
        );

        session.set_time_map(Some(host_map(10.0)));
        assert!((session.output_duration() - 10.0).abs() < 1e-6);
        // Output 5 is original 5 on the host axis; the cut map would say 7.
        assert!((session.evaluate(5.0).source_time - 5.0).abs() < 1e-6);
    }

    #[test]
    fn replacing_the_scene_keeps_the_hosts_time_map() {
        let Some(ctx) = context() else { return };
        let mut session = Session::new(
            &ctx,
            scene(r#""cuts": [{"start": 2.0, "end": 4.0}],"#),
            source(),
        )
        .expect("session");
        session.set_time_map(Some(host_map(10.0)));
        session.set_scene(scene(
            r#""cuts": [{"start": 2.0, "end": 4.0}], "padding": 5.0,"#,
        ));
        assert!((session.output_duration() - 10.0).abs() < 1e-6);
    }

    #[test]
    fn clearing_the_time_map_goes_back_to_the_scene() {
        let Some(ctx) = context() else { return };
        let mut session = Session::new(
            &ctx,
            scene(r#""cuts": [{"start": 2.0, "end": 4.0}],"#),
            source(),
        )
        .expect("session");
        session.set_time_map(Some(host_map(10.0)));
        session.set_time_map(None);
        assert!((session.output_duration() - 8.0).abs() < 1e-6);
    }

    #[test]
    fn changing_the_source_size_rebuilds_the_geometry() {
        let Some(ctx) = context() else { return };
        let mut session = Session::new(&ctx, scene(""), source()).expect("session");
        session.set_source(SourceGeometry {
            width: 1920,
            height: 1080,
        });
        assert_eq!(
            session.output_size(),
            OutputSize {
                width: 1920,
                height: 1080
            }
        );
    }

    #[test]
    fn the_screen_and_camera_layers_are_addressable_by_id() {
        let Some(ctx) = context() else { return };
        let session = Session::new(&ctx, scene(""), source()).expect("session");
        let screen = session.screen_layer().expect("screen layer");
        let camera = session.camera_layer().expect("camera layer");
        assert_ne!(screen, camera);
    }

    #[test]
    fn rendering_without_a_frame_draws_the_background_and_skips_the_layer() {
        let Some(ctx) = context() else { return };
        let mut session = Session::new(&ctx, scene(""), source()).expect("session");
        let (_, stats) = session.render_to_texture(0.0, &FrameInputs::new());
        assert_eq!(stats.layers_drawn, 0);
        assert!(stats.layers_skipped > 0);
    }

    #[test]
    fn evaluating_past_the_end_clamps_rather_than_panicking() {
        let Some(ctx) = context() else { return };
        let session = Session::new(&ctx, scene(""), source()).expect("session");
        let params = session.evaluate(1_000.0);
        assert!(params.source_time <= 10.0 + 1e-6);
    }
}

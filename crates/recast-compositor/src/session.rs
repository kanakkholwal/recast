use recast_captions::TranscriptWord;
use recast_cursor::CursorTrack;
use recast_gpu::{GpuContext, GpuError};
use recast_scene::{LayerId, LayerSource, Scene};
use recast_text::{FontFace, GlyphAtlas};
use recast_time::TimeMap;

use crate::caption::{layout_caption, CaptionClock, CaptionFrame, VideoRect};
use crate::eval::{Evaluator, FrameParams, SourceGeometry};
use crate::render::{Compositor, FrameInputs, LayerInput, RenderStats};

/// Atlas width. Fixed, so growth never restrides the buffer, and a multiple of
/// 256 so the row upload is aligned on every backend.
const ATLAS_WIDTH: u32 = 1024;

/// Ceiling before the atlas refuses a glyph. 4 MB of coverage, which is far
/// more than a caption needs even at 4K.
const ATLAS_MAX_HEIGHT: u32 = 4096;

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
    /// The caption face and the glyphs packed from it. Held across frames
    /// because re-rasterising a line every frame is the whole cost.
    caption_face: Option<CaptionFace>,
    atlas: GlyphAtlas,
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
            caption_face: None,
            atlas: GlyphAtlas::new(ATLAS_WIDTH, ATLAS_MAX_HEIGHT),
        })
    }

    /// Rebuilds the evaluator, because a scene edit can move the time map and
    /// the canvas geometry. Cheap: no GPU resources are touched.
    pub fn set_scene(&mut self, mut scene: Scene) {
        // The pointer path and the transcript arrive on their own channels, so a
        // scene edit that does not carry them must not drop what is loaded.
        if scene.cursor_track.is_none() {
            scene.cursor_track = self.scene.cursor_track.take();
        }
        if scene.caption_track.is_none() {
            scene.caption_track = self.scene.caption_track.take();
        }
        self.evaluator = Evaluator::with_time_map(&scene, self.source, self.time_map.clone());
        self.scene = scene;
    }

    /// The evaluator reads the track off the scene per frame, so no rebuild.
    pub fn set_cursor_track(&mut self, track: Option<CursorTrack>) {
        self.scene.cursor_track = track;
    }

    /// The transcribed words captions are drawn from. Its own channel for the
    /// same reason as the pointer path: bulky, and it arrives separately.
    pub fn set_caption_track(&mut self, words: Option<Vec<TranscriptWord>>) {
        self.scene.caption_track = words;
    }

    /// The face to draw captions with. Required on wasm32, where there is no
    /// filesystem to resolve a family against; native falls back to resolving
    /// the style's own family.
    pub fn set_caption_font(&mut self, data: Vec<u8>, index: u32) -> bool {
        let Some(face) = FontFace::from_bytes(std::sync::Arc::new(data), index) else {
            // Bytes we cannot read leave the working face alone. On wasm32 there
            // is no resolution to fall back on, so dropping it would turn
            // captions off for the rest of the session.
            return false;
        };
        self.caption_face = Some(CaptionFace::Host(face));
        // The packed glyphs were rasterised from the old face at the same ids.
        self.atlas.reset();
        true
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

    /// Lays out the caption for `output_time` and uploads any glyph it had to
    /// rasterise. The host hands the result back through `FrameInputs`, the way
    /// it hands over sprites and annotation images.
    pub fn caption_frame(&mut self, output_time: f64) -> CaptionFrame {
        let Some(style) = self.scene.captions.clone() else {
            return CaptionFrame::default();
        };
        let Some(words) = self.scene.caption_track.clone() else {
            return CaptionFrame::default();
        };
        let Some(face) = self.caption_face_for(&style) else {
            return CaptionFrame::default();
        };

        let params = self.evaluator.evaluate(&self.scene, output_time);
        let Some(video) = screen_rect(&params) else {
            return CaptionFrame::default();
        };
        let canvas = (params.geometry.canvas_w, params.geometry.canvas_h);
        let clock = CaptionClock {
            source: params.source_time,
            output: output_time,
            time_map: self.evaluator.time_map(),
        };
        let frame = layout_caption(
            &style,
            &words,
            clock,
            video,
            canvas,
            &face,
            0,
            &mut self.atlas,
        );
        self.compositor.sync_glyph_atlas(&mut self.atlas);
        frame
    }

    /// The face for this style, resolving it the first time and again whenever
    /// the family or weight changes. Without the key a font switch in the panel
    /// would keep drawing the old face for the rest of the session.
    fn caption_face_for(&mut self, style: &recast_captions::CaptionStyle) -> Option<FontFace> {
        let key = (first_family(&style.font_family), style.font_weight);
        match &self.caption_face {
            Some(CaptionFace::Host(face)) => return Some(face.clone()),
            Some(CaptionFace::Resolved(cached, face)) if *cached == key => return face.clone(),
            _ => {}
        }
        let resolved = resolve_caption_face(style);
        self.caption_face = Some(CaptionFace::Resolved(key, resolved.clone()));
        // Same glyph ids, different outlines.
        self.atlas.reset();
        resolved
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

/// Where the caption face came from. A host-supplied face is never replaced by
/// resolution; a resolved one is keyed so a style change re-resolves, and a
/// failed resolution is remembered rather than retried every frame.
enum CaptionFace {
    Host(FontFace),
    Resolved((String, u32), Option<FontFace>),
}

/// The screen card's rect on the canvas, which is what a caption is placed
/// against.
fn screen_rect(params: &FrameParams) -> Option<VideoRect> {
    let layer = params.layers.first()?;
    Some(VideoRect {
        x: layer.dest.x as f64,
        y: layer.dest.y as f64,
        w: layer.dest.w as f64,
        h: layer.dest.h as f64,
    })
}

#[cfg(not(target_arch = "wasm32"))]
fn resolve_caption_face(style: &recast_captions::CaptionStyle) -> Option<FontFace> {
    recast_text::resolve_face(
        &first_family(&style.font_family),
        style.font_weight as u16,
        None,
    )
    .map(|resolved| resolved.face)
}

/// No filesystem to resolve against: the host must call `set_caption_font`.
#[cfg(target_arch = "wasm32")]
fn resolve_caption_face(_style: &recast_captions::CaptionStyle) -> Option<FontFace> {
    None
}

/// The first family of a CSS stack, unquoted. fontdb matches one name, not a
/// fallback list.
fn first_family(stack: &str) -> String {
    stack
        .split(',')
        .next()
        .unwrap_or(stack)
        .trim()
        .trim_matches(['\'', '"'])
        .to_string()
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
    use recast_captions::TranscriptWord;
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

    const CAPTION_STYLE: &str = r##""captionStyle": {
        "enabled": true, "fontFamily": "Arial", "fontWeight": 400,
        "fontSizePct": 6.0, "position": "bottom", "align": "center",
        "offsetPct": 0.0, "color": "#ffffff", "uppercase": false,
        "letterSpacing": 0.0, "background": "box", "backgroundColor": "#000000",
        "backgroundOpacity": 80.0, "outlineWidth": 0.0, "outlineColor": "#000000",
        "maxLines": 2,
        "animation": {
            "chunk": "word", "chunkSize": 1, "emphasis": "none",
            "emphasisColor": "#ffffff", "highlight": "none",
            "entrance": "none", "entranceMs": 0.0, "holdGaps": true
        }
    },"##;

    fn transcript() -> Vec<TranscriptWord> {
        vec![
            TranscriptWord {
                start: 1.0,
                end: 1.5,
                text: "l".into(),
            },
            TranscriptWord {
                start: 5.0,
                end: 5.5,
                text: "wwww".into(),
            },
        ]
    }

    /// Skips when no system font resolves, the same shape as the GPU skip.
    fn has_font() -> bool {
        recast_text::resolve_face("Arial", 400, None).is_some()
    }

    #[test]
    fn a_caption_needs_both_a_style_and_a_track() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let mut session = Session::new(&ctx, scene(CAPTION_STYLE), source()).expect("session");
        assert!(
            session.caption_frame(1.2).is_empty(),
            "a style with no words drew something"
        );
        session.set_caption_track(Some(transcript()));
        assert!(!session.caption_frame(1.2).is_empty());

        let mut styleless = Session::new(&ctx, scene(""), source()).expect("session");
        styleless.set_caption_track(Some(transcript()));
        assert!(styleless.caption_frame(1.2).is_empty());
    }

    /// The editor pushes a fresh scene on any store write, so a replace that
    /// drops the transcript makes captions vanish mid-session.
    #[test]
    fn replacing_the_scene_keeps_the_caption_track() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let mut session = Session::new(&ctx, scene(CAPTION_STYLE), source()).expect("session");
        session.set_caption_track(Some(transcript()));
        assert!(!session.caption_frame(1.2).is_empty());

        session.set_scene(scene(&format!(r#"{CAPTION_STYLE} "padding": 5.0,"#)));
        assert!(!session.caption_frame(1.2).is_empty());
    }

    #[test]
    fn clearing_the_caption_track_is_not_undone_by_the_next_scene() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let mut session = Session::new(&ctx, scene(CAPTION_STYLE), source()).expect("session");
        session.set_caption_track(Some(transcript()));
        session.set_caption_track(None);
        assert!(session.caption_frame(1.2).is_empty());

        session.set_scene(scene(&format!(r#"{CAPTION_STYLE} "padding": 5.0,"#)));
        assert!(session.caption_frame(1.2).is_empty());
    }

    /// Words carry ORIGINAL times, so a cut has to move which one is on screen
    /// at a given OUTPUT time. Resolving on the output axis would show the
    /// wrong word for the whole tail of the video.
    #[test]
    fn a_caption_resolves_on_the_original_axis_not_the_output_one() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let cut = format!(r#"{CAPTION_STYLE} "cuts": [{{"start": 2.0, "end": 4.0}}],"#);
        let mut session = Session::new(&ctx, scene(&cut), source()).expect("session");
        session.set_caption_track(Some(transcript()));

        // Output 1.2 is original 1.2: the one-glyph word.
        let early = session.caption_frame(1.2);
        // Output 3.2 is original 5.2, past the cut: the four-glyph word.
        let late = session.caption_frame(3.2);
        assert_eq!(early.glyphs.len(), 1, "expected the short word before the cut");
        assert_eq!(late.glyphs.len(), 4, "expected the long word after the cut");
    }

    #[test]
    fn a_new_face_drops_the_glyphs_packed_from_the_old_one() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let mut session = Session::new(&ctx, scene(CAPTION_STYLE), source()).expect("session");
        session.set_caption_track(Some(transcript()));
        let before = session.caption_frame(1.2);
        assert!(!before.is_empty());

        let face = recast_text::resolve_face("Arial", 400, None).expect("arial");
        let bytes = face.face.data().to_vec();
        assert!(session.set_caption_font(bytes, 0));
        let after = session.caption_frame(1.2);
        assert_eq!(after.glyphs.len(), before.glyphs.len());
        // Bytes we cannot read are refused. That they leave the working face in
        // place only shows on wasm32, where nothing re-resolves behind it.
        assert!(!session.set_caption_font(vec![0, 1, 2, 3], 0));
    }

    /// A host-supplied face outranks the style's family: on wasm32 there is
    /// nothing to resolve with, so a style edit must not replace it.
    #[test]
    fn a_style_change_does_not_replace_a_host_supplied_face() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let mut session = Session::new(&ctx, scene(CAPTION_STYLE), source()).expect("session");
        session.set_caption_track(Some(transcript()));
        let bytes = recast_text::resolve_face("Arial", 400, None)
            .expect("arial")
            .face
            .data()
            .to_vec();
        assert!(session.set_caption_font(bytes, 0));

        let missing = CAPTION_STYLE.replace(r#""fontFamily": "Arial""#, r#""fontFamily": "NoSuchFamilyAnywhere""#);
        session.set_scene(scene(&missing));
        assert!(
            !session.caption_frame(1.2).is_empty(),
            "the host face was dropped for an unresolvable family"
        );
    }

    /// The face is cached, so it has to be keyed on the family: without that a
    /// font switch in the panel keeps drawing the old one all session.
    #[test]
    fn a_family_that_cannot_be_resolved_stops_drawing() {
        let Some(ctx) = context() else { return };
        if !has_font() {
            return;
        }
        let mut session = Session::new(&ctx, scene(CAPTION_STYLE), source()).expect("session");
        session.set_caption_track(Some(transcript()));
        assert!(!session.caption_frame(1.2).is_empty());

        let missing = CAPTION_STYLE.replace(r#""fontFamily": "Arial""#, r#""fontFamily": "NoSuchFamilyAnywhere""#);
        session.set_scene(scene(&missing));
        assert!(
            session.caption_frame(1.2).is_empty(),
            "the old face was reused for a family that does not resolve"
        );
    }

    #[test]
    fn evaluating_past_the_end_clamps_rather_than_panicking() {
        let Some(ctx) = context() else { return };
        let session = Session::new(&ctx, scene(""), source()).expect("session");
        let params = session.evaluate(1_000.0);
        assert!(params.source_time <= 10.0 + 1e-6);
    }
}

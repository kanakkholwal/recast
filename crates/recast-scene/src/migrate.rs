use recast_color::{parse_css_color, parse_gradient, serialize_gradient, Srgba};

use crate::scene::{
    AudioGraph, CursorSpec, Effect, Layer, LayerSource, OutputSpec, Scene, SceneFlags, Timeline,
    TimelineCut, SCHEMA_VERSION,
};
use crate::v1::{CutRange, RenderState};

const BACKGROUND_LAYER: u32 = 0;
const SCREEN_LAYER: u32 = 1;
const CAMERA_LAYER: u32 = 2;
const CURSOR_LAYER: u32 = 3;
const FIRST_ANNOTATION_LAYER: u32 = 4;

impl From<&RenderState> for Scene {
    fn from(state: &RenderState) -> Self {
        to_scene(state)
    }
}

impl From<&Scene> for RenderState {
    fn from(scene: &Scene) -> Self {
        to_render_state(scene)
    }
}

pub fn to_scene(state: &RenderState) -> Scene {
    let mut layers = vec![
        Layer::new(BACKGROUND_LAYER, background_source(state))
            .with_effects(background_effects(state)),
        Layer::new(SCREEN_LAYER, LayerSource::Screen).with_effects(screen_effects(state)),
        Layer::new(
            CAMERA_LAYER,
            LayerSource::Camera(Box::new(state.camera_overlay.clone())),
        ),
        Layer::new(
            CURSOR_LAYER,
            LayerSource::Cursor(Box::new(cursor_spec(state))),
        ),
    ];
    layers[CAMERA_LAYER as usize].hidden = !state.camera_overlay.enabled;
    layers[CURSOR_LAYER as usize].hidden = !state.cursor_enabled;

    for (index, annotation) in state.annotations.iter().enumerate() {
        let mut layer = Layer::new(
            FIRST_ANNOTATION_LAYER + index as u32,
            LayerSource::Annotation(Box::new(annotation.clone())),
        );
        layer.hidden = annotation.hidden;
        layer.opacity = annotation.opacity;
        layers.push(layer);
    }

    Scene {
        schema: SCHEMA_VERSION,
        // The v1 state never carried the pointer path: it lives in its own file and the editor attaches it after migrating.
        cursor_track: None,
        captions: state.caption_style.clone(),
        // The words live in their own file, like the pointer path, and the editor attaches them after migrating.
        caption_track: None,
        flags: SceneFlags {
            focus: state.focus_enabled,
            annotations: state.annotations_enabled,
        },
        output: OutputSpec {
            aspect: state.output_aspect.clone(),
            padding: state.padding,
        },
        timeline: Timeline {
            trim_start: state.trim_start,
            trim_end: state.trim_end,
            cuts: state
                .cuts
                .iter()
                .map(|c| TimelineCut {
                    start: c.start,
                    end: c.end,
                    extra: c.extra.clone(),
                })
                .collect(),
            split_points: state.split_points.clone(),
            segment_speeds: state.segment_speeds.clone(),
        },
        layers,
        audio: AudioGraph {
            settings: state.audio_settings.clone(),
            clips: state.music_clips.clone(),
        },
        passthrough: state.passthrough.clone(),
    }
}

fn background_source(state: &RenderState) -> LayerSource {
    match state.background_type.as_str() {
        "color" => match parse_css_color(&state.background_value) {
            Some(color) => LayerSource::Solid { color },
            None => LayerSource::Asset {
                kind: state.background_type.clone(),
                value: state.background_value.clone(),
            },
        },
        "gradient" => LayerSource::Gradient {
            gradient: parse_gradient(&state.background_value),
        },
        _ => LayerSource::Asset {
            kind: state.background_type.clone(),
            value: state.background_value.clone(),
        },
    }
}

fn background_effects(state: &RenderState) -> Vec<Effect> {
    if state.background_blur == 0.0 {
        Vec::new()
    } else {
        vec![Effect::Blur {
            amount: state.background_blur,
        }]
    }
}

fn screen_effects(state: &RenderState) -> Vec<Effect> {
    let mut effects: Vec<Effect> = state
        .zoom_regions
        .iter()
        .map(|z| Effect::Zoom(Box::new(z.clone())))
        .collect();
    effects.extend(
        state
            .scene_animations
            .iter()
            .map(|a| Effect::SceneAnim(Box::new(a.clone()))),
    );
    if state.border_radius != 0.0 {
        effects.push(Effect::CornerRadius {
            percent: state.border_radius,
        });
    }
    effects.push(Effect::DropShadow(Box::new(state.shadow.clone())));
    effects
}

fn cursor_spec(state: &RenderState) -> CursorSpec {
    CursorSpec {
        size: state.cursor_size,
        smoothing: state.cursor_smoothing,
        snap_to_clicks: state.cursor_snap_to_clicks,
        motion_easing: state.cursor_motion_easing,
        snap_window_ms: state.cursor_snap_window_ms,
        highlight_clicks: state.cursor_highlight_clicks,
        highlight_color: state.cursor_highlight_color.clone(),
        highlight_opacity: state.cursor_highlight_opacity,
        hide_when_idle: state.cursor_hide_when_idle,
        idle_timeout: state.cursor_idle_timeout,
        motion_blur: state.cursor_motion_blur,
        click_bounce: state.cursor_click_bounce,
        bounce_speed_ms: state.cursor_bounce_speed_ms,
        sway: state.cursor_sway,
        sprite_rest: state.cursor_sprite_rest.clone(),
        sprite_press: state.cursor_sprite_press.clone(),
        sprite_right_press: state.cursor_sprite_right_press.clone(),
        sprite_drag: state.cursor_sprite_drag.clone(),
        sprite_hotspot_rest: state.cursor_sprite_hotspot_rest,
        sprite_hotspot_press: state.cursor_sprite_hotspot_press,
        sprite_hotspot_right_press: state.cursor_sprite_hotspot_right_press,
        sprite_hotspot_drag: state.cursor_sprite_hotspot_drag,
        sprite_size_px: state.cursor_sprite_size_px,
    }
}

pub fn to_render_state(scene: &Scene) -> RenderState {
    let mut state = RenderState {
        trim_start: scene.timeline.trim_start,
        trim_end: scene.timeline.trim_end,
        padding: scene.output.padding,
        output_aspect: scene.output.aspect.clone(),
        cuts: scene
            .timeline
            .cuts
            .iter()
            .map(|c| CutRange {
                start: c.start,
                end: c.end,
                extra: c.extra.clone(),
            })
            .collect(),
        focus_enabled: scene.flags.focus,
        annotations_enabled: scene.flags.annotations,
        caption_style: scene.captions.clone(),
        split_points: scene.timeline.split_points.clone(),
        segment_speeds: scene.timeline.segment_speeds.clone(),
        audio_settings: scene.audio.settings.clone(),
        music_clips: scene.audio.clips.clone(),
        passthrough: scene.passthrough.clone(),
        ..RenderState::default()
    };

    for layer in &scene.layers {
        match &layer.source {
            LayerSource::Solid { color } => {
                state.background_type = "color".into();
                state.background_value = hex_or_original(*color);
                state.background_blur = blur_amount(layer);
            }
            LayerSource::Gradient { gradient } => {
                state.background_type = "gradient".into();
                state.background_value = serialize_gradient(gradient);
                state.background_blur = blur_amount(layer);
            }
            LayerSource::Asset { kind, value } => {
                state.background_type = kind.clone();
                state.background_value = value.clone();
                state.background_blur = blur_amount(layer);
            }
            LayerSource::Screen => apply_screen_effects(&mut state, layer),
            LayerSource::Camera(camera) => {
                state.camera_overlay = (**camera).clone();
            }
            LayerSource::Cursor(cursor) => {
                state.cursor_enabled = !layer.hidden;
                apply_cursor(&mut state, cursor);
            }
            LayerSource::Annotation(annotation) => {
                state.annotations.push((**annotation).clone());
            }
        }
    }
    state
}

/// A colour that did not survive parsing is stored as an `Asset` layer, so
/// anything reaching here round-trips through the canonical hex spelling.
fn hex_or_original(color: Srgba) -> String {
    color.to_hex()
}

fn blur_amount(layer: &Layer) -> f64 {
    layer
        .effects
        .iter()
        .find_map(|e| match e {
            Effect::Blur { amount } => Some(*amount),
            _ => None,
        })
        .unwrap_or(0.0)
}

fn apply_screen_effects(state: &mut RenderState, layer: &Layer) {
    for effect in &layer.effects {
        match effect {
            Effect::Zoom(zoom) => state.zoom_regions.push((**zoom).clone()),
            Effect::SceneAnim(anim) => state.scene_animations.push((**anim).clone()),
            Effect::CornerRadius { percent } => state.border_radius = *percent,
            Effect::DropShadow(shadow) => state.shadow = (**shadow).clone(),
            Effect::Blur { .. } => {}
        }
    }
}

fn apply_cursor(state: &mut RenderState, cursor: &CursorSpec) {
    state.cursor_size = cursor.size;
    state.cursor_smoothing = cursor.smoothing;
    state.cursor_snap_to_clicks = cursor.snap_to_clicks;
    state.cursor_motion_easing = cursor.motion_easing;
    state.cursor_snap_window_ms = cursor.snap_window_ms;
    state.cursor_highlight_clicks = cursor.highlight_clicks;
    state.cursor_highlight_color = cursor.highlight_color.clone();
    state.cursor_highlight_opacity = cursor.highlight_opacity;
    state.cursor_hide_when_idle = cursor.hide_when_idle;
    state.cursor_idle_timeout = cursor.idle_timeout;
    state.cursor_motion_blur = cursor.motion_blur;
    state.cursor_click_bounce = cursor.click_bounce;
    state.cursor_bounce_speed_ms = cursor.bounce_speed_ms;
    state.cursor_sway = cursor.sway;
    state.cursor_sprite_rest = cursor.sprite_rest.clone();
    state.cursor_sprite_press = cursor.sprite_press.clone();
    state.cursor_sprite_right_press = cursor.sprite_right_press.clone();
    state.cursor_sprite_drag = cursor.sprite_drag.clone();
    state.cursor_sprite_hotspot_rest = cursor.sprite_hotspot_rest;
    state.cursor_sprite_hotspot_press = cursor.sprite_hotspot_press;
    state.cursor_sprite_hotspot_right_press = cursor.sprite_hotspot_right_press;
    state.cursor_sprite_hotspot_drag = cursor.sprite_hotspot_drag;
    state.cursor_sprite_size_px = cursor.sprite_size_px;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::v1::nodes::ZoomRegion;

    fn state_json(extra: serde_json::Value) -> RenderState {
        let mut base = serde_json::json!({
            "trimStart": 1.0,
            "trimEnd": 9.0,
            "backgroundType": "color",
            "backgroundValue": "#0f172a",
            "backgroundBlur": 0.0,
            "padding": 6.0,
            "cursorEnabled": true,
            "cursorSize": 3.0,
            "cursorSmoothing": 50.0,
            "cursorHighlightClicks": true,
            "cursorHighlightColor": "#3b82f6",
            "cursorHighlightOpacity": 40.0,
            "cursorHideWhenIdle": false,
            "cursorIdleTimeout": 3.0,
            "zoomRegions": []
        });
        if let (Some(base), Some(extra)) = (base.as_object_mut(), extra.as_object()) {
            for (k, v) in extra {
                base.insert(k.clone(), v.clone());
            }
        }
        serde_json::from_value(base).expect("fixture render state")
    }

    fn round_trip(state: &RenderState) -> RenderState {
        to_render_state(&to_scene(state))
    }

    fn assert_round_trips(state: &RenderState) {
        let before = serde_json::to_value(state).expect("serialize before");
        let after = serde_json::to_value(round_trip(state)).expect("serialize after");
        assert_eq!(before, after);
    }

    #[test]
    fn a_minimal_project_round_trips() {
        assert_round_trips(&state_json(serde_json::json!({})));
    }

    #[test]
    fn a_gradient_background_round_trips() {
        assert_round_trips(&state_json(serde_json::json!({
            "backgroundType": "gradient",
            "backgroundValue": "linear-gradient(135deg, #6366f1 0%, #d946ef 100%)",
            "backgroundBlur": 12.0
        })));
    }

    #[test]
    fn a_wallpaper_background_keeps_its_tag_and_path() {
        let state = state_json(serde_json::json!({
            "backgroundType": "wallpaper",
            "backgroundValue": "C:/wallpapers/ridge.jpg"
        }));
        let scene = to_scene(&state);
        assert!(matches!(scene.layers[0].source, LayerSource::Asset { .. }));
        assert_round_trips(&state);
    }

    #[test]
    fn an_unparseable_colour_falls_back_to_an_asset_layer_rather_than_being_lost() {
        let state = state_json(serde_json::json!({
            "backgroundType": "color",
            "backgroundValue": "var(--brand)"
        }));
        assert_round_trips(&state);
    }

    #[test]
    fn zoom_regions_become_ordered_effects_on_the_screen_layer() {
        let state = state_json(serde_json::json!({
            "zoomRegions": [
                { "start": 1.0, "end": 3.0, "scale": 1.8, "id": "z1" },
                { "start": 4.0, "end": 6.0, "scale": 2.2, "id": "z2" }
            ]
        }));
        let scene = to_scene(&state);
        let regions: Vec<&ZoomRegion> = scene.zoom_regions();
        assert_eq!(regions.len(), 2);
        assert_eq!(regions[0].scale, 1.8);
        assert_round_trips(&state);
    }

    #[test]
    fn a_zoom_regions_unknown_editor_keys_survive_the_round_trip() {
        let state = state_json(serde_json::json!({
            "zoomRegions": [
                { "start": 1.0, "end": 3.0, "scale": 1.8, "id": "z1", "source": "auto" }
            ]
        }));
        let after = round_trip(&state);
        assert_eq!(
            after.zoom_regions[0].extra.get("source"),
            Some(&serde_json::Value::String("auto".into()))
        );
    }

    #[test]
    fn cuts_keep_their_editor_identity() {
        let state = state_json(serde_json::json!({
            "cuts": [{ "start": 2.0, "end": 3.0, "id": "c1", "source": "silence" }],
            "splitPoints": [5.0],
            "segmentSpeeds": [{ "start": 5.0, "speed": 2.0 }]
        }));
        let after = round_trip(&state);
        assert_eq!(
            after.cuts[0].extra.get("id"),
            Some(&serde_json::Value::String("c1".into()))
        );
        assert_round_trips(&state);
    }

    #[test]
    fn annotations_become_their_own_layers_in_order() {
        let state = state_json(serde_json::json!({
            "annotations": [
                { "id": "a1", "start": 0.0, "end": 2.0, "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2 } },
                { "id": "a2", "start": 1.0, "end": 3.0, "kind": { "kind": "ellipse", "x": 0.3, "y": 0.3, "w": 0.2, "h": 0.2 } }
            ]
        }));
        let scene = to_scene(&state);
        assert_eq!(scene.annotations().len(), 2);
        assert_eq!(scene.annotations()[0].id, "a1");
        assert_round_trips(&state);
    }

    #[test]
    fn a_hidden_annotation_marks_its_layer_hidden() {
        let state = state_json(serde_json::json!({
            "annotations": [
                { "id": "a1", "start": 0.0, "end": 2.0, "hidden": true,
                  "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2 } }
            ]
        }));
        let scene = to_scene(&state);
        assert!(scene.layers.last().map(|l| l.hidden).unwrap_or(false));
        assert_round_trips(&state);
    }

    #[test]
    fn a_disabled_cursor_marks_its_layer_hidden_and_comes_back_disabled() {
        let state = state_json(serde_json::json!({ "cursorEnabled": false }));
        let scene = to_scene(&state);
        let cursor = scene
            .layers
            .iter()
            .find(|l| matches!(l.source, LayerSource::Cursor(_)))
            .expect("cursor layer");
        assert!(cursor.hidden);
        assert!(!round_trip(&state).cursor_enabled);
    }

    #[test]
    fn editor_only_keys_survive_the_round_trip() {
        let state = state_json(serde_json::json!({
            "cursorStyle": "macos",
            "layoutMode": "fill",
            "autoZoomEnabled": true
        }));
        let after = round_trip(&state);
        assert_eq!(
            after.passthrough.get("cursorStyle"),
            Some(&serde_json::Value::String("macos".into()))
        );
        assert_round_trips(&state);
    }

    #[test]
    fn the_scene_carries_the_resolved_time_map() {
        let state = state_json(serde_json::json!({
            "trimStart": 0.0,
            "trimEnd": 10.0,
            "splitPoints": [5.0],
            "segmentSpeeds": [{ "start": 5.0, "speed": 2.0 }]
        }));
        let scene = to_scene(&state);
        assert!((scene.timeline.output_duration() - 7.5).abs() < 1e-6);
    }

    #[test]
    fn the_scene_serialises_and_deserialises_byte_stably() {
        let state = state_json(serde_json::json!({
            "backgroundType": "gradient",
            "backgroundValue": "linear-gradient(45deg, #ff0000 0%, #0000ff 100%)",
            "zoomRegions": [{ "start": 1.0, "end": 3.0, "scale": 1.8 }],
            "annotations": [
                { "id": "a1", "start": 0.0, "end": 2.0, "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2 } }
            ]
        }));
        let scene = to_scene(&state);
        let once = serde_json::to_string(&scene).expect("serialize");
        let decoded: Scene = serde_json::from_str(&once).expect("deserialize");
        let twice = serde_json::to_string(&decoded).expect("re-serialize");
        assert_eq!(once, twice);
        assert_eq!(scene, decoded);
    }

    fn fully_populated() -> RenderState {
        let extra: serde_json::Value = serde_json::from_str(FULLY_POPULATED).expect("fixture json");
        state_json(extra)
    }

    const FULLY_POPULATED: &str = r##"{
        "focusEnabled": false,
        "annotationsEnabled": false,
        "captionStyle": {
            "enabled": true, "fontFamily": "Anton", "fontWeight": 800,
            "fontSizePct": 5.5, "position": "top", "align": "left",
            "offsetPct": -4.0, "color": "#00ff00", "mutedColor": "#334455",
            "uppercase": true, "letterSpacing": 0.05, "background": "soft",
            "backgroundColor": "#123456", "backgroundOpacity": 42.0,
            "boxPaddingXEm": 0.9, "boxPaddingYEm": 0.4, "boxRadiusEm": 1.2,
            "lineHeight": 1.5, "outlineWidth": 3.0, "outlineColor": "#654321",
            "maxLines": 3, "maxCharsPerLine": 30,
            "animation": {
                "chunk": "word", "chunkSize": 2, "emphasis": "scale",
                "emphasisColor": "#ff00ff", "highlight": "progressive",
                "entrance": "pop", "entranceMs": 90.0, "holdGaps": false
            }
        },
        "backgroundType": "gradient",
        "backgroundValue": "linear-gradient(45deg, #ff0000 0%, #0000ff 100%)",
        "backgroundBlur": 18.0,
        "borderRadius": 12.5,
        "outputAspect": "9:16",
        "cursorEnabled": false,
        "cursorSize": 5.5,
        "cursorSmoothing": 75.0,
        "cursorHighlightClicks": false,
        "cursorHighlightColor": "#f59e0b",
        "cursorHighlightOpacity": 65.0,
        "cursorHideWhenIdle": true,
        "cursorIdleTimeout": 1.5,
        "cursorSnapToClicks": false,
        "cursorSnapWindowMs": 120.0,
        "cursorMotionBlur": 0.4,
        "cursorClickBounce": 1.5,
        "cursorBounceSpeedMs": 300.0,
        "cursorSway": 0.2,
        "cursorSpriteRest": "data:image/png;base64,AAA",
        "cursorSpritePress": "data:image/png;base64,BBB",
        "cursorSpriteRightPress": "data:image/png;base64,CCC",
        "cursorSpriteDrag": "data:image/png;base64,DDD",
        "cursorSpriteHotspotRest": [0.1, 0.2],
        "cursorSpriteHotspotPress": [0.3, 0.4],
        "cursorSpriteHotspotRightPress": [0.5, 0.6],
        "cursorSpriteHotspotDrag": [0.7, 0.8],
        "cursorSpriteSizePx": 48.0,
        "cursorMotionEasing": { "x1": 0.2, "y1": 0.1, "x2": 0.8, "y2": 0.9 },
        "zoomRegions": [{ "start": 1.0, "end": 3.0, "scale": 1.8, "id": "z1", "source": "auto" }],
        "cuts": [{ "start": 4.0, "end": 4.5, "id": "c1", "source": "silence" }],
        "splitPoints": [6.0],
        "segmentSpeeds": [{ "start": 6.0, "speed": 2.0 }],
        "segmentAnims": [{ "start": 6.0, "in": { "kind": "slide", "durationMs": 400.0 } }],
        "annotations": [
            { "id": "a1", "start": 0.0, "end": 2.0, "zIndex": 3, "locked": true,
              "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2, "radius": 0.05 } }
        ],
        "shadow": { "enabled": true, "blur": 30.0, "spread": 4.0, "offsetY": 18.0, "opacity": 55.0, "color": "#101010" },
        "audioSettings": { "volume": 0.8, "muted": false, "systemVolume": 0.6, "systemMuted": true },
        "musicClips": [
            { "id": "m1", "source": { "kind": "local", "path": "C:/music/a.mp3" },
              "startOutputSec": 1.0, "durationSec": 8.0, "gain": 0.7 }
        ],
        "cameraOverlay": { "enabled": true, "mirror": false, "shape": "circle", "cornerRadius": 0.3 }
    }"##;

    /// The round trip is only an oracle if a fixture actually sets the field.
    /// This one populates every branch the migration touches, so dropping any
    /// single field on the way back fails here.
    #[test]
    fn every_field_the_migration_touches_round_trips() {
        assert_round_trips(&fully_populated());
    }

    #[test]
    fn the_fixture_leaves_no_render_state_key_at_its_default() {
        let populated = serde_json::to_value(fully_populated()).expect("serialize populated");
        let default = serde_json::to_value(RenderState::default()).expect("serialize default");
        let (Some(populated), Some(default)) = (populated.as_object(), default.as_object()) else {
            panic!("render state is not a JSON object");
        };
        let untouched: Vec<&String> = default
            .keys()
            .filter(|k| populated.get(*k) == default.get(*k))
            .collect();
        assert!(
            untouched.is_empty(),
            "these keys are still at their default, so the round-trip test cannot see them: {untouched:?}"
        );

        // A skipped-when-None field is in NEITHER object, so the comparison can't see it; the count is the tripwire.
        assert_eq!(
            populated.len(),
            RENDER_STATE_KEYS,
            "give every new RenderState field a non-default value in FULLY_POPULATED, then bump this"
        );
    }

    /// Keys `fully_populated()` must emit. Bumped deliberately, never to make a
    /// failing test pass.
    const RENDER_STATE_KEYS: usize = 45;

    #[test]
    fn layer_ids_are_unique_and_next_id_does_not_collide() {
        let state = state_json(serde_json::json!({
            "annotations": [
                { "id": "a1", "start": 0.0, "end": 2.0, "kind": { "kind": "rect", "x": 0.1, "y": 0.1, "w": 0.2, "h": 0.2 } }
            ]
        }));
        let scene = to_scene(&state);
        let mut ids: Vec<u32> = scene.layers.iter().map(|l| l.id.0).collect();
        let count = ids.len();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), count);
        assert!(!ids.contains(&scene.next_layer_id().0));
    }
}

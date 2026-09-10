use recast_scene::v1::nodes::{
    Annotation, AnnotationGlow, AnnotationKind, AudioClip, AudioClipSource, CameraClipLayout,
    CameraKeyframe, CameraLayout, CameraPlacement, LayoutSide, ZoomRegion,
};
use recast_scene::v1::{CutRange, RenderState, SceneAnimSpec, SegmentAnim, SegmentSpeed};
use serde_json::json;

use super::read::to_render_state;
use super::write::{default_annotation, from_render_state, MediaFile, MediaRefs, TrackFile};
use crate::ids::IdGen;
use crate::parse::parse;
use crate::serialize::serialize;
use crate::validate::validate;

fn media() -> MediaRefs {
    MediaRefs {
        recording: Some(MediaFile {
            src: "media/recording.mp4".into(),
            width: 1920,
            height: 1080,
            fps: 60.0,
            duration: 92.4,
            offset: 0.0,
        }),
        camera: Some(MediaFile {
            src: "media/camera.mp4".into(),
            offset: 0.856,
            ..Default::default()
        }),
        mic: Some("media/mic.wav".into()),
        system: Some("media/system.wav".into()),
        cursor: Some(TrackFile {
            src: "tracks/cursor.json".into(),
            rows: 2614,
            span: Some((0.0, 92.4)),
            ..Default::default()
        }),
        words: Some(TrackFile {
            src: "tracks/words.json".into(),
            rows: 731,
            span: Some((0.52, 91.9)),
            engine: Some("parakeet".into()),
            ..Default::default()
        }),
    }
}

fn annotation(id: &str, kind: AnnotationKind) -> Annotation {
    Annotation {
        id: id.into(),
        start: 16.8,
        end: 20.8,
        kind,
        glow: Some(AnnotationGlow {
            color: "#ff5c5c".into(),
            blur: 8.0,
            opacity: 0.5,
        }),
        opacity: 0.9,
        z_index: 2,
        fill: "transparent".into(),
        ..default_annotation()
    }
}

fn rich_state() -> RenderState {
    let mut state = RenderState {
        trim_start: 0.5,
        trim_end: 92.4,
        background_type: "gradient".into(),
        background_value: "linear-gradient(135deg, #ff5c5c 0%, #3b82f6 100%)".into(),
        background_blur: 12.0,
        padding: 40.0,
        output_aspect: Some("16:9".into()),
        border_radius: 1.2,
        cursor_smoothing: 65.0,
        cursor_highlight_opacity: 40.0,
        cursor_hide_when_idle: true,
        cursor_motion_easing: Some(recast_scene::v1::Easing {
            x1: 0.4,
            y1: 0.0,
            x2: 0.2,
            y2: 1.0,
        }),
        split_points: vec![30.0],
        segment_speeds: vec![SegmentSpeed {
            start: 30.0,
            speed: 1.5,
        }],
        scene_animations: vec![SegmentAnim {
            start: 30.0,
            anim_in: Some(SceneAnimSpec {
                kind: "slide".into(),
                duration_ms: 400.0,
                easing: Default::default(),
                dir: Some("left".into()),
                intensity: Some(0.5),
            }),
            anim_out: None,
        }],
        ..RenderState::default()
    };
    state.cuts.push(CutRange {
        start: 12.3,
        end: 14.8,
        extra: [
            ("id".to_owned(), json!("c1")),
            ("source".to_owned(), json!("silence")),
        ]
        .into_iter()
        .collect(),
    });
    state.zoom_regions.push(ZoomRegion {
        start: 4.5,
        end: 10.5,
        scale: 1.8,
        ease_in: Default::default(),
        ease_out: Default::default(),
        ramp_in: 0.6,
        ramp_out: 0.6,
        center_x: 0.32,
        center_y: 0.28,
        hidden: false,
        motion_blur: 0.0,
        extra: [
            ("id".to_owned(), json!("z1")),
            ("source".to_owned(), json!("manual")),
        ]
        .into_iter()
        .collect(),
    });
    state.shadow.enabled = true;
    state.shadow.opacity = 35.0;
    state.camera_overlay.enabled = true;
    state.camera_overlay.cursor_dodge = true;
    state.camera_overlay.keyframes.push(CameraKeyframe {
        at_sec: 28.4,
        placement: CameraPlacement {
            x: 0.04,
            y: 0.72,
            width: 0.22,
            height: 0.3,
        },
    });
    state.camera_overlay.clip_layouts.push(CameraClipLayout {
        start: 30.0,
        layout: CameraLayout::SplitH {
            fraction: 0.4,
            side: LayoutSide::End,
        },
    });
    rich_extras(&mut state);
    state
}

fn rich_extras(state: &mut RenderState) {
    state.annotations.push(annotation(
        "a1",
        AnnotationKind::Arrow {
            x1: 0.1,
            y1: 0.2,
            x2: 0.5,
            y2: 0.6,
            head_size: 0.2,
        },
    ));
    state.annotations.push(annotation(
        "t1",
        AnnotationKind::Text {
            x: 0.1,
            y: 0.1,
            w: 0.4,
            h: 0.1,
            content: "Hello & <world>".into(),
            font_family: "Inter".into(),
            font_size: 42.0,
            font_weight: 600.0,
            color: "#ffffff".into(),
            align: "center".into(),
            line_height: 1.3,
        },
    ));
    state.caption_style = Some(recast_captions::CaptionStyle {
        background_opacity: 76.0,
        ..Default::default()
    });
    state.audio_settings.mic_volume = 80.0;
    state.audio_settings.normalize_loudness = true;
    state.music_clips.push(AudioClip {
        id: "m1".into(),
        source: AudioClipSource::Provider {
            provider_id: "pixabay".into(),
            track_id: "42".into(),
            asset_path: "C:/cache/42.mp3".into(),
            attribution: Some("by X".into()),
            license: None,
        },
        role: Default::default(),
        start_output_sec: 2.0,
        offset_sec: 0.5,
        duration_sec: 30.0,
        gain: 45.0,
        muted: false,
        fade_in: 1.0,
        fade_out: 2.0,
        looping: true,
        ducking: true,
    });
    state
        .passthrough
        .insert("cursorStyle".into(), json!("macos"));
    state.passthrough.insert("cutsEnabled".into(), json!(false));
    state
        .passthrough
        .insert("autoZoomEnabled".into(), json!(true));
    state.passthrough.insert("layoutMode".into(), json!("crop"));
    state.passthrough.insert(
        "dismissedSilences".into(),
        json!([{ "start": 50.0, "end": 51.5 }]),
    );
    state
        .passthrough
        .insert("someFutureToggle".into(), json!({ "a": 1 }));
}

#[test]
fn a_generic_bind_reads_into_the_state_survives_a_save_that_does_not_name_it_and_is_validated() {
    use recast_scene::bind::{LayerRef, Map, Signal};
    let text = r##"<recast v="3" timebase="source-seconds">
  <timeline in="0" out="10"/>
  <screen id="scr"><bind id="b1" prop="opacity" src="time" map="wave" from="0.2" to="1" period="2"/></screen>
  <camera id="bubble" enabled="true"><bind id="keys" prop="x y w h" src="time" map="keys"/></camera>
  <annotations>
    <rect id="a1" at="1" dur="2" x="0.1" y="0.1" w="0.2" h="0.2"><bind id="b2" prop="opacity" src="cursor.x" map="clamp" from="0" to="1" in="0.500" out="1.500"/></rect>
  </annotations>
</recast>
"##;
    let doc = parse(text).unwrap();
    let report = validate(&doc);
    assert!(report.is_ok(), "{:?}", report.issues);
    let state = to_render_state(&doc).unwrap();
    assert_eq!(
        state.bindings.len(),
        2,
        "the camera's keys stack is settings, not a binding"
    );
    assert_eq!(state.bindings[0].layer, LayerRef::Screen);
    assert_eq!(
        state.bindings[0].binding.map,
        Map::Wave {
            from: 0.2,
            to: 1.0,
            period: 2.0
        }
    );
    assert_eq!(
        state.bindings[1].layer,
        LayerRef::Annotation { id: "a1".into() }
    );
    assert_eq!(state.bindings[1].binding.signal, Signal::CursorX);
    assert_eq!(state.bindings[1].binding.window, Some((0.5, 1.5)));
    let scene = super::read::to_scene(&doc).unwrap();
    assert_eq!(
        scene.layers[1].bindings.len(),
        1,
        "the screen layer carries its binding"
    );

    // The editor's whole-state save never names bindings; the base document keeps them.
    let mut saved = state.clone();
    saved.bindings.clear();
    let mut ids = IdGen::seeded(3);
    let rewritten = from_render_state(&saved, &MediaRefs::default(), Some(&doc), &mut ids);
    let again = to_render_state(&rewritten).unwrap();
    assert_eq!(again.bindings, state.bindings, "{}", serialize(&rewritten));

    let bad = parse(
        &text.replace("src=\"cursor.x\"", "src=\"mouse.x\"").replace(
            "prop=\"opacity\" src=\"time\"",
            "prop=\"rotation\" src=\"time\"",
        ),
    )
    .unwrap();
    let codes: Vec<&str> = validate(&bad).issues.iter().map(|i| i.code).collect();
    assert!(codes.contains(&"unknown_signal"), "{codes:?}");
    assert!(codes.contains(&"unbound_prop"), "{codes:?}");
}

#[test]
fn a_transform_child_reads_into_the_state_and_writes_back_elided_against_the_identity() {
    use recast_scene::bind::{LayerRef, Transform3};
    let text = r##"<recast v="3" timebase="source-seconds">
  <timeline in="0" out="10"/>
  <screen id="scr"><transform ry="35" z="0.15"/></screen>
  <annotations>
    <rect id="a1" at="1" dur="2" x="0.1" y="0.1" w="0.2" h="0.2"><transform scale="1.2" ax="0" ay="0"/></rect>
  </annotations>
</recast>
"##;
    let doc = parse(text).unwrap();
    assert!(validate(&doc).is_ok(), "{:?}", validate(&doc).issues);
    let state = to_render_state(&doc).unwrap();
    assert_eq!(state.transforms.len(), 2);
    assert_eq!(state.transforms[0].layer, LayerRef::Screen);
    assert_eq!(
        state.transforms[0].transform,
        Transform3 {
            ry: 35.0,
            z: 0.15,
            ..Transform3::IDENTITY
        }
    );
    let scene = super::read::to_scene(&doc).unwrap();
    assert_eq!(scene.layers[1].transform.ry, 35.0);
    let mut ids = IdGen::seeded(4);
    let rewritten = serialize(&from_render_state(
        &state,
        &MediaRefs::default(),
        Some(&doc),
        &mut ids,
    ));
    assert!(
        rewritten.contains("<transform z=\"0.15\" ry=\"35\"/>"),
        "{rewritten}"
    );
    assert!(
        rewritten.contains("<transform scale=\"1.2\" ax=\"0\" ay=\"0\"/>"),
        "{rewritten}"
    );
    assert_eq!(
        to_render_state(&parse(&rewritten).unwrap())
            .unwrap()
            .transforms,
        state.transforms
    );
}

#[test]
fn media_refs_lift_back_out_of_the_document_they_were_written_into() {
    let refs = MediaRefs {
        recording: Some(MediaFile {
            src: "media/recording.mp4".into(),
            width: 1920,
            height: 1080,
            fps: 60.0,
            duration: 11.35,
            offset: 0.0,
        }),
        camera: Some(MediaFile {
            src: "media/camera.mp4".into(),
            offset: 0.856,
            ..Default::default()
        }),
        mic: Some("media/mic.wav".into()),
        system: Some("media/system.wav".into()),
        cursor: Some(TrackFile {
            src: "tracks/cursor.json".into(),
            rows: 1200,
            span: Some((0.0, 11.3)),
            ..Default::default()
        }),
        words: Some(TrackFile::words(
            "tracks/words.json",
            &json!({ "engine": "parakeet", "modelId": "v3", "language": "en",
                     "segments": [{ "words": [{ "start": 0.5, "end": 0.9 }, { "start": 1.0, "end": 1.4 }] }] }),
        )),
    };
    let mut ids = IdGen::seeded(1);
    let doc = from_render_state(&rich_state(), &refs, None, &mut ids);
    assert_eq!(MediaRefs::from_document(&doc), refs);
    assert_eq!(refs.words.as_ref().unwrap().rows, 2);
    assert_eq!(refs.words.as_ref().unwrap().span, Some((0.5, 1.4)));
}

#[test]
fn a_rich_state_survives_the_document_and_comes_back_equal() {
    let state = rich_state();
    let doc = from_render_state(&state, &media(), None, &mut IdGen::seeded(1));
    let back = to_render_state(&doc).unwrap();
    assert_eq!(back, state, "\n{}", serialize(&doc));
}

#[test]
fn the_writer_emits_a_schema_valid_document_with_no_warnings() {
    let doc = from_render_state(&rich_state(), &media(), None, &mut IdGen::seeded(1));
    let report = validate(&doc);
    assert!(
        report.issues.is_empty(),
        "{:#?}\n{}",
        report.issues,
        serialize(&doc)
    );
}

#[test]
fn writing_reading_and_writing_again_is_idempotent_and_deterministic() {
    let first = from_render_state(&rich_state(), &media(), None, &mut IdGen::seeded(9));
    let second = from_render_state(
        &to_render_state(&first).unwrap(),
        &media(),
        Some(&first),
        &mut IdGen::seeded(9),
    );
    assert_eq!(serialize(&first), serialize(&second));
    let again = from_render_state(&rich_state(), &media(), None, &mut IdGen::seeded(9));
    assert_eq!(serialize(&first), serialize(&again));
}

#[test]
fn a_default_state_serialises_to_a_small_skeleton() {
    let state = RenderState {
        trim_end: 10.0,
        ..RenderState::default()
    };
    let doc = from_render_state(&state, &MediaRefs::default(), None, &mut IdGen::seeded(1));
    let text = serialize(&doc);
    assert!(text.lines().count() < 20, "{text}");
    assert!(!text.contains("<media"));
    assert!(text.contains("out=\"10.000\""), "{text}");
}

#[test]
fn base_elements_the_state_cannot_carry_survive_a_rewrite() {
    let base = parse(
        "<recast v=\"3\"><vars><var name=\"accent\" type=\"color\" value=\"#ff5c5c\"/></vars><future id=\"f1\"/></recast>",
    )
    .unwrap();
    let doc = from_render_state(&rich_state(), &media(), Some(&base), &mut IdGen::seeded(3));
    let text = serialize(&doc);
    assert!(text.contains("<var name=\"accent\""));
    assert!(text.contains("<future id=\"f1\"/>"));
}

/// Lighting is a few numbers on the layer, and absent means unlit, which is
/// how every project made before it renders.
#[test]
fn a_material_reads_writes_and_stays_off_when_it_is_not_declared() {
    let src = concat!(
        r#"<recast v="3"><timeline in="0.000" out="10.000"/>"#,
        r#"<screen id="scr"><material contact="0.4" rim="0.25" rimWidth="0.06"/></screen>"#,
        r#"</recast>"#
    );

    let state = to_render_state(&parse(src).unwrap()).unwrap();

    assert_eq!(state.materials.len(), 1);
    assert!((state.materials[0].material.contact - 0.4).abs() < 1e-9);
    assert!((state.materials[0].material.rim_width - 0.06).abs() < 1e-9);

    let again = from_render_state(&state, &MediaRefs::default(), None, &mut IdGen::seeded(13));
    let text = serialize(&again);
    assert!(text.contains(r#"<material contact="0.4""#), "{text}");
    assert_eq!(to_render_state(&again).unwrap().materials, state.materials);

    let unlit = r#"<recast v="3"><timeline in="0.000" out="10.000"/><screen id="scr"/></recast>"#;
    let bare = to_render_state(&parse(unlit).unwrap()).unwrap();
    assert!(bare.materials.is_empty(), "no element, no lighting");
    let rewritten = from_render_state(&bare, &MediaRefs::default(), None, &mut IdGen::seeded(14));
    assert!(
        !serialize(&rewritten).contains("<material"),
        "and none written back"
    );
}

/// A composition has no recording, so the sequence's own end has to become the
/// output duration or every clock downstream would think the project is empty.
#[test]
fn a_sequence_reads_as_a_composition_and_sets_the_output_duration() {
    use recast_scene::composition::{Fit, ItemContent, TextAlign, Transition};
    let src = concat!(
        r##"<recast v="3"><sequence transition="dissolve" transitionDur="0.500">"##,
        r#"<img id="i1" at="0.000" dur="4.000" src="media/a.png" fit="contain" radius="0.02"/>"#,
        r##"<text id="t1" at="3.500" dur="3.000" size="0.08" color="#ffffff" align="left" y="0.4">Hello</text>"##,
        r#"</sequence></recast>"#
    );

    let state = to_render_state(&parse(src).unwrap()).unwrap();

    let composition = state.composition.as_ref().expect("a composition");
    assert_eq!(composition.transition, Transition::Dissolve);
    assert_eq!(composition.items.len(), 2);
    assert!((state.trim_end - 6.5).abs() < 1e-9, "the last item's end");
    match &composition.items[0].content {
        ItemContent::Image { src, fit, radius } => {
            assert_eq!(src, "media/a.png");
            assert_eq!(*fit, Fit::Contain);
            assert!((radius - 0.02).abs() < 1e-9);
        }
        other => panic!("{other:?}"),
    }
    match &composition.items[1].content {
        ItemContent::Text { content, align, .. } => {
            assert_eq!(content, "Hello");
            assert_eq!(*align, TextAlign::Start, "the CSS spelling reads too");
        }
        other => panic!("{other:?}"),
    }
    assert!((composition.items[1].area.y - 0.4).abs() < 1e-9);
}

#[test]
fn a_composition_survives_a_whole_state_rewrite() {
    let src = concat!(
        r##"<recast v="3"><sequence transition="dissolve" transitionDur="0.500">"##,
        r#"<img id="i1" at="0.000" dur="4.000" src="media/a.png" fit="contain"/>"#,
        r##"<text id="t1" at="3.500" dur="3.000" size="0.08" color="#ffffff">Hello</text>"##,
        r#"</sequence></recast>"#
    );
    let state = to_render_state(&parse(src).unwrap()).unwrap();

    let again = from_render_state(&state, &MediaRefs::default(), None, &mut IdGen::seeded(11));

    let text = serialize(&again);
    assert!(
        text.contains(r#"<sequence transition="dissolve""#),
        "{text}"
    );
    assert!(text.contains(">Hello</text>"), "{text}");
    assert_eq!(
        to_render_state(&again).unwrap().composition,
        state.composition
    );
}

/// The inspector edits declarations, so they travel in the state rather than
/// being copied off the file: an editor that never saw the document keeps them.
#[test]
fn declarations_round_trip_through_the_state_and_an_edited_value_reaches_the_file() {
    use recast_scene::vars::VarType;
    let src = concat!(
        r##"<recast v="3"><vars>"##,
        r##"<var name="accent" type="color" value="#22d3ee" path="Brand"/>"##,
        r#"<var name="pad" type="number" value="40" min="0" max="100" step="5"/>"#,
        r#"<var name="mood" type="select" value="warm" options="warm,cool"/>"#,
        r#"</vars><timeline in="0.000" out="10.000"/></recast>"#
    );

    let mut state = to_render_state(&parse(src).unwrap()).unwrap();

    assert_eq!(state.vars.len(), 3);
    assert_eq!(state.vars[0].ty, VarType::Color);
    assert_eq!(state.vars[0].path.as_deref(), Some("Brand"));
    assert_eq!(state.vars[1].min, Some(0.0));
    assert_eq!(state.vars[2].options, ["warm", "cool"]);

    state.vars[0].value = "#ff0000".into();
    let doc = from_render_state(&state, &MediaRefs::default(), None, &mut IdGen::seeded(7));

    let text = serialize(&doc);
    assert!(text.contains(r##"value="#ff0000""##), "{text}");
    assert!(text.contains(r#"options="warm,cool""#), "{text}");
    assert_eq!(
        to_render_state(&doc).unwrap().vars,
        state.vars,
        "and the whole block survives the trip"
    );
}

/// A uniform may name a variable, and the engine is handed the value: nothing
/// downstream of the reader knows variables exist.
#[test]
fn a_shader_uniform_may_be_a_variable_and_reaches_the_engine_resolved() {
    let src = concat!(
        r##"<recast v="3"><vars><var name="accent" type="color" value="#22d3ee"/>"##,
        r#"<var name="spin" type="number" value="42"/></vars>"#,
        r#"<timeline in="0.000" out="10.000"/>"#,
        r#"<shader id="s1" component="sweep@1.0">"#,
        r#"<uniform name="tint" value="$accent"/><uniform name="angle" value="$spin"/>"#,
        r#"</shader></recast>"#
    );
    let doc = parse(src).unwrap();

    let state = to_render_state(&doc).unwrap();

    let params = &state.graphics[0].params;
    assert_eq!(params.get("tint").map(String::as_str), Some("#22d3ee"));
    assert_eq!(params.get("angle").map(String::as_str), Some("42"));
    assert!(
        serialize(&doc).contains("value=\"$accent\""),
        "and the document still holds the reference"
    );
}

/// Both spellings survive a whole-state rewrite through the state itself, not
/// by being copied off the base: an editor that never saw the file still keeps them.
#[test]
fn component_instances_round_trip_through_the_state() {
    let src = concat!(
        "<recast v=\"3\">",
        "<timeline in=\"0.000\" out=\"10.000\"/>",
        "<graphic id=\"g1\" component=\"spotlight@1.0\" at=\"1.000\" dur=\"4.000\" radius=\"0.2\"/>",
        "<shader id=\"s1\" component=\"sweep@1.0\"><uniform name=\"angle\" value=\"20\"/></shader>",
        "</recast>"
    );
    let doc = parse(src).unwrap();

    let state = to_render_state(&doc).unwrap();
    let again = from_render_state(&state, &MediaRefs::default(), None, &mut IdGen::seeded(4));

    assert_eq!(state.graphics.len(), 2);
    assert_eq!(
        state.graphics[0].params.get("radius").map(String::as_str),
        Some("0.2")
    );
    assert_eq!(
        state.graphics[1].params.get("angle").map(String::as_str),
        Some("20")
    );
    let text = serialize(&again);
    assert!(
        text.contains("<graphic id=\"g1\" component=\"spotlight@1.0\""),
        "{text}"
    );
    assert!(
        text.contains("<uniform name=\"angle\" value=\"20\"/>"),
        "{text}"
    );
    assert_eq!(to_render_state(&again).unwrap().graphics, state.graphics);
}

#[test]
fn a_hand_written_document_reads_into_the_state_it_describes() {
    let src = r##"<recast v="3" aspect="9:16" pad="12">
  <media id="rec" kind="video" src="media/recording.mp4"/>
  <timeline src="rec" in="1" out="60"><cuts><cut id="c1" at="10" dur="2"/></cuts><clip id="k1" at="20" speed="2" layout="cameraOnly"/></timeline>
  <background><solid color="#123456"/></background>
  <screen id="scr" src="rec"><corner pct="2"/><zooms><zoom id="z1" at="5" dur="3" scale="2" cx="0.25" cy="0.75"/></zooms></screen>
  <camera id="bubble" enabled="true" shape="circle" x="0.1" y="0.1" w="0.2" h="0.2">
    <bind id="follow" prop="x y" src="zoom.center" map="follow" strength="0.9"/>
  </camera>
  <cursor id="pointer" enabled="true" style="windows" smoothing="0.25"/>
  <annotations><rect id="r1" at="3" dur="2" x="0.1" y="0.1" w="0.3" h="0.2" fill="none" strokeStyle="dashed"/></annotations>
  <audio gain="0.5"><mic id="mic" src="media/mic.wav" muted="true"/></audio>
</recast>"##;
    let state = to_render_state(&parse(src).unwrap()).unwrap();
    assert_eq!(state.output_aspect.as_deref(), Some("9:16"));
    assert_eq!(state.padding, 12.0);
    assert_eq!((state.trim_start, state.trim_end), (1.0, 60.0));
    assert_eq!(state.cuts[0].end, 12.0);
    assert_eq!(state.segment_speeds[0].speed, 2.0);
    assert!(matches!(
        state.camera_overlay.clip_layouts[0].layout,
        CameraLayout::CameraOnly
    ));
    assert_eq!(state.background_value, "#123456");
    assert_eq!(state.border_radius, 2.0);
    assert_eq!(state.zoom_regions[0].center_y, 0.75);
    assert!(state.camera_overlay.enabled);
    assert!(state.camera_overlay.zoom_follow && !state.camera_overlay.cursor_dodge);
    assert_eq!(state.camera_overlay.zoom_follow_strength, 0.9);
    assert_eq!(state.cursor_smoothing, 25.0);
    assert_eq!(state.passthrough["cursorStyle"], json!("windows"));
    assert_eq!(state.annotations[0].fill, "transparent");
    assert_eq!(state.audio_settings.volume, 50.0);
    assert!(state.audio_settings.mic_muted);
}

#[test]
fn a_missing_required_attribute_is_named_with_its_element_and_id() {
    let doc = parse("<recast v=\"3\"><timeline out=\"5\"><cuts><cut id=\"c1\" at=\"1\"/></cuts></timeline></recast>").unwrap();
    let err = to_render_state(&doc).unwrap_err();
    assert_eq!(err.to_string(), "<cut id=\"c1\"> needs dur");
    let err = to_render_state(&parse("<recast v=\"2\"/>").unwrap()).unwrap_err();
    assert!(err.to_string().contains("version 2"));
}

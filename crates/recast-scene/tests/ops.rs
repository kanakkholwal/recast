use recast_scene::migrate::to_scene;
use recast_scene::ops::{apply, apply_all, is_v1_representable, with_render_state, Op, OpError};
use recast_scene::v1::RenderState;
use recast_scene::{Effect, Layer, LayerId, LayerSource, Scene};
use serde_json::{json, Map, Value};

fn scene() -> Scene {
    to_scene(&RenderState {
        trim_end: 60.0,
        ..RenderState::default()
    })
}

fn set(path: &str, value: Value) -> Op {
    Op::Set {
        path: path.into(),
        value,
    }
}

fn at(scene: &Scene, path: &str) -> Value {
    let doc = serde_json::to_value(scene).expect("serialisable");
    let parsed = recast_scene::ScenePath::parse(path).expect("a path");
    recast_scene::ops::path::resolve(&doc, &parsed)
        .cloned()
        .unwrap_or(Value::Null)
}

#[test]
fn a_set_writes_the_field_the_path_names() {
    let mut s = scene();
    apply(&mut s, &set("output/padding", json!(12.0))).expect("applied");
    assert_eq!(at(&s, "output/padding"), json!(12.0));
}

#[test]
fn a_set_at_an_unknown_path_changes_nothing() {
    let mut s = scene();
    let before = s.clone();
    let err = apply(&mut s, &set("output/nonsense/deeper", json!(1))).unwrap_err();
    assert!(matches!(err, OpError::NotFound(_)), "got {err:?}");
    assert_eq!(s, before, "a failed op moved the scene");
}

/// The important half of "pure": a value of the wrong shape has to be refused
/// with the scene untouched, not applied and left to break a later read.
#[test]
fn a_value_that_does_not_fit_leaves_the_scene_exactly_as_it_was() {
    let mut s = scene();
    let before = s.clone();
    let err = apply(&mut s, &set("output/padding", json!("not a number"))).unwrap_err();
    assert!(matches!(err, OpError::TypeMismatch { .. }), "got {err:?}");
    assert_eq!(s, before, "a rejected value was half applied");
}

#[test]
fn a_merge_writes_several_fields_of_one_object() {
    let mut s = scene();
    let mut patch = Map::new();
    patch.insert("padding".into(), json!(8.0));
    patch.insert("aspect".into(), json!("9:16"));
    apply(
        &mut s,
        &Op::Merge {
            path: "output".into(),
            patch,
        },
    )
    .expect("applied");
    assert_eq!(at(&s, "output/padding"), json!(8.0));
    assert_eq!(at(&s, "output/aspect"), json!("9:16"));
}

#[test]
fn a_merge_onto_something_that_is_not_an_object_is_refused() {
    let mut s = scene();
    let before = s.clone();
    let err = apply(
        &mut s,
        &Op::Merge {
            path: "output/padding".into(),
            patch: Map::new(),
        },
    )
    .unwrap_err();
    assert!(matches!(err, OpError::TypeMismatch { .. }), "got {err:?}");
    assert_eq!(s, before);
}

#[test]
fn a_layer_can_be_added_removed_and_moved() {
    let mut s = scene();
    let base = s.layers.len();
    let id = s.next_layer_id();
    apply(
        &mut s,
        &Op::LayerAdd {
            layer: Box::new(Layer::new(id.0, LayerSource::Solid { color: Default::default() })),
        },
    )
    .expect("added");
    assert_eq!(s.layers.len(), base + 1);
    assert_eq!(s.layers.last().map(|l| l.id), Some(id));

    apply(&mut s, &Op::LayerMove { id: id.0, to: 0 }).expect("moved");
    assert_eq!(s.layers.first().map(|l| l.id), Some(id));

    apply(&mut s, &Op::LayerRemove { id: id.0 }).expect("removed");
    assert_eq!(s.layers.len(), base);
}

/// Index IS z-order, so a move has to shift the rest rather than swap.
#[test]
fn moving_a_layer_shifts_the_others_rather_than_swapping() {
    let mut s = Scene::default();
    for i in 0..4 {
        s.layers
            .push(Layer::new(i, LayerSource::Solid { color: Default::default() }));
    }
    apply(&mut s, &Op::LayerMove { id: 3, to: 1 }).expect("moved");
    let order: Vec<u32> = s.layers.iter().map(|l| l.id.0).collect();
    assert_eq!(order, vec![0, 3, 1, 2]);
}

/// Clamping a move to the end instead of refusing it would quietly put the
/// layer somewhere the caller did not ask for, which in a z-order is a visible
/// wrong answer rather than a near-miss.
#[test]
fn moving_a_layer_past_the_end_is_refused_rather_than_clamped() {
    let mut s = Scene::default();
    for i in 0..3 {
        s.layers
            .push(Layer::new(i, LayerSource::Solid { color: Default::default() }));
    }
    let before = s.clone();
    let err = apply(&mut s, &Op::LayerMove { id: 0, to: 3 }).unwrap_err();
    assert!(
        matches!(err, OpError::BadPosition { index: 3, len: 3 }),
        "got {err:?}"
    );
    assert_eq!(s, before);
}

#[test]
fn an_unknown_layer_is_an_error_rather_than_a_silent_no_op() {
    let mut s = scene();
    let before = s.clone();
    for op in [
        Op::LayerRemove { id: 999 },
        Op::LayerMove { id: 999, to: 0 },
        Op::EffectRemove {
            layer: 999,
            index: 0,
        },
    ] {
        let err = apply(&mut s, &op).unwrap_err();
        assert!(matches!(err, OpError::NoSuchLayer(999)), "got {err:?}");
    }
    assert_eq!(s, before);
}

#[test]
fn an_effect_can_be_added_and_removed_on_a_named_layer() {
    let mut s = scene();
    let id = s.layers.first().expect("a layer").id.0;
    let before = s.layers[0].effects.len();
    apply(
        &mut s,
        &Op::EffectAdd {
            layer: id,
            effect: Box::new(Effect::Blur { amount: 0.4 }),
        },
    )
    .expect("added");
    assert_eq!(s.layers[0].effects.len(), before + 1);

    apply(
        &mut s,
        &Op::EffectRemove {
            layer: id,
            index: before,
        },
    )
    .expect("removed");
    assert_eq!(s.layers[0].effects.len(), before);
}

#[test]
fn removing_an_effect_past_the_end_is_refused() {
    let mut s = scene();
    let id = s.layers.first().expect("a layer").id.0;
    let before = s.clone();
    let err = apply(
        &mut s,
        &Op::EffectRemove {
            layer: id,
            index: 99,
        },
    )
    .unwrap_err();
    assert!(matches!(err, OpError::NoSuchEffect { .. }), "got {err:?}");
    assert_eq!(s, before);
}

/// A layer id addresses the same layer however the list has been reordered. A
/// position does not, and that is what makes it wrong for a journal.
#[test]
fn an_op_addressed_by_layer_id_survives_a_reorder() {
    let mut s = Scene::default();
    for i in 0..3 {
        s.layers
            .push(Layer::new(i, LayerSource::Solid { color: Default::default() }));
    }
    apply(&mut s, &Op::LayerMove { id: 2, to: 0 }).expect("moved");
    apply(&mut s, &set("layers/id:2/opacity", json!(0.25))).expect("applied");

    let target = s.layers.iter().find(|l| l.id == LayerId(2)).expect("layer 2");
    assert_eq!(target.opacity, 0.25);
    // And nobody else moved.
    assert!(s
        .layers
        .iter()
        .filter(|l| l.id != LayerId(2))
        .all(|l| l.opacity == 1.0));
}

/// The property a journal rests on. Same base, same ops, same scene, every time.
#[test]
fn replaying_the_same_ops_on_the_same_scene_gives_the_same_scene() {
    let ops = vec![
        set("output/padding", json!(12.0)),
        Op::LayerAdd {
            layer: Box::new(Layer::new(77, LayerSource::Solid { color: Default::default() })),
        },
        Op::EffectAdd {
            layer: 77,
            effect: Box::new(Effect::CornerRadius { percent: 12.0 }),
        },
        Op::LayerMove { id: 77, to: 0 },
        set("layers/id:77/opacity", json!(0.5)),
    ];

    let mut first = scene();
    apply_all(&mut first, &ops).expect("applied");
    let mut second = scene();
    apply_all(&mut second, &ops).expect("applied");
    assert_eq!(first, second);

    // And byte-stable through JSON, which is what gets hashed and stored.
    let a = serde_json::to_string(&first).expect("serialisable");
    let b = serde_json::to_string(&second).expect("serialisable");
    assert_eq!(a, b);
}

/// Ops are stored, so their names and fields are a wire contract. This pins the
/// exact JSON: a rename here is a break, and it should be read as one.
#[test]
fn the_wire_shape_of_an_op_is_pinned() {
    let op = set("output/padding", json!(12.0));
    assert_eq!(
        serde_json::to_value(&op).expect("serialisable"),
        json!({"op": "set", "path": "output/padding", "value": 12.0})
    );

    let decoded: Op = serde_json::from_value(json!({
        "op": "layerMove", "id": 4, "to": 2
    }))
    .expect("decodes");
    assert_eq!(decoded, Op::LayerMove { id: 4, to: 2 });
}

#[test]
fn every_op_round_trips_through_json() {
    let ops = vec![
        set("output/padding", json!(12.0)),
        Op::Merge {
            path: "output".into(),
            patch: Map::new(),
        },
        Op::LayerAdd {
            layer: Box::new(Layer::new(3, LayerSource::Screen)),
        },
        Op::LayerRemove { id: 3 },
        Op::LayerMove { id: 3, to: 1 },
        Op::EffectAdd {
            layer: 3,
            effect: Box::new(Effect::Blur { amount: 1.0 }),
        },
        Op::EffectRemove { layer: 3, index: 0 },
    ];
    for op in ops {
        let text = serde_json::to_string(&op).expect("serialisable");
        let back: Op = serde_json::from_str(&text).expect("decodes");
        assert_eq!(back, op, "{text}");
    }
}

/// An op is only replayable if it never reads anything outside the scene it is
/// given. Applying to a default scene and to a populated one must differ ONLY
/// where the op touched.
#[test]
fn an_op_reads_nothing_outside_the_scene_it_is_given() {
    let mut populated = scene();
    let mut untouched = scene();
    apply(&mut populated, &set("output/padding", json!(12.0))).expect("applied");

    untouched.output.padding = 12.0;
    assert_eq!(populated, untouched);
}

/// `aspect` is `skip_serializing_if = "Option::is_none"`, so it is ABSENT from
/// the JSON of a scene that has none. Treating absent as "no such field" would
/// make an optional field unsettable, which is the bug the write path guards.
#[test]
fn an_optional_field_that_is_currently_absent_can_still_be_set() {
    let mut s = scene();
    s.output.aspect = None;
    assert_eq!(at(&s, "output/aspect"), Value::Null, "the fixture is not absent");

    apply(&mut s, &set("output/aspect", json!("16:9"))).expect("applied");
    assert_eq!(s.output.aspect.as_deref(), Some("16:9"));
}

/// The journal migration. Every op already on disk addresses a flat
/// `RenderState` field, and it keeps working through the projection.
#[test]
fn a_v1_edit_reaches_a_scene_through_the_projection() {
    let mut s = scene();
    with_render_state(&mut s, |state| state.trim_start = 3.0).expect("v1 representable");
    assert_eq!(s.timeline.trim_start, 3.0);
}

/// The trap, and why the bridge checks rather than assumes: `RenderState` has no
/// room for an arbitrary extra layer, so projecting a scene that has one down
/// and back would DROP it. Silently, and with a perfectly valid scene left over.
#[test]
fn a_v1_edit_is_refused_on_a_scene_v1_cannot_hold() {
    let mut s = scene();
    assert!(is_v1_representable(&s), "the fixture is already beyond v1");

    let fresh = s.next_layer_id().0;
    apply(
        &mut s,
        &Op::LayerAdd {
            layer: Box::new(Layer::new(
                fresh,
                LayerSource::Solid { color: Default::default() },
            )),
        },
    )
    .expect("added");
    assert!(!is_v1_representable(&s), "v1 was able to hold the extra layer");

    let before = s.clone();
    let err = with_render_state(&mut s, |state| state.trim_start = 3.0).unwrap_err();
    assert!(matches!(err, OpError::NotV1Representable), "got {err:?}");
    assert_eq!(s, before, "a refused v1 edit still moved the scene");
}

/// Reordering is the other way a scene leaves v1 behind: v1 has a fixed layer
/// order baked into its shape, so a moved layer cannot survive the projection.
#[test]
fn reordering_layers_takes_a_scene_beyond_what_v1_can_hold() {
    let mut s = scene();
    if s.layers.len() < 2 {
        return;
    }
    let id = s.layers[1].id.0;
    apply(&mut s, &Op::LayerMove { id, to: 0 }).expect("moved");
    assert!(
        !is_v1_representable(&s),
        "v1 round-tripped a reordered scene, so the order is not really carried"
    );
}

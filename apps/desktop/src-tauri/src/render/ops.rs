//! Typed, replayable edits over a [`RenderState`]; the CLI socket, branch journal and MCP all reduce to [`apply_op`].
//! Pure in `(state, op)`: no clock, randomness or I/O, so anything generated at edit time is baked in at the dispatch edge.

use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use super::graph::{CutRange, RenderState, SegmentSpeed};
use super::node_types::{Annotation, ZoomRegion};
use super::scene_anim::{SceneAnimSpec, SegmentAnim};

/// Tolerance for "the same boundary" when matching a row by value, mirroring
/// `VALIDATION_EPS` in `commands::editor`.
const MATCH_EPS: f64 = 1e-4;

/// Why an [`Op`] could not be applied to the state it was handed.
#[derive(Debug, thiserror::Error)]
pub enum OpError {
    #[error("cut index {index} is out of range ({len} cuts)")]
    CutIndexOutOfRange { index: usize, len: usize },
    #[error("cut.remove needs an index, or both start and end")]
    CutSelectorMissing,
    #[error("no cut matching start={start}, end={end}")]
    CutNotFound { start: f64, end: f64 },
    #[error("zoom index {index} is out of range ({len} regions)")]
    ZoomIndexOutOfRange { index: usize, len: usize },
    #[error("no split point at {at}")]
    SplitPointNotFound { at: f64 },
    #[error("no speed override at segment start {start}")]
    SpeedNotFound { start: f64 },
    #[error("no annotation with id '{id}'")]
    AnnotationNotFound { id: String },
    #[error("no scene animation at segment start {start}")]
    AnimationNotFound { start: f64 },
    #[error("no field at path '{field}'")]
    FieldNotFound { field: String },
    #[error("value at '{field}' does not fit the render state")]
    FieldTypeMismatch {
        field: String,
        #[source]
        source: serde_json::Error,
    },
    #[error("patch produced an invalid annotation")]
    AnnotationPatchInvalid {
        #[source]
        source: serde_json::Error,
    },
    #[error("render state is not serializable")]
    StateNotSerializable {
        #[source]
        source: serde_json::Error,
    },
}

/// One edit.
/// Variant and field names are serialized into stored journals, so renaming one invalidates every journal already on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum Op {
    Replace {
        state: Box<RenderState>,
    },
    Trim {
        start: f64,
        end: f64,
    },
    CutAdd {
        start: f64,
        end: f64,
    },
    /// Selected by index, falling back to a `(start, end)` value match.
    CutRemove {
        #[serde(default)]
        index: Option<usize>,
        #[serde(default)]
        start: Option<f64>,
        #[serde(default)]
        end: Option<f64>,
    },
    ZoomAdd {
        region: Box<ZoomRegion>,
    },
    ZoomRemove {
        index: usize,
    },
    SplitPointAdd {
        at: f64,
    },
    SplitPointRemove {
        at: f64,
    },
    SpeedSet {
        segment_start: f64,
        rate: f64,
    },
    SpeedRemove {
        segment_start: f64,
    },
    AnnotationAdd {
        annotation: Box<Annotation>,
    },
    /// Shallow field merge over the annotation's JSON, re-parsed before it lands.
    AnnotationUpdate {
        id: String,
        patch: Map<String, Value>,
    },
    AnnotationRemove {
        id: String,
    },
    AnimationAdd {
        start: f64,
        anim_in: Option<SceneAnimSpec>,
        anim_out: Option<SceneAnimSpec>,
    },
    AnimationRemove {
        start: f64,
    },
    /// Escape hatch for any field without its own op, by dotted JSON pointer.
    Set {
        field: String,
        value: Value,
    },
}

/// Apply one op in place, returning the verb's wire result.
///
/// # Errors
/// Returns [`OpError`] when the op does not match the state (a missing id, an
/// out-of-range index, a patch that breaks the target's shape). The state is
/// left untouched in every error case.
pub fn apply_op(state: &mut RenderState, op: &Op) -> Result<Value, OpError> {
    match op {
        Op::Replace { state: next } => {
            *state = (**next).clone();
            Ok(json!({ "applied": true }))
        }

        Op::Trim { start, end } => {
            state.trim_start = *start;
            state.trim_end = *end;
            Ok(json!({ "trimStart": start, "trimEnd": end }))
        }

        Op::CutAdd { start, end } => {
            state.cuts.push(CutRange {
                start: *start,
                end: *end,
                extra: Map::new(),
            });
            Ok(json!({ "added": { "start": start, "end": end } }))
        }

        Op::CutRemove { index, start, end } => {
            let removed = state
                .cuts
                .remove(locate_cut(&state.cuts, *index, *start, *end)?);
            Ok(json!({ "removed": { "start": removed.start, "end": removed.end } }))
        }

        Op::ZoomAdd { region } => {
            let index = state.zoom_regions.len();
            let (start, end) = (region.start, region.end);
            state.zoom_regions.push((**region).clone());
            Ok(json!({ "index": index, "start": start, "end": end }))
        }

        Op::ZoomRemove { index } => {
            let len = state.zoom_regions.len();
            if *index >= len {
                return Err(OpError::ZoomIndexOutOfRange { index: *index, len });
            }
            let removed = state.zoom_regions.remove(*index);
            Ok(json!({ "removed": { "start": removed.start, "end": removed.end } }))
        }

        Op::SplitPointAdd { at } => {
            if !state.split_points.iter().any(|point| near(*point, *at)) {
                state.split_points.push(*at);
                state.split_points.sort_by(f64::total_cmp);
            }
            Ok(json!({ "added": at }))
        }

        Op::SplitPointRemove { at } => {
            if !retain_away_from(&mut state.split_points, *at, |point| *point) {
                return Err(OpError::SplitPointNotFound { at: *at });
            }
            Ok(json!({ "removed": at }))
        }

        Op::SpeedSet {
            segment_start,
            rate,
        } => {
            match state
                .segment_speeds
                .iter_mut()
                .find(|speed| near(speed.start, *segment_start))
            {
                Some(existing) => existing.speed = *rate,
                None => state.segment_speeds.push(SegmentSpeed {
                    start: *segment_start,
                    speed: *rate,
                }),
            }
            Ok(json!({ "segmentStart": segment_start, "rate": rate }))
        }

        Op::SpeedRemove { segment_start } => {
            if !retain_away_from(&mut state.segment_speeds, *segment_start, |s| s.start) {
                return Err(OpError::SpeedNotFound {
                    start: *segment_start,
                });
            }
            Ok(json!({ "removed": segment_start }))
        }

        Op::AnnotationAdd { annotation } => {
            state.annotations.push((**annotation).clone());
            Ok(json!({ "id": annotation.id }))
        }

        Op::AnnotationUpdate { id, patch } => {
            let target = state
                .annotations
                .iter_mut()
                .find(|annotation| &annotation.id == id)
                .ok_or_else(|| OpError::AnnotationNotFound { id: id.clone() })?;
            *target = merge_annotation(target, patch)?;
            Ok(json!({ "id": id }))
        }

        Op::AnnotationRemove { id } => {
            let before = state.annotations.len();
            state.annotations.retain(|annotation| &annotation.id != id);
            if state.annotations.len() == before {
                return Err(OpError::AnnotationNotFound { id: id.clone() });
            }
            Ok(json!({ "removed": id }))
        }

        Op::AnimationAdd {
            start,
            anim_in,
            anim_out,
        } => {
            let anim = SegmentAnim {
                start: *start,
                anim_in: anim_in.clone(),
                anim_out: anim_out.clone(),
            };
            match state
                .scene_animations
                .iter_mut()
                .find(|existing| near(existing.start, *start))
            {
                Some(existing) => *existing = anim,
                None => state.scene_animations.push(anim),
            }
            Ok(json!({ "start": start }))
        }

        Op::AnimationRemove { start } => {
            if !retain_away_from(&mut state.scene_animations, *start, |anim| anim.start) {
                return Err(OpError::AnimationNotFound { start: *start });
            }
            Ok(json!({ "removed": start }))
        }

        Op::Set { field, value } => {
            *state = set_dotted_path(state, field, value.clone())?;
            Ok(json!({ "applied": true, "field": field }))
        }
    }
}

/// Fold ops in order, returning each one's wire result.
/// # Errors Propagates the first [`OpError`]. `state` is left partially applied, so callers fold onto a clone they can discard.
pub fn apply_ops(state: &mut RenderState, ops: &[Op]) -> Result<Vec<Value>, OpError> {
    ops.iter().map(|op| apply_op(state, op)).collect()
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < MATCH_EPS
}

/// Drop every element whose key is within [`MATCH_EPS`] of `key`, reporting
/// whether anything went.
fn retain_away_from<T>(items: &mut Vec<T>, key: f64, key_of: impl Fn(&T) -> f64) -> bool {
    let before = items.len();
    items.retain(|item| !near(key_of(item), key));
    items.len() != before
}

fn locate_cut(
    cuts: &[CutRange],
    index: Option<usize>,
    start: Option<f64>,
    end: Option<f64>,
) -> Result<usize, OpError> {
    if let Some(index) = index {
        return (index < cuts.len())
            .then_some(index)
            .ok_or(OpError::CutIndexOutOfRange {
                index,
                len: cuts.len(),
            });
    }
    let (Some(start), Some(end)) = (start, end) else {
        return Err(OpError::CutSelectorMissing);
    };
    cuts.iter()
        .position(|cut| near(cut.start, start) && near(cut.end, end))
        .ok_or(OpError::CutNotFound { start, end })
}

fn merge_annotation(
    target: &Annotation,
    patch: &Map<String, Value>,
) -> Result<Annotation, OpError> {
    let mut merged =
        serde_json::to_value(target).map_err(|source| OpError::StateNotSerializable { source })?;
    let Some(fields) = merged.as_object_mut() else {
        return Err(OpError::AnnotationNotFound {
            id: target.id.clone(),
        });
    };
    for (key, value) in patch {
        fields.insert(key.clone(), value.clone());
    }
    serde_json::from_value(merged).map_err(|source| OpError::AnnotationPatchInvalid { source })
}

fn set_dotted_path(state: &RenderState, field: &str, value: Value) -> Result<RenderState, OpError> {
    let mut json =
        serde_json::to_value(state).map_err(|source| OpError::StateNotSerializable { source })?;
    let pointer = format!("/{}", field.replace('.', "/"));
    let target = json
        .pointer_mut(&pointer)
        .ok_or_else(|| OpError::FieldNotFound {
            field: field.to_string(),
        })?;
    *target = value;
    serde_json::from_value(json).map_err(|source| OpError::FieldTypeMismatch {
        field: field.to_string(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::node_types::{AnnotationAnchor, AnnotationKind};

    fn state() -> RenderState {
        RenderState {
            trim_end: 60.0,
            ..RenderState::default()
        }
    }

    fn annotation(id: &str) -> Annotation {
        Annotation {
            id: id.to_string(),
            start: 1.0,
            end: 2.0,
            ramp_in: 0.2,
            ramp_out: 0.2,
            ease_in: Default::default(),
            ease_out: Default::default(),
            stroke: Default::default(),
            fill: "rgba(59,130,246,0.20)".into(),
            kind: AnnotationKind::Rect {
                x: 0.1,
                y: 0.1,
                w: 0.2,
                h: 0.2,
                radius: 0.0,
            },
            name: None,
            z_index: 0,
            locked: false,
            hidden: false,
            opacity: 1.0,
            glow: None,
            anchor: AnnotationAnchor::default(),
        }
    }

    fn zoom(start: f64, end: f64) -> Box<ZoomRegion> {
        Box::new(ZoomRegion {
            start,
            end,
            scale: 1.5,
            ease_in: Default::default(),
            ease_out: Default::default(),
            ramp_in: 0.35,
            ramp_out: 0.35,
            center_x: 0.5,
            center_y: 0.5,
            hidden: false,
            motion_blur: 0.0,
            extra: Map::new(),
        })
    }

    fn state_with_cuts(ranges: &[(f64, f64)]) -> RenderState {
        let mut state = state();
        for &(start, end) in ranges {
            apply_op(&mut state, &Op::CutAdd { start, end }).unwrap();
        }
        state
    }

    fn state_with_annotation(id: &str) -> RenderState {
        let mut state = state();
        apply_op(
            &mut state,
            &Op::AnnotationAdd {
                annotation: Box::new(annotation(id)),
            },
        )
        .unwrap();
        state
    }

    fn patch(key: &str, value: Value) -> Map<String, Value> {
        let mut patch = Map::new();
        patch.insert(key.to_string(), value);
        patch
    }

    mod trim {
        use super::*;

        #[test]
        fn moves_both_ends() {
            let mut state = state();

            apply_op(
                &mut state,
                &Op::Trim {
                    start: 2.0,
                    end: 8.0,
                },
            )
            .unwrap();

            assert_eq!((state.trim_start, state.trim_end), (2.0, 8.0));
        }
    }

    mod cut_remove {
        use super::*;

        #[test]
        fn drops_the_row_at_the_given_index() {
            let mut state = state_with_cuts(&[(1.0, 2.0), (5.0, 6.0)]);

            apply_op(
                &mut state,
                &Op::CutRemove {
                    index: Some(0),
                    start: None,
                    end: None,
                },
            )
            .unwrap();

            assert_eq!(state.cuts[0].start, 5.0);
        }

        #[test]
        fn falls_back_to_matching_start_and_end() {
            let mut state = state_with_cuts(&[(1.0, 2.0), (5.0, 6.0)]);

            apply_op(
                &mut state,
                &Op::CutRemove {
                    index: None,
                    start: Some(5.0),
                    end: Some(6.0),
                },
            )
            .unwrap();

            assert_eq!(state.cuts[0].start, 1.0);
        }

        #[test]
        fn rejects_an_index_past_the_end() {
            let mut state = state_with_cuts(&[(1.0, 2.0)]);

            let error = apply_op(
                &mut state,
                &Op::CutRemove {
                    index: Some(3),
                    start: None,
                    end: None,
                },
            )
            .unwrap_err();

            assert!(
                matches!(error, OpError::CutIndexOutOfRange { index: 3, len: 1 }),
                "got: {error}"
            );
        }

        #[test]
        fn rejects_a_selector_with_neither_index_nor_bounds() {
            let mut state = state_with_cuts(&[(1.0, 2.0)]);

            let error = apply_op(
                &mut state,
                &Op::CutRemove {
                    index: None,
                    start: None,
                    end: None,
                },
            )
            .unwrap_err();

            assert!(matches!(error, OpError::CutSelectorMissing), "got: {error}");
        }

        #[test]
        fn leaves_the_state_untouched_when_nothing_matches() {
            let mut state = state_with_cuts(&[(1.0, 2.0)]);

            let _ = apply_op(
                &mut state,
                &Op::CutRemove {
                    index: None,
                    start: Some(9.0),
                    end: Some(9.5),
                },
            );

            assert_eq!(state.cuts.len(), 1);
        }
    }

    mod zoom {
        use super::*;

        #[test]
        fn add_reports_the_index_it_landed_at() {
            let mut state = state();
            apply_op(
                &mut state,
                &Op::ZoomAdd {
                    region: zoom(1.0, 3.0),
                },
            )
            .unwrap();

            let result = apply_op(
                &mut state,
                &Op::ZoomAdd {
                    region: zoom(4.0, 6.0),
                },
            )
            .unwrap();

            assert_eq!(result["index"], json!(1));
        }

        #[test]
        fn remove_rejects_an_index_past_the_end() {
            let mut state = state();

            let error = apply_op(&mut state, &Op::ZoomRemove { index: 0 }).unwrap_err();

            assert!(
                matches!(error, OpError::ZoomIndexOutOfRange { index: 0, len: 0 }),
                "got: {error}"
            );
        }
    }

    mod split_points {
        use super::*;

        fn state_with_points(points: &[f64]) -> RenderState {
            let mut state = state();
            for &at in points {
                apply_op(&mut state, &Op::SplitPointAdd { at }).unwrap();
            }
            state
        }

        #[test]
        fn stay_sorted_after_an_out_of_order_add() {
            let state = state_with_points(&[5.0, 1.0, 3.0]);

            assert_eq!(state.split_points, vec![1.0, 3.0, 5.0]);
        }

        #[test]
        fn ignore_a_duplicate_add() {
            let state = state_with_points(&[5.0, 5.0]);

            assert_eq!(state.split_points, vec![5.0]);
        }

        #[test]
        fn ignore_an_add_within_the_match_tolerance() {
            let state = state_with_points(&[5.0, 5.000_01]);

            assert_eq!(state.split_points, vec![5.0]);
        }

        #[test]
        fn remove_reports_a_point_that_was_never_there() {
            let mut state = state_with_points(&[1.0]);

            let error = apply_op(&mut state, &Op::SplitPointRemove { at: 3.0 }).unwrap_err();

            assert!(
                matches!(error, OpError::SplitPointNotFound { at } if at == 3.0),
                "got: {error}"
            );
        }
    }

    mod speed {
        use super::*;

        fn set(state: &mut RenderState, segment_start: f64, rate: f64) {
            apply_op(
                state,
                &Op::SpeedSet {
                    segment_start,
                    rate,
                },
            )
            .unwrap();
        }

        #[test]
        fn a_second_set_updates_in_place_rather_than_appending() {
            let mut state = state();
            set(&mut state, 4.0, 2.0);

            set(&mut state, 4.0, 0.5);

            assert_eq!(state.segment_speeds.len(), 1);
        }

        #[test]
        fn a_second_set_keeps_the_newer_rate() {
            let mut state = state();
            set(&mut state, 4.0, 2.0);

            set(&mut state, 4.0, 0.5);

            assert_eq!(state.segment_speeds[0].speed, 0.5);
        }

        #[test]
        fn remove_reports_a_segment_with_no_override() {
            let mut state = state();

            let error = apply_op(&mut state, &Op::SpeedRemove { segment_start: 4.0 }).unwrap_err();

            assert!(
                matches!(error, OpError::SpeedNotFound { start } if start == 4.0),
                "got: {error}"
            );
        }
    }

    mod annotation_update {
        use super::*;

        #[test]
        fn writes_the_patched_field() {
            let mut state = state_with_annotation("a1");

            apply_op(
                &mut state,
                &Op::AnnotationUpdate {
                    id: "a1".into(),
                    patch: patch("opacity", json!(0.4)),
                },
            )
            .unwrap();

            assert_eq!(state.annotations[0].opacity, 0.4);
        }

        #[test]
        fn leaves_untouched_fields_alone() {
            let mut state = state_with_annotation("a1");

            apply_op(
                &mut state,
                &Op::AnnotationUpdate {
                    id: "a1".into(),
                    patch: patch("opacity", json!(0.4)),
                },
            )
            .unwrap();

            assert_eq!(state.annotations[0].start, 1.0);
        }

        #[test]
        fn rejects_a_patch_that_breaks_the_shape() {
            let mut state = state_with_annotation("a1");

            let error = apply_op(
                &mut state,
                &Op::AnnotationUpdate {
                    id: "a1".into(),
                    patch: patch("start", json!("not a number")),
                },
            )
            .unwrap_err();

            assert!(
                matches!(error, OpError::AnnotationPatchInvalid { .. }),
                "got: {error}"
            );
        }

        #[test]
        fn keeps_the_original_when_the_patch_is_rejected() {
            let mut state = state_with_annotation("a1");

            let _ = apply_op(
                &mut state,
                &Op::AnnotationUpdate {
                    id: "a1".into(),
                    patch: patch("start", json!("not a number")),
                },
            );

            assert_eq!(state.annotations[0].start, 1.0);
        }

        #[test]
        fn reports_an_unknown_id() {
            let mut state = state_with_annotation("a1");

            let error = apply_op(
                &mut state,
                &Op::AnnotationUpdate {
                    id: "nope".into(),
                    patch: patch("opacity", json!(0.4)),
                },
            )
            .unwrap_err();

            assert!(
                matches!(error, OpError::AnnotationNotFound { ref id } if id == "nope"),
                "got: {error}"
            );
        }
    }

    mod animation_add {
        use super::*;

        fn add(state: &mut RenderState, start: f64) {
            apply_op(
                state,
                &Op::AnimationAdd {
                    start,
                    anim_in: None,
                    anim_out: None,
                },
            )
            .unwrap();
        }

        #[test]
        fn replaces_the_entry_at_the_same_start() {
            let mut state = state();
            add(&mut state, 3.0);

            add(&mut state, 3.0);

            assert_eq!(state.scene_animations.len(), 1);
        }

        #[test]
        fn keeps_distinct_starts_apart() {
            let mut state = state();
            add(&mut state, 3.0);

            add(&mut state, 9.0);

            assert_eq!(state.scene_animations.len(), 2);
        }
    }

    mod set {
        use super::*;

        #[test]
        fn writes_a_dotted_path() {
            let mut state = state();

            apply_op(
                &mut state,
                &Op::Set {
                    field: "trimStart".into(),
                    value: json!(4.0),
                },
            )
            .unwrap();

            assert_eq!(state.trim_start, 4.0);
        }

        #[test]
        fn reports_a_path_that_does_not_exist() {
            let mut state = state();

            let error = apply_op(
                &mut state,
                &Op::Set {
                    field: "nope.nope".into(),
                    value: json!(1),
                },
            )
            .unwrap_err();

            assert!(
                matches!(error, OpError::FieldNotFound { ref field } if field == "nope.nope"),
                "got: {error}"
            );
        }

        #[test]
        fn reports_a_value_of_the_wrong_type() {
            let mut state = state();

            let error = apply_op(
                &mut state,
                &Op::Set {
                    field: "trimStart".into(),
                    value: json!("half past two"),
                },
            )
            .unwrap_err();

            assert!(
                matches!(error, OpError::FieldTypeMismatch { ref field, .. } if field == "trimStart"),
                "got: {error}"
            );
        }

        #[test]
        fn leaves_the_state_untouched_when_the_value_is_rejected() {
            let mut state = state();

            let _ = apply_op(
                &mut state,
                &Op::Set {
                    field: "trimStart".into(),
                    value: json!("half past two"),
                },
            );

            assert_eq!(state.trim_start, 0.0);
        }
    }

    mod apply_ops {
        use super::*;

        fn script() -> Vec<Op> {
            vec![
                Op::Trim {
                    start: 1.0,
                    end: 30.0,
                },
                Op::CutAdd {
                    start: 4.0,
                    end: 5.0,
                },
                Op::ZoomAdd {
                    region: zoom(6.0, 9.0),
                },
                Op::SplitPointAdd { at: 12.0 },
                Op::SpeedSet {
                    segment_start: 12.0,
                    rate: 1.5,
                },
                Op::AnnotationAdd {
                    annotation: Box::new(annotation("a1")),
                },
                Op::AnimationAdd {
                    start: 12.0,
                    anim_in: None,
                    anim_out: None,
                },
                Op::Set {
                    field: "trimStart".into(),
                    value: json!(2.0),
                },
            ]
        }

        #[test]
        fn replaying_the_same_script_rebuilds_the_same_state() {
            let mut first = state();
            let mut second = state();

            super::apply_ops(&mut first, &script()).unwrap();
            super::apply_ops(&mut second, &script()).unwrap();

            assert_eq!(
                serde_json::to_string(&first).unwrap(),
                serde_json::to_string(&second).unwrap()
            );
        }

        #[test]
        fn stops_at_the_first_failure() {
            let mut state = state();
            let ops = vec![
                Op::CutAdd {
                    start: 1.0,
                    end: 2.0,
                },
                Op::ZoomRemove { index: 7 },
                Op::CutAdd {
                    start: 3.0,
                    end: 4.0,
                },
            ];

            let _ = super::apply_ops(&mut state, &ops);

            assert_eq!(state.cuts.len(), 1);
        }
    }

    mod wire_shape {
        use super::*;

        fn sample_of_every_variant() -> Vec<Op> {
            vec![
                Op::Replace {
                    state: Box::new(state()),
                },
                Op::Trim {
                    start: 0.0,
                    end: 1.0,
                },
                Op::CutAdd {
                    start: 0.0,
                    end: 1.0,
                },
                Op::CutRemove {
                    index: Some(0),
                    start: None,
                    end: None,
                },
                Op::ZoomAdd {
                    region: zoom(0.0, 1.0),
                },
                Op::ZoomRemove { index: 0 },
                Op::SplitPointAdd { at: 1.0 },
                Op::SplitPointRemove { at: 1.0 },
                Op::SpeedSet {
                    segment_start: 0.0,
                    rate: 1.0,
                },
                Op::SpeedRemove { segment_start: 0.0 },
                Op::AnnotationAdd {
                    annotation: Box::new(annotation("a1")),
                },
                Op::AnnotationUpdate {
                    id: "a1".into(),
                    patch: Map::new(),
                },
                Op::AnnotationRemove { id: "a1".into() },
                Op::AnimationAdd {
                    start: 0.0,
                    anim_in: None,
                    anim_out: None,
                },
                Op::AnimationRemove { start: 0.0 },
                Op::Set {
                    field: "trimStart".into(),
                    value: json!(0.0),
                },
            ]
        }

        #[test]
        fn variants_serialize_under_a_camel_case_tag() {
            let wire = serde_json::to_value(Op::SplitPointAdd { at: 1.0 }).unwrap();

            assert_eq!(wire["op"], json!("splitPointAdd"));
        }

        #[test]
        fn fields_serialize_in_camel_case() {
            let wire = serde_json::to_value(Op::SpeedSet {
                segment_start: 1.0,
                rate: 2.0,
            })
            .unwrap();

            assert_eq!(wire["segmentStart"], json!(1.0));
        }

        /// The tag set is the TS/Rust bridge contract. `packages/editor`'s
        /// `EDIT_OP_TAGS` lists the same names; renaming one fails both sides.
        #[test]
        fn every_variant_tag_is_accounted_for() {
            let tags: Vec<String> = sample_of_every_variant()
                .iter()
                .map(|op| {
                    serde_json::to_value(op).unwrap()["op"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect();

            assert_eq!(
                tags,
                [
                    "replace",
                    "trim",
                    "cutAdd",
                    "cutRemove",
                    "zoomAdd",
                    "zoomRemove",
                    "splitPointAdd",
                    "splitPointRemove",
                    "speedSet",
                    "speedRemove",
                    "annotationAdd",
                    "annotationUpdate",
                    "annotationRemove",
                    "animationAdd",
                    "animationRemove",
                    "set",
                ]
            );
        }

        #[test]
        fn a_journal_written_without_optional_fields_still_parses() {
            let stored = json!({ "op": "cutRemove", "start": 1.0, "end": 2.0 });

            let op: Op = serde_json::from_value(stored).unwrap();

            assert!(
                matches!(op, Op::CutRemove { index: None, .. }),
                "got: {op:?}"
            );
        }
    }
}

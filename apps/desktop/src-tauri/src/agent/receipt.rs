//! What a write hands back: the delta in the read's own vocabulary, so the agent patches its model instead of re-reading.
//! `apply_changes` is the contract: patching the state the agent read with `changes` must reproduce the state after the write.

use serde::Serialize;
#[cfg(test)]
use serde_json::Value;

use super::check::{check, Finding, ProjectFacts};
use crate::commands::{derive_project_timeline, KeptSegment};
use crate::project::journal::{self, FieldChange, JournalError, StateHash};
use crate::render::graph::RenderState;

/// The timeline facts a cut or speed change moves, re-emitted so an output-axis model does not rot.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineDelta {
    pub output_duration_before: f64,
    pub output_duration_after: f64,
    pub kept_segments: Vec<KeptSegment>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Receipt {
    pub seq: u64,
    /// `false` when the idem key was already on the branch: the ops sent were IGNORED and `changes` is empty.
    pub recorded: bool,
    pub compacted: bool,
    /// Hash of the state the branch forked from.
    pub base: String,
    /// Hash of the state the branch now produces.
    pub head: String,
    pub changes: Vec<FieldChange>,
    pub timeline: TimelineDelta,
    /// Check findings this append introduced, so the agent sees them before a reviewer does.
    pub introduced: Vec<Finding>,
}

/// Builds the receipt for a write that took `before` to `after`.
/// # Errors When either state cannot be serialised.
pub fn receipt(
    before: &RenderState,
    after: &RenderState,
    base: StateHash,
    facts: &ProjectFacts,
    seq: u64,
    recorded: bool,
    compacted: bool,
) -> Result<Receipt, JournalError> {
    let head = StateHash::of(after)?.to_string();
    receipt_with(
        before,
        after,
        &base.to_string(),
        &head,
        facts,
        seq,
        recorded,
        compacted,
    )
}

/// The receipt with the fork point and head as the journal spells them: a state hash for a bundle, a document hash for a folder.
#[allow(clippy::too_many_arguments)]
pub fn receipt_with(
    before: &RenderState,
    after: &RenderState,
    base: &str,
    head: &str,
    facts: &ProjectFacts,
    seq: u64,
    recorded: bool,
    compacted: bool,
) -> Result<Receipt, JournalError> {
    let changes = if recorded {
        journal::diff(before, after)?
    } else {
        Vec::new()
    };
    let tl_before = derive_project_timeline(before, facts.source_duration);
    let tl_after = derive_project_timeline(after, facts.source_duration);
    Ok(Receipt {
        seq,
        recorded,
        compacted,
        base: base.to_owned(),
        head: head.to_owned(),
        changes,
        timeline: TimelineDelta {
            output_duration_before: tl_before.output_duration,
            output_duration_after: tl_after.output_duration,
            kept_segments: tl_after.kept_segments,
        },
        introduced: check(after, facts).introduced_since(&check(before, facts)),
    })
}

/// Patches `before` with `changes`. Sets land first in order; removals last, deepest index first, so array shifts cannot invalidate a later path.
/// The oracle for the receipt contract; production clients patch on their own side, so it is compiled for tests only.
#[cfg(test)]
pub fn apply_changes(before: &Value, changes: &[FieldChange]) -> Value {
    let mut doc = before.clone();
    for change in changes.iter().filter(|c| c.after.is_some()) {
        if let (Some(slot), Some(after)) = (slot_mut(&mut doc, &change.field), &change.after) {
            *slot = after.clone();
        }
    }
    let mut removals: Vec<&FieldChange> = changes.iter().filter(|c| c.after.is_none()).collect();
    removals.sort_by_key(|c| std::cmp::Reverse(path_order(&c.field)));
    for change in removals {
        remove(&mut doc, &change.field);
    }
    doc
}

/// Numeric-aware ordering so `cuts.10` sorts after `cuts.2`.
#[cfg(test)]
fn path_order(path: &str) -> Vec<(bool, u64, String)> {
    path.split('.')
        .map(|s| match s.parse::<u64>() {
            Ok(n) => (true, n, String::new()),
            Err(_) => (false, 0, s.to_string()),
        })
        .collect()
}

#[cfg(test)]
fn slot_mut<'a>(doc: &'a mut Value, path: &str) -> Option<&'a mut Value> {
    let mut at = doc;
    for step in path.split('.') {
        at = match step.parse::<usize>() {
            Ok(index) => {
                if !at.is_array() {
                    *at = Value::Array(Vec::new());
                }
                let items = at.as_array_mut()?;
                if items.len() <= index {
                    items.resize(index + 1, Value::Null);
                }
                &mut items[index]
            }
            Err(_) => {
                if !at.is_object() {
                    *at = Value::Object(Default::default());
                }
                at.as_object_mut()?.entry(step).or_insert(Value::Null)
            }
        };
    }
    Some(at)
}

#[cfg(test)]
fn remove(doc: &mut Value, path: &str) {
    let Some((parent_path, last)) = path.rsplit_once('.') else {
        if let Some(object) = doc.as_object_mut() {
            object.remove(path);
        }
        return;
    };
    let Some(parent) = slot_mut(doc, parent_path) else {
        return;
    };
    match (last.parse::<usize>(), parent) {
        (Ok(index), Value::Array(items)) => {
            if index < items.len() {
                items.remove(index);
            }
        }
        (_, Value::Object(object)) => {
            object.remove(last);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::render::graph::CutRange;
    use crate::render::node_types::ZoomRegion;
    use crate::render::ops::{apply_op, Op};
    use recast_scene::v1::Easing;

    fn facts() -> ProjectFacts {
        ProjectFacts {
            source_duration: 60.0,
            has_camera: false,
            has_cursor_track: false,
        }
    }

    fn base() -> RenderState {
        let mut state = RenderState {
            trim_start: 0.0,
            trim_end: 60.0,
            ..RenderState::default()
        };
        state.camera_overlay.enabled = false;
        state.cursor_enabled = false;
        state.cuts.push(CutRange {
            start: 5.0,
            end: 6.0,
            extra: Default::default(),
        });
        state.cuts.push(CutRange {
            start: 20.0,
            end: 21.0,
            extra: Default::default(),
        });
        state
    }

    fn zoom(start: f64, end: f64) -> ZoomRegion {
        ZoomRegion {
            start,
            end,
            scale: 1.5,
            ease_in: Easing::default(),
            ease_out: Easing::default(),
            ramp_in: 0.2,
            ramp_out: 0.2,
            center_x: 0.5,
            center_y: 0.5,
            hidden: false,
            motion_blur: 0.0,
            extra: Default::default(),
        }
    }

    /// A tiny deterministic generator, so the property test needs no dependency and every failure replays from its seed.
    struct Lcg(u64);

    impl Lcg {
        fn next(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            self.0 >> 33
        }
        fn secs(&mut self, max: f64) -> f64 {
            (self.next() % 1000) as f64 / 1000.0 * max
        }
    }

    fn random_op(rng: &mut Lcg, state: &RenderState) -> Op {
        match rng.next() % 7 {
            0 => Op::Trim {
                start: rng.secs(5.0),
                end: 30.0 + rng.secs(30.0),
            },
            1 => {
                let start = 25.0 + rng.secs(10.0);
                Op::CutAdd {
                    start,
                    end: start + 0.5 + rng.secs(2.0),
                }
            }
            2 if !state.cuts.is_empty() => Op::CutRemove {
                index: Some((rng.next() as usize) % state.cuts.len()),
                start: None,
                end: None,
            },
            3 => {
                let start = rng.secs(50.0);
                Op::ZoomAdd {
                    region: Box::new(zoom(start, start + 2.0)),
                }
            }
            4 => Op::SplitPointAdd { at: rng.secs(60.0) },
            5 => Op::SpeedSet {
                segment_start: 0.0,
                rate: 1.0 + rng.secs(1.0),
            },
            _ => Op::Set {
                field: "padding".into(),
                value: serde_json::json!(rng.secs(20.0)),
            },
        }
    }

    /// The contract in one assertion: a model that patches its read with the receipt holds the state after the write.
    #[test]
    fn patching_the_read_with_the_receipt_reproduces_the_written_state() {
        for seed in 1..=60u64 {
            let mut rng = Lcg(seed);
            let before = base();
            let mut after = before.clone();
            for _ in 0..(1 + rng.next() % 6) {
                let op = random_op(&mut rng, &after);
                if let Err(e) = apply_op(&mut after, &op) {
                    panic!("seed {seed}: {op:?} failed: {e}");
                }
            }
            let changes = journal::diff(&before, &after).unwrap();
            let patched = apply_changes(&serde_json::to_value(&before).unwrap(), &changes);
            assert_eq!(
                patched,
                serde_json::to_value(&after).unwrap(),
                "seed {seed} diverged with {changes:?}"
            );
        }
    }

    #[test]
    fn a_receipt_carries_hashes_the_timeline_delta_and_new_findings() {
        let before = base();
        let mut after = before.clone();
        apply_op(
            &mut after,
            &Op::CutAdd {
                start: 30.0,
                end: 40.0,
            },
        )
        .unwrap();
        apply_op(
            &mut after,
            &Op::ZoomAdd {
                region: Box::new(zoom(31.0, 39.0)),
            },
        )
        .unwrap();
        let base_hash = StateHash::of(&before).unwrap();
        let r = receipt(&before, &after, base_hash, &facts(), 3, true, false).unwrap();
        assert_eq!(r.seq, 3);
        assert_eq!(r.base, base_hash.to_string());
        assert_eq!(r.head, StateHash::of(&after).unwrap().to_string());
        assert!((r.timeline.output_duration_before - 58.0).abs() < 1e-9);
        assert!((r.timeline.output_duration_after - 48.0).abs() < 1e-9);
        assert!(!r.changes.is_empty());
        assert_eq!(r.introduced.len(), 1);
        assert_eq!(r.introduced[0].code, "zoom_never_visible");
    }

    #[test]
    fn an_ignored_retry_carries_no_changes_and_says_so() {
        let state = base();
        let hash = StateHash::of(&state).unwrap();
        let r = receipt(&state, &state, hash, &facts(), 2, false, false).unwrap();
        assert!(!r.recorded);
        assert!(r.changes.is_empty());
        assert_eq!(r.base, r.head);
    }

    #[test]
    fn removals_apply_deepest_index_first() {
        let doc = serde_json::json!({ "cuts": [1, 2, 3, 4] });
        let changes = vec![
            FieldChange {
                field: "cuts.1".into(),
                before: Some(2.into()),
                after: None,
            },
            FieldChange {
                field: "cuts.3".into(),
                before: Some(4.into()),
                after: None,
            },
        ];
        assert_eq!(
            apply_changes(&doc, &changes),
            serde_json::json!({ "cuts": [1, 3] })
        );
    }
}

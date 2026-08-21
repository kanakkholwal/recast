//! Branch journals: edits an agent has proposed but not yet applied.
//!
//! A branch records a list of [`Op`]s against the [`StateHash`] of the render
//! state it forked from. Nothing here touches the `.recast` bundle, so an agent
//! edit costs one small sidecar write instead of a full archive rewrite; the
//! bundle is rewritten once, when a human applies the branch.
//!
//! The fork point is identified by content hash rather than a revision counter:
//! the hash also catches a bundle edited out of band, which a counter stored
//! beside it would miss.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::render::graph::RenderState;
use crate::render::ops::{apply_ops, Op, OpError};

/// Ops past which a journal should be compacted onto a fresh base.
pub const COMPACT_AFTER_ENTRIES: usize = 512;

/// How long an empty branch survives before it is treated as a crashed agent's
/// leftover. Nothing was ever proposed on it, so there is no work to lose.
pub const EMPTY_BRANCH_MAX_AGE_MS: i64 = 24 * 60 * 60 * 1000;

/// How long a branch with ops goes untouched before it is *marked* stale.
/// Never deleted: it is pending human review, and the reviewer decides.
pub const STALE_AFTER_MS: i64 = 7 * 24 * 60 * 60 * 1000;

#[derive(Debug, thiserror::Error)]
pub enum JournalError {
    #[error("branch '{0}' does not exist")]
    NoSuchBranch(BranchId),
    #[error("branch id '{0}' is not a safe file name")]
    UnsafeBranchId(String),
    #[error("branch forked from {expected} but the project is now at {actual}")]
    BaseMoved {
        expected: StateHash,
        actual: StateHash,
    },
    #[error("expected the branch at seq {expected}, but it is at {actual}")]
    SeqMismatch { expected: u64, actual: u64 },
    #[error("replaying branch '{branch}' failed at seq {seq}")]
    Replay {
        branch: BranchId,
        seq: u64,
        #[source]
        source: OpError,
    },
    #[error("render state is not serializable")]
    StateNotSerializable {
        #[source]
        source: serde_json::Error,
    },
    #[error("journal at {path} is not readable")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("journal at {path} is corrupt")]
    Corrupt {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("journal at {path} could not be written")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

/// Opaque, client-chosen branch name. Doubles as the journal's file stem, so it
/// is restricted to characters that are safe on every platform.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct BranchId(String);

impl BranchId {
    pub fn new(id: impl Into<String>) -> Result<Self, JournalError> {
        let id = id.into();
        let safe = !id.is_empty()
            && id.len() <= 64
            && id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            && !id.starts_with('.');
        if safe {
            Ok(Self(id))
        } else {
            Err(JournalError::UnsafeBranchId(id))
        }
    }
}

impl std::fmt::Display for BranchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Content address of a [`RenderState`], used as the fork point of a branch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StateHash(#[serde(with = "hex_bytes")] [u8; 32]);

impl StateHash {
    pub fn of(state: &RenderState) -> Result<Self, JournalError> {
        let bytes = serde_json::to_vec(state)
            .map_err(|source| JournalError::StateNotSerializable { source })?;
        Ok(Self(Sha256::digest(&bytes).into()))
    }
}

impl std::fmt::Display for StateHash {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&hex::encode(self.0))
    }
}

mod hex_bytes {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8; 32], serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        let bytes = hex::decode(&text).map_err(serde::de::Error::custom)?;
        bytes
            .try_into()
            .map_err(|_| serde::de::Error::custom("expected 32 bytes"))
    }
}

/// One atomic append: every op in it lands, or none of them do.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Entry {
    pub seq: u64,
    pub idem_key: String,
    pub ops: Vec<Op>,
    pub at_ms: i64,
}

/// A set of proposed edits on top of one fork point.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Branch {
    pub id: BranchId,
    pub base: StateHash,
    pub author: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
    #[serde(default)]
    pub entries: Vec<Entry>,
}

/// What an append did, so a retried request can be told apart from a new one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Append {
    Recorded { seq: u64 },
    AlreadyApplied { seq: u64 },
}

impl Append {
    pub fn seq(self) -> u64 {
        match self {
            Self::Recorded { seq } | Self::AlreadyApplied { seq } => seq,
        }
    }

    /// `false` when the idem key was already on the branch.
    pub fn is_recorded(self) -> bool {
        matches!(self, Self::Recorded { .. })
    }
}

impl Branch {
    pub fn new(
        id: BranchId,
        base: StateHash,
        author: impl Into<String>,
        label: Option<String>,
        now_ms: i64,
    ) -> Self {
        Self {
            id,
            base,
            author: author.into(),
            label,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            entries: Vec::new(),
        }
    }

    /// Sequence number the next append will receive.
    pub fn next_seq(&self) -> u64 {
        self.entries.last().map_or(1, |entry| entry.seq + 1)
    }

    pub fn op_count(&self) -> usize {
        self.entries.iter().map(|entry| entry.ops.len()).sum()
    }

    /// Created but never appended to, so discarding it loses nothing.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Untouched long enough to be worth flagging in a review list.
    pub fn is_stale(&self, now_ms: i64) -> bool {
        now_ms - self.updated_at_ms > STALE_AFTER_MS
    }

    pub fn needs_compaction(&self) -> bool {
        self.entries.len() >= COMPACT_AFTER_ENTRIES
    }

    /// Record `ops` as one entry.
    ///
    /// Re-sending an `idem_key` that is already present is a no-op that reports
    /// the original sequence number, so a client that retries after a dropped
    /// socket never double-applies.
    ///
    /// # Errors
    /// [`JournalError::SeqMismatch`] when `expect_seq` does not match
    /// [`Branch::next_seq`], meaning another writer got in first.
    pub fn append(
        &mut self,
        idem_key: impl Into<String>,
        ops: Vec<Op>,
        expect_seq: Option<u64>,
        now_ms: i64,
    ) -> Result<Append, JournalError> {
        let idem_key = idem_key.into();
        if let Some(existing) = self.entries.iter().find(|e| e.idem_key == idem_key) {
            return Ok(Append::AlreadyApplied { seq: existing.seq });
        }

        let seq = self.next_seq();
        if let Some(expected) = expect_seq {
            if expected != seq {
                return Err(JournalError::SeqMismatch {
                    expected,
                    actual: seq,
                });
            }
        }

        self.entries.push(Entry {
            seq,
            idem_key,
            ops,
            at_ms: now_ms,
        });
        self.updated_at_ms = now_ms;
        Ok(Append::Recorded { seq })
    }

    /// Drop every entry after `seq`, undoing the tail of the branch.
    pub fn truncate_after(&mut self, seq: u64, now_ms: i64) {
        let before = self.entries.len();
        self.entries.retain(|entry| entry.seq <= seq);
        if self.entries.len() != before {
            self.updated_at_ms = now_ms;
        }
    }

    /// Fold every recorded op onto the state the branch forked from.
    ///
    /// # Errors
    /// [`JournalError::BaseMoved`] when `base_state` is not the fork point, and
    /// [`JournalError::Replay`] when an op no longer fits the state it reaches.
    pub fn materialize(&self, base_state: &RenderState) -> Result<RenderState, JournalError> {
        let actual = StateHash::of(base_state)?;
        if actual != self.base {
            return Err(JournalError::BaseMoved {
                expected: self.base,
                actual,
            });
        }

        let mut state = base_state.clone();
        for entry in &self.entries {
            apply_ops(&mut state, &entry.ops).map_err(|source| JournalError::Replay {
                branch: self.id.clone(),
                seq: entry.seq,
                source,
            })?;
        }
        Ok(state)
    }

    /// Collapse every recorded op into a single [`Op::Replace`].
    ///
    /// The fork point is left alone: it is what [`Branch::materialize`] checks
    /// the project against, so moving it would reject the very state the branch
    /// is meant to apply to.
    ///
    /// # Errors
    /// Whatever [`Branch::materialize`] would return for `base_state`.
    pub fn compact(&mut self, base_state: &RenderState, now_ms: i64) -> Result<(), JournalError> {
        let folded = self.materialize(base_state)?;
        let seq = self.next_seq();
        self.entries = vec![Entry {
            seq,
            idem_key: format!("compact:{seq}"),
            ops: vec![Op::Replace {
                state: Box::new(folded),
            }],
            at_ms: now_ms,
        }];
        self.updated_at_ms = now_ms;
        Ok(())
    }
}

/// On-disk journals for one project.
///
/// Branches live under the app's data directory rather than beside the
/// `.recast`: they are pending work, so neither the temp-dir sweeper nor the
/// user's own folder is the right home for them.
#[derive(Debug, Clone)]
pub struct BranchStore {
    dir: PathBuf,
}

impl BranchStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, id: &BranchId) -> PathBuf {
        self.dir.join(format!("{id}.json"))
    }

    /// Ids of every readable journal, sorted. Unparseable files are skipped so
    /// one corrupt journal cannot hide the rest.
    pub fn list(&self) -> Vec<BranchId> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut ids: Vec<BranchId> = entries
            .flatten()
            .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "json"))
            .filter_map(|entry| {
                let stem = entry.path().file_stem()?.to_str()?.to_string();
                BranchId::new(stem).ok()
            })
            .collect();
        ids.sort();
        ids
    }

    /// # Errors
    /// [`JournalError::NoSuchBranch`] when the journal is absent, and
    /// [`JournalError::Corrupt`] when it will not parse.
    pub fn load(&self, id: &BranchId) -> Result<Branch, JournalError> {
        let path = self.path_for(id);
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
                return Err(JournalError::NoSuchBranch(id.clone()))
            }
            Err(source) => return Err(JournalError::Read { path, source }),
        };
        serde_json::from_slice(&bytes).map_err(|source| JournalError::Corrupt { path, source })
    }

    /// # Errors
    /// [`JournalError::Write`] when the directory or the temp-then-rename fails.
    pub fn save(&self, branch: &Branch) -> Result<(), JournalError> {
        let path = self.path_for(&branch.id);
        let write = |source| JournalError::Write {
            path: path.clone(),
            source,
        };
        std::fs::create_dir_all(&self.dir).map_err(write)?;
        let bytes = serde_json::to_vec_pretty(branch)
            .map_err(|source| JournalError::StateNotSerializable { source })?;
        let tmp = path.with_extension("json.tmp");
        crate::commands::system::write_atomic(&tmp, &path, &bytes).map_err(write)
    }

    /// Discard branches that are provably worthless: created, never appended to,
    /// and older than [`EMPTY_BRANCH_MAX_AGE_MS`].
    ///
    /// A branch carrying ops is never touched, however old. It is pending human
    /// review, and quietly deleting someone's proposed edits is worse than the
    /// few KB a stale journal costs. Unreadable journals are left alone too: we
    /// cannot tell whether they hold work.
    pub fn sweep(&self, now_ms: i64) -> Vec<BranchId> {
        self.list()
            .into_iter()
            .filter(|id| {
                self.load(id).is_ok_and(|branch| {
                    branch.is_empty() && now_ms - branch.created_at_ms > EMPTY_BRANCH_MAX_AGE_MS
                })
            })
            .filter(|id| self.remove(id).is_ok())
            .collect()
    }

    /// Removing a branch that is already gone succeeds.
    pub fn remove(&self, id: &BranchId) -> Result<(), JournalError> {
        let path = self.path_for(id);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(source) => Err(JournalError::Write { path, source }),
        }
    }
}

/// One leaf that differs between two render states.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FieldChange {
    /// Dotted path, matching the shape `ValidationIssue::field` uses so a
    /// review row can navigate straight to the control.
    pub field: String,
    pub before: Option<serde_json::Value>,
    pub after: Option<serde_json::Value>,
}

/// Leaf-level differences between two render states, in path order.
///
/// # Errors
/// [`JournalError::StateNotSerializable`] if either state will not serialize.
pub fn diff(before: &RenderState, after: &RenderState) -> Result<Vec<FieldChange>, JournalError> {
    let to_json = |state: &RenderState| {
        serde_json::to_value(state).map_err(|source| JournalError::StateNotSerializable { source })
    };
    let mut changes = Vec::new();
    walk_diff("", &to_json(before)?, &to_json(after)?, &mut changes);
    Ok(changes)
}

fn walk_diff(
    path: &str,
    before: &serde_json::Value,
    after: &serde_json::Value,
    changes: &mut Vec<FieldChange>,
) {
    use serde_json::Value;

    let record = |changes: &mut Vec<FieldChange>| {
        changes.push(FieldChange {
            field: path.to_string(),
            before: (!before.is_null()).then(|| before.clone()),
            after: (!after.is_null()).then(|| after.clone()),
        });
    };

    match (before, after) {
        (Value::Object(a), Value::Object(b)) => {
            let mut keys: Vec<&String> = a.keys().chain(b.keys()).collect();
            keys.sort_unstable();
            keys.dedup();
            for key in keys {
                let child = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                walk_diff(
                    &child,
                    a.get(key).unwrap_or(&Value::Null),
                    b.get(key).unwrap_or(&Value::Null),
                    changes,
                );
            }
        }
        (Value::Array(a), Value::Array(b)) => {
            for index in 0..a.len().max(b.len()) {
                walk_diff(
                    &format!("{path}.{index}"),
                    a.get(index).unwrap_or(&Value::Null),
                    b.get(index).unwrap_or(&Value::Null),
                    changes,
                );
            }
        }
        _ if before != after => record(changes),
        _ => {}
    }
}

/// Stable per-project directory name, disambiguating same-named projects in
/// different folders.
pub fn project_key(project_path: &Path) -> String {
    let stem = project_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("project");
    let digest = Sha256::digest(project_path.to_string_lossy().as_bytes());
    format!("{stem}-{}", hex::encode(&digest[..8]))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_700_000_000_000;

    fn state(trim_start: f64) -> RenderState {
        RenderState {
            trim_start,
            trim_end: 60.0,
            ..RenderState::default()
        }
    }

    fn branch_id(id: &str) -> BranchId {
        BranchId::new(id).unwrap()
    }

    fn branch_on(state: &RenderState) -> Branch {
        Branch::new(
            branch_id("agent-1"),
            StateHash::of(state).unwrap(),
            "agent:test",
            None,
            NOW,
        )
    }

    fn cut(start: f64, end: f64) -> Op {
        Op::CutAdd { start, end }
    }

    /// Scratch store in a per-run temp dir, matching `project::reader`'s test
    /// idiom rather than pulling in a dev-dependency.
    fn temp_store() -> BranchStore {
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);

        let n = N.fetch_add(1, Ordering::Relaxed);
        BranchStore::new(
            std::env::temp_dir()
                .join(format!("recast-journal-{}-{n}", std::process::id()))
                .join("branches"),
        )
    }

    mod branch_id {
        use super::*;

        #[test]
        fn accepts_a_plain_slug() {
            assert!(BranchId::new("agent-1_v2.a").is_ok());
        }

        #[test]
        fn rejects_a_path_separator() {
            assert!(BranchId::new("../escape").is_err());
        }

        #[test]
        fn rejects_a_leading_dot() {
            assert!(BranchId::new(".hidden").is_err());
        }

        #[test]
        fn rejects_an_empty_id() {
            assert!(BranchId::new("").is_err());
        }
    }

    mod state_hash {
        use super::*;

        #[test]
        fn identical_states_hash_alike() {
            assert_eq!(
                StateHash::of(&state(1.0)).unwrap(),
                StateHash::of(&state(1.0)).unwrap()
            );
        }

        #[test]
        fn a_changed_field_changes_the_hash() {
            assert_ne!(
                StateHash::of(&state(1.0)).unwrap(),
                StateHash::of(&state(2.0)).unwrap()
            );
        }

        #[test]
        fn round_trips_as_hex() {
            let hash = StateHash::of(&state(1.0)).unwrap();

            let json = serde_json::to_string(&hash).unwrap();

            assert_eq!(serde_json::from_str::<StateHash>(&json).unwrap(), hash);
        }
    }

    mod append {
        use super::*;

        #[test]
        fn numbers_the_first_entry_one() {
            let mut branch = branch_on(&state(0.0));

            let result = branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            assert_eq!(result, Append::Recorded { seq: 1 });
        }

        #[test]
        fn a_repeated_idem_key_reports_the_original_seq() {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            let result = branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            assert_eq!(result, Append::AlreadyApplied { seq: 1 });
        }

        #[test]
        fn a_repeated_idem_key_does_not_record_a_second_entry() {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            assert_eq!(branch.entries.len(), 1);
        }

        #[test]
        fn a_matching_expect_seq_is_accepted() {
            let mut branch = branch_on(&state(0.0));

            let result = branch.append("k1", vec![cut(1.0, 2.0)], Some(1), NOW);

            assert!(result.is_ok(), "rejected: {:?}", result.err());
        }

        #[test]
        fn a_stale_expect_seq_is_rejected() {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            let error = branch
                .append("k2", vec![cut(3.0, 4.0)], Some(1), NOW)
                .unwrap_err();

            assert!(
                matches!(
                    error,
                    JournalError::SeqMismatch {
                        expected: 1,
                        actual: 2
                    }
                ),
                "got: {error}"
            );
        }

        #[test]
        fn a_rejected_append_records_nothing() {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            let _ = branch.append("k2", vec![cut(3.0, 4.0)], Some(1), NOW);

            assert_eq!(branch.entries.len(), 1);
        }
    }

    mod materialize {
        use super::*;

        #[test]
        fn folds_every_recorded_op() {
            let base = state(0.0);
            let mut branch = branch_on(&base);
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();
            branch.append("k2", vec![cut(3.0, 4.0)], None, NOW).unwrap();

            let result = branch.materialize(&base).unwrap();

            assert_eq!(result.cuts.len(), 2);
        }

        #[test]
        fn leaves_the_base_state_untouched() {
            let base = state(0.0);
            let mut branch = branch_on(&base);
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            branch.materialize(&base).unwrap();

            assert!(base.cuts.is_empty());
        }

        #[test]
        fn rejects_a_base_that_moved_under_the_branch() {
            let branch = branch_on(&state(0.0));

            let error = branch.materialize(&state(5.0)).unwrap_err();

            assert!(
                matches!(error, JournalError::BaseMoved { .. }),
                "got: {error}"
            );
        }

        #[test]
        fn reports_the_seq_of_an_op_that_no_longer_fits() {
            let base = state(0.0);
            let mut branch = branch_on(&base);
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();
            branch
                .append("k2", vec![Op::ZoomRemove { index: 9 }], None, NOW)
                .unwrap();

            let error = branch.materialize(&base).unwrap_err();

            assert!(
                matches!(error, JournalError::Replay { seq: 2, .. }),
                "got: {error}"
            );
        }

        #[test]
        fn replaying_twice_gives_the_same_state() {
            let base = state(0.0);
            let mut branch = branch_on(&base);
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            let first = branch.materialize(&base).unwrap();
            let second = branch.materialize(&base).unwrap();

            assert_eq!(
                serde_json::to_string(&first).unwrap(),
                serde_json::to_string(&second).unwrap()
            );
        }

        #[test]
        fn survives_a_round_trip_through_the_stored_json() {
            let base = state(0.0);
            let mut branch = branch_on(&base);
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();
            let stored = serde_json::to_string(&branch).unwrap();

            let reloaded: Branch = serde_json::from_str(&stored).unwrap();

            assert_eq!(reloaded.materialize(&base).unwrap().cuts.len(), 1);
        }
    }

    mod truncate_after {
        use super::*;

        fn two_entry_branch() -> Branch {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();
            branch.append("k2", vec![cut(3.0, 4.0)], None, NOW).unwrap();
            branch
        }

        #[test]
        fn drops_the_entries_past_the_given_seq() {
            let mut branch = two_entry_branch();

            branch.truncate_after(1, NOW);

            assert_eq!(branch.entries.len(), 1);
        }

        #[test]
        fn lets_the_next_append_reuse_the_freed_seq() {
            let mut branch = two_entry_branch();

            branch.truncate_after(1, NOW);

            assert_eq!(branch.next_seq(), 2);
        }
    }

    mod compact {
        use super::*;

        fn two_op_branch() -> Branch {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();
            branch.append("k2", vec![cut(3.0, 4.0)], None, NOW).unwrap();
            branch
        }

        #[test]
        fn leaves_one_entry_behind() {
            let mut branch = two_op_branch();

            branch.compact(&state(0.0), NOW).unwrap();

            assert_eq!(branch.entries.len(), 1);
        }

        #[test]
        fn keeps_the_fork_point_so_replay_still_matches_the_project() {
            let mut branch = two_op_branch();
            let base = branch.base;

            branch.compact(&state(0.0), NOW).unwrap();

            assert_eq!(branch.base, base);
        }

        #[test]
        fn materializes_to_the_state_it_folded() {
            let mut branch = two_op_branch();
            let folded = branch.materialize(&state(0.0)).unwrap();

            branch.compact(&state(0.0), NOW).unwrap();

            assert_eq!(
                serde_json::to_string(&branch.materialize(&state(0.0)).unwrap()).unwrap(),
                serde_json::to_string(&folded).unwrap()
            );
        }

        #[test]
        fn keeps_seq_moving_forward() {
            let mut branch = two_op_branch();

            branch.compact(&state(0.0), NOW).unwrap();

            assert_eq!(branch.next_seq(), 4);
        }

        #[test]
        fn refuses_to_compact_against_a_base_that_moved() {
            let mut branch = two_op_branch();

            let error = branch.compact(&state(5.0), NOW).unwrap_err();

            assert!(
                matches!(error, JournalError::BaseMoved { .. }),
                "got: {error}"
            );
        }
    }

    mod sweep {
        use super::*;

        const DAY_MS: i64 = 24 * 60 * 60 * 1000;

        fn store_with(branches: &[Branch]) -> BranchStore {
            let store = temp_store();
            for branch in branches {
                store.save(branch).unwrap();
            }
            store
        }

        fn empty_branch(id: &str, created_at_ms: i64) -> Branch {
            Branch::new(
                branch_id(id),
                StateHash::of(&state(0.0)).unwrap(),
                "agent:test",
                None,
                created_at_ms,
            )
        }

        fn working_branch(id: &str, created_at_ms: i64) -> Branch {
            let mut branch = empty_branch(id, created_at_ms);
            branch
                .append("k1", vec![cut(1.0, 2.0)], None, created_at_ms)
                .unwrap();
            branch
        }

        #[test]
        fn discards_an_empty_branch_a_crashed_agent_left_behind() {
            let store = store_with(&[empty_branch("abandoned", NOW)]);

            let removed = store.sweep(NOW + 2 * DAY_MS);

            assert_eq!(removed, vec![branch_id("abandoned")]);
        }

        #[test]
        fn keeps_a_fresh_empty_branch_an_agent_is_still_filling() {
            let store = store_with(&[empty_branch("in-progress", NOW)]);

            store.sweep(NOW + 1000);

            assert_eq!(store.list(), vec![branch_id("in-progress")]);
        }

        /// Proposed edits are pending human review; age is not consent to delete.
        #[test]
        fn never_discards_a_branch_carrying_ops_however_old() {
            let store = store_with(&[working_branch("proposed", NOW)]);

            store.sweep(NOW + 365 * DAY_MS);

            assert_eq!(store.list(), vec![branch_id("proposed")]);
        }

        #[test]
        fn leaves_an_unreadable_journal_alone() {
            let store = store_with(&[]);
            std::fs::create_dir_all(&store.dir).unwrap();
            std::fs::write(store.path_for(&branch_id("broken")), b"{ not json").unwrap();

            store.sweep(NOW + 365 * DAY_MS);

            assert!(store.path_for(&branch_id("broken")).exists());
        }

        #[test]
        fn reports_nothing_when_there_is_nothing_to_discard() {
            let store = store_with(&[working_branch("proposed", NOW)]);

            assert!(store.sweep(NOW + 365 * DAY_MS).is_empty());
        }
    }

    mod staleness {
        use super::*;

        #[test]
        fn a_branch_touched_today_is_not_stale() {
            let branch = branch_on(&state(0.0));

            assert!(!branch.is_stale(NOW));
        }

        #[test]
        fn a_branch_untouched_past_the_window_is_stale() {
            let branch = branch_on(&state(0.0));

            assert!(branch.is_stale(NOW + STALE_AFTER_MS + 1));
        }

        #[test]
        fn appending_clears_staleness() {
            let mut branch = branch_on(&state(0.0));
            let later = NOW + STALE_AFTER_MS + 1;

            branch
                .append("k1", vec![cut(1.0, 2.0)], None, later)
                .unwrap();

            assert!(!branch.is_stale(later));
        }

        #[test]
        fn a_branch_with_no_entries_reports_empty() {
            assert!(branch_on(&state(0.0)).is_empty());
        }

        #[test]
        fn a_branch_with_an_entry_does_not() {
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            assert!(!branch.is_empty());
        }
    }

    mod store {
        use super::*;

        #[test]
        fn saves_and_reloads_a_branch() {
            let store = temp_store();
            let mut branch = branch_on(&state(0.0));
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();
            store.save(&branch).unwrap();

            let reloaded = store.load(&branch_id("agent-1")).unwrap();

            assert_eq!(reloaded.entries.len(), 1);
        }

        #[test]
        fn a_second_save_replaces_the_first() {
            let store = temp_store();
            let mut branch = branch_on(&state(0.0));
            store.save(&branch).unwrap();
            branch.append("k1", vec![cut(1.0, 2.0)], None, NOW).unwrap();

            store.save(&branch).unwrap();

            assert_eq!(store.load(&branch_id("agent-1")).unwrap().entries.len(), 1);
        }

        #[test]
        fn reports_a_branch_that_was_never_saved() {
            let store = temp_store();

            let error = store.load(&branch_id("missing")).unwrap_err();

            assert!(
                matches!(error, JournalError::NoSuchBranch(_)),
                "got: {error}"
            );
        }

        #[test]
        fn lists_saved_branches() {
            let store = temp_store();
            store.save(&branch_on(&state(0.0))).unwrap();

            assert_eq!(store.list(), vec![branch_id("agent-1")]);
        }

        #[test]
        fn lists_nothing_when_the_directory_is_absent() {
            let store = temp_store();

            assert!(store.list().is_empty());
        }

        #[test]
        fn skips_a_corrupt_journal_when_loading_the_rest() {
            let store = temp_store();
            store.save(&branch_on(&state(0.0))).unwrap();
            std::fs::write(store.path_for(&branch_id("broken")), b"{ not json").unwrap();

            assert!(store.list().contains(&branch_id("agent-1")));
        }

        #[test]
        fn surfaces_a_corrupt_journal_when_it_is_the_one_asked_for() {
            let store = temp_store();
            store.save(&branch_on(&state(0.0))).unwrap();
            std::fs::write(store.path_for(&branch_id("broken")), b"{ not json").unwrap();

            let error = store.load(&branch_id("broken")).unwrap_err();

            assert!(
                matches!(error, JournalError::Corrupt { .. }),
                "got: {error}"
            );
        }

        #[test]
        fn removing_a_branch_twice_succeeds() {
            let store = temp_store();
            store.save(&branch_on(&state(0.0))).unwrap();
            store.remove(&branch_id("agent-1")).unwrap();

            assert!(store.remove(&branch_id("agent-1")).is_ok());
        }

        #[test]
        fn a_removed_branch_no_longer_lists() {
            let store = temp_store();
            store.save(&branch_on(&state(0.0))).unwrap();

            store.remove(&branch_id("agent-1")).unwrap();

            assert!(store.list().is_empty());
        }
    }

    mod diff {
        use super::*;

        fn changed(before: &RenderState, after: &RenderState) -> Vec<String> {
            super::diff(before, after)
                .unwrap()
                .into_iter()
                .map(|change| change.field)
                .collect()
        }

        #[test]
        fn reports_nothing_for_identical_states() {
            assert!(changed(&state(1.0), &state(1.0)).is_empty());
        }

        #[test]
        fn names_a_changed_scalar_by_its_dotted_path() {
            assert_eq!(changed(&state(1.0), &state(2.0)), vec!["trimStart"]);
        }

        #[test]
        fn reports_an_added_row_as_one_change_at_the_row_path() {
            let base = state(0.0);
            let mut after = base.clone();
            crate::render::ops::apply_op(&mut after, &cut(1.0, 2.0)).unwrap();

            assert_eq!(changed(&base, &after), vec!["cuts.0"]);
        }

        #[test]
        fn descends_into_a_row_that_was_edited_in_place() {
            let mut base = state(0.0);
            crate::render::ops::apply_op(&mut base, &cut(1.0, 2.0)).unwrap();
            let mut after = base.clone();
            after.cuts[0].end = 9.0;

            assert_eq!(changed(&base, &after), vec!["cuts.0.end"]);
        }

        #[test]
        fn carries_both_sides_of_a_scalar_change() {
            let change = super::diff(&state(1.0), &state(2.0)).unwrap().remove(0);

            assert_eq!(
                (change.before, change.after),
                (Some(serde_json::json!(1.0)), Some(serde_json::json!(2.0)))
            );
        }

        #[test]
        fn leaves_before_empty_for_an_added_row() {
            let base = state(0.0);
            let mut after = base.clone();
            crate::render::ops::apply_op(&mut after, &cut(1.0, 2.0)).unwrap();

            let added = super::diff(&base, &after)
                .unwrap()
                .into_iter()
                .find(|change| change.field == "cuts.0")
                .unwrap();

            assert_eq!(added.before, None);
        }
    }

    mod project_key {
        use super::*;

        #[test]
        fn keeps_the_stem_readable() {
            assert!(super::project_key(Path::new("/tmp/demo.recast")).starts_with("demo-"));
        }

        #[test]
        fn separates_same_named_projects_in_different_folders() {
            assert_ne!(
                super::project_key(Path::new("/a/demo.recast")),
                super::project_key(Path::new("/b/demo.recast"))
            );
        }

        #[test]
        fn is_stable_across_calls() {
            assert_eq!(
                super::project_key(Path::new("/a/demo.recast")),
                super::project_key(Path::new("/a/demo.recast"))
            );
        }
    }
}

/// The branch cycle against a real `.recast` on disk.
///
/// The unit tests above prove the journal folds correctly in memory. These prove
/// the part that only shows up on disk: that a branch saved, reloaded and
/// materialized against a freshly-opened project still lands the right edits in
/// the bundle, and that its fork point stops matching once it has.
#[cfg(test)]
mod disk_roundtrip_tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    use serde_json::json;

    use super::*;
    use crate::project::reader::open_project;
    use crate::project::writer::{self, ProjectWriteRequest};
    use crate::project::ProjectMetadata;
    use crate::render::ops::Op;

    static COUNTER: AtomicU32 = AtomicU32::new(0);
    const NOW: i64 = 1_700_000_000_000;

    fn workspace() -> PathBuf {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("recast-branch-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("workspace");
        dir
    }

    fn fixture_metadata() -> ProjectMetadata {
        serde_json::from_value(json!({
            "schemaVersion": 1,
            "createdAtUnixMs": 1_700_000_000_000u64,
            "captureTarget": {
                "kind": "display",
                "id": 1,
                "label": "Display 1",
                "source": { "x": 0, "y": 0, "width": 1920, "height": 1080 },
                "crop": { "x": 0, "y": 0, "width": 1920, "height": 1080 },
                "displayId": 1,
                "scaleFactor": 1.0
            },
            "stats": {
                "capturedFrames": 600, "encodedFrames": 600, "droppedFrames": 0,
                "durationMs": 10000, "nominalFps": 60
            },
            "video": { "width": 1920, "height": 1080, "fps": 60, "durationMs": 10000 }
        }))
        .expect("fixture metadata")
    }

    /// A v2 bundle whose edits are `RenderState::default()` trimmed to 10s.
    fn write_fixture_project(ws: &Path) -> PathBuf {
        let recording = ws.join("rec.mp4");
        let cursor = ws.join("cursor.json");
        fs::write(&recording, b"video-bytes").expect("recording");
        fs::write(&cursor, br#"{"samples":[]}"#).expect("cursor");

        let state = RenderState {
            trim_end: 10.0,
            ..RenderState::default()
        };
        let out = ws.join("project.recast");
        writer::write_project(ProjectWriteRequest {
            output_path: out.clone(),
            metadata: fixture_metadata(),
            recording_path: recording,
            cursor_path: cursor,
            audio_path: None,
            microphone_path: None,
            camera_path: None,
            edits_json: serde_json::to_string(&state).expect("serialize"),
        })
        .expect("write v2");
        out
    }

    fn read_state(project: &Path) -> RenderState {
        let opened = open_project(project).expect("open");
        serde_json::from_str(&fs::read_to_string(&opened.edits_path).expect("edits"))
            .expect("parse edits")
    }

    fn save_edits(project: &Path, state: &RenderState) {
        writer::update_project_edits(project, &serde_json::to_string(state).expect("serialize"))
            .expect("save edits");
    }

    /// Create a branch on the project's current state and record two ops.
    fn proposed_branch(store: &BranchStore, project: &Path) -> Branch {
        let base = StateHash::of(&read_state(project)).expect("hash");
        let mut branch = Branch::new(
            BranchId::new("agent-1").expect("id"),
            base,
            "agent:test",
            Some("tighten the intro".into()),
            NOW,
        );
        branch
            .append(
                "k1",
                vec![Op::Trim {
                    start: 1.0,
                    end: 9.0,
                }],
                None,
                NOW,
            )
            .expect("trim");
        branch
            .append(
                "k2",
                vec![Op::CutAdd {
                    start: 3.0,
                    end: 4.0,
                }],
                None,
                NOW,
            )
            .expect("cut");
        store.save(&branch).expect("save branch");
        branch
    }

    /// Reload from disk on both sides, then apply, as `branch.apply` does.
    fn apply_from_disk(store: &BranchStore, project: &Path) -> RenderState {
        let branch = store
            .load(&BranchId::new("agent-1").expect("id"))
            .expect("load");
        let applied = branch
            .materialize(&read_state(project))
            .expect("materialize");
        save_edits(project, &applied);
        applied
    }

    #[test]
    fn a_branch_written_to_disk_reloads_with_its_ops() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);

        let reloaded = store
            .load(&BranchId::new("agent-1").expect("id"))
            .expect("load");

        assert_eq!(reloaded.op_count(), 2);
    }

    #[test]
    fn applying_a_branch_writes_the_edits_into_the_bundle() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);

        apply_from_disk(&store, &project);

        assert_eq!(read_state(&project).cuts.len(), 1);
    }

    #[test]
    fn applying_a_branch_carries_every_op_not_just_the_last() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);

        apply_from_disk(&store, &project);

        assert_eq!(read_state(&project).trim_start, 1.0);
    }

    #[test]
    fn the_applied_state_passes_the_validator() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);

        let applied = apply_from_disk(&store, &project);

        assert!(
            crate::commands::validate_render_state(&applied, 10.0).is_ok(),
            "{:?}",
            crate::commands::validate_render_state(&applied, 10.0)
        );
    }

    /// Fast-forward only: once the project has moved, the branch cannot land again.
    #[test]
    fn a_branch_cannot_be_applied_twice() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);
        apply_from_disk(&store, &project);

        let branch = store
            .load(&BranchId::new("agent-1").expect("id"))
            .expect("load");
        let second = branch.materialize(&read_state(&project));

        assert!(
            matches!(second, Err(JournalError::BaseMoved { .. })),
            "got: {second:?}"
        );
    }

    /// The whole reason the journal exists: proposing costs no bundle rewrite.
    #[test]
    fn proposing_edits_leaves_the_bundle_untouched() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let before = fs::read(&project).expect("read bundle");
        let store = BranchStore::new(ws.join("branches"));

        proposed_branch(&store, &project);

        assert_eq!(fs::read(&project).expect("read bundle"), before);
    }

    #[test]
    fn an_edit_landing_between_fork_and_apply_is_caught() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);

        // Someone saves in the GUI while the branch is waiting for review.
        let mut moved = read_state(&project);
        moved.trim_start = 5.0;
        save_edits(&project, &moved);

        let branch = store
            .load(&BranchId::new("agent-1").expect("id"))
            .expect("load");

        assert!(
            matches!(
                branch.materialize(&read_state(&project)),
                Err(JournalError::BaseMoved { .. })
            ),
            "a moved base must be rejected"
        );
    }

    #[test]
    fn discarding_a_branch_leaves_the_project_as_it_was() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        proposed_branch(&store, &project);

        store
            .remove(&BranchId::new("agent-1").expect("id"))
            .expect("discard");

        assert_eq!(read_state(&project).trim_start, 0.0);
    }

    #[test]
    fn the_diff_matches_what_applying_actually_changes() {
        let ws = workspace();
        let project = write_fixture_project(&ws);
        let store = BranchStore::new(ws.join("branches"));
        let branch = proposed_branch(&store, &project);
        let base = read_state(&project);
        let proposed = branch.materialize(&base).expect("materialize");
        let predicted: Vec<String> = super::diff(&base, &proposed)
            .expect("diff")
            .into_iter()
            .map(|change| change.field)
            .collect();

        let applied = apply_from_disk(&store, &project);
        let actual: Vec<String> = super::diff(&base, &applied)
            .expect("diff")
            .into_iter()
            .map(|change| change.field)
            .collect();

        assert_eq!(predicted, actual);
    }
}

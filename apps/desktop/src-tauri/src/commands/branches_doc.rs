//! Branch operations on a v3 folder project: journal v2, document ops on a `DocHash`, kept in the project's `branches/`.
//! The harness still speaks render states for receipts, `check` and the intents; the document is what is journaled and landed.

use std::path::{Path, PathBuf};

use recast_project::scene::{from_render_state_public, to_render_state, MediaRefs};
use recast_project::store::Expect;
use recast_project::{diff, DocHash, Document, IdGen, Op};
use sha2::{Digest, Sha256};

use super::branches::{ApplyReport, BranchSummary};
use super::editor_session::now_ms;
use super::error::{AppError, AppResult};
use crate::agent::check::ProjectFacts;
use crate::agent::guard;
use crate::agent::intents::{self, SilencePolicy, ZoomIntent};
use crate::agent::receipt::{receipt_with, Receipt};
use crate::project::documents::{documents, Outcome};
use crate::project::journal::{BranchId, FieldChange, JournalError};
use crate::project::journal_doc::{doc_store, DocBranch, DocBranchStore};
use crate::render::graph::RenderState;

/// The folder's live document plus what the harness needs to know about its media.
pub struct Folder {
    pub path: PathBuf,
    pub doc: Document,
    pub render: RenderState,
    pub facts: ProjectFacts,
    pub audio_path: Option<String>,
    pub microphone_path: Option<String>,
    pub cursor_path: Option<String>,
}

impl Folder {
    /// The owner's copy of the document, never the file: while the app runs the file is a checkpoint behind it.
    pub fn open(project: &str) -> AppResult<Self> {
        let doc = documents()
            .document(Path::new(project))
            .map_err(AppError::msg)?;
        let render = to_render_state(&doc).map_err(AppError::msg)?;
        let loaded = tauri::async_runtime::block_on(super::load_document(project.to_string()))?;
        Ok(Self {
            path: PathBuf::from(project),
            doc,
            render,
            facts: ProjectFacts::of(&loaded),
            audio_path: loaded.audio_path,
            microphone_path: loaded.microphone_path,
            cursor_path: loaded.cursor_path,
        })
    }

    pub fn hash(&self) -> DocHash {
        DocHash::of(&self.doc)
    }
}

/// Render-state ops (what the intents produce) as document ops against `doc`: apply them to the document's own
/// state, write that state back onto the document, and diff. One mapping, in Rust, both ways.
pub fn doc_ops_for(doc: &Document, ops: &[crate::render::ops::Op]) -> AppResult<Vec<Op>> {
    let mut state = to_render_state(doc).map_err(AppError::msg)?;
    crate::render::ops::apply_ops(&mut state, ops).map_err(|e| AppError::msg(e.to_string()))?;
    doc_ops_for_state(doc, &state)
}

/// The document ops that take `doc` to what `state` describes.
pub fn doc_ops_for_state(doc: &Document, state: &RenderState) -> AppResult<Vec<Op>> {
    let json = serde_json::to_string(state).map_err(|e| AppError::msg(e.to_string()))?;
    let digest = Sha256::digest(json.as_bytes());
    let mut ids = IdGen::seeded(u64::from_le_bytes(digest[..8].try_into().unwrap_or([0; 8])));
    let refs = MediaRefs::from_document(doc);
    let target = from_render_state_public(state, &refs, Some(doc), &mut ids);
    diff(doc, &target).map_err(|e| AppError::msg(e.to_string()))
}

/// Stateless: the write lock and the activity record are the caller's (`BranchService`), so every operation here
/// runs against a temp folder in a test with nothing but the document owner.
pub struct DocBranches;

impl DocBranches {
    fn store(&self, project: &str) -> DocBranchStore {
        doc_store(Path::new(project))
    }

    pub fn create(
        &self,
        project: &str,
        id: BranchId,
        author: String,
        label: Option<String>,
    ) -> AppResult<BranchSummary> {
        let folder = Folder::open(project)?;
        let branch = DocBranch::new(id, folder.hash(), author, label, now_ms());
        self.store(project).create(&branch).map_err(AppError::msg)?;
        Ok(summary(&branch, now_ms()))
    }

    pub fn list(&self, project: &str) -> Vec<BranchSummary> {
        let store = self.store(project);
        let now = now_ms();
        store.sweep(now);
        store
            .list()
            .iter()
            .filter_map(|id| store.load(id).ok())
            .map(|b| summary(&b, now))
            .collect()
    }

    /// Records document ops as one entry, replaying and validating first so a bad proposal is refused where its author can fix it.
    pub fn append(
        &self,
        project: &str,
        id: &BranchId,
        idem_key: String,
        ops: Vec<Op>,
        expect_seq: Option<u64>,
        expect_base: Option<&str>,
    ) -> AppResult<Receipt> {
        let folder = Folder::open(project)?;
        self.append_to(&folder, id, idem_key, ops, expect_seq, expect_base)
    }

    pub fn remove_silences(
        &self,
        project: &str,
        id: &BranchId,
        idem_key: String,
        policy: SilencePolicy,
        expect_base: Option<&str>,
    ) -> AppResult<Receipt> {
        let folder = Folder::open(project)?;
        let silences = crate::silence::detect_blocking(
            folder.audio_path.as_deref(),
            folder.microphone_path.as_deref(),
            folder.cursor_path.as_deref(),
            Default::default(),
        )
        .map_err(AppError::msg)?;
        let (branch_doc, _) = self.head(&folder, id)?;
        let head_state = to_render_state(&branch_doc).map_err(AppError::msg)?;
        let render_ops = intents::cuts_for_silences(&head_state, &silences, policy);
        if render_ops.is_empty() {
            return Err(AppError::msg(format!(
                "no silence meets the policy (min {:.2}s after {:.2}s padding, confidence >= {:.2}) out of {} detected; nothing appended",
                policy.min_duration, policy.pad, policy.min_confidence, silences.len()
            )));
        }
        let ops = doc_ops_for(&branch_doc, &render_ops)?;
        self.append_to(&folder, id, idem_key, ops, None, expect_base)
    }

    pub fn add_zoom(
        &self,
        project: &str,
        id: &BranchId,
        idem_key: String,
        intent: &ZoomIntent,
        expect_base: Option<&str>,
    ) -> AppResult<Receipt> {
        let folder = Folder::open(project)?;
        let (branch_doc, _) = self.head(&folder, id)?;
        let head_state = to_render_state(&branch_doc).map_err(AppError::msg)?;
        let map = crate::agent::axis::time_map(&head_state);
        let op = intents::zoom_op(&map, intent).map_err(AppError::msg)?;
        let ops = doc_ops_for(&branch_doc, &[op])?;
        self.append_to(&folder, id, idem_key, ops, None, expect_base)
    }

    /// The branch's document as it stands (its ops on its fork point) and the loaded branch.
    fn head(&self, folder: &Folder, id: &BranchId) -> AppResult<(Document, DocBranch)> {
        let branch = self
            .store(&folder.path.to_string_lossy())
            .load(id)
            .map_err(AppError::msg)?;
        let doc = branch.materialize(&folder.doc).map_err(AppError::msg)?;
        Ok((doc, branch))
    }

    fn append_to(
        &self,
        folder: &Folder,
        id: &BranchId,
        idem_key: String,
        ops: Vec<Op>,
        expect_seq: Option<u64>,
        expect_base: Option<&str>,
    ) -> AppResult<Receipt> {
        guard::check_batch_size(ops.len()).map_err(AppError::msg)?;
        let current = folder.hash();
        if let Some(expected) = expect_base.filter(|e| *e != current.to_string()) {
            return Err(AppError::msg(format!(
                "stale: the project is at {current}, you expected {expected}; read again and retry"
            )));
        }
        let project = folder.path.to_string_lossy().into_owned();
        let store = self.store(&project);
        let mut branch = store.load(id).map_err(AppError::msg)?;
        let before_doc = branch.materialize(&folder.doc).map_err(AppError::msg)?;
        let outcome = branch
            .append(idem_key, ops, expect_seq, now_ms())
            .map_err(AppError::msg)?;
        let proposed_doc = branch.materialize(&folder.doc).map_err(AppError::msg)?;
        let before = to_render_state(&before_doc).map_err(AppError::msg)?;
        let proposed = to_render_state(&proposed_doc).map_err(AppError::msg)?;
        if outcome.is_recorded() {
            if let Err(issues) =
                super::validate_render_state(&proposed, folder.facts.source_duration)
            {
                return Err(AppError::msg(format!(
                    "branch '{id}' rejected: {}",
                    serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}"))
                )));
            }
            let report = recast_project::validate(&proposed_doc);
            let introduced: Vec<String> = report
                .errors()
                .filter(|i| {
                    !recast_project::validate(&before_doc)
                        .errors()
                        .any(|b| b.code == i.code && b.id == i.id)
                })
                .map(|i| i.message.clone())
                .collect();
            if !introduced.is_empty() {
                return Err(AppError::msg(format!(
                    "branch '{id}' rejected: {}",
                    introduced.join("; ")
                )));
            }
        }
        let compacted = branch.needs_compaction();
        if compacted {
            branch
                .compact(&folder.doc, now_ms())
                .map_err(AppError::msg)?;
        }
        store.save(&branch).map_err(AppError::msg)?;
        receipt_with(
            &before,
            &proposed,
            &branch.base.to_string(),
            &DocHash::of(&proposed_doc).to_string(),
            &folder.facts,
            outcome.seq(),
            outcome.is_recorded(),
            compacted,
        )
        .map_err(AppError::msg)
    }

    pub fn truncate(&self, project: &str, id: &BranchId, seq: u64) -> AppResult<BranchSummary> {
        let store = self.store(project);
        let mut branch = store.load(id).map_err(AppError::msg)?;
        let now = now_ms();
        branch.truncate_after(seq, now);
        store.save(&branch).map_err(AppError::msg)?;
        Ok(summary(&branch, now))
    }

    /// The render state the branch would produce, for the review panel's preview.
    pub fn materialize(&self, project: &str, id: &BranchId) -> AppResult<RenderState> {
        let folder = Folder::open(project)?;
        let (doc, _) = self.head(&folder, id)?;
        to_render_state(&doc).map_err(AppError::msg)
    }

    pub fn diff(&self, project: &str, id: &BranchId) -> AppResult<Vec<FieldChange>> {
        let folder = Folder::open(project)?;
        let proposed = self.materialize(project, id)?;
        crate::project::journal::diff(&folder.render, &proposed).map_err(AppError::msg)
    }

    pub fn discard(&self, project: &str, id: &BranchId) -> AppResult<()> {
        self.store(project).remove(id).map_err(AppError::msg)
    }

    /// Lands the branch on the live document. Fast-forward when the fork point still stands; otherwise the id-level
    /// rebase replays the ops onto the moved document and refuses only when an op no longer finds its target.
    pub fn apply(&self, project: &str, id: &BranchId) -> AppResult<ApplyReport> {
        let folder = Folder::open(project)?;
        let store = self.store(project);
        let branch = store.load(id).map_err(AppError::msg)?;
        let ops = branch.all_ops();
        let landed = match branch.materialize(&folder.doc) {
            Ok(_) => documents()
                .apply(
                    &folder.path,
                    &ops,
                    Expect {
                        seq: None,
                        hash: Some(folder.hash()),
                    },
                )
                .map_err(AppError::msg)?,
            Err(JournalError::DocBaseMoved { .. }) => {
                branch.replay_onto(&folder.doc).map_err(AppError::msg)?;
                documents()
                    .apply(&folder.path, &ops, Expect::default())
                    .map_err(AppError::msg)?
            }
            Err(e) => return Err(AppError::msg(e.to_string())),
        };
        let Outcome::Applied { .. } = landed else {
            return Err(AppError::msg(
                "the document moved while the branch was landing; retry",
            ));
        };
        let after = documents().document(&folder.path).map_err(AppError::msg)?;
        let changes = crate::project::journal::diff(
            &folder.render,
            &to_render_state(&after).map_err(AppError::msg)?,
        )
        .map_err(AppError::msg)?
        .len();
        store.remove(id).map_err(AppError::msg)?;
        Ok(ApplyReport { changes })
    }
}

fn summary(branch: &DocBranch, now: i64) -> BranchSummary {
    BranchSummary {
        id: branch.id.clone(),
        author: branch.author.clone(),
        label: branch.label.clone(),
        base: branch.base.to_string(),
        seq: branch.next_seq() - 1,
        ops: branch.op_count(),
        created_at_ms: branch.created_at_ms,
        updated_at_ms: branch.updated_at_ms,
        stale: branch.is_stale(now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::v3::write_project;
    use crate::project::v3::ProjectWriteRequest;
    use crate::project::ProjectMetadata;
    use recast_project::layout;

    fn folder_project(tmp: &Path) -> String {
        let staged = tmp.join("staged");
        std::fs::create_dir_all(&staged).unwrap();
        let rec = staged.join("x.recording.mp4");
        let cur = staged.join("x.cursor.json");
        std::fs::write(&rec, b"MP4").unwrap();
        std::fs::write(&cur, r#"{"samples":[],"clicks":[]}"#).unwrap();
        let metadata: ProjectMetadata = serde_json::from_str(
            r#"{"schemaVersion":1,"createdAtUnixMs":1,"captureTarget":{"kind":"display","id":1,"label":"x","source":{"x":0,"y":0,"width":1920,"height":1080},"crop":{"x":0,"y":0,"width":1920,"height":1080},"displayId":1,"scaleFactor":1},"stats":{"capturedFrames":600,"encodedFrames":600,"droppedFrames":0,"durationMs":10000,"nominalFps":60},"video":{"width":1920,"height":1080,"fps":60,"durationMs":10000}}"#,
        )
        .unwrap();
        let state = RenderState {
            trim_end: 10.0,
            ..RenderState::default()
        };
        let out = tmp.join("Branchy.recast");
        write_project(ProjectWriteRequest {
            output_path: out.clone(),
            metadata,
            recording_path: rec,
            cursor_path: cur,
            audio_path: None,
            microphone_path: None,
            camera_path: None,
            edits_json: serde_json::to_string(&state).unwrap(),
        })
        .unwrap();
        out.to_string_lossy().into_owned()
    }

    fn set_out(v: &str) -> Op {
        Op::Set {
            id: "/timeline".into(),
            attr: "out".into(),
            value: Some(v.into()),
        }
    }

    #[test]
    fn a_folder_branch_journals_document_ops_in_the_project_and_lands_them_on_the_live_document() {
        let tmp = tempfile::tempdir().unwrap();
        let project = folder_project(tmp.path());
        let svc = DocBranches;
        let id = BranchId::new("b1").unwrap();
        let created = svc
            .create(
                &project,
                id.clone(),
                "agent:test".into(),
                Some("trim".into()),
            )
            .unwrap();
        assert_eq!(created.base.len(), 16, "a document hash, not a state hash");
        assert!(Path::new(&project)
            .join(layout::BRANCHES_DIR)
            .join("b1.json")
            .is_file());

        let receipt = svc
            .append(
                &project,
                &id,
                "k1".into(),
                vec![set_out("8.000")],
                None,
                Some(&created.base),
            )
            .unwrap();
        assert!(receipt.recorded);
        assert!(
            receipt.changes.iter().any(|c| c.field.contains("trimEnd")),
            "{:?}",
            receipt.changes
        );
        assert_eq!(svc.materialize(&project, &id).unwrap().trim_end, 8.0);
        assert!(!svc.diff(&project, &id).unwrap().is_empty());

        // An intent produces render-state ops; on a folder they become document ops on the branch.
        let zoom = svc
            .add_zoom(
                &project,
                &id,
                "k2".into(),
                &ZoomIntent {
                    id: "z1".into(),
                    at: 2.0,
                    duration: 3.0,
                    center_x: 0.5,
                    center_y: 0.5,
                    scale: 1.8,
                    ramp: 0.5,
                },
                None,
            )
            .unwrap();
        assert!(zoom.recorded);
        assert_eq!(
            svc.materialize(&project, &id).unwrap().zoom_regions.len(),
            1
        );

        let stale = svc.append(
            &project,
            &id,
            "k3".into(),
            vec![set_out("7.000")],
            None,
            Some("0000000000000000"),
        );
        assert!(stale.unwrap_err().to_string().contains("stale"));

        let report = svc.apply(&project, &id).unwrap();
        assert!(report.changes >= 2, "{report:?}");
        let after = documents().document(Path::new(&project)).unwrap();
        assert_eq!(
            after.root.child("timeline").unwrap().attr("out"),
            Some("8.000")
        );
        assert!(after.find("z1").is_some(), "the zoom kept the intent's id");
        assert!(svc.list(&project).is_empty(), "applied branches are gone");
    }

    #[test]
    fn a_branch_whose_fork_point_moved_rebases_by_id_and_refuses_only_a_lost_target() {
        let tmp = tempfile::tempdir().unwrap();
        let project = folder_project(tmp.path());
        let svc = DocBranches;
        let id = BranchId::new("b2").unwrap();
        svc.create(&project, id.clone(), "agent:test".into(), None)
            .unwrap();
        svc.append(
            &project,
            &id,
            "k1".into(),
            vec![set_out("8.000")],
            None,
            None,
        )
        .unwrap();
        // Someone else edits the live document underneath the branch.
        documents()
            .apply(
                Path::new(&project),
                &[Op::Set {
                    id: "/".into(),
                    attr: "pad".into(),
                    value: Some("12".into()),
                }],
                Expect::default(),
            )
            .unwrap();
        assert!(svc
            .materialize(&project, &id)
            .unwrap_err()
            .to_string()
            .contains("forked from document"));
        svc.apply(&project, &id).unwrap();
        let after = documents().document(Path::new(&project)).unwrap();
        assert_eq!(after.root.attr("pad"), Some("12"));
        assert_eq!(
            after.root.child("timeline").unwrap().attr("out"),
            Some("8.000")
        );
    }
}

//! Journal v2: a branch of document ops forked from a `DocHash`, kept in the project's own `branches/`.
//! Replay is id-addressed, so a branch can land on a document that moved as long as every op still finds its target.

use std::path::Path;

use recast_project::{apply_all, diff, layout, DocHash, Document, Op};

use super::journal::{Branch, BranchStore, JournalError};

pub type DocBranch = Branch<Op, DocHash>;
pub type DocBranchStore = BranchStore<Op, DocHash>;

/// The store for a v3 project: `Name.recast/branches/`.
pub fn doc_store(project_dir: &Path) -> DocBranchStore {
    BranchStore::new(project_dir.join(layout::BRANCHES_DIR))
}

impl DocBranch {
    /// Every op in order; what `apply` lands and what a rebase replays.
    pub fn all_ops(&self) -> Vec<Op> {
        self.entries
            .iter()
            .flat_map(|e| e.ops.iter().cloned())
            .collect()
    }

    /// Folds the recorded ops onto the document the branch forked from.
    /// # Errors `DocBaseMoved` when `base` is not the fork point, `DocReplay` when an op no longer fits.
    pub fn materialize(&self, base: &Document) -> Result<Document, JournalError> {
        let actual = DocHash::of(base);
        if actual != self.base {
            return Err(JournalError::DocBaseMoved {
                expected: self.base.clone(),
                actual,
            });
        }
        self.replay_onto(base)
    }

    /// Folds the ops onto any document, the id-level rebase: the fork point is not checked, only that each op lands.
    /// # Errors `DocReplay` naming the entry whose op failed.
    pub fn replay_onto(&self, doc: &Document) -> Result<Document, JournalError> {
        let mut current = doc.clone();
        for entry in &self.entries {
            current =
                apply_all(&current, &entry.ops).map_err(|(index, e)| JournalError::DocReplay {
                    branch: self.id.clone(),
                    seq: entry.seq,
                    message: format!("op {index}: {e}"),
                })?;
        }
        Ok(current)
    }

    /// Collapses the entries into the one batch that reaches the same document, leaving the fork point alone.
    pub fn compact(&mut self, base: &Document, now_ms: i64) -> Result<(), JournalError> {
        let folded = self.materialize(base)?;
        let ops = diff(base, &folded).map_err(|e| JournalError::DocReplay {
            branch: self.id.clone(),
            seq: self.next_seq(),
            message: e.to_string(),
        })?;
        let seq = self.next_seq();
        self.entries = vec![super::journal::Entry {
            seq,
            idem_key: format!("compact:{seq}"),
            ops,
            at_ms: now_ms,
        }];
        self.updated_at_ms = now_ms;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::journal::BranchId;
    use recast_project::parse;

    fn doc() -> Document {
        parse("<recast v=\"3\"><timeline in=\"0\" out=\"60\"/><screen id=\"scr\"><zooms/></screen></recast>").unwrap()
    }

    fn set_out(v: &str) -> Op {
        Op::Set {
            id: "/timeline".into(),
            attr: "out".into(),
            value: Some(v.into()),
        }
    }

    #[test]
    fn a_branch_materialises_on_its_fork_point_and_rebases_by_id_when_it_moved() {
        let base = doc();
        let mut branch = DocBranch::new(
            BranchId::new("b1").unwrap(),
            DocHash::of(&base),
            "agent",
            None,
            0,
        );
        branch.append("k1", vec![set_out("50")], None, 1).unwrap();
        let folded = branch.materialize(&base).unwrap();
        assert_eq!(
            folded.root.child("timeline").unwrap().attr("out"),
            Some("50")
        );

        let moved = apply_all(
            &base,
            &[Op::Set {
                id: "/timeline".into(),
                attr: "in".into(),
                value: Some("2".into()),
            }],
        )
        .unwrap();
        assert!(matches!(
            branch.materialize(&moved),
            Err(JournalError::DocBaseMoved { .. })
        ));
        let rebased = branch.replay_onto(&moved).unwrap();
        let tl = rebased.root.child("timeline").unwrap();
        assert_eq!(
            (tl.attr("in"), tl.attr("out")),
            (Some("2"), Some("50")),
            "both edits survive"
        );

        let gone = apply_all(
            &base,
            &[Op::Remove {
                id: "/timeline".into(),
            }],
        )
        .unwrap();
        assert!(matches!(
            branch.replay_onto(&gone),
            Err(JournalError::DocReplay { seq: 1, .. })
        ));
    }

    #[test]
    fn compaction_keeps_the_document_and_the_fork_point() {
        let base = doc();
        let mut branch = DocBranch::new(
            BranchId::new("b2").unwrap(),
            DocHash::of(&base),
            "agent",
            None,
            0,
        );
        branch.append("k1", vec![set_out("50")], None, 1).unwrap();
        branch.append("k2", vec![set_out("40")], None, 2).unwrap();
        let before = branch.materialize(&base).unwrap();
        branch.compact(&base, 3).unwrap();
        assert_eq!(branch.entries.len(), 1);
        assert_eq!(branch.materialize(&base).unwrap(), before);
        assert_eq!(
            branch.all_ops().len(),
            1,
            "two sets of one attribute fold into one"
        );
    }

    #[test]
    fn the_store_lives_in_the_project_and_round_trips_document_ops() {
        let tmp = tempfile::tempdir().unwrap();
        let store = doc_store(tmp.path());
        let base = doc();
        let mut branch = DocBranch::new(
            BranchId::new("b3").unwrap(),
            DocHash::of(&base),
            "agent",
            Some("trim".into()),
            5,
        );
        branch.append("k1", vec![set_out("50")], None, 6).unwrap();
        store.create(&branch).unwrap();
        assert!(tmp
            .path()
            .join(layout::BRANCHES_DIR)
            .join("b3.json")
            .is_file());
        let loaded = store.load(&branch.id).unwrap();
        assert_eq!(loaded.base, branch.base);
        assert_eq!(loaded.all_ops(), branch.all_ops());
    }
}

//! The core's document owner: one open `Store` per v3 project, the sequencer every writer goes through, and the checkpoint debounce.
//! While the app runs this copy is the truth; `project.rcx` is a checkpoint at most `CHECKPOINT_DEBOUNCE` behind the last op.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use parking_lot::Mutex;
use recast_project::store::{Expect, Store, StoreError};
use recast_project::{DocHash, Document, Op};
use serde::Serialize;

pub const CHECKPOINT_DEBOUNCE: Duration = Duration::from_millis(500);

static DOCUMENTS: LazyLock<Documents> = LazyLock::new(Documents::default);

/// The process-wide owner. A global because every writer (IPC, control socket, CLI, MCP) must reach the same sequencer.
pub fn documents() -> &'static Documents {
    &DOCUMENTS
}

/// Told after every sequenced batch, whichever writer sent it: the GUI adapter, the socket, a branch apply.
pub type ChangeListener = Box<dyn Fn(&Path, u64, &DocHash) + Send + Sync>;

#[derive(Default)]
pub struct Documents {
    open: Mutex<HashMap<PathBuf, Arc<Mutex<Held>>>>,
    listener: Mutex<Option<ChangeListener>>,
}

struct Held {
    store: Store,
    /// Bumped per apply; a debounce task checkpoints only if nothing landed after it was scheduled.
    generation: u64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    pub text: String,
    pub hash: DocHash,
    pub seq: u64,
}

/// What an apply came back with: a structured conflict is an answer, not an error, so a writer can patch and retry.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "camelCase")]
pub enum Outcome {
    Applied {
        seq: u64,
        hash: DocHash,
    },
    Stale {
        seq: u64,
        hash: DocHash,
        #[serde(skip_serializing_if = "Option::is_none")]
        since: Option<Vec<Op>>,
    },
}

impl Documents {
    fn held(&self, root: &Path) -> Result<Arc<Mutex<Held>>, StoreError> {
        let key = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        if let Some(held) = self.open.lock().get(&key) {
            return Ok(Arc::clone(held));
        }
        let store = Store::open(root)?;
        let held = Arc::new(Mutex::new(Held {
            store,
            generation: 0,
        }));
        self.open
            .lock()
            .entry(key)
            .or_insert_with(|| Arc::clone(&held));
        Ok(held)
    }

    /// Runs `f` against the current document, opening the project (and replaying its WAL) on first touch.
    pub fn with<T>(&self, root: &Path, f: impl FnOnce(&Store) -> T) -> Result<T, StoreError> {
        let held = self.held(root)?;
        let guard = held.lock();
        Ok(f(&guard.store))
    }

    pub fn document(&self, root: &Path) -> Result<Document, StoreError> {
        self.with(root, |s| s.document().clone())
    }

    pub fn snapshot(&self, root: &Path) -> Result<Snapshot, StoreError> {
        self.with(root, |s| Snapshot {
            text: recast_project::serialize(s.document()),
            hash: s.hash(),
            seq: s.seq(),
        })
    }

    /// Installs the one listener (the app's event emitter). Replaces any earlier one.
    pub fn on_change(&self, listener: ChangeListener) {
        *self.listener.lock() = Some(listener);
    }

    /// Sequences one batch and schedules the checkpoint. `Stale` comes back as an outcome; every other failure is an error.
    pub fn apply(&self, root: &Path, ops: &[Op], expect: Expect) -> Result<Outcome, StoreError> {
        let held = self.held(root)?;
        let outcome = {
            let mut guard = held.lock();
            match guard.store.apply_expecting(ops, expect) {
                Ok(applied) => {
                    guard.generation += 1;
                    Outcome::Applied {
                        seq: applied.seq,
                        hash: applied.hash,
                    }
                }
                Err(StoreError::Stale { seq, hash, since }) => {
                    return Ok(Outcome::Stale { seq, hash, since })
                }
                Err(e) => return Err(e),
            }
        };
        schedule_checkpoint(held);
        if let Outcome::Applied { seq, hash } = &outcome {
            if let Some(listener) = self.listener.lock().as_ref() {
                listener(root, *seq, hash);
            }
        }
        Ok(outcome)
    }

    /// Checkpoints now. Idle, blur and exit call this so the file is exactly current the moment editing stops.
    pub fn flush(&self, root: &Path) -> Result<(), StoreError> {
        let held = self.held(root)?;
        let result = held.lock().store.checkpoint();
        result
    }

    /// Checkpoints every open project; the one call the exit path makes.
    pub fn flush_all(&self) {
        let open: Vec<Arc<Mutex<Held>>> = self.open.lock().values().cloned().collect();
        for held in open {
            if let Err(e) = held.lock().store.checkpoint() {
                log::error!("checkpoint on exit failed: {e}");
            }
        }
    }

    /// Flushes and forgets a project, so the next open re-reads the file (after an external rewrite, say).
    pub fn close(&self, root: &Path) -> Result<(), StoreError> {
        let key = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        let Some(held) = self.open.lock().remove(&key) else {
            return Ok(());
        };
        let result = held.lock().store.checkpoint();
        result
    }
}

fn schedule_checkpoint(held: Arc<Mutex<Held>>) {
    let scheduled_at = held.lock().generation;
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(CHECKPOINT_DEBOUNCE).await;
        let mut guard = held.lock();
        if guard.generation != scheduled_at {
            return;
        }
        if let Err(e) = guard.store.checkpoint() {
            log::error!("debounced checkpoint failed: {e}");
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use recast_project::layout;
    use recast_project::ops::NodeSpec;

    fn project(dir: &Path) -> Documents {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(layout::DOCUMENT),
            "<recast v=\"3\"><timeline in=\"0\" out=\"60\"/><screen id=\"scr\"><zooms/></screen></recast>\n",
        )
        .unwrap();
        Documents::default()
    }

    fn set_out(value: &str) -> Op {
        Op::Set {
            id: "/timeline".into(),
            attr: "out".into(),
            value: Some(value.into()),
        }
    }

    #[test]
    fn a_stale_write_is_an_outcome_carrying_the_missed_ops_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        let docs = project(&dir);
        let Outcome::Applied { seq, .. } = docs
            .apply(&dir, &[set_out("50")], Expect::default())
            .unwrap()
        else {
            panic!("first apply must land");
        };
        let outcome = docs
            .apply(
                &dir,
                &[set_out("40")],
                Expect {
                    seq: Some(seq - 1),
                    hash: None,
                },
            )
            .unwrap();
        let Outcome::Stale { seq: at, since, .. } = outcome else {
            panic!("expected Stale, got {outcome:?}");
        };
        assert_eq!(at, seq);
        assert_eq!(since.unwrap(), vec![set_out("50")]);
        assert_eq!(docs.snapshot(&dir).unwrap().seq, seq);
    }

    #[test]
    fn open_twice_is_one_store_and_close_writes_the_file() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        let docs = project(&dir);
        docs.apply(&dir, &[set_out("50")], Expect::default())
            .unwrap();
        assert_eq!(
            docs.snapshot(&dir).unwrap().seq,
            1,
            "the same store answers"
        );
        assert!(!std::fs::read_to_string(dir.join(layout::DOCUMENT))
            .unwrap()
            .contains("50.000"));
        docs.close(&dir).unwrap();
        assert!(std::fs::read_to_string(dir.join(layout::DOCUMENT))
            .unwrap()
            .contains("50.000"));
        assert_eq!(docs.snapshot(&dir).unwrap().seq, 1, "seq survives a reopen");
    }

    #[test]
    fn an_op_that_breaks_the_document_is_an_error_and_changes_nothing() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        let docs = project(&dir);
        // A second `scr` is a duplicate id, which the validator refuses; an unknown element would only warn.
        let bad = Op::Insert {
            parent: "/".into(),
            index: 0,
            node: NodeSpec {
                kind: "camera".into(),
                attrs: [("id".to_owned(), "scr".to_owned())].into(),
                text: None,
                children: vec![],
            },
        };
        assert!(docs.apply(&dir, &[bad], Expect::default()).is_err());
        assert_eq!(docs.snapshot(&dir).unwrap().seq, 0);
    }
}

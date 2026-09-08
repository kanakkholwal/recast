//! The project's write side: one in-memory document, a sequencer, a write-ahead log, and an atomic checkpoint.
//! Only `project.rcx` is ever mutable; the WAL under `.cache/` bridges a crash between checkpoints and is replayed on open.

use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Write};

use serde::{Deserialize, Serialize};

use crate::document::Document;
use crate::hash::DocHash;
use crate::layout::{locate, Layout, Located};
use crate::ops::{apply_all, Op, OpError};
use crate::parse::{parse, ParseError};
use crate::serialize::serialize;
use crate::validate::{validate, Report};

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Locate(#[from] crate::layout::LocateError),
    #[error("{0} is not a project directory")]
    NotADirectory(std::path::PathBuf),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error("op {index} could not apply: {source}")]
    Op {
        index: usize,
        #[source]
        source: OpError,
    },
    #[error("the ops would leave the document invalid: {0}")]
    Invalid(String),
    #[error("the document moved: it is at seq {seq} ({hash})")]
    Stale {
        seq: u64,
        hash: DocHash,
        /// The ops the writer missed, when its `seq` is still inside the ring; `None` means re-read.
        since: Option<Vec<Op>>,
    },
    #[error("WAL line {line} is corrupt: {message}")]
    Wal { line: usize, message: String },
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: io::Error,
    },
}

fn io_err(context: impl Into<String>) -> impl FnOnce(io::Error) -> StoreError {
    let context = context.into();
    move |source| StoreError::Io { context, source }
}

/// One WAL entry: the ops that took the document from `seq - 1` to `seq`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    pub seq: u64,
    pub ops: Vec<Op>,
}

/// How many recent batches stay in memory for `since`, so a writer a few edits behind gets ops instead of a re-read.
pub const RING: usize = 256;

/// What a writer believes the document is at; either half may be left open.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Expect {
    pub seq: Option<u64>,
    pub hash: Option<DocHash>,
}

/// What the checkpoint recorded, so `seq` stays monotonic across restarts.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Checkpoint {
    seq: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Applied {
    pub seq: u64,
    pub hash: DocHash,
}

pub struct Store {
    layout: Layout,
    doc: Document,
    seq: u64,
    checkpoint_seq: u64,
    recent: VecDeque<Entry>,
}

impl Store {
    /// Opens a v3 directory, replaying any WAL entries newer than the last checkpoint.
    /// # Errors When the path is not a project directory, the document does not parse, or the WAL is corrupt.
    pub fn open(root: &std::path::Path) -> Result<Self, StoreError> {
        let Located::V3Dir(root) = locate(root)? else {
            return Err(StoreError::NotADirectory(root.to_path_buf()));
        };
        let layout = Layout::new(root);
        let text = fs::read_to_string(layout.document()).map_err(io_err("reading the document"))?;
        let mut doc = parse(&text)?;
        let checkpoint_seq = read_checkpoint(&layout)?;
        let mut seq = checkpoint_seq;
        let mut recent = VecDeque::new();
        for (line_no, entry) in read_wal(&layout)?.into_iter().enumerate() {
            if entry.seq <= seq {
                continue;
            }
            doc = apply_all(&doc, &entry.ops).map_err(|(index, source)| StoreError::Wal {
                line: line_no + 1,
                message: format!("op {index}: {source}"),
            })?;
            seq = entry.seq;
            remember(&mut recent, entry);
        }
        Ok(Self {
            layout,
            doc,
            seq,
            checkpoint_seq,
            recent,
        })
    }

    /// The ops applied after `seq`, oldest first; `None` when `seq` is older than the ring remembers.
    #[must_use]
    pub fn since(&self, seq: u64) -> Option<Vec<Op>> {
        if seq >= self.seq {
            return Some(Vec::new());
        }
        let oldest = self.recent.front().map(|e| e.seq)?;
        if seq + 1 < oldest {
            return None;
        }
        Some(
            self.recent
                .iter()
                .filter(|e| e.seq > seq)
                .flat_map(|e| e.ops.iter().cloned())
                .collect(),
        )
    }

    /// `apply`, refused with the missed ops when the document is not where the writer expects.
    /// # Errors `Stale` on a mismatch, else as `apply`.
    pub fn apply_expecting(&mut self, ops: &[Op], expect: Expect) -> Result<Applied, StoreError> {
        let seq_ok = expect.seq.is_none_or(|s| s == self.seq);
        let hash_ok = expect.hash.as_ref().is_none_or(|h| *h == self.hash());
        if !seq_ok || !hash_ok {
            return Err(StoreError::Stale {
                seq: self.seq,
                hash: self.hash(),
                since: expect.seq.and_then(|s| self.since(s)),
            });
        }
        self.apply(ops)
    }

    #[must_use]
    pub fn document(&self) -> &Document {
        &self.doc
    }

    #[must_use]
    pub fn seq(&self) -> u64 {
        self.seq
    }

    #[must_use]
    pub fn hash(&self) -> DocHash {
        DocHash::of(&self.doc)
    }

    /// Entries applied since the last checkpoint; what a crash right now would have to replay.
    #[must_use]
    pub fn pending(&self) -> u64 {
        self.seq - self.checkpoint_seq
    }

    #[must_use]
    pub fn check(&self) -> Report {
        validate(&self.doc)
    }

    /// Applies `ops` as one batch: all or nothing, validated, logged before it becomes visible.
    /// # Errors When an op cannot apply, the result has validation errors, or the WAL cannot be written; the document is untouched.
    pub fn apply(&mut self, ops: &[Op]) -> Result<Applied, StoreError> {
        let next = apply_all(&self.doc, ops)
            .map_err(|(index, source)| StoreError::Op { index, source })?;
        // Only errors the batch introduces are refused: a legacy value already in the file must not block every later edit.
        let before = validate(&self.doc);
        let introduced: Vec<String> = validate(&next)
            .errors()
            .filter(|i| {
                !before
                    .errors()
                    .any(|b| b.code == i.code && b.id == i.id && b.attr == i.attr)
            })
            .map(|i| i.message.clone())
            .collect();
        if !introduced.is_empty() {
            return Err(StoreError::Invalid(introduced.join("; ")));
        }
        let seq = self.seq + 1;
        let entry = Entry {
            seq,
            ops: ops.to_vec(),
        };
        append_wal(&self.layout, &entry)?;
        remember(&mut self.recent, entry);
        self.doc = next;
        self.seq = seq;
        Ok(Applied {
            seq,
            hash: self.hash(),
        })
    }

    /// Writes the document atomically and empties the WAL. Safe to call at any time; a crash mid-way loses nothing.
    /// # Errors When the document or the checkpoint marker cannot be written.
    pub fn checkpoint(&mut self) -> Result<(), StoreError> {
        write_atomic(&self.layout.document(), serialize(&self.doc).as_bytes())?;
        fs::create_dir_all(self.layout.cache()).map_err(io_err("creating the cache directory"))?;
        let marker =
            serde_json::to_vec(&Checkpoint { seq: self.seq }).map_err(|e| StoreError::Wal {
                line: 0,
                message: e.to_string(),
            })?;
        write_atomic(&self.layout.store_state(), &marker)?;
        File::create(self.layout.wal())
            .and_then(|f| f.sync_all())
            .map_err(io_err("truncating the WAL"))?;
        self.checkpoint_seq = self.seq;
        Ok(())
    }
}

fn remember(recent: &mut VecDeque<Entry>, entry: Entry) {
    if recent.len() == RING {
        recent.pop_front();
    }
    recent.push_back(entry);
}

fn read_checkpoint(layout: &Layout) -> Result<u64, StoreError> {
    match fs::read(layout.store_state()) {
        Ok(bytes) => serde_json::from_slice::<Checkpoint>(&bytes)
            .map(|c| c.seq)
            .map_err(|e| StoreError::Wal {
                line: 0,
                message: format!("store.json: {e}"),
            }),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(0),
        Err(e) => Err(io_err("reading store.json")(e)),
    }
}

fn read_wal(layout: &Layout) -> Result<Vec<Entry>, StoreError> {
    let file = match File::open(layout.wal()) {
        Ok(f) => f,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(io_err("opening the WAL")(e)),
    };
    let mut entries = Vec::new();
    for (i, line) in BufReader::new(file).lines().enumerate() {
        let line = line.map_err(io_err("reading the WAL"))?;
        if line.trim().is_empty() {
            continue;
        }
        // A torn final line is the crash the WAL exists for: everything before it is intact and replays.
        match serde_json::from_str::<Entry>(&line) {
            Ok(entry) => entries.push(entry),
            Err(e) if i + 1 == count_lines(layout) => {
                let _ = e;
                break;
            }
            Err(e) => {
                return Err(StoreError::Wal {
                    line: i + 1,
                    message: e.to_string(),
                })
            }
        }
    }
    Ok(entries)
}

fn count_lines(layout: &Layout) -> usize {
    fs::read_to_string(layout.wal()).map_or(0, |t| t.lines().count())
}

fn append_wal(layout: &Layout, entry: &Entry) -> Result<(), StoreError> {
    fs::create_dir_all(layout.cache()).map_err(io_err("creating the cache directory"))?;
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(layout.wal())
        .map_err(io_err("opening the WAL"))?;
    let line = serde_json::to_string(entry).map_err(|e| StoreError::Wal {
        line: 0,
        message: e.to_string(),
    })?;
    writeln!(file, "{line}").map_err(io_err("writing the WAL"))?;
    file.sync_data().map_err(io_err("flushing the WAL"))
}

fn write_atomic(path: &std::path::Path, bytes: &[u8]) -> Result<(), StoreError> {
    let tmp = path.with_extension("tmp");
    {
        let mut file = File::create(&tmp).map_err(io_err(format!("creating {}", tmp.display())))?;
        file.write_all(bytes)
            .map_err(io_err(format!("writing {}", tmp.display())))?;
        file.sync_all()
            .map_err(io_err(format!("flushing {}", tmp.display())))?;
    }
    fs::rename(&tmp, path).map_err(io_err(format!("renaming into {}", path.display())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout;
    use crate::ops::NodeSpec;

    fn project(dir: &std::path::Path) {
        fs::create_dir_all(dir).unwrap();
        fs::write(
            dir.join(layout::DOCUMENT),
            "<recast v=\"3\"><timeline in=\"0\" out=\"60\"/><screen id=\"scr\"><zooms/></screen></recast>\n",
        )
        .unwrap();
    }

    fn add_zoom(id: &str, at: &str) -> Op {
        Op::Insert {
            parent: "scr".into(),
            index: 0,
            node: NodeSpec {
                kind: "zooms".into(),
                attrs: Default::default(),
                text: None,
                children: vec![NodeSpec {
                    kind: "zoom".into(),
                    attrs: [
                        ("id".to_owned(), id.to_owned()),
                        ("at".to_owned(), at.to_owned()),
                        ("dur".to_owned(), "2".to_owned()),
                    ]
                    .into_iter()
                    .collect(),
                    text: None,
                    children: vec![],
                }],
            },
        }
    }

    #[test]
    fn applied_ops_survive_a_crash_before_the_checkpoint() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        project(&dir);
        let mut store = Store::open(&dir).unwrap();
        let applied = store.apply(&[add_zoom("z1", "4.5")]).unwrap();
        assert_eq!(applied.seq, 1);
        assert_eq!(store.pending(), 1);
        drop(store);
        assert!(
            !fs::read_to_string(dir.join(layout::DOCUMENT))
                .unwrap()
                .contains("z1"),
            "the file is untouched until a checkpoint"
        );
        let reopened = Store::open(&dir).unwrap();
        assert!(reopened.document().find("z1").is_some(), "the WAL replayed");
        assert_eq!(reopened.seq(), 1);
    }

    #[test]
    fn a_stale_writer_gets_the_ops_it_missed_or_a_re_read_when_too_far_behind() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        project(&dir);
        let mut store = Store::open(&dir).unwrap();
        let first = store.apply(&[add_zoom("z1", "4.5")]).unwrap();
        let expect = Expect {
            seq: Some(first.seq),
            hash: Some(first.hash),
        };
        let second = store.apply(&[add_zoom("z2", "9")]).unwrap();
        let err = store
            .apply_expecting(&[add_zoom("z3", "1")], expect)
            .unwrap_err();
        let StoreError::Stale { seq, hash, since } = err else {
            panic!("expected Stale, got {err}");
        };
        assert_eq!((seq, hash), (second.seq, second.hash.clone()));
        assert_eq!(since.unwrap(), vec![add_zoom("z2", "9")]);
        assert!(store.document().find("z3").is_none(), "nothing applied");
        assert_eq!(
            store
                .apply_expecting(
                    &[add_zoom("z3", "1")],
                    Expect {
                        seq: Some(second.seq),
                        hash: None
                    }
                )
                .unwrap()
                .seq,
            3
        );
        assert_eq!(store.since(3), Some(Vec::new()));
        let dropped = Store::open(&dir).unwrap();
        assert_eq!(
            dropped.since(0).unwrap().len(),
            3,
            "the ring is rebuilt from the WAL replay"
        );
        for i in 0..RING {
            store.apply(&[add_zoom(&format!("r{i}"), "1")]).unwrap();
        }
        assert!(store.since(1).is_none(), "seq 1 fell out of the ring");
        assert_eq!(store.since(store.seq() - 1).unwrap().len(), 1);
    }

    #[test]
    fn a_checkpoint_writes_the_file_empties_the_wal_and_keeps_seq_monotonic() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        project(&dir);
        let mut store = Store::open(&dir).unwrap();
        store.apply(&[add_zoom("z1", "4.5")]).unwrap();
        store.checkpoint().unwrap();
        assert_eq!(store.pending(), 0);
        assert!(fs::read_to_string(dir.join(layout::DOCUMENT))
            .unwrap()
            .contains("z1"));
        assert_eq!(fs::metadata(dir.join(".cache/wal.log")).unwrap().len(), 0);
        let mut reopened = Store::open(&dir).unwrap();
        assert_eq!(reopened.seq(), 1);
        let next = reopened
            .apply(&[Op::Set {
                id: "z1".into(),
                attr: "scale".into(),
                value: Some("2".into()),
            }])
            .unwrap();
        assert_eq!(next.seq, 2);
    }

    #[test]
    fn an_invalid_batch_is_refused_and_leaves_document_and_wal_untouched() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        project(&dir);
        let mut store = Store::open(&dir).unwrap();
        let before = store.hash();
        let err = store
            .apply(&[Op::Set {
                id: "scr".into(),
                attr: "src".into(),
                value: Some("nope".into()),
            }])
            .unwrap_err();
        assert!(matches!(err, StoreError::Invalid(_)), "{err}");
        assert_eq!(store.hash(), before);
        assert_eq!(store.seq(), 0);
        assert!(!dir.join(".cache/wal.log").exists());
        let err = store
            .apply(&[Op::Remove { id: "ghost".into() }])
            .unwrap_err();
        assert!(matches!(err, StoreError::Op { index: 0, .. }));
    }

    #[test]
    fn a_legacy_error_already_in_the_file_does_not_block_unrelated_edits() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join(layout::DOCUMENT),
            "<recast v=\"3\"><timeline in=\"0\" out=\"60\"/><screen id=\"scr\"><zooms><zoom id=\"old\" at=\"1\" dur=\"2\" scale=\"4\"/></zooms></screen></recast>
",
        )
        .unwrap();
        let mut store = Store::open(&dir).unwrap();
        assert!(!store.check().is_ok(), "the legacy 4x is still reported");
        store
            .apply(&[Op::Set {
                id: "old".into(),
                attr: "cx".into(),
                value: Some("0.3".into()),
            }])
            .unwrap();
        let err = store
            .apply(&[Op::Set {
                id: "old".into(),
                attr: "cy".into(),
                value: Some("7".into()),
            }])
            .unwrap_err();
        assert!(
            matches!(err, StoreError::Invalid(_)),
            "a new error is still refused"
        );
    }

    #[test]
    fn a_torn_last_wal_line_is_dropped_and_earlier_entries_replay() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("P.recast");
        project(&dir);
        let mut store = Store::open(&dir).unwrap();
        store.apply(&[add_zoom("z1", "4.5")]).unwrap();
        drop(store);
        let wal = dir.join(".cache/wal.log");
        let mut text = fs::read_to_string(&wal).unwrap();
        text.push_str("{\"seq\":2,\"ops\":[{\"op\":\"set\",\"id\":\"z1\"");
        fs::write(&wal, text).unwrap();
        let reopened = Store::open(&dir).unwrap();
        assert_eq!(reopened.seq(), 1);
        assert!(reopened.document().find("z1").is_some());
    }
}

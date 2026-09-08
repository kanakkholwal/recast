//! `doc.*` verbs: the document owner on the wire. `show`, `apply`, `since` and `flush`, with `expectSeq`/`expectHash` on writes.
//! Shared by the control socket and the Tauri commands so an agent and the webview see one sequencer.

use std::path::Path;

use recast_project::store::Expect;
use recast_project::{DocHash, Op};
use serde_json::{json, Value};

use crate::agent::guard::{check_batch_size, within_budget, ProjectPath};
use crate::project::documents::documents;

/// Emitted after every sequenced batch, so a replica can pull `since` its own seq.
pub const DOCUMENT_CHANGED_EVENT: &str = "document:changed";

/// The live document: canonical text, its hash, and the seq it is at.
pub fn show(path: &str) -> Result<Value, String> {
    let project = guarded(path)?;
    let snapshot = documents().snapshot(&project).map_err(stringify)?;
    let value = serde_json::to_value(snapshot).map_err(stringify)?;
    Ok(within_budget(
        value,
        "the document is over budget; read it with doc.since from a known seq, or the file on disk",
    ))
}

/// Sequences `ops` when the document is where the writer expects; a conflict answers with the ops it missed.
/// The `document:changed` event comes from the owner's listener (see `announce_changes`), not from here.
pub fn apply(path: &str, ops: &[Op], expect: Expect) -> Result<Value, String> {
    let project = guarded(path)?;
    check_batch_size(ops.len()).map_err(stringify)?;
    let outcome = documents()
        .apply(&project, ops, expect)
        .map_err(stringify)?;
    let warnings = documents()
        .with(&project, |s| s.check().issues.len())
        .map_err(stringify)?;
    let mut value = serde_json::to_value(outcome).map_err(stringify)?;
    value["warnings"] = json!(warnings);
    Ok(value)
}

/// The ops after `seq`, or `null` when the ring no longer reaches that far and the caller must `show` again.
pub fn since(path: &str, seq: u64) -> Result<Value, String> {
    let project = guarded(path)?;
    let (ops, at, hash) = documents()
        .with(&project, |s| (s.since(seq), s.seq(), s.hash()))
        .map_err(stringify)?;
    Ok(json!({ "ops": ops, "seq": at, "hash": hash }))
}

/// Writes the checkpoint now instead of after the debounce.
pub fn flush(path: &str) -> Result<Value, String> {
    let project = guarded(path)?;
    documents().flush(&project).map_err(stringify)?;
    let seq = documents().with(&project, |s| s.seq()).map_err(stringify)?;
    Ok(json!({ "seq": seq }))
}

/// Wires the owner to the app: every sequenced batch, from any writer, reaches the webview as one event.
pub fn announce_changes(app: tauri::AppHandle) {
    use tauri::Emitter;
    documents().on_change(Box::new(move |path, seq, hash| {
        let _ = app.emit(
            DOCUMENT_CHANGED_EVENT,
            json!({ "path": path.to_string_lossy(), "seq": seq, "hash": hash }),
        );
    }));
}

/// Routes a `doc.*` method; `None` for any other method.
pub fn dispatch(method: &str, params: &Value) -> Option<Result<Value, String>> {
    let path = || {
        params
            .get("path")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| format!("{method} requires path"))
    };
    Some(match method {
        "doc.show" => path().and_then(|p| show(&p)),
        "doc.apply" => path().and_then(|p| {
            let ops: Vec<Op> = serde_json::from_value(
                params
                    .get("ops")
                    .cloned()
                    .ok_or_else(|| format!("{method} requires ops"))?,
            )
            .map_err(|e| format!("{method}: invalid ops: {e}"))?;
            apply(&p, &ops, expect_of(params)?)
        }),
        "doc.since" => path().and_then(|p| {
            let seq = params
                .get("seq")
                .and_then(Value::as_u64)
                .ok_or_else(|| format!("{method} requires seq"))?;
            since(&p, seq)
        }),
        "doc.flush" => path().and_then(|p| flush(&p)),
        _ => return None,
    })
}

pub fn expect_of(params: &Value) -> Result<Expect, String> {
    let hash = match params.get("expectHash").and_then(Value::as_str) {
        Some(h) => Some(DocHash::parse(h).map_err(stringify)?),
        None => None,
    };
    Ok(Expect {
        seq: params.get("expectSeq").and_then(Value::as_u64),
        hash,
    })
}

fn guarded(path: &str) -> Result<std::path::PathBuf, String> {
    let guarded = ProjectPath::parse(path).map_err(stringify)?.as_str();
    let path = Path::new(&guarded);
    if !crate::project::v3::is_project_dir(path) {
        return Err(format!(
            "{guarded} is not a v3 project directory; doc.* verbs need one (migrate it first)"
        ));
    }
    Ok(path.to_path_buf())
}

fn stringify(err: impl std::fmt::Display) -> String {
    err.to_string()
}

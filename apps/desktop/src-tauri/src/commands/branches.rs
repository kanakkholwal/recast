//! Branch operations, shared by the control socket, Tauri IPC and MCP.
//!
//! Every surface goes through [`BranchService`] so the three cannot drift: the
//! CLI, the editor's review panel and an agent all see the same journal, the
//! same guarantees and the same payload shapes.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

use super::editor_session::now_ms;
use super::error::{AppError, AppResult};
use super::types::AppState;
use crate::project::journal::{self, Branch, BranchId, BranchStore, FieldChange, StateHash};
use crate::render::graph::RenderState;
use crate::render::ops::Op;

/// Emitted whenever a branch journal is created, appended to or removed.
pub const BRANCHES_CHANGED_EVENT: &str = "editor-branches:changed";

/// A branch without its ops, for list views.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchSummary {
    pub id: BranchId,
    pub author: String,
    pub label: Option<String>,
    pub base: StateHash,
    /// Sequence number of the newest entry; `0` on an empty branch.
    pub seq: u64,
    pub ops: usize,
    pub created_at_ms: i64,
    pub updated_at_ms: i64,
}

impl From<&Branch> for BranchSummary {
    fn from(branch: &Branch) -> Self {
        Self {
            id: branch.id.clone(),
            author: branch.author.clone(),
            label: branch.label.clone(),
            base: branch.base,
            seq: branch.next_seq() - 1,
            ops: branch.op_count(),
            created_at_ms: branch.created_at_ms,
            updated_at_ms: branch.updated_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppendReport {
    pub seq: u64,
    /// `false` when the `idem_key` was already on the branch, so nothing new landed.
    pub recorded: bool,
    pub compacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyReport {
    pub changes: usize,
}

/// Branch operations against one running app.
pub struct BranchService<'a> {
    app: &'a AppHandle,
    state: &'a AppState,
}

impl<'a> BranchService<'a> {
    pub fn new(app: &'a AppHandle, state: &'a AppState) -> Self {
        Self { app, state }
    }

    /// # Errors
    /// [`AppError`] if the app data directory is unavailable or the project
    /// cannot be read.
    pub fn create(
        &self,
        project: &str,
        id: BranchId,
        author: String,
        label: Option<String>,
    ) -> AppResult<Branch> {
        let base = StateHash::of(&load_render_state(project)?).map_err(AppError::msg)?;
        let branch = Branch::new(id, base, author, label, now_ms());
        self.store(project)?.save(&branch).map_err(AppError::msg)?;
        self.announce(project);
        Ok(branch)
    }

    /// Journals that will not parse are skipped, so one corrupt file cannot
    /// hide the rest.
    pub fn list(&self, project: &str) -> AppResult<Vec<BranchSummary>> {
        let store = self.store(project)?;
        Ok(store
            .list()
            .iter()
            .filter_map(|id| store.load(id).ok())
            .map(|branch| BranchSummary::from(&branch))
            .collect())
    }

    pub fn load(&self, project: &str, id: &BranchId) -> AppResult<Branch> {
        self.store(project)?.load(id).map_err(AppError::msg)
    }

    /// Record `ops` as one atomic entry, replaying the branch before persisting
    /// so an op that cannot apply is rejected now rather than at review time.
    ///
    /// # Errors
    /// [`AppError`] wrapping a stale `expect_seq`, an op that no longer fits, or
    /// a failed write.
    pub fn append(
        &self,
        project: &str,
        id: &BranchId,
        idem_key: String,
        ops: Vec<Op>,
        expect_seq: Option<u64>,
    ) -> AppResult<AppendReport> {
        let store = self.store(project)?;
        let mut branch = store.load(id).map_err(AppError::msg)?;
        let outcome = branch
            .append(idem_key, ops, expect_seq, now_ms())
            .map_err(AppError::msg)?;

        let base = load_render_state(project)?;
        branch.materialize(&base).map_err(AppError::msg)?;
        let compacted = branch.needs_compaction();
        if compacted {
            branch.compact(&base, now_ms()).map_err(AppError::msg)?;
        }

        store.save(&branch).map_err(AppError::msg)?;
        self.announce(project);
        Ok(AppendReport {
            seq: outcome.seq(),
            recorded: outcome.is_recorded(),
            compacted,
        })
    }

    pub fn truncate(&self, project: &str, id: &BranchId, seq: u64) -> AppResult<BranchSummary> {
        let store = self.store(project)?;
        let mut branch = store.load(id).map_err(AppError::msg)?;
        branch.truncate_after(seq, now_ms());
        store.save(&branch).map_err(AppError::msg)?;
        self.announce(project);
        Ok(BranchSummary::from(&branch))
    }

    /// The render state the branch would produce.
    pub fn materialize(&self, project: &str, id: &BranchId) -> AppResult<RenderState> {
        self.load(project, id)?
            .materialize(&load_render_state(project)?)
            .map_err(AppError::msg)
    }

    /// Leaf-level changes the branch would make, in path order.
    pub fn diff(&self, project: &str, id: &BranchId) -> AppResult<Vec<FieldChange>> {
        let base = load_render_state(project)?;
        let proposed = self
            .load(project, id)?
            .materialize(&base)
            .map_err(AppError::msg)?;
        journal::diff(&base, &proposed).map_err(AppError::msg)
    }

    pub fn discard(&self, project: &str, id: &BranchId) -> AppResult<()> {
        self.store(project)?.remove(id).map_err(AppError::msg)?;
        self.announce(project);
        Ok(())
    }

    /// Write the branch into the project, then delete it.
    ///
    /// Fast-forward only: materializing against the state the write-lock just
    /// loaded is what rejects a project edited since the fork.
    ///
    /// # Errors
    /// [`AppError`] wrapping `editor_locked`, a moved fork point, or a
    /// validation failure on the resulting state.
    pub fn apply(&self, project: &str, id: &BranchId, writer_id: &str) -> AppResult<ApplyReport> {
        let store = self.store(project)?;
        let branch = store.load(id).map_err(AppError::msg)?;

        let changes =
            super::patch_render_state(self.state, self.app, project, writer_id, |current| {
                let proposed = branch.materialize(current).map_err(|e| e.to_string())?;
                let changes = journal::diff(current, &proposed)
                    .map_err(|e| e.to_string())?
                    .len();
                *current = proposed;
                Ok(changes)
            })?;

        store.remove(id).map_err(AppError::msg)?;
        self.announce(project);
        Ok(ApplyReport { changes })
    }

    fn store(&self, project: &str) -> AppResult<BranchStore> {
        branch_store(self.app, project)
    }

    fn announce(&self, project: &str) {
        let _ = self.app.emit(
            BRANCHES_CHANGED_EVENT,
            serde_json::json!({ "path": project }),
        );
    }
}

/// Journals live under the app data dir rather than beside the `.recast`: they
/// are pending work, so the temp-dir sweeper must not reclaim them.
pub fn branch_store(app: &AppHandle, project_path: &str) -> AppResult<BranchStore> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| AppError::msg(format!("app_data_dir unavailable: {e}")))?;
    Ok(BranchStore::new(root.join("branches").join(
        journal::project_key(std::path::Path::new(project_path)),
    )))
}

fn load_render_state(project_path: &str) -> AppResult<RenderState> {
    tauri::async_runtime::block_on(super::load_editor_document(project_path.to_string()))
        .map(|doc| doc.render_state)
}

/// Run `job` off the UI thread, handing it the service.
///
/// Tauri commands that block the main thread freeze the macOS WKWebView, and
/// every branch call reads or writes the project.
async fn off_thread<T, F>(app: AppHandle, job: F) -> AppResult<T>
where
    T: Send + 'static,
    F: FnOnce(&BranchService<'_>) -> AppResult<T> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(move || {
        let state = app.state::<AppState>();
        job(&BranchService::new(&app, &state))
    })
    .await
    .map_err(|e| AppError::msg(format!("branch task panicked: {e}")))?
}

fn parse_id(branch: String) -> AppResult<BranchId> {
    BranchId::new(branch).map_err(AppError::msg)
}

#[tauri::command]
pub async fn list_branches(app: AppHandle, project_path: String) -> AppResult<Vec<BranchSummary>> {
    off_thread(app, move |service| service.list(&project_path)).await
}

#[tauri::command]
pub async fn create_branch(
    app: AppHandle,
    project_path: String,
    branch: String,
    author: String,
    label: Option<String>,
) -> AppResult<BranchSummary> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| {
        service
            .create(&project_path, id, author, label)
            .map(|branch| BranchSummary::from(&branch))
    })
    .await
}

#[tauri::command]
pub async fn append_to_branch(
    app: AppHandle,
    project_path: String,
    branch: String,
    idem_key: String,
    ops: Vec<Op>,
    expect_seq: Option<u64>,
) -> AppResult<AppendReport> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| {
        service.append(&project_path, &id, idem_key, ops, expect_seq)
    })
    .await
}

#[tauri::command]
pub async fn diff_branch(
    app: AppHandle,
    project_path: String,
    branch: String,
) -> AppResult<Vec<FieldChange>> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| service.diff(&project_path, &id)).await
}

#[tauri::command]
pub async fn materialize_branch(
    app: AppHandle,
    project_path: String,
    branch: String,
) -> AppResult<RenderState> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| service.materialize(&project_path, &id)).await
}

#[tauri::command]
pub async fn truncate_branch(
    app: AppHandle,
    project_path: String,
    branch: String,
    seq: u64,
) -> AppResult<BranchSummary> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| {
        service.truncate(&project_path, &id, seq)
    })
    .await
}

#[tauri::command]
pub async fn discard_branch(app: AppHandle, project_path: String, branch: String) -> AppResult<()> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| service.discard(&project_path, &id)).await
}

#[tauri::command]
pub async fn apply_branch(
    app: AppHandle,
    project_path: String,
    branch: String,
    writer_id: String,
) -> AppResult<ApplyReport> {
    let id = parse_id(branch)?;
    off_thread(app, move |service| {
        service.apply(&project_path, &id, &writer_id)
    })
    .await
}

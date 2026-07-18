//! Per-project editor write-lock. The GUI user and a CLI agent both go
//! through the same `try_acquire_write` API so one holds the project at a
//! time; the other sees a structured `editor_locked` error (CLI) or a
//! banner + disabled mutators (GUI).
//!
//! Crash-safety: the in-memory state is mirrored to `recast_session.json`
//! under the app's data dir. On next boot the snapshot is read once and the
//! `holder_pid` is checked for liveness — if the prior holder is gone the
//! session is cleared, otherwise it stays valid.
//!
//! Activity: every successful acquire / release / mutate bumps
//! `last_activity_at_ms`. After `EditorSession::TTL_MS` of inactivity the lock
//! is reclaimable, so a crashed agent never strands the project.
//!
//! **Testability:** the lock helpers accept `&RwLock<EditorSession>` rather
//! than `&AppState`. Both surfaces (`&AppState` for production, bare lock
//! for tests) reduce to the same code — the production helpers just
//! unwrap `state.editor_session` and forward.

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use super::system::write_atomic;
use super::types::{AppState, EditorSession, EditorWriterKind};

const SESSION_FILE_NAME: &str = "recast_session.json";

/// On-disk shape. Mirrors `EditorSession` plus the holder's PID so we can
/// detect a crashed holder on boot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedEditorSession {
    project_path: PathBuf,
    writer: EditorWriterKind,
    writer_id: String,
    acquired_at_ms: i64,
    last_activity_at_ms: i64,
    holder_pid: u32,
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

#[cfg(unix)]
fn is_pid_alive(pid: u32) -> bool {
    // SAFETY: zero is a well-defined "send no signal" probe.
    let res = unsafe { libc::kill(pid as i32, 0) };
    if res == 0 {
        true
    } else {
        let err = std::io::Error::last_os_error();
        // EPERM = exists but not ours. Either way, alive.
        err.raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(windows)]
fn is_pid_alive(pid: u32) -> bool {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION};

    unsafe {
        let Ok(handle) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        // The handle itself is a proof of existence: a successfully opened
        // handle (even on a zombie we can't signal) means the kernel still
        // has the PID. Close it immediately; we don't need to wait.
        if handle.is_invalid() {
            return false;
        }
        let _ = CloseHandle(handle);
        true
    }
}

#[cfg(not(any(unix, windows)))]
fn is_pid_alive(_pid: u32) -> bool {
    true
}

/// Production wrapper: forwards to `try_acquire_write_lock` against
/// `state.editor_session`.
pub(crate) fn try_acquire_write(
    state: &AppState,
    project_path: PathBuf,
    kind: EditorWriterKind,
    writer_id: String,
) -> Result<(), String> {
    try_acquire_write_lock(&state.editor_session, project_path, kind, writer_id)
}

/// Try to acquire the project write-lock against a bare `RwLock`. Pure; no
/// `AppState`. Tests use this directly.
pub(crate) fn try_acquire_write_lock(
    lock: &RwLock<EditorSession>,
    project_path: PathBuf,
    kind: EditorWriterKind,
    writer_id: String,
) -> Result<(), String> {
    let mut session = lock.write();
    let now = now_ms();

    let stale_holder =
        session.writer.is_some() && (now - session.last_activity_at_ms) > EditorSession::TTL_MS;

    if session.writer.is_some() && !stale_holder {
        return Err(format!(
            "editor_locked: project '{}' held by '{}' (acquired {} ms ago); \
             use `recast project unlock --force` to reclaim or wait {}s for TTL.",
            session
                .project_path
                .as_ref()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default(),
            session.writer_id,
            now - session.acquired_at_ms,
            EditorSession::TTL_MS / 1000,
        ));
    }

    session.project_path = Some(project_path);
    session.writer = Some(kind);
    session.writer_id = writer_id;
    session.acquired_at_ms = now;
    session.last_activity_at_ms = now;
    Ok(())
}

pub(crate) fn release_if_owner(state: &AppState, writer_id: &str) -> bool {
    release_if_owner_lock(&state.editor_session, writer_id)
}

pub(crate) fn release_if_owner_lock(lock: &RwLock<EditorSession>, writer_id: &str) -> bool {
    let mut session = lock.write();
    if session.writer_id == writer_id {
        session.writer = None;
        session.project_path = None;
        session.writer_id.clear();
        session.last_activity_at_ms = 0;
        session.acquired_at_ms = 0;
        return true;
    }
    false
}

pub(crate) fn force_release(state: &AppState) -> Option<EditorWriterKind> {
    force_release_lock(&state.editor_session)
}

pub(crate) fn force_release_lock(lock: &RwLock<EditorSession>) -> Option<EditorWriterKind> {
    let prior = lock.read().writer;
    let mut session = lock.write();
    session.writer = None;
    session.project_path = None;
    session.writer_id.clear();
    session.last_activity_at_ms = 0;
    session.acquired_at_ms = 0;
    prior
}

pub(crate) fn record_activity(state: &AppState) {
    record_activity_lock(&state.editor_session);
}

pub(crate) fn record_activity_lock(lock: &RwLock<EditorSession>) {
    let mut session = lock.write();
    if session.writer.is_some() {
        session.last_activity_at_ms = now_ms();
    }
}

pub(crate) fn snapshot(state: &AppState) -> EditorSession {
    state.editor_session.read().clone()
}

/// Persist the current session to disk. Atomic write. Best-effort.
pub(crate) fn persist(state: &AppState, app: &AppHandle) {
    let session = state.editor_session.read().clone();
    let Some(writer) = session.writer else {
        remove_file(app);
        return;
    };
    let Some(path) = session.project_path.clone() else {
        return;
    };
    let payload = PersistedEditorSession {
        project_path: path,
        writer,
        writer_id: session.writer_id.clone(),
        acquired_at_ms: session.acquired_at_ms,
        last_activity_at_ms: session.last_activity_at_ms,
        holder_pid: std::process::id(),
    };
    let Ok(bytes) = serde_json::to_vec_pretty(&payload) else {
        return;
    };
    let Some(target) = session_file(app) else {
        return;
    };
    let tmp = target.with_extension("json.tmp");
    let _ = write_atomic(&tmp, &target, &bytes);
}

pub(crate) fn load_on_startup(state: &AppState, app: &AppHandle) -> bool {
    let Some(path) = session_file(app) else {
        return false;
    };
    if !path.exists() {
        return false;
    }
    let Ok(bytes) = std::fs::read(&path) else {
        return false;
    };
    let Ok(persisted) = serde_json::from_slice::<PersistedEditorSession>(&bytes) else {
        let _ = std::fs::remove_file(&path);
        return false;
    };
    if !is_pid_alive(persisted.holder_pid) {
        log::info!(
            "editor session: holder pid {} no longer alive; clearing stale lock",
            persisted.holder_pid
        );
        let _ = std::fs::remove_file(&path);
        return false;
    }
    let mut session = state.editor_session.write();
    session.project_path = Some(persisted.project_path);
    session.writer = Some(persisted.writer);
    session.writer_id = persisted.writer_id;
    session.acquired_at_ms = persisted.acquired_at_ms;
    session.last_activity_at_ms = persisted.last_activity_at_ms;
    log::info!(
        "editor session: restored lock for project {:?} held by {:?}",
        session.project_path,
        session.writer
    );
    true
}

fn session_file(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|base| base.join(SESSION_FILE_NAME))
}

fn remove_file(app: &AppHandle) {
    if let Some(path) = session_file(app) {
        if path.exists() {
            let _ = std::fs::remove_file(&path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fresh() -> RwLock<EditorSession> {
        RwLock::new(EditorSession::default())
    }

    #[test]
    fn acquire_release_round_trip() {
        let lock = fresh();
        try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Agent,
            "agent:test".into(),
        )
        .unwrap();
        let snap = lock.read().clone();
        assert_eq!(snap.writer, Some(EditorWriterKind::Agent));
        assert_eq!(snap.writer_id, "agent:test");
        assert_eq!(
            snap.project_path.as_ref().unwrap().to_string_lossy(),
            "/tmp/foo.recast"
        );

        assert!(release_if_owner_lock(&lock, "agent:test"));
        assert_eq!(lock.read().writer, None);
        assert!(lock.read().project_path.is_none());
    }

    #[test]
    fn second_writer_blocks_with_holder_name() {
        let lock = fresh();
        try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Agent,
            "agent:a".into(),
        )
        .unwrap();
        let err = try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Agent,
            "agent:b".into(),
        )
        .unwrap_err();
        assert!(err.contains("editor_locked"), "got: {err}");
        assert!(err.contains("agent:a"), "got: {err}");
    }

    #[test]
    fn ttl_reclaims_stale_holder() {
        let lock = fresh();
        try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Agent,
            "agent:a".into(),
        )
        .unwrap();
        // Pretend the holder's last activity is in the distant past.
        lock.write().last_activity_at_ms = now_ms() - EditorSession::TTL_MS - 1000;
        // The next acquire reclaims.
        try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Ui,
            "ui:user".into(),
        )
        .unwrap();
        assert_eq!(lock.read().writer_id, "ui:user");
    }

    #[test]
    fn force_release_clears_any_holder() {
        let lock = fresh();
        try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Agent,
            "agent:a".into(),
        )
        .unwrap();
        let released = force_release_lock(&lock);
        assert_eq!(released, Some(EditorWriterKind::Agent));
        assert_eq!(lock.read().writer, None);
    }

    #[test]
    fn release_with_wrong_owner_is_no_op() {
        let lock = fresh();
        try_acquire_write_lock(
            &lock,
            PathBuf::from("/tmp/foo.recast"),
            EditorWriterKind::Agent,
            "agent:a".into(),
        )
        .unwrap();
        assert!(!release_if_owner_lock(&lock, "agent:b"));
        assert_eq!(lock.read().writer, Some(EditorWriterKind::Agent));
    }
}

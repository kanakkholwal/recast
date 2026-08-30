//! Backend-owned export queue. The single source of truth for every export's
//! lifecycle. The frontend builds a self-contained `ExportRequest` (render state
//! rasterized in the browser) and hands it here via `enqueue_export`; a single
//! serial worker task drains the queue one job at a time (the real
//! concurrency-of-1 lock, replacing the old JS guard), so an export survives
//! closing its editor and an app restart.
//!
//! Durability split (see `crate::db`): the heavy `ExportRequest` payload is
//! written to a file under `export_queue/<id>.json`; the `export_jobs` table holds
//! only lightweight metadata plus that payload's path. Files stay authoritative
//! for the heavy data.
//!
//! Progress is surfaced two ways: `run_export_job` keeps emitting the per-job
//! `export-state` events exactly as before (live ring), and this module emits a
//! lightweight `export-jobs-changed` event whenever queue membership or a job's
//! status changes (the frontend re-fetches `list_export_jobs`).

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use super::error::{AppError, AppResult};
use super::system::write_atomic;
use super::types::{AppState, CaptionSidecar, ExportRequest};
use crate::db::Db;

const JOBS_CHANGED_EVENT: &str = "export-jobs-changed";

/// Terminal jobs older than this are swept at startup (row + kept payload). The
/// activity center is ephemeral notification UI, so a week-old export entry is
/// stale; the exported FILE on disk is never touched, only the queue record.
const TERMINAL_JOB_TTL_MS: i64 = 7 * 24 * 60 * 60 * 1000;

/// A queue row as the frontend read-model consumes it. Field names mirror the TS
/// `ExportItem`: `file_path` = the source project (camelCase `filePath`), `path` =
/// the output on success.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportJobDto {
    pub id: String,
    pub filename: String,
    pub file_path: String,
    /// `queued` | `running` | `success` | `error` | `cancelled` | `interrupted`.
    pub status: String,
    /// `preparing` | `encoding` | `finalizing` | `cancelling`.
    pub phase: String,
    pub progress: f64,
    pub path: Option<String>,
    pub error: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

fn payloads_dir(app: &AppHandle) -> PathBuf {
    let base = app
        .path()
        .app_data_dir()
        .unwrap_or_else(|_| std::env::temp_dir());
    base.join("export_queue")
}

fn base_name(path: &str) -> String {
    Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| path.to_string())
}

fn emit_jobs_changed(app: &AppHandle) {
    let _ = app.emit(JOBS_CHANGED_EVENT, ());
}

// --- Store operations (all synchronous; callers run them off the UI thread) ---

struct ClaimedJob {
    id: String,
    payload_path: String,
}

/// Atomically pick the oldest queued job and mark it running. Single worker, so
/// there is no concurrent claimant; the update just records the transition.
fn claim_next_queued(db: &Db) -> Option<ClaimedJob> {
    let started = now_millis();
    db.with(|c| {
        let row = c
            .query_row(
                "SELECT id, payload_path FROM export_jobs \
                 WHERE status = 'queued' ORDER BY created_at ASC LIMIT 1",
                [],
                |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)),
            )
            .optional()?;
        match row {
            Some((id, payload_path)) => {
                c.execute(
                    "UPDATE export_jobs \
                     SET status = 'running', phase = 'preparing', progress = 0, started_at = ? \
                     WHERE id = ?",
                    params![started, id],
                )?;
                Ok(Some(ClaimedJob { id, payload_path }))
            }
            None => Ok(None),
        }
    })
    .unwrap_or(None)
}

fn finish_success(db: &Db, id: &str, output_path: &str) {
    let now = now_millis();
    let written = db.with(|c| {
        c.execute(
            "UPDATE export_jobs \
             SET status = 'success', phase = 'finalizing', progress = 100, \
                 output_path = ?, finished_at = ? WHERE id = ?",
            params![output_path, now, id],
        )?;
        Ok(())
    });
    log_terminal_write(written, id, "success");
}

fn finish_cancelled(db: &Db, id: &str) {
    let now = now_millis();
    let written = db.with(|c| {
        c.execute(
            "UPDATE export_jobs SET status = 'cancelled', phase = 'cancelling', finished_at = ? \
             WHERE id = ?",
            params![now, id],
        )?;
        Ok(())
    });
    log_terminal_write(written, id, "cancelled");
}

fn finish_error(db: &Db, id: &str, message: String) {
    let now = now_millis();
    let written = db.with(|c| {
        c.execute(
            "UPDATE export_jobs SET status = 'error', error = ?, finished_at = ? WHERE id = ?",
            params![message, now, id],
        )?;
        Ok(())
    });
    log_terminal_write(written, id, "error");
}

/// A failed terminal write leaves the row `running`, so the UI shows an export
/// that never finishes. Nothing here can undo it, but it must not be silent.
fn log_terminal_write<E: std::fmt::Display>(result: Result<(), E>, id: &str, status: &str) {
    if let Err(e) = result {
        log::error!("export job {id} finished as {status} but the row could not be updated: {e}");
    }
}

fn load_payload(path: &str) -> Result<ExportRequest, String> {
    let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}

/// Swap a path's extension, e.g. `foo.mp4` + `vtt` -> `foo.vtt`.
fn with_extension(path: &str, ext: &str) -> String {
    Path::new(path)
        .with_extension(ext)
        .to_string_lossy()
        .to_string()
}

/// Write a subtitle sidecar next to the finished export. Best-effort: a sidecar
/// failure must not fail an otherwise-good export.
fn write_sidecar(output_path: &str, sidecar: &CaptionSidecar) {
    let body = match sidecar.format.as_str() {
        "srt" => crate::transcription::subtitles::to_srt(&sidecar.transcript),
        "vtt" => crate::transcription::subtitles::to_vtt(&sidecar.transcript),
        other => {
            log::warn!("unknown caption sidecar format '{other}'; skipping");
            return;
        }
    };
    let dest = with_extension(output_path, &sidecar.format);
    if let Err(e) = std::fs::write(&dest, body) {
        log::warn!("caption sidecar write failed: {e}");
    }
}

// --- Worker ---

/// Spawn the single serial export worker. It drains all queued jobs, then parks on
/// `export_wake` until the next enqueue/retry. Because there is exactly one worker
/// and it runs one job at a time, this IS the concurrency-of-1 guarantee.
pub(crate) fn spawn_export_worker(app: AppHandle) {
    let wake = { app.state::<AppState>().export_wake.clone() };
    // Its OWN thread and runtime: between awaits this does minutes of blocking work that starved a shared runtime worker.
    let spawned = std::thread::Builder::new()
        .name("recast-export-worker".into())
        .spawn(move || {
            let runtime = match tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
            {
                Ok(runtime) => runtime,
                Err(e) => {
                    log::error!("export worker runtime failed to start: {e}");
                    return;
                }
            };
            runtime.block_on(async move {
                loop {
                    // Drain first, so jobs queued at startup (restart survivors) run without waiting for a notify.
                    loop {
                        let db = { app.state::<AppState>().db.clone() };
                        let Some(job) = claim_next_queued(&db) else {
                            break;
                        };
                        run_one(&app, job).await;
                    }
                    wake.notified().await;
                }
            });
        });
    if let Err(e) = spawned {
        log::error!("export worker thread failed to start: {e}");
    }
}

async fn run_one(app: &AppHandle, job: ClaimedJob) {
    // The claim already flipped the row to running; tell the UI.
    emit_jobs_changed(app);
    let db = { app.state::<AppState>().db.clone() };

    let request = match load_payload(&job.payload_path) {
        Ok(r) => r,
        Err(e) => {
            finish_error(&db, &job.id, format!("export payload unreadable: {e}"));
            emit_jobs_changed(app);
            return;
        }
    };
    let sidecar = request.caption_sidecar.clone();

    // Browser-rendered payloads take the mux-only path; everything else runs the Rust filter_complex compositor.
    let run_result = if let Some(browser_video) = request.browser_video_path.clone() {
        crate::commands::editor::run_mux_job(app.clone(), request, browser_video).await
    } else {
        crate::commands::editor::run_export_job(app.clone(), request).await
    };
    match run_result {
        Ok(output_path) => {
            if let Some(sc) = sidecar {
                write_sidecar(&output_path, &sc);
            }
            finish_success(&db, &job.id, &output_path);
            // A completed export needs no retry, so drop its heavy payload.
            let _ = std::fs::remove_file(&job.payload_path);
        }
        Err(e) => {
            let msg = e.to_string();
            if msg.to_lowercase().contains("cancel") {
                finish_cancelled(&db, &job.id);
            } else {
                finish_error(&db, &job.id, msg);
            }
            // Keep the payload so a failed/cancelled job can be retried in place.
        }
    }
    emit_jobs_changed(app);
}

/// Recover from an unclean shutdown. Any job left `running` had its FFmpeg killed
/// with the previous process (partial output), so mark it `interrupted`; queued
/// jobs stay and the worker picks them up on its first drain. Call at setup after
/// the DB is managed and before spawning the worker.
pub(crate) fn reconcile_on_load(app: &AppHandle) {
    let db = { app.state::<AppState>().db.clone() };
    let now = now_millis();
    let _ = db.with(|c| {
        c.execute(
            "UPDATE export_jobs \
             SET status = 'interrupted', error = 'Interrupted by app restart', finished_at = ? \
             WHERE status = 'running'",
            params![now],
        )?;
        Ok(())
    });
}

/// Delete terminal jobs finished before `cutoff` and return their payload paths.
/// Never touches `queued`/`running` rows. Split out as a pure DB step so it is
/// unit-testable without a Tauri app.
fn delete_terminal_before(conn: &Connection, cutoff: i64) -> rusqlite::Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT payload_path FROM export_jobs \
         WHERE status IN ('success', 'error', 'cancelled', 'interrupted') \
           AND COALESCE(finished_at, created_at) < ?",
    )?;
    let paths = stmt
        .query_map([cutoff], |r| r.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    conn.execute(
        "DELETE FROM export_jobs \
         WHERE status IN ('success', 'error', 'cancelled', 'interrupted') \
           AND COALESCE(finished_at, created_at) < ?",
        params![cutoff],
    )?;
    Ok(paths)
}

/// Startup GC for the queue: drop terminal jobs older than the TTL (bounding the
/// activity list) and delete both their payloads and any orphaned payload files
/// left by a crash between write and insert. Fire-and-forget on a blocking worker;
/// runs at startup only, so there is no live-enqueue race with orphan cleanup.
pub(crate) fn sweep_stale_jobs(app: &AppHandle) {
    let db = { app.state::<AppState>().db.clone() };
    let dir = payloads_dir(app);
    let cutoff = now_millis() - TERMINAL_JOB_TTL_MS;
    tauri::async_runtime::spawn_blocking(move || {
        let removed = db
            .with(|c| delete_terminal_before(c, cutoff))
            .unwrap_or_default();
        for p in removed {
            let _ = std::fs::remove_file(p);
        }
        // Orphan payloads and leaked `.tmp` files: a path no row references any more. Keep the file if the lookup errors.
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                let path_str = path.to_string_lossy().to_string();
                let referenced = db
                    .with(|c| {
                        let n: i64 = c.query_row(
                            "SELECT count(*) FROM export_jobs WHERE payload_path = ?",
                            [&path_str],
                            |r| r.get(0),
                        )?;
                        Ok(n > 0)
                    })
                    .unwrap_or(true);
                if !referenced {
                    let _ = std::fs::remove_file(&path);
                }
            }
        }
    });
}

// --- Commands ---

/// Queue an export. Validates the render state against the source's metadata,
/// then persists the (heavy, self-contained) render payload to disk, inserts
/// a `queued` row, and wakes the worker. Returns once the job is durably
/// queued; the export itself runs in the background.
#[tauri::command]
pub async fn enqueue_export(
    app: AppHandle,
    mut request: ExportRequest,
    state: State<'_, AppState>,
) -> AppResult<Vec<String>> {
    // One extra ffprobe per enqueue buys rejecting bad JSON at the IPC door instead of a confusing crash mid-encode.
    let input_path = PathBuf::from(&request.input_path);
    let source_video: PathBuf =
        if input_path.extension().and_then(|value| value.to_str()) == Some("recast") {
            match crate::project::reader::open_project(&input_path) {
                Ok(p) => p.recording_path,
                Err(e) => {
                    return Err(AppError::msg(format!(
                        "enqueue_export: open project failed: {e}"
                    )));
                }
            }
        } else {
            input_path.clone()
        };
    let source_meta = tauri::async_runtime::spawn_blocking({
        let source = source_video.clone();
        move || crate::commands::ffmpeg::probe_video_metadata(&source)
    })
    .await
    .map_err(|e| AppError::msg(format!("probe source join error: {e}")))?
    .map_err(AppError::msg)?;
    // Auto-repair first: an older project's wall-clock trim_end can sit past the CFR video, which the strict validator rejects.
    let repairs =
        crate::commands::repair_render_state(&mut request.render_state, source_meta.duration);
    if !repairs.is_empty() {
        log::warn!(
            "enqueue_export[{}] auto-repaired render state: {:?}",
            request.export_id,
            repairs
        );
    }
    if let Err(issues) =
        crate::commands::validate_render_state(&request.render_state, source_meta.duration)
    {
        return Err(AppError::msg(format!(
            "enqueue_export: render state invalid ({} issue{}): {}",
            issues.len(),
            if issues.len() == 1 { "" } else { "s" },
            serde_json::to_string(&issues).unwrap_or_else(|_| format!("{issues:?}")),
        )));
    }

    let id = request.export_id.clone();
    let source_path = request.input_path.clone();
    let filename = base_name(&source_path);
    let payload_path = payloads_dir(&app).join(format!("{id}.json"));
    let db = state.db.clone();
    let created_at = now_millis();

    let payload_for_row = payload_path.to_string_lossy().to_string();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<()> {
        if let Some(parent) = payload_path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| AppError::msg(format!("create export_queue dir: {e}")))?;
        }
        let bytes = serde_json::to_vec(&request)
            .map_err(|e| AppError::msg(format!("serialize export payload: {e}")))?;
        let tmp = payload_path.with_extension("json.tmp");
        write_atomic(&tmp, &payload_path, &bytes)
            .map_err(|e| AppError::msg(format!("write export payload: {e}")))?;
        db.with(|c| {
            c.execute(
                "INSERT INTO export_jobs \
                 (id, filename, source_path, status, phase, progress, payload_path, created_at) \
                 VALUES (?, ?, ?, 'queued', 'preparing', 0, ?, ?)",
                params![id, filename, source_path, payload_for_row, created_at],
            )?;
            Ok(())
        })
        .map_err(|e| AppError::msg(format!("insert export job: {e}")))?;
        Ok(())
    })
    .await
    .map_err(|e| AppError::msg(format!("enqueue_export join error: {e}")))??;

    state.export_wake.notify_one();
    emit_jobs_changed(&app);
    Ok(repairs)
}

/// Persist a browser-rendered export video (Phase 4): the mp4 bytes ride the
/// invoke body as raw bytes (an ArrayBuffer), same as `save_recorded_camera`.
/// Returns the temp path to hand back as `browser_video_path` on the follow-up
/// `enqueue_export`; the mux job's success cleanup removes it.
#[tauri::command]
pub async fn save_browser_export_video(
    app: AppHandle,
    request: tauri::ipc::Request<'_>,
) -> AppResult<String> {
    let bytes: Vec<u8> = match request.body() {
        tauri::ipc::InvokeBody::Raw(bytes) => bytes.clone(),
        tauri::ipc::InvokeBody::Json(serde_json::Value::Array(arr)) => arr
            .iter()
            .map(|v| v.as_u64().map(|n| n as u8))
            .collect::<Option<Vec<u8>>>()
            .ok_or_else(|| AppError::msg("browser export payload was not a byte array"))?,
        tauri::ipc::InvokeBody::Json(_) => {
            return Err(AppError::msg("browser export payload must be raw bytes"));
        }
    };
    let dir = payloads_dir(&app).join("browser-videos");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = dir.join(format!("browser-{stamp}.mp4"));
    tauri::async_runtime::spawn_blocking(move || -> AppResult<String> {
        std::fs::create_dir_all(&dir)
            .map_err(|e| AppError::msg(format!("create browser-videos dir: {e}")))?;
        std::fs::write(&path, &bytes)
            .map_err(|e| AppError::msg(format!("write browser export video: {e}")))?;
        Ok(path.to_string_lossy().to_string())
    })
    .await
    .map_err(|e| AppError::msg(format!("save_browser_export_video worker panicked: {e}")))?
}

/// The whole queue (queued, running, and undismissed terminal jobs), oldest first.
#[tauri::command]
pub async fn list_export_jobs(state: State<'_, AppState>) -> AppResult<Vec<ExportJobDto>> {
    let db = state.db.clone();
    tauri::async_runtime::spawn_blocking(move || -> AppResult<Vec<ExportJobDto>> {
        db.with(|c| {
            let mut stmt = c.prepare(
                "SELECT id, filename, source_path, status, phase, progress, \
                        output_path, error, created_at, started_at, finished_at \
                 FROM export_jobs ORDER BY created_at ASC",
            )?;
            let rows = stmt
                .query_map([], |r| {
                    Ok(ExportJobDto {
                        id: r.get(0)?,
                        filename: r.get(1)?,
                        file_path: r.get(2)?,
                        status: r.get(3)?,
                        phase: r.get(4)?,
                        progress: r.get(5)?,
                        path: r.get(6)?,
                        error: r.get(7)?,
                        created_at: r.get(8)?,
                        started_at: r.get(9)?,
                        finished_at: r.get(10)?,
                    })
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            Ok(rows)
        })
        .map_err(|e| AppError::msg(format!("list export jobs: {e}")))
    })
    .await
    .map_err(|e| AppError::msg(format!("list_export_jobs join error: {e}")))?
}

enum CancelAction {
    RemovedQueued(Option<String>),
    SignalRunning,
    None,
}

/// Cancel or remove a job. A queued job is dropped from the queue (and its payload
/// deleted); a running job is signalled to abort (the existing cancel-flag path
/// kills FFmpeg, and the worker then records it `cancelled`).
#[tauri::command]
pub async fn cancel_export_job(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let db = state.db.clone();
    let id_for_db = id.clone();
    let action = tauri::async_runtime::spawn_blocking(move || -> AppResult<CancelAction> {
        db.with(|c| {
            let status: Option<String> = c
                .query_row(
                    "SELECT status FROM export_jobs WHERE id = ?",
                    [&id_for_db],
                    |r| r.get(0),
                )
                .optional()?;
            match status.as_deref() {
                Some("queued") => {
                    let payload: Option<String> = c
                        .query_row(
                            "SELECT payload_path FROM export_jobs WHERE id = ?",
                            [&id_for_db],
                            |r| r.get(0),
                        )
                        .optional()?;
                    c.execute("DELETE FROM export_jobs WHERE id = ?", [&id_for_db])?;
                    Ok(CancelAction::RemovedQueued(payload))
                }
                Some("running") => {
                    c.execute(
                        "UPDATE export_jobs SET phase = 'cancelling' WHERE id = ?",
                        [&id_for_db],
                    )?;
                    Ok(CancelAction::SignalRunning)
                }
                _ => Ok(CancelAction::None),
            }
        })
        .map_err(|e| AppError::msg(format!("cancel export job: {e}")))
    })
    .await
    .map_err(|e| AppError::msg(format!("cancel_export_job join error: {e}")))??;

    match action {
        CancelAction::RemovedQueued(payload) => {
            if let Some(p) = payload {
                let _ = std::fs::remove_file(p);
            }
        }
        // Reuse the existing per-id cancel token the running export installed.
        CancelAction::SignalRunning => {
            if let Some(flag) = state.export_cancel.lock().get(&id) {
                flag.store(true, Ordering::Release);
            }
        }
        CancelAction::None => {}
    }
    emit_jobs_changed(&app);
    Ok(())
}

/// Remove a finished (non-running) job from the list and delete any kept payload.
#[tauri::command]
pub async fn dismiss_export_job(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let db = state.db.clone();
    let payload = tauri::async_runtime::spawn_blocking(move || -> AppResult<Option<String>> {
        db.with(|c| {
            let payload: Option<String> = c
                .query_row(
                    "SELECT payload_path FROM export_jobs WHERE id = ? AND status != 'running'",
                    [&id],
                    |r| r.get(0),
                )
                .optional()?;
            c.execute(
                "DELETE FROM export_jobs WHERE id = ? AND status != 'running'",
                [&id],
            )?;
            Ok(payload)
        })
        .map_err(|e| AppError::msg(format!("dismiss export job: {e}")))
    })
    .await
    .map_err(|e| AppError::msg(format!("dismiss_export_job join error: {e}")))??;

    if let Some(p) = payload {
        let _ = std::fs::remove_file(p);
    }
    emit_jobs_changed(&app);
    Ok(())
}

/// Requeue a failed/cancelled/interrupted job. Its payload is still on disk, so
/// this just resets the row to `queued` and wakes the worker.
#[tauri::command]
pub async fn retry_export_job(
    app: AppHandle,
    id: String,
    state: State<'_, AppState>,
) -> AppResult<()> {
    let db = state.db.clone();
    let requeued = tauri::async_runtime::spawn_blocking(move || -> AppResult<bool> {
        db.with(|c| {
            let n = c.execute(
                "UPDATE export_jobs \
                 SET status = 'queued', phase = 'preparing', progress = 0, \
                     error = NULL, output_path = NULL, started_at = NULL, finished_at = NULL \
                 WHERE id = ? AND status IN ('error', 'cancelled', 'interrupted')",
                [&id],
            )?;
            Ok(n > 0)
        })
        .map_err(|e| AppError::msg(format!("retry export job: {e}")))
    })
    .await
    .map_err(|e| AppError::msg(format!("retry_export_job join error: {e}")))??;

    if requeued {
        state.export_wake.notify_one();
    }
    emit_jobs_changed(&app);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::migrations::run(&conn).unwrap();
        conn
    }

    fn seed(conn: &Connection, id: &str, status: &str, finished_at: Option<i64>, created_at: i64) {
        conn.execute(
            "INSERT INTO export_jobs \
             (id, filename, source_path, status, phase, progress, payload_path, created_at, finished_at) \
             VALUES (?, 'f', 's', ?, 'finalizing', 100, ?, ?, ?)",
            params![id, status, format!("/tmp/{id}.json"), created_at, finished_at],
        )
        .unwrap();
    }

    fn ids(conn: &Connection) -> Vec<String> {
        conn.prepare("SELECT id FROM export_jobs ORDER BY id")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<rusqlite::Result<Vec<_>>>()
            .unwrap()
    }

    #[test]
    fn sweep_drops_old_terminal_keeps_recent_and_active() {
        let conn = open_db();
        seed(&conn, "old_ok", "success", Some(1_000), 0);
        seed(&conn, "old_err", "error", Some(2_000), 0);
        seed(&conn, "recent_ok", "success", Some(10_000), 0);
        // No finished_at: running + queued must never be swept regardless of age.
        seed(&conn, "run", "running", None, 100);
        conn.execute(
            "UPDATE export_jobs SET status = 'running', progress = 50 WHERE id = 'run'",
            [],
        )
        .unwrap();
        seed(&conn, "q", "queued", None, 100);
        conn.execute(
            "UPDATE export_jobs SET status = 'queued' WHERE id = 'q'",
            [],
        )
        .unwrap();

        let removed = delete_terminal_before(&conn, 5_000).unwrap();

        // old_ok (1000) and old_err (2000) are before the cutoff; recent_ok is not.
        assert_eq!(removed.len(), 2);
        assert!(removed.contains(&"/tmp/old_ok.json".to_string()));
        assert!(removed.contains(&"/tmp/old_err.json".to_string()));
        assert_eq!(ids(&conn), vec!["q", "recent_ok", "run"]);
    }

    #[test]
    fn sweep_never_touches_queued_even_when_old() {
        let conn = open_db();
        // A queued job with a very old created_at and no finished_at.
        seed(&conn, "q", "queued", None, 0);
        conn.execute(
            "UPDATE export_jobs SET status = 'queued' WHERE id = 'q'",
            [],
        )
        .unwrap();

        let removed = delete_terminal_before(&conn, i64::MAX).unwrap();

        assert!(removed.is_empty());
        assert_eq!(ids(&conn), vec!["q"]);
    }
}

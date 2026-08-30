//! Embedded local store (SQLite): the operational/index layer that sits beside
//! the file-authoritative document store (`.recast` bundles + media on disk).
//!
//! Files stay the source of truth for documents. This DB holds structured
//! operational state that either does not exist as a file (the export queue's run
//! state, later upload history) or is a rebuildable index over files (the
//! recordings/exports library list, deferred). Heavy payloads are NEVER blobbed
//! here: a queued export's render state is written as a file on disk and only its
//! path is stored in the row.

use std::path::PathBuf;
use std::sync::Arc;

use parking_lot::Mutex;
use rusqlite::Connection;
use tauri::{AppHandle, Manager};

pub(crate) mod migrations;

/// Shared handle to the app's SQLite connection. `Connection` is `!Sync`, so it
/// lives behind a `Mutex` that is only ever locked inside `spawn_blocking` or the
/// export worker task (never on the UI thread), matching the app's "resolve from
/// state, then do the blocking work off-thread" rule.
#[derive(Clone)]
pub struct Db(Arc<Mutex<Connection>>);

impl Db {
    /// Open (creating if needed) `app_data_dir/recast_index.db`, apply pragmas,
    /// and run migrations. Falls back to an in-memory DB on any failure so the app
    /// still launches: the export queue degrades to non-durable rather than
    /// blocking startup. Mirrors `system::config_path`'s temp-fallback ethos.
    pub fn open(app: &AppHandle) -> Db {
        match Self::try_open(app) {
            Ok(db) => db,
            Err(e) => {
                log::error!(
                    "failed to open local store ({e}); using in-memory fallback \
                     (export queue will not persist across restarts)"
                );
                let conn = Connection::open_in_memory().expect("in-memory sqlite must open");
                let _ = migrations::run(&conn);
                Db(Arc::new(Mutex::new(conn)))
            }
        }
    }

    fn try_open(app: &AppHandle) -> rusqlite::Result<Db> {
        let path = db_path(app);
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let conn = Connection::open(&path)?;
        // WAL so readers never block the worker's writes, NORMAL is durable enough under it, and foreign keys are on for later tables.
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA synchronous=NORMAL;
             PRAGMA foreign_keys=ON;",
        )?;
        migrations::run(&conn)?;
        Ok(Db(Arc::new(Mutex::new(conn))))
    }

    /// Run `f` with the locked connection. Callers MUST already be off the UI
    /// thread (inside `spawn_blocking` or the worker task); the lock is held for
    /// the duration of `f`.
    pub fn with<T>(
        &self,
        f: impl FnOnce(&Connection) -> rusqlite::Result<T>,
    ) -> rusqlite::Result<T> {
        let conn = self.0.lock();
        f(&conn)
    }
}

fn db_path(app: &AppHandle) -> PathBuf {
    let dir = app.path().app_data_dir().unwrap_or_else(|e| {
        log::warn!("app_data_dir unavailable ({e}); using temp dir for local store");
        std::env::temp_dir()
    });
    dir.join("recast_index.db")
}

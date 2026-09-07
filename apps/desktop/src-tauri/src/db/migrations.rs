//! Schema migrations applied in order and tracked by SQLite's `user_version`.
//! APPEND-ONLY: a shipped step has already run on user machines, so add a new one rather than editing it.

use rusqlite::Connection;

/// Ordered DDL steps. After step at index `i` runs, `user_version` becomes `i + 1`.
const MIGRATIONS: &[&str] = &[
    // v1: one row per export job; the heavy render-state payload lives in a file at `payload_path`, never in the DB.
    "CREATE TABLE export_jobs (
        id           TEXT PRIMARY KEY,
        filename     TEXT NOT NULL,
        source_path  TEXT NOT NULL,
        status       TEXT NOT NULL,
        phase        TEXT NOT NULL,
        progress     REAL NOT NULL DEFAULT 0,
        output_path  TEXT,
        error        TEXT,
        payload_path TEXT NOT NULL,
        created_at   INTEGER NOT NULL,
        started_at   INTEGER,
        finished_at  INTEGER
     );
     CREATE INDEX idx_export_jobs_created ON export_jobs(created_at);",
];

/// Apply every migration newer than the connection's current `user_version`.
/// Idempotent: a fully-migrated DB runs no steps.
pub fn run(conn: &Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    let current = current.max(0) as usize;
    for (i, ddl) in MIGRATIONS.iter().enumerate().skip(current) {
        conn.execute_batch(ddl)?;
        // `user_version` can't be bound as a parameter, and the value is a trusted in-code constant.
        conn.pragma_update(None, "user_version", (i + 1) as i64)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(conn: &Connection) -> i64 {
        conn.query_row("PRAGMA user_version", [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn migrates_fresh_db_to_head() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        assert_eq!(version(&conn) as usize, MIGRATIONS.len());
        // The v1 table exists and is queryable.
        let n: i64 = conn
            .query_row("SELECT count(*) FROM export_jobs", [], |r| r.get(0))
            .unwrap();
        assert_eq!(n, 0);
    }

    #[test]
    fn run_is_idempotent() {
        let conn = Connection::open_in_memory().unwrap();
        run(&conn).unwrap();
        // Second run applies nothing and must not error on the existing table.
        run(&conn).unwrap();
        assert_eq!(version(&conn) as usize, MIGRATIONS.len());
    }
}

//! Storage crate: SQLite connection setup, migrations, and (later) repositories.
//!
//! Enforces the invariants from PROJECT_REQUIREMENTS.md §7: `PRAGMA foreign_keys = ON`,
//! WAL journal mode, and forward-only versioned migrations tracked in `schema_migrations`.
//! Large binaries (models, vectors) live on the filesystem; SQLite holds metadata only.

use std::path::Path;

use rusqlite::Connection;
use thiserror::Error;

pub mod migrations;

pub use migrations::{current_version, MigrationReport};

/// Errors surfaced by the storage layer.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Underlying SQLite error.
    #[error("sqlite error: {0}")]
    Sqlite(#[from] rusqlite::Error),
    /// A migration could not be applied or failed validation.
    #[error("migration error: {0}")]
    Migration(String),
}

/// An open SQLite database with the app's required pragmas applied.
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Open (creating if absent) a database file at `path` and configure pragmas.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, StorageError> {
        let conn = Connection::open(path)?;
        configure(&conn)?;
        Ok(Self { conn })
    }

    /// Open an in-memory database (used by tests). WAL is a no-op for `:memory:`.
    pub fn open_in_memory() -> Result<Self, StorageError> {
        let conn = Connection::open_in_memory()?;
        configure(&conn)?;
        Ok(Self { conn })
    }

    /// Apply all pending migrations, returning a report of what changed.
    pub fn migrate(&mut self) -> Result<MigrationReport, StorageError> {
        migrations::run(&mut self.conn)
    }

    /// The current schema version (highest applied migration, or 0).
    pub fn schema_version(&self) -> Result<i64, StorageError> {
        migrations::current_version(&self.conn)
    }

    /// Borrow the underlying connection (for repositories built in later phases).
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Whether foreign-key enforcement is currently on.
    pub fn foreign_keys_enabled(&self) -> Result<bool, StorageError> {
        let enabled: bool = self
            .conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))?;
        Ok(enabled)
    }
}

/// Apply the required connection pragmas (§7): foreign keys on + WAL journal mode.
fn configure(conn: &Connection) -> Result<(), StorageError> {
    conn.pragma_update(None, "foreign_keys", true)?;
    // journal_mode returns the resulting mode; ignore the row, surface real errors.
    let _mode: String = conn.query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foreign_keys_are_enabled() {
        let db = Database::open_in_memory().unwrap();
        assert!(db.foreign_keys_enabled().unwrap());
    }

    #[test]
    fn migrations_apply_from_empty_then_are_idempotent() {
        let mut db = Database::open_in_memory().unwrap();
        assert_eq!(db.schema_version().unwrap(), 0);

        let first = db.migrate().unwrap();
        assert_eq!(first.applied, vec![1]);
        assert_eq!(first.current_version, 1);
        assert_eq!(db.schema_version().unwrap(), 1);

        // Running again applies nothing (forward-only, idempotent).
        let second = db.migrate().unwrap();
        assert!(second.applied.is_empty());
        assert_eq!(second.current_version, 1);
    }

    #[test]
    fn migrations_persist_across_reopen_of_file_db() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("app.sqlite");

        {
            let mut db = Database::open(&path).unwrap();
            let report = db.migrate().unwrap();
            assert_eq!(report.current_version, 1);
        }

        // Reopen: schema version is remembered; migrating again is a no-op.
        let mut reopened = Database::open(&path).unwrap();
        assert_eq!(reopened.schema_version().unwrap(), 1);
        assert!(reopened.migrate().unwrap().applied.is_empty());
    }

    #[test]
    fn checksum_drift_is_detected() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();

        // Simulate a shipped migration whose SQL was later edited by tampering with
        // the recorded checksum: the next run must refuse to proceed.
        db.conn
            .execute(
                "UPDATE schema_migrations SET checksum = 'deadbeef' WHERE version = 1",
                [],
            )
            .unwrap();

        let err = db.migrate().unwrap_err();
        assert!(matches!(err, StorageError::Migration(_)));
    }

    #[test]
    fn schema_migrations_table_records_metadata() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();

        let (name, checksum): (String, String) = db
            .conn
            .query_row(
                "SELECT name, checksum FROM schema_migrations WHERE version = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert_eq!(name, "init");
        assert_eq!(checksum.len(), 64); // hex-encoded SHA-256
    }
}

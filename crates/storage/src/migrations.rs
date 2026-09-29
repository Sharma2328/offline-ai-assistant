//! Forward-only, versioned migration runner (PROJECT_REQUIREMENTS.md §7.4).
//!
//! Migrations are embedded at compile time and applied in a transaction. Each applied
//! migration is recorded in `schema_migrations` with a SHA-256 checksum so that a later
//! run detects tampering/drift (an already-applied migration whose SQL changed) and a
//! database created by a newer build (an applied version this binary does not know).

use std::collections::BTreeMap;

use rusqlite::{params, Connection};
use sha2::{Digest, Sha256};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

use crate::StorageError;

/// A single embedded migration.
struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

/// The ordered set of embedded migrations. Add new entries with strictly increasing
/// versions; never edit the SQL of an already-shipped migration (checksums are enforced).
const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "init",
        sql: include_str!("../migrations/0001_init.sql"),
    },
    Migration {
        version: 2,
        name: "app_settings",
        sql: include_str!("../migrations/0002_app_settings.sql"),
    },
    Migration {
        version: 3,
        name: "models",
        sql: include_str!("../migrations/0003_models.sql"),
    },
    Migration {
        version: 4,
        name: "conversations",
        sql: include_str!("../migrations/0004_conversations.sql"),
    },
    Migration {
        version: 5,
        name: "documents",
        sql: include_str!("../migrations/0005_documents.sql"),
    },
    Migration {
        version: 6,
        name: "citations",
        sql: include_str!("../migrations/0006_citations.sql"),
    },
    Migration {
        version: 7,
        name: "benchmarks",
        sql: include_str!("../migrations/0007_benchmarks.sql"),
    },
    Migration {
        version: 8,
        name: "diagnostics",
        sql: include_str!("../migrations/0008_diagnostics.sql"),
    },
];

/// Summary of a migration run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    /// Versions applied during this run (empty if already up to date).
    pub applied: Vec<i64>,
    /// The schema version after the run (highest applied version, or 0 if none).
    pub current_version: i64,
}

fn checksum(sql: &str) -> String {
    let digest = Sha256::digest(sql.as_bytes());
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use std::fmt::Write as _;
        // Infallible write into a String.
        let _ = write!(out, "{byte:02x}");
    }
    out
}

pub(crate) fn now_iso8601() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

fn ensure_bookkeeping(conn: &Connection) -> Result<(), StorageError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version    INTEGER PRIMARY KEY,
            name       TEXT NOT NULL,
            applied_at TEXT NOT NULL,
            checksum   TEXT NOT NULL
        );",
    )?;
    Ok(())
}

fn load_applied(conn: &Connection) -> Result<BTreeMap<i64, String>, StorageError> {
    let mut stmt = conn.prepare("SELECT version, checksum FROM schema_migrations")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = BTreeMap::new();
    for row in rows {
        let (version, checksum) = row?;
        map.insert(version, checksum);
    }
    Ok(map)
}

fn validate_embedded_ordering() -> Result<(), StorageError> {
    let mut previous = 0i64;
    for migration in MIGRATIONS {
        if migration.version <= previous {
            return Err(StorageError::Migration(format!(
                "embedded migrations are not strictly increasing (version {} after {})",
                migration.version, previous
            )));
        }
        previous = migration.version;
    }
    Ok(())
}

/// Apply all pending migrations. Idempotent: a second call is a no-op.
pub fn run(conn: &mut Connection) -> Result<MigrationReport, StorageError> {
    validate_embedded_ordering()?;
    ensure_bookkeeping(conn)?;

    let applied = load_applied(conn)?;

    // A database written by a newer build than this binary must not be silently used.
    let known: BTreeMap<i64, ()> = MIGRATIONS.iter().map(|m| (m.version, ())).collect();
    for version in applied.keys() {
        if !known.contains_key(version) {
            return Err(StorageError::Migration(format!(
                "database has applied migration {version} unknown to this build (downgrade?)"
            )));
        }
    }

    let mut newly_applied = Vec::new();
    for migration in MIGRATIONS {
        let expected = checksum(migration.sql);
        match applied.get(&migration.version) {
            Some(recorded) if recorded == &expected => continue,
            Some(_) => {
                return Err(StorageError::Migration(format!(
                    "checksum mismatch for migration {} ({}): already-applied SQL changed",
                    migration.version, migration.name
                )));
            }
            None => {
                let tx = conn.transaction()?;
                tx.execute_batch(migration.sql)?;
                tx.execute(
                    "INSERT INTO schema_migrations (version, name, applied_at, checksum)
                     VALUES (?1, ?2, ?3, ?4)",
                    params![migration.version, migration.name, now_iso8601(), expected],
                )?;
                tx.commit()?;
                newly_applied.push(migration.version);
            }
        }
    }

    Ok(MigrationReport {
        applied: newly_applied,
        current_version: current_version(conn)?,
    })
}

/// The highest applied migration version, or 0 if none have been applied.
pub fn current_version(conn: &Connection) -> Result<i64, StorageError> {
    ensure_bookkeeping(conn)?;
    let version: i64 = conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
        [],
        |row| row.get(0),
    )?;
    Ok(version)
}

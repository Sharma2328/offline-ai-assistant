//! `app_settings` key/value repository (PROJECT_REQUIREMENTS.md §7.2).
//!
//! Values are stored as JSON text (`value_json`); callers own (de)serialization so a
//! setting can be a bool, string, number, or small object. Used for the offline lock,
//! theme/accessibility, onboarding-complete flag, and score preset (FR-SET-*, FR-ONB-003).

use rusqlite::{params, Connection, OptionalExtension};

use crate::migrations::now_iso8601;
use crate::StorageError;

/// Thin repository over the `app_settings` table, bound to a live connection.
pub struct SettingsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> SettingsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Read a single setting's raw JSON value, or `None` if unset.
    pub fn get(&self, key: &str) -> Result<Option<String>, StorageError> {
        let value = self
            .conn
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        Ok(value)
    }

    /// Read every setting as `(key, value_json)` pairs, ordered by key.
    pub fn get_all(&self) -> Result<Vec<(String, String)>, StorageError> {
        let mut stmt = self
            .conn
            .prepare("SELECT key, value_json FROM app_settings ORDER BY key")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Insert or overwrite a setting's JSON value.
    pub fn set(&self, key: &str, value_json: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json,
                                            updated_at = excluded.updated_at",
            params![key, value_json, now_iso8601()],
        )?;
        Ok(())
    }

    /// Set a value only if the key is currently unset. Returns `true` when it was written.
    /// Used to seed defaults (e.g. offline-lock-on) without clobbering user choices.
    pub fn set_if_absent(&self, key: &str, value_json: &str) -> Result<bool, StorageError> {
        let changed = self.conn.execute(
            "INSERT OR IGNORE INTO app_settings (key, value_json, updated_at)
             VALUES (?1, ?2, ?3)",
            params![key, value_json, now_iso8601()],
        )?;
        Ok(changed == 1)
    }
}

#[cfg(test)]
mod tests {
    use crate::Database;

    fn migrated_db() -> Database {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn set_then_get_roundtrips() {
        let db = migrated_db();
        let repo = db.settings();
        repo.set("theme", "\"dark\"").unwrap();
        assert_eq!(repo.get("theme").unwrap().as_deref(), Some("\"dark\""));
    }

    #[test]
    fn get_missing_key_is_none() {
        let db = migrated_db();
        assert_eq!(db.settings().get("nope").unwrap(), None);
    }

    #[test]
    fn set_overwrites_existing_value() {
        let db = migrated_db();
        let repo = db.settings();
        repo.set("offline_lock", "true").unwrap();
        repo.set("offline_lock", "false").unwrap();
        assert_eq!(repo.get("offline_lock").unwrap().as_deref(), Some("false"));
    }

    #[test]
    fn set_if_absent_does_not_clobber() {
        let db = migrated_db();
        let repo = db.settings();
        assert!(repo.set_if_absent("offline_lock", "true").unwrap());
        assert!(!repo.set_if_absent("offline_lock", "false").unwrap());
        assert_eq!(repo.get("offline_lock").unwrap().as_deref(), Some("true"));
    }

    #[test]
    fn get_all_is_sorted_by_key() {
        let db = migrated_db();
        let repo = db.settings();
        repo.set("b", "1").unwrap();
        repo.set("a", "2").unwrap();
        let all = repo.get_all().unwrap();
        assert_eq!(all[0].0, "a");
        assert_eq!(all[1].0, "b");
    }
}

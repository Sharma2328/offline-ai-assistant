//! `models` and `runtime_profiles` repository (PROJECT_REQUIREMENTS.md §7.2, FR-MOD-*).
//!
//! Stores model **metadata** only — the model bytes live on the filesystem. Records use
//! primitive column types; the `app-core` layer maps them to/from the typed contract
//! (`StorageMode`, `RuntimeProfile`, `ModelRow`) so this crate stays free of higher-level
//! concerns. Inserting a model and its default runtime profile is atomic (one transaction).

use rusqlite::{params, Connection, OptionalExtension, Row};

use crate::migrations::now_iso8601;
use crate::StorageError;

/// A model row to insert (`created_at`/`last_used_at` are managed by the repository).
#[derive(Debug, Clone)]
pub struct NewModel {
    pub id: String,
    pub name: String,
    pub file_uri: String,
    pub storage_mode: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub format: String,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub metadata_json: String,
}

/// A persisted model row (all columns of the `models` table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelRecord {
    pub id: String,
    pub name: String,
    pub file_uri: String,
    pub storage_mode: String,
    pub sha256: String,
    pub size_bytes: i64,
    pub format: String,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub metadata_json: String,
    pub last_used_at: Option<String>,
    pub created_at: String,
}

/// A runtime profile to insert (`model_id`/`created_at` are supplied by the repository).
#[derive(Debug, Clone)]
pub struct NewRuntimeProfile {
    pub id: String,
    pub engine: String,
    pub context_length: i64,
    pub threads: i64,
    pub gpu_layers: i64,
    pub batch_size: i64,
    pub generation_defaults_json: String,
    pub is_default: bool,
}

/// A persisted runtime-profile row (all columns of the `runtime_profiles` table).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeProfileRecord {
    pub id: String,
    pub model_id: String,
    pub engine: String,
    pub context_length: i64,
    pub threads: i64,
    pub gpu_layers: i64,
    pub batch_size: i64,
    pub generation_defaults_json: String,
    pub is_default: bool,
    pub created_at: String,
}

/// Mutable fields of a runtime profile (`id`/`model_id`/`is_default`/`created_at` are fixed).
#[derive(Debug, Clone)]
pub struct ProfileUpdate<'a> {
    pub id: &'a str,
    pub engine: &'a str,
    pub context_length: i64,
    pub threads: i64,
    pub gpu_layers: i64,
    pub batch_size: i64,
    pub generation_defaults_json: &'a str,
}

/// Thin repository over `models` + `runtime_profiles`, bound to a live connection.
pub struct ModelsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ModelsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    /// Insert a model together with its default runtime profile, atomically. Both rows
    /// share `created_at`; the profile's `model_id` is the model's id.
    pub fn insert_with_default_profile(
        &self,
        model: &NewModel,
        profile: &NewRuntimeProfile,
    ) -> Result<(), StorageError> {
        let now = now_iso8601();
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO models
                (id, name, file_uri, storage_mode, sha256, size_bytes, format,
                 architecture, quantization, metadata_json, last_used_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, NULL, ?11)",
            params![
                model.id,
                model.name,
                model.file_uri,
                model.storage_mode,
                model.sha256,
                model.size_bytes,
                model.format,
                model.architecture,
                model.quantization,
                model.metadata_json,
                now,
            ],
        )?;
        tx.execute(
            "INSERT INTO runtime_profiles
                (id, model_id, engine, context_length, threads, gpu_layers, batch_size,
                 generation_defaults_json, is_default, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                profile.id,
                model.id,
                profile.engine,
                profile.context_length,
                profile.threads,
                profile.gpu_layers,
                profile.batch_size,
                profile.generation_defaults_json,
                profile.is_default,
                now,
            ],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// All models, most-recently-used first, then newest imported (FR-MOD-002).
    pub fn list(&self) -> Result<Vec<ModelRecord>, StorageError> {
        let mut stmt = self.conn.prepare(
            "SELECT id, name, file_uri, storage_mode, sha256, size_bytes, format,
                    architecture, quantization, metadata_json, last_used_at, created_at
             FROM models
             ORDER BY last_used_at IS NULL, last_used_at DESC, created_at DESC",
        )?;
        let rows = stmt.query_map([], map_model)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// A single model by id, or `None` if absent.
    pub fn get(&self, id: &str) -> Result<Option<ModelRecord>, StorageError> {
        let record = self
            .conn
            .query_row(
                "SELECT id, name, file_uri, storage_mode, sha256, size_bytes, format,
                        architecture, quantization, metadata_json, last_used_at, created_at
                 FROM models WHERE id = ?1",
                params![id],
                map_model,
            )
            .optional()?;
        Ok(record)
    }

    /// A model with a matching checksum, or `None` (used for import de-duplication).
    pub fn find_by_sha256(&self, sha256: &str) -> Result<Option<ModelRecord>, StorageError> {
        let record = self
            .conn
            .query_row(
                "SELECT id, name, file_uri, storage_mode, sha256, size_bytes, format,
                        architecture, quantization, metadata_json, last_used_at, created_at
                 FROM models WHERE sha256 = ?1",
                params![sha256],
                map_model,
            )
            .optional()?;
        Ok(record)
    }

    /// The default runtime profile for a model, or `None`.
    pub fn default_profile(
        &self,
        model_id: &str,
    ) -> Result<Option<RuntimeProfileRecord>, StorageError> {
        let record = self
            .conn
            .query_row(
                "SELECT id, model_id, engine, context_length, threads, gpu_layers, batch_size,
                        generation_defaults_json, is_default, created_at
                 FROM runtime_profiles WHERE model_id = ?1 AND is_default = 1",
                params![model_id],
                map_profile,
            )
            .optional()?;
        Ok(record)
    }

    /// A runtime profile by id, or `None`.
    pub fn profile(&self, id: &str) -> Result<Option<RuntimeProfileRecord>, StorageError> {
        let record = self
            .conn
            .query_row(
                "SELECT id, model_id, engine, context_length, threads, gpu_layers, batch_size,
                        generation_defaults_json, is_default, created_at
                 FROM runtime_profiles WHERE id = ?1",
                params![id],
                map_profile,
            )
            .optional()?;
        Ok(record)
    }

    /// Update a runtime profile's mutable fields. Returns `true` if a row was updated
    /// (`id`/`model_id`/`is_default`/`created_at` are never changed here).
    pub fn update_profile(&self, update: &ProfileUpdate<'_>) -> Result<bool, StorageError> {
        let changed = self.conn.execute(
            "UPDATE runtime_profiles
             SET engine = ?2, context_length = ?3, threads = ?4, gpu_layers = ?5,
                 batch_size = ?6, generation_defaults_json = ?7
             WHERE id = ?1",
            params![
                update.id,
                update.engine,
                update.context_length,
                update.threads,
                update.gpu_layers,
                update.batch_size,
                update.generation_defaults_json,
            ],
        )?;
        Ok(changed == 1)
    }

    /// Delete a model (its runtime profiles cascade). Returns the deleted record so the
    /// caller can act on `storage_mode`/`file_uri` (FR-MOD-006), or `None` if absent.
    pub fn remove(&self, id: &str) -> Result<Option<ModelRecord>, StorageError> {
        let record = self.get(id)?;
        if record.is_some() {
            self.conn
                .execute("DELETE FROM models WHERE id = ?1", params![id])?;
        }
        Ok(record)
    }
}

fn map_model(row: &Row<'_>) -> rusqlite::Result<ModelRecord> {
    Ok(ModelRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        file_uri: row.get(2)?,
        storage_mode: row.get(3)?,
        sha256: row.get(4)?,
        size_bytes: row.get(5)?,
        format: row.get(6)?,
        architecture: row.get(7)?,
        quantization: row.get(8)?,
        metadata_json: row.get(9)?,
        last_used_at: row.get(10)?,
        created_at: row.get(11)?,
    })
}

fn map_profile(row: &Row<'_>) -> rusqlite::Result<RuntimeProfileRecord> {
    Ok(RuntimeProfileRecord {
        id: row.get(0)?,
        model_id: row.get(1)?,
        engine: row.get(2)?,
        context_length: row.get(3)?,
        threads: row.get(4)?,
        gpu_layers: row.get(5)?,
        batch_size: row.get(6)?,
        generation_defaults_json: row.get(7)?,
        is_default: row.get(8)?,
        created_at: row.get(9)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    fn migrated_db() -> Database {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();
        db
    }

    fn sample_model(id: &str, sha: &str) -> NewModel {
        NewModel {
            id: id.to_string(),
            name: format!("model-{id}"),
            file_uri: format!("/models/{id}.gguf"),
            storage_mode: "reference".to_string(),
            sha256: sha.to_string(),
            size_bytes: 4_000_000_000,
            format: "gguf".to_string(),
            architecture: Some("llama".to_string()),
            quantization: Some("Q4_K_M".to_string()),
            metadata_json: "{}".to_string(),
        }
    }

    fn sample_profile(id: &str) -> NewRuntimeProfile {
        NewRuntimeProfile {
            id: id.to_string(),
            engine: "llama.cpp".to_string(),
            context_length: 4096,
            threads: 8,
            gpu_layers: 999,
            batch_size: 512,
            generation_defaults_json: "{}".to_string(),
            is_default: true,
        }
    }

    #[test]
    fn insert_creates_model_and_default_profile() {
        let db = migrated_db();
        let repo = db.models();
        repo.insert_with_default_profile(&sample_model("m1", "sha-1"), &sample_profile("p1"))
            .unwrap();

        let got = repo.get("m1").unwrap().unwrap();
        assert_eq!(got.name, "model-m1");
        assert_eq!(got.storage_mode, "reference");

        let profile = repo.default_profile("m1").unwrap().unwrap();
        assert_eq!(profile.id, "p1");
        assert_eq!(profile.context_length, 4096);
        assert!(profile.is_default);
    }

    #[test]
    fn duplicate_sha256_is_rejected() {
        let db = migrated_db();
        let repo = db.models();
        repo.insert_with_default_profile(&sample_model("m1", "dup"), &sample_profile("p1"))
            .unwrap();
        let err = repo
            .insert_with_default_profile(&sample_model("m2", "dup"), &sample_profile("p2"))
            .unwrap_err();
        assert!(matches!(err, StorageError::Sqlite(_)));
        // The failed insert rolled back — only the first model exists.
        assert!(repo.find_by_sha256("dup").unwrap().is_some());
        assert!(repo.get("m2").unwrap().is_none());
    }

    #[test]
    fn find_by_sha256_matches_import() {
        let db = migrated_db();
        let repo = db.models();
        repo.insert_with_default_profile(&sample_model("m1", "abc"), &sample_profile("p1"))
            .unwrap();
        assert_eq!(repo.find_by_sha256("abc").unwrap().unwrap().id, "m1");
        assert!(repo.find_by_sha256("nope").unwrap().is_none());
    }

    #[test]
    fn remove_returns_record_and_cascades_profiles() {
        let db = migrated_db();
        let repo = db.models();
        repo.insert_with_default_profile(&sample_model("m1", "abc"), &sample_profile("p1"))
            .unwrap();

        let removed = repo.remove("m1").unwrap().unwrap();
        assert_eq!(removed.storage_mode, "reference");
        assert!(repo.get("m1").unwrap().is_none());
        assert!(repo.default_profile("m1").unwrap().is_none());
    }

    #[test]
    fn remove_absent_model_is_none() {
        let db = migrated_db();
        assert!(db.models().remove("ghost").unwrap().is_none());
    }

    #[test]
    fn update_profile_changes_mutable_fields() {
        let db = migrated_db();
        let repo = db.models();
        repo.insert_with_default_profile(&sample_model("m1", "abc"), &sample_profile("p1"))
            .unwrap();

        let changed = repo
            .update_profile(&ProfileUpdate {
                id: "p1",
                engine: "llama.cpp",
                context_length: 8192,
                threads: 4,
                gpu_layers: 0,
                batch_size: 256,
                generation_defaults_json: "{\"temperature\":0.2}",
            })
            .unwrap();
        assert!(changed);

        let profile = repo.default_profile("m1").unwrap().unwrap();
        assert_eq!(profile.context_length, 8192);
        assert_eq!(profile.gpu_layers, 0);
        assert_eq!(profile.generation_defaults_json, "{\"temperature\":0.2}");
    }

    #[test]
    fn list_orders_recently_used_first() {
        let db = migrated_db();
        let repo = db.models();
        repo.insert_with_default_profile(&sample_model("m1", "s1"), &sample_profile("p1"))
            .unwrap();
        repo.insert_with_default_profile(&sample_model("m2", "s2"), &sample_profile("p2"))
            .unwrap();
        // Give m1 a last_used_at so it sorts ahead of the never-used m2.
        db.connection()
            .execute(
                "UPDATE models SET last_used_at = '2030-01-01T00:00:00Z' WHERE id = 'm1'",
                [],
            )
            .unwrap();

        let ids: Vec<String> = repo.list().unwrap().into_iter().map(|m| m.id).collect();
        assert_eq!(ids, vec!["m1".to_string(), "m2".to_string()]);
    }
}

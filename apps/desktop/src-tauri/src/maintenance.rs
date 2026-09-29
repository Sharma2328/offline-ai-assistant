use crate::{lock_db, Db};
use app_core::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Manager, State};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub path: String,
    pub database_bytes: u64,
    pub conversation_bytes: u64,
    pub index_bytes: u64,
    pub benchmark_bytes: u64,
    pub managed_model_bytes: u64,
    pub conversations: u32,
    pub documents: u32,
    pub benchmark_runs: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Diagnostic {
    pub id: i64,
    pub code: String,
    pub message: String,
    pub created_at: String,
}
fn err(error: impl ToString) -> AppError {
    AppError::internal(error.to_string())
}

/// Log fixed lifecycle summaries only. Never pass prompts, model output, documents, or raw errors.
pub fn log(app: &tauri::AppHandle, code: &str, message: &str) {
    if let Ok(db) = app.state::<Db>().lock() {
        let _ = db.connection().execute(
            "INSERT INTO diagnostic_logs(code,message) VALUES(?1,?2)",
            rusqlite::params![code, message],
        );
    }
}
#[tauri::command]
#[specta::specta]
pub fn storage_usage(app: tauri::AppHandle, db: State<'_, Db>) -> AppResult<StorageUsage> {
    let directory = app.path().app_data_dir().map_err(err)?;
    let db = lock_db(&db)?;
    let count = |table: &str| {
        db.connection()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| {
                r.get::<_, u32>(0)
            })
            .map_err(err)
    };
    let managed_model_bytes = db
        .models()
        .list()
        .map_err(err)?
        .iter()
        .filter(|m| m.storage_mode == "managed")
        .filter_map(|m| std::fs::metadata(&m.file_uri).ok())
        .map(|m| m.len())
        .sum();
    let database_bytes = ["app.sqlite", "app.sqlite-wal", "app.sqlite-shm"]
        .iter()
        .filter_map(|name| std::fs::metadata(directory.join(name)).ok())
        .map(|m| m.len())
        .sum();
    // Logical UTF-8 payload sizes; total database bytes above include page/index overhead.
    let payload_bytes = |sql: &str| {
        db.connection()
            .query_row(sql, [], |row| row.get::<_, u64>(0))
            .map_err(err)
    };
    let conversation_bytes =
        payload_bytes("SELECT COALESCE(SUM(length(CAST(content AS BLOB))),0) FROM messages")?;
    let index_bytes=payload_bytes("SELECT COALESCE(SUM(length(CAST(text AS BLOB))+length(CAST(vector_json AS BLOB))),0) FROM document_chunks")?;
    let benchmark_bytes = payload_bytes(
        "SELECT COALESCE(SUM(length(CAST(result_json AS BLOB))),0) FROM benchmark_case_results",
    )?;
    Ok(StorageUsage {
        path: directory.to_string_lossy().to_string(),
        database_bytes,
        conversation_bytes,
        index_bytes,
        benchmark_bytes,
        managed_model_bytes,
        conversations: count("conversations")?,
        documents: count("documents")?,
        benchmark_runs: count("benchmark_runs")?,
    })
}
#[tauri::command]
#[specta::specta]
pub fn diagnostics_list(db: State<'_, Db>) -> AppResult<Vec<Diagnostic>> {
    let db = lock_db(&db)?;
    let mut statement = db
        .connection()
        .prepare(
            "SELECT id,code,message,created_at FROM diagnostic_logs ORDER BY id DESC LIMIT 200",
        )
        .map_err(err)?;
    let rows = statement
        .query_map([], |r| {
            Ok(Diagnostic {
                id: r.get(0)?,
                code: r.get(1)?,
                message: r.get(2)?,
                created_at: r.get(3)?,
            })
        })
        .map_err(err)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(err)
}
#[tauri::command]
#[specta::specta]
pub fn diagnostics_clear(db: State<'_, Db>) -> AppResult<()> {
    let db = lock_db(&db)?;
    db.connection()
        .execute("DELETE FROM diagnostic_logs", [])
        .map_err(err)?;
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub fn diagnostics_export(db: State<'_, Db>, path: String) -> AppResult<()> {
    let logs = diagnostics_list(db)?;
    if std::path::Path::new(&path)
        .extension()
        .and_then(|p| p.to_str())
        != Some("json")
    {
        return Err(err("Save diagnostics as a .json file."));
    }
    std::fs::write(
        path,
        serde_json::to_string_pretty(
            &serde_json::json!({"includesContent":false,"redacted":true,"logs":logs}),
        )
        .map_err(err)?,
    )
    .map_err(err)
}
#[tauri::command]
#[specta::specta]
pub async fn storage_delete_all(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    runtime: State<'_, crate::runtime::RuntimeState>,
    documents: State<'_, crate::document_commands::DocumentState>,
    confirmation: String,
) -> AppResult<()> {
    if confirmation != "DELETE" {
        return Err(err(
            "Type DELETE to confirm removing local application data.",
        ));
    }
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| err("Stop chat or benchmarks before deleting data."))?;
    let _document_operation = documents
        .operation
        .try_lock()
        .map_err(|_| err("Stop document indexing before deleting data."))?;
    crate::document_commands::shutdown(&documents).await?;
    if let Ok(adapter) = crate::runtime::current_adapter(&runtime).await {
        if let Some(id) = inference::InferenceAdapter::loaded_model_id(adapter.as_ref()).await {
            inference::InferenceAdapter::unload_model(adapter.as_ref(), &id)
                .await
                .map_err(app_core::map_inference_error)?;
        }
    }
    let db = lock_db(&db)?;
    let managed = db
        .models()
        .list()
        .map_err(err)?
        .into_iter()
        .filter(|m| m.storage_mode == "managed")
        .map(|m| m.file_uri)
        .collect::<Vec<_>>();
    let tx = db.connection().unchecked_transaction().map_err(err)?;
    tx.execute_batch("DELETE FROM conversations; DELETE FROM document_collections; DELETE FROM benchmark_runs; DELETE FROM models; DELETE FROM diagnostic_logs; DELETE FROM app_settings WHERE key NOT IN ('offline_lock','runtime_binary_path','onboarding_complete');").map_err(err)?;
    tx.commit().map_err(err)?;
    let root = app.path().app_data_dir().map_err(err)?.join("models");
    if root.exists() {
        let root = root.canonicalize().map_err(err)?;
        for file in managed {
            let path = std::path::Path::new(&file);
            if path.exists() {
                let canonical = path.canonicalize().map_err(err)?;
                if !canonical.starts_with(&root) {
                    return Err(err("Refused to delete a model outside managed storage."));
                }
                std::fs::remove_file(canonical).map_err(err)?;
            }
        }
    }
    db.connection()
        .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); VACUUM;")
        .map_err(err)?;
    Ok(())
}

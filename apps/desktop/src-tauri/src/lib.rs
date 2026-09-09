//! Tauri host library: registers typed commands, generates TS bindings, runs startup
//! migrations, and launches the window. Domain logic lives in `app-core`/`crates/*`;
//! this crate only adapts them to Tauri (PROJECT_REQUIREMENTS.md §6, §8).

use std::sync::Mutex;

use app_core::{inspect, AppError, AppErrorCode, AppResult, SystemInspection, APP_VERSION};
use serde::{Deserialize, Serialize};
use specta::Type;
use storage::Database;
use tauri::{Manager, State};
use tauri_specta::{collect_commands, Builder};

/// Managed application database, guarded for exclusive access across commands.
///
/// SQLite (with our WAL + foreign-keys pragmas) is single-writer; serializing command
/// access behind a `Mutex` keeps the connection sound without a pool for MVP (§7).
type Db = Mutex<Database>;

/// Setting keys seeded on first launch (§7.2, FR-SET-003, FR-ONB-003).
const KEY_OFFLINE_LOCK: &str = "offline_lock";
const KEY_ONBOARDING_COMPLETE: &str = "onboarding_complete";

/// A single `app_settings` entry exposed to the UI. `value` is the parsed JSON so the
/// frontend receives a real bool/string/number/object rather than a JSON string (§9).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct SettingEntry {
    key: String,
    value: serde_json::Value,
}

/// Map a storage-layer error onto the shared `AppError` contract.
fn storage_err(err: storage::StorageError) -> AppError {
    AppError::internal(format!("storage error: {err}"))
}

/// Lock the managed database, converting a poisoned mutex into an internal error.
fn lock_db<'a>(db: &'a State<'_, Db>) -> AppResult<std::sync::MutexGuard<'a, Database>> {
    db.lock()
        .map_err(|_| AppError::internal("database lock was poisoned by a previous panic"))
}

/// Return the backend application version (used by the shell to confirm connectivity).
#[tauri::command]
#[specta::specta]
fn app_version() -> String {
    APP_VERSION.to_string()
}

/// Liveness probe exercising the `AppError` contract end-to-end through the bindings.
#[tauri::command]
#[specta::specta]
fn ping() -> AppResult<String> {
    Ok("pong".to_string())
}

/// Inspect local hardware + runtime capabilities (FR-ONB-001, contract §9.1).
///
/// Purely local: hardware probing via `sysinfo`, no network access. The app-data volume
/// is used for free-disk reporting so the number reflects where models/indexes will live.
#[tauri::command]
#[specta::specta]
fn system_inspect(app: tauri::AppHandle) -> AppResult<SystemInspection> {
    let probe = app.path().app_data_dir().ok();
    Ok(inspect(probe.as_deref()))
}

/// Read settings: one entry when `key` is given, otherwise all entries (contract §9).
#[tauri::command]
#[specta::specta]
fn settings_get(db: State<'_, Db>, key: Option<String>) -> AppResult<Vec<SettingEntry>> {
    let db = lock_db(&db)?;
    let repo = db.settings();

    let raw: Vec<(String, String)> = match key {
        Some(key) => match repo.get(&key).map_err(storage_err)? {
            Some(value_json) => vec![(key, value_json)],
            None => Vec::new(),
        },
        None => repo.get_all().map_err(storage_err)?,
    };

    raw.into_iter()
        .map(|(key, value_json)| {
            let value = serde_json::from_str(&value_json).map_err(|e| {
                AppError::internal(format!("stored setting '{key}' is not valid JSON: {e}"))
            })?;
            Ok(SettingEntry { key, value })
        })
        .collect()
}

/// Write a single setting; `value` is stored as JSON text (contract §9, FR-SET-*).
#[tauri::command]
#[specta::specta]
fn settings_set(db: State<'_, Db>, key: String, value: serde_json::Value) -> AppResult<()> {
    let value_json = serde_json::to_string(&value)
        .map_err(|e| AppError::internal(format!("could not serialize setting value: {e}")))?;
    let db = lock_db(&db)?;
    db.settings().set(&key, &value_json).map_err(storage_err)?;
    Ok(())
}

/// Build the tauri-specta command/event registry (single source of truth for bindings).
fn specta_builder() -> Builder {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        app_version,
        ping,
        system_inspect,
        settings_get,
        settings_set
    ])
}

/// Open the app database, apply pending migrations, and seed first-run defaults (§7).
///
/// Returns the migrated database for the caller to place in managed state. Defaults are
/// seeded with `set_if_absent` so a returning user's choices are never clobbered:
/// the offline lock is **on** by default (FR-SET-003) and onboarding starts incomplete
/// (FR-ONB-003).
fn initialize_database(app: &tauri::App) -> Result<Database, Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("app.sqlite");

    let mut db = Database::open(&db_path).map_err(|e| {
        AppError::new(
            AppErrorCode::Internal,
            format!("failed to open database: {e}"),
            "Check available disk space and app-data permissions, then relaunch.",
        )
    })?;
    let report = db.migrate().map_err(|e| {
        AppError::new(
            AppErrorCode::Internal,
            format!("failed to apply migrations: {e}"),
            "Relaunch the app; if this persists, export diagnostics from Settings.",
        )
    })?;
    tracing::info!(
        version = report.current_version,
        applied = ?report.applied,
        "database migrations up to date"
    );

    {
        let settings = db.settings();
        settings
            .set_if_absent(KEY_OFFLINE_LOCK, "true")
            .map_err(storage_err)?;
        settings
            .set_if_absent(KEY_ONBOARDING_COMPLETE, "false")
            .map_err(storage_err)?;
    }

    Ok(db)
}

/// Launch the desktop application.
pub fn run() {
    let builder = specta_builder();

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            let db = initialize_database(app)?;
            app.manage::<Db>(Mutex::new(db));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Offline AI Assistant");
}

#[cfg(test)]
mod bindings_export {
    use super::specta_builder;
    use specta_typescript::{BigIntExportBehavior, Typescript};

    /// Regenerates `apps/desktop/src/lib/bindings.ts` from the Rust command registry.
    /// `scripts/generate-bindings.mjs` runs this test; CI fails if the output drifts.
    #[test]
    fn export_typescript_bindings() {
        // Generated file: skip lint + strict-TS checks (it contains tooling-owned `any`
        // casts and helper imports that may be unused when there are no events yet).
        // Byte-count fields are `u64`; Tauri serializes them as JSON numbers and every
        // real value (memory/disk/VRAM in bytes) is far below JS's 2^53 safe-integer
        // ceiling, so mapping bigints to `number` matches the wire format exactly.
        let exporter = Typescript::default()
            .bigint(BigIntExportBehavior::Number)
            .header("// @ts-nocheck\n/* eslint-disable */\n");
        specta_builder()
            .export(exporter, "../src/lib/bindings.ts")
            .expect("failed to export TypeScript bindings");
    }
}

//! Tauri host library: registers typed commands, generates TS bindings, runs startup
//! migrations, and launches the window. Domain logic lives in `app-core`/`crates/*`;
//! this crate only adapts them to Tauri (PROJECT_REQUIREMENTS.md §6, §8).

use app_core::{AppError, AppErrorCode, AppResult, APP_VERSION};
use tauri::Manager;
use tauri_specta::{collect_commands, Builder};

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

/// Build the tauri-specta command/event registry (single source of truth for bindings).
fn specta_builder() -> Builder {
    Builder::<tauri::Wry>::new().commands(collect_commands![app_version, ping])
}

/// Open the app database and apply pending migrations at startup (§7).
fn run_startup_migrations(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let data_dir = app.path().app_data_dir()?;
    std::fs::create_dir_all(&data_dir)?;
    let db_path = data_dir.join("app.sqlite");

    let mut db = storage::Database::open(&db_path).map_err(|e| {
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
    Ok(())
}

/// Launch the desktop application.
pub fn run() {
    let builder = specta_builder();

    tauri::Builder::default()
        .invoke_handler(builder.invoke_handler())
        .setup(|app| {
            run_startup_migrations(app)?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the Offline AI Assistant");
}

#[cfg(test)]
mod bindings_export {
    use super::specta_builder;
    use specta_typescript::Typescript;

    /// Regenerates `apps/desktop/src/lib/bindings.ts` from the Rust command registry.
    /// `scripts/generate-bindings.mjs` runs this test; CI fails if the output drifts.
    #[test]
    fn export_typescript_bindings() {
        // Generated file: skip lint + strict-TS checks (it contains tooling-owned `any`
        // casts and helper imports that may be unused when there are no events yet).
        let exporter = Typescript::default().header("// @ts-nocheck\n/* eslint-disable */\n");
        specta_builder()
            .export(exporter, "../src/lib/bindings.ts")
            .expect("failed to export TypeScript bindings");
    }
}

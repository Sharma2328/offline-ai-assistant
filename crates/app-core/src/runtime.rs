//! Runtime orchestration helpers (Phase 5, FR-MOD-004/005). These are the pure, testable
//! pieces that sit between the storage layer and the live [`inference`] adapter owned by the
//! Tauri host: resolving a model into a [`LoadRequest`], stamping "last used", and mapping
//! [`InferenceError`] onto the shared [`AppError`] contract. The adapter and its process state
//! live in the host (they need the Tauri `AppHandle` to emit events); nothing here spawns a
//! process or holds runtime state.

use std::path::PathBuf;

use inference::{InferenceError, LoadRequest};
use storage::Database;

use crate::error::{AppError, AppErrorCode, AppResult};
use crate::models::get_runtime_profile;

fn storage_err(err: storage::StorageError) -> AppError {
    AppError::internal(format!("storage error: {err}"))
}

fn model_not_found(id: &str) -> AppError {
    AppError::new(
        AppErrorCode::ModelInvalid,
        format!("model '{id}' was not found"),
        "Refresh the model library; the model may have been removed.",
    )
}

/// Resolve a model id into the adapter's [`LoadRequest`]: its on-disk path plus default profile.
///
/// Fails with `ModelInvalid` if the model is unknown or its file is missing (e.g. a referenced
/// source file was moved or deleted since import).
pub fn resolve_load_request(db: &Database, model_id: &str) -> AppResult<LoadRequest> {
    let record = db
        .models()
        .get(model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(model_id))?;

    let model_path = PathBuf::from(&record.file_uri);
    if !model_path.is_file() {
        return Err(AppError::new(
            AppErrorCode::ModelInvalid,
            format!("model file is missing: {}", record.file_uri),
            "The model file was moved or deleted. Re-import it, or remove it from the library.",
        ));
    }

    let profile = get_runtime_profile(db, model_id)?;
    Ok(LoadRequest {
        model_id: model_id.to_string(),
        model_path,
        profile,
    })
}

/// Stamp the model as most-recently-used (FR-MOD library ordering). Best-effort: a failure to
/// update the timestamp must not fail a successful load, so callers may ignore the result.
pub fn mark_model_used(db: &Database, model_id: &str) -> AppResult<()> {
    db.models().touch_last_used(model_id).map_err(storage_err)?;
    Ok(())
}

/// Map an [`InferenceError`] onto the shared [`AppError`] contract (§8.5, spec §17).
pub fn map_inference_error(err: InferenceError) -> AppError {
    match err {
        InferenceError::RuntimeUnavailable(msg) => AppError::new(
            AppErrorCode::RuntimeUnavailable,
            format!("inference runtime unavailable: {msg}"),
            "Install a llama.cpp `llama-server` binary and set its path in Settings, or add it \
             to your PATH.",
        ),
        InferenceError::StartupFailed(msg) => AppError::new(
            AppErrorCode::RuntimeUnavailable,
            format!("the inference runtime failed to start: {msg}"),
            "Check that the runtime binary is compatible with this machine, then try again.",
        ),
        InferenceError::Crashed(msg) => AppError::new(
            AppErrorCode::RuntimeCrashed,
            format!("the inference runtime crashed: {msg}"),
            "Reload the model and try again. If it persists, the model may be too large for this \
             device.",
        ),
        InferenceError::OutOfMemory(msg) => AppError::new(
            AppErrorCode::ModelOom,
            format!("not enough memory to load or run the model: {msg}"),
            "Choose a smaller or more heavily quantized model, or reduce the context length.",
        ),
        InferenceError::ContextExceeded(msg) => AppError::new(
            AppErrorCode::ContextExceeded,
            format!("the request exceeded the model's context window: {msg}"),
            "Shorten the conversation or increase the context length in the runtime profile.",
        ),
        InferenceError::NotLoaded(msg) => AppError::new(
            AppErrorCode::ModelInvalid,
            format!("no matching model is loaded: {msg}"),
            "Load the model before generating.",
        ),
        InferenceError::Unsupported(msg) => AppError::new(
            AppErrorCode::Internal,
            format!("operation not supported by this runtime: {msg}"),
            "This action is not available for the current runtime.",
        ),
        InferenceError::Protocol(msg) => {
            AppError::internal(format!("runtime communication error: {msg}"))
        }
    }
}

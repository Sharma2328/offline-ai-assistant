//! Host-side inference runtime: owns the live [`LlamaAdapter`], streams generation events to
//! the UI, and enforces one-loaded-model-at-a-time (PROJECT_REQUIREMENTS.md §5.5, Phase 5).
//!
//! This is the only place that couples the inference crate to Tauri: it emits typed events
//! (`chat:token` / `chat:done` / `chat:error` / `runtime:crashed`) and maps
//! [`inference::InferenceError`] onto the shared `AppError` via `app-core`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use app_core::{
    map_inference_error, mark_model_used, resolve_load_request, AppError, AppErrorCode, AppResult,
    GenerationRequest, GenerationResult, LoadedModel, RuntimeEngine, TokenEvent,
};
use inference::{resolve_llama_binary, CrashInfo, InferenceAdapter, LlamaAdapter};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{Manager, State};
use tauri_specta::Event;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::{lock_db, Db};

/// Setting key holding a user-configured path to the `llama-server` binary (FR-SET, Phase 5).
pub const KEY_RUNTIME_BINARY_PATH: &str = "runtime_binary_path";

/// `chat:token` — a single streamed token (FR-CHAT-002).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct ChatToken(pub TokenEvent);

/// `chat:done` — the finalized generation result (finish reason, timing).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct ChatDone(pub GenerationResult);

/// `chat:error` — a generation failed mid-stream.
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
pub struct ChatError(pub AppError);

/// `runtime:crashed` — the out-of-process runtime exited unexpectedly (NFR-REL-001).
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCrashed {
    pub engine: RuntimeEngine,
    pub model_id: String,
    pub code: Option<i32>,
}

/// The adapter currently in use, remembered alongside the binary path it was built for so we
/// can rebuild it if the user points at a different runtime.
struct AdapterEntry {
    binary: PathBuf,
    adapter: Arc<LlamaAdapter>,
}

/// Managed host runtime state.
#[derive(Default)]
pub struct RuntimeState {
    pub(crate) operation: tokio::sync::Mutex<()>,
    /// The live adapter (async mutex: held across `.await` during load/generate/unload).
    adapter: tokio::sync::Mutex<Option<AdapterEntry>>,
    /// In-flight generations, keyed by `correlationId`, so `chat.cancel` can stop them.
    cancels: Mutex<HashMap<String, CancellationToken>>,
}

impl RuntimeState {
    pub(crate) fn register_cancel(&self, correlation_id: &str) -> CancelRegistration<'_> {
        let token = self
            .cancels
            .lock()
            .expect("cancel registry lock")
            .entry(correlation_id.to_string())
            .or_default()
            .clone();
        CancelRegistration {
            state: self,
            id: correlation_id.to_string(),
            token,
        }
    }

    fn take_cancel(&self, correlation_id: &str) -> Option<CancellationToken> {
        self.cancels
            .lock()
            .expect("cancel registry lock")
            .remove(correlation_id)
    }
}

pub(crate) struct CancelRegistration<'a> {
    state: &'a RuntimeState,
    id: String,
    pub token: CancellationToken,
}
impl Drop for CancelRegistration<'_> {
    fn drop(&mut self) {
        self.state.take_cancel(&self.id);
    }
}

/// Read a string-valued setting, returning `None` if unset or not a JSON string.
pub(crate) fn read_string_setting(db: &State<'_, Db>, key: &str) -> AppResult<Option<String>> {
    let db = lock_db(db)?;
    let raw = db
        .settings()
        .get(key)
        .map_err(|e| AppError::internal(format!("storage error: {e}")))?;
    Ok(raw.and_then(|json| serde_json::from_str::<String>(&json).ok()))
}

/// Resolve (or rebuild) the shared llama adapter for the configured/`PATH` binary.
async fn get_or_create_adapter(
    app: &tauri::AppHandle,
    state: &State<'_, RuntimeState>,
    db: &State<'_, Db>,
) -> AppResult<Arc<LlamaAdapter>> {
    let configured = read_string_setting(db, KEY_RUNTIME_BINARY_PATH)?;
    let binary = resolve_llama_binary(configured.as_deref()).ok_or_else(|| {
        AppError::new(
            AppErrorCode::RuntimeUnavailable,
            "no llama.cpp runtime binary was found",
            "Install `llama-server` and add it to your PATH, or set its path in Settings.",
        )
    })?;

    let mut guard = state.adapter.lock().await;
    if let Some(entry) = guard.as_ref() {
        if entry.binary == binary {
            return Ok(entry.adapter.clone());
        }
    }

    if let Some(old) = guard.take() {
        old.adapter
            .unload_model("")
            .await
            .map_err(map_inference_error)?;
    }
    // Build a fresh adapter whose crash handler emits `runtime:crashed` on the UI thread.
    let app_for_crash = app.clone();
    let adapter = Arc::new(LlamaAdapter::new(
        binary.clone(),
        Arc::new(move |info: CrashInfo| {
            let _ = RuntimeCrashed {
                engine: info.engine,
                model_id: info.model_id,
                code: info.code,
            }
            .emit(&app_for_crash);
        }),
    ));
    *guard = Some(AdapterEntry {
        binary,
        adapter: adapter.clone(),
    });
    Ok(adapter)
}

/// Currently-loaded adapter, or an error if nothing has been loaded yet.
pub(crate) async fn current_adapter(
    state: &State<'_, RuntimeState>,
) -> AppResult<Arc<LlamaAdapter>> {
    state
        .adapter
        .lock()
        .await
        .as_ref()
        .map(|entry| entry.adapter.clone())
        .ok_or_else(|| {
            AppError::new(
                AppErrorCode::ModelInvalid,
                "no model is loaded",
                "Load a model before generating.",
            )
        })
}

/// Load a model into the runtime (FR-MOD-005). Replaces any currently-loaded model.
#[tauri::command]
#[specta::specta]
pub async fn models_load(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
    db: State<'_, Db>,
    model_id: String,
) -> AppResult<LoadedModel> {
    let _operation = state.operation.try_lock().map_err(|_| {
        AppError::internal("Stop the current operation before loading another model.")
    })?;
    // Resolve DB-backed inputs first, releasing the DB lock before any await.
    let load_request = {
        let db_guard = lock_db(&db)?;
        let request = resolve_load_request(&db_guard, &model_id)?;
        let hardware = crate::app_hardware(&app);
        let assessment = app_core::estimate_compatibility(
            &db_guard,
            &model_id,
            &request.profile,
            hardware.available_memory_bytes,
        )?;
        if assessment.blocking {
            return Err(AppError::new(
                AppErrorCode::ModelOom,
                assessment.reasons.join(" "),
                "Reduce context length or choose a smaller model.",
            ));
        }
        request
    };

    let adapter = get_or_create_adapter(&app, &state, &db).await?;
    let loaded = adapter
        .load_model(load_request)
        .await
        .map_err(map_inference_error)?;

    // Best-effort "last used" stamp; never fail a successful load over it.
    if let Ok(db_guard) = lock_db(&db) {
        let _ = mark_model_used(&db_guard, &model_id);
    }
    crate::maintenance::log(&app, "MODEL_LOADED", "Local generation model loaded.");
    Ok(loaded)
}

/// Unload the current model, releasing its memory (FR-MOD-004).
#[tauri::command]
#[specta::specta]
pub async fn models_unload(state: State<'_, RuntimeState>, model_id: String) -> AppResult<()> {
    for token in state.cancels.lock().expect("cancel registry lock").values() {
        token.cancel();
    }
    let _operation = state.operation.lock().await;
    let adapter = {
        let guard = state.adapter.lock().await;
        guard.as_ref().map(|entry| entry.adapter.clone())
    };
    if let Some(adapter) = adapter {
        adapter
            .unload_model(&model_id)
            .await
            .map_err(map_inference_error)?;
    }
    Ok(())
}

/// Stream a chat generation (FR-CHAT-002). Tokens arrive via `chat:token`; the resolved value
/// is also broadcast as `chat:done`. Errors are emitted as `chat:error` and returned.
#[tauri::command]
#[specta::specta]
pub async fn chat_generate(
    app: tauri::AppHandle,
    state: State<'_, RuntimeState>,
    request: GenerationRequest,
) -> AppResult<GenerationResult> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| AppError::internal("Another operation is running."))?;
    run_generation(app, &state, request, None).await
}

pub(crate) async fn run_generation(
    app: tauri::AppHandle,
    state: &State<'_, RuntimeState>,
    request: GenerationRequest,
    persist_id: Option<String>,
) -> AppResult<GenerationResult> {
    let adapter = current_adapter(state).await?;
    let correlation_id = request.correlation_id.clone();
    let registration = state.register_cancel(&correlation_id);
    let cancel = registration.token.clone();

    // Forward streamed tokens to the UI as `chat:token` events.
    let (tx, mut rx) = mpsc::unbounded_channel::<TokenEvent>();
    let app_for_tokens = app.clone();
    let partial_id = persist_id.clone();
    let forwarder = tauri::async_runtime::spawn(async move {
        let mut partial = String::new();
        let mut checkpoint = std::time::Instant::now();
        while let Some(event) = rx.recv().await {
            partial.push_str(&event.token);
            if checkpoint.elapsed() > std::time::Duration::from_millis(500) {
                if let Some(id) = &partial_id {
                    if let Ok(db) = app_for_tokens.state::<Db>().lock() {
                        let _ =
                            storage::conversations::ConversationsRepository::new(db.connection())
                                .finish(id, &partial, "streaming", None);
                    }
                }
                checkpoint = std::time::Instant::now();
            }
            let _ = ChatToken(event).emit(&app_for_tokens);
        }
        partial
    });

    let result = adapter.generate(request, tx, cancel).await;
    let partial = forwarder.await.unwrap_or_default();
    state.take_cancel(&correlation_id);

    if let Some(id) = persist_id {
        let managed = app.state::<Db>();
        let db = lock_db(&managed)?;
        let repo = storage::conversations::ConversationsRepository::new(db.connection());
        match &result {
            Ok(output) => repo.finish(
                &id,
                &output.text,
                if output.finish_reason == inference::FinishReason::Cancelled {
                    "stopped"
                } else {
                    "complete"
                },
                serde_json::to_string(&output.timing).ok().as_deref(),
            ),
            Err(_) => repo.finish(&id, &partial, "error", None),
        }
        .map_err(crate::storage_err)?;
    }

    crate::maintenance::log(
        &app,
        if result.is_ok() {
            "GENERATION_FINISHED"
        } else {
            "GENERATION_FAILED"
        },
        "Generation ended; content excluded from diagnostics.",
    );
    match result {
        Ok(result) => {
            let _ = ChatDone(result.clone()).emit(&app);
            Ok(result)
        }
        Err(err) => {
            let app_error = map_inference_error(err);
            let _ = ChatError(app_error.clone()).emit(&app);
            Err(app_error)
        }
    }
}

/// Cancel an in-flight generation (FR-CHAT-006). Idempotent: cancelling an unknown id is a no-op.
#[tauri::command]
#[specta::specta]
pub async fn chat_cancel(state: State<'_, RuntimeState>, correlation_id: String) -> AppResult<()> {
    if let Some(token) = state
        .cancels
        .lock()
        .expect("cancel registry lock")
        .get(&correlation_id)
    {
        token.cancel();
    }
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_status(state: State<'_, RuntimeState>) -> AppResult<Option<String>> {
    let adapter = state
        .adapter
        .lock()
        .await
        .as_ref()
        .map(|entry| entry.adapter.clone());
    match adapter {
        Some(adapter) => Ok(adapter.loaded_model_id().await),
        None => Ok(None),
    }
}

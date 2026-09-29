//! Application core: command handlers, orchestration, and the shared error contract.
//!
//! Domain crates (`inference`, `benchmark`, `documents`, `storage`) stay free of Tauri
//! types; `app-core` adapts them and is consumed by the Tauri host in
//! `apps/desktop/src-tauri` (see PROJECT_REQUIREMENTS.md §6). Phase 1 provides the
//! `AppError` contract; Phase 3 adds hardware/runtime inspection and compatibility
//! estimation.

mod compat;
mod error;
mod hardware;
mod models;
mod runtime;
mod system;

pub use compat::{
    assess_compatibility, estimate_memory_bytes, CompatibilityAssessment, CompatibilityStatus,
};
pub use error::{AppError, AppErrorCode, AppResult};
pub use hardware::{detect_hardware, Gpu, GpuBackend, HardwareInfo};
pub use inference::{
    ChatMessage, ChatRole, EmbeddingConfig, FinishReason, GenerationRequest, GenerationResult,
    GenerationTiming, LoadModelConfig, LoadedModel, ModelFormat, ModelMetadata,
    RuntimeCapabilities, RuntimeEngine, RuntimeProfile, TokenEvent,
};
pub use models::{
    estimate_compatibility, get_runtime_profile, import_model, list_models,
    recommended_runtime_profile, remove_model, update_runtime_profile, ImportedModel,
    ModelImportResult, ModelRow, RemoveResult, StorageMode,
};
pub use runtime::{map_inference_error, mark_model_used, resolve_load_request};
pub use system::{inspect, SystemInspection};

/// The application version, sourced from the crate manifest.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

//! Inference runtime crate.
//!
//! Owns the [`InferenceAdapter`] contract (spec §12.2), the out-of-process llama.cpp adapter
//! ([`LlamaAdapter`]), a deterministic in-process [`FakeAdapter`] for tests, and runtime
//! capability reporting (FR-SYS-001). See PROJECT_REQUIREMENTS.md §5.6–5.8 / §13 (Phase 5).

pub mod capabilities;
pub mod contract;
pub mod fake;
pub mod gguf;
pub mod llama;

pub use capabilities::{
    detect_capabilities, RuntimeCapabilities, RuntimeEngine, ENGINE_VERSION_UNAVAILABLE,
};
pub use contract::{
    ChatMessage, ChatRole, EmbeddingConfig, FinishReason, GenerationRequest, GenerationResult,
    GenerationTiming, InferenceAdapter, InferenceError, InferenceResult, LoadModelConfig,
    LoadRequest, LoadedModel, RuntimeProfile, TokenEvent,
};
pub use fake::{FakeAdapter, FakeBehavior};
pub use gguf::{compute_sha256, inspect_model, GgufError, ModelFormat, ModelMetadata};
pub use llama::{resolve_llama_binary, CrashHandler, CrashInfo, LlamaAdapter, LLAMA_SERVER_BINARY};

/// Crate name, exposed for diagnostics/version surfaces.
pub const CRATE_NAME: &str = "inference";

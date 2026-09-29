//! Inference contract: the `InferenceAdapter` trait and its normalized DTOs (spec §12.2,
//! PROJECT_REQUIREMENTS.md §5.6 / §8.2). Every engine (llama.cpp today, Ollama in P1) speaks
//! these types so the UI behaves identically regardless of the runtime behind it.
//!
//! Types that cross the Tauri boundary derive `specta::Type` so the TS bindings stay in sync.
//! Rust-only plumbing (`LoadRequest`, `TokenSink`, cancellation) is deliberately *not* exported
//! to TS — the host maps the TS-facing command inputs onto these.

use std::path::PathBuf;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::capabilities::{RuntimeCapabilities, RuntimeEngine};
use crate::gguf::ModelMetadata;

/// A per-model runtime binding (contract §8.1). The generation-sampling fields are packed into
/// `runtime_profiles.generation_defaults_json` by the storage layer; the rest are columns.
///
/// Lives in the `inference` crate (not `storage`/`app-core`) because it is fundamentally a
/// runtime concept and is shared by both the adapter contract and the model repository. It is
/// re-exported from `app-core` so existing call sites are unaffected.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeProfile {
    pub id: String,
    pub model_id: String,
    pub engine: RuntimeEngine,
    pub context_length: u32,
    pub max_tokens: u32,
    pub temperature: f32,
    pub top_p: f32,
    pub top_k: u32,
    pub repeat_penalty: f32,
    pub seed: Option<i64>,
    pub threads: u32,
    pub batch_size: u32,
    pub gpu_layers: u32,
}

/// TS-facing input for `models.load` (contract §8.2). The host resolves the on-disk model path
/// from `model_id` and turns this into a [`LoadRequest`] for the adapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoadModelConfig {
    pub model_id: String,
    pub profile: RuntimeProfile,
}

/// Result of a successful load (contract §8.2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LoadedModel {
    pub model_id: String,
    pub load_time_ms: u64,
    pub capabilities: RuntimeCapabilities,
}

/// Embedding request configuration (contract §8.2). Used by RAG in Phase 7.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EmbeddingConfig {
    pub model_id: String,
    pub normalize: bool,
}

/// Chat role for a single turn (contract §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// One message in a generation request (contract §8.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

/// A streaming generation request (contract §8.2). `correlation_id` routes streamed events and
/// cancellation to the right view.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerationRequest {
    pub correlation_id: String,
    pub model_id: String,
    pub messages: Vec<ChatMessage>,
    pub profile: RuntimeProfile,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub stop: Vec<String>,
}

/// A single streamed token (payload of the `chat:token` event, contract §8.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenEvent {
    pub correlation_id: String,
    pub token: String,
    pub index: u32,
}

/// Why a generation ended (contract §8.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum FinishReason {
    Stop,
    Length,
    Cancelled,
    Error,
}

/// Timing metrics for a completed generation (contract §8.2). `ttft_ms` is time-to-first-token.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerationTiming {
    pub ttft_ms: u64,
    pub total_ms: u64,
    pub output_tokens: u32,
    pub tokens_per_second: f32,
}

/// The final result of a generation (contract §8.2; payload of `chat:done`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GenerationResult {
    pub correlation_id: String,
    pub text: String,
    pub finish_reason: FinishReason,
    pub timing: GenerationTiming,
    /// Engine-specific diagnostics preserved verbatim (spec §12.2).
    pub raw: serde_json::Value,
}

/// Adapter-level load input (Rust-only). Carries the resolved model file path that the
/// TS-facing [`LoadModelConfig`] omits.
#[derive(Debug, Clone)]
pub struct LoadRequest {
    pub model_id: String,
    pub model_path: PathBuf,
    pub profile: RuntimeProfile,
}

/// Errors surfaced by an inference adapter. The host maps these onto the shared `AppError`
/// contract (§8.5) — this crate has no knowledge of `AppError`.
#[derive(Debug, thiserror::Error)]
pub enum InferenceError {
    /// No runtime binary could be found (unset/invalid path and none on `PATH`).
    #[error("inference runtime binary not found: {0}")]
    RuntimeUnavailable(String),
    /// The runtime process failed to start or become healthy.
    #[error("failed to start inference runtime: {0}")]
    StartupFailed(String),
    /// The runtime process exited or crashed unexpectedly.
    #[error("inference runtime crashed: {0}")]
    Crashed(String),
    /// The device ran out of memory loading the model.
    #[error("out of memory: {0}")]
    OutOfMemory(String),
    /// The request exceeded the model's context window.
    #[error("context window exceeded: {0}")]
    ContextExceeded(String),
    /// No model is currently loaded, or a different model was requested.
    #[error("model not loaded: {0}")]
    NotLoaded(String),
    /// The operation is not supported by this engine.
    #[error("operation not supported: {0}")]
    Unsupported(String),
    /// A transport/protocol error talking to the runtime.
    #[error("runtime protocol error: {0}")]
    Protocol(String),
}

/// Convenience result alias for adapter operations.
pub type InferenceResult<T> = Result<T, InferenceError>;

/// The common inference contract every engine implements (spec §12.2, §5.6).
///
/// `generate` streams tokens through `tokens` and resolves with the final [`GenerationResult`].
/// It must observe `cancel`: when triggered, stop promptly (target ≤500 ms, FR-CHAT-006) and
/// return a result with `finish_reason = Cancelled`.
#[async_trait]
pub trait InferenceAdapter: Send + Sync {
    /// Read model metadata without loading it (delegates to the GGUF inspector for llama.cpp).
    async fn inspect_model(&self, source: &str) -> InferenceResult<ModelMetadata>;

    /// Load a model into the runtime, replacing any currently-loaded model.
    async fn load_model(&self, request: LoadRequest) -> InferenceResult<LoadedModel>;

    /// Stream a generation. Tokens are sent on `tokens`; the future resolves once complete,
    /// cancelled, or errored.
    async fn generate(
        &self,
        request: GenerationRequest,
        tokens: UnboundedSender<TokenEvent>,
        cancel: CancellationToken,
    ) -> InferenceResult<GenerationResult>;

    /// Embed texts into vectors (RAG, Phase 7).
    async fn embed(
        &self,
        texts: &[String],
        config: &EmbeddingConfig,
    ) -> InferenceResult<Vec<Vec<f32>>>;

    /// Tokenize text into token ids.
    async fn tokenize(&self, text: &str) -> InferenceResult<Vec<u32>>;

    /// Release the loaded model and its memory (FR-MOD-004).
    async fn unload_model(&self, model_id: &str) -> InferenceResult<()>;

    /// Report the capabilities of the active runtime.
    async fn capabilities(&self) -> InferenceResult<RuntimeCapabilities>;

    /// Whether a model is currently loaded (host uses this to enforce one-at-a-time).
    async fn loaded_model_id(&self) -> Option<String>;
}

//! An in-process fake `InferenceAdapter` used by the shared contract-test suite and by any
//! caller that needs deterministic inference without a runtime binary (spec §14 adapter
//! contract tests). It streams a canned response token-by-token, honours cancellation within
//! one token interval, and can be told to fail load (to exercise crash-isolation handling).

use std::sync::Mutex;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::capabilities::{RuntimeCapabilities, RuntimeEngine};
use crate::contract::{
    ChatRole, EmbeddingConfig, FinishReason, GenerationRequest, GenerationResult, GenerationTiming,
    InferenceAdapter, InferenceError, InferenceResult, LoadRequest, LoadedModel, TokenEvent,
};
use crate::gguf::ModelMetadata;

/// How the fake behaves on `load_model` — used to exercise error/crash handling deterministically.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FakeBehavior {
    /// Loads and generates normally.
    Ok,
    /// `load_model` fails as if the device ran out of memory.
    OomOnLoad,
    /// `load_model` fails as if the runtime process crashed on startup.
    CrashOnLoad,
}

/// A deterministic, in-process adapter.
pub struct FakeAdapter {
    behavior: FakeBehavior,
    /// The canned reply, emitted one whitespace-delimited token at a time.
    reply: String,
    /// Delay between tokens; small but non-zero so cancellation timing is observable.
    token_delay: Duration,
    loaded: Mutex<Option<String>>,
}

impl FakeAdapter {
    /// A normal fake that echoes a fixed reply.
    pub fn new() -> Self {
        Self {
            behavior: FakeBehavior::Ok,
            reply: "Hello from the fake runtime .".to_string(),
            token_delay: Duration::from_millis(5),
            loaded: Mutex::new(None),
        }
    }

    /// A fake whose `load_model` fails according to `behavior`.
    pub fn with_behavior(behavior: FakeBehavior) -> Self {
        Self {
            behavior,
            ..Self::new()
        }
    }

    /// Override the canned reply.
    pub fn with_reply(mut self, reply: impl Into<String>) -> Self {
        self.reply = reply.into();
        self
    }

    /// Override the per-token delay (a longer delay makes cancellation windows easy to assert).
    pub fn with_token_delay(mut self, delay: Duration) -> Self {
        self.token_delay = delay;
        self
    }
}

impl Default for FakeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl InferenceAdapter for FakeAdapter {
    async fn inspect_model(&self, _source: &str) -> InferenceResult<ModelMetadata> {
        Err(InferenceError::Unsupported(
            "the fake adapter does not inspect files".to_string(),
        ))
    }

    async fn load_model(&self, request: LoadRequest) -> InferenceResult<LoadedModel> {
        match self.behavior {
            FakeBehavior::OomOnLoad => {
                return Err(InferenceError::OutOfMemory("fake OOM".to_string()))
            }
            FakeBehavior::CrashOnLoad => {
                return Err(InferenceError::Crashed("fake crash on load".to_string()))
            }
            FakeBehavior::Ok => {}
        }
        *self.loaded.lock().expect("fake loaded lock") = Some(request.model_id.clone());
        Ok(LoadedModel {
            model_id: request.model_id,
            load_time_ms: 1,
            capabilities: self.capabilities().await?,
        })
    }

    async fn generate(
        &self,
        request: GenerationRequest,
        tokens: UnboundedSender<TokenEvent>,
        cancel: CancellationToken,
    ) -> InferenceResult<GenerationResult> {
        if self.loaded.lock().expect("fake loaded lock").as_deref()
            != Some(request.model_id.as_str())
        {
            return Err(InferenceError::NotLoaded(request.model_id));
        }

        let started = Instant::now();
        let mut ttft_ms = 0u64;
        let mut text = String::new();
        let mut index = 0u32;

        // Echo the last user message when present, else the canned reply — enough for the
        // contract suite to assert streaming without depending on a real model.
        let source = request
            .messages
            .iter()
            .rev()
            .find(|m| m.role == ChatRole::User)
            .map(|m| m.content.clone())
            .filter(|c| !c.is_empty())
            .unwrap_or_else(|| self.reply.clone());

        for piece in source.split_inclusive(' ') {
            // Cancellation is checked once per token interval, bounding the ack latency by
            // `token_delay` (well under the 500 ms budget in tests).
            tokio::select! {
                _ = cancel.cancelled() => {
                    return Ok(self.finish(&request.correlation_id, text, FinishReason::Cancelled, ttft_ms, started, index));
                }
                _ = tokio::time::sleep(self.token_delay) => {}
            }

            if index == 0 {
                ttft_ms = started.elapsed().as_millis() as u64;
            }
            text.push_str(piece);
            // A dropped receiver simply means nobody is listening; keep producing so the
            // final result is still well-formed.
            let _ = tokens.send(TokenEvent {
                correlation_id: request.correlation_id.clone(),
                token: piece.to_string(),
                index,
            });
            index += 1;
        }

        Ok(self.finish(
            &request.correlation_id,
            text,
            FinishReason::Stop,
            ttft_ms,
            started,
            index,
        ))
    }

    async fn embed(
        &self,
        texts: &[String],
        _config: &EmbeddingConfig,
    ) -> InferenceResult<Vec<Vec<f32>>> {
        // Deterministic, length-derived pseudo-embeddings; enough for retrieval unit tests.
        Ok(texts
            .iter()
            .map(|t| vec![t.len() as f32, t.chars().count() as f32, 1.0])
            .collect())
    }

    async fn tokenize(&self, text: &str) -> InferenceResult<Vec<u32>> {
        Ok(text
            .split_whitespace()
            .enumerate()
            .map(|(i, _)| i as u32)
            .collect())
    }

    async fn unload_model(&self, _model_id: &str) -> InferenceResult<()> {
        *self.loaded.lock().expect("fake loaded lock") = None;
        Ok(())
    }

    async fn capabilities(&self) -> InferenceResult<RuntimeCapabilities> {
        Ok(RuntimeCapabilities {
            engine: RuntimeEngine::LlamaCpp,
            engine_version: "fake-1.0".to_string(),
            supports_seed: true,
            supports_gpu_offload: false,
            supports_embeddings: true,
            deterministic_sampling: true,
            max_context: Some(4096),
        })
    }

    async fn loaded_model_id(&self) -> Option<String> {
        self.loaded.lock().expect("fake loaded lock").clone()
    }
}

impl FakeAdapter {
    fn finish(
        &self,
        correlation_id: &str,
        text: String,
        finish_reason: FinishReason,
        ttft_ms: u64,
        started: Instant,
        output_tokens: u32,
    ) -> GenerationResult {
        let total_ms = started.elapsed().as_millis() as u64;
        let tokens_per_second = if total_ms > 0 {
            (f64::from(output_tokens) / (total_ms as f64 / 1000.0)) as f32
        } else {
            0.0
        };
        GenerationResult {
            correlation_id: correlation_id.to_string(),
            text,
            finish_reason,
            timing: GenerationTiming {
                ttft_ms,
                total_ms,
                output_tokens,
                tokens_per_second,
            },
            raw: serde_json::json!({ "adapter": "fake" }),
        }
    }
}

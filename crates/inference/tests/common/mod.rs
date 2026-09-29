//! Shared adapter contract-test harness (PROJECT_REQUIREMENTS.md §14, item 7).
//!
//! Every `InferenceAdapter` must pass `run_contract`, so llama.cpp and (P1) Ollama behave
//! identically from the UI's perspective. The `FakeAdapter` runs it in-process; the real
//! `LlamaAdapter` runs it only when a runtime binary is discovered (see `llama_integration.rs`).

use inference::{
    ChatMessage, ChatRole, GenerationRequest, InferenceAdapter, LoadRequest, RuntimeEngine,
    RuntimeProfile, TokenEvent,
};
use tokio::sync::mpsc::{self, UnboundedReceiver};
use tokio_util::sync::CancellationToken;

/// A permissive sample profile for the given model id.
pub fn sample_profile(model_id: &str) -> RuntimeProfile {
    RuntimeProfile {
        id: format!("{model_id}-profile"),
        model_id: model_id.to_string(),
        engine: RuntimeEngine::LlamaCpp,
        context_length: 2048,
        max_tokens: 128,
        temperature: 0.7,
        top_p: 0.95,
        top_k: 40,
        repeat_penalty: 1.1,
        seed: Some(42),
        threads: 4,
        batch_size: 256,
        gpu_layers: 0,
    }
}

/// Build a simple single-turn request.
pub fn user_request(model_id: &str, correlation_id: &str, prompt: &str) -> GenerationRequest {
    GenerationRequest {
        correlation_id: correlation_id.to_string(),
        model_id: model_id.to_string(),
        messages: vec![ChatMessage {
            role: ChatRole::User,
            content: prompt.to_string(),
        }],
        profile: sample_profile(model_id),
        stop: Vec::new(),
    }
}

/// Drain every token from a receiver once the sender side is closed.
pub async fn collect_tokens(mut rx: UnboundedReceiver<TokenEvent>) -> Vec<TokenEvent> {
    let mut out = Vec::new();
    while let Some(event) = rx.recv().await {
        out.push(event);
    }
    out
}

/// Exercise the full lifecycle every adapter must support: load → capabilities → streaming
/// generate → tokenize → unload. `load` carries a valid model path for real adapters (ignored
/// by the fake).
pub async fn run_contract<A: InferenceAdapter>(adapter: &A, load: LoadRequest) {
    let model_id = load.model_id.clone();

    // Load.
    let loaded = adapter
        .load_model(load)
        .await
        .expect("load_model should succeed");
    assert_eq!(loaded.model_id, model_id);
    assert_eq!(
        adapter.loaded_model_id().await.as_deref(),
        Some(model_id.as_str())
    );

    // Capabilities.
    let caps = adapter
        .capabilities()
        .await
        .expect("capabilities should succeed");
    assert_eq!(caps.engine, RuntimeEngine::LlamaCpp);

    // Streaming generate: tokens must stream, and the concatenation must equal the final text.
    let (tx, rx) = mpsc::unbounded_channel();
    let request = user_request(&model_id, "corr-1", "Say hello");
    let cancel = CancellationToken::new();
    let gen = adapter.generate(request, tx, cancel);
    let (result, tokens) = tokio::join!(gen, collect_tokens(rx));
    let result = result.expect("generate should succeed");

    assert!(!tokens.is_empty(), "expected at least one streamed token");
    assert_eq!(result.correlation_id, "corr-1");
    let streamed: String = tokens.iter().map(|t| t.token.as_str()).collect();
    assert_eq!(
        streamed, result.text,
        "streamed tokens must reconstruct the text"
    );
    assert!(result.timing.output_tokens > 0);
    // Token indices are contiguous from zero.
    for (i, token) in tokens.iter().enumerate() {
        assert_eq!(token.index as usize, i);
    }

    // Tokenize returns something for non-empty input.
    let ids = adapter
        .tokenize("one two three")
        .await
        .expect("tokenize should succeed");
    assert!(!ids.is_empty());

    // Unload releases the model.
    adapter
        .unload_model(&model_id)
        .await
        .expect("unload should succeed");
    assert!(adapter.loaded_model_id().await.is_none());
}

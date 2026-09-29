//! The llama.cpp adapter (PROJECT_REQUIREMENTS.md §5.7). It runs `llama-server` as an
//! out-of-process child bound to loopback with a per-session bearer token (spec §5.4, §15),
//! and talks to it over HTTP: OpenAI-style `/v1/chat/completions` (SSE streaming),
//! `/v1/embeddings`, `/tokenize`, and `/health`.
//!
//! Crash isolation (NFR-REL-001): the child is supervised by a background watcher. If it exits
//! unexpectedly, the loaded state is cleared and the registered [`CrashHandler`] fires so the
//! host can emit `runtime:crashed` — the UI process is never affected.

use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use futures_util::StreamExt;
use tokio::sync::mpsc::UnboundedSender;
use tokio_util::sync::CancellationToken;

use crate::capabilities::{RuntimeCapabilities, RuntimeEngine};
use crate::contract::{
    EmbeddingConfig, FinishReason, GenerationRequest, GenerationResult, GenerationTiming,
    InferenceAdapter, InferenceError, InferenceResult, LoadRequest, LoadedModel, TokenEvent,
};
use crate::gguf::{self, ModelMetadata};

/// The conventional binary name of the llama.cpp HTTP server.
pub const LLAMA_SERVER_BINARY: &str = "llama-server";

/// Enforced for both generation and embedding runtime children on the supported platform.
pub const LOOPBACK_POLICY: &str = r#"(version 1)
(allow default)
(deny network*)
(allow network-bind (local ip "localhost:*"))
(allow network-inbound (local ip "localhost:*"))
(allow network-outbound (remote ip "localhost:*"))
"#;

/// Maximum time to wait for a spawned server to report healthy.
const HEALTH_TIMEOUT: Duration = Duration::from_secs(120);

/// Details of a runtime crash, handed to the [`CrashHandler`].
#[derive(Debug, Clone)]
pub struct CrashInfo {
    pub engine: RuntimeEngine,
    pub model_id: String,
    pub code: Option<i32>,
}

/// Callback fired when the runtime process exits unexpectedly (host emits `runtime:crashed`).
pub type CrashHandler = Arc<dyn Fn(CrashInfo) + Send + Sync>;

/// Resolve the `llama-server` binary: an explicit configured path wins, else search `PATH`.
/// Returns `None` when nothing usable is found (host surfaces `RUNTIME_UNAVAILABLE`).
pub fn resolve_llama_binary(configured: Option<&str>) -> Option<PathBuf> {
    if let Some(path) = configured {
        let trimmed = path.trim();
        if !trimmed.is_empty() {
            let candidate = PathBuf::from(trimmed);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            let bundled = parent.join("../Resources/runtime/bin/llama-server");
            if bundled.is_file() {
                return Some(bundled);
            }
        }
    }
    #[cfg(debug_assertions)]
    {
        let development = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../apps/desktop/src-tauri/runtime/bin/llama-server");
        if development.is_file() {
            return Some(development);
        }
    }
    let path_var = std::env::var_os("PATH")?;
    std::env::split_paths(&path_var)
        .map(|dir| dir.join(LLAMA_SERVER_BINARY))
        .find(|candidate| candidate.is_file())
}

/// A running `llama-server` child plus how to reach it.
struct RunningServer {
    child: tokio::process::Child,
    base_url: String,
    token: String,
    model_id: String,
    capabilities: RuntimeCapabilities,
    /// Set before an intentional kill so the watcher does not report a crash.
    expected_exit: Arc<AtomicBool>,
}

/// The llama.cpp adapter. Holds at most one running server (one loaded model at a time, §5.5).
pub struct LlamaAdapter {
    embedding_mode: bool,
    binary: PathBuf,
    client: reqwest::Client,
    on_crash: CrashHandler,
    state: Arc<Mutex<Option<RunningServer>>>,
}

impl Drop for LlamaAdapter {
    fn drop(&mut self) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(mut server) = state.take() {
                server.expected_exit.store(true, Ordering::SeqCst);
                let _ = server.child.start_kill();
            }
        }
    }
}

/// A lightweight snapshot of the running server used to make HTTP calls without holding the
/// state lock across `await` points.
struct Endpoint {
    base_url: String,
    token: String,
}

impl LlamaAdapter {
    /// Create an adapter for a resolved binary. `on_crash` is invoked from a background task
    /// when the runtime exits unexpectedly.
    pub fn new(binary: PathBuf, on_crash: CrashHandler) -> Self {
        Self {
            embedding_mode: false,
            binary,
            client: reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .expect("local HTTP client"),
            on_crash,
            state: Arc::new(Mutex::new(None)),
        }
    }

    /// Snapshot the current endpoint for the given model, or error if it is not loaded.
    pub fn process_id(&self) -> Option<u32> {
        self.state.lock().ok()?.as_ref()?.child.id()
    }

    pub fn for_embeddings(mut self) -> Self {
        self.embedding_mode = true;
        self
    }

    fn endpoint_for(&self, model_id: &str) -> InferenceResult<Endpoint> {
        let guard = self.state.lock().expect("llama state lock");
        match guard.as_ref() {
            Some(server) if server.model_id == model_id => Ok(Endpoint {
                base_url: server.base_url.clone(),
                token: server.token.clone(),
            }),
            Some(server) => Err(InferenceError::NotLoaded(format!(
                "requested '{model_id}' but '{}' is loaded",
                server.model_id
            ))),
            None => Err(InferenceError::NotLoaded(model_id.to_string())),
        }
    }

    /// Pick a free loopback port by binding to :0 and releasing it immediately.
    fn free_port() -> InferenceResult<u16> {
        let listener = TcpListener::bind("127.0.0.1:0")
            .map_err(|e| InferenceError::StartupFailed(format!("no free port: {e}")))?;
        listener
            .local_addr()
            .map(|addr| addr.port())
            .map_err(|e| InferenceError::StartupFailed(format!("no local addr: {e}")))
    }
}

#[async_trait]
impl InferenceAdapter for LlamaAdapter {
    async fn inspect_model(&self, source: &str) -> InferenceResult<ModelMetadata> {
        let path = source.to_string();
        // GGUF inspection is blocking file IO; run it off the async runtime.
        tokio::task::spawn_blocking(move || gguf::inspect_model(Path::new(&path)))
            .await
            .map_err(|e| InferenceError::Protocol(format!("inspect task failed: {e}")))?
            .map_err(|e| InferenceError::Protocol(format!("invalid model file: {e}")))
    }

    async fn load_model(&self, request: LoadRequest) -> InferenceResult<LoadedModel> {
        // Enforce one-model-at-a-time: unload any current model first.
        self.unload_model(&request.model_id).await.ok();
        {
            let mut guard = self.state.lock().expect("llama state lock");
            if guard.is_some() {
                // A different model is loaded; take it out and kill it below.
                if let Some(mut old) = guard.take() {
                    old.expected_exit.store(true, Ordering::SeqCst);
                    let _ = old.child.start_kill();
                }
            }
        }

        let port = Self::free_port()?;
        let token = uuid::Uuid::new_v4().to_string();
        let base_url = format!("http://127.0.0.1:{port}");
        let profile = &request.profile;

        let started = Instant::now();
        // Child-level policy: even a runtime bug cannot open a non-loopback connection.
        #[cfg(target_os = "macos")]
        let mut command = {
            let mut command = tokio::process::Command::new("/usr/bin/sandbox-exec");
            command.args(["-p", LOOPBACK_POLICY]).arg(&self.binary);
            command
        };
        #[cfg(not(target_os = "macos"))]
        let mut command = tokio::process::Command::new(&self.binary);
        if self.embedding_mode {
            command.args(["--embedding", "--pooling", "cls"]);
        }
        let mut child = command
            .arg("-m")
            .arg(&request.model_path)
            .args(["--host", "127.0.0.1"])
            .args(["--parallel", "1"])
            .args(["--port", &port.to_string()])
            .args(["--api-key", &token])
            .args(["-c", &profile.context_length.to_string()])
            .args(["-ngl", &profile.gpu_layers.to_string()])
            .args(["-t", &profile.threads.to_string()])
            .args(["-b", &profile.batch_size.to_string()])
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| InferenceError::StartupFailed(format!("could not spawn runtime: {e}")))?;

        // Wait for /health, aborting early if the process dies during startup.
        self.await_healthy(&base_url, &token, started, &mut child)
            .await?;

        let capabilities = self.probe_capabilities(&base_url, &token).await;
        let expected_exit = Arc::new(AtomicBool::new(false));
        let server = RunningServer {
            child,
            base_url,
            token,
            model_id: request.model_id.clone(),
            capabilities: capabilities.clone(),
            expected_exit: expected_exit.clone(),
        };
        *self.state.lock().expect("llama state lock") = Some(server);
        self.spawn_watcher(request.model_id.clone(), expected_exit);

        Ok(LoadedModel {
            model_id: request.model_id,
            load_time_ms: started.elapsed().as_millis() as u64,
            capabilities,
        })
    }

    async fn generate(
        &self,
        request: GenerationRequest,
        tokens: UnboundedSender<TokenEvent>,
        cancel: CancellationToken,
    ) -> InferenceResult<GenerationResult> {
        let endpoint = self.endpoint_for(&request.model_id)?;
        let body = chat_completion_body(&request);

        let started = Instant::now();
        let response = tokio::select! {
            _ = cancel.cancelled() => return Ok(build_result(&request.correlation_id, String::new(), FinishReason::Cancelled, 0, started, 0)),
            result = self.client.post(format!("{}/v1/chat/completions", endpoint.base_url)).bearer_auth(&endpoint.token).json(&body).send() => result.map_err(map_reqwest)?,
        };
        if !response.status().is_success() {
            return Err(classify_http(response).await);
        }

        let mut stream = response.bytes_stream();
        let mut buffer = Vec::new();
        let mut text = String::new();
        let mut index = 0u32;
        let mut ttft_ms = 0u64;
        let mut finish_reason = FinishReason::Stop;
        let mut usage_tokens: Option<u32> = None;
        let mut saw_finish = false;

        loop {
            let chunk = tokio::select! {
                _ = cancel.cancelled() => {
                    // Dropping the stream closes the connection; the server stops generating.
                    return Ok(build_result(&request.correlation_id, text, FinishReason::Cancelled, ttft_ms, started, index));
                }
                next = stream.next() => next,
            };
            let Some(chunk) = chunk else { break };
            let bytes = chunk.map_err(map_reqwest)?;
            buffer.extend_from_slice(&bytes);

            while let Some(pos) = buffer.iter().position(|byte| *byte == b'\n') {
                let line = String::from_utf8_lossy(&buffer[..pos]).trim().to_string();
                buffer.drain(..=pos);
                let Some(payload) = line.strip_prefix("data:") else {
                    continue;
                };
                let payload = payload.trim();
                if payload == "[DONE]" {
                    return Ok(build_result(
                        &request.correlation_id,
                        text,
                        finish_reason,
                        ttft_ms,
                        started,
                        usage_tokens.unwrap_or(index),
                    ));
                }
                let json: serde_json::Value = match serde_json::from_str(payload) {
                    Ok(value) => value,
                    Err(_) => continue,
                };
                if let Some(count) = json["usage"]["completion_tokens"].as_u64() {
                    usage_tokens = Some(count as u32);
                }
                let choice = &json["choices"][0];
                if let Some(content) = choice["delta"]["content"].as_str() {
                    if !content.is_empty() {
                        if index == 0 {
                            ttft_ms = started.elapsed().as_millis() as u64;
                        }
                        text.push_str(content);
                        let _ = tokens.send(TokenEvent {
                            correlation_id: request.correlation_id.clone(),
                            token: content.to_string(),
                            index,
                        });
                        index += 1;
                    }
                }
                if let Some(reason) = choice["finish_reason"].as_str() {
                    saw_finish = true;
                    finish_reason = match reason {
                        "length" => FinishReason::Length,
                        "stop" => FinishReason::Stop,
                        _ => finish_reason,
                    };
                }
            }
        }

        if !saw_finish {
            return Err(InferenceError::Protocol(
                "Stream closed before completion.".into(),
            ));
        }
        Ok(build_result(
            &request.correlation_id,
            text,
            finish_reason,
            ttft_ms,
            started,
            usage_tokens.unwrap_or(index),
        ))
    }

    async fn embed(
        &self,
        texts: &[String],
        config: &EmbeddingConfig,
    ) -> InferenceResult<Vec<Vec<f32>>> {
        let endpoint = self.endpoint_for(&config.model_id)?;
        let response = self
            .client
            .post(format!("{}/v1/embeddings", endpoint.base_url))
            .bearer_auth(&endpoint.token)
            .json(&serde_json::json!({ "input": texts }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(classify_http(response).await);
        }
        let json: serde_json::Value = response.json().await.map_err(map_reqwest)?;
        let data = json["data"].as_array().ok_or_else(|| {
            InferenceError::Protocol("embeddings: missing data array".to_string())
        })?;
        if data.len() != texts.len() {
            return Err(InferenceError::Protocol(
                "embeddings: vector count mismatch".into(),
            ));
        }
        data.iter()
            .map(|item| {
                let values = item["embedding"]
                    .as_array()
                    .ok_or_else(|| InferenceError::Protocol("embeddings: missing vector".into()))?;
                let mut vector: Vec<f32> = values
                    .iter()
                    .map(|v| {
                        v.as_f64()
                            .map(|f| f as f32)
                            .filter(|f| f.is_finite())
                            .ok_or_else(|| {
                                InferenceError::Protocol(
                                    "embeddings: non-numeric or non-finite value".into(),
                                )
                            })
                    })
                    .collect::<InferenceResult<_>>()?;
                let norm = vector
                    .iter()
                    .map(|v| f64::from(*v).powi(2))
                    .sum::<f64>()
                    .sqrt();
                if norm <= f64::EPSILON {
                    return Err(InferenceError::Protocol(
                        "embeddings: empty or zero vector".into(),
                    ));
                }
                if config.normalize {
                    for value in &mut vector {
                        *value = (f64::from(*value) / norm) as f32;
                    }
                }
                Ok(vector)
            })
            .collect()
    }

    async fn tokenize(&self, text: &str) -> InferenceResult<Vec<u32>> {
        // Tokenization needs a loaded model; use whichever is currently loaded.
        let model_id = self
            .loaded_model_id()
            .await
            .ok_or_else(|| InferenceError::NotLoaded("no model loaded".to_string()))?;
        let endpoint = self.endpoint_for(&model_id)?;
        let response = self
            .client
            .post(format!("{}/tokenize", endpoint.base_url))
            .bearer_auth(&endpoint.token)
            .json(&serde_json::json!({ "content": text }))
            .send()
            .await
            .map_err(map_reqwest)?;
        if !response.status().is_success() {
            return Err(classify_http(response).await);
        }
        let json: serde_json::Value = response.json().await.map_err(map_reqwest)?;
        let tokens = json["tokens"]
            .as_array()
            .ok_or_else(|| InferenceError::Protocol("tokenize: missing tokens".to_string()))?;
        Ok(tokens
            .iter()
            .filter_map(|v| v.as_u64().map(|n| n as u32))
            .collect())
    }

    async fn unload_model(&self, _model_id: &str) -> InferenceResult<()> {
        let server = self.state.lock().expect("llama state lock").take();
        if let Some(mut server) = server {
            server.expected_exit.store(true, Ordering::SeqCst);
            let _ = server.child.kill().await;
        }
        Ok(())
    }

    async fn capabilities(&self) -> InferenceResult<RuntimeCapabilities> {
        if let Some(server) = self.state.lock().expect("llama state lock").as_ref() {
            return Ok(server.capabilities.clone());
        }
        // Not loaded: report the adapter's declared capabilities with an unknown version.
        Ok(RuntimeCapabilities {
            engine: RuntimeEngine::LlamaCpp,
            engine_version: crate::capabilities::ENGINE_VERSION_UNAVAILABLE.to_string(),
            supports_seed: true,
            supports_gpu_offload: true,
            supports_embeddings: true,
            deterministic_sampling: true,
            max_context: None,
        })
    }

    async fn loaded_model_id(&self) -> Option<String> {
        self.state
            .lock()
            .expect("llama state lock")
            .as_ref()
            .map(|s| s.model_id.clone())
    }
}

impl LlamaAdapter {
    /// Poll `/health` until the server is ready, the process dies, or the timeout elapses.
    async fn await_healthy(
        &self,
        base_url: &str,
        token: &str,
        started: Instant,
        child: &mut tokio::process::Child,
    ) -> InferenceResult<()> {
        let health_url = format!("{base_url}/health");
        loop {
            if started.elapsed() > HEALTH_TIMEOUT {
                let _ = child.start_kill();
                return Err(InferenceError::StartupFailed(
                    "runtime did not become healthy in time".to_string(),
                ));
            }
            if let Ok(resp) = self
                .client
                .get(&health_url)
                .bearer_auth(token)
                .timeout(Duration::from_secs(2))
                .send()
                .await
            {
                if resp.status().is_success() {
                    return Ok(());
                }
            }
            // Detect an early crash so we don't wait the full timeout.
            if let Ok(Some(status)) = child.try_wait() {
                return Err(InferenceError::StartupFailed(format!(
                    "runtime exited during startup ({status})"
                )));
            }
            tokio::time::sleep(Duration::from_millis(200)).await;
        }
    }

    /// Probe `/props` for the engine version; fall back to declared capabilities.
    async fn probe_capabilities(&self, base_url: &str, token: &str) -> RuntimeCapabilities {
        let props = match self
            .client
            .get(format!("{base_url}/props"))
            .bearer_auth(token)
            .timeout(Duration::from_secs(2))
            .send()
            .await
        {
            Ok(resp) => resp.json::<serde_json::Value>().await.ok(),
            Err(_) => None,
        };
        let engine_version = props
            .as_ref()
            .and_then(|v| v["build_info"].as_str().or_else(|| v["version"].as_str()))
            .map(str::to_string)
            .unwrap_or_else(|| crate::capabilities::binary_version(&self.binary));
        RuntimeCapabilities {
            engine: RuntimeEngine::LlamaCpp,
            engine_version,
            supports_seed: true,
            supports_gpu_offload: true,
            supports_embeddings: true,
            deterministic_sampling: true,
            max_context: None,
        }
    }

    /// Watch the child; if it exits unexpectedly, clear state and fire the crash handler.
    fn spawn_watcher(&self, model_id: String, expected_exit: Arc<AtomicBool>) {
        let state = self.state.clone();
        let on_crash = self.on_crash.clone();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_millis(500)).await;
                let mut crashed: Option<Option<i32>> = None;
                {
                    let mut guard = state.lock().expect("llama state lock");
                    match guard.as_mut() {
                        Some(server)
                            if server.model_id == model_id
                                && Arc::ptr_eq(&server.expected_exit, &expected_exit) =>
                        {
                            match server.child.try_wait() {
                                Ok(Some(status)) => {
                                    if expected_exit.load(Ordering::SeqCst) {
                                        return; // intentional shutdown
                                    }
                                    crashed = Some(status.code());
                                    *guard = None;
                                }
                                Ok(None) => {}
                                Err(_) => return,
                            }
                        }
                        // A different model was loaded, or nothing is loaded: stop watching.
                        _ => return,
                    }
                }
                if let Some(code) = crashed {
                    on_crash(CrashInfo {
                        engine: RuntimeEngine::LlamaCpp,
                        model_id,
                        code,
                    });
                    return;
                }
            }
        });
    }
}

/// Build the OpenAI-style chat-completions request body from a [`GenerationRequest`].
fn chat_completion_body(request: &GenerationRequest) -> serde_json::Value {
    let messages: Vec<serde_json::Value> = request
        .messages
        .iter()
        .map(|m| {
            serde_json::json!({
                "role": match m.role {
                    crate::contract::ChatRole::System => "system",
                    crate::contract::ChatRole::User => "user",
                    crate::contract::ChatRole::Assistant => "assistant",
                },
                "content": m.content,
            })
        })
        .collect();
    let profile = &request.profile;
    let mut body = serde_json::json!({
        "model": request.model_id,
        "messages": messages,
        "stream": true,
        "stream_options": {"include_usage": true},
        "temperature": profile.temperature,
        "top_p": profile.top_p,
        "top_k": profile.top_k,
        "repeat_penalty": profile.repeat_penalty,
        "n_predict": profile.max_tokens,
        "max_tokens": profile.max_tokens,
    });
    if let Some(seed) = profile.seed {
        body["seed"] = serde_json::json!(seed);
    }
    if !request.stop.is_empty() {
        body["stop"] = serde_json::json!(request.stop);
    }
    body
}

fn build_result(
    correlation_id: &str,
    text: String,
    finish_reason: FinishReason,
    ttft_ms: u64,
    started: Instant,
    output_tokens: u32,
) -> GenerationResult {
    let total_ms = started.elapsed().as_millis() as u64;
    let generation_ms = total_ms.saturating_sub(ttft_ms);
    let tokens_per_second = if generation_ms > 0 {
        (f64::from(output_tokens.saturating_sub(1)) / (generation_ms as f64 / 1000.0)) as f32
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
        raw: serde_json::json!({ "engine": "llama.cpp" }),
    }
}

/// Map a transport error onto the inference contract.
fn map_reqwest(err: reqwest::Error) -> InferenceError {
    if err.is_connect() {
        InferenceError::Crashed(format!("lost connection to runtime: {err}"))
    } else {
        InferenceError::Protocol(err.to_string())
    }
}

/// Classify a non-2xx HTTP response. 5xx with an OOM hint maps to `OutOfMemory`; a 400/413 with
/// a context hint maps to `ContextExceeded`.
async fn classify_http(response: reqwest::Response) -> InferenceError {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let lower = text.to_lowercase();
    if lower.contains("out of memory") || lower.contains("oom") || lower.contains("alloc") {
        InferenceError::OutOfMemory(text)
    } else if lower.contains("context") && (lower.contains("exceed") || lower.contains("too long"))
    {
        InferenceError::ContextExceeded(text)
    } else if status.is_server_error() {
        InferenceError::Crashed(format!("runtime error {status}: {text}"))
    } else {
        InferenceError::Protocol(format!("runtime returned {status}: {text}"))
    }
}

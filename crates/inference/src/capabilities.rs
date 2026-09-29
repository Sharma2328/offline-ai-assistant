//! Runtime capability reporting (FR-SYS-001).
//! Reports the installed runtime without loading a model.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Sentinel used for `engine_version` when no runtime binary has been detected yet.
pub const ENGINE_VERSION_UNAVAILABLE: &str = "unavailable";

/// Inference engine identifier. Ollama is a post-MVP (P1) adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeEngine {
    #[serde(rename = "llama.cpp")]
    LlamaCpp,
    Ollama,
}

/// Capabilities of the active inference runtime (spec §12.2; contract §8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCapabilities {
    pub engine: RuntimeEngine,
    /// Detected runtime version, or [`ENGINE_VERSION_UNAVAILABLE`] when not installed.
    pub engine_version: String,
    pub supports_seed: bool,
    pub supports_gpu_offload: bool,
    pub supports_embeddings: bool,
    pub deterministic_sampling: bool,
    /// Maximum context the runtime advertises, when known.
    pub max_context: Option<u32>,
}

impl RuntimeCapabilities {
    /// Whether a runtime binary was actually detected.
    pub fn is_available(&self) -> bool {
        self.engine_version != ENGINE_VERSION_UNAVAILABLE
    }
}

/// Identify bundled releases from their adjacent build manifest.
pub fn binary_version(binary: &std::path::Path) -> String {
    binary
        .parent()
        .and_then(std::path::Path::parent)
        .and_then(|root| std::fs::read(root.join("manifest.json")).ok())
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|manifest| manifest["version"].as_str().map(str::to_owned))
        .unwrap_or_else(|| "custom (version unreported)".into())
}

/// Report installed llama.cpp capabilities; a model handshake adds context limits.
pub fn detect_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities {
        engine: RuntimeEngine::LlamaCpp,
        engine_version: crate::llama::resolve_llama_binary(None)
            .as_deref()
            .map(binary_version)
            .unwrap_or_else(|| ENGINE_VERSION_UNAVAILABLE.to_string()),
        supports_seed: true,
        supports_gpu_offload: true,
        supports_embeddings: true,
        deterministic_sampling: true,
        max_context: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runtime_availability_matches_discovery() {
        let caps = detect_capabilities();
        assert_eq!(caps.engine, RuntimeEngine::LlamaCpp);
        assert_eq!(
            caps.is_available(),
            crate::llama::resolve_llama_binary(None).is_some()
        );
    }

    #[test]
    fn engine_serializes_with_dotted_name() {
        let json = serde_json::to_string(&RuntimeEngine::LlamaCpp).unwrap();
        assert_eq!(json, "\"llama.cpp\"");
    }
}

//! Runtime capability reporting (FR-SYS-001).
//!
//! The generation runtime (llama.cpp) runs out-of-process and is integrated in Phase 5.
//! Until a runtime binary is present we still report the *shape* of what the llama.cpp
//! adapter will support, but mark `engine_version` as unavailable so the UI can show a
//! "runtime not installed" notice and degrade gracefully (FR-SYS-001 acceptance (c)).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Sentinel used for `engine_version` when no runtime binary has been detected yet.
pub const ENGINE_VERSION_UNAVAILABLE: &str = "unavailable";

/// Inference engine identifier. Ollama is a post-MVP (P1) adapter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeEngine {
    #[serde(rename = "llama.cpp")]
    LlamaCpp,
    Ollama,
}

/// Capabilities of the active inference runtime (spec §12.2; contract §8.1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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

/// Report the capabilities of the default (llama.cpp) runtime.
///
/// Phase 3 has no bundled runtime binary, so this reports the llama.cpp adapter's declared
/// capabilities with an `unavailable` version. Phase 5 replaces the version probe with an
/// actual handshake against the spawned runtime process.
pub fn detect_capabilities() -> RuntimeCapabilities {
    RuntimeCapabilities {
        engine: RuntimeEngine::LlamaCpp,
        engine_version: ENGINE_VERSION_UNAVAILABLE.to_string(),
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
    fn default_runtime_is_llama_cpp_and_currently_unavailable() {
        let caps = detect_capabilities();
        assert_eq!(caps.engine, RuntimeEngine::LlamaCpp);
        assert!(!caps.is_available());
    }

    #[test]
    fn engine_serializes_with_dotted_name() {
        let json = serde_json::to_string(&RuntimeEngine::LlamaCpp).unwrap();
        assert_eq!(json, "\"llama.cpp\"");
    }
}

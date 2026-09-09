//! Inference runtime crate.
//!
//! Owns the `InferenceAdapter` trait, the out-of-process llama.cpp adapter, and
//! the runtime supervisor. The adapter/generation surface arrives in Phase 5
//! (see PROJECT_REQUIREMENTS.md §13). Phase 3 adds runtime-capability reporting
//! (FR-SYS-001).

pub mod capabilities;
pub mod gguf;

pub use capabilities::{detect_capabilities, RuntimeCapabilities, RuntimeEngine};
pub use gguf::{compute_sha256, inspect_model, GgufError, ModelFormat, ModelMetadata};

/// Crate name, exposed for diagnostics/version surfaces.
pub const CRATE_NAME: &str = "inference";

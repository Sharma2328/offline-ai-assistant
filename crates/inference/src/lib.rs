//! Inference runtime crate.
//!
//! Owns the `InferenceAdapter` trait, the out-of-process llama.cpp adapter, and
//! the runtime supervisor. Populated in Phase 5 (see PROJECT_REQUIREMENTS.md §13).
//! This is an intentional Phase 1 placeholder so the workspace is complete and buildable.

/// Crate name, exposed for diagnostics/version surfaces.
pub const CRATE_NAME: &str = "inference";

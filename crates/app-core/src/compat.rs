//! Model↔hardware compatibility estimation (FR-ONB-002).
//!
//! Pure, deterministic, unit-tested functions with **no I/O** so they can be exercised in
//! isolation. Phase 3 ships the estimator scaffolding and classifier; Phase 4 refines the
//! estimate once GGUF metadata (layer/embedding dimensions) is parsed at import time.
//!
//! The estimate is intentionally conservative — it must never classify a guaranteed-OOM
//! configuration as loadable (FR-ONB-002: an *invalid* config is blocked; a *warning* may
//! be overridden).

use serde::{Deserialize, Serialize};
use specta::Type;

/// Fixed runtime working-set overhead (framework, buffers, scratch) beyond weights + KV.
pub const RUNTIME_OVERHEAD_BYTES: u64 = 512 * 1024 * 1024; // 512 MiB

/// Heuristic KV-cache cost per context token, absent parsed model dimensions.
///
/// Refined in Phase 4 from GGUF `n_layer`/`n_embd`. 160 KiB/token approximates a mid-size
/// 7–8B fp16 KV cache (~640 MiB at 4096 ctx) and errs on the high side for smaller models,
/// keeping the estimate conservative.
pub const KV_BYTES_PER_TOKEN: u64 = 160 * 1024; // 160 KiB

/// Fraction of available memory above which a *fitting* config is still flagged slow.
pub const TIGHT_FIT_FRACTION: f64 = 0.85;

/// Three-way compatibility class (contract §8.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CompatibilityStatus {
    Recommended,
    MayBeSlow,
    NotRecommended,
}

/// Result of a preflight estimate for a model + context length against current hardware.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CompatibilityAssessment {
    pub status: CompatibilityStatus,
    pub estimated_memory_bytes: u64,
    /// Available memory used for the estimate, or `0` when memory could not be detected.
    pub available_memory_bytes: u64,
    /// Plain-language reasons (never conveyed by color alone, FR-ONB-002).
    pub reasons: Vec<String>,
    /// `true` → the configuration is invalid and loading must be disabled.
    pub blocking: bool,
}

/// Estimate the peak memory a model will need for the given context length.
pub fn estimate_memory_bytes(model_size_bytes: u64, context_length: u32) -> u64 {
    let kv_cache = u64::from(context_length).saturating_mul(KV_BYTES_PER_TOKEN);
    model_size_bytes
        .saturating_add(kv_cache)
        .saturating_add(RUNTIME_OVERHEAD_BYTES)
}

/// Classify a model + context length against available memory.
///
/// - No memory reading → `NotRecommended` (non-blocking): we cannot promise it will fit.
/// - Estimate ≥ available → `NotRecommended` **blocking**: guaranteed OOM, load disabled.
/// - Estimate ≥ 85% of available → `MayBeSlow`: fits but tight (paging/thermal risk).
/// - Otherwise → `Recommended`.
pub fn assess_compatibility(
    model_size_bytes: u64,
    context_length: u32,
    available_memory_bytes: Option<u64>,
) -> CompatibilityAssessment {
    let estimated = estimate_memory_bytes(model_size_bytes, context_length);

    let Some(available) = available_memory_bytes.filter(|&a| a > 0) else {
        return CompatibilityAssessment {
            status: CompatibilityStatus::NotRecommended,
            estimated_memory_bytes: estimated,
            available_memory_bytes: 0,
            reasons: vec![
                "Available memory could not be detected, so compatibility cannot be \
                 guaranteed. You may still attempt to load the model."
                    .to_string(),
            ],
            blocking: false,
        };
    };

    let gib = |bytes: u64| (bytes as f64) / (1024.0 * 1024.0 * 1024.0);
    let estimate_str = format!("Estimated need ≈ {:.1} GiB", gib(estimated));
    let available_str = format!("available ≈ {:.1} GiB", gib(available));

    if estimated >= available {
        return CompatibilityAssessment {
            status: CompatibilityStatus::NotRecommended,
            estimated_memory_bytes: estimated,
            available_memory_bytes: available,
            reasons: vec![
                format!("{estimate_str}, which exceeds {available_str}."),
                "Loading would run out of memory. Reduce the context length or choose a \
                 smaller / more-quantized model."
                    .to_string(),
            ],
            blocking: true,
        };
    }

    let tight_threshold = (available as f64 * TIGHT_FIT_FRACTION) as u64;
    if estimated >= tight_threshold {
        return CompatibilityAssessment {
            status: CompatibilityStatus::MayBeSlow,
            estimated_memory_bytes: estimated,
            available_memory_bytes: available,
            reasons: vec![
                format!("{estimate_str}, close to {available_str}."),
                "It should load but may be slow or cause memory pressure. A shorter \
                 context length will help."
                    .to_string(),
            ],
            blocking: false,
        };
    }

    CompatibilityAssessment {
        status: CompatibilityStatus::Recommended,
        estimated_memory_bytes: estimated,
        available_memory_bytes: available,
        reasons: vec![format!(
            "{estimate_str}, comfortably within {available_str}."
        )],
        blocking: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024 * 1024 * 1024;

    #[test]
    fn estimate_includes_weights_kv_and_overhead() {
        let est = estimate_memory_bytes(4 * GIB, 4096);
        let expected = 4 * GIB + 4096 * KV_BYTES_PER_TOKEN + RUNTIME_OVERHEAD_BYTES;
        assert_eq!(est, expected);
    }

    #[test]
    fn longer_context_costs_more_memory() {
        let short = estimate_memory_bytes(4 * GIB, 2048);
        let long = estimate_memory_bytes(4 * GIB, 8192);
        assert!(long > short);
    }

    #[test]
    fn comfortable_fit_is_recommended() {
        let a = assess_compatibility(4 * GIB, 4096, Some(32 * GIB));
        assert_eq!(a.status, CompatibilityStatus::Recommended);
        assert!(!a.blocking);
    }

    #[test]
    fn tight_fit_is_may_be_slow_and_not_blocking() {
        // Pick available so the estimate lands between 85% and 100% of it.
        let est = estimate_memory_bytes(8 * GIB, 4096);
        let available = (est as f64 / 0.9) as u64; // estimate ≈ 90% of available
        let a = assess_compatibility(8 * GIB, 4096, Some(available));
        assert_eq!(a.status, CompatibilityStatus::MayBeSlow);
        assert!(!a.blocking);
    }

    #[test]
    fn oversized_model_is_not_recommended_and_blocking() {
        let a = assess_compatibility(64 * GIB, 8192, Some(8 * GIB));
        assert_eq!(a.status, CompatibilityStatus::NotRecommended);
        assert!(a.blocking);
    }

    #[test]
    fn unknown_memory_is_not_recommended_but_overridable() {
        let a = assess_compatibility(4 * GIB, 4096, None);
        assert_eq!(a.status, CompatibilityStatus::NotRecommended);
        assert!(!a.blocking);
        assert_eq!(a.available_memory_bytes, 0);
    }

    #[test]
    fn status_serializes_snake_case() {
        assert_eq!(
            serde_json::to_string(&CompatibilityStatus::MayBeSlow).unwrap(),
            "\"may_be_slow\""
        );
    }
}

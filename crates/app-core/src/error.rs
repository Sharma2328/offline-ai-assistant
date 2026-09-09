//! Application error contract shared across all Tauri commands.
//!
//! Mirrors PROJECT_REQUIREMENTS.md §8.5. Every command returns `Result<T, AppError>`;
//! the UI renders `message` + `recovery` and never sees a bare string. `details` is an
//! optional structured map that is **redacted before export** (NFR-SEC redaction rules).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use specta::Type;

/// Stable machine-readable error codes (spec §17 / doc §8.5).
///
/// Serialized as `SCREAMING_SNAKE_CASE` to match the generated TS discriminated union.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AppErrorCode {
    /// Model file is missing, corrupt, or an unsupported format.
    ModelInvalid,
    /// Model could not be loaded because the device is out of memory.
    ModelOom,
    /// The out-of-process inference runtime crashed or exited unexpectedly.
    RuntimeCrashed,
    /// The request exceeded the model's context window.
    ContextExceeded,
    /// A document could not be parsed (unsupported/corrupt/blocked entity).
    DocumentParseFailed,
    /// A benchmark case exceeded its per-case time budget.
    BenchCaseTimeout,
    /// A benchmark dataset failed validation.
    DatasetInvalid,
    /// Not enough free disk space to complete the operation.
    DiskSpaceLow,
    /// An unexpected internal error (bug / unhandled condition).
    Internal,
}

/// The single error type returned across the command boundary (doc §8.5).
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppError {
    /// Machine-readable classification.
    pub code: AppErrorCode,
    /// Human-readable description of what went wrong.
    pub message: String,
    /// Actionable guidance for how the user can recover.
    pub recovery: String,
    /// Optional structured context; redacted before any export.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub details: Option<HashMap<String, serde_json::Value>>,
}

impl AppError {
    /// Construct an error with a code, message, and recovery hint.
    pub fn new(
        code: AppErrorCode,
        message: impl Into<String>,
        recovery: impl Into<String>,
    ) -> Self {
        Self {
            code,
            message: message.into(),
            recovery: recovery.into(),
            details: None,
        }
    }

    /// Attach structured details (redacted before export).
    #[must_use]
    pub fn with_details(mut self, details: HashMap<String, serde_json::Value>) -> Self {
        self.details = Some(details);
        self
    }

    /// Convenience constructor for the catch-all internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            AppErrorCode::Internal,
            message,
            "This is unexpected. Please retry; if it persists, export diagnostics from Settings.",
        )
    }
}

impl std::fmt::Display for AppError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.code, self.message)
    }
}

impl std::error::Error for AppError {}

/// Convenience result alias used throughout command handlers.
pub type AppResult<T> = Result<T, AppError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn error_code_serializes_screaming_snake_case() {
        let json = serde_json::to_string(&AppErrorCode::ModelInvalid).unwrap();
        assert_eq!(json, "\"MODEL_INVALID\"");
        let json = serde_json::to_string(&AppErrorCode::BenchCaseTimeout).unwrap();
        assert_eq!(json, "\"BENCH_CASE_TIMEOUT\"");
    }

    #[test]
    fn error_serializes_camel_case_and_omits_empty_details() {
        let err = AppError::new(AppErrorCode::DiskSpaceLow, "no space", "free up disk");
        let value = serde_json::to_value(&err).unwrap();
        assert_eq!(value["code"], "DISK_SPACE_LOW");
        assert_eq!(value["message"], "no space");
        assert_eq!(value["recovery"], "free up disk");
        assert!(value.get("details").is_none());
    }

    #[test]
    fn internal_helper_sets_code() {
        let err = AppError::internal("boom");
        assert_eq!(err.code, AppErrorCode::Internal);
    }
}

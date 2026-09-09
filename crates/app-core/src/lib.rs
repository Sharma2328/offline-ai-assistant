//! Application core: command handlers, orchestration, and the shared error contract.
//!
//! Domain crates (`inference`, `benchmark`, `documents`, `storage`) stay free of Tauri
//! types; `app-core` adapts them and is consumed by the Tauri host in
//! `apps/desktop/src-tauri` (see PROJECT_REQUIREMENTS.md §6). Phase 1 provides the
//! `AppError` contract; command handlers arrive in later phases.

mod error;

pub use error::{AppError, AppErrorCode, AppResult};

/// The application version, sourced from the crate manifest.
pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

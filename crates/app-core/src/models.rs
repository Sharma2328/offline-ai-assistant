//! Model-management orchestration (Phase 4, FR-MOD-001/002/003/006).
//!
//! Adapts the `storage` repository + `inference` GGUF parser + the compatibility estimator
//! into the typed contract the Tauri host exposes. Contract types (`StorageMode`,
//! `RuntimeProfile`, `ModelRow`, …) live here because they compose lower-layer types
//! (`ModelMetadata`, `CompatibilityAssessment`, `RuntimeEngine`). No Tauri types leak in.

use std::collections::BTreeMap;
use std::path::Path;

use inference::{inspect_model, ModelFormat, ModelMetadata};
use serde::{Deserialize, Serialize};
use specta::Type;
use storage::{Database, NewModel, NewRuntimeProfile, ProfileUpdate, RuntimeProfileRecord};
use uuid::Uuid;

use crate::compat::{assess_compatibility, CompatibilityAssessment};
use crate::error::{AppError, AppErrorCode, AppResult};
use crate::hardware::HardwareInfo;
use crate::{RuntimeCapabilities, RuntimeEngine, RuntimeProfile};

/// Where a model's bytes live: referenced in place (default) or copied into app storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum StorageMode {
    Reference,
    Managed,
}

impl StorageMode {
    fn as_str(self) -> &'static str {
        match self {
            StorageMode::Reference => "reference",
            StorageMode::Managed => "managed",
        }
    }

    fn from_str(value: &str) -> Self {
        // Unknown persisted values are treated as `reference` (never deletes a file).
        if value == "managed" {
            StorageMode::Managed
        } else {
            StorageMode::Reference
        }
    }
}

/// Sampling defaults persisted as the `generation_defaults_json` blob.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GenerationDefaults {
    max_tokens: u32,
    temperature: f32,
    top_p: f32,
    top_k: u32,
    repeat_penalty: f32,
    seed: Option<i64>,
}

/// A library row (FR-MOD-002); returned by `models.list`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    pub id: String,
    pub name: String,
    pub file_uri: String,
    pub storage_mode: StorageMode,
    pub size_bytes: u64,
    pub sha256: String,
    pub architecture: Option<String>,
    pub quantization: Option<String>,
    pub last_used_at: Option<String>,
    pub compatibility: Option<CompatibilityAssessment>,
}

/// `ModelMetadata & { id }` — the import response payload (contract §9.2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedModel {
    pub id: String,
    pub format: ModelFormat,
    pub architecture: Option<String>,
    pub parameter_count: Option<u64>,
    pub quantization: Option<String>,
    pub context_length_max: Option<u32>,
    pub size_bytes: u64,
    pub sha256: String,
    pub raw: BTreeMap<String, String>,
}

impl ImportedModel {
    fn new(id: String, meta: ModelMetadata) -> Self {
        Self {
            id,
            format: meta.format,
            architecture: meta.architecture,
            parameter_count: meta.parameter_count,
            quantization: meta.quantization,
            context_length_max: meta.context_length_max,
            size_bytes: meta.size_bytes,
            sha256: meta.sha256,
            raw: meta.raw,
        }
    }
}

/// Result of `models.import`. `deduplicated` is `true` when the file's checksum already
/// existed in the library — no new copy was made and `model` refers to the existing entry
/// (FR-MOD-001d: "detected and de-duplicated with a prompt").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ModelImportResult {
    pub model: ImportedModel,
    pub deduplicated: bool,
}

/// Result of `models.remove` (contract §9.2). `source_file_affected` is `true` only when a
/// file was deleted from disk — which happens for **managed** copies, never for referenced
/// source files (FR-MOD-006).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoveResult {
    pub ok: bool,
    pub source_file_affected: bool,
}

fn storage_err(err: storage::StorageError) -> AppError {
    AppError::internal(format!("storage error: {err}"))
}

fn model_not_found(id: &str) -> AppError {
    AppError::new(
        AppErrorCode::Internal,
        format!("model '{id}' was not found"),
        "Refresh the model library; the model may have been removed.",
    )
}

fn engine_str(engine: RuntimeEngine) -> &'static str {
    match engine {
        RuntimeEngine::LlamaCpp => "llama.cpp",
        RuntimeEngine::Ollama => "ollama",
    }
}

fn parse_engine(value: &str) -> RuntimeEngine {
    match value {
        "ollama" => RuntimeEngine::Ollama,
        _ => RuntimeEngine::LlamaCpp,
    }
}

/// Import a GGUF model: validate, read metadata, checksum, de-duplicate, and register it
/// with a hardware-derived default runtime profile (FR-MOD-001).
pub fn import_model(
    db: &Database,
    models_dir: &Path,
    source_path: &Path,
    storage_mode: StorageMode,
    hardware: &HardwareInfo,
    capabilities: &RuntimeCapabilities,
) -> AppResult<ModelImportResult> {
    let metadata = inspect_model(source_path).map_err(|err| {
        AppError::new(
            AppErrorCode::ModelInvalid,
            format!("could not read model file: {err}"),
            "Choose a valid, uncorrupted GGUF model file.",
        )
    })?;

    let repo = db.models();

    // De-duplicate on checksum: return the existing entry without copying/inserting.
    if let Some(existing) = repo.find_by_sha256(&metadata.sha256).map_err(storage_err)? {
        return Ok(ModelImportResult {
            model: ImportedModel::new(existing.id, metadata),
            deduplicated: true,
        });
    }

    let file_uri = match storage_mode {
        StorageMode::Reference => source_path
            .canonicalize()
            .unwrap_or_else(|_| source_path.to_path_buf())
            .to_string_lossy()
            .into_owned(),
        StorageMode::Managed => {
            if let Some(free) = hardware.available_disk_bytes {
                if free < metadata.size_bytes {
                    return Err(AppError::new(
                        AppErrorCode::DiskSpaceLow,
                        format!(
                            "not enough disk space to copy the model ({} bytes needed, {} free)",
                            metadata.size_bytes, free
                        ),
                        "Free up disk space, or import the model as a reference instead of a \
                         managed copy.",
                    ));
                }
            }
            std::fs::create_dir_all(models_dir).map_err(|e| {
                AppError::internal(format!("could not create managed model storage: {e}"))
            })?;
            let dest = models_dir.join(format!("{}.gguf", metadata.sha256));
            std::fs::copy(source_path, &dest).map_err(|e| {
                AppError::new(
                    AppErrorCode::DiskSpaceLow,
                    format!("could not copy the model into app storage: {e}"),
                    "Check available disk space and permissions, then try again.",
                )
            })?;
            dest.to_string_lossy().into_owned()
        }
    };

    let name = metadata
        .raw
        .get("general.name")
        .cloned()
        .filter(|n| !n.trim().is_empty())
        .unwrap_or_else(|| {
            source_path
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Untitled model".to_string())
        });

    let model_id = Uuid::new_v4().to_string();
    let profile_id = Uuid::new_v4().to_string();
    let recommended = recommended_values(metadata.context_length_max, hardware, capabilities);

    let metadata_json = serde_json::to_string(&metadata)
        .map_err(|e| AppError::internal(format!("could not serialize model metadata: {e}")))?;
    let defaults_json = serde_json::to_string(&recommended.defaults)
        .map_err(|e| AppError::internal(format!("could not serialize runtime defaults: {e}")))?;

    let new_model = NewModel {
        id: model_id.clone(),
        name,
        file_uri,
        storage_mode: storage_mode.as_str().to_string(),
        sha256: metadata.sha256.clone(),
        size_bytes: i64::try_from(metadata.size_bytes).unwrap_or(i64::MAX),
        format: "gguf".to_string(),
        architecture: metadata.architecture.clone(),
        quantization: metadata.quantization.clone(),
        metadata_json,
    };
    let new_profile = NewRuntimeProfile {
        id: profile_id,
        engine: engine_str(capabilities.engine).to_string(),
        context_length: i64::from(recommended.context_length),
        threads: i64::from(recommended.threads),
        gpu_layers: i64::from(recommended.gpu_layers),
        batch_size: i64::from(recommended.batch_size),
        generation_defaults_json: defaults_json,
        is_default: true,
    };

    repo.insert_with_default_profile(&new_model, &new_profile)
        .map_err(storage_err)?;

    Ok(ModelImportResult {
        model: ImportedModel::new(model_id, metadata),
        deduplicated: false,
    })
}

/// List every model with a compatibility badge computed against current memory (FR-MOD-002).
pub fn list_models(db: &Database, available_memory: Option<u64>) -> AppResult<Vec<ModelRow>> {
    let repo = db.models();
    let records = repo.list().map_err(storage_err)?;
    let mut rows = Vec::with_capacity(records.len());
    for record in records {
        let context_length = repo
            .default_profile(&record.id)
            .map_err(storage_err)?
            .map(|p| u32::try_from(p.context_length).unwrap_or(4096))
            .unwrap_or(4096);
        let size_bytes = u64::try_from(record.size_bytes).unwrap_or(0);
        let compatibility = assess_compatibility(size_bytes, context_length, available_memory);
        rows.push(ModelRow {
            id: record.id,
            name: record.name,
            file_uri: record.file_uri,
            storage_mode: StorageMode::from_str(&record.storage_mode),
            size_bytes,
            sha256: record.sha256,
            architecture: record.architecture,
            quantization: record.quantization,
            last_used_at: record.last_used_at,
            compatibility: Some(compatibility),
        });
    }
    Ok(rows)
}

/// Preflight a model + profile against current memory (FR-ONB-002).
pub fn estimate_compatibility(
    db: &Database,
    model_id: &str,
    profile: &RuntimeProfile,
    available_memory: Option<u64>,
) -> AppResult<CompatibilityAssessment> {
    let record = db
        .models()
        .get(model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(model_id))?;
    let size_bytes = u64::try_from(record.size_bytes).unwrap_or(0);
    Ok(assess_compatibility(
        size_bytes,
        profile.context_length,
        available_memory,
    ))
}

/// Read a model's default runtime profile (backs the FR-MOD-003 editor).
pub fn get_runtime_profile(db: &Database, model_id: &str) -> AppResult<RuntimeProfile> {
    let record = db
        .models()
        .default_profile(model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(model_id))?;
    profile_from_record(&record)
}

/// Persist edits to a runtime profile after validating ranges (FR-MOD-003a).
pub fn update_runtime_profile(
    db: &Database,
    profile: &RuntimeProfile,
) -> AppResult<RuntimeProfile> {
    validate_profile(profile)?;
    let model = db
        .models()
        .get(&profile.model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(&profile.model_id))?;
    if let Ok(metadata) = serde_json::from_str::<ModelMetadata>(&model.metadata_json) {
        if metadata
            .context_length_max
            .is_some_and(|limit| profile.context_length > limit)
        {
            return Err(AppError::internal(
                "Context length exceeds the model's advertised limit.",
            ));
        }
    }
    let defaults = GenerationDefaults {
        max_tokens: profile.max_tokens,
        temperature: profile.temperature,
        top_p: profile.top_p,
        top_k: profile.top_k,
        repeat_penalty: profile.repeat_penalty,
        seed: profile.seed,
    };
    let defaults_json = serde_json::to_string(&defaults)
        .map_err(|e| AppError::internal(format!("could not serialize runtime defaults: {e}")))?;

    let changed = db
        .models()
        .update_profile(&ProfileUpdate {
            id: &profile.id,
            engine: engine_str(profile.engine),
            context_length: i64::from(profile.context_length),
            threads: i64::from(profile.threads),
            gpu_layers: i64::from(profile.gpu_layers),
            batch_size: i64::from(profile.batch_size),
            generation_defaults_json: &defaults_json,
        })
        .map_err(storage_err)?;
    if !changed {
        return Err(AppError::new(
            AppErrorCode::Internal,
            format!("runtime profile '{}' was not found", profile.id),
            "Refresh the model library and try again.",
        ));
    }

    let record = db
        .models()
        .profile(&profile.id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(&profile.id))?;
    profile_from_record(&record)
}

/// Compute a hardware-derived recommended profile for a model (FR-MOD-003b "Reset").
pub fn recommended_runtime_profile(
    db: &Database,
    model_id: &str,
    hardware: &HardwareInfo,
    capabilities: &RuntimeCapabilities,
) -> AppResult<RuntimeProfile> {
    let repo = db.models();
    let model = repo
        .get(model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(model_id))?;
    let profile = repo
        .default_profile(model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(model_id))?;

    let context_max = serde_json::from_str::<ModelMetadata>(&model.metadata_json)
        .ok()
        .and_then(|m| m.context_length_max);
    let recommended = recommended_values(context_max, hardware, capabilities);

    Ok(RuntimeProfile {
        id: profile.id,
        model_id: profile.model_id,
        engine: capabilities.engine,
        context_length: recommended.context_length,
        max_tokens: recommended.defaults.max_tokens,
        temperature: recommended.defaults.temperature,
        top_p: recommended.defaults.top_p,
        top_k: recommended.defaults.top_k,
        repeat_penalty: recommended.defaults.repeat_penalty,
        seed: recommended.defaults.seed,
        threads: recommended.threads,
        batch_size: recommended.batch_size,
        gpu_layers: recommended.gpu_layers,
    })
}

/// Remove a model. Managed copies are deleted from disk; referenced source files are never
/// touched (FR-MOD-006). Runtime profiles cascade at the DB layer.
pub fn remove_model(db: &Database, model_id: &str) -> AppResult<RemoveResult> {
    let removed = db
        .models()
        .remove(model_id)
        .map_err(storage_err)?
        .ok_or_else(|| model_not_found(model_id))?;

    let mut source_file_affected = false;
    if StorageMode::from_str(&removed.storage_mode) == StorageMode::Managed {
        // Best-effort: an already-missing managed file should not fail the removal.
        if Path::new(&removed.file_uri).exists() {
            std::fs::remove_file(&removed.file_uri).map_err(|e| {
                AppError::internal(format!("could not delete managed model file: {e}"))
            })?;
        }
        source_file_affected = true;
    }

    Ok(RemoveResult {
        ok: true,
        source_file_affected,
    })
}

/// Hardware-derived recommended runtime values.
struct Recommended {
    context_length: u32,
    threads: u32,
    gpu_layers: u32,
    batch_size: u32,
    defaults: GenerationDefaults,
}

fn recommended_values(
    context_length_max: Option<u32>,
    hardware: &HardwareInfo,
    capabilities: &RuntimeCapabilities,
) -> Recommended {
    // Default to a safe 4096 window, but never exceed what the model advertises.
    let context_length = context_length_max
        .map(|max| max.min(4096))
        .unwrap_or(4096)
        .max(32);

    let threads = hardware.logical_cores.unwrap_or(4).max(1);

    let has_accelerator = hardware
        .gpus
        .iter()
        .any(|g| !matches!(g.backend, crate::GpuBackend::Cpu));
    let gpu_layers = if capabilities.supports_gpu_offload && has_accelerator {
        999 // offload all layers; the runtime clamps to the real layer count (Phase 5)
    } else {
        0
    };

    Recommended {
        context_length,
        threads,
        gpu_layers,
        batch_size: 512,
        defaults: GenerationDefaults {
            max_tokens: 512.min(context_length / 2),
            temperature: 0.7,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.1,
            seed: None,
        },
    }
}

fn profile_from_record(record: &RuntimeProfileRecord) -> AppResult<RuntimeProfile> {
    let defaults: GenerationDefaults = serde_json::from_str(&record.generation_defaults_json)
        .map_err(|e| {
            AppError::internal(format!("stored runtime defaults are not valid JSON: {e}"))
        })?;
    Ok(RuntimeProfile {
        id: record.id.clone(),
        model_id: record.model_id.clone(),
        engine: parse_engine(&record.engine),
        context_length: u32::try_from(record.context_length).unwrap_or(4096),
        max_tokens: defaults.max_tokens,
        temperature: defaults.temperature,
        top_p: defaults.top_p,
        top_k: defaults.top_k,
        repeat_penalty: defaults.repeat_penalty,
        seed: defaults.seed,
        threads: u32::try_from(record.threads).unwrap_or(4),
        batch_size: u32::try_from(record.batch_size).unwrap_or(512),
        gpu_layers: u32::try_from(record.gpu_layers).unwrap_or(0),
    })
}

fn validate_profile(profile: &RuntimeProfile) -> AppResult<()> {
    let invalid = |msg: String| {
        Err(AppError::new(
            AppErrorCode::Internal,
            msg,
            "Adjust the runtime settings to valid ranges, or reset to recommended.",
        ))
    };
    if profile.context_length < 32 {
        return invalid("context length must be at least 32 tokens".to_string());
    }
    if profile.max_tokens >= profile.context_length {
        return invalid("Reserve context space for the prompt: generation tokens must be lower than context length.".into());
    }
    if profile.max_tokens < 1 {
        return invalid("max generation tokens must be at least 1".to_string());
    }
    if !(0.0..=2.0).contains(&profile.temperature) {
        return invalid("temperature must be between 0.0 and 2.0".to_string());
    }
    if !(0.0..=1.0).contains(&profile.top_p) {
        return invalid("top-p must be between 0.0 and 1.0".to_string());
    }
    if !(0.0..=4.0).contains(&profile.repeat_penalty) {
        return invalid("repeat penalty must be between 0.0 and 4.0".to_string());
    }
    if profile.threads < 1 {
        return invalid("thread count must be at least 1".to_string());
    }
    if profile.batch_size < 1 {
        return invalid("batch size must be at least 1".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hardware::{Gpu, GpuBackend};
    use inference::detect_capabilities;
    use std::io::Write;

    fn hardware(cores: u32, disk: Option<u64>, gpu: bool) -> HardwareInfo {
        HardwareInfo {
            os: "macos".to_string(),
            arch: "aarch64".to_string(),
            cpu_model: Some("Apple M3".to_string()),
            logical_cores: Some(cores),
            total_memory_bytes: Some(16 * 1024 * 1024 * 1024),
            available_memory_bytes: Some(8 * 1024 * 1024 * 1024),
            gpus: if gpu {
                vec![Gpu {
                    name: "Apple M3 GPU".to_string(),
                    backend: GpuBackend::Metal,
                    vram_bytes: None,
                }]
            } else {
                vec![]
            },
            available_disk_bytes: disk,
            undetected: vec![],
        }
    }

    /// Minimal valid GGUF v3 with an architecture + context length.
    fn write_model(path: &Path, unique_byte: u8) {
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"GGUF");
        buf.extend_from_slice(&3u32.to_le_bytes());
        buf.extend_from_slice(&0u64.to_le_bytes()); // tensors
        buf.extend_from_slice(&2u64.to_le_bytes()); // kv count
        let put_str = |buf: &mut Vec<u8>, s: &str| {
            buf.extend_from_slice(&(s.len() as u64).to_le_bytes());
            buf.extend_from_slice(s.as_bytes());
        };
        put_str(&mut buf, "general.architecture");
        buf.extend_from_slice(&8u32.to_le_bytes());
        put_str(&mut buf, "llama");
        put_str(&mut buf, "llama.context_length");
        buf.extend_from_slice(&4u32.to_le_bytes());
        buf.extend_from_slice(&8192u32.to_le_bytes());
        // Trailing byte makes each fixture's checksum distinct.
        buf.push(unique_byte);

        let mut f = std::fs::File::create(path).unwrap();
        f.write_all(&buf).unwrap();
    }

    fn fresh_db() -> Database {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();
        db
    }

    #[test]
    fn import_registers_model_and_default_profile() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 1);
        let db = fresh_db();

        let result = import_model(
            &db,
            &dir.path().join("managed"),
            &src,
            StorageMode::Reference,
            &hardware(8, Some(100 * 1024 * 1024 * 1024), true),
            &detect_capabilities(),
        )
        .unwrap();

        assert!(!result.deduplicated);
        assert_eq!(result.model.architecture.as_deref(), Some("llama"));

        let profile = get_runtime_profile(&db, &result.model.id).unwrap();
        assert_eq!(profile.context_length, 4096); // capped from the model's 8192
        assert_eq!(profile.threads, 8);
        assert_eq!(profile.gpu_layers, 999); // metal + gpu offload
    }

    #[test]
    fn reimport_same_bytes_is_deduplicated() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 2);
        let db = fresh_db();
        let hw = hardware(4, Some(100 * 1024 * 1024 * 1024), false);

        let first = import_model(
            &db,
            dir.path(),
            &src,
            StorageMode::Reference,
            &hw,
            &detect_capabilities(),
        )
        .unwrap();
        let second = import_model(
            &db,
            dir.path(),
            &src,
            StorageMode::Reference,
            &hw,
            &detect_capabilities(),
        )
        .unwrap();

        assert!(!first.deduplicated);
        assert!(second.deduplicated);
        assert_eq!(first.model.id, second.model.id);
        assert_eq!(
            list_models(&db, Some(8 * 1024 * 1024 * 1024))
                .unwrap()
                .len(),
            1
        );
    }

    #[test]
    fn managed_import_copies_file_and_remove_deletes_it() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 3);
        let managed_dir = dir.path().join("managed");
        let db = fresh_db();

        let result = import_model(
            &db,
            &managed_dir,
            &src,
            StorageMode::Managed,
            &hardware(4, Some(100 * 1024 * 1024 * 1024), false),
            &detect_capabilities(),
        )
        .unwrap();

        let rows = list_models(&db, Some(8 * 1024 * 1024 * 1024)).unwrap();
        let copied = Path::new(&rows[0].file_uri);
        assert!(copied.exists());
        assert_eq!(rows[0].storage_mode, StorageMode::Managed);

        let removal = remove_model(&db, &result.model.id).unwrap();
        assert!(removal.ok);
        assert!(removal.source_file_affected);
        assert!(!copied.exists());
    }

    #[test]
    fn removing_referenced_model_never_deletes_source() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 4);
        let db = fresh_db();

        let result = import_model(
            &db,
            dir.path(),
            &src,
            StorageMode::Reference,
            &hardware(4, Some(100 * 1024 * 1024 * 1024), false),
            &detect_capabilities(),
        )
        .unwrap();

        let removal = remove_model(&db, &result.model.id).unwrap();
        assert!(removal.ok);
        assert!(!removal.source_file_affected);
        assert!(
            src.exists(),
            "referenced source file must never be deleted (FR-MOD-006)"
        );
    }

    #[test]
    fn managed_import_rejects_when_disk_is_too_small() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 5);
        let db = fresh_db();

        let err = import_model(
            &db,
            &dir.path().join("managed"),
            &src,
            StorageMode::Managed,
            &hardware(4, Some(1), false), // 1 byte free
            &detect_capabilities(),
        )
        .unwrap_err();
        assert_eq!(err.code, AppErrorCode::DiskSpaceLow);
    }

    #[test]
    fn import_rejects_non_gguf_file() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("bogus.gguf");
        std::fs::write(&src, b"not a model").unwrap();
        let db = fresh_db();

        let err = import_model(
            &db,
            dir.path(),
            &src,
            StorageMode::Reference,
            &hardware(4, Some(100 * 1024 * 1024 * 1024), false),
            &detect_capabilities(),
        )
        .unwrap_err();
        assert_eq!(err.code, AppErrorCode::ModelInvalid);
    }

    #[test]
    fn update_and_reset_runtime_profile() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 6);
        let db = fresh_db();
        let hw = hardware(8, Some(100 * 1024 * 1024 * 1024), true);
        let caps = detect_capabilities();

        let result =
            import_model(&db, dir.path(), &src, StorageMode::Reference, &hw, &caps).unwrap();
        let mut profile = get_runtime_profile(&db, &result.model.id).unwrap();
        profile.temperature = 0.2;
        profile.context_length = 2048;

        let saved = update_runtime_profile(&db, &profile).unwrap();
        assert!((saved.temperature - 0.2).abs() < f32::EPSILON);
        assert_eq!(saved.context_length, 2048);

        let reset = recommended_runtime_profile(&db, &result.model.id, &hw, &caps).unwrap();
        assert_eq!(reset.context_length, 4096);
        assert_eq!(reset.id, profile.id);
    }

    #[test]
    fn update_rejects_out_of_range_temperature() {
        let dir = tempfile::tempdir().unwrap();
        let src = dir.path().join("model.gguf");
        write_model(&src, 7);
        let db = fresh_db();
        let hw = hardware(8, Some(100 * 1024 * 1024 * 1024), true);
        let caps = detect_capabilities();

        let result =
            import_model(&db, dir.path(), &src, StorageMode::Reference, &hw, &caps).unwrap();
        let mut profile = get_runtime_profile(&db, &result.model.id).unwrap();
        profile.temperature = 9.0;
        assert!(update_runtime_profile(&db, &profile).is_err());
    }

    #[test]
    fn storage_mode_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&StorageMode::Managed).unwrap(),
            "\"managed\""
        );
    }
}

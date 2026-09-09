-- Migration 0003 — models & runtime profiles (Phase 4, PROJECT_REQUIREMENTS.md §7.2).
--
-- `models` holds metadata only; the model bytes live on the filesystem (referenced in
-- place or copied into app-managed storage per `storage_mode`). `sha256` is UNIQUE so a
-- re-import of the same bytes de-duplicates (FR-MOD-001d). Each model owns one or more
-- `runtime_profiles`; exactly one is the default (enforced by a partial unique index).

CREATE TABLE models (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    file_uri      TEXT NOT NULL,
    storage_mode  TEXT NOT NULL CHECK (storage_mode IN ('reference', 'managed')),
    sha256        TEXT NOT NULL UNIQUE,
    size_bytes    INTEGER NOT NULL,
    format        TEXT NOT NULL,
    architecture  TEXT,
    quantization  TEXT,
    metadata_json TEXT NOT NULL,
    last_used_at  TEXT,
    created_at    TEXT NOT NULL
);

-- Library ordering surfaces most-recently-used models first (FR-MOD-002).
CREATE INDEX idx_models_last_used_at ON models (last_used_at);

CREATE TABLE runtime_profiles (
    id                       TEXT PRIMARY KEY,
    model_id                 TEXT NOT NULL REFERENCES models (id) ON DELETE CASCADE,
    engine                   TEXT NOT NULL,
    context_length           INTEGER NOT NULL,
    threads                  INTEGER NOT NULL,
    gpu_layers               INTEGER NOT NULL,
    batch_size               INTEGER NOT NULL,
    generation_defaults_json TEXT NOT NULL,
    is_default               INTEGER NOT NULL DEFAULT 0,
    created_at               TEXT NOT NULL
);

CREATE INDEX idx_runtime_profiles_model_id ON runtime_profiles (model_id);

-- At most one default runtime profile per model.
CREATE UNIQUE INDEX idx_runtime_profiles_one_default
    ON runtime_profiles (model_id) WHERE is_default = 1;

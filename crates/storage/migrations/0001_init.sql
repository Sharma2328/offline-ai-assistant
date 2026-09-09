-- Migration 0001 — baseline.
--
-- Intentionally empty (PROJECT_REQUIREMENTS.md §13, Phase 1: "0001_init.sql empty
-- baseline"). The `schema_migrations` bookkeeping table is created by the migration
-- runner itself (crates/storage/src/migrations.rs), not here.
--
-- Domain tables from §7.2 are introduced by later, feature-scoped migrations
-- (0002+) as each development phase needs them, keeping every schema change tied to
-- a versioned, checksummed migration.

-- No-op statement so the batch is valid SQL and the baseline is explicit.
SELECT 1;

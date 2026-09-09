-- Migration 0002: application settings (key/value).
-- Backs the offline lock (default-on), theme/accessibility, onboarding-complete flag, and
-- score preset (PROJECT_REQUIREMENTS.md §7.2; FR-SET-*, FR-ONB-003). Values are stored as
-- validated JSON text so a setting can hold a bool, string, number, or small object.

CREATE TABLE app_settings (
    key        TEXT PRIMARY KEY NOT NULL,
    value_json TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE benchmark_runs (
 id TEXT PRIMARY KEY, status TEXT NOT NULL, config_json TEXT NOT NULL,
 environment_json TEXT NOT NULL, error TEXT,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE benchmark_case_results (
 run_id TEXT NOT NULL REFERENCES benchmark_runs(id) ON DELETE CASCADE,
 model_id TEXT NOT NULL, case_id TEXT NOT NULL, repetition INTEGER NOT NULL,
 result_json TEXT NOT NULL, PRIMARY KEY(run_id,model_id,case_id,repetition)
);
CREATE INDEX benchmark_created ON benchmark_runs(created_at DESC);

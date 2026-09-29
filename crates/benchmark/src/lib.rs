//! Reproducible benchmark contracts, deterministic scoring, statistics, and reports.
use inference::{GenerationTiming, RuntimeProfile};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use specta::Type;
use std::collections::{BTreeMap, HashMap};
pub mod sandbox;

#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkConfig {
    pub model_ids: Vec<String>,
    pub suite_id: String,
    pub warmup_reps: u32,
    pub measured_reps: u32,
    pub max_tokens: u32,
    pub temperature: f32,
    pub seed: i64,
    pub timeout_ms: u64,
    pub preset: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelSnapshot {
    pub id: String,
    pub name: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub profile: RuntimeProfile,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunConfig {
    pub settings: BenchmarkConfig,
    pub models: Vec<ModelSnapshot>,
    pub dataset_checksum: String,
    pub dataset_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkRun {
    pub id: String,
    pub status: String,
    pub config: RunConfig,
    pub environment: Value,
    pub error: Option<String>,
    pub created_at: String,
    pub completed: u32,
    pub total: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CaseResult {
    #[serde(default)]
    pub scorer: String,
    #[serde(default)]
    pub model_load_time_ms: Option<u64>,
    pub model_id: String,
    pub case_id: String,
    pub category: String,
    pub repetition: u32,
    pub input: String,
    pub output: String,
    pub score: f64,
    pub timing: GenerationTiming,
    pub peak_ram_bytes: Option<u64>,
    pub error: Option<String>,
    pub runtime_version: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ModelMetrics {
    pub model_load_time_ms: Option<u64>,
    pub model_id: String,
    pub name: String,
    pub quality: f64,
    pub quality_by_category: BTreeMap<String, f64>,
    pub latency_median_ms: f64,
    pub latency_p95_ms: f64,
    pub ttft_median_ms: f64,
    pub tokens_per_second_median: f64,
    pub tokens_per_second_mean: f64,
    pub tokens_per_second_std_dev: f64,
    pub peak_ram_bytes: Option<u64>,
    pub failure_rate: f64,
    pub timeout_rate: f64,
    pub overall: Option<f64>,
    pub weights: [f64; 4],
    pub unavailable_metrics: Vec<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkReport {
    pub preset: String,
    pub warnings: Vec<String>,
    pub best_by_category: BTreeMap<String, String>,
    pub schema_version: String,
    pub run: BenchmarkRun,
    pub models: Vec<ModelMetrics>,
    pub cases: Vec<CaseResult>,
    pub best_overall: Option<String>,
    pub best_quality: Option<String>,
    pub fastest: Option<String>,
    pub most_memory_efficient: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, schemars::JsonSchema)]
pub struct Case {
    pub id: String,
    pub category: String,
    pub prompt: String,
    pub expected: Value,
    pub scorer: String,
}

pub fn cases(suite: &str) -> Vec<Case> {
    serde_json::from_str::<Vec<Case>>(include_str!(
        "../../../packages/benchmark-suites/core-v1.json"
    ))
    .expect("validated built-in suite")
    .into_iter()
    .filter(|case| suite == "all" || case.category == suite)
    .collect()
}
pub fn dataset_checksum() -> String {
    format!(
        "{:x}",
        Sha256::digest(include_bytes!(
            "../../../packages/benchmark-suites/core-v1.json"
        ))
    )
}
pub fn validate(config: &BenchmarkConfig) -> Result<(), String> {
    let unique: std::collections::HashSet<_> = config.model_ids.iter().collect();
    if config.model_ids.is_empty()
        || config.model_ids.len() > 8
        || unique.len() != config.model_ids.len()
    {
        return Err("Choose 1–8 distinct models.".into());
    }
    if cases(&config.suite_id).is_empty()
        || !(1..=3).contains(&config.warmup_reps)
        || !(1..=10).contains(&config.measured_reps)
        || !(16..=8192).contains(&config.max_tokens)
        || !config.temperature.is_finite()
        || !(0.0..=2.0).contains(&config.temperature)
        || !(1000..=600000).contains(&config.timeout_ms)
        || !["balanced", "quality", "speed"].contains(&config.preset.as_str())
    {
        return Err("Invalid benchmark settings. Use a built-in suite, at least one warm-up, and valid limits.".into());
    }
    Ok(())
}
fn normalize(text: &str) -> String {
    text.to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .trim_matches(|c: char| c.is_ascii_punctuation())
        .to_string()
}
pub fn score(case: &Case, output: &str) -> Result<f64, String> {
    let expected = case.expected.as_str().unwrap_or("");
    let score = match case.scorer.as_str() {
        "exact_match" => {
            if output.trim() == expected {
                100.0
            } else {
                0.0
            }
        }
        "normalized_match" => {
            if normalize(output) == normalize(expected) {
                100.0
            } else {
                0.0
            }
        }
        "keypoint" => {
            let keys = case
                .expected
                .as_array()
                .ok_or("Key points must be an array.")?;
            if keys.is_empty() {
                return Err("Empty key points.".into());
            }
            100.0
                * keys
                    .iter()
                    .filter(|key| {
                        normalize(output).contains(&normalize(key.as_str().unwrap_or("")))
                    })
                    .count() as f64
                / keys.len() as f64
        }
        "json_fields" => {
            let actual = serde_json::from_str::<Value>(output.trim()).unwrap_or(Value::Null);
            let fields = case
                .expected
                .as_object()
                .ok_or("Expected JSON must be an object.")?;
            if fields.is_empty() {
                return Err("Empty expected JSON.".into());
            }
            100.0
                * fields
                    .iter()
                    .filter(|(key, value)| actual.get(*key) == Some(*value))
                    .count() as f64
                / fields.len() as f64
        }
        "schema_valid" => {
            let validator = jsonschema::validator_for(&case.expected)
                .map_err(|e| format!("Invalid local schema: {e}"))?;
            if serde_json::from_str::<Value>(output).is_ok_and(|value| validator.is_valid(&value)) {
                100.0
            } else {
                0.0
            }
        }
        "citation_recall" => {
            let gold: std::collections::HashSet<_> = case
                .expected
                .as_array()
                .ok_or("Expected citation identifiers must be an array")?
                .iter()
                .filter_map(Value::as_str)
                .collect();
            if gold.is_empty() {
                return Err("Expected at least one gold citation".into());
            }
            let cited: std::collections::HashSet<_> = output
                .split('[')
                .skip(1)
                .filter_map(|part| part.split_once(']').map(|(id, _)| id.trim()))
                .collect();
            100.0 * gold.intersection(&cited).count() as f64 / gold.len() as f64
        }
        "instruction_rules" => {
            let rules = case
                .expected
                .as_array()
                .ok_or("Expected instruction rules must be an array")?;
            if rules.is_empty() {
                return Err("Expected at least one instruction rule".into());
            }
            let mut passed = 0;
            for rule in rules {
                let kind = rule["kind"]
                    .as_str()
                    .ok_or("Instruction rule requires kind")?;
                let satisfied = match kind {
                    "max_words" => {
                        output.split_whitespace().count() as u64
                            <= rule["value"]
                                .as_u64()
                                .ok_or("max_words requires an integer")?
                    }
                    "exact_lines" => {
                        output
                            .lines()
                            .filter(|line| !line.trim().is_empty())
                            .count() as u64
                            == rule["value"]
                                .as_u64()
                                .ok_or("exact_lines requires an integer")?
                    }
                    "contains" => {
                        output.contains(rule["value"].as_str().ok_or("contains requires text")?)
                    }
                    "excludes" => {
                        !output.contains(rule["value"].as_str().ok_or("excludes requires text")?)
                    }
                    "starts_with" => output
                        .trim_start()
                        .starts_with(rule["value"].as_str().ok_or("starts_with requires text")?),
                    "json" => serde_json::from_str::<Value>(output).is_ok(),
                    _ => return Err(format!("Unknown instruction rule: {kind}")),
                };
                if satisfied {
                    passed += 1;
                }
            }
            100.0 * passed as f64 / rules.len() as f64
        }
        "token_f1" | "token_precision" | "token_recall" => {
            let a = normalize(output);
            let b = normalize(expected);
            let a: Vec<_> = a.split_whitespace().collect();
            let b: Vec<_> = b.split_whitespace().collect();
            let mut counts = HashMap::new();
            for token in &b {
                *counts.entry(*token).or_insert(0usize) += 1;
            }
            let mut hits = 0;
            for token in &a {
                if let Some(count) = counts.get_mut(token) {
                    if *count > 0 {
                        hits += 1;
                        *count -= 1;
                    }
                }
            }
            if a.is_empty() || b.is_empty() {
                0.0
            } else {
                match case.scorer.as_str() {
                    "token_precision" => 100.0 * hits as f64 / a.len() as f64,
                    "token_recall" => 100.0 * hits as f64 / b.len() as f64,
                    _ => 200.0 * hits as f64 / (a.len() + b.len()) as f64,
                }
            }
        }
        "unit_tests" => return Err("Code must be evaluated in the restricted runner.".into()),
        _ => return Err("Unknown scorer.".into()),
    };
    Ok(score)
}
pub fn percentile(values: &[f64], p: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let mut values = values.to_vec();
    values.sort_by(f64::total_cmp);
    let index = (values.len() - 1) as f64 * p;
    let low = index.floor() as usize;
    let high = index.ceil() as usize;
    values[low] + (values[high] - values[low]) * (index - low as f64)
}
fn mean(values: &[f64]) -> f64 {
    if values.is_empty() {
        0.0
    } else {
        values.iter().sum::<f64>() / values.len() as f64
    }
}
fn normalize_metric(value: f64, values: &[f64], inverse: bool) -> f64 {
    let min = values.iter().copied().fold(f64::INFINITY, f64::min);
    let max = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if (max - min).abs() < f64::EPSILON {
        100.0
    } else if inverse {
        100.0 * (max - value) / (max - min)
    } else {
        100.0 * (value - min) / (max - min)
    }
}
pub fn report(run: BenchmarkRun, results: Vec<CaseResult>, preset: &str) -> BenchmarkReport {
    let mut models = Vec::new();
    for snapshot in &run.config.models {
        let rows: Vec<_> = results
            .iter()
            .filter(|r| r.model_id == snapshot.id)
            .collect();
        if rows.is_empty() {
            continue;
        }
        let valid: Vec<_> = rows.iter().copied().filter(|r| r.error.is_none()).collect();
        let mut categories: BTreeMap<String, Vec<f64>> = BTreeMap::new();
        for row in &rows {
            categories
                .entry(row.category.clone())
                .or_default()
                .push(row.score);
        }
        let quality_by_category: BTreeMap<String, f64> = categories
            .into_iter()
            .map(|(category, scores)| (category, mean(&scores)))
            .collect();
        let latency: Vec<_> = valid.iter().map(|r| r.timing.total_ms as f64).collect();
        let ttft: Vec<_> = valid.iter().map(|r| r.timing.ttft_ms as f64).collect();
        let speed: Vec<_> = valid
            .iter()
            .map(|r| f64::from(r.timing.tokens_per_second))
            .collect();
        let speed_mean = mean(&speed);
        let ram = rows.iter().filter_map(|r| r.peak_ram_bytes).max();
        models.push(ModelMetrics {
            model_load_time_ms: rows.iter().find_map(|row| row.model_load_time_ms),
            model_id: snapshot.id.clone(),
            name: snapshot.name.clone(),
            quality: mean(&quality_by_category.values().copied().collect::<Vec<_>>()),
            quality_by_category,
            latency_median_ms: percentile(&latency, 0.5),
            latency_p95_ms: percentile(&latency, 0.95),
            ttft_median_ms: percentile(&ttft, 0.5),
            tokens_per_second_median: percentile(&speed, 0.5),
            tokens_per_second_mean: speed_mean,
            tokens_per_second_std_dev: mean(
                &speed
                    .iter()
                    .map(|s| (s - speed_mean).powi(2))
                    .collect::<Vec<_>>(),
            )
            .sqrt(),
            peak_ram_bytes: ram,
            failure_rate: 100.0 * (rows.len() - valid.len()) as f64 / rows.len() as f64,
            timeout_rate: 100.0
                * rows
                    .iter()
                    .filter(|r| r.error.as_deref() == Some("BENCH_CASE_TIMEOUT"))
                    .count() as f64
                / rows.len() as f64,
            overall: None,
            weights: [0.0; 4],
            unavailable_metrics: vec!["VRAM".into(), "thermal state".into(), "energy".into()],
        });
    }
    let eligible = |m: &&ModelMetrics| m.failure_rate < 100.0;
    let comparable = run.status == "completed"
        && run.completed == run.total
        && models.iter().filter(eligible).count() > 1;
    let quality: Vec<_> = models.iter().filter(eligible).map(|m| m.quality).collect();
    let speed: Vec<_> = models
        .iter()
        .filter(eligible)
        .map(|m| m.tokens_per_second_median)
        .collect();
    let ram: Vec<_> = models
        .iter()
        .filter_map(|m| m.peak_ram_bytes.map(|v| v as f64))
        .collect();
    let count = models.len();
    for model in &mut models {
        let mut weights = match preset {
            "quality" => [0.8, 0.1, 0.05, 0.05],
            "speed" => [0.4, 0.35, 0.15, 0.1],
            _ => [0.6, 0.2, 0.1, 0.1],
        };
        if ram.len() != count {
            weights[2] = 0.0;
            model
                .unavailable_metrics
                .push("RAM (weight redistributed)".into());
        }
        let total: f64 = weights.iter().sum();
        for weight in &mut weights {
            *weight /= total;
        }
        model.weights = weights;
        if comparable && model.failure_rate < 100.0 {
            model.overall = Some(
                weights[0] * normalize_metric(model.quality, &quality, false)
                    + weights[1] * normalize_metric(model.tokens_per_second_median, &speed, false)
                    + weights[2]
                        * model
                            .peak_ram_bytes
                            .map_or(0.0, |v| normalize_metric(v as f64, &ram, true))
                    + weights[3] * (100.0 - model.failure_rate),
            );
        }
    }
    let best_overall = if comparable {
        models
            .iter()
            .filter(eligible)
            .max_by(|a, b| {
                a.overall
                    .unwrap_or(0.0)
                    .total_cmp(&b.overall.unwrap_or(0.0))
            })
            .map(|m| m.name.clone())
    } else {
        None
    };
    let best_quality = models
        .iter()
        .filter(eligible)
        .max_by(|a, b| a.quality.total_cmp(&b.quality))
        .map(|m| m.name.clone());
    let fastest = models
        .iter()
        .filter(|m| m.failure_rate < 100.0)
        .min_by(|a, b| {
            a.latency_median_ms
                .total_cmp(&b.latency_median_ms)
                .then_with(|| {
                    b.tokens_per_second_median
                        .total_cmp(&a.tokens_per_second_median)
                })
        })
        .map(|m| m.name.clone());
    let most_memory_efficient = models
        .iter()
        .filter(|m| m.peak_ram_bytes.is_some() && m.failure_rate < 100.0)
        .min_by_key(|m| m.peak_ram_bytes)
        .map(|m| m.name.clone());
    let mut best_by_category = BTreeMap::new();
    if run.status == "completed" {
        let categories: std::collections::BTreeSet<_> = models
            .iter()
            .flat_map(|m| m.quality_by_category.keys())
            .collect();
        for category in categories {
            if let Some(best) = models
                .iter()
                .filter(eligible)
                .filter(|m| m.quality_by_category.contains_key(category))
                .max_by(|a, b| {
                    a.quality_by_category[category].total_cmp(&b.quality_by_category[category])
                })
            {
                best_by_category.insert(category.clone(), best.name.clone());
            }
        }
    }
    let warnings = run.environment["warnings"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    BenchmarkReport {
        preset: preset.into(),
        warnings,
        best_by_category,
        schema_version: "1.0".into(),
        run,
        models,
        cases: results,
        best_overall,
        best_quality,
        fastest,
        most_memory_efficient,
    }
}
/// Redact paths in JSON string values before serialization (including generated responses).
pub fn redact_paths(value: &mut Value) {
    match value {
        Value::String(text) => {
            static PATHS: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            let pattern = PATHS.get_or_init(|| {
                regex::Regex::new(
                    r#"(?m)(^|[\s"'=(])(?:/(?:[^/\s"'<>][^\r\n"'<>]*)|[A-Za-z]:\\[^\r\n"'<>]+)"#,
                )
                .expect("path pattern")
            });
            *text = pattern.replace_all(text, "${1}[local path]").into_owned();
        }
        Value::Array(values) => {
            for value in values {
                redact_paths(value);
            }
        }
        Value::Object(values) => {
            for value in values.values_mut() {
                redact_paths(value);
            }
        }
        _ => {}
    }
}

pub fn csv(report: &BenchmarkReport) -> String {
    let mut output="model,overall,quality,ttft_ms_median,tokens_per_second_mean,latency_ms_median,peak_ram_mib,peak_vram_mib,failure_rate,timeout_rate,preset\n".to_string();
    for model in &report.models {
        let safe = if model.name.starts_with(['=', '+', '-', '@', '\t', '\r']) {
            format!("'{}", model.name)
        } else {
            model.name.clone()
        };
        output.push_str(&format!(
            "\"{}\",{},{:.2},{:.2},{:.2},{:.2},{},,{:.2},{:.2},{}\n",
            safe.replace('"', "\"\""),
            model.overall.map(|v| format!("{v:.2}")).unwrap_or_default(),
            model.quality,
            model.ttft_median_ms,
            model.tokens_per_second_mean,
            model.latency_median_ms,
            model
                .peak_ram_bytes
                .map(|v| format!("{:.2}", v as f64 / 1048576.0))
                .unwrap_or_default(),
            model.failure_rate,
            model.timeout_rate,
            report.preset
        ));
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (BenchmarkRun, Vec<CaseResult>) {
        let profile = RuntimeProfile {
            id: "profile".into(),
            model_id: "a".into(),
            engine: inference::RuntimeEngine::LlamaCpp,
            context_length: 4096,
            max_tokens: 256,
            temperature: 0.0,
            top_p: 0.95,
            top_k: 40,
            repeat_penalty: 1.1,
            seed: Some(42),
            threads: 4,
            batch_size: 512,
            gpu_layers: 0,
        };
        let models = ["a", "b"]
            .iter()
            .map(|id| ModelSnapshot {
                id: (*id).into(),
                name: (*id).into(),
                sha256: "sha".into(),
                size_bytes: 1,
                profile: RuntimeProfile {
                    model_id: (*id).into(),
                    ..profile.clone()
                },
            })
            .collect();
        let run = BenchmarkRun {
            id: "run".into(),
            status: "completed".into(),
            config: RunConfig {
                settings: BenchmarkConfig {
                    model_ids: vec!["a".into(), "b".into()],
                    suite_id: "reasoning".into(),
                    warmup_reps: 1,
                    measured_reps: 1,
                    max_tokens: 256,
                    temperature: 0.0,
                    seed: 42,
                    timeout_ms: 1000,
                    preset: "balanced".into(),
                },
                models,
                dataset_checksum: dataset_checksum(),
                dataset_version: "core-v1".into(),
            },
            environment: Value::Null,
            error: None,
            created_at: "now".into(),
            completed: 4,
            total: 4,
        };
        let rows = ["a", "b"]
            .iter()
            .flat_map(|id| {
                (0..2).map(move |i| CaseResult {
                    scorer: "exact_match".into(),
                    model_load_time_ms: Some(100),
                    model_id: (*id).into(),
                    case_id: format!("case-{i}"),
                    category: "reasoning".into(),
                    repetition: 0,
                    input: "Task".into(),
                    output: "Answer".into(),
                    score: if *id == "a" { 90.0 } else { 40.0 },
                    timing: GenerationTiming {
                        ttft_ms: 10,
                        total_ms: 100,
                        output_tokens: 10,
                        tokens_per_second: if *id == "a" { 20.0 } else { 40.0 },
                    },
                    peak_ram_bytes: None,
                    error: None,
                    runtime_version: "test".into(),
                })
            })
            .collect();
        (run, rows)
    }
    #[test]
    fn schema_citation_and_instruction_scorers_reject_false_positives() {
        let mut case = Case {
            id: "test".into(),
            category: "extraction".into(),
            prompt: "test".into(),
            expected: serde_json::json!({"type":"object","required":["count"],"properties":{"count":{"type":"integer"}},"additionalProperties":false}),
            scorer: "schema_valid".into(),
        };
        assert_eq!(score(&case, r#"{"count":2}"#).unwrap(), 100.0);
        assert_eq!(score(&case, r#"{"count":"2"}"#).unwrap(), 0.0);
        case.expected = serde_json::json!({"$ref":"https://invalid.example/schema.json"});
        assert!(score(&case, "{}").is_err());
        case.expected = serde_json::json!(["1", "2"]);
        case.scorer = "citation_recall".into();
        assert_eq!(score(&case, "[1] [1] [7]").unwrap(), 50.0);
        case.expected = serde_json::json!([{"kind":"max_words","value":3},{"kind":"starts_with","value":"Yes"}]);
        case.scorer = "instruction_rules".into();
        assert_eq!(score(&case, "Yes this works").unwrap(), 100.0);
        assert_eq!(score(&case, "No this works").unwrap(), 50.0);
    }
    #[test]
    fn published_export_schema_matches_and_validates_reports() {
        let generated = serde_json::to_value(schemars::schema_for!(BenchmarkReport)).unwrap();
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../packages/benchmark-suites/report.schema.json");
        if std::env::var_os("UPDATE_SCHEMAS").is_some() {
            std::fs::write(
                &path,
                serde_json::to_string_pretty(&generated).unwrap() + "\n",
            )
            .unwrap();
        }
        let published: Value = serde_json::from_slice(
            &std::fs::read(path).expect("Generate the export schema using UPDATE_SCHEMAS=1"),
        )
        .unwrap();
        assert_eq!(generated, published, "Published schema drifted");
        let validator = jsonschema::validator_for(&published).unwrap();
        let (run, rows) = fixture();
        let value = serde_json::to_value(report(run, rows, "balanced")).unwrap();
        assert!(validator.is_valid(&value));
        let mut invalid = value;
        invalid["models"][0]["quality"] = Value::String("invalid".into());
        assert!(!validator.is_valid(&invalid));
    }

    #[test]
    fn paths_are_redacted_before_json_escaping() {
        let mut value = serde_json::json!({"posix":"Read /Users/Ada/My Documents/private.txt","windows":r"C:\Users\Ada\secret.txt","url":"https://example.com/data","number":2});
        redact_paths(&mut value);
        assert_eq!(value["posix"], "Read [local path]");
        assert_eq!(value["windows"], "[local path]");
        assert_eq!(value["url"], "https://example.com/data");
    }

    #[test]
    fn failed_and_partial_comparisons_have_no_overall_winner() {
        let (mut run, mut rows) = fixture();
        run.status = "paused".into();
        assert!(report(run.clone(), rows.clone(), "balanced")
            .best_overall
            .is_none());
        run.status = "completed".into();
        for row in &mut rows {
            row.error = Some("BENCH_CASE_TIMEOUT".into());
            row.score = 0.0;
        }
        let report = report(run, rows, "balanced");
        assert!(report.best_overall.is_none());
        assert!(report.best_quality.is_none());
        assert!(report.fastest.is_none());
        assert!(report.models.iter().all(|m| m.overall.is_none()));
    }
    #[test]
    fn missing_ram_redistributes_weights_without_changing_raw_quality() {
        let (run, rows) = fixture();
        let balanced = report(run.clone(), rows.clone(), "balanced");
        let speed = report(run, rows, "speed");
        for (a, b) in balanced.models.iter().zip(&speed.models) {
            assert_eq!(a.quality, b.quality);
            assert_eq!(a.weights[2], 0.0);
            assert!((a.weights.iter().sum::<f64>() - 1.0).abs() < 1e-9);
        }
        assert_eq!(balanced.best_overall.as_deref(), Some("a"));
    }
    #[test]
    fn csv_escapes_formula_cells_and_quotes() {
        let (mut run, rows) = fixture();
        run.config.models[0].name = "=SUM(1,2)\"".into();
        assert!(csv(&report(run, rows, "balanced")).contains("\"'=SUM(1,2)\"\"\""));
    }

    #[test]
    fn original_suites_have_unique_cases_and_scorers() {
        let all = cases("all");
        assert_eq!(all.len(), 12);
        let ids: std::collections::HashSet<_> = all.iter().map(|c| &c.id).collect();
        assert_eq!(ids.len(), all.len());
        for case in &all {
            if case.scorer == "unit_tests" {
                continue;
            }
            let ideal = if case.expected.is_string() {
                case.expected.as_str().unwrap().to_string()
            } else if case.expected.is_array() {
                case.expected
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap())
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                case.expected.to_string()
            };
            assert_eq!(score(case, &ideal).unwrap(), 100.0, "{}", case.id);
        }
    }
    #[test]
    fn repeated_tokens_do_not_inflate_f1() {
        let case = Case {
            id: "test".into(),
            category: "retrieval".into(),
            prompt: String::new(),
            expected: Value::String("a b".into()),
            scorer: "token_f1".into(),
        };
        assert_eq!(score(&case, "a a").unwrap(), 50.0);
    }
    #[test]
    fn percentile_interpolates() {
        assert_eq!(percentile(&[40.0, 10.0, 20.0, 30.0], 0.5), 25.0);
        assert_eq!(percentile(&[], 0.95), 0.0);
    }
}

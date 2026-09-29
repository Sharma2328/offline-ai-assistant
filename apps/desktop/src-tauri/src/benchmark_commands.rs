use crate::runtime::{read_string_setting, RuntimeState, KEY_RUNTIME_BINARY_PATH};
use crate::{lock_db, Db};
use app_core::{AppError, AppResult};
use benchmark::{
    BenchmarkConfig, BenchmarkReport, BenchmarkRun, CaseResult, ModelSnapshot, RunConfig,
};
use inference::{
    ChatMessage, ChatRole, GenerationRequest, GenerationTiming, InferenceAdapter, LlamaAdapter,
};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::{
    collections::HashSet,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tauri::{Manager, State};
use tokio_util::sync::CancellationToken;

struct ActiveRun {
    started: std::time::Instant,
    id: String,
    cancel: CancellationToken,
    pause: Arc<AtomicBool>,
}
#[derive(Default)]
pub struct BenchmarkState {
    active: Mutex<Option<ActiveRun>>,
    progress: Mutex<Option<BenchmarkProgress>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BenchmarkProgress {
    pub run_id: String,
    pub model_name: String,
    pub case_id: Option<String>,
    pub phase: String,
    pub completed: u32,
    pub total: u32,
    pub elapsed_ms: u64,
    pub eta_ms: Option<u64>,
    pub current_ram_bytes: Option<u64>,
    pub current_cpu_percent: Option<f32>,
}
#[tauri::command]
#[specta::specta]
pub fn benchmarks_progress(
    state: State<'_, BenchmarkState>,
    id: String,
) -> AppResult<Option<BenchmarkProgress>> {
    let elapsed = state
        .active
        .lock()
        .map_err(error)?
        .as_ref()
        .filter(|run| run.id == id)
        .map(|run| run.started.elapsed().as_millis() as u64);
    let mut progress = state
        .progress
        .lock()
        .map_err(error)?
        .as_ref()
        .filter(|p| p.run_id == id)
        .cloned();
    if let (Some(progress), Some(elapsed)) = (&mut progress, elapsed) {
        progress.elapsed_ms = elapsed;
    }
    Ok(progress)
}

fn set_progress(app: &tauri::AppHandle, progress: BenchmarkProgress) {
    if let Ok(mut state) = app.state::<BenchmarkState>().progress.lock() {
        *state = Some(progress);
    }
}
fn record_model_environment(
    app: &tauri::AppHandle,
    run: &str,
    model: &str,
    value: serde_json::Value,
) -> AppResult<()> {
    let db = app.state::<Db>();
    let db = lock_db(&db)?;
    let json: String = db
        .connection()
        .query_row(
            "SELECT environment_json FROM benchmark_runs WHERE id=?1",
            [run],
            |row| row.get(0),
        )
        .map_err(error)?;
    let mut environment: serde_json::Value = serde_json::from_str(&json).map_err(error)?;
    if !environment["modelRuns"].is_object() {
        environment["modelRuns"] = serde_json::json!({});
    }
    environment["modelRuns"][model] = value;
    db.connection()
        .execute(
            "UPDATE benchmark_runs SET environment_json=?2 WHERE id=?1",
            params![run, environment.to_string()],
        )
        .map_err(error)?;
    Ok(())
}

fn error(message: impl ToString) -> AppError {
    AppError::internal(message.to_string())
}
fn read_run(db: &storage::Database, id: &str) -> AppResult<BenchmarkRun> {
    let (status,config,environment,run_error,created):(String,String,String,Option<String>,String)=db.connection().query_row("SELECT status,config_json,environment_json,error,created_at FROM benchmark_runs WHERE id=?1",[id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?))).map_err(error)?;
    let config: RunConfig = serde_json::from_str(&config).map_err(error)?;
    let completed: u32 = db
        .connection()
        .query_row(
            "SELECT COUNT(*) FROM benchmark_case_results WHERE run_id=?1",
            [id],
            |r| r.get(0),
        )
        .map_err(error)?;
    let total = config.settings.model_ids.len() as u32
        * benchmark::cases(&config.settings.suite_id).len() as u32
        * config.settings.measured_reps;
    Ok(BenchmarkRun {
        id: id.into(),
        status,
        config,
        environment: serde_json::from_str(&environment).map_err(error)?,
        error: run_error,
        created_at: created,
        completed,
        total,
    })
}
fn set_status(
    app: &tauri::AppHandle,
    id: &str,
    status: &str,
    message: Option<&str>,
) -> AppResult<()> {
    let state = app.state::<Db>();
    let db = lock_db(&state)?;
    db.connection()
        .execute(
            "UPDATE benchmark_runs SET status=?2,error=?3 WHERE id=?1",
            params![id, status, message],
        )
        .map_err(error)?;
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub fn benchmarks_list(db: State<'_, Db>) -> AppResult<Vec<BenchmarkRun>> {
    let db = lock_db(&db)?;
    let mut statement = db
        .connection()
        .prepare("SELECT id FROM benchmark_runs ORDER BY created_at DESC LIMIT 200")
        .map_err(error)?;
    let ids = statement
        .query_map([], |r| r.get::<_, String>(0))
        .map_err(error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(error)?;
    ids.iter().map(|id| read_run(&db, id)).collect()
}
#[tauri::command]
#[specta::specta]
pub fn benchmarks_create(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    config: BenchmarkConfig,
) -> AppResult<String> {
    benchmark::validate(&config).map_err(error)?;
    let db = lock_db(&db)?;
    let mut models = Vec::new();
    for id in &config.model_ids {
        let model = db
            .models()
            .get(id)
            .map_err(error)?
            .ok_or_else(|| error("Model was removed."))?;
        let mut profile = app_core::get_runtime_profile(&db, id)?;
        if config.max_tokens >= profile.context_length {
            return Err(error(format!(
                "{} needs a response limit below its {}-token context window.",
                model.name, profile.context_length
            )));
        }
        profile.max_tokens = config.max_tokens;
        profile.temperature = config.temperature;
        profile.seed = Some(config.seed);
        profile.top_p = 0.95;
        profile.top_k = 40;
        profile.repeat_penalty = 1.1;
        models.push(ModelSnapshot {
            id: id.clone(),
            name: model.name,
            sha256: model.sha256,
            size_bytes: model.size_bytes as u64,
            profile,
        });
    }
    let run = RunConfig {
        settings: config,
        models,
        dataset_checksum: benchmark::dataset_checksum(),
        dataset_version: "core-v1".into(),
    };
    let id = uuid::Uuid::new_v4().to_string();
    let environment = serde_json::json!({"appVersion":app_core::APP_VERSION,"hardware":crate::app_hardware(&app),"runtimeEngine":"llama.cpp","sampling":{"topP":0.95,"topK":40,"repeatPenalty":1.1},"datasetLicense":"CC0-1.0"});
    db.connection().execute("INSERT INTO benchmark_runs(id,status,config_json,environment_json) VALUES(?1,'created',?2,?3)",params![id,serde_json::to_string(&run).map_err(error)?,environment.to_string()]).map_err(error)?;
    Ok(id)
}
#[tauri::command]
#[specta::specta]
pub fn benchmarks_report(
    db: State<'_, Db>,
    id: String,
    preset: String,
) -> AppResult<BenchmarkReport> {
    let db = lock_db(&db)?;
    let run = read_run(&db, &id)?;
    let mut statement=db.connection().prepare("SELECT result_json FROM benchmark_case_results WHERE run_id=?1 ORDER BY model_id,case_id,repetition").map_err(error)?;
    let rows = statement
        .query_map([&id], |r| r.get::<_, String>(0))
        .map_err(error)?;
    let mut results = Vec::new();
    for row in rows {
        results.push(serde_json::from_str(&row.map_err(error)?).map_err(error)?);
    }
    Ok(benchmark::report(run, results, &preset))
}
#[tauri::command]
#[specta::specta]
pub async fn benchmarks_start(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    state: State<'_, BenchmarkState>,
    id: String,
) -> AppResult<()> {
    let run = {
        let db = lock_db(&db)?;
        read_run(&db, &id)?
    };
    if run.status == "completed" || run.status == "running" {
        return Err(error(
            "This run is completed or already running. Create a new run to rerun it.",
        ));
    }
    if run.config.dataset_checksum != benchmark::dataset_checksum() {
        return Err(error("The dataset changed. Create a fresh benchmark."));
    }
    let cancel = CancellationToken::new();
    let pause = Arc::new(AtomicBool::new(false));
    {
        let mut active = state.active.lock().map_err(error)?;
        if active.is_some() {
            return Err(error("Another benchmark is running."));
        }
        *active = Some(ActiveRun {
            started: std::time::Instant::now(),
            id: id.clone(),
            cancel: cancel.clone(),
            pause: pause.clone(),
        });
    }
    if let Err(err) = set_status(&app, &id, "running", None) {
        *state.active.lock().map_err(error)? = None;
        return Err(err);
    }
    tauri::async_runtime::spawn(async move {
        let result = execute(&app, &run, cancel.clone(), pause.clone()).await;
        let (status, message) = match result {
            Ok(()) => {
                if cancel.is_cancelled() {
                    ("cancelled", None)
                } else if pause.load(Ordering::SeqCst) {
                    ("paused", None)
                } else {
                    ("completed", None)
                }
            }
            Err(err) => ("interrupted", Some(err.message)),
        };
        let _ = set_status(&app, &id, status, message.as_deref());
        crate::maintenance::log(&app, "BENCHMARK_FINISHED", status);
        if let Ok(mut active) = app.state::<BenchmarkState>().active.lock() {
            *active = None;
        }
    });
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub fn benchmarks_control(
    state: State<'_, BenchmarkState>,
    id: String,
    action: String,
) -> AppResult<()> {
    let active = state.active.lock().map_err(error)?;
    let run = active
        .as_ref()
        .filter(|r| r.id == id)
        .ok_or_else(|| error("The run is no longer active."))?;
    match action.as_str() {
        "pause" => run.pause.store(true, Ordering::SeqCst),
        "cancel" => run.cancel.cancel(),
        _ => return Err(error("Unknown benchmark action.")),
    };
    Ok(())
}

async fn execute(
    app: &tauri::AppHandle,
    run: &BenchmarkRun,
    cancel: CancellationToken,
    pause: Arc<AtomicBool>,
) -> AppResult<()> {
    let runtime = app.state::<RuntimeState>();
    let _operation = runtime
        .operation
        .try_lock()
        .map_err(|_| error("Stop chat generation before starting a benchmark."))?;
    if let Ok(adapter) = crate::runtime::current_adapter(&runtime).await {
        if let Some(id) = adapter.loaded_model_id().await {
            adapter
                .unload_model(&id)
                .await
                .map_err(app_core::map_inference_error)?;
        }
    }
    let mut background = sysinfo::System::new();
    background.refresh_cpu_usage();
    tokio::time::sleep(sysinfo::MINIMUM_CPU_UPDATE_INTERVAL).await;
    background.refresh_cpu_usage();
    {
        let db = app.state::<Db>();
        let db = lock_db(&db)?;
        let json: String = db
            .connection()
            .query_row(
                "SELECT environment_json FROM benchmark_runs WHERE id=?1",
                [&run.id],
                |row| row.get(0),
            )
            .map_err(error)?;
        let mut environment: serde_json::Value = serde_json::from_str(&json).map_err(error)?;
        environment["backgroundCpuPercent"] = serde_json::json!(background.global_cpu_usage());
        environment["warnings"] = if background.global_cpu_usage() > 30.0 {
            serde_json::json!(["Background CPU load exceeded 30% at measurement start; close other demanding apps before comparing results."])
        } else {
            serde_json::json!([])
        };
        db.connection()
            .execute(
                "UPDATE benchmark_runs SET environment_json=?2 WHERE id=?1",
                params![run.id, environment.to_string()],
            )
            .map_err(error)?;
    }
    let db = app.state::<Db>();
    let configured = read_string_setting(&db, KEY_RUNTIME_BINARY_PATH)?;
    let binary = inference::resolve_llama_binary(configured.as_deref())
        .ok_or_else(|| error("Set a llama-server path in Settings."))?;
    let adapter = Arc::new(LlamaAdapter::new(binary, Arc::new(|_| {})));
    let started = std::time::Instant::now();
    let mut progress = BenchmarkProgress {
        run_id: run.id.clone(),
        model_name: String::new(),
        case_id: None,
        phase: "loading".into(),
        completed: run.completed,
        total: run.total,
        elapsed_ms: 0,
        eta_ms: None,
        current_ram_bytes: None,
        current_cpu_percent: None,
    };
    let result = async {
        let cases = benchmark::cases(&run.config.settings.suite_id);
        let (completed, mut measured_ms): (HashSet<(String,String,u32)>,u64) = {
            let db = lock_db(&db)?;
            let mut statement = db.connection().prepare("SELECT model_id,case_id,repetition FROM benchmark_case_results WHERE run_id=?1").map_err(error)?;
            let rows = statement.query_map([&run.id],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?))).map_err(error)?;
            let elapsed=db.connection().query_row("SELECT COALESCE(SUM(json_extract(result_json,'$.timing.totalMs')),0) FROM benchmark_case_results WHERE run_id=?1",[&run.id],|r|r.get(0)).map_err(error)?;
            (rows.collect::<Result<_,_>>().map_err(error)?,elapsed)
        };
        for model in &run.config.models {
            if cancel.is_cancelled() || pause.load(Ordering::SeqCst) { break; }
            if cases.iter().all(|case|(0..run.config.settings.measured_reps).all(|rep|completed.contains(&(model.id.clone(),case.id.clone(),rep)))) {continue;}
            progress.model_name=model.name.clone(); progress.phase="loading".into();progress.case_id=None;set_progress(app,progress.clone());
            let mut request = {let db=lock_db(&db)?;app_core::resolve_load_request(&db,&model.id)?};
            let path=request.model_path.clone();
            let metadata=tauri::async_runtime::spawn_blocking(move||inference::gguf::inspect_model(&path)).await.map_err(error)?.map_err(error)?;
            if metadata.sha256!=model.sha256 {return Err(error("A model checksum changed. Create a new benchmark."));}
            request.profile=model.profile.clone();
            let assessment={let db=lock_db(&db)?;app_core::estimate_compatibility(&db,&model.id,&request.profile,crate::app_hardware(app).available_memory_bytes)?};
            if assessment.blocking {return Err(error(assessment.reasons.join(" ")));}
            let loaded=adapter.load_model(request).await.map_err(app_core::map_inference_error)?;
            let prior_version:Option<String>={let db=lock_db(&db)?; db.connection().query_row("SELECT json_extract(result_json,'$.runtimeVersion') FROM benchmark_case_results WHERE run_id=?1 AND model_id=?2 LIMIT 1",params![run.id,model.id],|r|r.get(0)).optional().map_err(error)?};
            if prior_version.is_some_and(|version|version!=loaded.capabilities.engine_version) {return Err(error("Runtime version changed. Create a fresh benchmark."));}
            let mut warmups=Vec::new();
            for _ in 0..run.config.settings.warmup_reps {
                if cancel.is_cancelled(){break;}
                progress.phase="warming up".into();progress.elapsed_ms=started.elapsed().as_millis() as u64;set_progress(app,progress.clone());
                let warmup=generate_case(&adapter,model,&cases[0],cancel.clone(),run.config.settings.timeout_ms).await.map_err(|e|error(format!("Warm-up failed: {e}")))?;
                warmups.push(serde_json::json!({"isWarmup":true,"caseId":cases[0].id,"timing":warmup.timing}));
            }
            record_model_environment(app,&run.id,&model.id,serde_json::json!({"loadTimeMs":loaded.load_time_ms,"runtimeVersion":loaded.capabilities.engine_version,"warmups":warmups}))?;
            for case in &cases {
                for repetition in 0..run.config.settings.measured_reps {
                    if cancel.is_cancelled()||pause.load(Ordering::SeqCst){break;}
                    if completed.contains(&(model.id.clone(),case.id.clone(),repetition)){continue;}
                    progress.case_id=Some(case.id.clone());progress.phase="measuring".into();progress.elapsed_ms=started.elapsed().as_millis() as u64;
                    progress.eta_ms=if progress.completed>0 {Some(measured_ms/u64::from(progress.completed)*u64::from(progress.total-progress.completed))}else{None};
                    set_progress(app,progress.clone());
                    let monitor_cancel=CancellationToken::new(); let monitor_end=monitor_cancel.clone(); let pid=adapter.process_id();let monitor_app=app.clone();let mut live=progress.clone();
                    let monitor=tokio::spawn(async move {
                        let pid=sysinfo::Pid::from_u32(pid?);let mut system=sysinfo::System::new();let mut peak=0;
                        loop {
                            system.refresh_processes(sysinfo::ProcessesToUpdate::Some(&[pid]),true);
                            if let Some(process)=system.process(pid){peak=peak.max(process.memory());live.current_ram_bytes=Some(process.memory());live.current_cpu_percent=Some(process.cpu_usage());}
                            live.elapsed_ms=started.elapsed().as_millis() as u64;set_progress(&monitor_app,live.clone());
                            tokio::select!{_=monitor_end.cancelled()=>break,_=tokio::time::sleep(Duration::from_millis(250))=>{}}
                        }
                        if peak>0 {Some(peak)}else{None}
                    });
                    let case_started=std::time::Instant::now();
                    let generated=generate_case(&adapter,model,case,cancel.clone(),run.config.settings.timeout_ms).await;
                    monitor_cancel.cancel();let peak_ram_bytes=monitor.await.unwrap_or(None);
                    if cancel.is_cancelled(){break;}
                    let (output,timing,mut case_error)=match generated {Ok(result)=>(result.text,result.timing,None),Err(err)=>(String::new(),GenerationTiming{ttft_ms:0,total_ms:case_started.elapsed().as_millis() as u64,output_tokens:0,tokens_per_second:0.0},Some(err))};
                    let score=if case_error.is_some(){0.0}else{
                        let scored=if case.scorer=="unit_tests" {benchmark::sandbox::run(&output,case.expected.as_str().unwrap_or(""),cancel.clone()).await}else{benchmark::score(case,&output)};
                        match scored {Ok(value)=>value,Err(err)=>{case_error=Some(err);0.0}}
                    };
                    if cancel.is_cancelled(){break;}
                    let result=CaseResult {scorer:case.scorer.clone(),model_load_time_ms:Some(loaded.load_time_ms),model_id:model.id.clone(),case_id:case.id.clone(),category:case.category.clone(),repetition,input:case.prompt.clone(),output,score,timing,peak_ram_bytes,error:case_error,runtime_version:loaded.capabilities.engine_version.clone()};
                    {let db=lock_db(&db)?;db.connection().execute("INSERT INTO benchmark_case_results(run_id,model_id,case_id,repetition,result_json) VALUES(?1,?2,?3,?4,?5)",params![run.id,model.id,case.id,repetition,serde_json::to_string(&result).map_err(error)?]).map_err(error)?;}
                    progress.completed+=1;measured_ms+=timing.total_ms;progress.elapsed_ms=started.elapsed().as_millis() as u64;set_progress(app,progress.clone());
                }
            }
            adapter.unload_model(&model.id).await.map_err(app_core::map_inference_error)?;
        }
        Ok(())
    }.await;
    if let Some(id) = adapter.loaded_model_id().await {
        let _ = adapter.unload_model(&id).await;
    }
    result
}
async fn generate_case(
    adapter: &LlamaAdapter,
    model: &ModelSnapshot,
    case: &benchmark::Case,
    cancel: CancellationToken,
    timeout_ms: u64,
) -> Result<inference::GenerationResult, String> {
    let request = GenerationRequest {
        correlation_id: uuid::Uuid::new_v4().to_string(),
        model_id: model.id.clone(),
        messages: vec![
            ChatMessage {
                role: ChatRole::System,
                content: "Follow the task instructions precisely.".into(),
            },
            ChatMessage {
                role: ChatRole::User,
                content: case.prompt.clone(),
            },
        ],
        profile: model.profile.clone(),
        stop: vec![],
    };
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let drain = tokio::spawn(async move { while receiver.recv().await.is_some() {} });
    let result = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        adapter.generate(request, sender, cancel),
    )
    .await
    .map_err(|_| "BENCH_CASE_TIMEOUT".to_string())
    .and_then(|result| result.map_err(|e| e.to_string()));
    let _ = drain.await;
    result
}

#[tauri::command]
#[specta::specta]
pub fn benchmarks_export(
    db: State<'_, Db>,
    id: String,
    preset: String,
    kind: String,
    path: String,
    selected_responses: Vec<String>,
) -> AppResult<()> {
    let report = benchmarks_report(db, id, preset)?;
    let mut value = if kind == "responses" {
        if selected_responses.is_empty() {
            return Err(error("Select at least one response to export."));
        }
        let cases: Vec<_> = report
            .cases
            .iter()
            .filter(|case| {
                selected_responses.contains(&format!(
                    "{}:{}:{}",
                    case.model_id, case.case_id, case.repetition
                ))
            })
            .collect();
        if cases.len() != selected_responses.len() {
            return Err(error("One or more selected responses no longer exist."));
        }
        serde_json::json!({"schemaVersion":"1.0","runId":report.run.id,"responses":cases})
    } else {
        serde_json::to_value(&report).map_err(error)?
    };
    benchmark::redact_paths(&mut value);
    let content = match kind.as_str() {
        "json" | "responses" => serde_json::to_string_pretty(&value).map_err(error)?,
        "csv" => benchmark::csv(&serde_json::from_value(value).map_err(error)?),
        _ => return Err(error("Choose JSON, CSV, or selected responses.")),
    };
    let path = std::path::Path::new(&path);
    let extension = if kind == "csv" { "csv" } else { "json" };
    if path.extension().and_then(|x| x.to_str()) != Some(extension) {
        return Err(error("The export filename must match the selected format."));
    }
    std::fs::write(path, content).map_err(error)
}

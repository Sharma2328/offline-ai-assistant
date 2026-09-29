//! Contract tests run against the in-process `FakeAdapter` (always available), plus
//! cancellation-latency (FR-CHAT-006) and load-failure isolation (NFR-REL-001) checks.

mod common;

use std::time::{Duration, Instant};

use inference::{FakeAdapter, FakeBehavior, InferenceAdapter, InferenceError, LoadRequest};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use common::{run_contract, sample_profile, user_request};

fn fake_load(model_id: &str) -> LoadRequest {
    LoadRequest {
        model_id: model_id.to_string(),
        model_path: std::path::PathBuf::from("/dev/null"),
        profile: sample_profile(model_id),
    }
}

#[tokio::test]
async fn fake_adapter_satisfies_the_contract() {
    let adapter = FakeAdapter::new();
    run_contract(&adapter, fake_load("m1")).await;
}

#[tokio::test]
async fn cancellation_is_acknowledged_well_under_500ms() {
    // A long reply with a 40 ms/token cadence; cancelling after the first token must return
    // promptly (FR-CHAT-006 budget is 500 ms).
    let adapter = FakeAdapter::new()
        .with_reply("one two three four five six seven eight nine ten eleven twelve")
        .with_token_delay(Duration::from_millis(40));
    adapter.load_model(fake_load("m1")).await.unwrap();

    let (tx, mut rx) = mpsc::unbounded_channel();
    let cancel = CancellationToken::new();
    let request = user_request("m1", "corr-cancel", "");
    // Use the canned reply, not the (empty) prompt.
    let request = inference::GenerationRequest {
        messages: Vec::new(),
        ..request
    };

    let cancel_for_task = cancel.clone();
    let handle = tokio::spawn(async move { adapter.generate(request, tx, cancel_for_task).await });

    // Wait for streaming to start, then cancel and time the acknowledgement.
    let _first = rx.recv().await.expect("expected a first token");
    let started = Instant::now();
    cancel.cancel();
    let result = handle
        .await
        .unwrap()
        .expect("generate returns a result on cancel");
    let elapsed = started.elapsed();

    assert_eq!(result.finish_reason, inference::FinishReason::Cancelled);
    assert!(
        elapsed < Duration::from_millis(500),
        "cancellation took {elapsed:?}, over the 500 ms budget"
    );
}

#[tokio::test]
async fn load_failure_is_reported_not_panicked() {
    // A runtime that OOMs on load must surface an error the host can map — never crash us.
    let adapter = FakeAdapter::with_behavior(FakeBehavior::OomOnLoad);
    let err = adapter.load_model(fake_load("m1")).await.unwrap_err();
    assert!(matches!(err, InferenceError::OutOfMemory(_)));
    // The adapter remains usable afterwards (no model loaded).
    assert!(adapter.loaded_model_id().await.is_none());

    let crashy = FakeAdapter::with_behavior(FakeBehavior::CrashOnLoad);
    let err = crashy.load_model(fake_load("m1")).await.unwrap_err();
    assert!(matches!(err, InferenceError::Crashed(_)));
}

#[tokio::test]
async fn generate_requires_a_loaded_model() {
    let adapter = FakeAdapter::new();
    let (tx, _rx) = mpsc::unbounded_channel();
    let err = adapter
        .generate(user_request("m1", "c", "hi"), tx, CancellationToken::new())
        .await
        .unwrap_err();
    assert!(matches!(err, InferenceError::NotLoaded(_)));
}

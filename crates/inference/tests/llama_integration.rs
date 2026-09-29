//! Explicit native integration tests: run with the pinned runtime and fixture model paths.

mod common;

use std::path::PathBuf;
use std::sync::Arc;

use inference::{resolve_llama_binary, LlamaAdapter, LoadRequest};

use common::{run_contract, sample_profile};

#[tokio::test]
#[ignore = "requires local GGUF fixtures and macOS runtime permissions"]
async fn llama_adapter_satisfies_the_contract_when_available() {
    let binary =
        resolve_llama_binary(None).expect("Provision llama-server before running native tests");
    let model = std::env::var_os("LLAMA_TEST_MODEL")
        .map(PathBuf::from)
        .expect("Set LLAMA_TEST_MODEL");
    assert!(model.is_file(), "LLAMA_TEST_MODEL must point to a GGUF");

    let adapter = LlamaAdapter::new(
        binary,
        Arc::new(|info| eprintln!("runtime crashed: {info:?}")),
    );
    let load = LoadRequest {
        model_id: "integration-model".to_string(),
        model_path: model,
        profile: sample_profile("integration-model"),
    };
    run_contract(&adapter, load).await;
}

#[tokio::test]
#[ignore = "requires local GGUF fixtures and macOS runtime permissions"]
async fn real_embedding_runtime_returns_valid_local_vectors_when_available() {
    use inference::{EmbeddingConfig, InferenceAdapter};
    let model = std::env::var_os("LLAMA_TEST_EMBEDDING_MODEL")
        .map(PathBuf::from)
        .expect("Set LLAMA_TEST_EMBEDDING_MODEL");
    let binary =
        resolve_llama_binary(None).expect("set up the runtime before real embedding tests");
    let adapter = LlamaAdapter::new(binary, Arc::new(|_| {})).for_embeddings();
    let mut profile = sample_profile("embedding-model");
    profile.context_length = 512;
    profile.batch_size = 512;
    adapter
        .load_model(LoadRequest {
            model_id: "embedding-model".into(),
            model_path: model,
            profile,
        })
        .await
        .unwrap();
    let vectors = adapter
        .embed(
            &[
                "The field station opens at 9 AM.".into(),
                "What time does the field station open?".into(),
            ],
            &EmbeddingConfig {
                model_id: "embedding-model".into(),
                normalize: true,
            },
        )
        .await
        .unwrap();
    assert_eq!(vectors.len(), 2);
    assert!(vectors
        .iter()
        .all(|vector| vector.len() == 384 && vector.iter().all(|v| v.is_finite())));
    adapter.unload_model("embedding-model").await.unwrap();
}

#[test]
#[ignore = "requires macOS Seatbelt execution"]
fn runtime_policy_denies_external_sockets() {
    let output = std::process::Command::new("/usr/bin/sandbox-exec")
        .args(["-p", inference::llama::LOOPBACK_POLICY, "python3", "-I", "-c", "import socket, errno; s=socket.socket(); s.settimeout(1); result=s.connect_ex(('1.1.1.1',443)); assert result in (errno.EPERM, errno.EACCES), result"])
        .output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

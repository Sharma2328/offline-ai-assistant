//! Native smoke test using the shipped runtime, real GGUFs, and a disposable database.
use inference::{EmbeddingConfig, InferenceAdapter, LlamaAdapter};
use std::{path::PathBuf, sync::Arc};
use storage::conversations::{ConversationsRepository, Message};
use tokio_util::sync::CancellationToken;

#[tokio::test]
#[ignore = "requires the pinned runtime and fixtures; scripts/verify-native.mjs provisions no downloads"]
async fn import_generate_persist_reopen_and_retrieve_real_document() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let scratch = tempfile::tempdir().unwrap();
    let db_path = scratch.path().join("smoke.sqlite");
    let mut db = storage::Database::open(&db_path).unwrap();
    db.migrate().unwrap();
    let hardware = app_core::detect_hardware(Some(scratch.path()));
    let capabilities = inference::detect_capabilities();
    let import = |name: &str| {
        app_core::import_model(
            &db,
            scratch.path(),
            &root.join("fixtures/models").join(name),
            app_core::StorageMode::Reference,
            &hardware,
            &capabilities,
        )
        .unwrap()
        .model
        .id
    };
    let model_id = import("stories15M-q4_0.gguf");
    let embedding_id = import("bge-small-en-v1.5-q8_0.gguf");
    let binary = inference::resolve_llama_binary(None)
        .expect("Run node scripts/setup-runtime.mjs --fixtures first");
    let adapter = LlamaAdapter::new(binary.clone(), Arc::new(|_| {}));
    let mut request = app_core::resolve_load_request(&db, &model_id).unwrap();
    request.profile.max_tokens = 16;
    adapter.load_model(request.clone()).await.unwrap();
    let repo = ConversationsRepository::new(db.connection());
    let conversation = repo
        .create("conversation", "Native smoke", Some(&model_id), "", None)
        .unwrap();
    repo.append(&Message {
        id: "user".into(),
        conversation_id: conversation.id.clone(),
        parent_id: None,
        role: "user".into(),
        content: "Once upon a time".into(),
        status: "complete".into(),
        metrics_json: None,
        citations_json: None,
        created_at: String::new(),
    })
    .unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let drain = tokio::spawn(async move {
        let mut text = String::new();
        while let Some(event) = rx.recv().await {
            let event: inference::TokenEvent = event;
            text.push_str(&event.token);
        }
        text
    });
    let generated = adapter
        .generate(
            inference::GenerationRequest {
                correlation_id: "native-smoke".into(),
                model_id: model_id.clone(),
                profile: request.profile,
                messages: vec![inference::ChatMessage {
                    role: inference::ChatRole::User,
                    content: "Once upon a time".into(),
                }],
                stop: vec![],
            },
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap();
    assert_eq!(drain.await.unwrap(), generated.text);
    assert!(!generated.text.trim().is_empty());
    repo.append(&Message {
        id: "assistant".into(),
        conversation_id: conversation.id,
        parent_id: Some("user".into()),
        role: "assistant".into(),
        content: generated.text.clone(),
        status: "complete".into(),
        metrics_json: Some(serde_json::to_string(&generated.timing).unwrap()),
        citations_json: None,
        created_at: String::new(),
    })
    .unwrap();
    adapter.unload_model(&model_id).await.unwrap();
    let embedding = LlamaAdapter::new(binary, Arc::new(|_| {})).for_embeddings();
    let mut request = app_core::resolve_load_request(&db, &embedding_id).unwrap();
    request.profile.context_length = 512;
    embedding.load_model(request).await.unwrap();
    let collection =
        documents::create_collection(db.connection(), "Station notes", &embedding_id, 256, 32, 4)
            .unwrap();
    let document = scratch.path().join("station.txt");
    std::fs::write(&document, "The field station opens at 9 AM on weekdays.").unwrap();
    let parsed = documents::parse(&document).unwrap();
    let document_id = documents::register(db.connection(), &collection, &parsed).unwrap();
    let config = EmbeddingConfig {
        model_id: embedding_id.clone(),
        normalize: true,
    };
    let vectors=embedding.embed(&[parsed.pages[0].clone(),"Represent this sentence for searching relevant passages: When does the field station open?".into()],&config).await.unwrap();
    documents::save_chunks(
        db.connection(),
        &document_id,
        &[(parsed.pages[0].clone(), 1, vectors[0].clone())],
    )
    .unwrap();
    let hits = documents::retrieve(db.connection(), &collection, &vectors[1], 4).unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].text.contains("9 AM"));
    assert_eq!(hits[0].page, Some(1));
    embedding.unload_model(&embedding_id).await.unwrap();
    drop(db);
    let mut reopened = storage::Database::open(&db_path).unwrap();
    reopened.migrate().unwrap();
    let saved = ConversationsRepository::new(reopened.connection())
        .messages("conversation")
        .unwrap();
    assert_eq!(saved.len(), 2);
    assert_eq!(saved[1].content, generated.text);
    assert_eq!(
        documents::list_documents(reopened.connection(), &collection).unwrap()[0].chunk_count,
        1
    );
}

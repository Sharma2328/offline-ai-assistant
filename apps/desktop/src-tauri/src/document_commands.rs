use crate::{lock_db, Db};
use app_core::{AppError, AppErrorCode, AppResult};
use documents::{Collection, Document, SourceChunk};
use inference::{EmbeddingConfig, InferenceAdapter, LlamaAdapter};
use serde::{Deserialize, Serialize};
use specta::Type;
use std::sync::{Arc, Mutex};
use tauri::{Manager, State};
use tauri_specta::Event;
use tokio_util::sync::CancellationToken;

#[derive(Default)]
pub struct DocumentState {
    pub(crate) operation: tokio::sync::Mutex<()>,
    adapter: tokio::sync::Mutex<Option<(String, Arc<LlamaAdapter>)>>,
    cancel: Mutex<Option<CancellationToken>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type, Event)]
#[serde(rename_all = "camelCase")]
pub struct DocumentProgress {
    pub collection_id: String,
    pub file_name: String,
    pub completed: u32,
    pub total: u32,
    pub phase: String,
}
fn doc_error(message: String) -> AppError {
    AppError::new(
        AppErrorCode::DocumentParseFailed,
        message,
        "Check the file and embedding model, then retry indexing.",
    )
}

#[tauri::command]
#[specta::specta]
pub fn collections_list(db: State<'_, Db>) -> AppResult<Vec<Collection>> {
    let db = lock_db(&db)?;
    documents::collections(db.connection()).map_err(doc_error)
}
#[tauri::command]
#[specta::specta]
pub fn collections_create(
    db: State<'_, Db>,
    name: String,
    embedding_model_id: String,
    chunk_size: u32,
    overlap: u32,
    top_k: u32,
) -> AppResult<String> {
    let db = lock_db(&db)?;
    documents::create_collection(
        db.connection(),
        &name,
        &embedding_model_id,
        chunk_size,
        overlap,
        top_k,
    )
    .map_err(doc_error)
}
#[tauri::command]
#[specta::specta]
pub fn documents_list(db: State<'_, Db>, collection_id: String) -> AppResult<Vec<Document>> {
    let db = lock_db(&db)?;
    documents::list_documents(db.connection(), &collection_id).map_err(doc_error)
}
#[tauri::command]
#[specta::specta]
pub fn collections_delete(
    db: State<'_, Db>,
    state: State<'_, DocumentState>,
    id: String,
) -> AppResult<()> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| doc_error("Cancel indexing before deleting a collection.".into()))?;
    let db = lock_db(&db)?;
    db.connection()
        .execute("DELETE FROM document_collections WHERE id=?1", [id])
        .map_err(|e| doc_error(e.to_string()))?;
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub fn documents_remove(
    db: State<'_, Db>,
    state: State<'_, DocumentState>,
    id: String,
) -> AppResult<()> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| doc_error("Cancel indexing before deleting a document.".into()))?;
    let db = lock_db(&db)?;
    db.connection()
        .execute("DELETE FROM documents WHERE id=?1", [id])
        .map_err(|e| doc_error(e.to_string()))?;
    Ok(())
}
#[tauri::command]
#[specta::specta]
pub fn documents_cancel(state: State<'_, DocumentState>) -> AppResult<()> {
    if let Some(cancel) = state
        .cancel
        .lock()
        .map_err(|_| AppError::internal("Indexing lock failed."))?
        .as_ref()
    {
        cancel.cancel();
    }
    Ok(())
}

async fn embedding_adapter(
    app: &tauri::AppHandle,
    collection: &Collection,
) -> AppResult<Arc<LlamaAdapter>> {
    let db = app.state::<Db>();
    let state = app.state::<DocumentState>();
    let configured =
        crate::runtime::read_string_setting(&db, crate::runtime::KEY_RUNTIME_BINARY_PATH)?;
    let binary = inference::resolve_llama_binary(configured.as_deref())
        .ok_or_else(|| doc_error("Set the llama-server path in Settings.".into()))?;
    let key = format!("{}:{}", collection.embedding_model_id, binary.display());
    let mut guard = state.adapter.lock().await;
    if let Some((existing, adapter)) = guard.as_ref() {
        if existing == &key && adapter.loaded_model_id().await.is_some() {
            return Ok(adapter.clone());
        }
        adapter
            .unload_model(&collection.embedding_model_id)
            .await
            .map_err(app_core::map_inference_error)?;
    }
    let mut request = {
        let db = lock_db(&db)?;
        app_core::resolve_load_request(&db, &collection.embedding_model_id)?
    };
    let source = request.model_path.clone();
    let metadata = tauri::async_runtime::spawn_blocking(move || inference::inspect_model(&source))
        .await
        .map_err(|e| doc_error(e.to_string()))?
        .map_err(|e| doc_error(e.to_string()))?;
    let expected = {
        let db = lock_db(&db)?;
        db.models()
            .get(&collection.embedding_model_id)
            .map_err(|e| doc_error(e.to_string()))?
            .ok_or_else(|| doc_error("Embedding model was removed.".into()))?
            .sha256
    };
    if metadata.sha256 != expected {
        return Err(doc_error(
            "Embedding model bytes changed. Re-import the model and create a new collection."
                .into(),
        ));
    }
    request.profile.context_length = 512;
    request.profile.batch_size = 512;
    let adapter = Arc::new(LlamaAdapter::new(binary, Arc::new(|_| {})).for_embeddings());
    adapter
        .load_model(request)
        .await
        .map_err(app_core::map_inference_error)?;
    *guard = Some((key, adapter.clone()));
    Ok(adapter)
}
fn get_collection(db: &State<'_, Db>, id: &str) -> AppResult<Collection> {
    let db = lock_db(db)?;
    documents::collections(db.connection())
        .map_err(doc_error)?
        .into_iter()
        .find(|c| c.id == id)
        .ok_or_else(|| doc_error("Collection no longer exists.".into()))
}

/// Find a UTF-8 boundary whose actual model token count is within the requested budget.
async fn prefix(adapter: &LlamaAdapter, text: &str, tokens: usize) -> AppResult<usize> {
    let boundaries: Vec<usize> = text
        .char_indices()
        .map(|(i, _)| i)
        .chain(std::iter::once(text.len()))
        .take(tokens * 8 + 1)
        .collect();
    if boundaries.len() < 2 {
        return Ok(text.len());
    }
    let mut low = 1;
    let mut high = boundaries.len() - 1;
    let mut best = boundaries[1];
    while low <= high {
        let mid = (low + high) / 2;
        let count = adapter
            .tokenize(&text[..boundaries[mid]])
            .await
            .map_err(app_core::map_inference_error)?
            .len();
        if count <= tokens {
            best = boundaries[mid];
            low = mid + 1;
        } else {
            if mid == 0 {
                break;
            }
            high = mid - 1;
        }
    }
    Ok(best)
}

#[tauri::command]
#[specta::specta]
pub async fn documents_ingest(
    app: tauri::AppHandle,
    db: State<'_, Db>,
    state: State<'_, DocumentState>,
    collection_id: String,
    paths: Vec<String>,
) -> AppResult<()> {
    let _operation = state
        .operation
        .try_lock()
        .map_err(|_| doc_error("An indexing or retrieval operation is already running.".into()))?;
    if paths.is_empty() || paths.len() > 100 {
        return Err(doc_error("Select between 1 and 100 documents.".into()));
    }
    let collection = get_collection(&db, &collection_id)?;
    let cancel = CancellationToken::new();
    *state
        .cancel
        .lock()
        .map_err(|_| AppError::internal("Indexing lock failed."))? = Some(cancel.clone());
    let result=async {
        let adapter=embedding_adapter(&app,&collection).await?;
        for path in paths {
            if cancel.is_cancelled() {break;}
            let parsed=crate::parser::parse(std::path::PathBuf::from(path),cancel.clone()).await?;
            let id={let db=lock_db(&db)?;documents::register(db.connection(),&collection_id,&parsed).map_err(doc_error)?};
            let indexed=async {
                let mut chunks=Vec::new(); let mut completed=0;
                for (page,text) in parsed.pages.iter().enumerate() {
                    let mut offset=0;
                    while offset<text.len() {
                        if cancel.is_cancelled() {return Err(doc_error("Indexing cancelled. Existing completed indexes are preserved.".into()));}
                        let length=prefix(&adapter,&text[offset..],collection.chunk_size as usize).await?;
                        let chunk=text[offset..offset+length].to_string();
                        let embedding_config=EmbeddingConfig {model_id:collection.embedding_model_id.clone(),normalize:true};
                        let vectors=tokio::select! { _=cancel.cancelled()=>return Err(doc_error("Indexing cancelled.".into())), vectors=adapter.embed(std::slice::from_ref(&chunk),&embedding_config)=>vectors.map_err(app_core::map_inference_error)? };
                        let vector=vectors.into_iter().next().ok_or_else(||doc_error("Embedding runtime returned no vector.".into()))?;
                        chunks.push((chunk,page as u32+1,vector)); completed+=1;
                        let _=DocumentProgress{collection_id:collection_id.clone(),file_name:parsed.name.clone(),completed,total:0,phase:"embedding".into()}.emit(&app);
                        if offset+length>=text.len() {break;}
                        let advance=if collection.overlap>0 {prefix(&adapter,&text[offset..offset+length],(collection.chunk_size-collection.overlap) as usize).await?} else {length};
                        offset+=advance.max(1);
                    }
                }
                let db=lock_db(&db)?; documents::save_chunks(db.connection(),&id,&chunks).map_err(doc_error)
            }.await;
            if let Err(error)=indexed {
                let db=lock_db(&db)?;
                db.connection().execute("UPDATE documents SET status=CASE WHEN EXISTS(SELECT 1 FROM document_chunks WHERE document_id=?1) THEN 'indexed' ELSE 'error' END,error=?2 WHERE id=?1",rusqlite::params![id,error.message]).map_err(|e|doc_error(e.to_string()))?;
                return Err(error);
            }
            let _=DocumentProgress{collection_id:collection_id.clone(),file_name:parsed.name,completed:1,total:1,phase:"complete".into()}.emit(&app);
        }
        Ok(())
    }.await;
    *state
        .cancel
        .lock()
        .map_err(|_| AppError::internal("Indexing lock failed."))? = None;
    result
}

pub async fn search(
    app: &tauri::AppHandle,
    collection_id: &str,
    query: &str,
) -> AppResult<Vec<SourceChunk>> {
    let db = app.state::<Db>();
    let state = app.state::<DocumentState>();
    let _operation = state.operation.lock().await;
    let collection = get_collection(&db, collection_id)?;
    let adapter = embedding_adapter(app, &collection).await?;
    let query = format!("Represent this sentence for searching relevant passages: {query}");
    let length = prefix(&adapter, &query, 384).await?;
    let vectors = adapter
        .embed(
            &[query[..length].to_string()],
            &EmbeddingConfig {
                model_id: collection.embedding_model_id,
                normalize: true,
            },
        )
        .await
        .map_err(app_core::map_inference_error)?;
    let vector = vectors
        .first()
        .ok_or_else(|| doc_error("Embedding runtime returned no vector.".into()))?;
    let db = lock_db(&db)?;
    documents::retrieve(db.connection(), collection_id, vector, collection.top_k).map_err(doc_error)
}
#[tauri::command]
#[specta::specta]
pub async fn documents_search(
    app: tauri::AppHandle,
    collection_id: String,
    query: String,
) -> AppResult<Vec<SourceChunk>> {
    search(&app, &collection_id, &query).await
}

pub async fn shutdown(state: &State<'_, DocumentState>) -> AppResult<()> {
    if let Some((_, adapter)) = state.adapter.lock().await.take() {
        if let Some(id) = adapter.loaded_model_id().await {
            adapter
                .unload_model(&id)
                .await
                .map_err(app_core::map_inference_error)?;
        }
    }
    Ok(())
}

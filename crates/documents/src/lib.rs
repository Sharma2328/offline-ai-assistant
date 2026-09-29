//! Local document parsing, collection storage, and cosine retrieval.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use specta::Type;
use std::{io::Read, path::Path};

pub type Result<T> = std::result::Result<T, String>;
const MAX_FILE: u64 = 32 * 1024 * 1024;
const MAX_TEXT: usize = 8 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Collection {
    pub id: String,
    pub name: String,
    pub embedding_model_id: String,
    pub chunk_size: u32,
    pub overlap: u32,
    pub top_k: u32,
    pub document_count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Document {
    pub id: String,
    pub collection_id: String,
    pub name: String,
    pub status: String,
    pub error: Option<String>,
    pub chunk_count: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceChunk {
    pub id: String,
    pub document_id: String,
    pub file_name: String,
    pub page: Option<u32>,
    pub text: String,
    pub score: f32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ParsedDocument {
    pub name: String,
    pub source: String,
    pub sha256: String,
    pub pages: Vec<String>,
}

/// Only reads selected files. DOCX relationships are never followed, and XML DTDs are rejected.
pub fn parse(path: &Path) -> Result<ParsedDocument> {
    let canonical = path.canonicalize().map_err(|e| e.to_string())?;
    let metadata = canonical.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() > MAX_FILE {
        return Err("Select a document smaller than 32 MiB.".into());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(&canonical)
        .map_err(|e| e.to_string())?
        .take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_FILE {
        return Err("Document exceeds the size limit.".into());
    }
    let extension = canonical
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_lowercase();
    let pages = match extension.as_str() {
        "txt" | "md" | "markdown" => vec![String::from_utf8(bytes.clone())
            .map_err(|_| "Save the document as UTF-8 text.".to_string())?],
        "pdf" => {
            if !bytes.starts_with(b"%PDF-") {
                return Err("The file is not a valid PDF.".into());
            }
            // The parser reads the PDF text layer only; no XML metadata or external files are resolved.
            pdf_extract::extract_text_from_mem_by_pages(&bytes)
                .map_err(|e| format!("PDF extraction failed: {e}"))?
        }
        "docx" => vec![parse_docx(&bytes)?],
        _ => return Err("Supported files: PDF, TXT, Markdown, DOCX.".into()),
    };
    let text_size: usize = pages.iter().map(String::len).sum();
    if text_size > MAX_TEXT {
        return Err("Extracted text exceeds 8 MiB. Split the document into smaller files.".into());
    }
    if pages.iter().all(|page| page.trim().is_empty()) {
        return Err("No text was found. Scanned PDFs require OCR before import.".into());
    }
    Ok(ParsedDocument {
        name: canonical
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string(),
        source: canonical.to_string_lossy().to_string(),
        sha256: format!("{:x}", Sha256::digest(&bytes)),
        pages,
    })
}

fn parse_docx(bytes: &[u8]) -> Result<String> {
    let mut archive =
        zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let entry = archive
        .by_name("word/document.xml")
        .map_err(|_| "DOCX has no document body.".to_string())?;
    if entry.size() > MAX_TEXT as u64 {
        return Err("DOCX body exceeds the size limit.".into());
    }
    let mut xml = String::new();
    entry
        .take(MAX_TEXT as u64 + 1)
        .read_to_string(&mut xml)
        .map_err(|e| e.to_string())?;
    if xml.len() > MAX_TEXT {
        return Err("DOCX body exceeds the size limit.".into());
    }
    parse_xml_text(&xml)
}

fn parse_xml_text(xml: &str) -> Result<String> {
    use quick_xml::events::Event;
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut result = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::DocType(_) => {
                return Err(
                    "Document type declarations and external entities are not allowed.".into(),
                )
            }
            Event::Start(element) => {
                if element.local_name().as_ref() == b"t" {
                    in_text = true;
                }
            }
            Event::End(element) => {
                if element.local_name().as_ref() == b"t" {
                    in_text = false;
                }
                if element.local_name().as_ref() == b"p" {
                    result.push('\n');
                }
            }
            Event::Text(text) if in_text => {
                result.push_str(&text.unescape().map_err(|e| e.to_string())?)
            }
            Event::Empty(element) if element.local_name().as_ref() == b"tab" => result.push('\t'),
            Event::Eof => break,
            _ => {}
        }
    }
    Ok(result)
}

pub fn collections(conn: &Connection) -> Result<Vec<Collection>> {
    let mut statement=conn.prepare("SELECT c.id,c.name,c.embedding_model_id,c.chunk_size,c.overlap,c.top_k,(SELECT COUNT(*) FROM documents d WHERE d.collection_id=c.id) FROM document_collections c ORDER BY c.created_at DESC").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map([], |r| {
            Ok(Collection {
                id: r.get(0)?,
                name: r.get(1)?,
                embedding_model_id: r.get(2)?,
                chunk_size: r.get(3)?,
                overlap: r.get(4)?,
                top_k: r.get(5)?,
                document_count: r.get(6)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn create_collection(
    conn: &Connection,
    name: &str,
    model: &str,
    chunk_size: u32,
    overlap: u32,
    top_k: u32,
) -> Result<String> {
    if name.trim().is_empty()
        || name.len() > 200
        || !(32..=384).contains(&chunk_size)
        || overlap >= chunk_size
        || !(1..=12).contains(&top_k)
    {
        return Err(
            "Choose a name, 32–384 tokens per chunk, smaller overlap, and 1–12 results.".into(),
        );
    }
    let id = uuid::Uuid::new_v4().to_string();
    conn.execute("INSERT INTO document_collections(id,name,embedding_model_id,chunk_size,overlap,top_k) VALUES(?1,?2,?3,?4,?5,?6)",params![id,name.trim(),model,chunk_size,overlap,top_k]).map_err(|e|e.to_string())?;
    Ok(id)
}
pub fn list_documents(conn: &Connection, collection: &str) -> Result<Vec<Document>> {
    let mut statement=conn.prepare("SELECT d.id,d.collection_id,d.name,d.status,d.error,(SELECT COUNT(*) FROM document_chunks c WHERE c.document_id=d.id) FROM documents d WHERE d.collection_id=?1 ORDER BY d.name").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map([collection], |r| {
            Ok(Document {
                id: r.get(0)?,
                collection_id: r.get(1)?,
                name: r.get(2)?,
                status: r.get(3)?,
                error: r.get(4)?,
                chunk_count: r.get(5)?,
            })
        })
        .map_err(|e| e.to_string())?;
    rows.collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())
}
pub fn register(conn: &Connection, collection: &str, parsed: &ParsedDocument) -> Result<String> {
    let existing: Option<String> = conn
        .query_row(
            "SELECT id FROM documents WHERE collection_id=?1 AND sha256=?2",
            params![collection, parsed.sha256],
            |r| r.get(0),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let id = existing.unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    // Reindex atomically per document: old vectors remain usable until replacement commits.
    conn.execute("INSERT INTO documents(id,collection_id,name,source_uri,sha256,status) VALUES(?1,?2,?3,?4,?5,'pending') ON CONFLICT(id) DO UPDATE SET source_uri=excluded.source_uri,error=NULL",params![id,collection,parsed.name,parsed.source,parsed.sha256]).map_err(|e|e.to_string())?;
    Ok(id)
}
pub fn save_chunks(conn: &Connection, id: &str, chunks: &[(String, u32, Vec<f32>)]) -> Result<()> {
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    tx.execute("DELETE FROM document_chunks WHERE document_id=?1", [id])
        .map_err(|e| e.to_string())?;
    for (ordinal, (text, page, vector)) in chunks.iter().enumerate() {
        validate_vector(vector)?;
        let json = serde_json::to_string(vector).map_err(|e| e.to_string())?;
        tx.execute("INSERT INTO document_chunks(id,document_id,ordinal,page,text,vector_json) VALUES(?1,?2,?3,?4,?5,?6)",params![uuid::Uuid::new_v4().to_string(),id,ordinal,page,text,json]).map_err(|e|e.to_string())?;
    }
    tx.execute(
        "UPDATE documents SET status='indexed',error=NULL WHERE id=?1",
        [id],
    )
    .map_err(|e| e.to_string())?;
    tx.commit().map_err(|e| e.to_string())
}
pub fn validate_vector(vector: &[f32]) -> Result<()> {
    if vector.is_empty()
        || vector.iter().any(|v| !v.is_finite())
        || vector.iter().all(|v| *v == 0.0)
    {
        return Err("The embedding runtime returned an invalid vector.".into());
    }
    Ok(())
}
pub fn cosine(a: &[f32], b: &[f32]) -> Result<f32> {
    validate_vector(a)?;
    validate_vector(b)?;
    if a.len() != b.len() {
        return Err("Embedding dimensions changed. Reindex the collection.".into());
    }
    let dot: f64 = a
        .iter()
        .zip(b)
        .map(|(a, b)| f64::from(*a) * f64::from(*b))
        .sum();
    let norm = |v: &[f32]| v.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>().sqrt();
    Ok((dot / (norm(a) * norm(b))) as f32)
}
pub fn retrieve(
    conn: &Connection,
    collection: &str,
    query: &[f32],
    top_k: u32,
) -> Result<Vec<SourceChunk>> {
    let mut statement=conn.prepare("SELECT c.id,c.document_id,d.name,c.page,c.text,c.vector_json FROM document_chunks c JOIN documents d ON d.id=c.document_id WHERE d.collection_id=?1 AND d.status='indexed'").map_err(|e|e.to_string())?;
    let rows = statement
        .query_map([collection], |r| {
            Ok((
                SourceChunk {
                    id: r.get(0)?,
                    document_id: r.get(1)?,
                    file_name: r.get(2)?,
                    page: r.get(3)?,
                    text: r.get(4)?,
                    score: 0.0,
                },
                r.get::<_, String>(5)?,
            ))
        })
        .map_err(|e| e.to_string())?;
    let mut best = Vec::new();
    for row in rows {
        let (mut chunk, json) = row.map_err(|e| e.to_string())?;
        let vector: Vec<f32> = serde_json::from_str(&json).map_err(|e| e.to_string())?;
        chunk.score = cosine(query, &vector)?;
        if chunk.score > 0.2 {
            best.push(chunk);
            best.sort_by(|a, b| b.score.total_cmp(&a.score));
            best.truncate(top_k.min(12) as usize);
        }
    }
    Ok(best)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xml_rejects_external_entities_and_preserves_text() {
        assert!(parse_xml_text(
            "<!DOCTYPE x [<!ENTITY leak SYSTEM 'file:///etc/passwd'>]><w:t>&leak;</w:t>"
        )
        .is_err());
        assert_eq!(
            parse_xml_text("<w:p><w:t>A &amp; B</w:t></w:p>").unwrap(),
            "A & B\n"
        );
    }
    #[test]
    fn cosine_rejects_corrupt_vectors() {
        assert_eq!(cosine(&[1.0, 0.0], &[1.0, 0.0]).unwrap(), 1.0);
        assert!(cosine(&[0.0], &[1.0]).is_err());
        assert!(cosine(&[f32::NAN], &[1.0]).is_err());
        assert!(cosine(&[1.0], &[1.0, 0.0]).is_err());
    }
    #[test]
    fn indexing_is_atomic_and_collection_deletion_removes_vectors() {
        let mut db = storage::Database::open_in_memory().unwrap();
        db.migrate().unwrap();
        let conn = db.connection();
        conn.execute("INSERT INTO models(id,name,file_uri,storage_mode,sha256,size_bytes,format,metadata_json,created_at) VALUES('embedding','Embedding','/fixture.gguf','reference','checksum',1,'gguf','{}','now')",[]).unwrap();
        let collection = create_collection(conn, "Research", "embedding", 256, 32, 4).unwrap();
        let parsed = ParsedDocument {
            name: "notes.txt".into(),
            source: "/notes.txt".into(),
            sha256: "document".into(),
            pages: vec!["Station opens at nine.".into()],
        };
        let id = register(conn, &collection, &parsed).unwrap();
        assert_eq!(register(conn, &collection, &parsed).unwrap(), id);
        save_chunks(conn, &id, &[(parsed.pages[0].clone(), 1, vec![1.0, 0.0])]).unwrap();
        assert!(save_chunks(conn, &id, &[("corrupt".into(), 1, vec![f32::NAN, 0.0])]).is_err());
        let hits = retrieve(conn, &collection, &[1.0, 0.0], 4).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].text, parsed.pages[0]);
        assert!(conn
            .execute("DELETE FROM models WHERE id='embedding'", [])
            .is_err());
        conn.execute("DELETE FROM document_collections WHERE id=?1", [collection])
            .unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM document_chunks", [], |r| r
                .get::<_, u32>(0))
                .unwrap(),
            0
        );
    }

    #[test]
    fn text_extraction_and_limits() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("notes.md");
        std::fs::write(&file, "# Notes\nLocal data").unwrap();
        let parsed = parse(&file).unwrap();
        assert_eq!(parsed.pages[0], "# Notes\nLocal data");
        assert_eq!(parsed.sha256.len(), 64);
    }
}

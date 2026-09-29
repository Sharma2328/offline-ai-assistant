CREATE TABLE document_collections (
 id TEXT PRIMARY KEY, name TEXT NOT NULL,
 embedding_model_id TEXT NOT NULL REFERENCES models(id) ON DELETE RESTRICT,
 chunk_size INTEGER NOT NULL, overlap INTEGER NOT NULL, top_k INTEGER NOT NULL,
 created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);
CREATE TABLE documents (
 id TEXT PRIMARY KEY, collection_id TEXT NOT NULL REFERENCES document_collections(id) ON DELETE CASCADE,
 name TEXT NOT NULL, source_uri TEXT NOT NULL, sha256 TEXT NOT NULL,
 status TEXT NOT NULL CHECK(status IN ('pending','indexed','error')),
 error TEXT, UNIQUE(collection_id,sha256)
);
CREATE TABLE document_chunks (
 id TEXT PRIMARY KEY, document_id TEXT NOT NULL REFERENCES documents(id) ON DELETE CASCADE,
 ordinal INTEGER NOT NULL, page INTEGER, text TEXT NOT NULL, vector_json TEXT NOT NULL,
 UNIQUE(document_id,ordinal)
);
CREATE INDEX chunks_document ON document_chunks(document_id);
CREATE TRIGGER collection_deleted AFTER DELETE ON document_collections BEGIN
 UPDATE conversations SET collection_id=NULL WHERE collection_id=old.id;
END;

CREATE TABLE conversations (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    model_id TEXT REFERENCES models(id) ON DELETE SET NULL,
    system_prompt TEXT NOT NULL DEFAULT '',
    collection_id TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);
CREATE TABLE messages (
    id TEXT PRIMARY KEY,
    conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
    parent_id TEXT REFERENCES messages(id) ON DELETE CASCADE,
    role TEXT NOT NULL CHECK (role IN ('user', 'assistant', 'system')),
    content TEXT NOT NULL,
    status TEXT NOT NULL CHECK (status IN ('complete', 'streaming', 'stopped', 'error')),
    metrics_json TEXT,
    created_at TEXT NOT NULL
);
CREATE INDEX messages_conversation ON messages(conversation_id, created_at);
CREATE INDEX conversations_updated ON conversations(updated_at DESC);
CREATE VIRTUAL TABLE conversation_search USING fts5(conversation_id UNINDEXED, content);
CREATE TRIGGER conversations_search_insert AFTER INSERT ON conversations BEGIN
    INSERT INTO conversation_search(conversation_id, content) VALUES(new.id, new.title);
END;
CREATE TRIGGER conversations_search_update AFTER UPDATE OF title ON conversations BEGIN
    DELETE FROM conversation_search WHERE conversation_id = new.id;
    INSERT INTO conversation_search(conversation_id, content) VALUES(new.id, new.title);
    INSERT INTO conversation_search(conversation_id, content)
      SELECT conversation_id, content FROM messages WHERE conversation_id = new.id;
END;
CREATE TRIGGER conversations_search_delete AFTER DELETE ON conversations BEGIN
    DELETE FROM conversation_search WHERE conversation_id = old.id;
END;
CREATE TRIGGER messages_search_insert AFTER INSERT ON messages BEGIN
    INSERT INTO conversation_search(conversation_id, content) VALUES(new.conversation_id, new.content);
END;
CREATE TRIGGER messages_search_update AFTER UPDATE OF content ON messages BEGIN
    UPDATE conversations SET title = title WHERE id = new.conversation_id;
END;
CREATE TRIGGER messages_search_delete AFTER DELETE ON messages BEGIN
    UPDATE conversations SET title = title WHERE id = old.conversation_id;
END;

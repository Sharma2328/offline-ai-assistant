//! Persistent conversation trees. Regeneration and edits add branches without losing history.
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::{migrations::now_iso8601, StorageError};

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub model_id: Option<String>,
    pub system_prompt: String,
    pub collection_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub citations_json: Option<String>,
    pub id: String,
    pub conversation_id: String,
    pub parent_id: Option<String>,
    pub role: String,
    pub content: String,
    pub status: String,
    pub metrics_json: Option<String>,
    pub created_at: String,
}

pub struct ConversationsRepository<'a> {
    conn: &'a Connection,
}

impl<'a> ConversationsRepository<'a> {
    pub fn new(conn: &'a Connection) -> Self {
        Self { conn }
    }

    pub fn list(&self, search: &str) -> Result<Vec<Conversation>, StorageError> {
        let query = search
            .split_whitespace()
            .map(|word| format!("\"{}\"*", word.replace('"', "\"\"")))
            .collect::<Vec<_>>()
            .join(" AND ");
        let mut stmt = self.conn.prepare(
            "SELECT id, title, model_id, system_prompt, collection_id, created_at, updated_at
             FROM conversations WHERE ?1 = '' OR id IN
             (SELECT conversation_id FROM conversation_search WHERE conversation_search MATCH ?2)
             ORDER BY updated_at DESC LIMIT 1000",
        )?;
        let rows = stmt.query_map(
            params![
                search.trim(),
                if query.is_empty() { "\"\"" } else { &query }
            ],
            conversation_row,
        )?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn get(&self, id: &str) -> Result<Option<Conversation>, StorageError> {
        Ok(self.conn.query_row("SELECT id, title, model_id, system_prompt, collection_id, created_at, updated_at FROM conversations WHERE id=?1", [id], conversation_row).optional()?)
    }

    pub fn create(
        &self,
        id: &str,
        title: &str,
        model: Option<&str>,
        system_prompt: &str,
        collection: Option<&str>,
    ) -> Result<Conversation, StorageError> {
        let now = now_iso8601();
        self.conn.execute("INSERT INTO conversations(id,title,model_id,system_prompt,collection_id,created_at,updated_at) VALUES(?1,?2,?3,?4,?5,?6,?6)", params![id,title,model,system_prompt,collection,now])?;
        self.get(id)?
            .ok_or_else(|| StorageError::Migration("conversation was not created".into()))
    }

    pub fn rename(&self, id: &str, title: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE conversations SET title=?2, updated_at=?3 WHERE id=?1",
            params![id, title, now_iso8601()],
        )?;
        Ok(())
    }

    pub fn configure(
        &self,
        id: &str,
        model: &str,
        system_prompt: &str,
        collection: Option<&str>,
    ) -> Result<(), StorageError> {
        self.conn.execute("UPDATE conversations SET model_id=?2,system_prompt=?3,collection_id=?4,updated_at=?5 WHERE id=?1", params![id,model,system_prompt,collection,now_iso8601()])?;
        Ok(())
    }

    pub fn delete(&self, id: &str) -> Result<(), StorageError> {
        self.conn
            .execute("DELETE FROM conversations WHERE id=?1", [id])?;
        Ok(())
    }

    pub fn messages(&self, conversation_id: &str) -> Result<Vec<Message>, StorageError> {
        let mut stmt = self.conn.prepare("SELECT id,conversation_id,parent_id,role,content,status,metrics_json,created_at,citations_json FROM messages WHERE conversation_id=?1 ORDER BY created_at,id")?;
        let rows = stmt.query_map([conversation_id], |row| {
            Ok(Message {
                citations_json: row.get(8)?,
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                parent_id: row.get(2)?,
                role: row.get(3)?,
                content: row.get(4)?,
                status: row.get(5)?,
                metrics_json: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    pub fn append(&self, message: &Message) -> Result<(), StorageError> {
        let tx = self.conn.unchecked_transaction()?;
        if let Some(parent) = &message.parent_id {
            let valid: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM messages WHERE id=?1 AND conversation_id=?2)",
                params![parent, message.conversation_id],
                |row| row.get(0),
            )?;
            if !valid {
                return Err(StorageError::Migration(
                    "message parent belongs to a different conversation or was removed".into(),
                ));
            }
        }
        tx.execute("INSERT INTO messages(id,conversation_id,parent_id,role,content,status,metrics_json,created_at,citations_json) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9)", params![message.id,message.conversation_id,message.parent_id,message.role,message.content,message.status,message.metrics_json,now_iso8601(),message.citations_json])?;
        tx.execute(
            "UPDATE conversations SET updated_at=?2 WHERE id=?1",
            params![message.conversation_id, now_iso8601()],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn finish(
        &self,
        id: &str,
        content: &str,
        status: &str,
        metrics: Option<&str>,
    ) -> Result<(), StorageError> {
        let tx = self.conn.unchecked_transaction()?;
        tx.execute(
            "UPDATE messages SET content=?2,status=?3,metrics_json=?4 WHERE id=?1",
            params![id, content, status, metrics],
        )?;
        tx.execute("UPDATE conversations SET updated_at=?2 WHERE id=(SELECT conversation_id FROM messages WHERE id=?1)", params![id,now_iso8601()])?;
        tx.commit()?;
        Ok(())
    }

    pub fn delete_message(&self, conversation_id: &str, id: &str) -> Result<(), StorageError> {
        self.conn.execute(
            "DELETE FROM messages WHERE id=?1 AND conversation_id=?2",
            params![id, conversation_id],
        )?;
        Ok(())
    }

    pub fn recover_interrupted(&self) -> Result<(), StorageError> {
        self.conn.execute(
            "UPDATE messages SET status='stopped' WHERE status='streaming'",
            [],
        )?;
        Ok(())
    }
}

fn conversation_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<Conversation> {
    Ok(Conversation {
        id: row.get(0)?,
        title: row.get(1)?,
        model_id: row.get(2)?,
        system_prompt: row.get(3)?,
        collection_id: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Database;

    #[test]
    fn search_one_thousand_conversations_within_budget() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();
        let repo = ConversationsRepository::new(db.connection());
        for index in 0..1000 {
            repo.create(
                &format!("conversation-{index}"),
                &format!("Research topic {index}"),
                None,
                "",
                None,
            )
            .unwrap();
        }
        let start = std::time::Instant::now();
        let hits = repo.list("topic 999").unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id, "conversation-999");
        assert!(start.elapsed() < std::time::Duration::from_millis(300));
        assert!(repo.list("\" AND OR ( *").is_ok());
    }

    #[test]
    fn branches_search_recovery_and_cascading_delete() {
        let mut db = Database::open_in_memory().unwrap();
        db.migrate().unwrap();
        let repo = ConversationsRepository::new(db.connection());
        repo.create("chat", "Local notes", None, "", None).unwrap();
        repo.create("other", "Other", None, "", None).unwrap();
        let mut msg = Message {
            citations_json: None,
            id: "user".into(),
            conversation_id: "chat".into(),
            parent_id: None,
            role: "user".into(),
            content: "quartz question".into(),
            status: "complete".into(),
            metrics_json: None,
            created_at: String::new(),
        };
        repo.append(&msg).unwrap();
        assert_eq!(repo.list("quartz").unwrap().len(), 1);
        msg.id = "answer".into();
        msg.parent_id = Some("user".into());
        msg.role = "assistant".into();
        msg.status = "streaming".into();
        repo.append(&msg).unwrap();
        msg.id = "alternative".into();
        repo.append(&msg).unwrap();
        msg.id = "invalid".into();
        msg.conversation_id = "other".into();
        assert!(repo.append(&msg).is_err());
        repo.recover_interrupted().unwrap();
        assert_eq!(
            repo.messages("chat")
                .unwrap()
                .iter()
                .filter(|m| m.status == "stopped")
                .count(),
            2
        );
        repo.delete_message("chat", "answer").unwrap();
        assert_eq!(repo.messages("chat").unwrap().len(), 2);
        repo.delete("chat").unwrap();
        assert!(repo.messages("chat").unwrap().is_empty());
        assert!(repo.list("quartz").unwrap().is_empty());
    }
}

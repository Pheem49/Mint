use super::{MemoryError, MemoryStore};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SourceAttachment {
    pub name: String,
    pub kind: String,
    pub mime_type: String,
    pub thumbnail_data_uri: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionAttachment {
    pub id: i64,
    pub interaction_id: Option<i64>,
    pub library_item_id: Option<String>,
    pub availability: Option<String>,
    pub trashed: bool,
    #[serde(flatten)]
    pub attachment: SourceAttachment,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSources {
    pub attachments: Vec<SessionAttachment>,
    pub web_search_progress: Vec<serde_json::Value>,
}

impl MemoryStore {
    /// Store only names and bounded thumbnails; original attachment bytes never enter history.
    pub fn save_source_attachments(
        &self,
        chat_id: &str,
        interaction_id: i64,
        attachments: &[SourceAttachment],
    ) -> Result<(), String> {
        use base64::Engine;
        if attachments.len() > 64 {
            return Err("Too many source attachments".into());
        }
        for attachment in attachments {
            if attachment.name.trim().is_empty()
                || attachment.name.len() > 1024
                || attachment.mime_type.len() > 256
                || !matches!(attachment.kind.as_str(), "image" | "video" | "file")
            {
                return Err("Invalid source attachment metadata".into());
            }
            if let Some(thumbnail) = &attachment.thumbnail_data_uri {
                let valid = attachment.kind == "image"
                    && thumbnail.len() <= 262_144
                    && [
                        "data:image/png;base64,",
                        "data:image/jpeg;base64,",
                        "data:image/webp;base64,",
                    ]
                    .iter()
                    .any(|prefix| thumbnail.starts_with(prefix))
                    && thumbnail.split_once(',').is_some_and(|(_, data)| {
                        base64::engine::general_purpose::STANDARD
                            .decode(data)
                            .is_ok()
                    });
                if !valid {
                    return Err("Invalid source thumbnail".into());
                }
            }
        }
        let mut connection = self.connection().map_err(|e| e.to_string())?;
        let transaction = connection.transaction().map_err(|e| e.to_string())?;
        let owns_turn: bool = transaction
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM interaction_memories WHERE id = ?1 AND chat_id = ?2)",
                rusqlite::params![interaction_id, super::normalized_chat_id(chat_id)],
                |row| row.get(0),
            )
            .map_err(|e| e.to_string())?;
        if !owns_turn {
            return Err("Interaction does not belong to this conversation".into());
        }
        for attachment in attachments {
            let metadata = serde_json::to_string(attachment).map_err(|e| e.to_string())?;
            transaction.execute("INSERT OR IGNORE INTO conversation_source_attachments (interaction_id, metadata) VALUES (?1, ?2)", rusqlite::params![interaction_id, metadata]).map_err(|e| e.to_string())?;
        }
        transaction.commit().map_err(|e| e.to_string())
    }

    pub fn conversation_sources(&self, chat_id: &str) -> Result<ConversationSources, MemoryError> {
        let connection = self.connection()?;
        let chat_id = super::normalized_chat_id(chat_id);
        let mut result = ConversationSources::default();
        let mut statement = connection.prepare("SELECT s.id, s.interaction_id, s.metadata FROM conversation_source_attachments s JOIN interaction_memories m ON m.id = s.interaction_id WHERE m.chat_id = ?1 ORDER BY s.id")?;
        let rows = statement.query_map([&chat_id], |row| {
            Ok((
                row.get::<_, i64>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;
        for row in rows {
            let (id, interaction_id, metadata) = row?;
            if let Ok(attachment) = serde_json::from_str(&metadata) {
                result.attachments.push(SessionAttachment {
                    id,
                    interaction_id: Some(interaction_id),
                    library_item_id: None,
                    availability: None,
                    trashed: false,
                    attachment,
                });
            }
        }
        let library = crate::library::LibraryStore::for_memory(self.clone())
            .map_err(|e| MemoryError::Library(e.to_string()))?;
        let indexed = library
            .session_attachments(&chat_id)
            .map_err(|e| MemoryError::Library(e.to_string()))?;
        result.attachments.retain(|legacy| {
            !indexed.iter().any(|item| {
                item.interaction_id == legacy.interaction_id
                    && item.attachment.name == legacy.attachment.name
                    && item.attachment.kind == legacy.attachment.kind
            })
        });
        result.attachments.extend(indexed);
        // Query every turn, independently of the chat UI's 50-message history window.
        let mut statement = connection.prepare("SELECT agent_activity_json FROM interaction_memories WHERE chat_id = ?1 AND agent_activity_json IS NOT NULL ORDER BY id")?;
        for row in statement.query_map([&chat_id], |row| row.get::<_, String>(0))? {
            if let Ok(events) = serde_json::from_str::<Vec<serde_json::Value>>(&row?) {
                result
                    .web_search_progress
                    .extend(events.into_iter().filter(|event| {
                        event["type"] == "ToolEnd" && event["data"]["action"] == "web_search"
                    }));
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_sources_survive_reopen_are_scoped_and_follow_history_deletion() {
        let dir =
            std::env::temp_dir().join(format!("mint-session-sources-{}", uuid::Uuid::new_v4()));
        let path = dir.as_path().join("memory.db");
        let store = MemoryStore::open(&path);
        let first = store
            .add_interaction_for_chat("A", "image", "reply", "test", "test")
            .unwrap();
        let other = store
            .add_interaction_for_chat("B", "document", "reply", "test", "test")
            .unwrap();
        let attachment = SourceAttachment {
            name: "photo.png".into(),
            kind: "image".into(),
            mime_type: "image/png".into(),
            thumbnail_data_uri: Some("data:image/png;base64,aGVsbG8=".into()),
        };
        store
            .save_source_attachments("A", first, std::slice::from_ref(&attachment))
            .unwrap();
        store
            .save_source_attachments("A", first, &[attachment])
            .unwrap();
        assert!(store.save_source_attachments("B", first, &[]).is_err());
        store.set_interaction_agent_activity_json(first, r#"[{"type":"ToolEnd","data":{"action":"web_search","result":"1. Docs\nURL: https://example.com/docs"}},{"type":"ToolEnd","data":{"action":"read_file","result":"private"}}]"#).unwrap();
        let reopened = MemoryStore::open(path);
        let a = reopened.conversation_sources("A").unwrap();
        assert_eq!(a.attachments.len(), 1);
        assert_eq!(a.attachments[0].interaction_id, Some(first));
        assert_eq!(a.attachments[0].attachment.name, "photo.png");
        assert_eq!(a.web_search_progress.len(), 1);
        assert_eq!(a.web_search_progress[0]["data"]["action"], "web_search");
        assert!(
            reopened
                .conversation_sources("B")
                .unwrap()
                .attachments
                .is_empty()
        );
        assert!(reopened.save_source_attachments("A", other, &[]).is_err());
        reopened.clear_interactions_for_chat("A").unwrap();
        assert!(
            reopened
                .conversation_sources("A")
                .unwrap()
                .attachments
                .is_empty()
        );
        assert!(
            reopened
                .conversation_sources("A")
                .unwrap()
                .web_search_progress
                .is_empty()
        );
    }

    #[test]
    fn attachment_metadata_rejects_external_thumbnails_and_oversized_values() {
        let dir =
            std::env::temp_dir().join(format!("mint-session-sources-{}", uuid::Uuid::new_v4()));
        let store = MemoryStore::open(dir.as_path().join("memory.db"));
        let id = store
            .add_interaction_for_chat("A", "hello", "reply", "test", "test")
            .unwrap();
        let mut attachment = SourceAttachment {
            name: "photo.png".into(),
            kind: "image".into(),
            mime_type: "image/png".into(),
            thumbnail_data_uri: Some("https://external.example/image.png".into()),
        };
        assert!(
            store
                .save_source_attachments("A", id, std::slice::from_ref(&attachment))
                .is_err()
        );
        attachment.thumbnail_data_uri =
            Some(format!("data:image/png;base64,{}", "A".repeat(300_000)));
        assert!(
            store
                .save_source_attachments("A", id, &[attachment])
                .is_err()
        );
        assert!(
            store
                .conversation_sources("A")
                .unwrap()
                .attachments
                .is_empty()
        );
    }
}

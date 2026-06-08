use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CHAT_CLI_ID: &str = "cli";
pub const DEFAULT_CONVERSATION_ID: &str = "conversation-default";

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("unable to determine the user config directory")]
    ConfigDirectoryUnavailable,
    #[error("unable to create database directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct MemoryStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InteractionMemory {
    pub id: i64,
    pub chat_id: String,
    pub user_text: String,
    pub ai_text: String,
    pub provider: String,
    pub model: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearnedSkill {
    pub id: i64,
    pub name: String,
    pub source_path: String,
    pub content: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSession {
    pub workspace_path: String,
    pub summary: String,
    pub verification: String,
    pub updated_at: String,
}

impl MemoryStore {
    pub fn open_default() -> Result<Self, MemoryError> {
        Ok(Self::open(memory_path()?))
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn set_profile(&self, key: &str, value: &str) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO user_profile (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET
               value = excluded.value,
               updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_profile(&self, key: &str) -> Result<Option<String>, MemoryError> {
        let connection = self.connection()?;
        Ok(connection
            .query_row(
                "SELECT value FROM user_profile WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn add_interaction(&self, user_text: &str, ai_text: &str) -> Result<i64, MemoryError> {
        self.add_interaction_with_metadata(user_text, ai_text, "", "")
    }

    pub fn add_interaction_with_metadata(
        &self,
        user_text: &str,
        ai_text: &str,
        provider: &str,
        model: &str,
    ) -> Result<i64, MemoryError> {
        self.add_interaction_for_chat(DEFAULT_CONVERSATION_ID, user_text, ai_text, provider, model)
    }

    pub fn add_interaction_for_chat(
        &self,
        chat_id: &str,
        user_text: &str,
        ai_text: &str,
        provider: &str,
        model: &str,
    ) -> Result<i64, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        ensure_builtin_chat_sessions(&connection)?;
        ensure_chat_session_row(&connection, &chat_id)?;
        connection.execute(
            "INSERT INTO interaction_memories (chat_id, user_text, ai_text, provider, model)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![chat_id, user_text, ai_text, provider, model],
        )?;
        connection.execute(
            "UPDATE chat_sessions
             SET title = CASE
               WHEN title = 'New chat' AND ?2 != '' THEN substr(?2, 1, 80)
               ELSE title
             END,
             updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![chat_id, user_text.trim()],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn recent_interactions(&self, limit: usize) -> Result<Vec<InteractionMemory>, MemoryError> {
        self.recent_interactions_for_chat(DEFAULT_CONVERSATION_ID, limit)
    }

    pub fn recent_interactions_for_chat(
        &self,
        chat_id: &str,
        limit: usize,
    ) -> Result<Vec<InteractionMemory>, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        ensure_builtin_chat_sessions(&connection)?;
        ensure_chat_session_row(&connection, &chat_id)?;
        let mut statement = connection.prepare(
            "SELECT id, chat_id, user_text, ai_text, provider, model, created_at
             FROM interaction_memories
             WHERE chat_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![chat_id, limit as i64], interaction_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_chat_sessions(&self) -> Result<Vec<ChatSession>, MemoryError> {
        let connection = self.connection()?;
        ensure_builtin_chat_sessions(&connection)?;
        let mut statement = connection.prepare(
            "SELECT id, title, kind, created_at, updated_at
             FROM chat_sessions
             ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END, updated_at DESC",
        )?;
        let rows = statement.query_map(params![CHAT_CLI_ID], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                kind: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn clear_interactions(&self) -> Result<usize, MemoryError> {
        self.clear_interactions_for_chat(DEFAULT_CONVERSATION_ID)
    }

    pub fn clear_interactions_for_chat(&self, chat_id: &str) -> Result<usize, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        Ok(connection.execute(
            "DELETE FROM interaction_memories WHERE chat_id = ?1",
            params![chat_id],
        )?)
    }

    pub fn delete_chat_session(&self, chat_id: &str) -> Result<usize, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        if chat_id == CHAT_CLI_ID {
            return Ok(0);
        }
        let connection = self.connection()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM interaction_memories WHERE chat_id = ?1",
            params![chat_id],
        )?;
        let deleted = transaction.execute(
            "DELETE FROM chat_sessions
             WHERE id = ?1 AND kind = 'conversation'",
            params![chat_id],
        )?;
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn rename_chat_session(&self, chat_id: &str, new_title: &str) -> Result<usize, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        let updated = connection.execute(
            "UPDATE chat_sessions
             SET title = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![chat_id, new_title.trim()],
        )?;
        Ok(updated)
    }

    pub fn save_workspace_session(
        &self,
        workspace_path: &str,
        summary: &str,
        verification: &str,
    ) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO workspace_sessions (workspace_path, summary, verification, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(workspace_path) DO UPDATE SET
               summary = excluded.summary,
               verification = excluded.verification,
               updated_at = CURRENT_TIMESTAMP",
            params![workspace_path, summary, verification],
        )?;
        Ok(())
    }

    pub fn workspace_session(
        &self,
        workspace_path: &str,
    ) -> Result<Option<WorkspaceSession>, MemoryError> {
        let connection = self.connection()?;
        Ok(connection
            .query_row(
                "SELECT workspace_path, summary, verification, updated_at
                 FROM workspace_sessions WHERE workspace_path = ?1",
                params![workspace_path],
                |row| {
                    Ok(WorkspaceSession {
                        workspace_path: row.get(0)?,
                        summary: row.get(1)?,
                        verification: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn add_learned_skill(
        &self,
        name: &str,
        source_path: &str,
        content: &str,
    ) -> Result<LearnedSkill, MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO learned_skills (name, source_path, content, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(source_path) DO UPDATE SET
               name = excluded.name,
               content = excluded.content,
               updated_at = CURRENT_TIMESTAMP",
            params![name, source_path, content],
        )?;
        Ok(connection.query_row(
            "SELECT id, name, source_path, content, created_at
             FROM learned_skills WHERE source_path = ?1",
            params![source_path],
            learned_skill_row,
        )?)
    }

    pub fn learned_skills(&self, limit: usize) -> Result<Vec<LearnedSkill>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, source_path, content, created_at
             FROM learned_skills ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], learned_skill_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_learned_skill(&self, identifier: &str) -> Result<usize, MemoryError> {
        let connection = self.connection()?;
        Ok(connection.execute(
            "DELETE FROM learned_skills
             WHERE CAST(id AS TEXT) = ?1 OR source_path = ?1 OR name = ?1",
            params![identifier],
        )?)
    }

    fn connection(&self) -> Result<Connection, MemoryError> {
        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory).map_err(|source| MemoryError::CreateDirectory {
                path: directory.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&self.path)?;

        static INITIALIZED_DATABASES: std::sync::LazyLock<
            std::sync::Mutex<std::collections::HashSet<PathBuf>>,
        > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

        let needs_init = {
            let mut set = INITIALIZED_DATABASES.lock().unwrap();
            set.insert(self.path.clone())
        };

        if needs_init {
            initialize(
                &connection,
                memory_path().is_ok_and(|default_path| default_path == self.path),
            )?;
        }
        Ok(connection)
    }
}

pub fn memory_path() -> Result<PathBuf, MemoryError> {
    dirs::config_dir()
        .map(|directory| directory.join("mint").join("mint-knowledge.sqlite"))
        .ok_or(MemoryError::ConfigDirectoryUnavailable)
}

fn migrate_json_history(connection: &Connection) -> Result<(), rusqlite::Error> {
    if cfg!(test) {
        return Ok(());
    }
    let config_dir = match dirs::config_dir() {
        Some(dir) => dir,
        None => return Ok(()),
    };
    let json_path = config_dir.join("mint").join("mint-chat-history.json");
    if !json_path.exists() {
        return Ok(());
    }

    let already_migrated: bool = connection
        .query_row(
            "SELECT 1 FROM user_profile WHERE key = 'json_history_migrated'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if already_migrated {
        return Ok(());
    }

    let file_content = match std::fs::read_to_string(&json_path) {
        Ok(content) => content,
        Err(_) => return Ok(()),
    };

    let messages: Vec<serde_json::Value> = match serde_json::from_str(&file_content) {
        Ok(msgs) => msgs,
        Err(_) => return Ok(()),
    };

    let mut i = 0;
    while i < messages.len() {
        let msg = &messages[i];
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        if role == "user" {
            let user_text = msg
                .get("parts")
                .and_then(|p| p.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            let mut ai_text = "";
            let ai_text_buf;

            if i + 1 < messages.len() {
                let next_msg = &messages[i + 1];
                let next_role = next_msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                if next_role == "model" {
                    let raw_ai_text = next_msg
                        .get("parts")
                        .and_then(|p| p.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|first| first.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw_ai_text) {
                        if let Some(resp) = parsed.get("response").and_then(|r| r.as_str()) {
                            ai_text_buf = resp.to_string();
                            ai_text = &ai_text_buf;
                        } else {
                            ai_text = raw_ai_text;
                        }
                    } else {
                        ai_text = raw_ai_text;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }

            if !user_text.trim().is_empty() {
                let created_at = msg.get("timestamp").and_then(|t| t.as_str()).unwrap_or("");
                if !created_at.is_empty() {
                    let _ = connection.execute(
                        "INSERT INTO interaction_memories (user_text, ai_text, created_at)
                         VALUES (?1, ?2, ?3)",
                        params![user_text, ai_text, created_at],
                    );
                } else {
                    let _ = connection.execute(
                        "INSERT INTO interaction_memories (user_text, ai_text)
                         VALUES (?1, ?2)",
                        params![user_text, ai_text],
                    );
                }
            }
        } else {
            i += 1;
        }
    }

    let _ = connection.execute(
        "INSERT OR REPLACE INTO user_profile (key, value, updated_at)
         VALUES ('json_history_migrated', 'true', CURRENT_TIMESTAMP)",
        [],
    );

    Ok(())
}

fn initialize(
    connection: &Connection,
    migrate_legacy_history: bool,
) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         CREATE TABLE IF NOT EXISTS user_profile (
           key TEXT PRIMARY KEY,
           value TEXT,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS interaction_memories (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           chat_id TEXT NOT NULL DEFAULT 'conversation-default',
           user_text TEXT NOT NULL,
           ai_text TEXT NOT NULL,
           provider TEXT NOT NULL DEFAULT '',
           model TEXT NOT NULL DEFAULT '',
           keywords TEXT DEFAULT '',
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS chat_sessions (
           id TEXT PRIMARY KEY,
           title TEXT NOT NULL DEFAULT 'New chat',
           kind TEXT NOT NULL DEFAULT 'conversation',
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS learned_skills (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           name TEXT NOT NULL,
           source_path TEXT NOT NULL UNIQUE,
           content TEXT NOT NULL,
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS workspace_sessions (
           workspace_path TEXT PRIMARY KEY,
           summary TEXT NOT NULL,
           verification TEXT NOT NULL DEFAULT '',
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "chat_id",
        "TEXT NOT NULL DEFAULT 'conversation-default'",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "provider",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "model",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "chat_sessions",
        "kind",
        "TEXT NOT NULL DEFAULT 'conversation'",
    )?;
    connection.execute(
        "UPDATE interaction_memories
         SET chat_id = ?1
         WHERE chat_id IS NULL OR trim(chat_id) = ''",
        params![DEFAULT_CONVERSATION_ID],
    )?;
    ensure_builtin_chat_sessions(connection)?;
    connection.execute(
        "UPDATE interaction_memories
         SET chat_id = ?1
         WHERE chat_id = ?2",
        params![CHAT_CLI_ID, DEFAULT_CONVERSATION_ID],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_interaction_memories_chat_id_id
         ON interaction_memories(chat_id, id)",
        [],
    )?;
    if migrate_legacy_history {
        migrate_json_history(connection)?;
    }
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), rusqlite::Error> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    connection.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

fn learned_skill_row(row: &rusqlite::Row<'_>) -> Result<LearnedSkill, rusqlite::Error> {
    Ok(LearnedSkill {
        id: row.get(0)?,
        name: row.get(1)?,
        source_path: row.get(2)?,
        content: row.get(3)?,
        created_at: row.get(4)?,
    })
}

fn interaction_row(row: &rusqlite::Row<'_>) -> Result<InteractionMemory, rusqlite::Error> {
    Ok(InteractionMemory {
        id: row.get(0)?,
        chat_id: row.get(1)?,
        user_text: row.get(2)?,
        ai_text: row.get(3)?,
        provider: row.get(4)?,
        model: row.get(5)?,
        created_at: row.get(6)?,
    })
}

fn normalized_chat_id(chat_id: &str) -> String {
    let trimmed = chat_id.trim();
    if trimmed.is_empty() {
        DEFAULT_CONVERSATION_ID.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn ensure_builtin_chat_sessions(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
         VALUES (?1, 'cli', 'cli')",
        params![CHAT_CLI_ID],
    )?;
    connection.execute(
        "UPDATE chat_sessions
         SET title = 'cli', kind = 'cli'
         WHERE id = ?1",
        params![CHAT_CLI_ID],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
         VALUES (?1, 'Conversation', 'conversation')",
        params![DEFAULT_CONVERSATION_ID],
    )?;
    Ok(())
}

fn ensure_chat_session_row(connection: &Connection, chat_id: &str) -> Result<(), rusqlite::Error> {
    let kind = if chat_id == CHAT_CLI_ID {
        "cli"
    } else {
        "conversation"
    };
    let title = if chat_id == CHAT_CLI_ID {
        "Chat CLI"
    } else {
        "New chat"
    };
    connection.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
         VALUES (?1, ?2, ?3)",
        params![chat_id, title, kind],
    )?;
    Ok(())
}

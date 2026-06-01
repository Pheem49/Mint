use std::path::PathBuf;

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

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
    pub user_text: String,
    pub ai_text: String,
    pub created_at: String,
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
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO interaction_memories (user_text, ai_text)
             VALUES (?1, ?2)",
            params![user_text, ai_text],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn recent_interactions(&self, limit: usize) -> Result<Vec<InteractionMemory>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, user_text, ai_text, created_at
             FROM interaction_memories
             ORDER BY id DESC
             LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], |row| {
            Ok(InteractionMemory {
                id: row.get(0)?,
                user_text: row.get(1)?,
                ai_text: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn clear_interactions(&self) -> Result<usize, MemoryError> {
        let connection = self.connection()?;
        Ok(connection.execute("DELETE FROM interaction_memories", [])?)
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
        initialize(&connection)?;
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

fn initialize(connection: &Connection) -> Result<(), rusqlite::Error> {
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
           user_text TEXT NOT NULL,
           ai_text TEXT NOT NULL,
           keywords TEXT DEFAULT '',
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP
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
    migrate_json_history(connection)?;
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

#[cfg(test)]
mod tests {
    use super::*;

    fn store(name: &str) -> MemoryStore {
        MemoryStore::open(
            std::env::temp_dir().join(format!("mint-core-{name}-{}.sqlite", std::process::id())),
        )
    }

    #[test]
    fn stores_and_reads_profile_values() {
        let store = store("profile");
        store.set_profile("name", "Mint").unwrap();
        assert_eq!(store.get_profile("name").unwrap().as_deref(), Some("Mint"));
    }

    #[test]
    fn stores_recent_interactions() {
        let store = store("interactions");
        store.add_interaction("hello", "hi").unwrap();
        let interactions = store.recent_interactions(1).unwrap();
        assert_eq!(interactions[0].user_text, "hello");
        assert_eq!(interactions[0].ai_text, "hi");
    }

    #[test]
    fn clears_interactions() {
        let store = store("clear-interactions");
        store.add_interaction("hello", "hi").unwrap();
        assert_eq!(store.clear_interactions().unwrap(), 1);
        assert!(store.recent_interactions(10).unwrap().is_empty());
    }

    #[test]
    fn stores_workspace_session_summary() {
        let store = store("workspace-session");
        store
            .save_workspace_session("/tmp/project", "implemented", "cargo test")
            .unwrap();
        let session = store.workspace_session("/tmp/project").unwrap().unwrap();
        assert_eq!(session.summary, "implemented");
        assert_eq!(session.verification, "cargo test");
    }

    #[test]
    fn stores_lists_and_deletes_learned_skills() {
        let store = store("skills");
        store
            .add_learned_skill("guide", "/tmp/guide.md", "Use focused patches.")
            .unwrap();
        assert_eq!(store.learned_skills(10).unwrap()[0].name, "guide");
        assert_eq!(store.delete_learned_skill("guide").unwrap(), 1);
        assert!(store.learned_skills(10).unwrap().is_empty());
    }
}

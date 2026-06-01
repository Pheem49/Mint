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
         );",
    )
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

use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
};

use rusqlite::{Connection, params};
use serde::Serialize;
use thiserror::Error;

use crate::{Capability, MintConfig, SafetyError, assert_path_capability, memory_path};

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const CHUNK_CHARACTERS: usize = 1000;
const CHUNK_OVERLAP: usize = 200;

#[derive(Debug, Error)]
pub enum KnowledgeError {
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("unable to locate Mint knowledge database: {0}")]
    DatabasePath(String),
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("unable to read knowledge file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("knowledge file is too large (> 10 MiB): {0}")]
    TooLarge(PathBuf),
    #[error("native text indexing does not support this file type yet: {0}")]
    UnsupportedFileType(PathBuf),
    #[error("knowledge file does not contain readable text: {0}")]
    Empty(PathBuf),
}

#[derive(Debug, Clone)]
pub struct KnowledgeStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeSource {
    pub id: i64,
    pub path: String,
    pub name: String,
    pub last_indexed: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KnowledgeHit {
    pub source: String,
    pub text: String,
}

impl KnowledgeStore {
    pub fn open_default() -> Result<Self, KnowledgeError> {
        Ok(Self::open(memory_path().map_err(|error| {
            KnowledgeError::DatabasePath(error.to_string())
        })?))
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn index_file(&self, path: &Path, config: &MintConfig) -> Result<usize, KnowledgeError> {
        let path = assert_path_capability(path, Capability::Read, config)?;
        let metadata = fs::metadata(&path).map_err(|source| KnowledgeError::Read {
            path: path.clone(),
            source,
        })?;
        if metadata.len() > MAX_FILE_BYTES {
            return Err(KnowledgeError::TooLarge(path));
        }
        if !supported(&path) {
            return Err(KnowledgeError::UnsupportedFileType(path));
        }
        let content = fs::read_to_string(&path).map_err(|source| KnowledgeError::Read {
            path: path.clone(),
            source,
        })?;
        if content.trim().is_empty() {
            return Err(KnowledgeError::Empty(path));
        }
        let chunks = chunks(&content);
        let connection = self.connection()?;
        let path_text = path.to_string_lossy().to_string();
        let name = path
            .file_name()
            .map(|name| name.to_string_lossy().to_string())
            .unwrap_or_else(|| path_text.clone());
        let hash = content_hash(&content);
        connection.execute(
            "INSERT INTO sources (path, name, hash, last_indexed)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(path) DO UPDATE SET
               name = excluded.name,
               hash = excluded.hash,
               last_indexed = CURRENT_TIMESTAMP",
            params![path_text, name, hash],
        )?;
        let source_id: i64 = connection.query_row(
            "SELECT id FROM sources WHERE path = ?1",
            params![path_text],
            |row| row.get(0),
        )?;
        connection.execute(
            "DELETE FROM chunks WHERE source_id = ?1",
            params![source_id],
        )?;
        for chunk in &chunks {
            connection.execute(
                "INSERT INTO chunks (source_id, text, embedding) VALUES (?1, ?2, NULL)",
                params![source_id, chunk],
            )?;
        }
        Ok(chunks.len())
    }

    pub fn list_sources(&self) -> Result<Vec<KnowledgeSource>, KnowledgeError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, path, name, last_indexed FROM sources ORDER BY last_indexed DESC, id DESC",
        )?;
        statement
            .query_map([], |row| {
                Ok(KnowledgeSource {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    name: row.get(2)?,
                    last_indexed: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<KnowledgeHit>, KnowledgeError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT sources.name, chunks.text
             FROM chunks
             JOIN sources ON sources.id = chunks.source_id
             WHERE chunks.text LIKE ?1
             ORDER BY chunks.id DESC
             LIMIT ?2",
        )?;
        statement
            .query_map(
                params![format!("%{}%", query.trim()), limit as i64],
                |row| {
                    Ok(KnowledgeHit {
                        source: row.get(0)?,
                        text: row.get(1)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    fn connection(&self) -> Result<Connection, KnowledgeError> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory).map_err(|source| KnowledgeError::Read {
                path: directory.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&self.path)?;
        connection.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             CREATE TABLE IF NOT EXISTS sources (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               path TEXT UNIQUE,
               name TEXT,
               hash TEXT,
               last_indexed DATETIME DEFAULT CURRENT_TIMESTAMP
             );
             CREATE TABLE IF NOT EXISTS chunks (
               id INTEGER PRIMARY KEY AUTOINCREMENT,
               source_id INTEGER,
               text TEXT,
               embedding BLOB,
               FOREIGN KEY(source_id) REFERENCES sources(id) ON DELETE CASCADE
             );
             CREATE INDEX IF NOT EXISTS idx_chunks_source ON chunks(source_id);",
        )?;
        Ok(connection)
    }
}

fn supported(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            matches!(
                extension.to_ascii_lowercase().as_str(),
                "txt"
                    | "md"
                    | "json"
                    | "toml"
                    | "yaml"
                    | "yml"
                    | "rs"
                    | "ts"
                    | "tsx"
                    | "js"
                    | "jsx"
                    | "py"
                    | "html"
                    | "css"
            )
        })
}

fn chunks(text: &str) -> Vec<String> {
    let characters = text.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < characters.len() {
        let end = (start + CHUNK_CHARACTERS).min(characters.len());
        chunks.push(characters[start..end].iter().collect());
        if end == characters.len() {
            break;
        }
        start = end.saturating_sub(CHUNK_OVERLAP);
    }
    chunks
}

fn content_hash(content: &str) -> String {
    let mut hasher = DefaultHasher::new();
    content.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_and_searches_text_files() {
        let root = std::env::temp_dir().join(format!("mint-knowledge-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("notes.md");
        fs::write(&file, "Mint native knowledge search").unwrap();
        let store = KnowledgeStore::open(root.join("knowledge.sqlite"));
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            ..MintConfig::default()
        };
        assert_eq!(store.index_file(&file, &config).unwrap(), 1);
        assert_eq!(
            store.search("native knowledge", 5).unwrap()[0].source,
            "notes.md"
        );
        let _ = fs::remove_dir_all(root);
    }
}

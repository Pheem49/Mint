use std::{
    collections::hash_map::DefaultHasher,
    fs,
    hash::{Hash, Hasher},
    path::{Path, PathBuf},
    process::Command,
};

use quick_xml::{Reader, events::Event};
use rusqlite::{Connection, params, types::Type};
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
    #[error("unable to extract text from {path}: {message}")]
    Extract { path: PathBuf, message: String },
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
    pub score: f32,
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
        let content = extract_text(&path)?;
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
                "INSERT INTO chunks (source_id, text, embedding) VALUES (?1, ?2, ?3)",
                params![source_id, chunk, encode_embedding(&embedding(chunk))],
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
        let query_embedding = embedding(query);
        let mut statement = connection.prepare(
            "SELECT sources.name, chunks.text, chunks.embedding
             FROM chunks JOIN sources ON sources.id = chunks.source_id",
        )?;
        let mut hits = statement
            .query_map([], |row| {
                let text: String = row.get(1)?;
                let vector = row
                    .get::<_, Option<Vec<u8>>>(2)?
                    .map(|raw| {
                        decode_embedding(&raw).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(2, Type::Blob, error.into())
                        })
                    })
                    .transpose()?
                    .unwrap_or_else(|| embedding(&text));
                Ok(KnowledgeHit {
                    source: row.get(0)?,
                    text,
                    score: cosine_similarity(&query_embedding, &vector),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        hits.sort_by(|left, right| right.score.total_cmp(&left.score));
        hits.truncate(limit);
        Ok(hits)
    }

    fn connection(&self) -> Result<Connection, KnowledgeError> {
        if let Some(directory) = self.path.parent() {
            fs::create_dir_all(directory).map_err(|source| KnowledgeError::Read {
                path: directory.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&self.path)?;
        
        static INITIALIZED_DATABASES: std::sync::LazyLock<std::sync::Mutex<std::collections::HashSet<PathBuf>>> =
            std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

        let needs_init = {
            let mut set = INITIALIZED_DATABASES.lock().unwrap();
            set.insert(self.path.clone())
        };

        if needs_init {
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
        }
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
                    | "pdf"
                    | "docx"
                    | "xlsx"
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

fn extract_text(path: &Path) -> Result<String, KnowledgeError> {
    match extension(path).as_str() {
        "pdf" => command_text(path, "pdftotext", &["-layout"], Some("-")),
        "docx" => {
            let xml = command_text(path, "unzip", &["-p"], Some("word/document.xml"))?;
            xml_text(&xml, &["w:t", "w:tab", "w:br", "w:p"])
        }
        "xlsx" => extract_xlsx(path),
        _ => fs::read_to_string(path).map_err(|source| KnowledgeError::Read {
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn extract_xlsx(path: &Path) -> Result<String, KnowledgeError> {
    let files = command_text(path, "unzip", &["-Z1"], None)?;
    let mut text = String::new();
    for entry in files.lines().filter(|entry| {
        *entry == "xl/sharedStrings.xml"
            || (entry.starts_with("xl/worksheets/") && entry.ends_with(".xml"))
    }) {
        let xml = command_text(path, "unzip", &["-p"], Some(entry))?;
        let extracted = xml_text(&xml, &["t", "v", "row"])?;
        if !extracted.trim().is_empty() {
            text.push_str(&format!("\nSheet XML: {entry}\n{extracted}\n"));
        }
    }
    Ok(text)
}

fn command_text(
    path: &Path,
    program: &'static str,
    prefix: &[&str],
    suffix: Option<&str>,
) -> Result<String, KnowledgeError> {
    let mut command = Command::new(program);
    command.args(prefix).arg(path);
    if let Some(suffix) = suffix {
        command.arg(suffix);
    }
    let output = command.output().map_err(|error| KnowledgeError::Extract {
        path: path.to_path_buf(),
        message: format!("unable to run {program}: {error}"),
    })?;
    if !output.status.success() {
        return Err(KnowledgeError::Extract {
            path: path.to_path_buf(),
            message: format!(
                "{program} exited with {}: {}",
                output.status,
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).into_owned())
}

fn xml_text(xml: &str, text_elements: &[&str]) -> Result<String, KnowledgeError> {
    let mut reader = Reader::from_str(xml);
    let mut active = false;
    let mut output = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                active = text_elements.contains(&name.as_str());
                if matches!(name.as_str(), "w:p" | "row") {
                    output.push('\n');
                }
            }
            Ok(Event::Empty(element)) => {
                let name = String::from_utf8_lossy(element.name().as_ref()).into_owned();
                if matches!(name.as_str(), "w:tab") {
                    output.push('\t');
                } else if matches!(name.as_str(), "w:br") {
                    output.push('\n');
                }
            }
            Ok(Event::Text(text)) if active => {
                output.push_str(&text.decode().map_err(|error| KnowledgeError::Extract {
                    path: PathBuf::from("<xml>"),
                    message: error.to_string(),
                })?);
                output.push(' ');
            }
            Ok(Event::End(_)) => active = false,
            Ok(Event::Eof) => break,
            Err(error) => {
                return Err(KnowledgeError::Extract {
                    path: PathBuf::from("<xml>"),
                    message: error.to_string(),
                });
            }
            _ => {}
        }
    }
    Ok(output)
}

fn extension(path: &Path) -> String {
    path.extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
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

const EMBEDDING_DIMENSIONS: usize = 256;

fn embedding(text: &str) -> Vec<f32> {
    let mut vector = vec![0.0; EMBEDDING_DIMENSIONS];
    for token in text
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
    {
        let mut hasher = DefaultHasher::new();
        let has_upper = token.chars().any(|c| c.is_uppercase());
        if has_upper {
            use std::cell::RefCell;
            thread_local! {
                static LOWERCASE_BUF: RefCell<String> = RefCell::new(String::with_capacity(64));
            }
            LOWERCASE_BUF.with(|buf| {
                let mut buf = buf.borrow_mut();
                buf.clear();
                for c in token.chars() {
                    for lc in c.to_lowercase() {
                        buf.push(lc);
                    }
                }
                use std::hash::Hash;
                buf.hash(&mut hasher);
            });
        } else {
            token.hash(&mut hasher);
        }
        let hash = hasher.finish();
        let index = hash as usize % EMBEDDING_DIMENSIONS;
        vector[index] += if hash & 1 == 0 { 1.0 } else { -1.0 };
    }
    let norm = vector.iter().map(|value| value * value).sum::<f32>().sqrt();
    if norm > 0.0 {
        vector.iter_mut().for_each(|value| *value /= norm);
    }
    vector
}

fn encode_embedding(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn decode_embedding(raw: &[u8]) -> Result<Vec<f32>, &'static str> {
    if !raw.len().is_multiple_of(4) {
        return Err("embedding blob length is invalid");
    }
    Ok(raw
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect())
}

fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
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

    #[test]
    fn extracts_docx_xml_text() {
        assert_eq!(
            xml_text(
                "<w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t>Mint</w:t></w:r></w:p>",
                &["w:t", "w:p"]
            )
            .unwrap()
            .trim(),
            "Hello Mint"
        );
    }

    #[test]
    fn embedding_search_ranks_related_chunk_first() {
        let root = std::env::temp_dir().join("mint-knowledge-embedding");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let store = KnowledgeStore::open(root.join("knowledge.sqlite"));
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            ..MintConfig::default()
        };
        let rust = root.join("rust.md");
        let cooking = root.join("cooking.md");
        fs::write(&rust, "Rust backend ownership borrowing cargo").unwrap();
        fs::write(&cooking, "Pasta tomato basil kitchen recipe").unwrap();
        store.index_file(&rust, &config).unwrap();
        store.index_file(&cooking, &config).unwrap();
        assert_eq!(
            store.search("cargo rust backend", 1).unwrap()[0].source,
            "rust.md"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn searches_legacy_chunks_without_stored_embeddings() {
        let root = std::env::temp_dir().join("mint-knowledge-legacy-embedding");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let store = KnowledgeStore::open(root.join("knowledge.sqlite"));
        let connection = store.connection().unwrap();
        connection
            .execute(
                "INSERT INTO sources (path, name, hash) VALUES ('legacy', 'legacy.md', 'hash')",
                [],
            )
            .unwrap();
        connection
            .execute(
                "INSERT INTO chunks (source_id, text, embedding) VALUES (1, 'legacy rust backend', NULL)",
                [],
            )
            .unwrap();
        assert_eq!(store.search("rust", 1).unwrap()[0].source, "legacy.md");
        let _ = fs::remove_dir_all(root);
    }
}

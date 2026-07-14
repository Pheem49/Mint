use std::{
    cmp::Ordering,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{CodeInspectionError, MintConfig, list_code_files};

const EMBEDDING_MODEL: &str = "gemini-embedding-001";
const MAX_CHARS: usize = 1800;

#[derive(Debug, Error)]
pub enum SemanticError {
    #[error(transparent)]
    Inspect(#[from] CodeInspectionError),
    #[error("Gemini API key is required for semantic code embeddings")]
    MissingApiKey,
    #[error("unable to read semantic index {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to write semantic index {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to parse semantic index {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("semantic embedding request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("semantic embedding response did not contain an embedding")]
    MissingEmbedding,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticChunk {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub text: String,
    pub embedding: Vec<f64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticIndex {
    pub root: PathBuf,
    pub model: String,
    pub file_count: usize,
    pub chunk_count: usize,
    pub chunks: Vec<SemanticChunk>,
    pub store_path: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticHit {
    pub file: PathBuf,
    pub start_line: usize,
    pub end_line: usize,
    pub score: f64,
    pub text: String,
}

pub async fn index_semantic_code(
    root: &Path,
    config: &MintConfig,
) -> Result<SemanticIndex, SemanticError> {
    let root = fs::canonicalize(root).map_err(|source| SemanticError::Read {
        path: root.to_path_buf(),
        source,
    })?;
    let files = list_code_files(&root, usize::MAX, config)?;
    let mut chunks = Vec::new();
    for file in &files {
        if file.size > 512 * 1024 || !is_source_file(&file.path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.path) else {
            continue;
        };
        for (start_line, end_line, text) in chunk_text(&content) {
            chunks.push(SemanticChunk {
                file: file.path.clone(),
                start_line,
                end_line,
                embedding: embed_text(config, &text).await?,
                text,
            });
        }
    }
    let store_path = semantic_store_path(&root)?;
    let index = SemanticIndex {
        root,
        model: EMBEDDING_MODEL.into(),
        file_count: files.len(),
        chunk_count: chunks.len(),
        chunks,
        store_path: store_path.clone(),
    };
    if let Some(directory) = store_path.parent() {
        fs::create_dir_all(directory).map_err(|source| SemanticError::Write {
            path: directory.into(),
            source,
        })?;
    }
    fs::write(&store_path, serde_json::to_string_pretty(&index).unwrap()).map_err(|source| {
        SemanticError::Write {
            path: store_path,
            source,
        }
    })?;
    Ok(index)
}

pub async fn search_semantic_code(
    root: &Path,
    query: &str,
    limit: usize,
    config: &MintConfig,
) -> Result<Vec<SemanticHit>, SemanticError> {
    let root = fs::canonicalize(root).map_err(|source| SemanticError::Read {
        path: root.to_path_buf(),
        source,
    })?;
    let path = semantic_store_path(&root)?;
    let raw = fs::read_to_string(&path).map_err(|source| SemanticError::Read {
        path: path.clone(),
        source,
    })?;
    let index: SemanticIndex =
        serde_json::from_str(&raw).map_err(|source| SemanticError::Parse { path, source })?;
    let query = embed_text(config, query).await?;
    let mut hits = index
        .chunks
        .into_iter()
        .map(|chunk| SemanticHit {
            score: cosine_similarity(&query, &chunk.embedding),
            file: chunk.file,
            start_line: chunk.start_line,
            end_line: chunk.end_line,
            text: chunk.text,
        })
        .collect::<Vec<_>>();
    hits.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(Ordering::Equal)
    });
    hits.truncate(limit.max(1));
    Ok(hits)
}

fn chunk_text(content: &str) -> Vec<(usize, usize, String)> {
    let lines = content.lines().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let mut end = start;
        let mut chars = 0;
        while end < lines.len() && (end == start || chars + lines[end].len() < MAX_CHARS) {
            chars += lines[end].len() + 1;
            end += 1;
        }
        chunks.push((start + 1, end, lines[start..end].join("\n")));
        start = end;
    }
    chunks
}

async fn embed_text(config: &MintConfig, text: &str) -> Result<Vec<f64>, SemanticError> {
    let key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if key.trim().is_empty() {
        return Err(SemanticError::MissingApiKey);
    }
    let value: Value = crate::HTTP_CLIENT.clone()
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{EMBEDDING_MODEL}:embedContent?key={key}"
        ))
        .json(&json!({ "content": { "parts": [{ "text": text }] } }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    value["embedding"]["values"]
        .as_array()
        .map(|values| values.iter().filter_map(Value::as_f64).collect())
        .filter(|values: &Vec<f64>| !values.is_empty())
        .ok_or(SemanticError::MissingEmbedding)
}

fn semantic_store_path(root: &Path) -> Result<PathBuf, SemanticError> {
    let hash = format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()));
    Ok(dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("mint")
        .join("semantic-code")
        .join(format!("{}.json", &hash[..16])))
}

fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| matches!(extension, "rs" | "js" | "jsx" | "ts" | "tsx" | "py"))
}

fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    let mut dot = 0.0;
    let mut norm_left = 0.0;
    let mut norm_right = 0.0;
    for (left, right) in left.iter().zip(right) {
        dot += left * right;
        norm_left += left * left;
        norm_right += right * right;
    }
    if norm_left == 0.0 || norm_right == 0.0 {
        0.0
    } else {
        dot / (norm_left.sqrt() * norm_right.sqrt())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunks_large_source_text() {
        let chunks = chunk_text(&format!("{}\n{}", "a".repeat(1700), "b".repeat(300)));
        assert_eq!(chunks.len(), 2);
    }

    #[test]
    fn computes_cosine_similarity() {
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[1.0, 0.0]), 1.0);
        assert_eq!(cosine_similarity(&[1.0, 0.0], &[0.0, 1.0]), 0.0);
    }
}

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

const GEMINI_EMBEDDING_MODEL: &str = "gemini-embedding-001";
pub const FASTEMBED_MODEL: &str = "fastembed-bge-small-en-v1.5";
pub const FEATURE_HASH_MODEL: &str = "feature-hash-256";
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
    let mut raw_chunks = Vec::new();
    for file in &files {
        if file.size > 512 * 1024 || !is_source_file(&file.path) {
            continue;
        }
        let Ok(content) = fs::read_to_string(&file.path) else {
            continue;
        };
        for (start_line, end_line, text) in chunk_text(&content) {
            raw_chunks.push((file.path.clone(), start_line, end_line, text));
        }
    }

    let texts: Vec<&str> = raw_chunks.iter().map(|(_, _, _, t)| t.as_str()).collect();
    let (embeddings, model_name) = generate_embeddings(config, &texts).await?;

    let mut chunks = Vec::with_capacity(raw_chunks.len());
    for ((file, start_line, end_line, text), embedding) in raw_chunks.into_iter().zip(embeddings) {
        chunks.push(SemanticChunk {
            file,
            start_line,
            end_line,
            text,
            embedding,
        });
    }

    let store_path = semantic_store_path(&root)?;
    let index = SemanticIndex {
        root,
        model: model_name,
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

    if index.chunks.is_empty() {
        return Ok(Vec::new());
    }

    let query_embedding = embed_query_for_index(&index, config, query).await?;
    let mut hits = index
        .chunks
        .into_iter()
        .filter(|chunk| chunk.embedding.len() == query_embedding.len())
        .map(|chunk| SemanticHit {
            score: cosine_similarity(&query_embedding, &chunk.embedding),
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

async fn generate_embeddings(
    config: &MintConfig,
    texts: &[&str],
) -> Result<(Vec<Vec<f64>>, String), SemanticError> {
    if texts.is_empty() {
        return Ok((Vec::new(), FASTEMBED_MODEL.to_string()));
    }

    // 1. Try local FastEmbed first (fast, multi-threaded CPU ONNX, offline)
    match crate::search::local_embedding::embed_texts(texts) {
        Ok(vectors) if !vectors.is_empty() => {
            let f64_vectors = vectors
                .into_iter()
                .map(|v| v.into_iter().map(|x| x as f64).collect())
                .collect();
            return Ok((f64_vectors, FASTEMBED_MODEL.to_string()));
        }
        Ok(_) => {}
        Err(err) => {
            eprintln!(
                "FastEmbed local embedding unavailable ({err}), checking fallback providers"
            );
        }
    }

    // 2. Fallback to Gemini API if key is available
    let key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if !key.trim().is_empty() {
        let mut gemini_vectors = Vec::with_capacity(texts.len());
        let mut success = true;
        for text in texts {
            match embed_text_gemini(&key, text).await {
                Ok(v) => gemini_vectors.push(v),
                Err(e) => {
                    eprintln!("Gemini embedding fallback failed: {e}");
                    success = false;
                    break;
                }
            }
        }
        if success && gemini_vectors.len() == texts.len() {
            return Ok((gemini_vectors, GEMINI_EMBEDDING_MODEL.to_string()));
        }
    }

    // 3. Fallback to 100% offline feature-hash vectorizer
    let hash_vectors = texts
        .iter()
        .map(|t| {
            crate::search::text_embedding::embedding(t)
                .into_iter()
                .map(|x| x as f64)
                .collect()
        })
        .collect();
    Ok((hash_vectors, FEATURE_HASH_MODEL.to_string()))
}

async fn embed_query_for_index(
    index: &SemanticIndex,
    config: &MintConfig,
    query: &str,
) -> Result<Vec<f64>, SemanticError> {
    let chunk_dim = index.chunks.first().map(|c| c.embedding.len()).unwrap_or(0);

    // If index was built with FastEmbed or has 384 dims:
    if index.model.contains("fastembed")
        || chunk_dim == crate::search::local_embedding::LOCAL_EMBEDDING_DIM
    {
        if let Ok(vec) = crate::search::local_embedding::embed_query(query) {
            return Ok(vec.into_iter().map(|x| x as f64).collect());
        }
    }

    // If index was built with Gemini embedding:
    if index.model.contains("gemini") {
        let key = if config.api_key.trim().is_empty() {
            std::env::var("GEMINI_API_KEY").unwrap_or_default()
        } else {
            config.api_key.clone()
        };
        if !key.trim().is_empty() {
            if let Ok(vec) = embed_text_gemini(&key, query).await {
                return Ok(vec);
            }
        }
    }

    // If index was built with feature hashing or has 256 dims (or fallback):
    if index.model.contains("feature-hash")
        || chunk_dim == crate::search::text_embedding::EMBEDDING_DIMENSIONS
    {
        return Ok(crate::search::text_embedding::embedding(query)
            .into_iter()
            .map(|x| x as f64)
            .collect());
    }

    // Fallback try:
    if let Ok(vec) = crate::search::local_embedding::embed_query(query) {
        return Ok(vec.into_iter().map(|x| x as f64).collect());
    }

    Ok(crate::search::text_embedding::embedding(query)
        .into_iter()
        .map(|x| x as f64)
        .collect())
}

pub async fn embed_text(config: &MintConfig, text: &str) -> Result<Vec<f64>, SemanticError> {
    if let Ok(vec) = crate::search::local_embedding::embed_query(text) {
        return Ok(vec.into_iter().map(|x| x as f64).collect());
    }
    let key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if !key.trim().is_empty() {
        if let Ok(vec) = embed_text_gemini(&key, text).await {
            return Ok(vec);
        }
    }
    Ok(crate::search::text_embedding::embedding(text)
        .into_iter()
        .map(|x| x as f64)
        .collect())
}

async fn embed_text_gemini(key: &str, text: &str) -> Result<Vec<f64>, SemanticError> {
    if key.trim().is_empty() {
        return Err(SemanticError::MissingApiKey);
    }
    let value: Value = crate::HTTP_CLIENT
        .clone()
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{GEMINI_EMBEDDING_MODEL}:embedContent?key={key}"
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

fn chunk_text(content: &str) -> Vec<(usize, usize, String)> {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < lines.len() {
        let mut max_end = start;
        let mut chars = 0;
        while max_end < lines.len()
            && (max_end == start || chars + lines[max_end].len() + 1 <= MAX_CHARS)
        {
            chars += lines[max_end].len() + 1;
            max_end += 1;
        }

        if max_end >= lines.len() {
            let chunk_lines = &lines[start..lines.len()];
            if !chunk_lines.is_empty() {
                chunks.push((start + 1, lines.len(), chunk_lines.join("\n")));
            }
            break;
        }

        // Search backwards from max_end to find the best syntax boundary line
        let mut best_split = max_end;
        let min_split = start + (max_end - start) / 2;

        for idx in (min_split..max_end).rev() {
            let line = lines[idx].trim();
            if line.is_empty() || line == "}" || line.ends_with('}') || line == "};" {
                best_split = idx + 1;
                break;
            }
            if line.starts_with("pub ")
                || line.starts_with("fn ")
                || line.starts_with("def ")
                || line.starts_with("class ")
                || line.starts_with("struct ")
                || line.starts_with("impl ")
                || line.starts_with("export ")
                || line.starts_with("///")
                || line.starts_with("/**")
            {
                best_split = idx;
                break;
            }
        }

        let actual_end = if best_split > start {
            best_split
        } else {
            max_end
        };
        chunks.push((start + 1, actual_end, lines[start..actual_end].join("\n")));
        start = actual_end;
    }
    chunks
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

    #[test]
    fn chunks_at_syntax_boundaries() {
        let fn1 = format!("pub fn foo() {{\n{}\n}}", "    let x = 1;\n".repeat(80));
        let fn2 = format!("pub fn bar() {{\n{}\n}}", "    let y = 2;\n".repeat(40));
        let code = format!("{}\n\n{}", fn1, fn2);
        let chunks = chunk_text(&code);
        assert!(chunks.len() >= 2);
        assert!(chunks[0].2.contains("pub fn foo"));
        assert!(!chunks[0].2.contains("pub fn bar"));
    }
}

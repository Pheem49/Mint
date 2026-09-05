//! A tiny, dependency-free text embedding used for on-device similarity ranking.
//!
//! The vector is a normalized feature-hash (a.k.a. the hashing trick): every
//! alphanumeric token is hashed into one of [`EMBEDDING_DIMENSIONS`] buckets with
//! a ±1 sign, then the whole vector is L2-normalized so a dot product is a
//! cosine. It is Unicode-aware, so Thai (which has no spaces) still contributes
//! tokens, and it is fully deterministic and offline — no model, no API key, no
//! network. Quality is "smart keyword matching": it captures shared vocabulary
//! and substrings, not true paraphrase.
//!
//! Used by the document knowledge store (`knowledge.rs`, over file chunks) and by
//! long-term fact recall (`agent/memory.rs` + `orchestration/memory_skill.rs`).
//! [`FactEmbeddingBackend`] is the seam where a real learned-embedding backend
//! can be slotted in later without the recall code or the stored blob format
//! having to change.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

pub(crate) const EMBEDDING_DIMENSIONS: usize = 256;

/// Normalized feature-hash embedding of `text`. See the module docs.
pub(crate) fn embedding(text: &str) -> Vec<f32> {
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

/// Packs a vector into a little-endian `f32` blob for SQLite storage. The
/// dimension is recoverable from the byte length, so no header is written.
pub(crate) fn encode_embedding(vector: &[f32]) -> Vec<u8> {
    vector
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

/// Inverse of [`encode_embedding`].
pub(crate) fn decode_embedding(raw: &[u8]) -> Result<Vec<f32>, &'static str> {
    if !raw.len().is_multiple_of(4) {
        return Err("embedding blob length is invalid");
    }
    Ok(raw
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().unwrap()))
        .collect())
}

/// Cosine similarity — a plain dot product, since both operands are expected to
/// come from [`embedding`] already L2-normalized.
pub(crate) fn cosine_similarity(left: &[f32], right: &[f32]) -> f32 {
    left.iter()
        .zip(right)
        .map(|(left, right)| left * right)
        .sum()
}

/// Which embedding implementation fact recall uses. [`Hash`] is the offline
/// feature-hash default; [`LocalFastEmbed`] uses local ONNX Runtime via
/// FastEmbed (BGESmallENV15, 384 dimensions).
///
/// [`Hash`]: FactEmbeddingBackend::Hash
/// [`LocalFastEmbed`]: FactEmbeddingBackend::LocalFastEmbed
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FactEmbeddingBackend {
    /// The offline feature-hash [`embedding`].
    Hash,
    /// The local FastEmbed ONNX model (BGESmallENV15, 384 dimensions).
    LocalFastEmbed,
}

impl FactEmbeddingBackend {
    pub(crate) fn embed(&self, text: &str) -> Vec<f32> {
        match self {
            FactEmbeddingBackend::Hash => embedding(text),
            FactEmbeddingBackend::LocalFastEmbed => {
                crate::search::local_embedding::embed_query(text)
                    .unwrap_or_else(|_| embedding(text))
            }
        }
    }

    /// Vector length this backend produces. A stored blob whose decoded length
    /// differs is treated as stale (from a previous backend) and recomputed.
    pub(crate) fn dim(&self) -> usize {
        match self {
            FactEmbeddingBackend::Hash => EMBEDDING_DIMENSIONS,
            FactEmbeddingBackend::LocalFastEmbed => {
                crate::search::local_embedding::LOCAL_EMBEDDING_DIM
            }
        }
    }
}

/// The backend fact recall currently uses. Checks `MINT_FACT_EMBEDDING` env var
/// or falls back to `FactEmbeddingBackend::Hash`.
pub(crate) fn fact_embedding_backend() -> FactEmbeddingBackend {
    if std::env::var("MINT_FACT_EMBEDDING")
        .map(|v| v.eq_ignore_ascii_case("fastembed") || v.eq_ignore_ascii_case("local"))
        .unwrap_or(false)
    {
        FactEmbeddingBackend::LocalFastEmbed
    } else {
        FactEmbeddingBackend::Hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_backend_dim_and_parity_with_embedding() {
        let backend = FactEmbeddingBackend::Hash;
        assert_eq!(backend.dim(), EMBEDDING_DIMENSIONS);
        for sample in ["prefers TypeScript and tabs", "ผู้ใช้ชอบภาษาไทย", ""]
        {
            assert_eq!(backend.embed(sample), embedding(sample));
        }
    }

    #[test]
    fn local_fastembed_backend_dim() {
        let backend = FactEmbeddingBackend::LocalFastEmbed;
        assert_eq!(
            backend.dim(),
            crate::search::local_embedding::LOCAL_EMBEDDING_DIM
        );
    }

    #[test]
    fn related_text_scores_higher_than_unrelated() {
        let query = embedding("rust cargo backend ownership");
        let related = embedding("the rust backend uses cargo and ownership");
        let unrelated = embedding("pasta tomato basil kitchen recipe");
        assert!(cosine_similarity(&query, &related) > cosine_similarity(&query, &unrelated));
    }

    #[test]
    fn encode_decode_roundtrips() {
        let vector = embedding("roundtrip me");
        assert_eq!(
            decode_embedding(&encode_embedding(&vector)).unwrap(),
            vector
        );
        assert!(decode_embedding(&[1, 2, 3]).is_err());
    }
}

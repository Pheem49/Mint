use std::sync::{Arc, Mutex, RwLock};
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};

pub const LOCAL_EMBEDDING_DIM: usize = 384;

static ENGINE: RwLock<Option<Arc<Mutex<TextEmbedding>>>> = RwLock::new(None);

/// Returns a shared, thread-safe FastEmbed model instance.
/// Loads `BGESmallENV15` locally via ONNX Runtime on first call.
pub fn get_embedding_engine() -> Result<Arc<Mutex<TextEmbedding>>, String> {
    if let Ok(read_guard) = ENGINE.read() {
        if let Some(engine) = &*read_guard {
            return Ok(Arc::clone(engine));
        }
    }
    let mut write_guard = ENGINE
        .write()
        .map_err(|e| format!("FastEmbed lock poisoned: {e}"))?;
    if let Some(engine) = &*write_guard {
        return Ok(Arc::clone(engine));
    }
    let options = InitOptions::new(EmbeddingModel::BGESmallENV15);
    let model = TextEmbedding::try_new(options)
        .map_err(|e| format!("FastEmbed local model initialization failed: {e}"))?;
    let arc = Arc::new(Mutex::new(model));
    *write_guard = Some(Arc::clone(&arc));
    Ok(arc)
}

/// Checks whether the local FastEmbed engine is loaded or can be initialized.
pub fn is_available() -> bool {
    if let Ok(read_guard) = ENGINE.read() {
        if read_guard.is_some() {
            return true;
        }
    }
    get_embedding_engine().is_ok()
}

/// Computes dense embeddings for a slice of text strings using the local ONNX model.
pub fn embed_texts(texts: &[&str]) -> Result<Vec<Vec<f32>>, String> {
    if texts.is_empty() {
        return Ok(Vec::new());
    }
    let engine = get_embedding_engine()?;
    let text_vec: Vec<String> = texts.iter().map(|s| s.to_string()).collect();
    let mut guard = engine
        .lock()
        .map_err(|e| format!("FastEmbed lock poisoned: {e}"))?;
    guard
        .embed(text_vec, None)
        .map_err(|e| format!("FastEmbed embedding error: {e}"))
}

/// Computes dense embedding for a single query string.
pub fn embed_query(query: &str) -> Result<Vec<f32>, String> {
    let embeddings = embed_texts(&[query])?;
    embeddings
        .into_iter()
        .next()
        .ok_or_else(|| "Empty embedding returned for query".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_local_embedding_dim() {
        assert_eq!(LOCAL_EMBEDDING_DIM, 384);
    }

    #[test]
    fn test_embed_empty_texts_returns_empty() {
        let res = embed_texts(&[]);
        assert!(res.is_ok());
        assert!(res.unwrap().is_empty());
    }
}

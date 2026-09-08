//! Async model-list fetcher for video generation providers with in-memory caching.
//!
//! Mirrors `media::image_model_fetcher` and `slash::model_fetcher` but targets video-generation
//! models (specifically Google Veo via Gemini API).

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde::Deserialize;

const CACHE_TTL: Duration = Duration::from_secs(60 * 60); // 1 hour
const HTTP_TIMEOUT: Duration = Duration::from_secs(6);

struct CacheEntry {
    models: Vec<String>,
    fetched_at: Instant,
}

type Cache = Mutex<HashMap<String, CacheEntry>>;

fn global_video_cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(key: &str) -> Option<Vec<String>> {
    let guard = global_video_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.get(key).and_then(|entry| {
        if entry.fetched_at.elapsed() < CACHE_TTL {
            Some(entry.models.clone())
        } else {
            None
        }
    })
}

fn cache_set(key: &str, models: Vec<String>) {
    let mut guard = global_video_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(
        key.to_string(),
        CacheEntry {
            models,
            fetched_at: Instant::now(),
        },
    );
}

/// Invalidate video model cache for a given provider.
pub fn invalidate_video_cache(provider: &str) {
    let mut guard = global_video_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.retain(|key, _| !key.starts_with(&format!("{provider}:")));
}

/// Fetch available video generation models for `provider` using its API.
/// Returns an empty `Vec` on any error or when the provider has no public models API.
pub async fn fetch_video_provider_models(provider: &str, api_key: &str) -> Vec<String> {
    let lower = provider.trim().to_lowercase();
    let canonical = match lower.as_str() {
        "gemini" | "google" => "veo",
        other => other,
    };

    if api_key.trim().is_empty() {
        return Vec::new();
    }

    let key_ident: String = api_key.chars().take(8).collect();
    let cache_key = format!("{canonical}:{key_ident}");

    if let Some(cached) = cache_get(&cache_key) {
        return cached;
    }

    let models = match canonical {
        "veo" => fetch_gemini_video_models(api_key).await,
        _ => Ok(Vec::new()),
    }
    .unwrap_or_default();

    if !models.is_empty() {
        cache_set(&cache_key, models.clone());
    }

    models
}

// ---------------------------------------------------------------------------
// Per-provider fetch helpers
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct GeminiModelsResponse {
    models: Vec<GeminiModel>,
}

#[derive(Deserialize)]
struct GeminiModel {
    name: String,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

async fn fetch_gemini_video_models(api_key: &str) -> Result<Vec<String>, ()> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}&pageSize=100"
    );
    let resp = reqwest_get(&url, None).await?;
    let parsed: GeminiModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .models
        .into_iter()
        .filter_map(|m| {
            let id = m.name.strip_prefix("models/").unwrap_or(&m.name).to_string();
            let lower = id.to_lowercase();

            let is_video_method = m
                .supported_generation_methods
                .as_ref()
                .map(|methods| {
                    methods
                        .iter()
                        .any(|meth| meth == "predictLongRunning" || meth.contains("video"))
                })
                .unwrap_or(false);

            let is_video_name = lower.contains("veo") || lower.contains("video");

            if (is_video_method || is_video_name)
                && !lower.contains("embedding")
                && !lower.contains("tts")
            {
                Some(id)
            } else {
                None
            }
        })
        .collect();

    Ok(models)
}

// ---------------------------------------------------------------------------
// HTTP helper
// ---------------------------------------------------------------------------

async fn reqwest_get(url: &str, extra_headers: Option<&str>) -> Result<String, ()> {
    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new());
    let mut req = client.get(url).header("User-Agent", "mint-cli");

    if let Some(headers_str) = extra_headers {
        for line in headers_str.lines() {
            if let Some((name, value)) = line.split_once(':') {
                req = req.header(name.trim(), value.trim());
            }
        }
    }

    let resp = req.send().await.map_err(|_| ())?;
    if !resp.status().is_success() {
        return Err(());
    }
    resp.text().await.map_err(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_store_and_retrieve_video() {
        let key = "veo:key123";
        let list = vec![
            "veo-3.1-generate-preview".to_string(),
            "veo-3.1-fast-generate-preview".to_string(),
        ];
        cache_set(key, list.clone());

        let cached = cache_get(key);
        assert_eq!(cached, Some(list));
    }

    #[test]
    fn cache_invalidation_works_video() {
        let key = "veo:key123";
        cache_set(key, vec!["veo-3.1-generate-preview".to_string()]);
        assert!(cache_get(key).is_some());

        invalidate_video_cache("veo");
        assert!(cache_get(key).is_none());
    }

    #[tokio::test]
    async fn fetch_empty_key_returns_empty_immediately_video() {
        let models = fetch_video_provider_models("veo", "").await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn fetch_unknown_provider_returns_empty_video() {
        let models = fetch_video_provider_models("unknown_provider", "testkey").await;
        assert!(models.is_empty());
    }
}

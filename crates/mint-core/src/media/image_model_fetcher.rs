//! Async model-list fetcher for image generation providers with in-memory caching.
//!
//! Mirrors `slash::model_fetcher` but targets image-generation models (e.g. Gemini/Imagen,
//! OpenAI DALL·E, Replicate text-to-image collections).

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

fn global_image_cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(key: &str) -> Option<Vec<String>> {
    let guard = global_image_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.get(key).and_then(|entry| {
        if entry.fetched_at.elapsed() < CACHE_TTL {
            Some(entry.models.clone())
        } else {
            None
        }
    })
}

fn cache_set(key: &str, models: Vec<String>) {
    let mut guard = global_image_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(
        key.to_string(),
        CacheEntry {
            models,
            fetched_at: Instant::now(),
        },
    );
}

/// Invalidate image model cache for a given provider.
pub fn invalidate_image_cache(provider: &str) {
    let mut guard = global_image_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.retain(|key, _| !key.starts_with(&format!("{provider}:")));
}

/// Fetch available image generation models for `provider` using its API.
/// Returns an empty `Vec` on any error or when the provider has no public models API.
pub async fn fetch_image_provider_models(provider: &str, api_key: &str) -> Vec<String> {
    let lower = provider.trim().to_lowercase();
    let canonical = match lower.as_str() {
        "gemini" | "google" => "nanobanana",
        "flux" => "bfl",
        "openai" => "dalle",
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
        "nanobanana" => fetch_gemini_image_models(api_key).await,
        "dalle" => fetch_dalle_image_models(api_key).await,
        "replicate" => fetch_replicate_image_models(api_key).await,
        // Stability, Ideogram, and BFL do not provide generic models listing endpoints;
        // fallback to static presets in image_models.rs.
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

async fn fetch_gemini_image_models(api_key: &str) -> Result<Vec<String>, ()> {
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

            let is_image_method = m
                .supported_generation_methods
                .as_ref()
                .map(|methods| {
                    methods
                        .iter()
                        .any(|meth| meth == "generateImages" || meth.contains("image"))
                })
                .unwrap_or(false);

            let is_image_name = lower.contains("imagen") || lower.contains("image");

            if (is_image_method || is_image_name)
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

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

async fn fetch_dalle_image_models(api_key: &str) -> Result<Vec<String>, ()> {
    let url = "https://api.openai.com/v1/models";
    let auth = format!("Authorization: Bearer {api_key}");
    let resp = reqwest_get(url, Some(&auth)).await?;
    let parsed: OpenAiModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| {
            let lower = id.to_lowercase();
            lower.contains("dall-e") || lower.contains("gpt-image")
        })
        .collect();

    Ok(models)
}

#[derive(Deserialize)]
struct ReplicateCollectionResponse {
    models: Option<Vec<ReplicateModelEntry>>,
}

#[derive(Deserialize)]
struct ReplicateModelEntry {
    owner: String,
    name: String,
}

async fn fetch_replicate_image_models(api_key: &str) -> Result<Vec<String>, ()> {
    let url = "https://api.replicate.com/v1/collections/text-to-image";
    let auth = format!("Authorization: Bearer {api_key}");
    let resp = reqwest_get(url, Some(&auth)).await?;
    let parsed: ReplicateCollectionResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .models
        .unwrap_or_default()
        .into_iter()
        .map(|m| format!("{}/{}", m.owner, m.name))
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
    fn cache_store_and_retrieve_image() {
        let key = "nanobanana:key123";
        let list = vec!["imagen-3.0-fast".to_string(), "imagen-3.0".to_string()];
        cache_set(key, list.clone());

        let cached = cache_get(key);
        assert_eq!(cached, Some(list));
    }

    #[test]
    fn cache_invalidation_works_image() {
        let key = "dalle:key123";
        cache_set(key, vec!["dall-e-3".to_string()]);
        assert!(cache_get(key).is_some());

        invalidate_image_cache("dalle");
        assert!(cache_get(key).is_none());
    }

    #[tokio::test]
    async fn fetch_empty_key_returns_empty_immediately_image() {
        let models = fetch_image_provider_models("nanobanana", "").await;
        assert!(models.is_empty());

        let models = fetch_image_provider_models("dalle", "").await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn fetch_unreachable_endpoint_returns_empty_without_panicking_image() {
        // Calling non-API provider returns empty immediately
        let models = fetch_image_provider_models("ideogram", "testkey").await;
        assert!(models.is_empty());
    }
}

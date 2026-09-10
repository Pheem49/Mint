//! Async model-list fetcher with a short-lived in-memory cache.
//!
//! `fetch_provider_models` is the single public entry point. It:
//!   1. Returns the cached list if it was populated less than [`CACHE_TTL`] ago.
//!   2. Otherwise calls the provider's `/models` (or equivalent) endpoint.
//!   3. Falls back silently to an empty `Vec` on any error so callers always
//!      receive a `Vec<String>` and decide themselves whether to fall back to
//!      the static presets in [`crate::slash::models`].
//!
//! The cache is process-wide (one `OnceLock<Mutex<HashMap>>`) — cheap enough
//! for the handful of providers Mint supports.

use std::{
    collections::HashMap,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

use serde::Deserialize;

// ---------------------------------------------------------------------------
// Cache
// ---------------------------------------------------------------------------

const CACHE_TTL: Duration = Duration::from_secs(60 * 60); // 1 hour

struct CacheEntry {
    models: Vec<String>,
    fetched_at: Instant,
}

type Cache = Mutex<HashMap<String, CacheEntry>>;

fn global_cache() -> &'static Cache {
    static CACHE: OnceLock<Cache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cache_get(key: &str) -> Option<Vec<String>> {
    let guard = global_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.get(key).and_then(|entry| {
        if entry.fetched_at.elapsed() < CACHE_TTL {
            Some(entry.models.clone())
        } else {
            None
        }
    })
}

fn cache_set(key: &str, models: Vec<String>) {
    let mut guard = global_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.insert(
        key.to_string(),
        CacheEntry {
            models,
            fetched_at: Instant::now(),
        },
    );
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Fetch the list of available models for `provider` using its API.
///
/// Returns `Vec::new()` on any error (network, parse, missing key).
/// The caller is responsible for falling back to static presets.
///
/// `api_key` — the user's key for this provider (may be empty; returns empty
///   list so the static preset fallback kicks in).
/// `base_url` — optional override for the provider base URL; used by
///   `local_openai` and `custom:*` providers.
pub async fn fetch_provider_models(
    provider: &str,
    api_key: &str,
    base_url: Option<&str>,
) -> Vec<String> {
    // Don't even try when no key is available (except local/ollama).
    let key_required = !matches!(provider, "ollama" | "local_openai");
    if key_required && api_key.is_empty() {
        return Vec::new();
    }

    let key_ident: String = api_key.chars().take(8).collect();
    let cache_key = if let Some(url) = base_url.filter(|s| !s.is_empty()) {
        format!("{provider}:{key_ident}:{url}")
    } else {
        format!("{provider}:{key_ident}")
    };
    if let Some(cached) = cache_get(&cache_key) {
        return cached;
    }

    let models = match provider {
        "gemini"     => fetch_gemini_models(api_key).await,
        "anthropic"  => fetch_anthropic_models(api_key).await,
        "openai"     => fetch_openai_models(api_key, "https://api.openai.com").await,
        "openrouter" => fetch_openrouter_models(api_key).await,
        "deepseek"   => fetch_openai_models(api_key, "https://api.deepseek.com").await,
        "local_openai" => {
            let url = base_url
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .unwrap_or("http://127.0.0.1:1234");
            fetch_openai_models(api_key, url).await
        }
        // HuggingFace has tens of thousands of models; keep static presets.
        // Ollama is handled separately via `installed_ollama_models()`.
        _ => Ok(Vec::new()),
    }
    .unwrap_or_default();

    if !models.is_empty() {
        cache_set(&cache_key, models.clone());
    }

    models
}

/// Invalidate the cache entries for `provider` (e.g. when the user changes
/// their API key so the next picker call gets a fresh list).
pub fn invalidate_cache(provider: &str) {
    let mut guard = global_cache().lock().unwrap_or_else(|e| e.into_inner());
    guard.retain(|key, _| !key.starts_with(&format!("{}:", provider)));
}

// ---------------------------------------------------------------------------
// Per-provider fetch helpers
// ---------------------------------------------------------------------------

// -- Gemini ------------------------------------------------------------------

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

async fn fetch_gemini_models(api_key: &str) -> Result<Vec<String>, ()> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}&pageSize=100"
    );
    let resp = reqwest_get(&url, None).await?;
    let parsed: GeminiModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .models
        .into_iter()
        .filter(|m| {
            // Only keep models that support generateContent (text chat)
            m.supported_generation_methods
                .as_ref()
                .map(|methods| methods.iter().any(|meth| meth == "generateContent"))
                .unwrap_or(false)
        })
        .filter_map(|m| {
            // Strip "models/" prefix → "gemini-2.5-flash"
            let id = m.name.strip_prefix("models/").unwrap_or(&m.name).to_string();
            // Exclude embedding, TTS, image, and live-only models
            let lower = id.to_lowercase();
            if lower.contains("embedding")
                || lower.contains("tts")
                || lower.contains("aqa")
                || lower.contains("image")
                || lower.contains("live")
            {
                None
            } else {
                Some(id)
            }
        })
        .collect();

    Ok(models)
}

// -- Gemini Live (BidiGenerateContent / native-audio) -----------------------

/// Fetch the list of Gemini models that support the Live / BidiGenerateContent
/// API (used by the realtime voice feature). Returns `Vec::new()` on any error
/// or when `api_key` is empty — callers fall back to the static preset list.
///
/// Uses a separate cache key (`gemini_live:…`) so it doesn't collide with
/// the regular `gemini:…` text-chat model cache.
pub async fn fetch_gemini_live_models(api_key: &str) -> Vec<String> {
    if api_key.is_empty() {
        return Vec::new();
    }

    let key_ident: String = api_key.chars().take(8).collect();
    let cache_key = format!("gemini_live:{key_ident}");
    if let Some(cached) = cache_get(&cache_key) {
        return cached;
    }

    let models = fetch_gemini_live_models_inner(api_key)
        .await
        .unwrap_or_default();

    if !models.is_empty() {
        cache_set(&cache_key, models.clone());
    }

    models
}

#[derive(Deserialize)]
struct GeminiLiveModelsResponse {
    models: Vec<GeminiLiveModel>,
}

#[derive(Deserialize)]
struct GeminiLiveModel {
    name: String,
    #[serde(rename = "supportedGenerationMethods")]
    supported_generation_methods: Option<Vec<String>>,
}

async fn fetch_gemini_live_models_inner(api_key: &str) -> Result<Vec<String>, ()> {
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}&pageSize=200"
    );
    let resp = reqwest_get(&url, None).await?;
    let parsed: GeminiLiveModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .models
        .into_iter()
        .filter(|m| {
            // Keep models that explicitly support BidiGenerateContent (the Live API),
            // or whose name contains live/native-audio heuristics for older API responses
            // that may not yet advertise the BidiGenerateContent method.
            let methods = m.supported_generation_methods.as_deref().unwrap_or(&[]);
            let supports_bidi = methods.iter().any(|meth| meth == "BidiGenerateContent");
            let name_lower = m.name.to_lowercase();
            let heuristic = name_lower.contains("native-audio") || name_lower.contains("-live-");
            supports_bidi || heuristic
        })
        .filter_map(|m| {
            // Strip "models/" prefix → e.g. "gemini-2.5-flash-native-audio-preview-12-2025"
            let id = m.name.strip_prefix("models/").unwrap_or(&m.name).to_string();
            // Exclude TTS-only models (they're not BidiGenerateContent even if
            // they mention audio) and embedding models.
            let lower = id.to_lowercase();
            if lower.contains("tts") || lower.contains("embedding") {
                None
            } else {
                Some(id)
            }
        })
        .collect();

    Ok(models)
}

// -- Anthropic ---------------------------------------------------------------

#[derive(Deserialize)]
struct AnthropicModelsResponse {
    data: Vec<AnthropicModel>,
}

#[derive(Deserialize)]
struct AnthropicModel {
    id: String,
}

async fn fetch_anthropic_models(api_key: &str) -> Result<Vec<String>, ()> {
    let url = "https://api.anthropic.com/v1/models?limit=100";
    let extra = format!("x-api-key: {api_key}\nanthropic-version: 2023-06-01");
    let resp = reqwest_get(url, Some(&extra)).await?;
    let parsed: AnthropicModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| id.starts_with("claude-"))
        .collect();

    Ok(models)
}

// -- OpenAI-compatible (OpenAI, DeepSeek, local_openai) ----------------------

#[derive(Deserialize)]
struct OpenAiModelsResponse {
    data: Vec<OpenAiModel>,
}

#[derive(Deserialize)]
struct OpenAiModel {
    id: String,
}

async fn fetch_openai_models(api_key: &str, base_url: &str) -> Result<Vec<String>, ()> {
    let clean_base = base_url.trim_end_matches('/');
    let url = if clean_base.ends_with("/v1") {
        format!("{clean_base}/models")
    } else {
        format!("{clean_base}/v1/models")
    };
    let auth = if api_key.is_empty() {
        None
    } else {
        Some(format!("Authorization: Bearer {api_key}"))
    };
    let resp = reqwest_get(&url, auth.as_deref()).await?;
    let parsed: OpenAiModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let models = parsed
        .data
        .into_iter()
        .map(|m| m.id)
        .filter(|id| {
            let lower = id.to_lowercase();
            !lower.contains("embedding")
                && !lower.contains("tts")
                && !lower.contains("dall-e")
                && !lower.contains("whisper")
                && !lower.contains("moderation")
                && !lower.contains("search")
                && !lower.contains("davinci")
                && !lower.contains("babbage")
                && !lower.contains("ada")
                && !lower.contains("curie")
        })
        .collect();

    Ok(models)
}

// -- OpenRouter --------------------------------------------------------------

#[derive(Deserialize)]
struct OpenRouterModelsResponse {
    data: Vec<OpenRouterModel>,
}

#[derive(Deserialize)]
struct OpenRouterModel {
    id: String,
}

async fn fetch_openrouter_models(api_key: &str) -> Result<Vec<String>, ()> {
    let url = "https://openrouter.ai/api/v1/models";
    let auth = format!("Authorization: Bearer {api_key}");
    let resp = reqwest_get(url, Some(&auth)).await?;
    let parsed: OpenRouterModelsResponse = serde_json::from_str(&resp).map_err(|_| ())?;

    let mut models: Vec<String> = parsed.data.into_iter().map(|m| m.id).collect();
    models.sort();
    Ok(models)
}

// ---------------------------------------------------------------------------
// HTTP helper
// ---------------------------------------------------------------------------

const HTTP_TIMEOUT: Duration = Duration::from_secs(6);

/// Minimal GET with optional extra headers (newline-separated `Key: Value` pairs).
/// Returns the response body as `String`, or `Err(())` on any failure.
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
    fn cache_store_and_retrieve() {
        let key = "test_prov:key123";
        let list = vec!["model-a".to_string(), "model-b".to_string()];
        cache_set(key, list.clone());

        let cached = cache_get(key);
        assert_eq!(cached, Some(list));
    }

    #[test]
    fn cache_invalidation_works() {
        let key = "test_inv:key123";
        cache_set(key, vec!["model-x".to_string()]);
        assert!(cache_get(key).is_some());

        invalidate_cache("test_inv");
        assert!(cache_get(key).is_none());
    }

    #[tokio::test]
    async fn fetch_empty_key_returns_empty_immediately() {
        let models = fetch_provider_models("gemini", "", None).await;
        assert!(models.is_empty());

        let models = fetch_provider_models("anthropic", "", None).await;
        assert!(models.is_empty());
    }

    #[tokio::test]
    async fn fetch_serves_from_cache_on_subsequent_calls() {
        let key = "gemini:cachetes";
        cache_set(key, vec!["cached-gemini".to_string()]);

        let models = fetch_provider_models("gemini", "cachetestkey", None).await;
        assert_eq!(models, vec!["cached-gemini".to_string()]);

        invalidate_cache("gemini");
    }

    #[tokio::test]
    async fn fetch_unreachable_endpoint_returns_empty_without_panicking() {
        // Points to an unused port on localhost that should immediately refuse or time out
        let models = fetch_provider_models("local_openai", "", Some("http://127.0.0.1:59999")).await;
        assert!(models.is_empty());
    }
}

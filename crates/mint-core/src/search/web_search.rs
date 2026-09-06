use futures_util::{StreamExt, future::join_all};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::MintConfig;

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error(
        "no web search provider configured (set googleSearchApiKey, braveSearchApiKey, or searxngBaseUrl)"
    )]
    NoApiKey,
    #[error("web search request failed: {0}")]
    Request(String),
    #[error("web search response was empty or unparseable")]
    EmptyResponse,
}

fn sanitize_reqwest_error(err: reqwest::Error) -> WebSearchError {
    let mut msg = err.to_string();
    if let Some(pos) = msg.find("https://www.googleapis.com") {
        let mut end_pos = msg.len();
        for (idx, ch) in msg[pos..].char_indices() {
            if ch == ' '
                || ch == ')'
                || ch == '"'
                || ch == '\''
                || ch == ']'
                || ch == '}'
                || ch == '>'
            {
                end_pos = pos + idx;
                break;
            }
        }
        msg.replace_range(pos..end_pos, "https://www.googleapis.com/customsearch/v1");
    }
    if let Some(pos) = msg.find("https://api.search.brave.com") {
        let mut end_pos = msg.len();
        for (idx, ch) in msg[pos..].char_indices() {
            if ch == ' '
                || ch == ')'
                || ch == '"'
                || ch == '\''
                || ch == ']'
                || ch == '}'
                || ch == '>'
            {
                end_pos = pos + idx;
                break;
            }
        }
        msg.replace_range(
            pos..end_pos,
            "https://api.search.brave.com/res/v1/web/search",
        );
    }
    WebSearchError::Request(msg)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
    /// Thumbnail image URL, if the search provider returned one.
    pub image_url: Option<String>,
}

/// Attempt to extract an Open Graph or Twitter Card image URL from a web page.
/// Reads at most ~8 KB of the response — enough to cover the <head> section.
/// Returns `None` on any network/parse error or if no og:image/twitter:image is found.
async fn og_image_fallback(url: &str) -> Option<String> {
    tokio::time::timeout(std::time::Duration::from_secs(4), async move {
        let client = crate::HTTP_CLIENT.clone();
        let resp = client
            .get(url)
            .header(
                "User-Agent",
                "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36",
            )
            .send()
            .await
            .ok()?;

        let mut stream = resp.bytes_stream();
        let mut buf = Vec::with_capacity(65536);
        while let Some(chunk) = stream.next().await {
            if let Ok(bytes) = chunk {
                buf.extend_from_slice(&bytes);
                if buf.len() >= 65536 {
                    break;
                }
            }
        }
        let html = String::from_utf8_lossy(&buf);

        // Scan for og:image, twitter:image, thumbnail, or link image_src meta tags
        // Handles both attribute orders (property before content and content before property)
        // as well as single and double quotes.
        static OG_META_RE: std::sync::LazyLock<Vec<regex::Regex>> = std::sync::LazyLock::new(|| {
            vec![
                regex::Regex::new(
                    r#"(?i)<meta\s+[^>]*?(?:property|name|itemprop)=["'](?:og:image|og:image:url|twitter:image|thumbnail|image)["'][^>]*?content=["']([^"']+)["']"#
                ).unwrap(),
                regex::Regex::new(
                    r#"(?i)<meta\s+[^>]*?content=["']([^"']+)["'][^>]*?(?:property|name|itemprop)=["'](?:og:image|og:image:url|twitter:image|thumbnail|image)["']"#
                ).unwrap(),
                regex::Regex::new(
                    r#"(?i)<link\s+[^>]*?rel=["'](?:image_src|apple-touch-icon)["'][^>]*?href=["']([^"']+)["']"#
                ).unwrap(),
            ]
        });

        for re in OG_META_RE.iter() {
            if let Some(caps) = re.captures(&html) {
                if let Some(img_match) = caps.get(1) {
                    let img_url = img_match.as_str().trim().to_owned();
                    if img_url.starts_with("http://") || img_url.starts_with("https://") {
                        return Some(img_url);
                    } else if img_url.starts_with("//") {
                        return Some(format!("https:{img_url}"));
                    } else if img_url.starts_with('/') {
                        if let Ok(base) = reqwest::Url::parse(url) {
                            if let Ok(joined) = base.join(&img_url) {
                                return Some(joined.to_string());
                            }
                        }
                    }
                }
            }
        }
        None


    })
    .await
    .ok()
    .flatten()
}

/// Fill in missing `image_url` fields by scraping Open Graph tags in parallel.
/// Only the first `max_fetch` hits missing an image are fetched (to limit latency).
async fn enrich_with_og_images(hits: Vec<SearchHit>, max_fetch: usize) -> Vec<SearchHit> {
    let futures: Vec<_> = hits
        .iter()
        .enumerate()
        .map(|(i, h)| {
            let url = h.url.clone();
            let existing = h.image_url.clone();
            async move {
                if existing.is_some() || i >= max_fetch {
                    existing
                } else {
                    og_image_fallback(&url).await
                }
            }
        })
        .collect();

    let resolved =
        match tokio::time::timeout(std::time::Duration::from_secs(30), join_all(futures)).await {
            Ok(res) => res,
            Err(_) => vec![None; hits.len()],
        };

    hits.into_iter()
        .enumerate()
        .map(|(i, mut h)| {
            if i < resolved.len() {
                h.image_url = resolved[i].clone();
            }
            h
        })
        .collect()
}

/// Search the web using the configured provider. The user's `searchProvider`
/// selection (from Settings) is tried first; if it's unconfigured or returns
/// no results, the remaining providers are tried in the default order
/// (Google → Brave → SearXNG) as a fallback.
///
/// Returns the search hits and the name of the provider used.
pub async fn search(
    query: &str,
    limit: usize,
    config: &MintConfig,
) -> Result<(Vec<SearchHit>, String), WebSearchError> {
    let google_key = config
        .extra
        .get("googleSearchApiKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_owned();
    let google_cx = config
        .extra
        .get("googleSearchCx")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_owned();
    let brave_key = config
        .extra
        .get("braveSearchApiKey")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .to_owned();
    let searxng_base_url = config
        .extra
        .get("searxngBaseUrl")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim()
        .trim_end_matches('/')
        .to_owned();

    let selected = config
        .extra
        .get("searchProvider")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_owned();

    let mut order = ["google", "brave", "searxng"];
    if let Some(pos) = order.iter().position(|p| *p == selected) {
        order.swap(0, pos);
    }

    let mut last_err = None;

    for provider in order {
        let result = match provider {
            "google" if !google_key.is_empty() && !google_cx.is_empty() => {
                google_search(query, limit, &google_key, &google_cx).await
            }
            "brave" if !brave_key.is_empty() => brave_search(query, limit, &brave_key).await,
            "searxng" if !searxng_base_url.is_empty() => {
                searxng_search(query, limit, &searxng_base_url).await
            }
            _ => continue,
        };

        match result {
            Ok(hits) if !hits.is_empty() => {
                // Enrich up to 4 hits with OG image fallback in parallel
                let enriched = enrich_with_og_images(hits, 4).await;
                let name = match provider {
                    "google" => "Google",
                    "brave" => "Brave",
                    _ => "SearXNG",
                };
                return Ok((enriched, name.to_owned()));
            }
            Ok(_) => {}
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or(WebSearchError::NoApiKey))
}

async fn google_search(
    query: &str,
    limit: usize,
    api_key: &str,
    cx: &str,
) -> Result<Vec<SearchHit>, WebSearchError> {
    let client = crate::HTTP_CLIENT.clone();
    let num_str = limit.min(10).to_string();
    let response: serde_json::Value = client
        .get("https://www.googleapis.com/customsearch/v1")
        .query(&[
            ("key", api_key),
            ("cx", cx),
            ("q", query),
            ("num", num_str.as_str()),
        ])
        .send()
        .await
        .map_err(sanitize_reqwest_error)?
        .error_for_status()
        .map_err(sanitize_reqwest_error)?
        .json()
        .await
        .map_err(sanitize_reqwest_error)?;

    let items = response["items"]
        .as_array()
        .ok_or(WebSearchError::EmptyResponse)?;

    Ok(items
        .iter()
        .take(limit)
        .filter_map(|item| {
            // Try to get a representative image from pagemap.cse_image[0].src
            let image_url = item["pagemap"]["cse_image"]
                .as_array()
                .and_then(|arr| arr.first())
                .and_then(|img| img["src"].as_str())
                .map(|s| s.to_owned());
            Some(SearchHit {
                title: item["title"].as_str()?.to_owned(),
                url: item["link"].as_str()?.to_owned(),
                snippet: item["snippet"].as_str().unwrap_or("").to_owned(),
                image_url,
            })
        })
        .collect())
}

async fn brave_search(
    query: &str,
    limit: usize,
    api_key: &str,
) -> Result<Vec<SearchHit>, WebSearchError> {
    let client = crate::HTTP_CLIENT.clone();
    let response: serde_json::Value = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", &limit.to_string())])
        .send()
        .await
        .map_err(sanitize_reqwest_error)?
        .error_for_status()
        .map_err(sanitize_reqwest_error)?
        .json()
        .await
        .map_err(sanitize_reqwest_error)?;

    let results = response["web"]["results"]
        .as_array()
        .ok_or(WebSearchError::EmptyResponse)?;

    Ok(results
        .iter()
        .take(limit)
        .filter_map(|item| {
            // Brave includes thumbnail.original (direct URL) and thumbnail.src (proxy URL)
            let image_url = item["thumbnail"]["original"]
                .as_str()
                .filter(|s| !s.is_empty())
                .map(|s| s.replace("&amp;", "&"))
                .or_else(|| {
                    item["thumbnail"]["src"]
                        .as_str()
                        .and_then(unwrap_brave_proxy_url)
                });
            Some(SearchHit {
                title: item["title"].as_str()?.to_owned(),
                url: item["url"].as_str()?.to_owned(),
                snippet: item["description"].as_str().unwrap_or("").to_owned(),
                image_url,
            })
        })
        .collect())
}

/// Query a self-hosted SearXNG instance. `base_url` should point at the
/// instance root (e.g. `https://searx.example.com`), without a trailing
/// `/search`. The instance must have the `json` output format enabled
/// (`search.formats` in `settings.yml`), which is disabled by default.
async fn searxng_search(
    query: &str,
    limit: usize,
    base_url: &str,
) -> Result<Vec<SearchHit>, WebSearchError> {
    let client = crate::HTTP_CLIENT.clone();
    let url = format!("{base_url}/search");
    let response: serde_json::Value = client
        .get(&url)
        .header("Accept", "application/json")
        .query(&[("q", query), ("format", "json")])
        .send()
        .await
        .map_err(sanitize_reqwest_error)?
        .error_for_status()
        .map_err(sanitize_reqwest_error)?
        .json()
        .await
        .map_err(sanitize_reqwest_error)?;

    let results = response["results"]
        .as_array()
        .ok_or(WebSearchError::EmptyResponse)?;

    Ok(results
        .iter()
        .take(limit)
        .filter_map(|item| {
            // img_src is usually empty for general-category results; the
            // engine-provided thumbnail is the more reliable field.
            let image_url = item["img_src"]
                .as_str()
                .filter(|s| !s.is_empty())
                .or_else(|| item["thumbnail"].as_str().filter(|s| !s.is_empty()))
                .map(|s| s.to_owned());
            Some(SearchHit {
                title: item["title"].as_str()?.to_owned(),
                url: item["url"].as_str()?.to_owned(),
                snippet: item["content"].as_str().unwrap_or("").to_owned(),
                image_url,
            })
        })
        .collect())
}

/// Unwraps Brave Search internal image proxy URLs (`imgs.search.brave.com`)
/// back to the direct source image URL so it can be loaded without 403 Forbidden.
pub(crate) fn unwrap_brave_proxy_url(url: &str) -> Option<String> {
    if !url.contains("imgs.search.brave.com") {
        return Some(url.to_owned());
    }
    let parts: Vec<&str> = url.split("imgs.search.brave.com/").collect();
    if parts.len() < 2 {
        return None;
    }
    let rest = parts[1];
    let segments: Vec<&str> = rest.splitn(4, '/').collect();
    if segments.len() < 4 {
        return None;
    }
    let b64_raw = segments[3].replace('/', "");
    let mut b64_clean = b64_raw.replace('-', "+").replace('_', "/");
    let pad = (4 - (b64_clean.len() % 4)) % 4;
    if pad > 0 {
        b64_clean.push_str(&"=".repeat(pad));
    }

    use base64::Engine;
    let decoded_bytes = base64::engine::general_purpose::STANDARD
        .decode(&b64_clean)
        .ok()?;
    let decoded_str = String::from_utf8(decoded_bytes).ok()?;
    let unescaped = decoded_str.replace("&amp;", "&");
    if unescaped.starts_with("http://") || unescaped.starts_with("https://") {
        Some(unescaped)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unwrap_brave_proxy_url() {
        let proxy_url = "https://imgs.search.brave.com/iiiU8DBa1f4RL6KkBtVSskc8YKDOrpvlttO7uM89tc/rs:fit:200:200:1:0/g:ce/aHR0cHM6Ly9pbWFnZXMuY3RmYXNzZXRzLm5ldC9rZnR6d2R5YXV3dDkvNmliQ2Fxc29PN0Y2WENOQ0lPOHphWi85NDc1Y2RlYmMxZjBhZjEwMDQxNGYxYzg1ODYwZmM0Yy9IZXJvXzE2eDkucG5nP3c9MTYwMCZhbXA7aD05MDAmYW1wO2ZpdD1maWxs";
        let unwrapped = unwrap_brave_proxy_url(proxy_url);
        assert_eq!(
            unwrapped.as_deref(),
            Some(
                "https://images.ctfassets.net/kftzwdyauwt9/6ibCaqsoO7F6XCNCIO8zaZ/9475cdebc1f0af100414f1c85860fc4c/Hero_16x9.png?w=1600&h=900&fit=fill"
            )
        );
    }
}

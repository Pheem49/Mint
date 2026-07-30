use futures_util::{future::join_all, StreamExt};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::MintConfig;

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error("no web search API key configured (set googleSearchApiKey or braveSearchApiKey)")]
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
    let client = crate::HTTP_CLIENT.clone();
    let resp = tokio::time::timeout(
        std::time::Duration::from_secs(4),
        client
            .get(url)
            .header("User-Agent", "Mozilla/5.0 (compatible; MintBot/1.0)")
            .send(),
    )
    .await
    .ok()?
    .ok()?;

    // Read only enough bytes to cover the HTML <head> — avoids downloading full pages
    let mut stream = resp.bytes_stream();
    let mut buf = Vec::with_capacity(8192);
    while let Some(chunk) = stream.next().await {
        if let Ok(bytes) = chunk {
            buf.extend_from_slice(&bytes);
            if buf.len() >= 8192 {
                break;
            }
        }
    }
    let html = String::from_utf8_lossy(&buf);

    // Scan for og:image or twitter:image meta tags
    // Handles both: <meta property="og:image" content="URL"> and
    //               <meta name="twitter:image" content="URL">
    for prefix in &["og:image", "twitter:image"] {
        if let Some(pos) = html.find(prefix) {
            let after = &html[pos..];
            if let Some(c_pos) = after.find("content=") {
                let rest = &after[c_pos + 8..];
                if let Some(quote) = rest.chars().next() {
                    if quote == '"' || quote == '\'' {
                        let inner = &rest[1..];
                        if let Some(end) = inner.find(quote) {
                            let img_url = inner[..end].trim().to_owned();
                            if img_url.starts_with("http") {
                                return Some(img_url);
                            }
                        }
                    }
                }
            }
        }
    }
    None
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

    let resolved = join_all(futures).await;

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

/// Search the web using the first configured provider (Google → Brave).
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

    let mut last_err = None;

    if !google_key.is_empty() && !google_cx.is_empty() {
        match google_search(query, limit, &google_key, &google_cx).await {
            Ok(hits) => {
                if !hits.is_empty() {
                    // Enrich up to 4 hits with OG image fallback in parallel
                    let enriched = enrich_with_og_images(hits, 4).await;
                    return Ok((enriched, "Google".to_owned()));
                }
            }
            Err(e) => {
                last_err = Some(e);
            }
        }
    }

    if !brave_key.is_empty() {
        match brave_search(query, limit, &brave_key).await {
            Ok(hits) => {
                let enriched = enrich_with_og_images(hits, 4).await;
                return Ok((enriched, "Brave".to_owned()));
            }
            Err(e) => {
                last_err = Some(e);
            }
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
            // Brave includes thumbnail.src for most results
            let image_url = item["thumbnail"]["src"]
                .as_str()
                .map(|s| s.to_owned());
            Some(SearchHit {
                title: item["title"].as_str()?.to_owned(),
                url: item["url"].as_str()?.to_owned(),
                snippet: item["description"].as_str().unwrap_or("").to_owned(),
                image_url,
            })
        })
        .collect())
}

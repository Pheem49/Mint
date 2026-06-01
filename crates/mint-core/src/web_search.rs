use reqwest::Client;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::MintConfig;

#[derive(Debug, Error)]
pub enum WebSearchError {
    #[error("no web search API key configured (set googleSearchApiKey or braveSearchApiKey)")]
    NoApiKey,
    #[error("web search request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("web search response was empty or unparseable")]
    EmptyResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHit {
    pub title: String,
    pub url: String,
    pub snippet: String,
}

/// Search the web using the first configured provider (Google → Brave).
pub async fn search(
    query: &str,
    limit: usize,
    config: &MintConfig,
) -> Result<Vec<SearchHit>, WebSearchError> {
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

    if !google_key.is_empty() && !google_cx.is_empty() {
        if let Ok(hits) = google_search(query, limit, &google_key, &google_cx).await {
            if !hits.is_empty() {
                return Ok(hits);
            }
        }
    }

    if !brave_key.is_empty() {
        return brave_search(query, limit, &brave_key).await;
    }

    Err(WebSearchError::NoApiKey)
}

async fn google_search(
    query: &str,
    limit: usize,
    api_key: &str,
    cx: &str,
) -> Result<Vec<SearchHit>, WebSearchError> {
    let client = Client::new();
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
        .await?
        .error_for_status()?
        .json()
        .await?;

    let items = response["items"]
        .as_array()
        .ok_or(WebSearchError::EmptyResponse)?;

    Ok(items
        .iter()
        .take(limit)
        .filter_map(|item| {
            Some(SearchHit {
                title: item["title"].as_str()?.to_owned(),
                url: item["link"].as_str()?.to_owned(),
                snippet: item["snippet"].as_str().unwrap_or("").to_owned(),
            })
        })
        .collect())
}

async fn brave_search(
    query: &str,
    limit: usize,
    api_key: &str,
) -> Result<Vec<SearchHit>, WebSearchError> {
    let client = Client::new();
    let response: serde_json::Value = client
        .get("https://api.search.brave.com/res/v1/web/search")
        .header("Accept", "application/json")
        .header("Accept-Encoding", "gzip")
        .header("X-Subscription-Token", api_key)
        .query(&[("q", query), ("count", &limit.to_string())])
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let results = response["web"]["results"]
        .as_array()
        .ok_or(WebSearchError::EmptyResponse)?;

    Ok(results
        .iter()
        .take(limit)
        .filter_map(|item| {
            Some(SearchHit {
                title: item["title"].as_str()?.to_owned(),
                url: item["url"].as_str()?.to_owned(),
                snippet: item["description"].as_str().unwrap_or("").to_owned(),
            })
        })
        .collect())
}

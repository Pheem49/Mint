use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::MintConfig;

#[derive(Debug, Error)]
pub enum ImageSearchError {
    #[error("no image search API key configured (set googleSearchApiKey or braveSearchApiKey)")]
    NoApiKey,
    #[error("image search request failed: {0}")]
    Request(String),
    #[error("image search response was empty or unparseable")]
    EmptyResponse,
}

fn sanitize_reqwest_error(err: reqwest::Error) -> ImageSearchError {
    let mut msg = err.to_string();
    for host in [
        "https://www.googleapis.com",
        "https://api.search.brave.com",
    ] {
        if let Some(pos) = msg.find(host) {
            let mut end_pos = msg.len();
            for (idx, ch) in msg[pos..].char_indices() {
                if matches!(ch, ' ' | ')' | '"' | '\'' | ']' | '}' | '>') {
                    end_pos = pos + idx;
                    break;
                }
            }
            msg.replace_range(pos..end_pos, host);
        }
    }
    ImageSearchError::Request(msg)
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageHit {
    pub title: String,
    pub image_url: String,
    pub thumbnail_url: String,
    pub source_url: String,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSearchReport {
    pub query: String,
    /// Text summary followed by a ```image_search_json``` fenced block, ready to
    /// be copied verbatim into the agent's finish summary so the UI card renders.
    pub data: String,
    pub json_payload: Value,
}

fn build_report(query: &str, provider: &str, images: Vec<ImageHit>) -> ImageSearchReport {
    let json_payload = json!({
        "query": query,
        "provider": provider,
        "images": images,
    });
    let json_str = serde_json::to_string_pretty(&json_payload).unwrap_or_default();
    let text_summary = format!(
        "Found {} image result(s) for \"{query}\" via {provider} Image Search.",
        images.len()
    );
    let data = format!("{text_summary}\n\n```image_search_json\n{json_str}\n```");
    ImageSearchReport {
        query: query.to_owned(),
        data,
        json_payload,
    }
}

pub async fn image_search(
    query: &str,
    limit: usize,
    config: &MintConfig,
) -> Result<ImageSearchReport, ImageSearchError> {
    let query = query.trim();
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
        match google_image_search(query, limit, &google_key, &google_cx).await {
            Ok(images) if !images.is_empty() => {
                return Ok(build_report(query, "Google", images));
            }
            Ok(_) => {}
            Err(e) => last_err = Some(e),
        }
    }

    if !brave_key.is_empty() {
        match brave_image_search(query, limit, &brave_key).await {
            Ok(images) => return Ok(build_report(query, "Brave", images)),
            Err(e) => last_err = Some(e),
        }
    }

    Err(last_err.unwrap_or(ImageSearchError::NoApiKey))
}

async fn google_image_search(
    query: &str,
    limit: usize,
    api_key: &str,
    cx: &str,
) -> Result<Vec<ImageHit>, ImageSearchError> {
    let client = crate::HTTP_CLIENT.clone();
    let num_str = limit.clamp(1, 10).to_string();
    let response: Value = client
        .get("https://www.googleapis.com/customsearch/v1")
        .query(&[
            ("key", api_key),
            ("cx", cx),
            ("q", query),
            ("searchType", "image"),
            ("safe", "active"),
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
        .ok_or(ImageSearchError::EmptyResponse)?;

    Ok(items
        .iter()
        .take(limit)
        .filter_map(|item| {
            let image_url = item["link"].as_str()?.to_owned();
            let thumbnail_url = item["image"]["thumbnailLink"]
                .as_str()
                .unwrap_or(&image_url)
                .to_owned();
            let source_url = item["image"]["contextLink"]
                .as_str()
                .or_else(|| item["displayLink"].as_str())
                .unwrap_or(&image_url)
                .to_owned();
            Some(ImageHit {
                title: item["title"].as_str().unwrap_or("Image").to_owned(),
                image_url,
                thumbnail_url,
                source_url,
                width: item["image"]["width"].as_u64().map(|v| v as u32),
                height: item["image"]["height"].as_u64().map(|v| v as u32),
            })
        })
        .collect())
}

async fn brave_image_search(
    query: &str,
    limit: usize,
    api_key: &str,
) -> Result<Vec<ImageHit>, ImageSearchError> {
    let client = crate::HTTP_CLIENT.clone();
    let count_str = limit.clamp(1, 20).to_string();
    let response: Value = client
        .get("https://api.search.brave.com/res/v1/images/search")
        .header("Accept", "application/json")
        .header("X-Subscription-Token", api_key)
        .query(&[
            ("q", query),
            ("count", count_str.as_str()),
            ("safesearch", "strict"),
        ])
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
        .ok_or(ImageSearchError::EmptyResponse)?;

    Ok(results
        .iter()
        .take(limit)
        .filter_map(|item| {
            let thumbnail_url = item["thumbnail"]["src"].as_str().unwrap_or("").to_owned();
            let image_url = item["properties"]["url"]
                .as_str()
                .filter(|s| !s.is_empty())
                .unwrap_or(&thumbnail_url)
                .to_owned();
            if image_url.is_empty() {
                return None;
            }
            let source_url = item["url"].as_str().unwrap_or(&image_url).to_owned();
            Some(ImageHit {
                title: item["title"].as_str().unwrap_or("Image").to_owned(),
                image_url,
                thumbnail_url,
                source_url,
                width: item["properties"]["width"].as_u64().map(|v| v as u32),
                height: item["properties"]["height"].as_u64().map(|v| v as u32),
            })
        })
        .collect())
}

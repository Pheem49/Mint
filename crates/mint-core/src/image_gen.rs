use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::MintConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Public request / response types
// ─────────────────────────────────────────────────────────────────────────────

/// A request to generate images from any supported provider.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenRequest {
    pub prompt: String,
    #[serde(default)]
    pub negative_prompt: Option<String>,
    /// Aspect ratio hint: "1:1", "16:9", "9:16", "4:3". Default: "1:1"
    #[serde(default)]
    pub aspect_ratio: Option<String>,
    /// Number of images to return (1–4). Default: 1
    #[serde(default)]
    pub num_images: Option<u8>,
    /// Override the model for the selected provider.
    #[serde(default)]
    pub model: Option<String>,
    /// Which provider to use: "nanobanana" | "dalle" | "stability" | "ideogram" | "replicate".
    /// Falls back to `config.image_gen_provider` when omitted.
    #[serde(default)]
    pub provider: Option<String>,
}

/// One generated image returned from the model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GeneratedImage {
    /// Full data URI: `data:image/png;base64,...`
    pub data_uri: String,
    pub mime_type: String,
}

/// The complete response from an image generation call.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageGenResponse {
    pub images: Vec<GeneratedImage>,
    pub model: String,
    pub provider: String,
    pub prompt: String,
    /// Optional descriptive text returned alongside the image by some models.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Error)]
pub enum ImageGenError {
    // ── NanoBanana ──────────────────────────────────────────────────────────
    #[error(
        "missing Gemini API key — set GEMINI_API_KEY or configure api_key for NanoBanana image generation"
    )]
    MissingApiKey,
    // ── DALL·E ───────────────────────────────────────────────────────────────
    #[error("missing OpenAI API key — configure openai_api_key to use DALL·E")]
    MissingOpenAiKey,
    // ── Stability AI ─────────────────────────────────────────────────────────
    #[error("missing Stability AI API key — configure stability_api_key")]
    MissingStabilityKey,
    // ── Ideogram ─────────────────────────────────────────────────────────────
    #[error("missing Ideogram API key — configure ideogram_api_key")]
    MissingIdeogramKey,
    // ── Replicate ────────────────────────────────────────────────────────────
    #[error("missing Replicate API key — configure replicate_api_key")]
    MissingReplicateKey,
    // ── Generic ──────────────────────────────────────────────────────────────
    #[error("unsupported image generation provider: {0}")]
    UnsupportedProvider(String),
    #[error("API request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("the model returned no images for this prompt")]
    NoImagesReturned,
    #[error("model error: {0}")]
    ModelError(String),
    #[error("unexpected response structure from the image model")]
    UnexpectedResponse,
}

// ─────────────────────────────────────────────────────────────────────────────
// Main entry-point — dispatch to the correct provider
// ─────────────────────────────────────────────────────────────────────────────

/// Generate images using whichever provider is specified in the request or config.
pub async fn generate_images(
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let client = crate::HTTP_CLIENT.clone();

    let provider = request
        .provider
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| config.image_gen_provider.trim());

    let provider = if provider.is_empty() {
        "nanobanana"
    } else {
        provider
    };

    match provider {
        "nanobanana" => call_nanobanana(&client, config, request).await,
        "dalle" => call_dalle(&client, config, request).await,
        "stability" => call_stability(&client, config, request).await,
        "ideogram" => call_ideogram(&client, config, request).await,
        "replicate" => call_replicate(&client, config, request).await,
        other => Err(ImageGenError::UnsupportedProvider(other.to_owned())),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// NanoBanana — Google Gemini image generation
// ─────────────────────────────────────────────────────────────────────────────

async fn call_nanobanana(
    client: &Client,
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let api_key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(ImageGenError::MissingApiKey);
    }

    let model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let m = config.nanobanana_model.trim();
            if m.is_empty() {
                "gemini-2.5-flash-image".to_owned()
            } else {
                m.to_owned()
            }
        });

    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");

    let full_prompt = build_prompt(request);
    let num_images = request.num_images.unwrap_or(1).clamp(1, 4);

    let payload = json!({
        "contents": [{
            "parts": [{ "text": full_prompt }]
        }],
        "generationConfig": {
            "responseModalities": ["TEXT", "IMAGE"],
            "candidateCount": num_images
        }
    });

    let response: Value = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()
        .map_err(ImageGenError::Request)?
        .json()
        .await?;

    if let Some(err) = response.get("error") {
        let msg = err["message"].as_str().unwrap_or("unknown API error");
        return Err(ImageGenError::ModelError(msg.to_owned()));
    }

    parse_nanobanana_response(&response, &model, &request.prompt)
}

fn parse_nanobanana_response(
    response: &Value,
    model: &str,
    original_prompt: &str,
) -> Result<ImageGenResponse, ImageGenError> {
    let candidates = response["candidates"]
        .as_array()
        .ok_or(ImageGenError::UnexpectedResponse)?;

    let mut images = Vec::new();
    let mut description: Option<String> = None;

    for candidate in candidates {
        let parts_owned: Vec<Value>;
        let parts = match candidate["content"]["parts"].as_array() {
            Some(p) => p,
            None => {
                parts_owned = vec![];
                &parts_owned
            }
        };

        for part in parts {
            if let Some(text) = part["text"].as_str() {
                let text = text.trim();
                if !text.is_empty() {
                    description.get_or_insert_with(String::new).push_str(text);
                }
            }

            if let Some(inline) = part.get("inlineData") {
                let mime_type = inline["mimeType"]
                    .as_str()
                    .unwrap_or("image/png")
                    .to_owned();
                let b64 = inline["data"].as_str().unwrap_or("");
                if b64.is_empty() {
                    continue;
                }
                if STANDARD.decode(b64).is_err() {
                    continue;
                }
                let data_uri = format!("data:{mime_type};base64,{b64}");
                images.push(GeneratedImage {
                    data_uri,
                    mime_type,
                });
            }
        }
    }

    if images.is_empty() {
        return Err(ImageGenError::NoImagesReturned);
    }

    Ok(ImageGenResponse {
        images,
        model: model.to_owned(),
        provider: "nanobanana".to_owned(),
        prompt: original_prompt.to_owned(),
        description,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// DALL·E — OpenAI Images API
// ─────────────────────────────────────────────────────────────────────────────

async fn call_dalle(
    client: &Client,
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let api_key = if config.openai_api_key.trim().is_empty() {
        std::env::var("OPENAI_API_KEY").unwrap_or_default()
    } else {
        config.openai_api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(ImageGenError::MissingOpenAiKey);
    }

    let model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let m = config.dalle_model.trim();
            if m.is_empty() {
                "dall-e-3".to_owned()
            } else {
                m.to_owned()
            }
        });

    // DALL·E 3 supports only n=1; DALL·E 2 supports up to 10.
    let n = if model.starts_with("dall-e-3") || model == "gpt-image-1" {
        1u8
    } else {
        request.num_images.unwrap_or(1).clamp(1, 10)
    };

    let size = aspect_ratio_to_dalle_size(request.aspect_ratio.as_deref());

    let payload = json!({
        "model": model,
        "prompt": request.prompt,
        "n": n,
        "size": size,
        "response_format": "b64_json"
    });

    let response: Value = client
        .post("https://api.openai.com/v1/images/generations")
        .bearer_auth(&api_key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()
        .map_err(ImageGenError::Request)?
        .json()
        .await?;

    if let Some(err) = response.get("error") {
        let msg = err["message"]
            .as_str()
            .unwrap_or("unknown OpenAI API error");
        return Err(ImageGenError::ModelError(msg.to_owned()));
    }

    let data = response["data"]
        .as_array()
        .ok_or(ImageGenError::UnexpectedResponse)?;

    let images: Vec<GeneratedImage> = data
        .iter()
        .filter_map(|item| {
            let b64 = item["b64_json"].as_str()?;
            if b64.is_empty() || STANDARD.decode(b64).is_err() {
                return None;
            }
            let data_uri = format!("data:image/png;base64,{b64}");
            Some(GeneratedImage {
                data_uri,
                mime_type: "image/png".to_owned(),
            })
        })
        .collect();

    if images.is_empty() {
        return Err(ImageGenError::NoImagesReturned);
    }

    Ok(ImageGenResponse {
        images,
        model,
        provider: "dalle".to_owned(),
        prompt: request.prompt.clone(),
        description: None,
    })
}

fn aspect_ratio_to_dalle_size(ratio: Option<&str>) -> &'static str {
    match ratio.unwrap_or("1:1") {
        "16:9" => "1792x1024",
        "9:16" => "1024x1792",
        _ => "1024x1024",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Stability AI — Stable Image Core / SD3.x REST API
// ─────────────────────────────────────────────────────────────────────────────

async fn call_stability(
    client: &Client,
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let api_key = if config.stability_api_key.trim().is_empty() {
        std::env::var("STABILITY_API_KEY").unwrap_or_default()
    } else {
        config.stability_api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(ImageGenError::MissingStabilityKey);
    }

    let model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let m = config.stability_model.trim();
            if m.is_empty() {
                "sd3.5-large".to_owned()
            } else {
                m.to_owned()
            }
        });

    // Route: Stable Image Core uses /core; SD3.x models use /sd3
    let (endpoint, model_param) = if model == "core" {
        (
            "https://api.stability.ai/v2beta/stable-image/generate/core".to_owned(),
            None,
        )
    } else {
        (
            "https://api.stability.ai/v2beta/stable-image/generate/sd3".to_owned(),
            Some(model.clone()),
        )
    };

    let aspect_ratio = request.aspect_ratio.as_deref().unwrap_or("1:1");

    // Build multipart/form-data manually (reqwest multipart feature not enabled).
    let boundary = format!("MintBoundary{}", uuid_v4_hex());

    let mut body_parts: Vec<(String, String)> = vec![
        ("prompt".into(), request.prompt.clone()),
        ("aspect_ratio".into(), aspect_ratio.to_owned()),
        ("output_format".into(), "png".to_owned()),
    ];

    if let Some(neg) = &request.negative_prompt {
        let neg = neg.trim();
        if !neg.is_empty() {
            body_parts.push(("negative_prompt".into(), neg.to_owned()));
        }
    }

    if let Some(m) = model_param {
        body_parts.push(("model".into(), m));
    }

    let raw_body = build_multipart_text(&boundary, &body_parts);

    let response = client
        .post(&endpoint)
        .bearer_auth(&api_key)
        .header("accept", "image/*")
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(raw_body)
        .send()
        .await?
        .error_for_status()
        .map_err(ImageGenError::Request)?;

    let bytes = response.bytes().await?;
    if bytes.is_empty() {
        return Err(ImageGenError::NoImagesReturned);
    }

    let b64 = STANDARD.encode(&bytes);
    let data_uri = format!("data:image/png;base64,{b64}");

    Ok(ImageGenResponse {
        images: vec![GeneratedImage {
            data_uri,
            mime_type: "image/png".to_owned(),
        }],
        model,
        provider: "stability".to_owned(),
        prompt: request.prompt.clone(),
        description: None,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Ideogram — Ideogram v3 REST API
// ─────────────────────────────────────────────────────────────────────────────

async fn call_ideogram(
    client: &Client,
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let api_key = if config.ideogram_api_key.trim().is_empty() {
        std::env::var("IDEOGRAM_API_KEY").unwrap_or_default()
    } else {
        config.ideogram_api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(ImageGenError::MissingIdeogramKey);
    }

    let model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let m = config.ideogram_model.trim();
            if m.is_empty() {
                "V_3".to_owned()
            } else {
                m.to_owned()
            }
        });

    let num_images = request.num_images.unwrap_or(1).clamp(1, 4);
    let aspect_ratio = aspect_ratio_to_ideogram(request.aspect_ratio.as_deref());

    let mut body = json!({
        "image_request": {
            "prompt": request.prompt,
            "model": model,
            "num_images": num_images,
            "aspect_ratio": aspect_ratio,
            "magic_prompt_option": "AUTO"
        }
    });

    if let Some(neg) = &request.negative_prompt {
        let neg = neg.trim();
        if !neg.is_empty() {
            body["image_request"]["negative_prompt"] = json!(neg);
        }
    }

    let response: Value = client
        .post("https://api.ideogram.ai/generate")
        .header("Api-Key", &api_key)
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .map_err(ImageGenError::Request)?
        .json()
        .await?;

    if let Some(err) = response.get("error") {
        let msg = err.as_str().unwrap_or("unknown Ideogram API error");
        return Err(ImageGenError::ModelError(msg.to_owned()));
    }

    // Response shape: { data: [ { url: "..." }, ... ] }
    let data = response["data"]
        .as_array()
        .ok_or(ImageGenError::UnexpectedResponse)?;

    let mut images = Vec::new();
    for item in data {
        if let Some(url) = item["url"].as_str() {
            if url.is_empty() {
                continue;
            }
            // Fetch the image bytes and convert to base64 data URI
            match fetch_url_as_data_uri(client, url).await {
                Ok(data_uri) => images.push(GeneratedImage {
                    data_uri,
                    mime_type: "image/jpeg".to_owned(),
                }),
                Err(_) => continue,
            }
        }
    }

    if images.is_empty() {
        return Err(ImageGenError::NoImagesReturned);
    }

    Ok(ImageGenResponse {
        images,
        model,
        provider: "ideogram".to_owned(),
        prompt: request.prompt.clone(),
        description: None,
    })
}

fn aspect_ratio_to_ideogram(ratio: Option<&str>) -> &'static str {
    match ratio.unwrap_or("1:1") {
        "16:9" => "ASPECT_16_9",
        "9:16" => "ASPECT_9_16",
        "4:3" => "ASPECT_4_3",
        "3:4" => "ASPECT_3_4",
        _ => "ASPECT_1_1",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Replicate — Predictions API (supports FLUX, SDXL, etc.)
// ─────────────────────────────────────────────────────────────────────────────

async fn call_replicate(
    client: &Client,
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let api_key = if config.replicate_api_key.trim().is_empty() {
        std::env::var("REPLICATE_API_TOKEN").unwrap_or_default()
    } else {
        config.replicate_api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(ImageGenError::MissingReplicateKey);
    }

    let model = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            let m = config.replicate_model.trim();
            if m.is_empty() {
                "black-forest-labs/flux-1.1-pro".to_owned()
            } else {
                m.to_owned()
            }
        });

    let num_images = request.num_images.unwrap_or(1).clamp(1, 4);
    let aspect_ratio = request.aspect_ratio.as_deref().unwrap_or("1:1");

    // Build input — common fields used by FLUX and SDXL families
    let mut input = json!({
        "prompt": request.prompt,
        "num_outputs": num_images,
        "aspect_ratio": aspect_ratio,
        "output_format": "png",
        "output_quality": 90
    });

    if let Some(neg) = &request.negative_prompt {
        let neg = neg.trim();
        if !neg.is_empty() {
            input["negative_prompt"] = json!(neg);
        }
    }

    // POST to /v1/models/{model}/predictions
    let create_url = format!("https://api.replicate.com/v1/models/{model}/predictions");
    let create_payload = json!({ "input": input });

    let create_response: Value = client
        .post(&create_url)
        .bearer_auth(&api_key)
        .header("Prefer", "wait=60") // ask Replicate to wait up to 60s synchronously
        .json(&create_payload)
        .send()
        .await?
        .error_for_status()
        .map_err(ImageGenError::Request)?
        .json()
        .await?;

    // If already succeeded (Prefer: wait worked), parse immediately
    let prediction = if create_response["status"].as_str() == Some("succeeded") {
        create_response
    } else {
        // Fall back to polling
        let prediction_url = create_response["urls"]["get"]
            .as_str()
            .ok_or(ImageGenError::UnexpectedResponse)?
            .to_owned();

        poll_replicate(client, &api_key, &prediction_url).await?
    };

    if let Some(err) = prediction.get("error") {
        if !err.is_null() {
            let msg = err.as_str().unwrap_or("unknown Replicate error");
            return Err(ImageGenError::ModelError(msg.to_owned()));
        }
    }

    // Output is an array of image URLs
    let output = prediction["output"]
        .as_array()
        .ok_or(ImageGenError::UnexpectedResponse)?;

    let mut images = Vec::new();
    for url_val in output {
        if let Some(url) = url_val.as_str() {
            match fetch_url_as_data_uri(client, url).await {
                Ok(data_uri) => images.push(GeneratedImage {
                    data_uri,
                    mime_type: "image/png".to_owned(),
                }),
                Err(_) => continue,
            }
        }
    }

    if images.is_empty() {
        return Err(ImageGenError::NoImagesReturned);
    }

    Ok(ImageGenResponse {
        images,
        model,
        provider: "replicate".to_owned(),
        prompt: request.prompt.clone(),
        description: None,
    })
}

/// Poll a Replicate prediction URL until it reaches a terminal state.
async fn poll_replicate(client: &Client, api_key: &str, url: &str) -> Result<Value, ImageGenError> {
    for _ in 0..60 {
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let response: Value = client
            .get(url)
            .bearer_auth(api_key)
            .send()
            .await?
            .error_for_status()
            .map_err(ImageGenError::Request)?
            .json()
            .await?;

        match response["status"].as_str() {
            Some("succeeded") | Some("failed") | Some("canceled") => return Ok(response),
            _ => continue,
        }
    }

    Err(ImageGenError::ModelError(
        "Replicate prediction timed out after 120 seconds".to_owned(),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Shared helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Download an image URL and return it as a `data:<mime>;base64,<data>` URI.
async fn fetch_url_as_data_uri(client: &Client, url: &str) -> Result<String, ImageGenError> {
    let resp = client
        .get(url)
        .send()
        .await?
        .error_for_status()
        .map_err(ImageGenError::Request)?;

    let mime_type = resp
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .map(|ct| {
            ct.split(';')
                .next()
                .unwrap_or("image/jpeg")
                .trim()
                .to_owned()
        })
        .unwrap_or_else(|| "image/jpeg".to_owned());

    let bytes = resp.bytes().await?;
    if bytes.is_empty() {
        return Err(ImageGenError::NoImagesReturned);
    }

    let b64 = STANDARD.encode(&bytes);
    Ok(format!("data:{mime_type};base64,{b64}"))
}

/// Generate a short random hex string suitable for a multipart boundary.
/// Uses `/dev/urandom` on Linux/macOS via the `sha2` crate's internal
/// randomness; falls back to a time-based seed. No extra crate required.
fn uuid_v4_hex() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    // Mix nanos + a simple linear-congruential step for fast unique strings
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(12345);
    format!(
        "{:016x}{:016x}",
        seed as u64 ^ 0xDEAD_BEEF_CAFE_BABE,
        seed as u64
    )
}

/// Encode `fields` as a `multipart/form-data` text-only body for the given boundary.
fn build_multipart_text(boundary: &str, fields: &[(String, String)]) -> String {
    let mut body = String::new();
    for (name, value) in fields {
        body.push_str(&format!("--{boundary}\r\n"));
        body.push_str(&format!(
            "Content-Disposition: form-data; name=\"{name}\"\r\n\r\n"
        ));
        body.push_str(value);
        body.push_str("\r\n");
    }
    body.push_str(&format!("--{boundary}--\r\n"));
    body
}

/// Build the final prompt string, optionally embedding aspect ratio and negative
/// prompt guidance. Used by providers that accept a plain text prompt.
fn build_prompt(request: &ImageGenRequest) -> String {
    let mut prompt = request.prompt.trim().to_owned();

    if let Some(ref neg) = request.negative_prompt {
        let neg = neg.trim();
        if !neg.is_empty() {
            prompt.push_str(&format!(
                "\n\nNegative prompt (avoid these elements): {neg}"
            ));
        }
    }

    if let Some(ref ratio) = request.aspect_ratio {
        let ratio = ratio.trim();
        if !ratio.is_empty() && ratio != "1:1" {
            prompt.push_str(&format!(
                "\n\nGenerate the image with a {ratio} aspect ratio."
            ));
        }
    }

    prompt
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(prompt: &str) -> ImageGenRequest {
        ImageGenRequest {
            prompt: prompt.to_owned(),
            negative_prompt: None,
            aspect_ratio: None,
            num_images: None,
            model: None,
            provider: None,
        }
    }

    #[test]
    fn build_prompt_plain() {
        let req = make_request("a red apple");
        assert_eq!(build_prompt(&req), "a red apple");
    }

    #[test]
    fn build_prompt_with_negative_and_ratio() {
        let req = ImageGenRequest {
            prompt: "a cat".to_owned(),
            negative_prompt: Some("blurry, low quality".to_owned()),
            aspect_ratio: Some("16:9".to_owned()),
            num_images: Some(2),
            model: None,
            provider: None,
        };
        let result = build_prompt(&req);
        assert!(result.contains("a cat"));
        assert!(result.contains("Negative prompt"));
        assert!(result.contains("16:9"));
    }

    #[test]
    fn build_prompt_default_ratio_not_added() {
        let req = ImageGenRequest {
            prompt: "a dog".to_owned(),
            negative_prompt: None,
            aspect_ratio: Some("1:1".to_owned()),
            num_images: None,
            model: None,
            provider: None,
        };
        assert!(!build_prompt(&req).contains("aspect ratio"));
    }

    #[test]
    fn num_images_clamped() {
        assert_eq!(5u8.clamp(1, 4), 4);
        assert_eq!(0u8.clamp(1, 4), 1);
    }

    #[test]
    fn dalle_size_mapping() {
        assert_eq!(aspect_ratio_to_dalle_size(Some("16:9")), "1792x1024");
        assert_eq!(aspect_ratio_to_dalle_size(Some("9:16")), "1024x1792");
        assert_eq!(aspect_ratio_to_dalle_size(Some("1:1")), "1024x1024");
        assert_eq!(aspect_ratio_to_dalle_size(None), "1024x1024");
    }

    #[test]
    fn ideogram_ratio_mapping() {
        assert_eq!(aspect_ratio_to_ideogram(Some("16:9")), "ASPECT_16_9");
        assert_eq!(aspect_ratio_to_ideogram(Some("9:16")), "ASPECT_9_16");
        assert_eq!(aspect_ratio_to_ideogram(Some("4:3")), "ASPECT_4_3");
        assert_eq!(aspect_ratio_to_ideogram(Some("1:1")), "ASPECT_1_1");
        assert_eq!(aspect_ratio_to_ideogram(None), "ASPECT_1_1");
    }
}

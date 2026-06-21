use base64::{Engine as _, engine::general_purpose::STANDARD};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::MintConfig;

/// A request to generate images with the NanoBanana (Gemini image) model.
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
    #[serde(default)]
    pub model: Option<String>,
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
    /// Optional descriptive text returned alongside the image by the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Error)]
pub enum ImageGenError {
    #[error("missing Gemini API key — set GEMINI_API_KEY or configure api_key for NanoBanana image generation")]
    MissingApiKey,
    #[error("API request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("the model returned no images for this prompt")]
    NoImagesReturned,
    #[error("model error: {0}")]
    ModelError(String),
    #[error("unexpected response structure from the image model")]
    UnexpectedResponse,
}

/// Generate images using Google NanoBanana (Gemini image generation model).
///
/// Uses the same `generativelanguage.googleapis.com` endpoint as text chat,
/// but with `responseModalities: ["TEXT", "IMAGE"]` in the generation config.
pub async fn generate_images(
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    let client = crate::HTTP_CLIENT.clone();
    call_nanobanana(&client, config, request).await
}

async fn call_nanobanana(
    client: &Client,
    config: &MintConfig,
    request: &ImageGenRequest,
) -> Result<ImageGenResponse, ImageGenError> {
    // Resolve API key — same field as Gemini chat
    let api_key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(ImageGenError::MissingApiKey);
    }

    let model = match request.model.as_ref().map(|m| m.trim()).filter(|m| !m.is_empty()) {
        Some(m) => m.to_owned(),
        None => {
            let config_model = config.nanobanana_model.trim();
            if config_model.is_empty() {
                "gemini-2.5-flash-image".to_owned()
            } else {
                config_model.to_owned()
            }
        }
    };

    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent"
    );

    // Build a prompt that embeds aspect ratio guidance if requested
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
        .map_err(|e| {
            // Surface useful error text from the model
            ImageGenError::Request(e)
        })?
        .json()
        .await?;

    // Check for API-level error block
    if let Some(err) = response.get("error") {
        let msg = err["message"].as_str().unwrap_or("unknown API error");
        return Err(ImageGenError::ModelError(msg.to_owned()));
    }

    parse_image_response(&response, &model, &request.prompt)
}

/// Parse the `generateContent` response and extract image parts.
fn parse_image_response(
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
            // Collect text description if present
            if let Some(text) = part["text"].as_str() {
                let text = text.trim();
                if !text.is_empty() {
                    description.get_or_insert_with(String::new).push_str(text);
                }
            }

            // Collect inline image data
            if let Some(inline) = part.get("inlineData") {
                let mime_type = inline["mimeType"]
                    .as_str()
                    .unwrap_or("image/png")
                    .to_owned();
                let b64 = inline["data"].as_str().unwrap_or("");
                if b64.is_empty() {
                    continue;
                }
                // Validate it's actually decodable base64
                if STANDARD.decode(b64).is_err() {
                    continue;
                }
                let data_uri = format!("data:{mime_type};base64,{b64}");
                images.push(GeneratedImage { data_uri, mime_type });
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

/// Build the final prompt string, optionally embedding aspect ratio guidance.
fn build_prompt(request: &ImageGenRequest) -> String {
    let mut prompt = request.prompt.trim().to_owned();

    if let Some(ref neg) = request.negative_prompt {
        let neg = neg.trim();
        if !neg.is_empty() {
            prompt.push_str(&format!("\n\nNegative prompt (avoid these elements): {neg}"));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_prompt_plain() {
        let req = ImageGenRequest {
            prompt: "a red apple".to_owned(),
            negative_prompt: None,
            aspect_ratio: None,
            num_images: None,
        };
        assert_eq!(build_prompt(&req), "a red apple");
    }

    #[test]
    fn build_prompt_with_negative_and_ratio() {
        let req = ImageGenRequest {
            prompt: "a cat".to_owned(),
            negative_prompt: Some("blurry, low quality".to_owned()),
            aspect_ratio: Some("16:9".to_owned()),
            num_images: Some(2),
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
        };
        // "1:1" is default — should not append extra text
        assert!(!build_prompt(&req).contains("aspect ratio"));
    }

    #[test]
    fn num_images_clamped() {
        assert_eq!(5u8.clamp(1, 4), 4);
        assert_eq!(0u8.clamp(1, 4), 1);
    }
}

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{MintConfig, PictureEntry};

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoGenRequest {
    pub prompt: String,
    pub negative_prompt: Option<String>,
    pub aspect_ratio: String,
    pub duration: u32,
    pub model: Option<String>,
    pub provider: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoGenResponse {
    pub videos: Vec<PictureEntry>,
    pub model: String,
    pub provider: String,
    pub prompt: String,
}

#[derive(Debug, Error)]
pub enum VideoGenError {
    #[error("missing Gemini API key — set GEMINI_API_KEY or configure api_key in config")]
    MissingApiKey,
    #[error("API request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("unexpected response structure: {0}")]
    UnexpectedResponse(String),
    #[error("video generation timed out")]
    Timeout,
    #[error("failed to save generated video: {0}")]
    SaveError(String),
}

pub async fn generate_video(
    config: &MintConfig,
    request: &VideoGenRequest,
) -> Result<VideoGenResponse, VideoGenError> {
    let api_key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err(VideoGenError::MissingApiKey);
    }

    let model_owned = request
        .model
        .as_deref()
        .map(str::trim)
        .filter(|m| !m.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            config
                .extra
                .get("veoModel")
                .and_then(|v| v.as_str())
                .unwrap_or("veo-2.0-flash-exp")
                .to_string()
        });
    let model = &model_owned;

    let client = crate::HTTP_CLIENT.clone();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:predictLongRunning"
    );

    let payload = json!({
        "instances": [
            {
                "prompt": request.prompt
            }
        ],
        "parameters": {
            "aspectRatio": request.aspect_ratio,
            "durationSeconds": request.duration,
            "sampleCount": 1
        }
    });

    let res: Value = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let operation_name = res
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| VideoGenError::UnexpectedResponse("Missing operation name".to_string()))?;

    let poll_url = format!("https://generativelanguage.googleapis.com/v1beta/{operation_name}");
    let mut done = false;
    let mut final_response = None;

    // Poll every 5 seconds, up to 60 times (5 minutes max)
    for _ in 0..60 {
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        let poll_res: Value = client
            .get(&poll_url)
            .header("x-goog-api-key", &api_key)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        if let Some(d) = poll_res.get("done").and_then(|v| v.as_bool()) {
            if d {
                done = true;
                final_response = Some(poll_res);
                break;
            }
        }
    }

    if !done {
        return Err(VideoGenError::Timeout);
    }

    let op_res = final_response.ok_or(VideoGenError::Timeout)?;

    // Check if error occurred in operation
    if let Some(err) = op_res.get("error") {
        let msg = err["message"].as_str().unwrap_or("unknown operation error");
        return Err(VideoGenError::UnexpectedResponse(msg.to_string()));
    }

    let response_obj = op_res
        .get("response")
        .ok_or_else(|| VideoGenError::UnexpectedResponse("Missing response object".to_string()))?;

    // Try to find generated samples or outputs
    let generated_samples = response_obj
        .get("generateVideoResponse")
        .and_then(|gvr| gvr.get("generatedSamples"))
        .or_else(|| {
            response_obj
                .get("outputs")
                .and_then(|o| o.as_array()?.first()?.get("generatedVideos"))
        })
        .ok_or_else(|| {
            VideoGenError::UnexpectedResponse(
                "No generated samples or videos in response".to_string(),
            )
        })?;

    let sample = generated_samples
        .as_array()
        .and_then(|arr| arr.first())
        .ok_or_else(|| {
            VideoGenError::UnexpectedResponse("Generated samples array is empty".to_string())
        })?;

    let video_uri = sample
        .get("video")
        .and_then(|v| v.get("uri"))
        .and_then(|u| u.as_str())
        .ok_or_else(|| VideoGenError::UnexpectedResponse("Missing video URI".to_string()))?;

    // Download the video file bytes
    let video_bytes = client
        .get(video_uri)
        .header("x-goog-api-key", &api_key)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?
        .to_vec();

    // Convert to data URI for saving
    let data_b64 = STANDARD.encode(&video_bytes);
    let data_uri = format!("data:video/mp4;base64,{data_b64}");

    // Save to the pictures directory and record index entry
    let saved = crate::pictures::save_chat_images(
        vec![data_uri],
        Some("veo".to_string()),
        Some(request.prompt.clone()),
    )
    .map_err(|e| VideoGenError::SaveError(e.to_string()))?;

    Ok(VideoGenResponse {
        videos: saved,
        model: model.to_string(),
        provider: "veo".to_string(),
        prompt: request.prompt.clone(),
    })
}

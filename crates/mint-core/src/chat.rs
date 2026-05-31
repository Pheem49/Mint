use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::MintConfig;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub system_instruction: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub provider: String,
    pub model: String,
    pub text: String,
}

#[derive(Debug, Error)]
pub enum ChatError {
    #[error("provider '{0}' is not implemented in the Rust backend yet")]
    UnsupportedProvider(String),
    #[error("missing API key for provider '{0}'")]
    MissingApiKey(String),
    #[error("provider request failed: {0}")]
    Request(#[from] reqwest::Error),
    #[error("provider response did not include assistant text")]
    MissingResponseText,
}

pub async fn send_chat(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, ChatError> {
    let client = Client::new();
    let provider = config.ai_provider.as_str();
    let (model, text) = match provider {
        "gemini" => call_gemini(&client, config, request).await?,
        "openai" | "local_openai" => call_openai(&client, config, request).await?,
        "ollama" => call_ollama(&client, config, request).await?,
        "anthropic" => call_anthropic(&client, config, request).await?,
        other => return Err(ChatError::UnsupportedProvider(other.into())),
    };
    Ok(ChatResponse {
        provider: provider.into(),
        model,
        text,
    })
}

async fn call_gemini(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(String, String), ChatError> {
    let api_key = provider_key(&config.api_key, "GEMINI_API_KEY");
    required_key("gemini", &api_key)?;
    let model = config.gemini_model.clone();
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent?key={}",
        api_key
    );
    let response: Value = client
        .post(url)
        .json(&json!({
            "systemInstruction": { "parts": [{ "text": request.system_instruction }] },
            "contents": [{ "role": "user", "parts": [{ "text": request.message }] }]
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok((
        model,
        response["candidates"][0]["content"]["parts"][0]["text"]
            .as_str()
            .ok_or(ChatError::MissingResponseText)?
            .into(),
    ))
}

async fn call_openai(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(String, String), ChatError> {
    let local = config.ai_provider == "local_openai";
    let api_key = provider_key(&config.openai_api_key, "OPENAI_API_KEY");
    if !local {
        required_key("openai", &api_key)?;
    }
    let base_url = if local {
        config.local_api_base_url.trim_end_matches('/')
    } else {
        "https://api.openai.com/v1"
    };
    let model = if local {
        config.local_model_name.clone()
    } else {
        config.openai_model.clone()
    };
    let response: Value = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(if local { "not-needed" } else { &api_key })
        .json(&json!({
            "model": model,
            "messages": [
                { "role": "system", "content": request.system_instruction },
                { "role": "user", "content": request.message }
            ]
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok((
        model,
        response["choices"][0]["message"]["content"]
            .as_str()
            .ok_or(ChatError::MissingResponseText)?
            .into(),
    ))
}

async fn call_ollama(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(String, String), ChatError> {
    let host = if config.ollama_host.trim().is_empty() {
        "http://localhost:11434"
    } else {
        config.ollama_host.trim_end_matches('/')
    };
    let model = config.ollama_model.clone();
    let response: Value = client
        .post(format!("{host}/api/chat"))
        .json(&json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": request.system_instruction },
                { "role": "user", "content": request.message }
            ]
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok((
        model,
        response["message"]["content"]
            .as_str()
            .ok_or(ChatError::MissingResponseText)?
            .into(),
    ))
}

async fn call_anthropic(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(String, String), ChatError> {
    let api_key = provider_key(&config.anthropic_api_key, "ANTHROPIC_API_KEY");
    required_key("anthropic", &api_key)?;
    let model = config.anthropic_model.clone();
    let response: Value = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "max_tokens": 8192,
            "system": request.system_instruction,
            "messages": [{ "role": "user", "content": request.message }]
        }))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    Ok((
        model,
        response["content"][0]["text"]
            .as_str()
            .ok_or(ChatError::MissingResponseText)?
            .into(),
    ))
}

fn required_key(provider: &str, key: &str) -> Result<(), ChatError> {
    if key.trim().is_empty() {
        Err(ChatError::MissingApiKey(provider.into()))
    } else {
        Ok(())
    }
}

fn provider_key(configured: &str, environment_variable: &str) -> String {
    if configured.trim().is_empty() {
        std::env::var(environment_variable).unwrap_or_default()
    } else {
        configured.into()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_api_key() {
        let error = required_key("gemini", "").unwrap_err();
        assert!(matches!(error, ChatError::MissingApiKey(provider) if provider == "gemini"));
    }
}

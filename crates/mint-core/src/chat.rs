use futures_util::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::MintConfig;

/// Send a chat request, automatically falling back to other configured providers
/// if the primary one returns a recoverable error.
/// Returns `(response, Option<fallback_provider>)`.
pub async fn send_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), ChatError> {
    match send_chat(config, request).await {
        Ok(r) => return Ok((r, None)),
        Err(e) if !is_recoverable(&e) => return Err(e),
        Err(_) => {}
    }
    for provider in config.available_providers() {
        if provider == config.ai_provider.as_str() {
            continue;
        }
        let alt = config_for_provider(config, provider);
        if let Ok(r) = send_chat(&alt, request).await {
            return Ok((r, Some(provider.to_owned())));
        }
    }
    // all fallbacks failed — retry primary to surface original error
    send_chat(config, request).await.map(|r| (r, None))
}

/// Stream a chat request with automatic provider fallback.
/// Returns `(response, Option<fallback_provider>)`.
pub async fn stream_chat_with_fallback<F>(
    config: &MintConfig,
    request: &ChatRequest,
    mut on_chunk: F,
) -> Result<(ChatResponse, Option<String>), ChatError>
where
    F: FnMut(String),
{
    match stream_chat(config, request, &mut on_chunk).await {
        Ok(r) => return Ok((r, None)),
        Err(e) if !is_recoverable(&e) => return Err(e),
        Err(_) => {}
    }
    for provider in config.available_providers() {
        if provider == config.ai_provider.as_str() {
            continue;
        }
        let alt = config_for_provider(config, provider);
        if let Ok(r) = stream_chat(&alt, request, &mut on_chunk).await {
            return Ok((r, Some(provider.to_owned())));
        }
    }
    stream_chat(config, request, &mut on_chunk)
        .await
        .map(|r| (r, None))
}

/// Whether an error warrants trying another provider.
fn is_recoverable(e: &ChatError) -> bool {
    matches!(
        e,
        ChatError::MissingApiKey(_)
            | ChatError::Request(_)
            | ChatError::MissingResponseText
            | ChatError::UnsupportedAttachments(_)
    )
}

/// Clone the config with a different active provider.
fn config_for_provider(config: &MintConfig, provider: &str) -> MintConfig {
    MintConfig {
        ai_provider: provider.to_owned(),
        ..config.clone()
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub message: String,
    #[serde(default)]
    pub system_instruction: String,
    #[serde(default)]
    pub image_data_uri: Option<String>,
    #[serde(default)]
    pub audio_data_uri: Option<String>,
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
    #[error("provider '{0}' does not support Mint multimodal attachments yet")]
    UnsupportedAttachments(String),
    #[error("invalid multimodal data URI")]
    InvalidAttachment,
}

pub async fn send_chat(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, ChatError> {
    let client = crate::HTTP_CLIENT.clone();
    let provider = config.ai_provider.as_str();
    require_supported_attachments(provider, request)?;
    let (model, text) = match provider {
        "gemini" => call_gemini(&client, config, request).await?,
        "openai" | "local_openai" => call_openai(&client, config, request).await?,
        "ollama" => call_ollama(&client, config, request).await?,
        "anthropic" => call_anthropic(&client, config, request).await?,
        "huggingface" => call_huggingface(&client, config, request).await?,
        other => return Err(ChatError::UnsupportedProvider(other.into())),
    };
    Ok(ChatResponse {
        provider: provider.into(),
        model,
        text,
    })
}

pub async fn stream_chat<F>(
    config: &MintConfig,
    request: &ChatRequest,
    mut on_chunk: F,
) -> Result<ChatResponse, ChatError>
where
    F: FnMut(String),
{
    let client = crate::HTTP_CLIENT.clone();
    let provider = config.ai_provider.as_str();
    require_supported_attachments(provider, request)?;
    let (model, text) = match provider {
        "gemini" => stream_gemini(&client, config, request, &mut on_chunk).await?,
        "openai" | "local_openai" => stream_openai(&client, config, request, &mut on_chunk).await?,
        "ollama" => stream_ollama(&client, config, request, &mut on_chunk).await?,
        "anthropic" => stream_anthropic(&client, config, request, &mut on_chunk).await?,
        "huggingface" => stream_huggingface(&client, config, request, &mut on_chunk).await?,
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
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");

    let response: Value = client
        .post(url)
        .header("x-goog-api-key", api_key)
        .json(&gemini_chat_payload(config, request)?)
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
        .json(&openai_chat_payload(&model, request, false))
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

async fn call_huggingface(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(String, String), ChatError> {
    let api_key = provider_key(&config.hf_api_key, "HF_TOKEN");
    required_key("huggingface", &api_key)?;
    let model = config.hf_model.clone();
    let response: Value = client
        .post("https://router.huggingface.co/v1/chat/completions")
        .bearer_auth(api_key)
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

async fn stream_gemini<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: &mut F,
) -> Result<(String, String), ChatError>
where
    F: FnMut(String),
{
    let api_key = provider_key(&config.api_key, "GEMINI_API_KEY");
    required_key("gemini", &api_key)?;
    let model = config.gemini_model.clone();

    let response = client
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"
        ))
        .header("x-goog-api-key", api_key)
        .json(&gemini_chat_payload(config, request)?)
        .send()
        .await?
        .error_for_status()?;
    collect_stream(response, StreamFormat::Gemini, on_chunk)
        .await
        .map(|text| (model, text))
}

async fn stream_openai<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: &mut F,
) -> Result<(String, String), ChatError>
where
    F: FnMut(String),
{
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
    let response = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(if local { "not-needed" } else { &api_key })
        .json(&openai_chat_payload(&model, request, true))
        .send()
        .await?
        .error_for_status()?;
    collect_stream(response, StreamFormat::OpenAi, on_chunk)
        .await
        .map(|text| (model, text))
}

async fn stream_ollama<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: &mut F,
) -> Result<(String, String), ChatError>
where
    F: FnMut(String),
{
    let host = if config.ollama_host.trim().is_empty() {
        "http://localhost:11434"
    } else {
        config.ollama_host.trim_end_matches('/')
    };
    let model = config.ollama_model.clone();
    let response = client
        .post(format!("{host}/api/chat"))
        .json(&json!({
            "model": model,
            "stream": true,
            "messages": [
                { "role": "system", "content": request.system_instruction },
                { "role": "user", "content": request.message }
            ]
        }))
        .send()
        .await?
        .error_for_status()?;
    collect_stream(response, StreamFormat::Ollama, on_chunk)
        .await
        .map(|text| (model, text))
}

async fn stream_anthropic<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: &mut F,
) -> Result<(String, String), ChatError>
where
    F: FnMut(String),
{
    let api_key = provider_key(&config.anthropic_api_key, "ANTHROPIC_API_KEY");
    required_key("anthropic", &api_key)?;
    let model = config.anthropic_model.clone();
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&json!({
            "model": model,
            "max_tokens": 8192,
            "stream": true,
            "system": request.system_instruction,
            "messages": [{ "role": "user", "content": request.message }]
        }))
        .send()
        .await?
        .error_for_status()?;
    collect_stream(response, StreamFormat::Anthropic, on_chunk)
        .await
        .map(|text| (model, text))
}

async fn stream_huggingface<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: &mut F,
) -> Result<(String, String), ChatError>
where
    F: FnMut(String),
{
    let api_key = provider_key(&config.hf_api_key, "HF_TOKEN");
    required_key("huggingface", &api_key)?;
    let model = config.hf_model.clone();
    let response = client
        .post("https://router.huggingface.co/v1/chat/completions")
        .bearer_auth(api_key)
        .json(&json!({
            "model": model,
            "stream": true,
            "messages": [
                { "role": "system", "content": request.system_instruction },
                { "role": "user", "content": request.message }
            ]
        }))
        .send()
        .await?
        .error_for_status()?;
    collect_stream(response, StreamFormat::OpenAi, on_chunk)
        .await
        .map(|text| (model, text))
}

#[derive(Clone, Copy)]
enum StreamFormat {
    Gemini,
    OpenAi,
    Ollama,
    Anthropic,
}

async fn collect_stream<F>(
    response: reqwest::Response,
    format: StreamFormat,
    on_chunk: &mut F,
) -> Result<String, ChatError>
where
    F: FnMut(String),
{
    let mut bytes = response.bytes_stream();
    let mut buffer = String::new();
    let mut text = String::new();
    while let Some(chunk) = bytes.next().await {
        buffer.push_str(&String::from_utf8_lossy(&chunk?));
        while let Some(index) = buffer.find('\n') {
            let line = buffer[..index].trim().to_owned();
            buffer.drain(..=index);
            if let Some(chunk) = parse_stream_line(format, &line) {
                on_chunk(chunk.clone());
                text.push_str(&chunk);
            }
        }
    }
    if let Some(chunk) = parse_stream_line(format, buffer.trim()) {
        on_chunk(chunk.clone());
        text.push_str(&chunk);
    }
    if text.is_empty() {
        Err(ChatError::MissingResponseText)
    } else {
        Ok(text)
    }
}

fn parse_stream_line(format: StreamFormat, line: &str) -> Option<String> {
    let payload = match format {
        StreamFormat::Ollama => line,
        _ => line.strip_prefix("data: ")?.trim(),
    };
    if payload.is_empty() || payload == "[DONE]" {
        return None;
    }
    let value: Value = serde_json::from_str(payload).ok()?;
    let text = match format {
        StreamFormat::Gemini => value["candidates"][0]["content"]["parts"][0]["text"].as_str(),
        StreamFormat::OpenAi => value["choices"][0]["delta"]["content"].as_str(),
        StreamFormat::Ollama => value["message"]["content"].as_str(),
        StreamFormat::Anthropic => value["delta"]["text"].as_str(),
    }?;
    (!text.is_empty()).then(|| text.to_owned())
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

fn require_supported_attachments(provider: &str, request: &ChatRequest) -> Result<(), ChatError> {
    if provider != "gemini"
        && (request.image_data_uri.is_some() || request.audio_data_uri.is_some())
    {
        return Err(ChatError::UnsupportedAttachments(provider.into()));
    }
    Ok(())
}

fn wants_agent_json(request: &ChatRequest) -> bool {
    let instruction = request.system_instruction.as_str();
    instruction.contains("Return only JSON")
        || (instruction.contains("Return exactly one JSON object per response")
            && instruction.contains("Input formats:")
            && instruction.contains("- finish:"))
}

fn gemini_chat_payload(config: &MintConfig, request: &ChatRequest) -> Result<Value, ChatError> {
    let mut payload = json!({
        "systemInstruction": { "parts": [{ "text": request.system_instruction }] },
        "contents": [{ "role": "user", "parts": gemini_parts(request)? }]
    });
    if wants_agent_json(request) {
        payload["generationConfig"] = gemini_agent_generation_config(config);
    }
    Ok(payload)
}

fn gemini_agent_generation_config(config: &MintConfig) -> Value {
    let mut allowed_actions = vec![
        "list_files",
        "read_file",
        "search_code",
        "symbols",
        "semantic_index",
        "semantic_search",
        "knowledge_search",
        "web_search",
        "memory_recall",
        "git_status",
        "git_diff",
        "git_log",
        "git_branch",
        "create_plan",
        "update_plan",
        "request_user_approval",
        "ask_user",
        "detect_project",
        "list_tests",
        "read_diagnostics",
        "view_image",
        "note_write",
        "run_plugin",
        "mcp_tool",
        "run_shell",
        "verify",
        "apply_patch",
        "write_file",
    ];
    allowed_actions.retain(|action| !config.disabled_tools.contains(&action.to_string()));
    allowed_actions.push("finish");

    json!({
        "responseMimeType": "application/json",
        "responseSchema": {
            "type": "OBJECT",
            "properties": {
                "thought": { "type": "STRING" },
                "action": {
                    "type": "STRING",
                    "enum": allowed_actions
                },
                "input": {
                    "type": "OBJECT",
                    "properties": {
                        "path": { "type": "STRING", "description": "The target file or directory path (required for list_files, read_file, write_file, apply_patch, symbols, semantic_index, semantic_search)" },
                        "query": { "type": "STRING", "description": "The search query string (required for search_code, semantic_search, knowledge_search, web_search, memory_recall)" },
                        "command": { "type": "STRING", "description": "The local read-only or test shell command to run (required for run_shell)" },
                        "commands": {
                            "type": "ARRAY",
                            "items": { "type": "STRING" },
                            "description": "List of verification commands (required for verify)"
                        },
                        "steps": {
                            "type": "ARRAY",
                            "items": { "type": "STRING" },
                            "description": "Plan steps (for create_plan/update_plan)"
                        },
                        "fileContent": { "type": "STRING", "description": "The complete new content of a new file (required for write_file, note_write)" },
                        "summary": { "type": "STRING", "description": "The final detailed answer, explanation, or response to the user's query (required for finish)" },
                        "verification": { "type": "STRING", "description": "The description of checks to run before finishing" },
                        "title": { "type": "STRING", "description": "Short approval or plan title" },
                        "status": { "type": "STRING", "description": "Plan item status or project status" },
                        "startLine": { "type": "INTEGER", "description": "First line to read (1-indexed, for read_file)" },
                        "endLine": { "type": "INTEGER", "description": "Last line to read (for read_file)" },
                        "limit": { "type": "INTEGER", "description": "Max number of items/lines/files to return" },
                        "server": { "type": "STRING", "description": "MCP server name (for mcp_tool)" },
                        "tool": { "type": "STRING", "description": "MCP tool name (for mcp_tool)" },
                        "notePath": { "type": "STRING", "description": "The file path for note writing (for note_write)" },
                        "name": { "type": "STRING", "description": "Plugin name (for run_plugin)" },
                        "instruction": { "type": "STRING", "description": "Instruction to run the plugin (for run_plugin)" },
                        "patch": {
                            "type": "OBJECT",
                            "properties": {
                                "path": { "type": "STRING", "description": "The target file path (required for apply_patch)" },
                                "hunks": {
                                    "type": "ARRAY",
                                    "items": {
                                        "type": "OBJECT",
                                        "properties": {
                                            "oldText": { "type": "STRING", "description": "The exact block of code to replace" },
                                            "newText": { "type": "STRING", "description": "The replacement block of code" }
                                        }
                                    }
                                }
                            }
                        },
                        "arguments": { "type": "OBJECT", "description": "MCP tool arguments (for mcp_tool)" }
                    },
                    "required": ["path", "query", "command", "fileContent", "summary"]
                }
            },
            "required": ["thought", "action", "input"]
        }
    })
}

fn openai_chat_payload(model: &str, request: &ChatRequest, stream: bool) -> Value {
    let mut payload = json!({
        "model": model,
        "stream": stream,
        "messages": [
            { "role": "system", "content": request.system_instruction },
            { "role": "user", "content": request.message }
        ]
    });
    if wants_agent_json(request) {
        payload["response_format"] = json!({ "type": "json_object" });
    }
    payload
}

fn gemini_parts(request: &ChatRequest) -> Result<Vec<Value>, ChatError> {
    let mut parts = vec![json!({ "text": request.message })];
    for attachment in [&request.image_data_uri, &request.audio_data_uri]
        .into_iter()
        .flatten()
    {
        let payload = attachment
            .strip_prefix("data:")
            .and_then(|payload| payload.split_once(";base64,"))
            .filter(|(mime_type, data)| {
                (mime_type.starts_with("image/") || mime_type.starts_with("audio/"))
                    && !data.is_empty()
            })
            .ok_or(ChatError::InvalidAttachment)?;
        parts.push(json!({
            "inlineData": {
                "mimeType": payload.0,
                "data": payload.1
            }
        }));
    }
    Ok(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_missing_api_key() {
        let error = required_key("gemini", "").unwrap_err();
        assert!(matches!(error, ChatError::MissingApiKey(provider) if provider == "gemini"));
    }

    #[test]
    fn parses_stream_provider_formats() {
        assert_eq!(
            parse_stream_line(
                StreamFormat::OpenAi,
                r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#
            )
            .as_deref(),
            Some("hello")
        );
        assert_eq!(
            parse_stream_line(StreamFormat::Ollama, r#"{"message":{"content":"hi"}}"#).as_deref(),
            Some("hi")
        );
        assert_eq!(
            parse_stream_line(
                StreamFormat::Anthropic,
                r#"data: {"delta":{"type":"text_delta","text":"hey"}}"#
            )
            .as_deref(),
            Some("hey")
        );
    }

    #[test]
    fn builds_gemini_multimodal_parts() {
        let parts = gemini_parts(&ChatRequest {
            message: "describe".into(),
            system_instruction: String::new(),
            image_data_uri: Some("data:image/png;base64,aGk=".into()),
            audio_data_uri: None,
        })
        .unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
    }
}

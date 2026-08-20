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
        if provider.as_str() == config.ai_provider.as_str() {
            continue;
        }
        let alt = config_for_provider(config, &provider);
        if let Ok(mut r) = send_chat(&alt, request).await {
            r.fallback_provider = Some(config.ai_provider.clone());
            return Ok((r, Some(provider)));
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
        Err(e) => {
            eprintln!("Ollama stream chat failed, falling back: {:?}", e);
        }
    }
    for provider in config.available_providers() {
        if provider.as_str() == config.ai_provider.as_str() {
            continue;
        }
        let alt = config_for_provider(config, &provider);
        if let Ok(mut r) = stream_chat(&alt, request, &mut on_chunk).await {
            r.fallback_provider = Some(config.ai_provider.clone());
            return Ok((r, Some(provider)));
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
    pub chat_id: Option<String>,
    #[serde(default)]
    pub image_data_uri: Option<String>,
    #[serde(default)]
    pub audio_data_uri: Option<String>,
    #[serde(default)]
    pub video_data_uri: Option<String>,
    #[serde(default)]
    pub document_attachment: Option<DocumentAttachment>,
    #[serde(default)]
    pub workspace_path: Option<String>,
    #[serde(default)]
    pub agent_id: Option<String>,
    #[serde(default)]
    pub plan_mode: bool,
    /// Structured conversation history for native tool-calling. When set, provider
    /// adapters build the request from this instead of the flat `message` field.
    #[serde(default)]
    pub messages: Option<Vec<ChatMessage>>,
    /// Tool definitions to offer the model via each provider's native
    /// function/tool-calling API. Ignored when `messages` is `None`.
    #[serde(default)]
    pub tools: Option<Vec<ToolSpec>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
    Tool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: Vec<ContentBlock>,
}

impl ChatMessage {
    pub fn text(role: ChatRole, text: impl Into<String>) -> Self {
        ChatMessage {
            role,
            content: vec![ContentBlock::Text { text: text.into() }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: Value,
        /// See `ToolCall::thought_signature` — carried on `ToolUse` too so it
        /// survives being replayed back into `native_messages` history on the
        /// next turn. `None` for every provider but Gemini.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        thought_signature: Option<String>,
    },
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(default)]
        is_error: bool,
    },
    Image {
        data_uri: String,
    },
    Audio {
        data_uri: String,
    },
    Video {
        data_uri: String,
    },
}

/// A tool definition offered to the model via native function/tool-calling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolSpec {
    pub name: String,
    pub description: String,
    /// JSON schema (draft-7-ish, as accepted by the provider's tool-parameter format).
    pub parameters: Value,
}

/// A tool invocation the model requested via native function/tool-calling.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
    /// Gemini 3.x's "thinking" models attach an opaque `thoughtSignature` to
    /// each `functionCall` part; it must be echoed back verbatim on the next
    /// turn's replayed `functionCall` or the API rejects the request with a
    /// 400 (`INVALID_ARGUMENT: Function call is missing a thought_signature`).
    /// `None` for every other provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub thought_signature: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentAttachment {
    pub filename: String,
    pub data_uri: String,
}

const MAX_DOCUMENT_BYTES: usize = 10 * 1024 * 1024;
const MAX_DOCUMENT_CONTEXT_CHARS: usize = 400_000;
static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl ChatRequest {
    pub fn with_document_context(&self, config: &crate::MintConfig) -> Result<ChatRequest, String> {
        let Some(document) = &self.document_attachment else {
            return Ok(self.clone());
        };

        let (_mime_type, encoded) = document
            .data_uri
            .strip_prefix("data:")
            .and_then(|payload| payload.split_once(";base64,"))
            .ok_or_else(|| "invalid document attachment data URI".to_string())?;

        // Dispatch by the filename's extension, not the data URI's MIME type:
        // browsers report an empty or generic `application/octet-stream` type
        // for many of these formats (.md, .toml, .yaml, ...), so MIME type
        // isn't a reliable signal here. `extract_document_text` below performs
        // the real allowlist check and rejects anything it doesn't recognize.
        let extension = std::path::Path::new(document.filename.trim())
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .filter(|ext| !ext.is_empty())
            .unwrap_or_else(|| "txt".to_string());

        use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
        let bytes = BASE64_STANDARD
            .decode(encoded)
            .map_err(|error| format!("invalid document attachment encoding: {error}"))?;
        if bytes.len() > MAX_DOCUMENT_BYTES {
            return Err("document attachment is too large (> 10 MiB)".into());
        }

        let directory = crate::config::config_path()
            .map_err(|error| error.to_string())?
            .with_file_name("Attachments");
        std::fs::create_dir_all(&directory).map_err(|error| error.to_string())?;

        let safe_name: String = document
            .filename
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(*c, '.' | '-' | '_'))
            .take(80)
            .collect();
        let safe_name = if safe_name.is_empty() {
            "document".into()
        } else {
            safe_name
        };

        let path = directory.join(format!(
            "mint-doc-{}-{}.{}",
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::SeqCst),
            safe_name,
            extension
        ));
        std::fs::write(&path, bytes).map_err(|error| error.to_string())?;

        let extracted = crate::knowledge::extract_document_text(&path, config)
            .map_err(|error| error.to_string());
        let _ = std::fs::remove_file(&path);
        let extracted = extracted?;

        let mut context: String = extracted.chars().take(MAX_DOCUMENT_CONTEXT_CHARS).collect();
        if extracted.chars().count() > MAX_DOCUMENT_CONTEXT_CHARS {
            context.push_str("\n\n[Document text truncated because it is long.]");
        }

        let filename = document.filename.trim();
        let filename = if filename.is_empty() {
            "attached document"
        } else {
            filename
        };

        let mut enriched = self.clone();
        enriched.document_attachment = None;
        enriched.message = format!(
            "{}\n\nAttached document: {filename}\nUse the extracted document text below when answering. If the user asks for a summary, summarize this document.\n\n--- Extracted document text ---\n{context}\n--- End extracted document text ---",
            self.message
        );
        Ok(enriched)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatResponse {
    pub provider: String,
    pub model: String,
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_provider: Option<String>,
    /// Tool calls the model requested via native function/tool-calling, when the
    /// request set `ChatRequest.tools` and the provider honored it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    /// Provider-reported reason generation stopped (e.g. `"tool_use"`, `"end_turn"`,
    /// `"stop"`, `"max_tokens"`), when the provider reports one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stop_reason: Option<String>,
    /// Total tokens (input + output) the provider billed for this turn, when
    /// reported. Used to track how close a long-running conversation is to the
    /// model's context window, rather than estimating client-side.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
}

/// Internal, provider-agnostic result of a single non-streaming provider call.
/// `send_chat` maps this onto the public `ChatResponse`.
struct ProviderReply {
    model: String,
    text: String,
    tool_calls: Option<Vec<ToolCall>>,
    stop_reason: Option<String>,
    total_tokens: Option<u32>,
}

impl ProviderReply {
    fn text_only(model: String, text: String) -> Self {
        ProviderReply {
            model,
            text,
            tool_calls: None,
            stop_reason: None,
            total_tokens: None,
        }
    }
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
    let reply = match provider {
        "gemini" => call_gemini(&client, config, request).await?,
        "openai" | "local_openai" | "openrouter" | "deepseek" => {
            call_openai(&client, config, request).await?
        }
        "ollama" => call_ollama(&client, config, request).await?,
        "anthropic" => call_anthropic(&client, config, request).await?,
        "huggingface" => call_huggingface(&client, config, request).await?,
        p if p.starts_with("custom:") => call_custom_provider(&client, config, request).await?,
        other => return Err(ChatError::UnsupportedProvider(other.into())),
    };
    Ok(ChatResponse {
        provider: provider.into(),
        model: reply.model,
        text: reply.text,
        fallback_provider: None,
        tool_calls: reply.tool_calls,
        stop_reason: reply.stop_reason,
        total_tokens: reply.total_tokens,
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
        "openai" | "local_openai" | "openrouter" | "deepseek" => {
            stream_openai(&client, config, request, &mut on_chunk).await?
        }
        "ollama" => stream_ollama(&client, config, request, &mut on_chunk).await?,
        "anthropic" => stream_anthropic(&client, config, request, &mut on_chunk).await?,
        "huggingface" => stream_huggingface(&client, config, request, &mut on_chunk).await?,
        p if p.starts_with("custom:") => {
            stream_custom_provider(&client, config, request, &mut on_chunk).await?
        }
        other => return Err(ChatError::UnsupportedProvider(other.into())),
    };
    Ok(ChatResponse {
        provider: provider.into(),
        model,
        text,
        fallback_provider: None,
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
    })
}

async fn call_gemini(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let api_key = provider_key(&config.api_key, "GEMINI_API_KEY");
    required_key("gemini", &api_key)?;
    let model = config.gemini_model.clone();
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");

    let payload = if request.messages.is_some() {
        gemini_native_payload(request)?
    } else {
        gemini_chat_payload(config, request)?
    };

    let response: Value = client
        .post(url)
        .header("x-goog-api-key", api_key)
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_gemini_reply(model, &response)
}

/// Builds a Gemini `generateContent` payload from structured history, using
/// native `functionDeclarations`/`functionCall`/`functionResponse` instead of
/// the legacy `responseSchema` JSON-prompt scheme (see `gemini_chat_payload`).
fn gemini_native_payload(request: &ChatRequest) -> Result<Value, ChatError> {
    let messages = request.messages.as_deref().unwrap_or(&[]);
    let mut payload = json!({
        "systemInstruction": { "parts": [{ "text": request.system_instruction }] },
        "contents": gemini_native_contents(messages)?,
    });
    if let Some(tools) = &request.tools {
        payload["tools"] = json!([{
            "functionDeclarations": tools.iter().map(|tool| json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": tool.parameters,
            })).collect::<Vec<_>>()
        }]);
    }
    Ok(payload)
}

fn gemini_native_contents(messages: &[ChatMessage]) -> Result<Vec<Value>, ChatError> {
    // Gemini's `functionResponse` part is matched by function *name*, not by a
    // call id like Anthropic/OpenAI. Build an id -> name lookup from the
    // `ToolUse` blocks already in history so `ToolResult` blocks (which only
    // carry `tool_use_id`) can be translated correctly.
    let mut tool_names: std::collections::HashMap<&str, &str> = std::collections::HashMap::new();
    for message in messages {
        for block in &message.content {
            if let ContentBlock::ToolUse { id, name, .. } = block {
                tool_names.insert(id.as_str(), name.as_str());
            }
        }
    }
    messages
        .iter()
        .filter(|message| message.role != ChatRole::System)
        .map(|message| {
            let role = match message.role {
                ChatRole::Assistant => "model",
                _ => "user",
            };
            let parts = message
                .content
                .iter()
                .map(|block| gemini_native_part(block, &tool_names))
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({ "role": role, "parts": parts }))
        })
        .collect()
}

fn gemini_native_part(
    block: &ContentBlock,
    tool_names: &std::collections::HashMap<&str, &str>,
) -> Result<Value, ChatError> {
    Ok(match block {
        ContentBlock::Text { text } => json!({ "text": text }),
        ContentBlock::ToolUse {
            name,
            input,
            thought_signature,
            ..
        } => {
            // A `thoughtSignature` captured off the model's own `functionCall`
            // part must be echoed back verbatim here — a sibling key of
            // `functionCall`, not nested inside it — or Gemini 3.x rejects the
            // request outright (400 INVALID_ARGUMENT: "Function call is
            // missing a thought_signature"), which without this looked like a
            // recoverable provider error and silently triggered a fallback to
            // a different (much weaker) provider on every second tool-calling
            // turn. See `ToolCall::thought_signature`.
            let mut part = json!({ "functionCall": { "name": name, "args": input } });
            if let Some(signature) = thought_signature {
                part["thoughtSignature"] = json!(signature);
            }
            part
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            ..
        } => {
            let name = tool_names
                .get(tool_use_id.as_str())
                .copied()
                .unwrap_or(tool_use_id.as_str());
            json!({ "functionResponse": { "name": name, "response": { "result": content } } })
        }
        ContentBlock::Image { data_uri } => {
            let (mime_type, data) = split_data_uri(data_uri, "image/")?;
            json!({ "inlineData": { "mimeType": mime_type, "data": data } })
        }
        ContentBlock::Audio { data_uri } => {
            let (mime_type, data) = split_data_uri(data_uri, "audio/")?;
            json!({ "inlineData": { "mimeType": mime_type, "data": data } })
        }
        ContentBlock::Video { data_uri } => {
            let (mime_type, data) = split_data_uri(data_uri, "video/")?;
            json!({ "inlineData": { "mimeType": mime_type, "data": data } })
        }
    })
}

/// Parses a Gemini `generateContent` response, collecting text parts and any
/// `functionCall` parts into tool calls. Gemini doesn't assign call ids, so a
/// stable per-response index (`call_0`, `call_1`, ...) is synthesized; this is
/// safe because `gemini_native_part` resolves `functionResponse` back to the
/// function *name*, not the id.
fn parse_gemini_reply(model: String, response: &Value) -> Result<ProviderReply, ChatError> {
    let parts = response["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if let Some(t) = part["text"].as_str() {
            text.push_str(t);
        }
        if let Some(call) = part.get("functionCall") {
            tool_calls.push(ToolCall {
                id: format!("call_{index}"),
                name: call["name"].as_str().unwrap_or_default().to_string(),
                input: call["args"].clone(),
                // Sibling of `functionCall` within the same `part`, not nested
                // inside it — see the module-level doc on `ToolCall::thought_signature`.
                thought_signature: part
                    .get("thoughtSignature")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
        }
    }
    if text.is_empty() && tool_calls.is_empty() {
        return Err(ChatError::MissingResponseText);
    }
    Ok(ProviderReply {
        model,
        text,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        stop_reason: response["candidates"][0]["finishReason"]
            .as_str()
            .map(str::to_owned),
        total_tokens: response["usageMetadata"]["totalTokenCount"]
            .as_u64()
            .map(|n| n as u32),
    })
}

async fn call_openai(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let local = config.ai_provider == "local_openai";
    let openrouter = config.ai_provider == "openrouter";
    let deepseek = config.ai_provider == "deepseek";
    let api_key = if openrouter {
        provider_key(&config.openrouter_api_key, "OPENROUTER_API_KEY")
    } else if deepseek {
        provider_key(&config.deepseek_api_key, "DEEPSEEK_API_KEY")
    } else {
        provider_key(&config.openai_api_key, "OPENAI_API_KEY")
    };
    if openrouter {
        required_key("openrouter", &api_key)?;
    } else if deepseek {
        required_key("deepseek", &api_key)?;
    } else if !local {
        required_key("openai", &api_key)?;
    }
    let base_url = if openrouter {
        "https://openrouter.ai/api/v1"
    } else if deepseek {
        "https://api.deepseek.com"
    } else if local {
        config.local_api_base_url.trim_end_matches('/')
    } else {
        "https://api.openai.com/v1"
    };
    let model = if openrouter {
        config.openrouter_model.clone()
    } else if deepseek {
        config.deepseek_model.clone()
    } else if local {
        config.local_model_name.clone()
    } else {
        config.openai_model.clone()
    };
    let response: Value = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(if local { "not-needed" } else { &api_key })
        .json(&openai_chat_payload(&model, request, false)?)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_openai_style_reply(model, &response)
}

/// Parses an OpenAI Chat Completions-shaped response (`choices[0].message`),
/// extracting both plain text and any native `tool_calls`. Shared by OpenAI,
/// custom OpenAI-compatible providers, and (for its response shape only) reused
/// by nothing else — Ollama's native `/api/chat` response is shaped differently
/// and has its own parser below.
fn parse_openai_style_reply(model: String, response: &Value) -> Result<ProviderReply, ChatError> {
    let message = &response["choices"][0]["message"];
    let text = message["content"].as_str().unwrap_or_default().to_string();
    let tool_calls: Vec<ToolCall> = message["tool_calls"]
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .filter_map(|call| {
                    let name = call["function"]["name"].as_str()?.to_string();
                    let input = call["function"]["arguments"]
                        .as_str()
                        .and_then(|raw| serde_json::from_str(raw).ok())
                        .unwrap_or_else(|| json!({}));
                    Some(ToolCall {
                        id: call["id"].as_str().unwrap_or_default().to_string(),
                        name,
                        input,
                        thought_signature: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if text.is_empty() && tool_calls.is_empty() {
        return Err(ChatError::MissingResponseText);
    }
    Ok(ProviderReply {
        model,
        text,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        stop_reason: response["choices"][0]["finish_reason"]
            .as_str()
            .map(str::to_owned),
        total_tokens: response["usage"]["total_tokens"].as_u64().map(|n| n as u32),
    })
}

async fn call_ollama(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let host = if config.ollama_host.trim().is_empty() {
        "http://localhost:11434"
    } else {
        config.ollama_host.trim_end_matches('/')
    };
    let model = config.ollama_model.clone();
    let native = request.messages.is_some()
        && config.tool_calling_mode() == crate::config::ToolCallingMode::Native;
    let body = if native {
        let messages = request.messages.as_deref().unwrap_or(&[]);
        let mut payload = json!({
            "model": model,
            "stream": false,
            "messages": ollama_native_messages(messages),
        });
        if let Some(tools) = &request.tools {
            payload["tools"] = json!(
                tools
                    .iter()
                    .map(|tool| json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.parameters,
                        }
                    }))
                    .collect::<Vec<_>>()
            );
        }
        payload
    } else {
        let mut user_message = json!({ "role": "user", "content": request.message });
        if let Some(images) = ollama_images(request) {
            user_message["images"] = json!(images);
        }
        json!({
            "model": model,
            "stream": false,
            "messages": [
                { "role": "system", "content": request.system_instruction },
                user_message
            ]
        })
    };
    let response: Value = client
        .post(format!("{host}/api/chat"))
        .json(&body)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_ollama_reply(model, &response)
}

/// Converts structured history into Ollama's native `/api/chat` message shape.
/// Unlike OpenAI, Ollama expects tool-call `arguments` as a JSON object (not a
/// stringified JSON blob) and doesn't require a `tool_call_id` on tool-result
/// messages.
fn ollama_native_messages(messages: &[ChatMessage]) -> Vec<Value> {
    let mut result = Vec::new();
    for message in messages {
        match message.role {
            ChatRole::System => {
                result.push(json!({ "role": "system", "content": message_text(message) }));
            }
            ChatRole::User => {
                let mut entry = json!({ "role": "user", "content": message_text(message) });
                let images: Vec<String> = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::Image { data_uri } => split_data_uri(data_uri, "image/")
                            .ok()
                            .map(|(_, data)| data),
                        _ => None,
                    })
                    .collect();
                if !images.is_empty() {
                    entry["images"] = json!(images);
                }
                result.push(entry);
            }
            ChatRole::Assistant => {
                let text = message_text(message);
                let tool_calls: Vec<Value> = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolUse { name, input, .. } => {
                            Some(json!({ "function": { "name": name, "arguments": input } }))
                        }
                        _ => None,
                    })
                    .collect();
                let mut entry = json!({ "role": "assistant", "content": text });
                if !tool_calls.is_empty() {
                    entry["tool_calls"] = json!(tool_calls);
                }
                result.push(entry);
            }
            ChatRole::Tool => {
                // Ollama's `/api/chat` `tool` role message has no image field —
                // like OpenAI, an `Image` block riding along with a tool result
                // (e.g. a browser screenshot) is emitted as a follow-up `user`
                // message using the same `images` field regular user turns use.
                let mut pending_images: Vec<String> = Vec::new();
                for block in &message.content {
                    match block {
                        ContentBlock::ToolResult { content, .. } => {
                            result.push(json!({ "role": "tool", "content": content }));
                        }
                        ContentBlock::Image { data_uri } => {
                            if let Ok((_, data)) = split_data_uri(data_uri, "image/") {
                                pending_images.push(data);
                            }
                        }
                        _ => {}
                    }
                }
                if !pending_images.is_empty() {
                    result.push(json!({
                        "role": "user",
                        "content": "",
                        "images": pending_images,
                    }));
                }
            }
        }
    }
    result
}

/// Parses Ollama's native `/api/chat` response (`{"message": {...}}`), which is
/// shaped differently from the OpenAI Chat Completions envelope
/// (`{"choices": [{"message": {...}}]}`) that `parse_openai_style_reply` handles.
fn parse_ollama_reply(model: String, response: &Value) -> Result<ProviderReply, ChatError> {
    let message = &response["message"];
    let text = message["content"].as_str().unwrap_or_default().to_string();
    let tool_calls: Vec<ToolCall> = message["tool_calls"]
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .enumerate()
                .filter_map(|(index, call)| {
                    let name = call["function"]["name"].as_str()?.to_string();
                    let arguments = &call["function"]["arguments"];
                    let input = if let Some(raw) = arguments.as_str() {
                        serde_json::from_str(raw).unwrap_or_else(|_| json!({}))
                    } else {
                        arguments.clone()
                    };
                    let id = call["id"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| format!("call_{index}"));
                    Some(ToolCall {
                        id,
                        name,
                        input,
                        thought_signature: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    if text.is_empty() && tool_calls.is_empty() {
        return Err(ChatError::MissingResponseText);
    }
    let total_tokens = match (
        response["prompt_eval_count"].as_u64(),
        response["eval_count"].as_u64(),
    ) {
        (Some(prompt), Some(completion)) => Some((prompt + completion) as u32),
        _ => None,
    };
    Ok(ProviderReply {
        model,
        text,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        stop_reason: None,
        total_tokens,
    })
}

async fn call_anthropic(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let api_key = provider_key(&config.anthropic_api_key, "ANTHROPIC_API_KEY");
    required_key("anthropic", &api_key)?;
    let model = config.anthropic_model.clone();
    let payload = anthropic_chat_payload(&model, request, false)?;
    let response: Value = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_anthropic_reply(model, &response)
}

/// Builds the Anthropic Messages API payload. When `request.messages` is set,
/// uses the structured history and (if present) `request.tools` for native
/// tool-calling; otherwise falls back to the single flat `message` field exactly
/// as before (legacy JSON-prompt callers are unaffected).
fn anthropic_chat_payload(
    model: &str,
    request: &ChatRequest,
    stream: bool,
) -> Result<Value, ChatError> {
    let system = anthropic_system_blocks(&request.system_instruction);
    let mut payload = if let Some(messages) = &request.messages {
        json!({
            "model": model,
            "max_tokens": 8192,
            "system": system,
            "messages": anthropic_messages(messages)?,
        })
    } else {
        json!({
            "model": model,
            "max_tokens": 8192,
            "system": system,
            "messages": [{
                "role": "user",
                "content": anthropic_message_content(&request.message, request.image_data_uri.as_deref())?,
            }]
        })
    };
    if stream {
        payload["stream"] = json!(true);
    }
    if let Some(tools) = &request.tools {
        let last_idx = tools.len().saturating_sub(1);
        payload["tools"] = json!(
            tools
                .iter()
                .enumerate()
                .map(|(i, tool)| {
                    let mut value = json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.parameters,
                    });
                    // The tool catalog is identical on every step of an agent
                    // run — marking the last tool definition as a cache
                    // breakpoint caches the whole system+tools prefix (Anthropic
                    // caches everything up to and including a marked block), so
                    // repeated steps hit cache instead of reprocessing it.
                    if i == last_idx {
                        value["cache_control"] = json!({ "type": "ephemeral" });
                    }
                    value
                })
                .collect::<Vec<_>>()
        );
    }
    Ok(payload)
}

/// Anthropic requires `system` to be an array of text blocks (not a bare
/// string) to attach a `cache_control` breakpoint to it. The system prompt is
/// identical on every step of an agent run, so marking it cacheable turns
/// repeated multi-step turns into a cache hit instead of a full reprocess
/// each time. Falls back to the old bare-string shape when there's no system
/// prompt at all, since an empty cacheable block isn't useful.
fn anthropic_system_blocks(system_instruction: &str) -> Value {
    if system_instruction.is_empty() {
        return json!(system_instruction);
    }
    json!([{
        "type": "text",
        "text": system_instruction,
        "cache_control": { "type": "ephemeral" },
    }])
}

fn anthropic_messages(messages: &[ChatMessage]) -> Result<Vec<Value>, ChatError> {
    messages
        .iter()
        .filter(|message| message.role != ChatRole::System)
        .map(|message| {
            let role = match message.role {
                ChatRole::Assistant => "assistant",
                _ => "user",
            };
            let content = message
                .content
                .iter()
                .map(anthropic_content_block)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(json!({ "role": role, "content": content }))
        })
        .collect()
}

/// Builds an Anthropic `content` value for the plain (non-native-messages)
/// chat path: bare text when there's no image, otherwise a `text` + `image`
/// parts array. Mirrors `anthropic_content_block`, which handles the same
/// shape for the native-tool-calling `messages` path. `image_data_uri` may
/// contain multiple space-separated data URIs, matching Gemini's `gemini_parts`.
fn anthropic_message_content(text: &str, image_data_uri: Option<&str>) -> Result<Value, ChatError> {
    let Some(image_data) = image_data_uri else {
        return Ok(json!(text));
    };
    let mut parts = vec![json!({ "type": "text", "text": text })];
    for img in image_data.split_whitespace() {
        let (mime_type, data) = split_data_uri(img, "image/")?;
        parts.push(json!({
            "type": "image",
            "source": { "type": "base64", "media_type": mime_type, "data": data },
        }));
    }
    Ok(json!(parts))
}

fn anthropic_content_block(block: &ContentBlock) -> Result<Value, ChatError> {
    Ok(match block {
        ContentBlock::Text { text } => json!({ "type": "text", "text": text }),
        ContentBlock::ToolUse {
            id, name, input, ..
        } => {
            json!({ "type": "tool_use", "id": id, "name": name, "input": input })
        }
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => json!({
            "type": "tool_result",
            "tool_use_id": tool_use_id,
            "content": content,
            "is_error": is_error,
        }),
        ContentBlock::Image { data_uri } => {
            let (mime_type, data) = split_data_uri(data_uri, "image/")?;
            json!({
                "type": "image",
                "source": { "type": "base64", "media_type": mime_type, "data": data },
            })
        }
        // Anthropic's Messages API has no inline audio/video content block; this
        // is unreachable in practice because `require_supported_attachments`
        // rejects audio/video for "anthropic" before a request gets this far.
        ContentBlock::Audio { .. } | ContentBlock::Video { .. } => {
            return Err(ChatError::UnsupportedAttachments("anthropic".into()));
        }
    })
}

/// Parses an Anthropic Messages API response, collecting all `text` and
/// `tool_use` content blocks (Anthropic may return a `tool_use` block before,
/// after, or interleaved with text — unlike the old code, this does not assume
/// the first block is text).
fn parse_anthropic_reply(model: String, response: &Value) -> Result<ProviderReply, ChatError> {
    let blocks = response["content"].as_array().cloned().unwrap_or_default();
    let mut text = String::new();
    let mut tool_calls = Vec::new();
    for block in &blocks {
        match block["type"].as_str() {
            Some("text") => {
                if let Some(t) = block["text"].as_str() {
                    text.push_str(t);
                }
            }
            Some("tool_use") => {
                tool_calls.push(ToolCall {
                    id: block["id"].as_str().unwrap_or_default().into(),
                    name: block["name"].as_str().unwrap_or_default().into(),
                    input: block["input"].clone(),
                    thought_signature: None,
                });
            }
            _ => {}
        }
    }
    if text.is_empty() && tool_calls.is_empty() {
        return Err(ChatError::MissingResponseText);
    }
    let total_tokens = match (
        response["usage"]["input_tokens"].as_u64(),
        response["usage"]["output_tokens"].as_u64(),
    ) {
        (Some(input), Some(output)) => Some((input + output) as u32),
        _ => None,
    };
    Ok(ProviderReply {
        model,
        text,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        stop_reason: response["stop_reason"].as_str().map(str::to_owned),
        total_tokens,
    })
}

async fn call_huggingface(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
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
    let text = response["choices"][0]["message"]["content"]
        .as_str()
        .ok_or(ChatError::MissingResponseText)?
        .into();
    Ok(ProviderReply::text_only(model, text))
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
    let openrouter = config.ai_provider == "openrouter";
    let deepseek = config.ai_provider == "deepseek";
    let api_key = if openrouter {
        provider_key(&config.openrouter_api_key, "OPENROUTER_API_KEY")
    } else if deepseek {
        provider_key(&config.deepseek_api_key, "DEEPSEEK_API_KEY")
    } else {
        provider_key(&config.openai_api_key, "OPENAI_API_KEY")
    };
    if openrouter {
        required_key("openrouter", &api_key)?;
    } else if deepseek {
        required_key("deepseek", &api_key)?;
    } else if !local {
        required_key("openai", &api_key)?;
    }
    let base_url = if openrouter {
        "https://openrouter.ai/api/v1"
    } else if deepseek {
        "https://api.deepseek.com"
    } else if local {
        config.local_api_base_url.trim_end_matches('/')
    } else {
        "https://api.openai.com/v1"
    };
    let model = if openrouter {
        config.openrouter_model.clone()
    } else if deepseek {
        config.deepseek_model.clone()
    } else if local {
        config.local_model_name.clone()
    } else {
        config.openai_model.clone()
    };
    let response = client
        .post(format!("{base_url}/chat/completions"))
        .bearer_auth(if local { "not-needed" } else { &api_key })
        .json(&openai_chat_payload(&model, request, true)?)
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
    let mut user_message = json!({ "role": "user", "content": request.message });
    if let Some(images) = ollama_images(request) {
        user_message["images"] = json!(images);
    }
    let response = client
        .post(format!("{host}/api/chat"))
        .json(&json!({
            "model": model,
            "stream": true,
            "messages": [
                { "role": "system", "content": request.system_instruction },
                user_message
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

/// Resolve which model to use for the current custom provider.
///
/// Checks `config.extra["customModelSelections"]` first (set by the UI when the
/// user picks a specific model from the multi-model dropdown), then falls back to
/// the first model defined in the `CustomProvider`.
fn resolve_custom_model(config: &crate::MintConfig) -> String {
    let provider_id = config
        .ai_provider
        .strip_prefix("custom:")
        .unwrap_or(&config.ai_provider);

    // Check if a per-provider model selection was persisted in extra config.
    if let Some(selections) = config
        .extra
        .get("customModelSelections")
        .and_then(|v| v.as_object())
        && let Some(model) = selections.get(provider_id).and_then(|v| v.as_str())
        && !model.is_empty()
    {
        return model.to_owned();
    }

    // Fall back to the first model in the provider definition.
    config
        .resolve_custom_provider(&config.ai_provider)
        .and_then(|cp| cp.models.first())
        .map(|m| m.model_id.clone())
        .unwrap_or_default()
}

async fn call_custom_provider(
    client: &Client,
    config: &crate::MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let provider = config
        .resolve_custom_provider(&config.ai_provider)
        .ok_or_else(|| ChatError::UnsupportedProvider(config.ai_provider.clone()))?;

    let base_url = provider.base_url.trim_end_matches('/');
    let model = resolve_custom_model(config);
    let payload = openai_chat_payload(&model, request, false)?;

    let mut builder = client
        .post(format!("{base_url}/chat/completions"))
        .json(&payload);

    if !provider.api_key.is_empty() {
        builder = builder.bearer_auth(&provider.api_key);
    }
    for h in &provider.headers {
        if !h.name.is_empty() {
            builder = builder.header(h.name.as_str(), h.value.as_str());
        }
    }

    let response: serde_json::Value = builder.send().await?.error_for_status()?.json().await?;

    parse_openai_style_reply(model, &response)
}

async fn stream_custom_provider<F>(
    client: &Client,
    config: &crate::MintConfig,
    request: &ChatRequest,
    on_chunk: &mut F,
) -> Result<(String, String), ChatError>
where
    F: FnMut(String),
{
    let provider = config
        .resolve_custom_provider(&config.ai_provider)
        .ok_or_else(|| ChatError::UnsupportedProvider(config.ai_provider.clone()))?;

    let base_url = provider.base_url.trim_end_matches('/');
    let model = resolve_custom_model(config);
    let payload = openai_chat_payload(&model, request, true)?;

    let mut builder = client
        .post(format!("{base_url}/chat/completions"))
        .json(&payload);

    if !provider.api_key.is_empty() {
        builder = builder.bearer_auth(&provider.api_key);
    }
    for h in &provider.headers {
        if !h.name.is_empty() {
            builder = builder.header(h.name.as_str(), h.value.as_str());
        }
    }

    let response = builder.send().await?.error_for_status()?;
    collect_stream(response, StreamFormat::OpenAi, on_chunk)
        .await
        .map(|text| (model, text))
}

fn provider_key(configured: &str, environment_variable: &str) -> String {
    if configured.trim().is_empty() {
        std::env::var(environment_variable).unwrap_or_default()
    } else {
        configured.into()
    }
}

fn require_supported_attachments(provider: &str, request: &ChatRequest) -> Result<(), ChatError> {
    let has_image = request
        .image_data_uri
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_audio = request
        .audio_data_uri
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());
    let has_video = request
        .video_data_uri
        .as_ref()
        .is_some_and(|s| !s.trim().is_empty());

    match provider {
        "gemini" => Ok(()),
        "ollama" if has_audio || has_video => {
            Err(ChatError::UnsupportedAttachments(provider.into()))
        }
        "ollama" => Ok(()),
        // The real OpenAI API supports audio input natively (`input_audio` content
        // parts, see `openai_audio_part`) — video still isn't implemented for it.
        "openai" if has_video => Err(ChatError::UnsupportedAttachments(provider.into())),
        "openai" => Ok(()),
        // These all build their payload via `openai_chat_payload`/`anthropic_chat_payload`,
        // which support image attachments (see `openai_message_content`/
        // `anthropic_message_content`). Unlike literal `"openai"` above, these are
        // separate services/proxies that aren't confirmed to speak the same
        // `input_audio` schema, so audio/video stay unsupported for them.
        "local_openai" | "openrouter" | "deepseek" | "huggingface" | "anthropic"
            if has_audio || has_video =>
        {
            Err(ChatError::UnsupportedAttachments(provider.into()))
        }
        "local_openai" | "openrouter" | "deepseek" | "huggingface" | "anthropic" => Ok(()),
        // Custom providers route through `call_custom_provider`, which also uses
        // `openai_chat_payload`, so images are supported the same way; audio/video
        // are not implemented for them yet.
        p if p.starts_with("custom:") && (has_audio || has_video) => {
            Err(ChatError::UnsupportedAttachments(provider.into()))
        }
        p if p.starts_with("custom:") => Ok(()),
        _ if has_image || has_audio || has_video => {
            Err(ChatError::UnsupportedAttachments(provider.into()))
        }
        _ => Ok(()),
    }
}

/// Extract base64 image data from the request for Ollama's `images` field.
/// Ollama expects a plain array of base64 strings (without the data URI prefix).
fn ollama_images(request: &ChatRequest) -> Option<Vec<String>> {
    let image_data = request.image_data_uri.as_ref()?;
    let images: Vec<String> = image_data
        .split_whitespace()
        .filter_map(|img| {
            img.strip_prefix("data:")
                .and_then(|payload| payload.split_once(";base64,"))
                .filter(|(mime, data)| mime.starts_with("image/") && !data.is_empty())
                .map(|(_, data)| data.to_owned())
        })
        .collect();
    if images.is_empty() {
        None
    } else {
        Some(images)
    }
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
        "weather",
        "stock",
        "calculation",
        "browser_open",
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
                        "query": { "type": "STRING", "description": "The search query string (required for search_code, semantic_search, knowledge_search, web_search, stock, memory_recall)" },
                        "city": { "type": "STRING", "description": "The target city or location name (required for weather)" },
                        "expression": { "type": "STRING", "description": "The math expression or percentage calculation (required for calculation)" },
                        "url": { "type": "STRING", "description": "The target website URL to open (required for browser_open)" },
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

fn openai_chat_payload(
    model: &str,
    request: &ChatRequest,
    stream: bool,
) -> Result<Value, ChatError> {
    let messages = if let Some(messages) = &request.messages {
        let mut built = Vec::new();
        if !request.system_instruction.is_empty()
            && !messages.iter().any(|m| m.role == ChatRole::System)
        {
            built.push(json!({ "role": "system", "content": request.system_instruction }));
        }
        built.extend(openai_native_messages(messages)?);
        built
    } else {
        vec![
            json!({ "role": "system", "content": request.system_instruction }),
            json!({
                "role": "user",
                "content": openai_message_content(
                    &request.message,
                    request.image_data_uri.as_deref(),
                    request.audio_data_uri.as_deref(),
                )?,
            }),
        ]
    };
    let mut payload = json!({
        "model": model,
        "stream": stream,
        "messages": messages,
    });
    if wants_agent_json(request) {
        payload["response_format"] = json!({ "type": "json_object" });
    }
    if let Some(tools) = &request.tools {
        payload["tools"] = json!(
            tools
                .iter()
                .map(|tool| json!({
                    "type": "function",
                    "function": {
                        "name": tool.name,
                        "description": tool.description,
                        "parameters": tool.parameters,
                    }
                }))
                .collect::<Vec<_>>()
        );
    }
    Ok(payload)
}

/// Converts structured history into OpenAI Chat Completions message objects,
/// including assistant `tool_calls` and `role: "tool"` tool-result messages.
fn openai_native_messages(messages: &[ChatMessage]) -> Result<Vec<Value>, ChatError> {
    let mut result = Vec::new();
    for message in messages {
        match message.role {
            ChatRole::System => {
                result.push(json!({ "role": "system", "content": message_text(message) }));
            }
            ChatRole::User => {
                result.push(json!({ "role": "user", "content": openai_user_content(message)? }));
            }
            ChatRole::Assistant => {
                let text = message_text(message);
                let tool_calls: Vec<Value> = message
                    .content
                    .iter()
                    .filter_map(|block| match block {
                        ContentBlock::ToolUse {
                            id, name, input, ..
                        } => Some(json!({
                            "id": id,
                            "type": "function",
                            "function": { "name": name, "arguments": input.to_string() }
                        })),
                        _ => None,
                    })
                    .collect();
                let mut entry = json!({ "role": "assistant" });
                entry["content"] = if text.is_empty() {
                    Value::Null
                } else {
                    json!(text)
                };
                if !tool_calls.is_empty() {
                    entry["tool_calls"] = json!(tool_calls);
                }
                result.push(entry);
            }
            ChatRole::Tool => {
                // OpenAI's Chat Completions `tool` role only accepts a plain
                // string `content` — an image can't be attached to it directly.
                // Any `Image` block riding along with a tool result (e.g. a
                // browser screenshot) is instead emitted as a follow-up `user`
                // message, the same pattern OpenAI's own vision examples use to
                // let a tool result be "shown" to the model.
                let mut pending_images: Vec<&str> = Vec::new();
                for block in &message.content {
                    match block {
                        ContentBlock::ToolResult {
                            tool_use_id,
                            content,
                            ..
                        } => {
                            result.push(json!({
                                "role": "tool",
                                "tool_call_id": tool_use_id,
                                "content": content,
                            }));
                        }
                        ContentBlock::Image { data_uri } => pending_images.push(data_uri),
                        _ => {}
                    }
                }
                if !pending_images.is_empty() {
                    let parts: Vec<Value> = pending_images
                        .iter()
                        .map(|data_uri| {
                            json!({ "type": "image_url", "image_url": { "url": data_uri } })
                        })
                        .collect();
                    result.push(json!({ "role": "user", "content": parts }));
                }
            }
        }
    }
    Ok(result)
}

/// Concatenates all `Text` blocks in a message (tool-use/result/image blocks
/// contribute nothing).
fn message_text(message: &ChatMessage) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("")
}

/// Builds an OpenAI `content` value for the plain (non-native-messages) chat
/// path: bare text when there's no image/audio, otherwise a `text` +
/// `image_url`/`input_audio` parts array. Mirrors `openai_user_content`,
/// which handles the same shape for the native-tool-calling `messages` path.
/// `image_data_uri`/`audio_data_uri` may each contain multiple
/// space-separated data URIs, matching Gemini's `gemini_parts`.
fn openai_message_content(
    text: &str,
    image_data_uri: Option<&str>,
    audio_data_uri: Option<&str>,
) -> Result<Value, ChatError> {
    if image_data_uri.is_none() && audio_data_uri.is_none() {
        return Ok(json!(text));
    }
    let mut parts = vec![json!({ "type": "text", "text": text })];
    for img in image_data_uri.into_iter().flat_map(str::split_whitespace) {
        parts.push(json!({ "type": "image_url", "image_url": { "url": img } }));
    }
    for aud in audio_data_uri.into_iter().flat_map(str::split_whitespace) {
        parts.push(openai_audio_part(aud)?);
    }
    Ok(json!(parts))
}

fn openai_user_content(message: &ChatMessage) -> Result<Value, ChatError> {
    if let [ContentBlock::Text { text }] = message.content.as_slice() {
        return Ok(json!(text));
    }
    let parts: Vec<Value> = message
        .content
        .iter()
        .map(|block| -> Result<Value, ChatError> {
            Ok(match block {
                ContentBlock::Text { text } => json!({ "type": "text", "text": text }),
                ContentBlock::Image { data_uri } => {
                    json!({ "type": "image_url", "image_url": { "url": data_uri } })
                }
                ContentBlock::Audio { data_uri } => openai_audio_part(data_uri)?,
                // OpenAI Chat Completions has no inline video content part in this
                // format; unreachable in practice because
                // `require_supported_attachments` rejects video for these
                // providers before a request gets this far. `Value::Null` here
                // is a "skip this block" sentinel, filtered out below.
                ContentBlock::ToolUse { .. }
                | ContentBlock::ToolResult { .. }
                | ContentBlock::Video { .. } => Value::Null,
            })
        })
        .filter_map(|part| match part {
            Ok(Value::Null) => None,
            other => Some(other),
        })
        .collect::<Result<_, _>>()?;
    Ok(json!(parts))
}

/// Builds an OpenAI `input_audio` content part. Unlike `image_url`, which
/// takes the whole `data:` URI as-is, OpenAI's audio input wants the base64
/// payload and format split apart, so the URI has to be parsed here instead
/// of just passed through.
fn openai_audio_part(data_uri: &str) -> Result<Value, ChatError> {
    let (mime_type, data) = split_data_uri(data_uri, "audio/")?;
    Ok(json!({
        "type": "input_audio",
        "input_audio": { "data": data, "format": openai_audio_format(&mime_type) }
    }))
}

/// OpenAI's `input_audio.format` only accepts `"wav"` or `"mp3"` — anything
/// else (webm, ogg, m4a, …) is mapped to `"mp3"` as the closest-effort
/// fallback rather than rejected outright; the API itself is the final judge
/// of whether the actual bytes decode.
fn openai_audio_format(mime_type: &str) -> &'static str {
    if mime_type.contains("wav") {
        "wav"
    } else {
        "mp3"
    }
}

/// Splits a `data:<mime>;base64,<data>` URI into `(mime_type, base64_data)`,
/// validating the mime type starts with `expected_prefix` (e.g. `"image/"`).
fn split_data_uri(data_uri: &str, expected_prefix: &str) -> Result<(String, String), ChatError> {
    data_uri
        .strip_prefix("data:")
        .and_then(|payload| payload.split_once(";base64,"))
        .filter(|(mime, data)| mime.starts_with(expected_prefix) && !data.is_empty())
        .map(|(mime, data)| (mime.to_owned(), data.to_owned()))
        .ok_or(ChatError::InvalidAttachment)
}

fn gemini_parts(request: &ChatRequest) -> Result<Vec<Value>, ChatError> {
    let mut parts = vec![json!({ "text": request.message })];
    if let Some(ref image_data) = request.image_data_uri {
        for img in image_data.split_whitespace() {
            let payload = img
                .strip_prefix("data:")
                .and_then(|payload| payload.split_once(";base64,"))
                .filter(|(mime_type, data)| mime_type.starts_with("image/") && !data.is_empty())
                .ok_or(ChatError::InvalidAttachment)?;
            parts.push(json!({
                "inlineData": {
                    "mimeType": payload.0,
                    "data": payload.1
                }
            }));
        }
    }
    if let Some(ref audio_data) = request.audio_data_uri {
        for aud in audio_data.split_whitespace() {
            let payload = aud
                .strip_prefix("data:")
                .and_then(|payload| payload.split_once(";base64,"))
                .filter(|(mime_type, data)| mime_type.starts_with("audio/") && !data.is_empty())
                .ok_or(ChatError::InvalidAttachment)?;
            parts.push(json!({
                "inlineData": {
                    "mimeType": payload.0,
                    "data": payload.1
                }
            }));
        }
    }
    if let Some(ref video_data) = request.video_data_uri {
        for vid in video_data.split_whitespace() {
            let payload = vid
                .strip_prefix("data:")
                .and_then(|payload| payload.split_once(";base64,"))
                .filter(|(mime_type, data)| mime_type.starts_with("video/") && !data.is_empty())
                .ok_or(ChatError::InvalidAttachment)?;
            parts.push(json!({
                "inlineData": {
                    "mimeType": payload.0,
                    "data": payload.1
                }
            }));
        }
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
    fn rejects_video_attachments_for_non_gemini_providers() {
        let request = ChatRequest {
            message: "describe this video".into(),
            system_instruction: String::new(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: Some("data:video/mp4;base64,AAAA".into()),
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        };

        let error = require_supported_attachments("openai", &request).unwrap_err();
        assert!(
            matches!(error, ChatError::UnsupportedAttachments(provider) if provider == "openai")
        );
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
            chat_id: None,
            image_data_uri: Some("data:image/png;base64,aGk= data:image/jpeg;base64,Ynl5".into()),
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        })
        .unwrap();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[1]["inlineData"]["mimeType"], "image/png");
        assert_eq!(parts[2]["inlineData"]["mimeType"], "image/jpeg");
    }

    fn sample_tool() -> ToolSpec {
        ToolSpec {
            name: "read_file".into(),
            description: "Reads a file".into(),
            parameters: json!({
                "type": "object",
                "properties": { "path": { "type": "string" } },
                "required": ["path"]
            }),
        }
    }

    fn native_request(messages: Vec<ChatMessage>, tools: Option<Vec<ToolSpec>>) -> ChatRequest {
        ChatRequest {
            message: String::new(),
            system_instruction: "You are Mint.".into(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: Some(messages),
            tools,
        }
    }

    #[test]
    fn anthropic_native_payload_includes_tools_and_tool_result_messages() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "read main.rs"),
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "read_file".into(),
                    input: json!({ "path": "main.rs" }),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "fn main() {}".into(),
                    is_error: false,
                }],
            },
        ];
        let request = native_request(messages, Some(vec![sample_tool()]));
        let payload = anthropic_chat_payload("claude-x", &request, false).unwrap();

        assert_eq!(payload["system"][0]["text"], "You are Mint.");
        assert_eq!(payload["tools"][0]["name"], "read_file");
        assert_eq!(payload["tools"][0]["input_schema"]["type"], "object");
        let msgs = payload["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"][0]["type"], "tool_use");
        assert_eq!(msgs[2]["role"], "user");
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "call_1");
    }

    #[test]
    fn anthropic_payload_marks_the_system_prompt_as_cacheable() {
        let request = native_request(vec![ChatMessage::text(ChatRole::User, "hi")], None);
        let payload = anthropic_chat_payload("claude-x", &request, false).unwrap();

        assert_eq!(payload["system"][0]["type"], "text");
        assert_eq!(payload["system"][0]["text"], "You are Mint.");
        assert_eq!(payload["system"][0]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn anthropic_payload_omits_the_cache_breakpoint_for_an_empty_system_prompt() {
        let mut request = native_request(vec![ChatMessage::text(ChatRole::User, "hi")], None);
        request.system_instruction = String::new();
        let payload = anthropic_chat_payload("claude-x", &request, false).unwrap();

        // Falls back to the plain-string shape rather than an empty cacheable
        // block, matching the pre-caching behavior exactly for this case.
        assert_eq!(payload["system"], "");
    }

    #[test]
    fn anthropic_payload_marks_only_the_last_tool_as_the_cache_breakpoint() {
        let second_tool = ToolSpec {
            name: "write_file".into(),
            description: "Writes a file".into(),
            parameters: json!({ "type": "object" }),
        };
        let request = native_request(
            vec![ChatMessage::text(ChatRole::User, "hi")],
            Some(vec![sample_tool(), second_tool]),
        );
        let payload = anthropic_chat_payload("claude-x", &request, false).unwrap();

        let tools = payload["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 2);
        assert!(tools[0].get("cache_control").is_none());
        assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");
    }

    #[test]
    fn parses_anthropic_reply_with_text_and_tool_use_blocks() {
        let response = json!({
            "stop_reason": "tool_use",
            "content": [
                { "type": "text", "text": "Let me check that file." },
                { "type": "tool_use", "id": "toolu_1", "name": "read_file", "input": { "path": "main.rs" } }
            ]
        });
        let reply = parse_anthropic_reply("claude-x".into(), &response).unwrap();
        assert_eq!(reply.text, "Let me check that file.");
        let calls = reply.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].input["path"], "main.rs");
        assert_eq!(reply.stop_reason.as_deref(), Some("tool_use"));
    }

    #[test]
    fn parses_anthropic_reply_with_tool_use_only_and_no_text() {
        let response = json!({
            "content": [
                { "type": "tool_use", "id": "toolu_1", "name": "run_shell", "input": { "command": "ls" } }
            ]
        });
        let reply = parse_anthropic_reply("claude-x".into(), &response).unwrap();
        assert_eq!(reply.text, "");
        assert_eq!(reply.tool_calls.unwrap().len(), 1);
    }

    #[test]
    fn openai_native_payload_encodes_tool_calls_and_results() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "list files"),
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "list_files".into(),
                    input: json!({ "path": "." }),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "Cargo.toml\nsrc/".into(),
                    is_error: false,
                }],
            },
        ];
        let request = native_request(messages, Some(vec![sample_tool()]));
        let payload = openai_chat_payload("gpt-x", &request, false).unwrap();

        assert_eq!(payload["tools"][0]["type"], "function");
        assert_eq!(payload["tools"][0]["function"]["name"], "read_file");
        let msgs = payload["messages"].as_array().unwrap();
        // system + user + assistant(tool_calls) + tool
        assert_eq!(msgs.len(), 4);
        assert_eq!(msgs[2]["role"], "assistant");
        assert_eq!(msgs[2]["tool_calls"][0]["function"]["name"], "list_files");
        assert_eq!(msgs[3]["role"], "tool");
        assert_eq!(msgs[3]["tool_call_id"], "call_1");
    }

    #[test]
    fn openai_native_payload_forwards_a_screenshot_riding_a_tool_result_as_a_user_turn() {
        // OpenAI's Chat Completions `tool` role only accepts string content, so an
        // `Image` block attached to a tool result (e.g. `browser_screenshot`) must
        // surface as a follow-up `user` message instead of being dropped.
        let messages = vec![ChatMessage {
            role: ChatRole::Tool,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "[Screenshot captured — see attached image]".into(),
                    is_error: false,
                },
                ContentBlock::Image {
                    data_uri: "data:image/png;base64,QUJD".into(),
                },
            ],
        }];
        let built = openai_native_messages(&messages).unwrap();
        assert_eq!(built.len(), 2);
        assert_eq!(built[0]["role"], "tool");
        assert_eq!(built[0]["tool_call_id"], "call_1");
        assert_eq!(built[1]["role"], "user");
        assert_eq!(built[1]["content"][0]["type"], "image_url");
        assert_eq!(
            built[1]["content"][0]["image_url"]["url"],
            "data:image/png;base64,QUJD"
        );
    }

    #[test]
    fn openai_allows_audio_attachments() {
        let request = ChatRequest {
            message: "transcribe this".into(),
            system_instruction: String::new(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: Some("data:audio/wav;base64,QUJD".into()),
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        };
        assert!(require_supported_attachments("openai", &request).is_ok());
    }

    #[test]
    fn non_openai_openai_family_providers_still_reject_audio() {
        // Only the real OpenAI API is confirmed to speak the `input_audio`
        // schema — proxies/other services built on the same wire format
        // (local_openai, openrouter, deepseek, huggingface) and Anthropic
        // (no audio modality at all) must stay rejected.
        let request = ChatRequest {
            message: "transcribe this".into(),
            system_instruction: String::new(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: Some("data:audio/wav;base64,QUJD".into()),
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        };
        for provider in [
            "local_openai",
            "openrouter",
            "deepseek",
            "huggingface",
            "anthropic",
        ] {
            let error = require_supported_attachments(provider, &request).unwrap_err();
            assert!(
                matches!(&error, ChatError::UnsupportedAttachments(p) if p == provider),
                "expected {provider} to reject audio, got {error:?}"
            );
        }
    }

    #[test]
    fn openai_audio_part_splits_the_data_uri_and_maps_wav_format() {
        let part = openai_audio_part("data:audio/wav;base64,QUJD").unwrap();
        assert_eq!(part["type"], "input_audio");
        assert_eq!(part["input_audio"]["data"], "QUJD");
        assert_eq!(part["input_audio"]["format"], "wav");
    }

    #[test]
    fn openai_audio_part_defaults_unrecognized_formats_to_mp3() {
        let part = openai_audio_part("data:audio/webm;base64,QUJD").unwrap();
        assert_eq!(part["input_audio"]["format"], "mp3");
    }

    #[test]
    fn openai_audio_part_rejects_a_malformed_data_uri() {
        assert!(openai_audio_part("not-a-data-uri").is_err());
    }

    #[test]
    fn openai_message_content_embeds_audio_alongside_text() {
        let content =
            openai_message_content("what is this?", None, Some("data:audio/mp3;base64,QUJD"))
                .unwrap();
        let parts = content.as_array().unwrap();
        assert_eq!(parts[0]["type"], "text");
        assert_eq!(parts[1]["type"], "input_audio");
        assert_eq!(parts[1]["input_audio"]["data"], "QUJD");
    }

    #[test]
    fn openai_user_content_encodes_an_audio_content_block() {
        let message = ChatMessage {
            role: ChatRole::User,
            content: vec![
                ContentBlock::Text {
                    text: "what is this?".into(),
                },
                ContentBlock::Audio {
                    data_uri: "data:audio/wav;base64,QUJD".into(),
                },
            ],
        };
        let content = openai_user_content(&message).unwrap();
        let parts = content.as_array().unwrap();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[1]["type"], "input_audio");
        assert_eq!(parts[1]["input_audio"]["format"], "wav");
    }

    #[test]
    fn parses_openai_style_reply_with_tool_calls() {
        let response = json!({
            "choices": [{
                "finish_reason": "tool_calls",
                "message": {
                    "content": null,
                    "tool_calls": [{
                        "id": "call_1",
                        "function": { "name": "read_file", "arguments": "{\"path\":\"main.rs\"}" }
                    }]
                }
            }]
        });
        let reply = parse_openai_style_reply("gpt-x".into(), &response).unwrap();
        assert_eq!(reply.text, "");
        let calls = reply.tool_calls.unwrap();
        assert_eq!(calls[0].name, "read_file");
        assert_eq!(calls[0].input["path"], "main.rs");
        assert_eq!(reply.stop_reason.as_deref(), Some("tool_calls"));
    }

    #[test]
    fn gemini_native_payload_uses_function_declarations_and_resolves_tool_result_names() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "check weather"),
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_0".into(),
                    name: "weather".into(),
                    input: json!({ "city": "Bangkok" }),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "call_0".into(),
                    content: "32C, sunny".into(),
                    is_error: false,
                }],
            },
        ];
        let request = native_request(messages, Some(vec![sample_tool()]));
        let payload = gemini_native_payload(&request).unwrap();

        assert_eq!(
            payload["tools"][0]["functionDeclarations"][0]["name"],
            "read_file"
        );
        let contents = payload["contents"].as_array().unwrap();
        assert_eq!(contents[1]["role"], "model");
        assert_eq!(contents[1]["parts"][0]["functionCall"]["name"], "weather");
        assert_eq!(contents[2]["role"], "user");
        // Resolved back to the function name "weather", not the call id "call_0".
        assert_eq!(
            contents[2]["parts"][0]["functionResponse"]["name"],
            "weather"
        );
    }

    #[test]
    fn gemini_native_payload_replays_the_thought_signature_on_the_next_turn() {
        // Gemini 3.x rejects a replayed `functionCall` that's missing the
        // `thoughtSignature` the model originally attached to it (400
        // INVALID_ARGUMENT) — without this, every second tool-calling turn
        // failed and silently fell back to a different, weaker provider.
        let messages = vec![
            ChatMessage::text(ChatRole::User, "check weather"),
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_0".into(),
                    name: "weather".into(),
                    input: json!({ "city": "Bangkok" }),
                    thought_signature: Some("opaque-signature-abc".into()),
                }],
            },
        ];
        let request = native_request(messages, Some(vec![sample_tool()]));
        let payload = gemini_native_payload(&request).unwrap();

        let contents = payload["contents"].as_array().unwrap();
        assert_eq!(
            contents[1]["parts"][0]["thoughtSignature"],
            "opaque-signature-abc"
        );
        // Sibling of `functionCall`, not nested inside it.
        assert_eq!(contents[1]["parts"][0]["functionCall"]["name"], "weather");
    }

    #[test]
    fn gemini_native_payload_omits_thought_signature_when_absent() {
        let messages = vec![ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_0".into(),
                name: "weather".into(),
                input: json!({ "city": "Bangkok" }),
                thought_signature: None,
            }],
        }];
        let request = native_request(messages, Some(vec![sample_tool()]));
        let payload = gemini_native_payload(&request).unwrap();

        let contents = payload["contents"].as_array().unwrap();
        assert!(contents[0]["parts"][0].get("thoughtSignature").is_none());
    }

    #[test]
    fn parses_gemini_reply_with_function_call_parts() {
        let response = json!({
            "candidates": [{
                "finishReason": "STOP",
                "content": {
                    "parts": [
                        { "text": "Checking the weather now." },
                        { "functionCall": { "name": "weather", "args": { "city": "Bangkok" } } }
                    ]
                }
            }]
        });
        let reply = parse_gemini_reply("gemini-x".into(), &response).unwrap();
        assert_eq!(reply.text, "Checking the weather now.");
        let calls = reply.tool_calls.unwrap();
        assert_eq!(calls[0].name, "weather");
        assert_eq!(calls[0].input["city"], "Bangkok");
    }

    #[test]
    fn parses_gemini_reply_captures_the_thought_signature_off_the_function_call_part() {
        let response = json!({
            "candidates": [{
                "finishReason": "STOP",
                "content": {
                    "parts": [{
                        "functionCall": { "name": "weather", "args": { "city": "Bangkok" } },
                        "thoughtSignature": "opaque-signature-abc"
                    }]
                }
            }]
        });
        let reply = parse_gemini_reply("gemini-x".into(), &response).unwrap();
        let calls = reply.tool_calls.unwrap();
        assert_eq!(
            calls[0].thought_signature.as_deref(),
            Some("opaque-signature-abc")
        );
    }

    #[test]
    fn ollama_native_messages_send_tool_call_arguments_as_object_not_string() {
        let messages = vec![
            ChatMessage::text(ChatRole::User, "list files"),
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: "call_1".into(),
                    name: "list_files".into(),
                    input: json!({ "path": "." }),
                    thought_signature: None,
                }],
            },
        ];
        let built = ollama_native_messages(&messages);
        assert_eq!(built[1]["tool_calls"][0]["function"]["name"], "list_files");
        // Ollama expects `arguments` as a JSON object, unlike OpenAI's stringified form.
        assert!(built[1]["tool_calls"][0]["function"]["arguments"].is_object());
    }

    #[test]
    fn ollama_native_messages_forward_a_screenshot_riding_a_tool_result_as_a_user_turn() {
        // Ollama's `/api/chat` `tool` role message has no image field, so an
        // `Image` block attached to a tool result (e.g. `browser_screenshot`)
        // must surface as a follow-up `user` message with the `images` field.
        let messages = vec![ChatMessage {
            role: ChatRole::Tool,
            content: vec![
                ContentBlock::ToolResult {
                    tool_use_id: "call_1".into(),
                    content: "[Screenshot captured — see attached image]".into(),
                    is_error: false,
                },
                ContentBlock::Image {
                    data_uri: "data:image/png;base64,QUJD".into(),
                },
            ],
        }];
        let built = ollama_native_messages(&messages);
        assert_eq!(built.len(), 2);
        assert_eq!(built[0]["role"], "tool");
        assert_eq!(built[1]["role"], "user");
        assert_eq!(built[1]["images"][0], "QUJD");
    }

    #[test]
    fn parses_ollama_reply_from_native_message_envelope() {
        let response = json!({
            "message": {
                "content": "",
                "tool_calls": [{
                    "function": { "name": "list_files", "arguments": { "path": "." } }
                }]
            }
        });
        let reply = parse_ollama_reply("llama3.1".into(), &response).unwrap();
        assert_eq!(reply.tool_calls.unwrap()[0].name, "list_files");
    }

    #[test]
    fn extracts_total_tokens_from_each_provider_usage_shape() {
        let anthropic = parse_anthropic_reply(
            "claude-x".into(),
            &json!({
                "content": [{ "type": "text", "text": "hi" }],
                "usage": { "input_tokens": 100, "output_tokens": 20 }
            }),
        )
        .unwrap();
        assert_eq!(anthropic.total_tokens, Some(120));

        let openai = parse_openai_style_reply(
            "gpt-x".into(),
            &json!({
                "choices": [{ "message": { "content": "hi" } }],
                "usage": { "total_tokens": 150 }
            }),
        )
        .unwrap();
        assert_eq!(openai.total_tokens, Some(150));

        let gemini = parse_gemini_reply(
            "gemini-x".into(),
            &json!({
                "candidates": [{ "content": { "parts": [{ "text": "hi" }] } }],
                "usageMetadata": { "totalTokenCount": 200 }
            }),
        )
        .unwrap();
        assert_eq!(gemini.total_tokens, Some(200));

        let ollama = parse_ollama_reply(
            "llama3.1".into(),
            &json!({
                "message": { "content": "hi" },
                "prompt_eval_count": 80,
                "eval_count": 40
            }),
        )
        .unwrap();
        assert_eq!(ollama.total_tokens, Some(120));
    }

    #[test]
    fn missing_usage_data_yields_no_total_tokens() {
        let reply = parse_openai_style_reply(
            "gpt-x".into(),
            &json!({ "choices": [{ "message": { "content": "hi" } }] }),
        )
        .unwrap();
        assert_eq!(reply.total_tokens, None);
    }
}

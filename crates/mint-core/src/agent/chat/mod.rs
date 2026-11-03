use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::MintConfig;

mod anthropic;
mod custom;
mod gemini;
mod ollama;
mod openai;
mod streaming;

#[cfg(test)]
mod tests;

use anthropic::*;
use custom::*;
use gemini::*;
use ollama::*;
use openai::*;

/// Provider-neutral pieces produced while a model response is still arriving.
/// Callers never need to know which vendor-specific field carried the data.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatStreamEvent {
    TextDelta {
        delta: String,
    },
    ReasoningDelta {
        delta: String,
    },
    ToolCallDelta {
        index: usize,
        name: Option<String>,
        arguments: Option<String>,
        input: Option<Value>,
    },
}

/// `available_providers()`'s configured-providers list, reordered so
/// `gemini` is tried first among fallbacks (when it's configured and isn't
/// already the primary) — the user's explicit preference, ahead of
/// `available_providers()`'s own anthropic/openai/openrouter/deepseek/
/// gemini/... ordering (which exists for display purposes, e.g. the
/// `/models` picker, and is left alone). Everything else keeps its existing
/// relative order.
fn fallback_provider_order(config: &MintConfig) -> Vec<String> {
    let mut providers = config.available_providers();
    if let Some(pos) = providers.iter().position(|p| p == "gemini") {
        let gemini = providers.remove(pos);
        providers.insert(0, gemini);
    }
    providers
}

/// Send a chat request, automatically falling back to other configured providers
/// if the primary one returns a recoverable error. Tries `gemini` first among
/// the fallbacks when it's configured (see `fallback_provider_order`), then
/// the rest in their usual order.
/// Returns `(response, Option<fallback_provider>)`.
pub async fn send_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), ChatError> {
    let primary_error = match send_chat(config, request).await {
        Ok(r) => return Ok((r, None)),
        Err(e) if !is_recoverable(&e) => return Err(e),
        Err(e) => e,
    };
    // No point cycling through other cloud providers if the network itself is
    // unreachable — they all sit behind the same dead link, so every attempt
    // would just fail the same way after its own connect timeout. Surface
    // this immediately so the caller can retry-and-wait instead.
    if matches!(primary_error, ChatError::NetworkUnavailable) {
        return Err(primary_error);
    }
    let fallback_reason = fallback_reason_text(&config.ai_provider, &primary_error);
    for provider in fallback_provider_order(config) {
        if provider.as_str() == config.ai_provider.as_str() {
            continue;
        }
        let alt = config_for_provider(config, &provider);
        if let Ok(mut r) = send_chat(&alt, request).await {
            r.fallback_provider = Some(config.ai_provider.clone());
            r.fallback_reason = fallback_reason.clone();
            return Ok((r, Some(provider)));
        }
    }
    // all fallbacks failed — retry primary to surface original error
    send_chat(config, request).await.map(|r| (r, None))
}

/// Short, human-readable explanation for why `send_chat_with_fallback` swapped
/// away from `provider` — `None` for an ordinary transient failure, so routine
/// hiccups don't get an extra notice while specific, actionable ones do.
fn fallback_reason_text(provider: &str, error: &ChatError) -> Option<String> {
    let provider_name = if provider.starts_with("custom:") {
        provider.strip_prefix("custom:").unwrap_or(provider)
    } else {
        provider
    };
    match error {
        ChatError::InsufficientBalance(_) => Some(format!("{provider_name} ran out of balance")),
        ChatError::Request(e) => {
            if let Some(status) = e.status() {
                if status == reqwest::StatusCode::PAYLOAD_TOO_LARGE {
                    Some(format!("{provider_name} payload too large (HTTP 413)"))
                } else if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
                    Some(format!("{provider_name} rate limit exceeded (HTTP 429)"))
                } else {
                    Some(format!("{provider_name} returned HTTP {status}"))
                }
            } else {
                None
            }
        }
        _ => None,
    }
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
    let primary_error = match stream_chat(config, request, &mut on_chunk).await {
        Ok(r) => return Ok((r, None)),
        Err(e) if !is_recoverable(&e) => return Err(e),
        Err(e) => {
            eprintln!("Ollama stream chat failed, falling back: {:?}", e);
            e
        }
    };
    if matches!(primary_error, ChatError::NetworkUnavailable) {
        return Err(primary_error);
    }
    let fallback_reason = fallback_reason_text(&config.ai_provider, &primary_error);
    for provider in fallback_provider_order(config) {
        if provider.as_str() == config.ai_provider.as_str() {
            continue;
        }
        let alt = config_for_provider(config, &provider);
        if let Ok(mut r) = stream_chat(&alt, request, &mut on_chunk).await {
            r.fallback_provider = Some(config.ai_provider.clone());
            r.fallback_reason = fallback_reason.clone();
            return Ok((r, Some(provider)));
        }
    }
    stream_chat(config, request, &mut on_chunk)
        .await
        .map(|r| (r, None))
}

/// Streaming equivalent of [`send_chat_with_fallback`] that preserves the
/// complete response (tool calls, usage, stop reason, and reasoning) while
/// exposing provider-neutral deltas as they arrive.
pub async fn stream_chat_events_with_fallback<F>(
    config: &MintConfig,
    request: &ChatRequest,
    mut on_event: F,
) -> Result<(ChatResponse, Option<String>), ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let primary_error = match stream_chat_events(config, request, &mut on_event).await {
        Ok(r) => return Ok((r, None)),
        Err(e) if !is_recoverable(&e) => return Err(e),
        Err(e) => e,
    };
    if matches!(primary_error, ChatError::NetworkUnavailable) {
        return Err(primary_error);
    }
    let fallback_reason = fallback_reason_text(&config.ai_provider, &primary_error);
    for provider in fallback_provider_order(config) {
        if provider == config.ai_provider {
            continue;
        }
        let alt = config_for_provider(config, &provider);
        if let Ok(mut response) = stream_chat_events(&alt, request, &mut on_event).await {
            response.fallback_provider = Some(config.ai_provider.clone());
            response.fallback_reason = fallback_reason.clone();
            return Ok((response, Some(provider)));
        }
    }
    stream_chat_events(config, request, &mut on_event)
        .await
        .map(|response| (response, None))
}

/// Whether an error warrants trying another provider.
fn is_recoverable(e: &ChatError) -> bool {
    matches!(
        e,
        ChatError::MissingApiKey(_)
            | ChatError::Request(_)
            | ChatError::MissingResponseText
            | ChatError::UnsupportedAttachments(_)
            | ChatError::InsufficientBalance(_)
            | ChatError::NetworkUnavailable
    )
}

/// Clone the config with a different active provider.
fn config_for_provider(config: &MintConfig, provider: &str) -> MintConfig {
    MintConfig {
        ai_provider: provider.to_owned(),
        ..config.clone()
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
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
    /// When set, restricts this turn's `mcp_tool`/`mcp_list_tools` calls to this one
    /// configured MCP server — set by picking a plugin from the composer's `@` mention
    /// dropdown, rather than leaving server selection to the model's own judgment.
    #[serde(default)]
    pub pinned_mcp_server: Option<String>,
    /// Structured conversation history for native tool-calling. When set, provider
    /// adapters build the request from this instead of the flat `message` field.
    #[serde(default)]
    pub messages: Option<Vec<ChatMessage>>,
    /// Tool definitions to offer the model via each provider's native
    /// function/tool-calling API. Ignored when `messages` is `None`.
    #[serde(default)]
    pub tools: Option<Vec<ToolSpec>>,
    /// Optional per-request temperature override. When `None`, falls back to
    /// `config.resolved_temperature()`.
    #[serde(default)]
    pub temperature: Option<f64>,
    /// Optional per-request thinking/reasoning enabled override.
    #[serde(default)]
    pub thinking_enabled: Option<bool>,
    /// Optional per-request thinking effort override ("low", "medium", "high", "extra_high").
    #[serde(default)]
    pub thinking_effort: Option<String>,
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
    pub(crate) fn with_stored_document_context(
        &self,
        _config: &crate::MintConfig,
        path: &std::path::Path,
    ) -> Result<ChatRequest, String> {
        let Some(document) = &self.document_attachment else {
            return Ok(self.clone());
        };
        let extracted =
            crate::knowledge::extract_managed_document_text(path).map_err(|e| e.to_string())?;
        self.with_document_text(document, extracted)
    }
    fn with_document_text(
        &self,
        document: &DocumentAttachment,
        extracted: String,
    ) -> Result<ChatRequest, String> {
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

        self.with_document_text(document, extracted)
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
    /// Short, human-readable reason `send_chat_with_fallback` swapped providers,
    /// when the primary's failure was specific enough to explain (insufficient
    /// balance, no network) — `None` for an ordinary transient error, so a
    /// ordinary hiccup doesn't get an extra notice while a genuinely actionable
    /// one does.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
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
    /// model's context window, rather than estimating client-side. Derived from
    /// `input_tokens + output_tokens` when both are present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_tokens: Option<u32>,
    /// Prompt/context tokens the provider processed for this turn (the whole
    /// resent history + system prompt + tool schemas), when reported — the
    /// "sent up to the model" half, tracked separately from `output_tokens` so
    /// the UI can show `↑ context · ↓ generated` instead of one opaque sum.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub input_tokens: Option<u32>,
    /// Completion tokens the model generated this turn, when reported — the
    /// "came back down from the model" half. See `input_tokens`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_tokens: Option<u32>,
    /// Model reasoning / chain-of-thought tokens, when reported by reasoning models
    /// (e.g. DeepSeek Reasoner, Claude 3.7 Thinking, Gemini Thinking, or <think> tags).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thought: Option<String>,
}

/// Internal, provider-agnostic result of a single non-streaming provider call.
/// `send_chat` maps this onto the public `ChatResponse`.
pub(super) struct ProviderReply {
    pub(super) model: String,
    pub(super) text: String,
    pub(super) thought: Option<String>,
    pub(super) tool_calls: Option<Vec<ToolCall>>,
    pub(super) stop_reason: Option<String>,
    /// Prompt/context tokens processed for this call, when the provider reports
    /// usage. `send_chat` derives `ChatResponse::total_tokens` from this plus
    /// `output_tokens`.
    pub(super) input_tokens: Option<u32>,
    /// Completion tokens generated for this call, when reported.
    pub(super) output_tokens: Option<u32>,
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
    /// Provider returned `402 Payment Required` — distinguished from the generic
    /// `Request` case so callers can tell "your account ran out of balance" apart
    /// from an ordinary transient failure instead of silently swapping providers
    /// with no explanation.
    #[error("provider '{0}' rejected the request: insufficient balance")]
    InsufficientBalance(String),
    /// The request never reached any server at all (DNS failure, connection
    /// refused, no route) — as opposed to reaching a server that returned an
    /// error. Distinguished so a caller can retry-and-wait instead of pointlessly
    /// cycling through other cloud providers that sit behind the same dead
    /// network link.
    #[error("no network connection to reach the provider")]
    NetworkUnavailable,
    #[error("provider API error: {0}")]
    Api(String),
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
        thought: reply.thought,
        fallback_provider: None,
        fallback_reason: None,
        tool_calls: reply.tool_calls,
        stop_reason: reply.stop_reason,
        total_tokens: match (reply.input_tokens, reply.output_tokens) {
            (Some(i), Some(o)) => Some(i + o),
            (i, o) => i.or(o),
        },
        input_tokens: reply.input_tokens,
        output_tokens: reply.output_tokens,
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
    stream_chat_events(config, request, |event| {
        if let ChatStreamEvent::TextDelta { delta } = event {
            on_chunk(delta);
        }
    })
    .await
}

pub async fn stream_chat_events<F>(
    config: &MintConfig,
    request: &ChatRequest,
    mut on_event: F,
) -> Result<ChatResponse, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let client = crate::HTTP_CLIENT.clone();
    let provider = config.ai_provider.as_str();
    require_supported_attachments(provider, request)?;
    let reply = match provider {
        "gemini" => stream_gemini_reply(&client, config, request, &mut on_event).await?,
        "openai" | "local_openai" | "openrouter" | "deepseek" => {
            stream_openai_reply(&client, config, request, &mut on_event).await?
        }
        "ollama" => stream_ollama_reply(&client, config, request, &mut on_event).await?,
        "anthropic" => stream_anthropic_reply(&client, config, request, &mut on_event).await?,
        "huggingface" => stream_huggingface_reply(&client, config, request, &mut on_event).await?,
        p if p.starts_with("custom:") => {
            stream_custom_provider_reply(&client, config, request, &mut on_event).await?
        }
        other => return Err(ChatError::UnsupportedProvider(other.into())),
    };
    Ok(ChatResponse {
        provider: provider.into(),
        model: reply.model,
        text: reply.text,
        thought: reply.thought,
        fallback_provider: None,
        fallback_reason: None,
        tool_calls: reply.tool_calls,
        stop_reason: reply.stop_reason,
        total_tokens: match (reply.input_tokens, reply.output_tokens) {
            (Some(input), Some(output)) => Some(input + output),
            (input, output) => input.or(output),
        },
        input_tokens: reply.input_tokens,
        output_tokens: reply.output_tokens,
    })
}

/// Extracts `<think>...</think>` or `<thought>...</thought>` chain-of-thought blocks emitted by
/// reasoning models or the prompt-based CoT fallback when embedded directly into plain text content.
/// Returns `(Option<thought>, remaining_text)`.
pub(crate) fn extract_think_tag(text: &str) -> (Option<String>, String) {
    for (open_tag, close_tag) in [("<think>", "</think>"), ("<thought>", "</thought>")] {
        if let Some(start) = text.find(open_tag) {
            let open_len = open_tag.len();
            let close_len = close_tag.len();
            if let Some(end) = text[start + open_len..].find(close_tag) {
                let thought = text[start + open_len..start + open_len + end]
                    .trim()
                    .to_string();
                let mut remaining = text[..start].to_string();
                remaining.push_str(&text[start + open_len + end + close_len..]);
                let cleaned = remaining.trim().to_string();
                return (
                    if thought.is_empty() {
                        None
                    } else {
                        Some(thought)
                    },
                    cleaned,
                );
            } else {
                let thought = text[start + open_len..].trim().to_string();
                let cleaned = text[..start].trim().to_string();
                return (
                    if thought.is_empty() {
                        None
                    } else {
                        Some(thought)
                    },
                    cleaned,
                );
            }
        }
    }
    (None, text.to_string())
}

pub(super) fn required_key(provider: &str, key: &str) -> Result<(), ChatError> {
    if key.trim().is_empty() {
        Err(ChatError::MissingApiKey(provider.into()))
    } else {
        Ok(())
    }
}

pub(super) fn provider_key(configured: &str, environment_variable: &str) -> String {
    if configured.trim().is_empty() {
        std::env::var(environment_variable).unwrap_or_default()
    } else {
        configured.into()
    }
}

pub(super) fn require_supported_attachments(
    provider: &str,
    request: &ChatRequest,
) -> Result<(), ChatError> {
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

pub(super) fn wants_agent_json(request: &ChatRequest) -> bool {
    let instruction = request.system_instruction.as_str();
    instruction.contains("Return only JSON")
        || (instruction.contains("Return exactly one JSON object per response")
            && instruction.contains("Input formats:")
            && instruction.contains("- finish:"))
}

/// Resolves the effective sampling temperature for this request, prioritizing
/// `request.temperature` if present, otherwise falling back to `config.resolved_temperature()`.
pub(crate) fn effective_temperature(config: &MintConfig, request: &ChatRequest) -> f64 {
    request
        .temperature
        .unwrap_or_else(|| config.resolved_temperature())
}

/// Resolves (enabled, effort) for this request and model, prioritizing request overrides.
pub(crate) fn effective_thinking(
    config: &MintConfig,
    request: &ChatRequest,
    model: &str,
) -> (bool, String) {
    if !MintConfig::is_thinking_supported_for_model(&config.ai_provider, model) {
        return (false, "medium".to_string());
    }
    let enabled = request
        .thinking_enabled
        .unwrap_or_else(|| config.resolved_thinking_enabled_for_model(&config.ai_provider, model));
    let effort = request
        .thinking_effort
        .as_deref()
        .map(str::to_ascii_lowercase)
        .unwrap_or_else(|| config.resolved_thinking_effort_for_model(model).to_string());
    (enabled, effort)
}

pub(crate) fn thinking_prompt_directive(effort: &str) -> &'static str {
    match effort {
        "low" => {
            "\n\n[Thinking Mode: Enabled (Low Effort)]\nBefore providing your response or calling any tool, you MUST write a brief and concise (2-4 sentences) internal reasoning process enclosed strictly within <think>...</think> tags at the very beginning of your message content (never inside tool arguments), followed by your tool call or final answer outside the tags."
        }
        "high" => {
            "\n\n[Thinking Mode: Enabled (High Effort)]\nBefore providing your response or calling any tool, you MUST write a thorough and detailed internal reasoning process, carefully analyzing edge cases and trade-offs, enclosed strictly within <think>...</think> tags at the very beginning of your message content (never inside tool arguments), followed by your tool call or final answer outside the tags."
        }
        "extra_high" => {
            "\n\n[Thinking Mode: Enabled (Extra High Effort)]\nBefore providing your response or calling any tool, you MUST write an exhaustive, step-by-step internal reasoning process, verifying assumptions and alternative approaches in depth, enclosed strictly within <think>...</think> tags at the very beginning of your message content (never inside tool arguments), followed by your tool call or final answer outside the tags."
        }
        _ => {
            "\n\n[Thinking Mode: Enabled (Medium Effort)]\nBefore providing your response or calling any tool, you MUST write a clear and structured internal reasoning process enclosed strictly within <think>...</think> tags at the very beginning of your message content (never inside tool arguments), followed by your tool call or final answer outside the tags."
        }
    }
}

/// Returns the effective system instruction for a request, appending a `<think>...</think>`
/// Chain-of-Thought directive when Thinking is enabled on a chat model that does not use
/// a native visible reasoning API parameter.
pub(crate) fn effective_system_instruction(
    config: &MintConfig,
    request: &ChatRequest,
    provider: &str,
    model: &str,
) -> String {
    let base = request.system_instruction.as_str();
    if wants_agent_json(request) {
        return base.to_string();
    }
    let (enabled, effort) = effective_thinking(config, request, model);
    if enabled && !MintConfig::has_native_thinking_api(provider, model) {
        format!("{base}{}", thinking_prompt_directive(&effort))
    } else {
        base.to_string()
    }
}

/// Concatenates all `Text` blocks in a message (tool-use/result/image blocks
/// contribute nothing).
pub(super) fn message_text(message: &ChatMessage) -> String {
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

/// Splits a `data:<mime>;base64,<data>` URI into `(mime_type, base64_data)`,
/// validating the mime type starts with `expected_prefix` (e.g. `"image/"`).
pub(super) fn split_data_uri(
    data_uri: &str,
    expected_prefix: &str,
) -> Result<(String, String), ChatError> {
    data_uri
        .strip_prefix("data:")
        .and_then(|payload| payload.split_once(";base64,"))
        .filter(|(mime, data)| mime.starts_with(expected_prefix) && !data.is_empty())
        .map(|(mime, data)| (mime.to_owned(), data.to_owned()))
        .ok_or(ChatError::InvalidAttachment)
}

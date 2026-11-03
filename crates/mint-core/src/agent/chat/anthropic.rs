use reqwest::Client;
use serde_json::{Value, json};

use super::streaming::{StreamFormat, collect_stream_reply};
use super::{
    ProviderReply, effective_system_instruction, effective_temperature, effective_thinking,
    extract_think_tag, provider_key, required_key, split_data_uri,
};
use crate::{
    ChatError, ChatMessage, ChatRequest, ChatRole, ChatStreamEvent, ContentBlock, MintConfig,
    ToolCall,
};

pub(super) async fn call_anthropic(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let api_key = provider_key(&config.anthropic_api_key, "ANTHROPIC_API_KEY");
    required_key("anthropic", &api_key)?;
    let model = config.anthropic_model.clone();
    let payload = anthropic_chat_payload(config, &model, request, false)?;
    let response: Value = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    parse_anthropic_reply(model, &response)
}

pub(super) async fn stream_anthropic_reply<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let api_key = provider_key(&config.anthropic_api_key, "ANTHROPIC_API_KEY");
    required_key("anthropic", &api_key)?;
    let model = config.anthropic_model.clone();
    let payload = anthropic_chat_payload(config, &model, request, true)?;
    let response = client
        .post("https://api.anthropic.com/v1/messages")
        .header("x-api-key", api_key)
        .header("anthropic-version", "2023-06-01")
        .header("anthropic-beta", "prompt-caching-2024-07-31")
        .json(&payload)
        .send()
        .await?
        .error_for_status()?;
    collect_stream_reply(response, StreamFormat::Anthropic, model, on_event).await
}

pub(super) fn anthropic_chat_payload(
    config: &MintConfig,
    model: &str,
    request: &ChatRequest,
    stream: bool,
) -> Result<Value, ChatError> {
    let system_instruction = effective_system_instruction(config, request, "anthropic", model);
    let system = anthropic_system_blocks(&system_instruction);
    let (thinking_enabled, thinking_effort) = effective_thinking(config, request, model);
    let is_thinking_supported = MintConfig::has_native_thinking_api("anthropic", model);
    let use_thinking = thinking_enabled && is_thinking_supported;

    let (budget, max_tokens, temp) = if use_thinking {
        let b = match thinking_effort.as_str() {
            "low" => 1024,
            "medium" => 4096,
            "high" => 16384,
            "extra_high" => 32768,
            _ => 4096,
        };
        (Some(b), b + 8192, 1.0)
    } else {
        (None, 8192, effective_temperature(config, request))
    };

    let mut payload = if let Some(messages) = &request.messages {
        json!({
            "model": model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": anthropic_messages(messages)?,
            "temperature": temp,
        })
    } else {
        json!({
            "model": model,
            "max_tokens": max_tokens,
            "system": system,
            "messages": [{
                "role": "user",
                "content": anthropic_message_content(&request.message, request.image_data_uri.as_deref())?,
            }],
            "temperature": temp,
        })
    };
    if let Some(b) = budget {
        payload["thinking"] = json!({
            "type": "enabled",
            "budget_tokens": b,
        });
    }
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

pub(super) fn anthropic_system_blocks(system_instruction: &str) -> Value {
    if system_instruction.is_empty() {
        return json!(system_instruction);
    }
    json!([{
        "type": "text",
        "text": system_instruction,
        "cache_control": { "type": "ephemeral" },
    }])
}

pub(super) fn anthropic_messages(messages: &[ChatMessage]) -> Result<Vec<Value>, ChatError> {
    let non_system: Vec<&ChatMessage> = messages
        .iter()
        .filter(|message| message.role != ChatRole::System)
        .collect();

    let cache_idx = if non_system.len() >= 2 {
        Some(non_system.len() - 2)
    } else {
        None
    };

    non_system
        .into_iter()
        .enumerate()
        .map(|(i, message)| {
            let role = match message.role {
                ChatRole::Assistant => "assistant",
                _ => "user",
            };
            let mut content = message
                .content
                .iter()
                .map(anthropic_content_block)
                .collect::<Result<Vec<_>, _>>()?;

            if Some(i) == cache_idx
                && let Some(last_block) = content.last_mut()
            {
                if let Some(obj) = last_block.as_object_mut() {
                    obj.insert("cache_control".to_string(), json!({ "type": "ephemeral" }));
                }
            }

            Ok(json!({ "role": role, "content": content }))
        })
        .collect()
}

pub(super) fn anthropic_message_content(
    text: &str,
    image_data_uri: Option<&str>,
) -> Result<Value, ChatError> {
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

pub(super) fn anthropic_content_block(block: &ContentBlock) -> Result<Value, ChatError> {
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
        ContentBlock::Audio { .. } | ContentBlock::Video { .. } => {
            return Err(ChatError::UnsupportedAttachments("anthropic".into()));
        }
    })
}

pub(crate) fn anthropic_input_tokens(usage: &Value) -> Option<u32> {
    [
        "input_tokens",
        "cache_read_input_tokens",
        "cache_creation_input_tokens",
    ]
    .into_iter()
    .filter_map(|key| usage[key].as_u64())
    .reduce(u64::saturating_add)
    .map(|n| n.min(u32::MAX as u64) as u32)
}

pub(super) fn parse_anthropic_reply(
    model: String,
    response: &Value,
) -> Result<ProviderReply, ChatError> {
    let blocks = response["content"].as_array().cloned().unwrap_or_default();
    let mut text = String::new();
    let mut thought_parts: Vec<&str> = Vec::new();
    let mut tool_calls = Vec::new();
    for block in &blocks {
        match block["type"].as_str() {
            Some("text") => {
                if let Some(t) = block["text"].as_str() {
                    text.push_str(t);
                }
            }
            Some("thinking") => {
                if let Some(t) = block["thinking"].as_str() {
                    thought_parts.push(t);
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
    let direct_thought = if !thought_parts.is_empty() {
        Some(thought_parts.join("").trim().to_string()).filter(|s| !s.is_empty())
    } else {
        None
    };
    let (thought, text) = if direct_thought.is_none() {
        extract_think_tag(&text)
    } else {
        (direct_thought, text)
    };
    if text.is_empty() && tool_calls.is_empty() {
        if let Some(th) = &thought {
            return Ok(ProviderReply {
                model,
                text: th.clone(),
                thought: Some(th.clone()),
                tool_calls: None,
                stop_reason: response["stop_reason"].as_str().map(str::to_owned),
                input_tokens: anthropic_input_tokens(&response["usage"]),
                output_tokens: response["usage"]["output_tokens"]
                    .as_u64()
                    .map(|n| n as u32),
            });
        }
        return Err(ChatError::MissingResponseText);
    }
    Ok(ProviderReply {
        model,
        text,
        thought,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        stop_reason: response["stop_reason"].as_str().map(str::to_owned),
        input_tokens: anthropic_input_tokens(&response["usage"]),
        output_tokens: response["usage"]["output_tokens"]
            .as_u64()
            .map(|n| n as u32),
    })
}

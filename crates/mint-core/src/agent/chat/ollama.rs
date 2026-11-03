use reqwest::Client;
use serde_json::{Value, json};

use super::streaming::{StreamFormat, collect_stream_reply};
use super::{
    ProviderReply, decode_tool_arguments, effective_system_instruction, effective_temperature,
    effective_thinking, extract_think_tag, split_data_uri,
};
use crate::{
    ChatError, ChatMessage, ChatRequest, ChatRole, ChatStreamEvent, ContentBlock, MintConfig,
    ToolCall,
};

pub(super) async fn call_ollama(
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
    let system_instruction = effective_system_instruction(config, request, "ollama", &model);
    let native = request.messages.is_some()
        && config.tool_calling_mode() == crate::config::ToolCallingMode::Native;
    let mut body = if native {
        let mut payload = json!({
            "model": model,
            "stream": false,
            "messages": ollama_request_messages(request, &system_instruction),
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
                { "role": "system", "content": system_instruction },
                user_message
            ]
        })
    };
    let temp = effective_temperature(config, request);
    body["options"] = json!({
        "num_ctx": config.ollama_num_ctx,
        "temperature": temp,
    });
    let (thinking_enabled, _) = effective_thinking(config, request, &model);
    if MintConfig::has_native_thinking_api("ollama", &model) {
        body["think"] = json!(thinking_enabled);
    }
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

pub(super) async fn stream_ollama_reply<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let host = if config.ollama_host.trim().is_empty() {
        "http://localhost:11434"
    } else {
        config.ollama_host.trim_end_matches('/')
    };
    let model = config.ollama_model.clone();
    let response = client
        .post(format!("{host}/api/chat"))
        .json(&ollama_stream_payload(config, request, &model)?)
        .send()
        .await?
        .error_for_status()?;
    collect_stream_reply(response, StreamFormat::Ollama, model, on_event).await
}

pub(super) fn ollama_stream_payload(
    config: &MintConfig,
    request: &ChatRequest,
    model: &str,
) -> Result<Value, ChatError> {
    let system_instruction = effective_system_instruction(config, request, "ollama", model);
    let native = request.messages.is_some()
        && config.tool_calling_mode() == crate::config::ToolCallingMode::Native;
    let mut payload = if native {
        let mut value = json!({
            "model": model,
            "stream": true,
            "messages": ollama_request_messages(request, &system_instruction),
        });
        if let Some(tools) = &request.tools {
            value["tools"] = json!(
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
        value
    } else {
        let mut user = json!({ "role": "user", "content": request.message });
        if let Some(images) = ollama_images(request) {
            user["images"] = json!(images);
        }
        json!({
            "model": model,
            "stream": true,
            "messages": [
                { "role": "system", "content": system_instruction },
                user
            ]
        })
    };
    payload["options"] = json!({
        "num_ctx": config.ollama_num_ctx,
        "temperature": effective_temperature(config, request),
    });
    let (thinking_enabled, _) = effective_thinking(config, request, model);
    if MintConfig::has_native_thinking_api("ollama", model) {
        payload["think"] = json!(thinking_enabled);
    }
    Ok(payload)
}

pub(super) fn ollama_request_messages(
    request: &ChatRequest,
    system_instruction: &str,
) -> Vec<Value> {
    let history = request.messages.as_deref().unwrap_or(&[]);
    let mut result = ollama_native_messages(history);
    if !system_instruction.is_empty() && !history.iter().any(|m| m.role == ChatRole::System) {
        result.insert(0, json!({"role":"system","content":system_instruction}));
    }
    result
}

pub(super) fn ollama_native_messages(messages: &[ChatMessage]) -> Vec<Value> {
    let mut result = Vec::new();
    for message in messages {
        match message.role {
            ChatRole::System => {
                result.push(json!({ "role": "system", "content": super::message_text(message) }));
            }
            ChatRole::User => {
                let mut entry = json!({ "role": "user", "content": super::message_text(message) });
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
                let text = super::message_text(message);
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

pub(super) fn parse_ollama_reply(
    model: String,
    response: &Value,
) -> Result<ProviderReply, ChatError> {
    let message = &response["message"];
    let raw_text = message["content"].as_str().unwrap_or_default().to_string();
    let direct_thought = message["thinking"]
        .as_str()
        .or_else(|| message["reasoning_content"].as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty());

    let (thought, text) = if direct_thought.is_none() {
        extract_think_tag(&raw_text)
    } else {
        (direct_thought, raw_text)
    };

    let tool_calls: Vec<ToolCall> = message["tool_calls"]
        .as_array()
        .map(|calls| {
            calls
                .iter()
                .enumerate()
                .filter_map(|(index, call)| {
                    let name = call["function"]["name"].as_str()?.to_string();
                    let arguments = &call["function"]["arguments"];
                    let input = decode_tool_arguments(arguments);
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
        if let Some(th) = &thought {
            return Ok(ProviderReply {
                model,
                text: th.clone(),
                thought: Some(th.clone()),
                tool_calls: None,
                stop_reason: None,
                input_tokens: response["prompt_eval_count"].as_u64().map(|n| n as u32),
                output_tokens: response["eval_count"].as_u64().map(|n| n as u32),
            });
        }
        return Err(ChatError::MissingResponseText);
    }
    Ok(ProviderReply {
        model,
        text,
        thought,
        tool_calls: (!tool_calls.is_empty()).then_some(tool_calls),
        stop_reason: None,
        input_tokens: response["prompt_eval_count"].as_u64().map(|n| n as u32),
        output_tokens: response["eval_count"].as_u64().map(|n| n as u32),
    })
}

pub(super) fn ollama_images(request: &ChatRequest) -> Option<Vec<String>> {
    let images = request.image_data_uri.as_deref()?;
    let mut result = Vec::new();
    for img in images.split_whitespace() {
        if let Ok((_, data)) = split_data_uri(img, "image/") {
            result.push(data);
        }
    }
    (!result.is_empty()).then_some(result)
}

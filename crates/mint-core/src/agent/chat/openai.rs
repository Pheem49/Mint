use reqwest::Client;
use serde_json::{Value, json};

use super::streaming::{StreamFormat, collect_stream_reply};
use super::{
    ProviderReply, effective_system_instruction, effective_temperature, effective_thinking,
    extract_think_tag, provider_key, required_key, split_data_uri, wants_agent_json,
};
use crate::{
    ChatError, ChatMessage, ChatRequest, ChatRole, ChatStreamEvent, ContentBlock, MintConfig,
    ToolCall,
};

pub(super) async fn call_openai(
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
    let response: Value = send_openai_style_request(
        client
            .post(format!("{base_url}/chat/completions"))
            .bearer_auth(if local { "not-needed" } else { &api_key })
            .json(&openai_chat_payload(config, &model, request, false)?),
        &config.ai_provider,
    )
    .await?;
    parse_openai_style_reply(model, &response)
}

pub(super) async fn stream_openai_reply<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
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
        .json(&openai_chat_payload(config, &model, request, true)?)
        .send()
        .await?;
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ChatError::Api(format!("HTTP {status}: {body}")));
    }
    collect_stream_reply(response, StreamFormat::OpenAi, model, on_event).await
}

pub(super) async fn send_openai_style_request(
    request: reqwest::RequestBuilder,
    provider: &str,
) -> Result<Value, ChatError> {
    let response = request.send().await.map_err(|e| {
        if e.is_connect() {
            ChatError::NetworkUnavailable
        } else {
            ChatError::Request(e)
        }
    })?;
    if response.status() == reqwest::StatusCode::PAYMENT_REQUIRED {
        return Err(ChatError::InsufficientBalance(provider.to_string()));
    }
    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(ChatError::Api(format!("HTTP {status}: {body}")));
    }
    Ok(response.json().await?)
}

pub(super) fn decode_tool_arguments(arguments: &Value) -> Value {
    match arguments.as_str() {
        Some(raw) => sanitize_and_parse_tool_arguments(raw).unwrap_or_else(|| arguments.clone()),
        None => arguments.clone(),
    }
}

pub(super) fn sanitize_and_parse_tool_arguments(raw: &str) -> Option<Value> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Ok(val) = serde_json::from_str::<Value>(trimmed) {
        return Some(val);
    }

    // 1. Strip markdown code fences if wrapped in ```json ... ```
    let without_fences = if let Some(stripped) = trimmed.strip_prefix("```json") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else if let Some(stripped) = trimmed.strip_prefix("```") {
        stripped.strip_suffix("```").unwrap_or(stripped).trim()
    } else {
        trimmed
    };
    if let Ok(val) = serde_json::from_str::<Value>(without_fences) {
        return Some(val);
    }

    // 2. Strip known special tokens (DeepSeek DSML markers, thinking tags, end of sentence markers)
    let cleaned = without_fences
        .replace("</｜｜DSML｜｜", "")
        .replace("<｜｜DSML｜｜", "")
        .replace("<｜end of sentence｜>", "")
        .replace("</think>", "")
        .replace("<think>", "");
    if let Ok(val) = serde_json::from_str::<Value>(cleaned.trim()) {
        return Some(val);
    }

    // 3. Fallback: extract substring between first '{' and last '}'
    if let Some(start) = cleaned.find('{')
        && let Some(end) = cleaned.rfind('}')
        && start < end
    {
        let candidate = &cleaned[start..=end];
        if let Ok(val) = serde_json::from_str::<Value>(candidate) {
            return Some(val);
        }
        // 4. Handle trailing commas before closing braces/brackets
        let stripped_commas = strip_json_trailing_commas(candidate);
        if let Ok(val) = serde_json::from_str::<Value>(&stripped_commas) {
            return Some(val);
        }
    }

    None
}

pub(super) fn strip_json_trailing_commas(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();

    let mut i = 0;
    while i < len {
        let c = chars[i];
        if in_string {
            out.push(c);
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_string = false;
            }
        } else if c == '"' {
            in_string = true;
            out.push(c);
        } else if c == ',' {
            let mut j = i + 1;
            while j < len && chars[j].is_whitespace() {
                j += 1;
            }
            if j < len && (chars[j] == '}' || chars[j] == ']') {
                // Skip the trailing comma
            } else {
                out.push(c);
            }
        } else {
            out.push(c);
        }
        i += 1;
    }
    out
}

pub(super) fn parse_openai_style_reply(
    model: String,
    response: &Value,
) -> Result<ProviderReply, ChatError> {
    let message = &response["choices"][0]["message"];
    let raw_text = message["content"].as_str().unwrap_or_default().to_string();
    let direct_thought = message["reasoning_content"]
        .as_str()
        .or_else(|| message["reasoning"].as_str())
        .or_else(|| message["thought"].as_str())
        .or_else(|| message["thinking"].as_str())
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
                .filter_map(|call| {
                    let name = call["function"]["name"].as_str()?.to_string();
                    let input = decode_tool_arguments(&call["function"]["arguments"]);
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
        if let Some(th) = &thought {
            return Ok(ProviderReply {
                model,
                text: th.clone(),
                thought: Some(th.clone()),
                tool_calls: None,
                stop_reason: response["choices"][0]["finish_reason"]
                    .as_str()
                    .map(str::to_owned),
                input_tokens: response["usage"]["prompt_tokens"]
                    .as_u64()
                    .map(|n| n as u32),
                output_tokens: response["usage"]["completion_tokens"]
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
        stop_reason: response["choices"][0]["finish_reason"]
            .as_str()
            .map(str::to_owned),
        input_tokens: response["usage"]["prompt_tokens"]
            .as_u64()
            .map(|n| n as u32),
        output_tokens: response["usage"]["completion_tokens"]
            .as_u64()
            .map(|n| n as u32),
    })
}

pub(super) fn openai_chat_payload(
    config: &MintConfig,
    model: &str,
    request: &ChatRequest,
    stream: bool,
) -> Result<Value, ChatError> {
    let provider = config.ai_provider.trim().to_ascii_lowercase();
    let effective_sys = effective_system_instruction(config, request, &provider, model);
    let messages = if let Some(messages) = &request.messages {
        let mut built = Vec::new();
        if !effective_sys.is_empty() && !messages.iter().any(|m| m.role == ChatRole::System) {
            built.push(json!({ "role": "system", "content": effective_sys }));
        }
        built.extend(openai_native_messages(messages)?);
        built
    } else {
        vec![
            json!({ "role": "system", "content": effective_sys }),
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
    let max_tokens = {
        let is_groq = config
            .resolve_custom_provider(&config.ai_provider)
            .map(|cp| cp.base_url.contains("groq.com"))
            .unwrap_or(false);
        if is_groq {
            if request.tools.is_some() { 2048 } else { 4096 }
        } else {
            8192
        }
    };
    let mut payload = json!({
        "model": model,
        "stream": stream,
        "messages": messages,
        "max_tokens": max_tokens,
    });
    if stream {
        payload["stream_options"] = json!({ "include_usage": true });
    }
    let clean_model = model
        .trim()
        .to_ascii_lowercase()
        .rsplit('/')
        .next()
        .unwrap_or("")
        .to_string();
    let is_openai_reasoning = clean_model.starts_with("o1")
        || clean_model.starts_with("o3")
        || clean_model.starts_with("o4")
        || clean_model.starts_with("gpt-5");
    let is_deepseek_native =
        provider == "deepseek" && MintConfig::has_native_thinking_api("deepseek", model);
    let is_openrouter_native =
        provider == "openrouter" && MintConfig::has_native_thinking_api("openrouter", model);

    let (thinking_enabled, thinking_effort) = effective_thinking(config, request, model);
    if is_deepseek_native {
        if thinking_enabled {
            let effort_str = match thinking_effort.as_str() {
                "low" => "low",
                "medium" => "medium",
                "high" => "high",
                "extra_high" => "max",
                _ => "high",
            };
            payload["reasoning_effort"] = json!(effort_str);
            payload["thinking"] = json!({ "type": "enabled" });
        } else {
            payload["reasoning_effort"] = json!("none");
            payload["thinking"] = json!({ "type": "disabled" });
            payload["temperature"] = json!(effective_temperature(config, request));
        }
    } else if is_openrouter_native {
        if thinking_enabled {
            let effort_str = match thinking_effort.as_str() {
                "low" => "low",
                "medium" => "medium",
                "high" => "high",
                "extra_high" => "xhigh",
                _ => "medium",
            };
            payload["reasoning"] = json!({ "effort": effort_str });
        } else {
            payload["reasoning"] = json!({ "exclude": true });
        }
        if !is_openai_reasoning {
            payload["temperature"] = json!(effective_temperature(config, request));
        }
    } else if is_openai_reasoning {
        if thinking_enabled {
            let effort_str = match thinking_effort.as_str() {
                "low" => "low",
                "medium" => "medium",
                "high" | "extra_high" => "high",
                _ => "medium",
            };
            payload["reasoning_effort"] = json!(effort_str);
        } else {
            payload["reasoning_effort"] = json!("low");
        }
    } else {
        payload["temperature"] = json!(effective_temperature(config, request));
    }
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

pub(super) fn openai_native_messages(messages: &[ChatMessage]) -> Result<Vec<Value>, ChatError> {
    let mut result = Vec::new();
    for message in messages {
        match message.role {
            ChatRole::System => {
                result.push(json!({ "role": "system", "content": super::message_text(message) }));
            }
            ChatRole::User => {
                result.push(json!({ "role": "user", "content": openai_user_content(message)? }));
            }
            ChatRole::Assistant => {
                let text = super::message_text(message);
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
                    entry["reasoning_content"] = json!("");
                }
                result.push(entry);
            }
            ChatRole::Tool => {
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

pub(super) fn openai_message_content(
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

pub(super) fn openai_user_content(message: &ChatMessage) -> Result<Value, ChatError> {
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

pub(super) fn openai_audio_part(data_uri: &str) -> Result<Value, ChatError> {
    let (mime_type, data) = split_data_uri(data_uri, "audio/")?;
    Ok(json!({
        "type": "input_audio",
        "input_audio": { "data": data, "format": openai_audio_format(&mime_type) }
    }))
}

pub(super) fn openai_audio_format(mime_type: &str) -> &'static str {
    if mime_type.contains("wav") {
        "wav"
    } else {
        "mp3"
    }
}

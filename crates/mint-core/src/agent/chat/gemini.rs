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

pub(super) async fn call_gemini(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ProviderReply, ChatError> {
    let api_key = provider_key(&config.api_key, "GEMINI_API_KEY");
    required_key("gemini", &api_key)?;
    let model = config.gemini_model.clone();
    let url =
        format!("https://generativelanguage.googleapis.com/v1beta/models/{model}:generateContent");

    let mut payload = if request.messages.is_some() {
        let mut p = gemini_native_payload(request)?;
        let sys = effective_system_instruction(config, request, "gemini", &model);
        p["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        let temp = effective_temperature(config, request);
        let mut gen_cfg = json!({ "temperature": temp });
        gemini_apply_thinking_config(&mut gen_cfg, config, request, &model);
        p["generationConfig"] = gen_cfg;
        p
    } else {
        gemini_chat_payload(config, request)?
    };

    let raw_resp = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .json(&payload)
        .send()
        .await?;
    let response: Value = if raw_resp.status() == reqwest::StatusCode::BAD_REQUEST
        && payload["generationConfig"].get("thinkingConfig").is_some()
    {
        if let Some(gen_obj) = payload["generationConfig"].as_object_mut() {
            gen_obj.remove("thinkingConfig");
        }
        client
            .post(&url)
            .header("x-goog-api-key", &api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?
    } else {
        raw_resp.error_for_status()?.json().await?
    };
    parse_gemini_reply(model, &response)
}

pub(super) async fn stream_gemini_reply<F>(
    client: &Client,
    config: &MintConfig,
    request: &ChatRequest,
    on_event: &mut F,
) -> Result<ProviderReply, ChatError>
where
    F: FnMut(ChatStreamEvent),
{
    let api_key = provider_key(&config.api_key, "GEMINI_API_KEY");
    required_key("gemini", &api_key)?;
    let model = config.gemini_model.clone();
    let mut payload = if request.messages.is_some() {
        let mut p = gemini_native_payload(request)?;
        let sys = effective_system_instruction(config, request, "gemini", &model);
        p["systemInstruction"] = json!({ "parts": [{ "text": sys }] });
        let temp = effective_temperature(config, request);
        let mut gen_cfg = json!({ "temperature": temp });
        gemini_apply_thinking_config(&mut gen_cfg, config, request, &model);
        p["generationConfig"] = gen_cfg;
        p
    } else {
        gemini_chat_payload(config, request)?
    };
    let url = format!(
        "https://generativelanguage.googleapis.com/v1beta/models/{model}:streamGenerateContent?alt=sse"
    );
    let raw_resp = client
        .post(&url)
        .header("x-goog-api-key", &api_key)
        .json(&payload)
        .send()
        .await?;
    let response = if raw_resp.status() == reqwest::StatusCode::BAD_REQUEST
        && payload["generationConfig"].get("thinkingConfig").is_some()
    {
        if let Some(gen_obj) = payload["generationConfig"].as_object_mut() {
            gen_obj.remove("thinkingConfig");
        }
        client
            .post(&url)
            .header("x-goog-api-key", &api_key)
            .json(&payload)
            .send()
            .await?
            .error_for_status()?
    } else {
        raw_resp.error_for_status()?
    };
    collect_stream_reply(response, StreamFormat::Gemini, model, on_event).await
}

/// Strips JSON Schema keywords that standard tool-parameter schemas use (built
/// once and shared across OpenAI/Anthropic/Gemini) but that Gemini's function-
/// calling `Schema` proto rejects outright.
pub(super) fn gemini_sanitize_schema(schema: &Value) -> Value {
    match schema {
        Value::Object(map) => Value::Object(
            map.iter()
                .filter(|(key, _)| key.as_str() != "additionalProperties")
                .map(|(key, value)| (key.clone(), gemini_sanitize_schema(value)))
                .collect(),
        ),
        Value::Array(items) => Value::Array(items.iter().map(gemini_sanitize_schema).collect()),
        other => other.clone(),
    }
}

/// Builds a Gemini `generateContent` payload from structured history, using
/// native `functionDeclarations`/`functionCall`/`functionResponse` instead of
/// the legacy `responseSchema` JSON-prompt scheme.
pub(super) fn gemini_native_payload(request: &ChatRequest) -> Result<Value, ChatError> {
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
                "parameters": gemini_sanitize_schema(&tool.parameters),
            })).collect::<Vec<_>>()
        }]);
    }
    Ok(payload)
}

pub(super) fn gemini_native_contents(messages: &[ChatMessage]) -> Result<Vec<Value>, ChatError> {
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

pub(super) fn gemini_native_part(
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

pub(super) fn gemini_chat_payload(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<Value, ChatError> {
    let temp = effective_temperature(config, request);
    let sys_instruction =
        effective_system_instruction(config, request, "gemini", &config.gemini_model);
    let mut payload = json!({
        "systemInstruction": { "parts": [{ "text": sys_instruction }] },
        "contents": [{ "role": "user", "parts": gemini_parts(request)? }]
    });
    let mut gen_cfg = if wants_agent_json(request) {
        let mut cfg = gemini_agent_generation_config(config);
        cfg["temperature"] = json!(temp);
        cfg
    } else {
        json!({ "temperature": temp })
    };
    gemini_apply_thinking_config(&mut gen_cfg, config, request, &config.gemini_model);
    payload["generationConfig"] = gen_cfg;
    Ok(payload)
}

pub(super) fn gemini_agent_generation_config(config: &MintConfig) -> Value {
    let mut allowed_actions = vec![
        "list_files",
        "read_file",
        "search_code",
        "symbols",
        "find_definition",
        "find_references",
        "semantic_index",
        "semantic_search",
        "knowledge_search",
        "web_search",
        "weather",
        "stock",
        "calculation",
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
        "run_tests",
        "run_typecheck",
        "run_linter",
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
                        "path": { "type": "STRING", "description": "The target file or directory path" },
                        "query": { "type": "STRING", "description": "The search query string" },
                        "city": { "type": "STRING", "description": "The target city or location name" },
                        "expression": { "type": "STRING", "description": "The math expression or percentage calculation" },
                        "url": { "type": "STRING", "description": "The target website URL" },
                        "command": { "type": "STRING", "description": "The local read-only or test shell command to run" },
                        "commands": {
                            "type": "ARRAY",
                            "items": { "type": "STRING" },
                            "description": "List of verification commands"
                        },
                        "steps": {
                            "type": "ARRAY",
                            "items": { "type": "STRING" },
                            "description": "Plan steps"
                        },
                        "fileContent": { "type": "STRING", "description": "The complete new content of a new file" },
                        "summary": { "type": "STRING", "description": "The final detailed answer, explanation, or response to the user's query" },
                        "verification": { "type": "STRING", "description": "The description of checks to run before finishing" },
                        "title": { "type": "STRING", "description": "Short approval or plan title" },
                        "status": { "type": "STRING", "description": "Plan item status or project status" },
                        "startLine": { "type": "INTEGER", "description": "First line to read" },
                        "endLine": { "type": "INTEGER", "description": "Last line to read" },
                        "limit": { "type": "INTEGER", "description": "Max number of items/lines/files to return" },
                        "server": { "type": "STRING", "description": "MCP server name" },
                        "tool": { "type": "STRING", "description": "MCP tool name" },
                        "notePath": { "type": "STRING", "description": "The file path for note writing" },
                        "name": { "type": "STRING", "description": "Plugin name" },
                        "instruction": { "type": "STRING", "description": "Instruction to run the plugin" },
                        "patch": {
                            "type": "OBJECT",
                            "properties": {
                                "path": { "type": "STRING", "description": "The target file path" },
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
                        "arguments": { "type": "OBJECT", "description": "MCP tool arguments" }
                    },
                    "required": ["path", "query", "command", "fileContent", "summary"]
                }
            },
            "required": ["thought", "action", "input"]
        }
    })
}

pub(super) fn gemini_apply_thinking_config(
    gen_cfg: &mut Value,
    config: &MintConfig,
    request: &ChatRequest,
    model: &str,
) {
    if !MintConfig::has_native_thinking_api("gemini", model) {
        return;
    }
    let (enabled, effort) = effective_thinking(config, request, model);
    if !enabled {
        gen_cfg["thinkingConfig"] = json!({
            "includeThoughts": false,
            "thinkingBudget": 0
        });
    } else {
        let budget = match effort.as_str() {
            "low" => 1024,
            "medium" => 4096,
            "high" => 16384,
            "extra_high" => 32768,
            _ => 4096,
        };
        gen_cfg["thinkingConfig"] = json!({
            "includeThoughts": true,
            "thinkingBudget": budget
        });
    }
}

pub(super) fn gemini_parts(request: &ChatRequest) -> Result<Vec<Value>, ChatError> {
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

pub(super) fn parse_gemini_reply(
    model: String,
    response: &Value,
) -> Result<ProviderReply, ChatError> {
    let parts = response["candidates"][0]["content"]["parts"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let mut text = String::new();
    let mut thought_parts: Vec<&str> = Vec::new();
    let mut tool_calls = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        let is_thought = part
            .get("thought")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if let Some(t) = part["text"].as_str() {
            if is_thought {
                thought_parts.push(t);
            } else {
                text.push_str(t);
            }
        }
        if let Some(call) = part.get("functionCall") {
            tool_calls.push(ToolCall {
                id: format!("call_{index}"),
                name: call["name"].as_str().unwrap_or_default().to_string(),
                input: call["args"].clone(),
                thought_signature: part
                    .get("thoughtSignature")
                    .and_then(Value::as_str)
                    .map(str::to_owned),
            });
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
                stop_reason: response["candidates"][0]["finishReason"]
                    .as_str()
                    .map(str::to_owned),
                input_tokens: response["usageMetadata"]["promptTokenCount"]
                    .as_u64()
                    .map(|n| n as u32),
                output_tokens: response["usageMetadata"]["candidatesTokenCount"]
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
        stop_reason: response["candidates"][0]["finishReason"]
            .as_str()
            .map(str::to_owned),
        input_tokens: response["usageMetadata"]["promptTokenCount"]
            .as_u64()
            .map(|n| n as u32),
        output_tokens: response["usageMetadata"]["candidatesTokenCount"]
            .as_u64()
            .map(|n| n as u32),
    })
}

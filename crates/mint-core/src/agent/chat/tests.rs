use super::anthropic::*;
use super::gemini::*;
use super::ollama::*;
use super::openai::*;
use super::streaming::*;
use super::*;
use reqwest::Client;
use serde_json::json;

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
        pinned_mcp_server: None,
        messages: None,
        tools: None,
        temperature: None,
        ..Default::default()
    };

    let error = require_supported_attachments("openai", &request).unwrap_err();
    assert!(matches!(error, ChatError::UnsupportedAttachments(provider) if provider == "openai"));
}

#[test]
fn parses_stream_provider_formats() {
    let openai = parse_stream_value(
        StreamFormat::OpenAi,
        r#"data: {"choices":[{"delta":{"content":"hello"}}]}"#,
    )
    .unwrap();
    assert_eq!(openai["choices"][0]["delta"]["content"], "hello");
    let ollama =
        parse_stream_value(StreamFormat::Ollama, r#"{"message":{"content":"hi"}}"#).unwrap();
    assert_eq!(ollama["message"]["content"], "hi");
    assert!(parse_stream_value(StreamFormat::Anthropic, "data: [DONE]").is_none());
}

#[test]
fn malformed_tool_arguments_survive_provider_parsing_for_rejection() {
    let malformed = r#"{"path":"~/Downloads/YOUTUBE_DL""#;
    let function = json!({"name": "list_files", "arguments": malformed});
    let openai = parse_openai_style_reply(
        "test".into(),
        &json!({
            "choices": [{"message": {"tool_calls": [{"id": "call_1", "function": function}]}}]
        }),
    )
    .unwrap();
    let ollama = parse_ollama_reply(
        "test".into(),
        &json!({
            "message": {"tool_calls": [{"function": function}]}
        }),
    )
    .unwrap();
    let mut accumulator = StreamReplyAccumulator::default();
    accumulator.consume_openai(&json!({
            "choices": [{"delta": {"tool_calls": [{"index": 0, "id": "call_1", "function": function}]}}]
        }), &mut |_| {});
    let streamed = accumulator.finish("test".into()).unwrap();
    for reply in [openai, ollama, streamed] {
        let calls = reply.tool_calls.unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].input, Value::String(malformed.into()));
    }
    assert_eq!(decode_tool_arguments(&json!("{}")), json!({}));
    assert_eq!(decode_tool_arguments(&Value::Null), Value::Null);
}

#[test]
fn accumulates_openai_reasoning_tools_and_usage() {
    let mut accumulator = StreamReplyAccumulator::default();
    let mut events = Vec::new();
    accumulator.consume_openai(
        &json!({
            "choices": [{"delta": {"reasoning_content": "inspect "}}]
        }),
        &mut |event| events.push(event),
    );
    accumulator.consume_openai(&json!({
            "choices": [{"delta": {"reasoning_content": "files", "tool_calls": [{
                "index": 0, "id": "call_1", "function": {"name": "read_file", "arguments": "{\"path\":"}
            }]}}]
        }), &mut |event| events.push(event));
    accumulator.consume_openai(
        &json!({
            "choices": [{"delta": {"tool_calls": [{
                "index": 0, "function": {"arguments": "\"README.md\"}"}
            }]}, "finish_reason": "tool_calls"}],
            "usage": {"prompt_tokens": 10, "completion_tokens": 5}
        }),
        &mut |event| events.push(event),
    );
    accumulator.flush_tag_buffer(&mut |event| events.push(event));
    let reply = accumulator.finish("test".into()).unwrap();
    assert_eq!(reply.thought.as_deref(), Some("inspect files"));
    assert_eq!(reply.stop_reason.as_deref(), Some("tool_calls"));
    assert_eq!(reply.input_tokens, Some(10));
    assert_eq!(reply.tool_calls.unwrap()[0].input["path"], "README.md");
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ChatStreamEvent::ReasoningDelta { .. }))
            .count(),
        2
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, ChatStreamEvent::ToolCallDelta { .. }))
            .count(),
        2
    );
}

#[test]
fn streams_think_tags_split_across_chunks() {
    let mut accumulator = StreamReplyAccumulator::default();
    let mut events = Vec::new();
    for chunk in ["ก่อน<thi", "nk>คิด", "อยู่</th", "ink>หลัง"] {
        accumulator.push_text(chunk, &mut |event| events.push(event));
    }
    accumulator.flush_tag_buffer(&mut |event| events.push(event));
    let reply = accumulator.finish("test".into()).unwrap();
    assert_eq!(reply.text, "ก่อนหลัง");
    assert_eq!(reply.thought.as_deref(), Some("คิดอยู่"));
    let streamed_reasoning: String = events
        .iter()
        .filter_map(|event| match event {
            ChatStreamEvent::ReasoningDelta { delta } => Some(delta.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(streamed_reasoning, "คิดอยู่");
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
        pinned_mcp_server: None,
        messages: None,
        tools: None,
        temperature: None,
        ..Default::default()
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
        pinned_mcp_server: None,
        messages: Some(messages),
        tools,
        temperature: None,
        ..Default::default()
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
    let cfg = MintConfig::default();
    let payload = anthropic_chat_payload(&cfg, "claude-x", &request, false).unwrap();

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
    let cfg = MintConfig::default();
    let payload = anthropic_chat_payload(&cfg, "claude-x", &request, false).unwrap();

    assert_eq!(payload["system"][0]["type"], "text");
    assert_eq!(payload["system"][0]["text"], "You are Mint.");
    assert_eq!(payload["system"][0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn review_anthropic_tool_results_preserve_failure_and_success_flags() {
    for (text, is_error) in [
        ("User denied MCP tool call", true),
        ("MCP call was not approved; user feedback", true),
        ("Error: literal diagnostic label", false),
    ] {
        let request = native_request(
            vec![ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: "mcp-call".into(),
                    content: text.into(),
                    is_error,
                }],
            }],
            None,
        );
        let payload =
            anthropic_chat_payload(&MintConfig::default(), "claude-x", &request, false).unwrap();
        assert_eq!(payload["messages"][0]["content"][0]["is_error"], is_error);
        assert_eq!(payload["messages"][0]["content"][0]["content"], text);
    }
}

#[test]
fn anthropic_payload_omits_the_cache_breakpoint_for_an_empty_system_prompt() {
    let mut request = native_request(vec![ChatMessage::text(ChatRole::User, "hi")], None);
    request.system_instruction = String::new();
    let cfg = MintConfig::default();
    let payload = anthropic_chat_payload(&cfg, "claude-x", &request, false).unwrap();

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
    let cfg = MintConfig::default();
    let payload = anthropic_chat_payload(&cfg, "claude-x", &request, false).unwrap();

    let tools = payload["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 2);
    assert!(tools[0].get("cache_control").is_none());
    assert_eq!(tools[1]["cache_control"]["type"], "ephemeral");
}

#[test]
fn anthropic_attaches_cache_control_to_penultimate_turn() {
    let request = native_request(
        vec![
            ChatMessage::text(ChatRole::User, "step 1 user"),
            ChatMessage::text(ChatRole::Assistant, "step 1 assistant"),
            ChatMessage::text(ChatRole::User, "step 2 user"),
        ],
        None,
    );
    let cfg = MintConfig::default();
    let payload = anthropic_chat_payload(&cfg, "claude-x", &request, false).unwrap();
    let messages = payload["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert!(messages[0]["content"][0].get("cache_control").is_none());
    assert_eq!(
        messages[1]["content"][0]["cache_control"]["type"],
        "ephemeral"
    );
    assert!(messages[2]["content"][0].get("cache_control").is_none());
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
    let cfg = MintConfig::default();
    let payload = openai_chat_payload(&cfg, "gpt-x", &request, false).unwrap();

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
    // `Image` block attached to a tool result (e.g. `view_image`) must
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
fn openai_native_messages_attaches_reasoning_content_for_tool_calls() {
    let messages = vec![ChatMessage {
        role: ChatRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "call_abc".into(),
            name: "read_file".into(),
            input: json!({"path": "src/main.rs"}),
            thought_signature: None,
        }],
    }];
    let built = openai_native_messages(&messages).unwrap();
    assert_eq!(built.len(), 1);
    assert_eq!(built[0]["role"], "assistant");
    assert_eq!(built[0]["reasoning_content"], json!(""));
    assert_eq!(built[0]["tool_calls"][0]["function"]["name"], "read_file");
}

#[test]
fn openai_serializes_compacted_history_with_role_alternation() {
    let messages = vec![
        ChatMessage::text(ChatRole::User, "Fix the bug"),
        ChatMessage::text(
            ChatRole::Assistant,
            "Summary of earlier steps:\n- read file.rs\n- found syntax error",
        ),
        ChatMessage::text(
            ChatRole::User,
            "Please continue the task from where you left off based on the summary above.",
        ),
        ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_1".into(),
                name: "apply_patch".into(),
                input: json!({"path": "file.rs", "patch": "..."}),
                thought_signature: None,
            }],
        },
        ChatMessage {
            role: ChatRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "Patch applied".into(),
                is_error: false,
            }],
        },
    ];
    let built = openai_native_messages(&messages).unwrap();
    assert_eq!(built.len(), 5);
    assert_eq!(built[0]["role"], "user");
    assert_eq!(built[1]["role"], "assistant");
    assert_eq!(built[2]["role"], "user");
    assert_eq!(built[3]["role"], "assistant");
    assert_eq!(built[4]["role"], "tool");
}

#[test]
fn anthropic_serializes_compacted_history_with_role_alternation() {
    let messages = vec![
        ChatMessage::text(ChatRole::User, "Fix the bug"),
        ChatMessage::text(ChatRole::Assistant, "Summary of earlier steps"),
        ChatMessage::text(ChatRole::User, "Please continue"),
        ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "toolu_1".into(),
                name: "read_file".into(),
                input: json!({"path": "file.rs"}),
                thought_signature: None,
            }],
        },
        ChatMessage {
            role: ChatRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "toolu_1".into(),
                content: "Code content".into(),
                is_error: false,
            }],
        },
    ];
    let built = anthropic_messages(&messages).unwrap();
    assert_eq!(built.len(), 5);
    assert_eq!(built[0]["role"], "user");
    assert_eq!(built[1]["role"], "assistant");
    assert_eq!(built[2]["role"], "user");
    assert_eq!(built[3]["role"], "assistant");
    assert_eq!(built[4]["role"], "user");
}

#[test]
fn gemini_serializes_compacted_history_with_role_alternation() {
    let messages = vec![
        ChatMessage::text(ChatRole::User, "Fix the bug"),
        ChatMessage::text(ChatRole::Assistant, "Summary of earlier steps"),
        ChatMessage::text(ChatRole::User, "Please continue"),
        ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::ToolUse {
                id: "call_1".into(),
                name: "read_file".into(),
                input: json!({"path": "file.rs"}),
                thought_signature: None,
            }],
        },
        ChatMessage {
            role: ChatRole::Tool,
            content: vec![ContentBlock::ToolResult {
                tool_use_id: "call_1".into(),
                content: "Code content".into(),
                is_error: false,
            }],
        },
    ];
    let built = gemini_native_contents(&messages).unwrap();
    assert!(built.len() >= 4);
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
        pinned_mcp_server: None,
        messages: None,
        tools: None,
        temperature: None,
        ..Default::default()
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
        pinned_mcp_server: None,
        messages: None,
        tools: None,
        temperature: None,
        ..Default::default()
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
        openai_message_content("what is this?", None, Some("data:audio/mp3;base64,QUJD")).unwrap();
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
fn gemini_sanitize_schema_strips_additional_properties_at_any_depth() {
    let schema = json!({
        "type": "object",
        "properties": {
            "emotions": {
                "type": "object",
                "additionalProperties": { "type": "string" }
            },
            "nested": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": true
                }
            }
        }
    });
    let sanitized = gemini_sanitize_schema(&schema);
    assert!(
        sanitized["properties"]["emotions"]
            .get("additionalProperties")
            .is_none()
    );
    assert!(
        sanitized["properties"]["nested"]["items"]
            .get("additionalProperties")
            .is_none()
    );
    // Everything else is preserved untouched.
    assert_eq!(sanitized["properties"]["emotions"]["type"], "object");
}

#[test]
fn gemini_native_payload_strips_additional_properties_from_tool_schemas() {
    let tool = ToolSpec {
        name: "avatar_signal".into(),
        description: "set avatar state".into(),
        parameters: json!({
            "type": "object",
            "properties": {
                "emotions": {
                    "type": "object",
                    "additionalProperties": { "type": "string" }
                }
            }
        }),
    };
    let request = native_request(
        vec![ChatMessage::text(ChatRole::User, "hi")],
        Some(vec![tool]),
    );
    let payload = gemini_native_payload(&request).unwrap();
    assert!(
        payload["tools"][0]["functionDeclarations"][0]["parameters"]["properties"]["emotions"]
            .get("additionalProperties")
            .is_none()
    );
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
    // `Image` block attached to a tool result (e.g. `view_image`)
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
fn extracts_input_and_output_tokens_from_each_provider_usage_shape() {
    let anthropic = parse_anthropic_reply(
        "claude-x".into(),
        &json!({
            "content": [{ "type": "text", "text": "hi" }],
            "usage": { "input_tokens": 100, "output_tokens": 20 }
        }),
    )
    .unwrap();
    assert_eq!(
        (anthropic.input_tokens, anthropic.output_tokens),
        (Some(100), Some(20))
    );

    let openai = parse_openai_style_reply(
        "gpt-x".into(),
        &json!({
            "choices": [{ "message": { "content": "hi" } }],
            "usage": { "prompt_tokens": 130, "completion_tokens": 20, "total_tokens": 150 }
        }),
    )
    .unwrap();
    assert_eq!(
        (openai.input_tokens, openai.output_tokens),
        (Some(130), Some(20))
    );

    let gemini = parse_gemini_reply(
        "gemini-x".into(),
        &json!({
            "candidates": [{ "content": { "parts": [{ "text": "hi" }] } }],
            "usageMetadata": {
                "promptTokenCount": 160,
                "candidatesTokenCount": 40,
                "totalTokenCount": 200
            }
        }),
    )
    .unwrap();
    assert_eq!(
        (gemini.input_tokens, gemini.output_tokens),
        (Some(160), Some(40))
    );

    let ollama = parse_ollama_reply(
        "llama3.1".into(),
        &json!({
            "message": { "content": "hi" },
            "prompt_eval_count": 80,
            "eval_count": 40
        }),
    )
    .unwrap();
    assert_eq!(
        (ollama.input_tokens, ollama.output_tokens),
        (Some(80), Some(40))
    );
}
#[test]
fn anthropic_cached_prompt_tokens_count_toward_context_in_json_and_streams() {
    let usage = json!({"input_tokens":100,"cache_read_input_tokens":98_000,"cache_creation_input_tokens":1_000,"output_tokens":400,"cache_creation":{"ephemeral_5m_input_tokens":1_000}});
    let reply = parse_anthropic_reply(
        "claude-test".into(),
        &json!({"content":[{"type":"text","text":"Ready"}],"usage":usage}),
    )
    .unwrap();
    assert_eq!(reply.input_tokens, Some(99_100));
    let mut stream = StreamReplyAccumulator::default();
    stream.consume_anthropic(
        &json!({"type":"message_start","message":{"usage":usage}}),
        &mut |_| {},
    );
    stream.consume_anthropic(
        &json!({"type":"content_block_delta","delta":{"type":"text_delta","text":"Ready"}}),
        &mut |_| {},
    );
    stream.consume_anthropic(
        &json!({"type":"message_delta","usage":{"output_tokens":400}}),
        &mut |_| {},
    );
    let reply = stream.finish("claude-test".into()).unwrap();
    assert_eq!(reply.input_tokens, Some(99_100));
    assert_eq!(reply.output_tokens, Some(400));
}

#[test]
fn missing_usage_data_yields_no_token_counts() {
    let reply = parse_openai_style_reply(
        "gpt-x".into(),
        &json!({ "choices": [{ "message": { "content": "hi" } }] }),
    )
    .unwrap();
    assert_eq!((reply.input_tokens, reply.output_tokens), (None, None));
}

#[tokio::test]
async fn send_openai_style_request_classifies_a_connect_failure_as_network_unavailable() {
    let client = Client::new();
    // Port 1 is reserved and nothing listens on it, so this fails fast
    // with a connection error rather than timing out.
    let result = send_openai_style_request(
        client.post("http://127.0.0.1:1/chat/completions"),
        "deepseek",
    )
    .await;
    assert!(
        matches!(result, Err(ChatError::NetworkUnavailable)),
        "expected NetworkUnavailable, got {result:?}"
    );
}

#[tokio::test]
async fn send_openai_style_request_classifies_402_as_insufficient_balance() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let mut buf = [0u8; 1024];
        let _ = socket.read(&mut buf).await;
        let body = "{}";
        let response = format!(
            "HTTP/1.1 402 Payment Required\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = socket.write_all(response.as_bytes()).await;
        let _ = socket.shutdown().await;
    });

    let client = Client::new();
    let result = send_openai_style_request(
        client.post(format!("http://{addr}/chat/completions")),
        "deepseek",
    )
    .await;
    assert!(
        matches!(&result, Err(ChatError::InsufficientBalance(p)) if p == "deepseek"),
        "expected InsufficientBalance(\"deepseek\"), got {result:?}"
    );
}

#[test]
fn fallback_provider_order_moves_gemini_to_the_front() {
    let config = MintConfig {
        anthropic_api_key: "a".into(),
        openai_api_key: "b".into(),
        deepseek_api_key: "c".into(),
        api_key: "d".into(), // gemini
        hf_api_key: "e".into(),
        ..MintConfig::default()
    };
    let order = fallback_provider_order(&config);
    assert_eq!(order[0], "gemini");
    // Everyone else keeps their existing relative order from
    // `available_providers()` — only gemini moves.
    let without_gemini: Vec<&String> = order.iter().skip(1).collect();
    assert_eq!(
        without_gemini,
        vec!["anthropic", "openai", "deepseek", "huggingface", "ollama"]
    );
}

#[test]
fn fallback_provider_order_is_unchanged_without_gemini_configured() {
    let config = MintConfig {
        deepseek_api_key: "c".into(),
        hf_api_key: "e".into(),
        ..MintConfig::default()
    };
    assert_eq!(
        fallback_provider_order(&config),
        config.available_providers()
    );
}

#[test]
fn openai_payload_includes_smart_temperature_for_deepseek_and_omits_for_o1() {
    let request = native_request(vec![ChatMessage::text(ChatRole::User, "hi")], None);

    // DeepSeek smart default is 0.6
    let ds_config = MintConfig {
        ai_provider: "deepseek".into(),
        deepseek_model: "deepseek-chat".into(),
        ..MintConfig::default()
    };
    let ds_payload = openai_chat_payload(&ds_config, "deepseek-chat", &request, false).unwrap();
    assert_eq!(ds_payload["temperature"], 0.6);

    // Standard OpenAI model default is 0.2
    let openai_config = MintConfig {
        ai_provider: "openai".into(),
        openai_model: "gpt-4o".into(),
        ..MintConfig::default()
    };
    let openai_payload = openai_chat_payload(&openai_config, "gpt-4o", &request, false).unwrap();
    assert_eq!(openai_payload["temperature"], 0.2);

    // Reasoning models (o1, o3) omit temperature parameter
    let o1_payload = openai_chat_payload(&openai_config, "o1-preview", &request, false).unwrap();
    assert!(o1_payload.get("temperature").is_none());

    let o3_payload = openai_chat_payload(&openai_config, "o3-mini", &request, false).unwrap();
    assert!(o3_payload.get("temperature").is_none());

    // Explicit request temperature override
    let mut override_req = request.clone();
    override_req.temperature = Some(0.75);
    let override_payload =
        openai_chat_payload(&ds_config, "deepseek-chat", &override_req, false).unwrap();
    assert_eq!(override_payload["temperature"], 0.75);
}

#[test]
fn anthropic_payload_includes_effective_temperature() {
    let request = native_request(vec![ChatMessage::text(ChatRole::User, "hi")], None);
    let config = MintConfig {
        ai_provider: "anthropic".into(),
        anthropic_model: "claude-sonnet-5".into(),
        thinking_enabled: false,
        ..MintConfig::default()
    };
    let payload = anthropic_chat_payload(&config, "claude-sonnet-5", &request, false).unwrap();
    assert_eq!(payload["temperature"], 0.2);
}

#[test]
fn parse_openai_style_reply_extracts_deepseek_reasoning_content() {
    let response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "",
                "reasoning_content": "Evaluating 15 + 6 * (8 - 3) - 14 / 2 step by step...",
                "tool_calls": [{
                    "id": "call_calc_1",
                    "type": "function",
                    "function": {
                        "name": "calculation",
                        "arguments": "{\"expression\":\"15 + 6 * (8 - 3) - 14 / 2\"}"
                    }
                }]
            },
            "finish_reason": "tool_calls"
        }]
    });
    let reply = parse_openai_style_reply("deepseek-reasoner".into(), &response).unwrap();
    assert_eq!(reply.text, "");
    assert_eq!(
        reply.thought.as_deref(),
        Some("Evaluating 15 + 6 * (8 - 3) - 14 / 2 step by step...")
    );
    let calls = reply.tool_calls.unwrap();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].name, "calculation");
}

#[test]
fn parse_openai_style_reply_extracts_openrouter_reasoning() {
    let response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "The answer is 38.",
                "reasoning": "First parentheses: 8 - 3 = 5..."
            },
            "finish_reason": "stop"
        }]
    });
    let reply = parse_openai_style_reply("deepseek/deepseek-r1".into(), &response).unwrap();
    assert_eq!(reply.text, "The answer is 38.");
    assert_eq!(
        reply.thought.as_deref(),
        Some("First parentheses: 8 - 3 = 5...")
    );
}

#[test]
fn parse_openai_style_reply_extracts_think_tags() {
    let response = json!({
        "choices": [{
            "message": {
                "role": "assistant",
                "content": "<think>\nLet me solve this problem.\n</think>\nThe result is 42."
            },
            "finish_reason": "stop"
        }]
    });
    let reply = parse_openai_style_reply("qwq-32b".into(), &response).unwrap();
    assert_eq!(reply.text, "The result is 42.");
    assert_eq!(reply.thought.as_deref(), Some("Let me solve this problem."));
}

#[test]
fn parse_gemini_reply_extracts_thought_parts() {
    let response = json!({
        "candidates": [{
            "content": {
                "parts": [
                    { "text": "Thinking through the formula...", "thought": true },
                    { "text": "The answer is 38." }
                ]
            },
            "finishReason": "STOP"
        }]
    });
    let reply = parse_gemini_reply("gemini-2.0-flash-thinking".into(), &response).unwrap();
    assert_eq!(reply.text, "The answer is 38.");
    assert_eq!(
        reply.thought.as_deref(),
        Some("Thinking through the formula...")
    );
}

#[test]
fn parse_anthropic_reply_extracts_thinking_blocks() {
    let response = json!({
        "content": [
            { "type": "thinking", "thinking": "Let's break down the arithmetic..." },
            { "type": "text", "text": "The result is 38." }
        ],
        "stop_reason": "end_turn"
    });
    let reply = parse_anthropic_reply("claude-3-7-sonnet".into(), &response).unwrap();
    assert_eq!(reply.text, "The result is 38.");
    assert_eq!(
        reply.thought.as_deref(),
        Some("Let's break down the arithmetic...")
    );
}

#[test]
fn parse_ollama_reply_extracts_thinking_field() {
    let response = json!({
        "message": {
            "role": "assistant",
            "content": "Result is 38.",
            "thinking": "Ollama internal thinking trace..."
        }
    });
    let reply = parse_ollama_reply("deepseek-r1:8b".into(), &response).unwrap();
    assert_eq!(reply.text, "Result is 38.");
    assert_eq!(
        reply.thought.as_deref(),
        Some("Ollama internal thinking trace...")
    );
}

#[test]
fn decode_tool_arguments_sanitizes_fences_tokens_and_trailing_commas() {
    // 1. Markdown fences
    let fenced =
        Value::String("```json\n{\"command\": \"curl -s https://example.com\"}\n```".into());
    let decoded = decode_tool_arguments(&fenced);
    assert_eq!(decoded["command"], "curl -s https://example.com");

    // 2. DeepSeek DSML token
    let dsml = Value::String("{\"command\": \"ls -la\"}</｜｜DSML｜｜".into());
    let decoded_dsml = decode_tool_arguments(&dsml);
    assert_eq!(decoded_dsml["command"], "ls -la");

    // 3. Trailing commas
    let trailing = Value::String("{\"command\": \"echo ok\", \"flag\": true, }".into());
    let decoded_trailing = decode_tool_arguments(&trailing);
    assert_eq!(decoded_trailing["command"], "echo ok");
    assert_eq!(decoded_trailing["flag"], true);
}

#[test]
fn gemini_thinking_config_includes_thoughts_flag_and_skips_non_native_models() {
    let mut config = MintConfig::default();
    config.ai_provider = "gemini".into();
    config.gemini_model = "gemini-2.5-flash".into();

    let request_on = ChatRequest {
        message: "Solve 12 * 13".into(),
        thinking_enabled: Some(true),
        thinking_effort: Some("high".into()),
        ..Default::default()
    };
    let payload_on = gemini_chat_payload(&config, &request_on).unwrap();
    assert_eq!(
        payload_on["generationConfig"]["thinkingConfig"]["includeThoughts"],
        true
    );
    assert_eq!(
        payload_on["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        16384
    );

    let request_off = ChatRequest {
        message: "Hello".into(),
        thinking_enabled: Some(false),
        ..Default::default()
    };
    let payload_off = gemini_chat_payload(&config, &request_off).unwrap();
    assert_eq!(
        payload_off["generationConfig"]["thinkingConfig"]["includeThoughts"],
        false
    );
    assert_eq!(
        payload_off["generationConfig"]["thinkingConfig"]["thinkingBudget"],
        0
    );

    // Non-text image generation model does not attach thinkingConfig or prompt directive
    config.gemini_model = "nano-banana-pro-preview".into();
    let payload_banana = gemini_chat_payload(&config, &request_on).unwrap();
    assert!(
        payload_banana["generationConfig"]
            .get("thinkingConfig")
            .is_none()
    );
    let banana_sys = payload_banana["systemInstruction"]["parts"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(!banana_sys.contains("<think>"));
}

#[test]
fn deepseek_v4_and_prompt_fallback_thinking_payloads() {
    let mut config = MintConfig::default();
    config.ai_provider = "deepseek".into();
    config.deepseek_model = "deepseek-flash".into();

    let req_on = ChatRequest {
        message: "Explain quantum entanglement".into(),
        system_instruction: "You are helpful.".into(),
        thinking_enabled: Some(true),
        thinking_effort: Some("extra_high".into()),
        ..Default::default()
    };
    let payload_on = openai_chat_payload(&config, "deepseek-flash", &req_on, false).unwrap();
    assert_eq!(payload_on["reasoning_effort"], "max");
    assert_eq!(payload_on["thinking"]["type"], "enabled");
    assert!(payload_on.get("temperature").is_none());
    // Native thinking model should not get prompt-fallback directive
    let sys_on = payload_on["messages"][0]["content"].as_str().unwrap();
    assert_eq!(sys_on, "You are helpful.");

    let req_off = ChatRequest {
        message: "Hi".into(),
        system_instruction: "You are helpful.".into(),
        thinking_enabled: Some(false),
        ..Default::default()
    };
    let payload_off = openai_chat_payload(&config, "deepseek-flash", &req_off, false).unwrap();
    assert_eq!(payload_off["reasoning_effort"], "none");
    assert_eq!(payload_off["thinking"]["type"], "disabled");
    assert!(payload_off.get("temperature").is_some());

    // Non-native chat model (`gpt-4o`) with thinking_enabled = true gets prompt-based CoT directive
    config.ai_provider = "openai".into();
    config.openai_model = "gpt-4o".into();
    let payload_fallback = openai_chat_payload(&config, "gpt-4o", &req_on, false).unwrap();
    let sys_fallback = payload_fallback["messages"][0]["content"].as_str().unwrap();
    assert!(sys_fallback.contains("[Thinking Mode: Enabled (Extra High Effort)]"));
    assert!(sys_fallback.contains("<think>...</think>"));
}

#[test]
fn extract_think_tag_supports_both_think_and_thought_tags() {
    let (thought1, cleaned1) = extract_think_tag("<think>\nStep 1\n</think>\nFinal answer 1");
    assert_eq!(cleaned1, "Final answer 1");
    assert_eq!(thought1.as_deref(), Some("Step 1"));

    let (thought2, cleaned2) = extract_think_tag("<thought>\nStep 2\n</thought>\nFinal answer 2");
    assert_eq!(cleaned2, "Final answer 2");
    assert_eq!(thought2.as_deref(), Some("Step 2"));
}

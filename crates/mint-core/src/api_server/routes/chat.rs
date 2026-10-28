use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiChatRequest {
    message: String,
    attachment_metadata: Option<crate::library::AttachmentMetadata>,
    system_instruction: Option<String>,
    chat_id: Option<String>,
    image_data_uri: Option<String>,
    audio_data_uri: Option<String>,
    video_data_uri: Option<String>,
    document_attachment: Option<crate::chat::DocumentAttachment>,
    #[serde(default)]
    workspace_path: Option<String>,
    agent_id: Option<String>,
    #[serde(default)]
    plan_mode: bool,
    #[serde(default)]
    pinned_mcp_server: Option<String>,
    #[serde(default)]
    temperature: Option<f64>,
    #[serde(default)]
    thinking_enabled: Option<bool>,
    #[serde(default)]
    thinking_effort: Option<String>,
}

impl ApiChatRequest {
    fn into_chat_request(self, config: &MintConfig) -> ChatRequest {
        ChatRequest {
            message: self.message,
            system_instruction: self.system_instruction.unwrap_or_default(),
            chat_id: self.chat_id,
            image_data_uri: self.image_data_uri,
            audio_data_uri: self.audio_data_uri,
            video_data_uri: self.video_data_uri,
            document_attachment: self.document_attachment,
            workspace_path: self.workspace_path,
            agent_id: self.agent_id,
            plan_mode: self.plan_mode,
            pinned_mcp_server: self.pinned_mcp_server,
            messages: None,
            tools: None,
            temperature: self.temperature.or(config.temperature),
            thinking_enabled: self.thinking_enabled,
            thinking_effort: self.thinking_effort,
            ..Default::default()
        }
    }
}

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, mut socket: TcpStream) {
    let owner = match background_request_owner(ctx.request_str) {
        Ok(owner) => owner,
        Err(message) => {
            send_json_response(
                socket,
                "401 Unauthorized",
                &json!({"message": message}).to_string(),
            )
            .await;
            return;
        }
    };
    if ctx.method == "POST" && matches!(ctx.route, "/api/chat" | "/api/chat-stream") {
        if let Ok(value) = serde_json::from_str::<Value>(ctx.body) {
            let chat = value
                .get("chatId")
                .or_else(|| value.get("chat_id"))
                .and_then(Value::as_str)
                .unwrap_or(DEFAULT_CONVERSATION_ID);
            if !crate::bg_shell::bind_chat_owner(chat, owner) {
                send_json_response(
                    socket,
                    "403 Forbidden",
                    "{\"message\":\"Conversation belongs to another user\"}",
                )
                .await;
                return;
            }
        }
    }

    let RequestCtx {
        method,
        route,
        query: _query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label,
    } = ctx;
    match (method, route) {
        ("POST", "/api/chat") => {
            if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                let config = load_config().unwrap_or_default();
                let metadata = req.attachment_metadata.clone();
                let chat_req = req.into_chat_request(&config);
                let (mut chat_req, receipt) =
                    match crate::library::prepare_chat(&chat_req, metadata.as_ref(), &config).await
                    {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            log_api_err("API /api/chat document error", &error);
                            let err_json = serde_json::json!({
                                "text": format!("Could not read the attached document: {error}")
                            });
                            send_json_response(socket, "400 Bad Request", &err_json.to_string())
                                .await;
                            return;
                        }
                    };

                let expected_chat_id = chat_req
                    .chat_id
                    .clone()
                    .unwrap_or_else(|| DEFAULT_CONVERSATION_ID.into());
                let response = crate::with_turn_start_listener(
                    expected_chat_id,
                    move |id| {
                        if let Some(receipt) = &receipt
                            && let Err(error) = receipt.bind(id)
                        {
                            eprintln!("Library reference: {error}")
                        }
                    },
                    async {
                        if let Some(clean_message) =
                            chat_req.message.strip_prefix("/chat ").map(str::to_owned)
                        {
                            chat_req.message = clean_message;
                            if chat_req.system_instruction.trim().is_empty() {
                                chat_req.system_instruction = default_chat_system_instruction();
                            }
                            orchestrate_chat_with_fallback(&config, &chat_req)
                                .await
                                .map(|(response, _)| response)
                                .map_err(|error| error.to_string())
                        } else {
                            run_web_agent_loop(&config, &chat_req).await
                        }
                    },
                )
                .await;

                match response {
                    Ok(resp) => {
                        log_api_req(
                            "POST",
                            "/api/chat",
                            "200 OK",
                            Some(&format!("Model: {} | {}", config.ai_provider, auth_label)),
                        );
                        if let Ok(json_str) = serde_json::to_string(&resp) {
                            send_json_response(socket, "200 OK", &json_str).await;
                            return;
                        }
                    }
                    Err(e) => {
                        log_api_err("API /api/chat error", &e);
                        let err_json = serde_json::json!({
                            "provider": "error",
                            "model": "error",
                            "text": format!("Error orchestrating chat: {e}")
                        });
                        send_json_response(
                            socket,
                            "500 Internal Server Error",
                            &err_json.to_string(),
                        )
                        .await;
                        return;
                    }
                }
            }
            send_json_response(
                socket,
                "400 Bad Request",
                "{\"status\":\"invalid chat request body\"}",
            )
            .await;
        }

        ("POST", "/api/cancel-chat") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct CancelRequest {
                chat_id: String,
            }
            if let Ok(req) = serde_json::from_str::<CancelRequest>(body) {
                crate::cancel_agent(&req.chat_id);
                send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
            } else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"status\":\"invalid cancel body\"}",
                )
                .await;
            }
        }

        ("POST", "/api/submit-approval") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct SubmitApprovalRequest {
                token: String,
                approved: bool,
                #[serde(default)]
                answer: Option<String>,
            }
            if let Ok(req) = serde_json::from_str::<SubmitApprovalRequest>(body) {
                let ok = crate::resolve_pending_approval(&req.token, req.approved, req.answer);
                send_json_response(socket, "200 OK", &format!("{{\"ok\":{ok}}}")).await;
            } else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"status\":\"invalid submit-approval body\"}",
                )
                .await;
            }
        }

        ("POST", "/api/chat-stream") => {
            if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                let config = load_config().unwrap_or_default();
                let metadata = req.attachment_metadata.clone();
                let chat_req = req.into_chat_request(&config);
                let (mut chat_req, receipt) =
                    match crate::library::prepare_chat(&chat_req, metadata.as_ref(), &config).await
                    {
                        Ok(prepared) => prepared,
                        Err(error) => {
                            log_api_err("API /api/chat-stream document error", &error);
                            let err_json = serde_json::json!({
                                "text": format!("Could not read the attached document: {error}")
                            });
                            send_json_response(socket, "400 Bad Request", &err_json.to_string())
                                .await;
                            return;
                        }
                    };

                let is_chat = if let Some(clean_message) =
                    chat_req.message.strip_prefix("/chat ").map(str::to_owned)
                {
                    chat_req.message = clean_message;
                    if chat_req.system_instruction.trim().is_empty() {
                        chat_req.system_instruction = default_chat_system_instruction();
                    }
                    true
                } else {
                    false
                };

                let headers = "HTTP/1.1 200 OK\r\n\
                               Access-Control-Allow-Origin: *\r\n\
                               Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
                               Content-Type: application/x-ndjson\r\n\
                               Cache-Control: no-cache\r\n\
                               Connection: close\r\n\r\n";
                if socket.write_all(headers.as_bytes()).await.is_ok() {
                    let _ = socket.flush().await;

                    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                    {
                        let tx_progress = tx.clone();
                        let progress_cb = move |progress: AgentProgress| {
                            if let Ok(json_val) = serde_json::to_string(&serde_json::json!({
                                "type": "progress",
                                "progress": progress
                            })) {
                                let _ = tx_progress.send(format!("{}\n", json_val));
                            }
                        };

                        let tx_chunk = tx.clone();
                        let on_chunk = move |chunk: String| {
                            if let Ok(json_val) = serde_json::to_string(&serde_json::json!({
                                "type": "chunk",
                                "chunk": chunk
                            })) {
                                let _ = tx_chunk.send(format!("{}\n", json_val));
                            }
                        };

                        if is_chat {
                            let tx_chunk_inner = tx.clone();
                            let config_clone = config.clone();
                            let chat_req_clone = chat_req.clone();
                            let tx_done = tx.clone();
                            let tx_started = tx.clone();
                            let chat_id_str = chat_req.chat_id.clone().unwrap_or_default();
                            let auth_label_clone = auth_label.clone();
                            let join_handle = tokio::spawn(async move {
                                let expected_chat_id = chat_req_clone
                                    .chat_id
                                    .clone()
                                    .unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
                                let result = crate::with_turn_start_listener(expected_chat_id, move |id| {
                                    if let Some(receipt)=&receipt && let Err(error)=receipt.bind(id){eprintln!("Library reference: {error}")}
                                    let event = serde_json::json!({ "type": "started", "interactionId": id });
                                    let _ = tx_started.send(format!("{}\n", event));
                                }, orchestrate_chat_stream_with_fallback(
                                    &config_clone,
                                    &chat_req_clone,
                                    move |chunk| {
                                        if let Ok(json_val) =
                                            serde_json::to_string(&serde_json::json!({
                                                "type": "chunk",
                                                "chunk": chunk
                                            }))
                                        {
                                            let _ = tx_chunk_inner.send(format!("{}\n", json_val));
                                        }
                                    },
                                ))
                                .await;

                                match result {
                                    Ok((response, _)) => {
                                        log_api_req(
                                            "POST",
                                            "/api/chat-stream",
                                            "200 OK",
                                            Some(&format!(
                                                "Provider: {} | {}",
                                                config_clone.ai_provider, auth_label_clone
                                            )),
                                        );
                                        if let Ok(json_val) =
                                            serde_json::to_string(&serde_json::json!({
                                                "type": "done",
                                                "response": response
                                            }))
                                        {
                                            let _ = tx_done.send(format!("{}\n", json_val));
                                        }
                                    }
                                    Err(e) => {
                                        log_api_err("API /api/chat-stream error", &e);
                                        let err_json = serde_json::json!({
                                            "type": "done",
                                            "response": {
                                                "provider": "error",
                                                "model": "error",
                                                "text": format!("Error orchestrating chat: {e}")
                                            }
                                        });
                                        let _ = tx_done.send(format!("{}\n", err_json));
                                    }
                                }
                            });

                            let registered_task_id = join_handle.id();
                            let abort_handle = join_handle.abort_handle();
                            if !chat_id_str.is_empty() {
                                crate::ACTIVE_AGENTS
                                    .lock()
                                    .unwrap()
                                    .insert(chat_id_str.clone(), abort_handle);
                            }

                            let chat_id_str_cleanup = chat_id_str.clone();
                            tokio::spawn(async move {
                                let _ = join_handle.await;
                                if !chat_id_str_cleanup.is_empty() {
                                    crate::unregister_agent(
                                        &chat_id_str_cleanup,
                                        registered_task_id,
                                    );
                                }
                            });
                        } else {
                            let root = resolve_agent_root(chat_req.workspace_path.as_deref());
                            let fast_mode = config
                                .extra
                                .get("enableFastMode")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);

                            let tx_done = tx.clone();
                            let config_clone = config.clone();
                            let chat_id = chat_req.chat_id.clone();
                            let chat_id_str = chat_id.clone().unwrap_or_default();
                            // Resolve client's workspace path for parity with desktop. Idempotent on scoped_chat_id.
                            let agent_scoped_chat_id = crate::agent::memory::scoped_chat_id(
                                chat_id.as_deref().unwrap_or(DEFAULT_CONVERSATION_ID),
                                chat_req.workspace_path.as_deref(),
                            );
                            let message = chat_req.message.clone();
                            let image_data_uri = chat_req.image_data_uri.clone();
                            let audio_data_uri = chat_req.audio_data_uri.clone();
                            let video_data_uri = chat_req.video_data_uri.clone();
                            let agent_id = chat_req.agent_id.clone();
                            let plan_mode = chat_req.plan_mode;
                            let pinned_mcp_server = chat_req.pinned_mcp_server.clone();
                            let tx_approval = tx.clone();
                            let tx_started = tx.clone();

                            let join_handle = tokio::spawn(async move {
                                let approve_cb = move |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                                    crate::PENDING_APPROVALS.request_blocking(|token| {
                                        let event = serde_json::to_string(&serde_json::json!({
                                            "type": "approval-requested", "token": token, "approval": approval
                                        })).map_err(|error| error.to_string())?;
                                        tx_approval.send(format!("{event}\n")).map_err(|error| error.to_string())
                                    })
                                };
                                let result = crate::with_turn_start_listener(agent_scoped_chat_id.clone(), move |id| {
                                    if let Some(receipt)=&receipt && let Err(error)=receipt.bind(id){eprintln!("Library reference: {error}")}
                                    let event = serde_json::json!({ "type": "started", "interactionId": id });
                                    let _ = tx_started.send(format!("{}\n", event));
                                }, orchestrate_agent_loop(
                                    &config_clone,
                                    &message,
                                    &root,
                                    image_data_uri,
                                    audio_data_uri,
                                    video_data_uri,
                                    Some(agent_scoped_chat_id.as_str()),
                                    agent_id.as_deref(),
                                    None,
                                    pinned_mcp_server.as_deref(),
                                    fast_mode,
                                    plan_mode,
                                    approve_cb,
                                    progress_cb,
                                    on_chunk,
                                ))
                                .await;

                                match result {
                                    Ok(res) => {
                                        let response = ChatResponse {
                                            provider: res.provider,
                                            model: res.model,
                                            text: res.summary,
                                            fallback_provider: res.fallback,
                                            fallback_reason: res.fallback_reason,
                                            tool_calls: None,
                                            stop_reason: None,
                                            total_tokens: None,
                                            input_tokens: None,
                                            output_tokens: None,
                                            thought: None,
                                        };
                                        if let Ok(json_val) =
                                            serde_json::to_string(&serde_json::json!({
                                                "type": "done",
                                                "response": response
                                            }))
                                        {
                                            let _ = tx_done.send(format!("{}\n", json_val));
                                        }
                                    }
                                    Err(e) => {
                                        let err_json = serde_json::json!({
                                            "type": "done",
                                            "response": {
                                                "provider": "error",
                                                "model": "error",
                                                "text": format!("Error orchestrating agent: {e}")
                                            }
                                        });
                                        let _ = tx_done.send(format!("{}\n", err_json));
                                    }
                                }
                            });

                            let registered_task_id = join_handle.id();
                            let abort_handle = join_handle.abort_handle();
                            if !chat_id_str.is_empty() {
                                crate::ACTIVE_AGENTS
                                    .lock()
                                    .unwrap()
                                    .insert(chat_id_str.clone(), abort_handle);
                            }

                            let chat_id_str_cleanup = chat_id_str.clone();
                            tokio::spawn(async move {
                                let _ = join_handle.await;
                                if !chat_id_str_cleanup.is_empty() {
                                    crate::unregister_agent(
                                        &chat_id_str_cleanup,
                                        registered_task_id,
                                    );
                                }
                            });
                        }
                    }

                    drop(tx);

                    while let Some(line) = rx.recv().await {
                        if socket.write_all(line.as_bytes()).await.is_err() {
                            break;
                        }
                        let _ = socket.flush().await;
                    }
                }
                return;
            }
            send_json_response(
                socket,
                "400 Bad Request",
                "{\"status\":\"invalid chat request body\"}",
            )
            .await;
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::chat::execute: {method} {route}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_chat_request_preserves_plan_mode_and_conversation_context() {
        let request: ApiChatRequest = serde_json::from_value(json!({
            "message": "inspect", "systemInstruction": "read only", "chatId": "chat-1",
            "workspacePath": "/project", "agentId": "agent-1", "pinnedMcpServer": "mcp-1",
            "imageDataUri": "image", "audioDataUri": "audio", "videoDataUri": "video",
            "planMode": true, "temperature": 0.4, "thinkingEnabled": true, "thinkingEffort": "high"
        }))
        .unwrap();
        let config = MintConfig {
            temperature: Some(0.8),
            ..Default::default()
        };
        let request = request.into_chat_request(&config);
        assert!(request.plan_mode);
        assert_eq!(request.message, "inspect");
        assert_eq!(request.system_instruction, "read only");
        assert_eq!(request.chat_id.as_deref(), Some("chat-1"));
        assert_eq!(request.workspace_path.as_deref(), Some("/project"));
        assert_eq!(request.agent_id.as_deref(), Some("agent-1"));
        assert_eq!(request.pinned_mcp_server.as_deref(), Some("mcp-1"));
        assert_eq!(request.image_data_uri.as_deref(), Some("image"));
        assert_eq!(request.audio_data_uri.as_deref(), Some("audio"));
        assert_eq!(request.video_data_uri.as_deref(), Some("video"));
        assert_eq!(request.temperature, Some(0.4));
        assert_eq!(request.thinking_enabled, Some(true));
        assert_eq!(request.thinking_effort.as_deref(), Some("high"));
    }

    #[test]
    fn api_chat_request_defaults_to_agent_mode_and_config_temperature() {
        for body in [
            r#"{"message":"hello"}"#,
            r#"{"message":"hello","planMode":false,"systemInstruction":null}"#,
        ] {
            let request: ApiChatRequest = serde_json::from_str(body).unwrap();
            let request = request.into_chat_request(&MintConfig {
                temperature: Some(0.8),
                ..Default::default()
            });
            assert!(!request.plan_mode);
            assert_eq!(request.system_instruction, "");
            assert_eq!(request.temperature, Some(0.8));
            assert!(request.messages.is_none());
            assert!(request.tools.is_none());
        }
    }

    #[test]
    fn resolve_agent_root_uses_workspace_path_when_valid_and_falls_back_to_current_dir() {
        let current = std::env::current_dir().unwrap_or_default();
        let canonical_current = current.canonicalize().unwrap_or_else(|_| current.clone());

        // None falls back to current_dir
        assert_eq!(resolve_agent_root(None), canonical_current);

        // Empty string falls back to current_dir
        assert_eq!(resolve_agent_root(Some("")), canonical_current);
        assert_eq!(resolve_agent_root(Some("   ")), canonical_current);

        // Non-existent directory falls back to current_dir
        assert_eq!(
            resolve_agent_root(Some("/path/that/does/not/exist_12345")),
            canonical_current
        );

        // Valid directory returns its canonical path
        let temp = std::env::temp_dir();
        let canonical_temp = temp.canonicalize().unwrap_or_else(|_| temp.clone());
        assert_eq!(
            resolve_agent_root(Some(temp.to_str().unwrap())),
            canonical_temp
        );
    }
}

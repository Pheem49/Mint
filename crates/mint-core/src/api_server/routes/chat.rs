use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, mut socket: TcpStream) {
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
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ApiChatRequest {
                message: String,
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
                pinned_mcp_server: Option<String>,
            }

            if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                let config = load_config().unwrap_or_default();
                let chat_req = ChatRequest {
                    message: req.message,
                    system_instruction: req.system_instruction.unwrap_or_default(),
                    chat_id: req.chat_id,
                    image_data_uri: req.image_data_uri,
                    audio_data_uri: req.audio_data_uri,
                    video_data_uri: req.video_data_uri,
                    document_attachment: req.document_attachment,
                    workspace_path: req.workspace_path,
                    agent_id: req.agent_id,
                    plan_mode: false,
                    pinned_mcp_server: req.pinned_mcp_server,
                    messages: None,
                    tools: None,
                };
                let mut chat_req = match chat_req.with_document_context(&config) {
                    Ok(req) => req,
                    Err(error) => {
                        log_api_err("API /api/chat document error", &error);
                        let err_json = serde_json::json!({
                            "text": format!("Could not read the attached document: {error}")
                        });
                        send_json_response(socket, "400 Bad Request", &err_json.to_string()).await;
                        return;
                    }
                };
                let sent_image = chat_req.image_data_uri.clone();
                let sent_video = chat_req.video_data_uri.clone();
                let sent_message = chat_req.message.clone();

                let response = if let Some(clean_message) =
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
                };

                match response {
                    Ok(resp) => {
                        log_api_req(
                            "POST",
                            "/api/chat",
                            "200 OK",
                            Some(&format!("Model: {} | {}", config.ai_provider, auth_label)),
                        );
                        if let Some(image) = sent_image {
                            let _ = save_chat_images(
                                image
                                    .split_whitespace()
                                    .map(str::to_owned)
                                    .collect::<Vec<_>>(),
                                Some("web".into()),
                                Some(sent_message.clone()),
                            );
                        }
                        if let Some(video) = sent_video {
                            let _ = save_chat_images(
                                video
                                    .split_whitespace()
                                    .map(str::to_owned)
                                    .collect::<Vec<_>>(),
                                Some("web".into()),
                                Some(sent_message),
                            );
                        }
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
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ApiChatRequest {
                message: String,
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
                pinned_mcp_server: Option<String>,
            }

            if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                let config = load_config().unwrap_or_default();
                let chat_req = ChatRequest {
                    message: req.message,
                    system_instruction: req.system_instruction.unwrap_or_default(),
                    chat_id: req.chat_id,
                    image_data_uri: req.image_data_uri,
                    audio_data_uri: req.audio_data_uri,
                    video_data_uri: req.video_data_uri,
                    document_attachment: req.document_attachment,
                    workspace_path: req.workspace_path,
                    agent_id: req.agent_id,
                    plan_mode: false,
                    pinned_mcp_server: req.pinned_mcp_server,
                    messages: None,
                    tools: None,
                };
                let mut chat_req = match chat_req.with_document_context(&config) {
                    Ok(req) => req,
                    Err(error) => {
                        log_api_err("API /api/chat-stream document error", &error);
                        let err_json = serde_json::json!({
                            "text": format!("Could not read the attached document: {error}")
                        });
                        send_json_response(socket, "400 Bad Request", &err_json.to_string()).await;
                        return;
                    }
                };
                let sent_image = chat_req.image_data_uri.clone();
                let sent_video = chat_req.video_data_uri.clone();
                let sent_message = chat_req.message.clone();

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
                            let chat_id_str = chat_req.chat_id.clone().unwrap_or_default();
                            let auth_label_clone = auth_label.clone();
                            let join_handle = tokio::spawn(async move {
                                let result = orchestrate_chat_stream_with_fallback(
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
                                )
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
                                    crate::ACTIVE_AGENTS
                                        .lock()
                                        .unwrap()
                                        .remove(&chat_id_str_cleanup);
                                }
                            });
                        } else {
                            let root = std::env::current_dir().unwrap_or_default();
                            let fast_mode = config
                                .extra
                                .get("enableFastMode")
                                .and_then(Value::as_bool)
                                .unwrap_or(false);

                            let tx_done = tx.clone();
                            let config_clone = config.clone();
                            let chat_id = chat_req.chat_id.clone();
                            let chat_id_str = chat_id.clone().unwrap_or_default();
                            // Unlike `root` above (this route's agent-mode tools
                            // deliberately still operate against the API server
                            // process's own cwd, not any client-sent workspace —
                            // out of scope for the chat_id-scoping fix), the
                            // conversation identity itself DOES need the client's
                            // workspace: `orchestrate_agent_loop` would otherwise
                            // self-derive from `root` (constant across every web
                            // request), which can't distinguish workspaces at
                            // all. Pre-scoping here is safe/idempotent — see
                            // `scoped_chat_id`'s docs — so its self-derivation
                            // becomes a no-op on the id we hand it below. The raw,
                            // unscoped `chat_id_str` above is left untouched: it's
                            // only a cancellation-token key, and `/api/cancel-chat`
                            // sends back the same raw id it started with.
                            let agent_scoped_chat_id = crate::agent::memory::scoped_chat_id(
                                chat_id.as_deref().unwrap_or(DEFAULT_CONVERSATION_ID),
                                chat_req.workspace_path.as_deref(),
                            );
                            let message = chat_req.message.clone();
                            let image_data_uri = chat_req.image_data_uri.clone();
                            let audio_data_uri = chat_req.audio_data_uri.clone();
                            let video_data_uri = chat_req.video_data_uri.clone();
                            let agent_id = chat_req.agent_id.clone();
                            let pinned_mcp_server = chat_req.pinned_mcp_server.clone();
                            let tx_approval = tx.clone();

                            let join_handle = tokio::spawn(async move {
                                // Real (not auto-deny) approval flow: the request-payload
                                // shape and blocking mechanism mirror desktop's approve_cb
                                // (src-tauri/src/lib.rs) exactly, but the "requested" event
                                // rides this same ndjson stream instead of a Tauri event,
                                // and the token is a UUID (not a sequential counter) since
                                // this endpoint, unlike Tauri IPC, is LAN-reachable by
                                // default — see mint_core::PENDING_APPROVALS' docs.
                                let approve_cb = move |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                                    let (approval_tx, approval_rx) = tokio::sync::oneshot::channel();
                                    let token = uuid::Uuid::new_v4().to_string();
                                    crate::PENDING_APPROVALS
                                        .lock()
                                        .unwrap()
                                        .insert(token.clone(), approval_tx);
                                    if let Ok(json_val) = serde_json::to_string(&serde_json::json!({
                                        "type": "approval-requested",
                                        "token": token,
                                        "approval": approval
                                    })) {
                                        let _ = tx_approval.send(format!("{}\n", json_val));
                                    }
                                    Ok(tokio::task::block_in_place(move || {
                                        tokio::runtime::Handle::current().block_on(approval_rx)
                                    })
                                    .unwrap_or(ApprovalOutcome::Denied))
                                };
                                let result = orchestrate_agent_loop(
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
                                    false,
                                    approve_cb,
                                    progress_cb,
                                    on_chunk,
                                )
                                .await;

                                match result {
                                    Ok(res) => {
                                        let response = ChatResponse {
                                            provider: res.provider,
                                            model: res.model,
                                            text: res.summary,
                                            fallback_provider: res.fallback,
                                            tool_calls: None,
                                            stop_reason: None,
                                            total_tokens: None,
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
                                    crate::ACTIVE_AGENTS
                                        .lock()
                                        .unwrap()
                                        .remove(&chat_id_str_cleanup);
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

                    if let Some(image) = sent_image {
                        let _ = save_chat_images(
                            image
                                .split_whitespace()
                                .map(str::to_owned)
                                .collect::<Vec<_>>(),
                            Some("web".into()),
                            Some(sent_message.clone()),
                        );
                    }
                    if let Some(video) = sent_video {
                        let _ = save_chat_images(
                            video
                                .split_whitespace()
                                .map(str::to_owned)
                                .collect::<Vec<_>>(),
                            Some("web".into()),
                            Some(sent_message),
                        );
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

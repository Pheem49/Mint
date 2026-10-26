use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
        ("GET", "/api/conversation-snapshot") => {
            let chat_id =
                query_param(query, "chatId").unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
            let before_id = query_param(query, "beforeId").and_then(|s| s.parse::<i64>().ok());
            let limit = query_param(query, "limit")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(50);
            let snapshot = tokio::task::spawn_blocking(move || {
                MemoryStore::open_default()
                    .and_then(|m| m.conversation_snapshot(&chat_id, before_id, limit))
            })
            .await;
            match snapshot {
                Ok(Ok(snapshot)) => {
                    send_json_response(
                        socket,
                        "200 OK",
                        &serde_json::to_string(&snapshot).unwrap_or_default(),
                    )
                    .await
                }
                _ => send_json_response(socket, "500 Internal Server Error", "{}").await,
            }
        }
        ("GET", "/api/conversation-changes") => {
            let chat_id =
                query_param(query, "chatId").unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
            let after = query_param(query, "after")
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            let limit = query_param(query, "limit")
                .and_then(|s| s.parse::<usize>().ok())
                .unwrap_or(100);
            let changes = tokio::task::spawn_blocking(move || {
                MemoryStore::open_default()
                    .and_then(|m| m.conversation_changes(&chat_id, after, limit))
            })
            .await;
            match changes {
                Ok(Ok(changes)) => {
                    send_json_response(
                        socket,
                        "200 OK",
                        &serde_json::to_string(&changes).unwrap_or_default(),
                    )
                    .await
                }
                _ => send_json_response(socket, "500 Internal Server Error", "{}").await,
            }
        }
        ("GET", "/api/interactions") => {
            let limit = query_param(query, "limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(50)
                .min(200);
            let chat_id = query_param(query, "chatId")
                .unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
            let workspace_path = query_param(query, "workspacePath");
            let list = tokio::task::spawn_blocking(move || {
                let scoped_chat_id =
                    crate::agent::memory::scoped_chat_id(&chat_id, workspace_path.as_deref());
                MemoryStore::open_default().map(|memory| {
                    memory
                        .recent_interactions_for_chat(&scoped_chat_id, limit)
                        .unwrap_or_default()
                })
            })
            .await;
            if let Ok(Ok(list)) = list && let Ok(json_str) = serde_json::to_string(&list) {
                send_json_response(socket, "200 OK", &json_str).await;
            } else {
                send_json_response(socket, "500 Internal Server Error", "[]").await;
            }
        }

        ("GET", "/api/chat-sessions") => {
            let list = tokio::task::spawn_blocking(|| {
                MemoryStore::open_default().map(|m| m.list_chat_sessions().unwrap_or_default())
            })
            .await;
            if let Ok(Ok(list)) = list && let Ok(json_str) = serde_json::to_string(&list) {
                send_json_response(socket, "200 OK", &json_str).await;
            } else {
                send_json_response(socket, "500 Internal Server Error", "[]").await;
            }
        }

        ("POST", "/api/chat-sessions/workspace") => {
            let value = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
            let chat_id = value
                .get("chatId")
                .and_then(Value::as_str)
                .map(str::trim)
                .filter(|id| !id.is_empty())
                .map(ToOwned::to_owned);
            let workspace = match value.get("workspacePath") {
                Some(Value::Null) => Some(None),
                Some(Value::String(path)) => Some(if path.trim().is_empty() {
                    None
                } else {
                    Some(path.trim().to_owned())
                }),
                _ => None,
            };
            let (Some(chat_id), Some(workspace)) = (chat_id, workspace) else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"chatId and a string or null workspacePath are required\"}",
                )
                .await;
                return;
            };
            let result = tokio::task::spawn_blocking(move || {
                MemoryStore::open_default()
                    .and_then(|memory| memory.set_chat_session_workspace(&chat_id, workspace.as_deref()))
            })
            .await;
            match result {
                Ok(Ok(())) => send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await,
                Ok(Err(error)) => {
                    send_json_response(
                        socket,
                        "500 Internal Server Error",
                        &json!({"error": error.to_string()}).to_string(),
                    )
                    .await
                }
                Err(_) => send_json_response(socket, "500 Internal Server Error", "{}").await,
            }
        }

        ("POST", "/api/chat-sessions/delete") => {
            let chat_id = query_param(query, "chatId").unwrap_or_default();
            crate::cancel_agent(&chat_id);
            let deleted = tokio::task::spawn_blocking(move || {
                MemoryStore::open_default().map(|memory| memory.delete_chat_session(&chat_id).unwrap_or(0))
            })
            .await;
            if let Ok(Ok(deleted)) = deleted {
                let response = serde_json::json!({ "status": "ok", "deleted": deleted });
                send_json_response(socket, "200 OK", &response.to_string()).await;
            } else {
                send_json_response(
                    socket,
                    "500 Internal Server Error",
                    "{\"status\":\"error\",\"deleted\":0}",
                )
                .await;
            }
        }

        ("POST", "/api/chat-sessions/rename") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct RenameRequest {
                chat_id: String,
                new_title: String,
            }

            if let Ok(req) = serde_json::from_str::<RenameRequest>(body) {
                let updated = tokio::task::spawn_blocking(move || {
                    MemoryStore::open_default()
                        .map(|memory| memory.rename_chat_session(&req.chat_id, &req.new_title).unwrap_or(0))
                })
                .await;
                if let Ok(Ok(updated)) = updated {
                    let response = serde_json::json!({ "status": "ok", "updated": updated });
                    send_json_response(socket, "200 OK", &response.to_string()).await;
                    return;
                }
            }
            send_json_response(
                socket,
                "500 Internal Server Error",
                "{\"status\":\"error\",\"updated\":0}",
            )
            .await;
        }

        ("POST", "/api/interactions/clear") => {
            let chat_id = query_param(query, "chatId")
                .unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
            let result = tokio::task::spawn_blocking(move || {
                MemoryStore::open_default().map(|memory| {
                    let _ = memory.clear_interactions_for_chat(&chat_id);
                })
            })
            .await;
            if let Ok(Ok(())) = result {
                send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
            } else {
                send_json_response(
                    socket,
                    "500 Internal Server Error",
                    "{\"status\":\"error\"}",
                )
                .await;
            }
        }

        ("POST", "/api/interactions") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct ApiSaveInteraction {
                chat_id: String,
                user_text: String,
                ai_text: Option<String>,
                provider: String,
                model: String,
            }

            if let Ok(req) = serde_json::from_str::<ApiSaveInteraction>(body) {
                let result = tokio::task::spawn_blocking(move || {
                    MemoryStore::open_default().and_then(|memory| {
                        memory.add_interaction_for_chat(
                            &req.chat_id,
                            &req.user_text,
                            req.ai_text.as_deref().unwrap_or(""),
                            &req.provider,
                            &req.model,
                        )
                    })
                })
                .await;
                match result {
                    Ok(Ok(row_id)) => {
                        let res_json = json!({ "success": true, "id": row_id });
                        send_json_response(socket, "200 OK", &res_json.to_string()).await;
                    }
                    Ok(Err(error)) => {
                        let err_json =
                            json!({ "success": false, "message": error.to_string() });
                        send_json_response(
                            socket,
                            "500 Internal Server Error",
                            &err_json.to_string(),
                        )
                        .await;
                    }
                    Err(_) => {
                        send_json_response(
                            socket,
                            "500 Internal Server Error",
                            "{\"success\":false,\"message\":\"db error\"}",
                        )
                        .await;
                    }
                }
            } else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"success\":false,\"message\":\"invalid body\"}",
                )
                .await;
            }
        }

        ("POST", "/api/interactions/agent-activity") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct SaveAgentActivityRequest {
                interaction_id: i64,
                activity: Vec<AgentProgress>,
            }

            if let Ok(payload) = serde_json::from_str::<SaveAgentActivityRequest>(body)
                && let Ok(activity_json) = serde_json::to_string(&payload.activity)
            {
                let result = tokio::task::spawn_blocking(move || {
                    MemoryStore::open_default().and_then(|memory| {
                        memory.set_interaction_agent_activity_json(payload.interaction_id, &activity_json)
                    })
                })
                .await;
                if let Ok(Ok(_)) = result {
                    send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                    return;
                }
            }
            send_json_response(
                socket,
                "400 Bad Request",
                "{\"status\":\"invalid agent activity payload\"}",
            )
            .await;
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::sessions::execute: {method} {route}"
        ),
    }
}

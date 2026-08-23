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
        ("GET", "/api/interactions") => {
            let limit = query_param(query, "limit")
                .and_then(|value| value.parse::<usize>().ok())
                .unwrap_or(50)
                .min(200);
            if let Ok(memory) = MemoryStore::open_default() {
                let chat_id = query_param(query, "chatId")
                    .unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
                let list = memory
                    .recent_interactions_for_chat(&chat_id, limit)
                    .unwrap_or_default();
                if let Ok(json_str) = serde_json::to_string(&list) {
                    send_json_response(socket, "200 OK", &json_str).await;
                    return;
                }
            }
            send_json_response(socket, "500 Internal Server Error", "[]").await;
        }

        ("GET", "/api/chat-sessions") => {
            if let Ok(memory) = MemoryStore::open_default() {
                let list = memory.list_chat_sessions().unwrap_or_default();
                if let Ok(json_str) = serde_json::to_string(&list) {
                    send_json_response(socket, "200 OK", &json_str).await;
                    return;
                }
            }
            send_json_response(socket, "500 Internal Server Error", "[]").await;
        }

        ("POST", "/api/chat-sessions/delete") => {
            let chat_id = query_param(query, "chatId").unwrap_or_default();
            if let Ok(memory) = MemoryStore::open_default() {
                let deleted = memory.delete_chat_session(&chat_id).unwrap_or(0);
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

            if let Ok(req) = serde_json::from_str::<RenameRequest>(body)
                && let Ok(memory) = MemoryStore::open_default()
            {
                let updated = memory
                    .rename_chat_session(&req.chat_id, &req.new_title)
                    .unwrap_or(0);
                let response = serde_json::json!({ "status": "ok", "updated": updated });
                send_json_response(socket, "200 OK", &response.to_string()).await;
                return;
            }
            send_json_response(
                socket,
                "500 Internal Server Error",
                "{\"status\":\"error\",\"updated\":0}",
            )
            .await;
        }

        ("POST", "/api/interactions/clear") => {
            if let Ok(memory) = MemoryStore::open_default() {
                let chat_id = query_param(query, "chatId")
                    .unwrap_or_else(|| DEFAULT_CONVERSATION_ID.to_owned());
                let _ = memory.clear_interactions_for_chat(&chat_id);
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
                if let Ok(memory) = MemoryStore::open_default() {
                    match memory.add_interaction_for_chat(
                        &req.chat_id,
                        &req.user_text,
                        req.ai_text.as_deref().unwrap_or(""),
                        &req.provider,
                        &req.model,
                    ) {
                        Ok(row_id) => {
                            let res_json = json!({ "success": true, "id": row_id });
                            send_json_response(socket, "200 OK", &res_json.to_string()).await;
                        }
                        Err(error) => {
                            let err_json =
                                json!({ "success": false, "message": error.to_string() });
                            send_json_response(
                                socket,
                                "500 Internal Server Error",
                                &err_json.to_string(),
                            )
                            .await;
                        }
                    }
                } else {
                    send_json_response(
                        socket,
                        "500 Internal Server Error",
                        "{\"success\":false,\"message\":\"db error\"}",
                    )
                    .await;
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
                && let Ok(memory) = MemoryStore::open_default()
                && let Ok(activity_json) = serde_json::to_string(&payload.activity)
                && memory
                    .set_interaction_agent_activity_json(payload.interaction_id, &activity_json)
                    .is_ok()
            {
                send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                return;
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

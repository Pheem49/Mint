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
        ("GET", "/api/learned-skills") => {
            let workspace = query_param(query, "workspacePath")
                .filter(|path| !path.trim().is_empty())
                .map(std::path::PathBuf::from)
                .or_else(|| std::env::current_dir().ok());
            let list = tokio::task::spawn_blocking(move || {
                MemoryStore::open_default()
                    .ok()
                    .and_then(|memory| {
                        crate::skill_inventory::list_available_skills(&memory, workspace.as_deref())
                            .ok()
                    })
                    .unwrap_or_default()
            })
            .await
            .unwrap_or_default();
            send_json_response(
                socket,
                "200 OK",
                &serde_json::to_string(&list).unwrap_or_default(),
            )
            .await;
            return;
        }

        ("GET", "/api/subagents") => {
            let root = std::env::current_dir().ok();
            let list = crate::subagents::list_subagents(root.as_deref());
            send_json_response(
                socket,
                "200 OK",
                &serde_json::to_string(&list).unwrap_or_default(),
            )
            .await;
        }

        ("POST", "/api/subagents") => {
            match serde_json::from_str::<crate::subagents::SubagentDraft>(body) {
                Ok(draft) => {
                    let root = std::env::current_dir().ok();
                    match crate::subagents::save_subagent(&draft, root.as_deref()) {
                        Ok(saved) => {
                            send_json_response(
                                socket,
                                "200 OK",
                                &serde_json::to_string(&saved).unwrap_or_default(),
                            )
                            .await;
                        }
                        Err(err) => {
                            let err_msg = json!({ "error": err }).to_string();
                            send_json_response(socket, "400 Bad Request", &err_msg).await;
                        }
                    }
                }
                Err(_) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        "{\"error\":\"Invalid request body.\"}",
                    )
                    .await;
                }
            }
        }

        ("DELETE", route) if route.starts_with("/api/subagents/") => {
            let source_path = percent_decode(route.trim_start_matches("/api/subagents/"));
            match crate::subagents::delete_subagent(&source_path) {
                Ok(()) => {
                    send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                }
                Err(err) => {
                    let err_msg = json!({ "error": err }).to_string();
                    send_json_response(socket, "400 Bad Request", &err_msg).await;
                }
            }
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::skills_subagents::execute: {method} {route}"
        ),
    }
}

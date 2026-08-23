use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query: _query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
        ("GET", "/api/linked-folders") => {
            let folders = load_config()
                .ok()
                .and_then(|config| crate::linked_folders::configured_linked_folders(&config).ok())
                .unwrap_or_default();
            send_json_response(
                socket,
                "200 OK",
                &serde_json::to_string(&folders).unwrap_or_default(),
            )
            .await;
        }

        ("POST", "/api/linked-folders") => {
            match serde_json::from_str::<crate::LinkedFolderDraft>(body) {
                Ok(draft) => {
                    match crate::add_linked_folder(&draft.name, &draft.path, draft.description) {
                        Ok(()) => {
                            send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                        }
                        Err(err) => {
                            let err_msg = json!({ "error": err.to_string() }).to_string();
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

        ("DELETE", route) if route.starts_with("/api/linked-folders/") => {
            let name = percent_decode(route.trim_start_matches("/api/linked-folders/"));
            match crate::remove_linked_folder(&name) {
                Ok(removed) => {
                    send_json_response(
                        socket,
                        "200 OK",
                        &json!({ "status": "ok", "removed": removed }).to_string(),
                    )
                    .await;
                }
                Err(err) => {
                    let err_msg = json!({ "error": err.to_string() }).to_string();
                    send_json_response(socket, "400 Bad Request", &err_msg).await;
                }
            }
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::linked_folders::execute: {method} {route}"
        ),
    }
}

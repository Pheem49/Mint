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
        ("GET", route)
            if route.starts_with("/api/linked-folders/") && route.ends_with("/status") =>
        {
            let name = percent_decode(
                route
                    .trim_start_matches("/api/linked-folders/")
                    .trim_end_matches("/status"),
            );
            match crate::linked_folder_status(&name) {
                Ok(status) => {
                    send_json_response(socket, "200 OK", &json!(status).to_string()).await
                }
                Err(err) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error":err.to_string()}).to_string(),
                    )
                    .await
                }
            }
        }
        ("POST", route)
            if route.starts_with("/api/linked-folders/") && route.ends_with("/refresh") =>
        {
            let name = percent_decode(
                route
                    .trim_start_matches("/api/linked-folders/")
                    .trim_end_matches("/refresh"),
            );
            match crate::refresh_linked_folder(&name) {
                Ok(status) => {
                    send_json_response(socket, "200 OK", &json!(status).to_string()).await
                }
                Err(err) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error":err.to_string()}).to_string(),
                    )
                    .await
                }
            }
        }
        ("GET", route)
            if route.starts_with("/api/linked-folders/") && route.ends_with("/notes") =>
        {
            let name = percent_decode(
                route
                    .trim_start_matches("/api/linked-folders/")
                    .trim_end_matches("/notes"),
            );
            match crate::list_linked_folder_notes(&name) {
                Ok(notes) => send_json_response(socket, "200 OK", &json!(notes).to_string()).await,
                Err(err) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error":err.to_string()}).to_string(),
                    )
                    .await
                }
            }
        }
        ("POST", route)
            if route.starts_with("/api/linked-folders/") && route.ends_with("/notes") =>
        {
            let name = percent_decode(
                route
                    .trim_start_matches("/api/linked-folders/")
                    .trim_end_matches("/notes"),
            );
            let content = serde_json::from_str::<serde_json::Value>(body)
                .ok()
                .and_then(|v| v.get("content").and_then(|c| c.as_str()).map(str::to_owned))
                .unwrap_or_default();
            match crate::save_linked_folder_note(&name, &content) {
                Ok(note) => send_json_response(socket, "200 OK", &json!(note).to_string()).await,
                Err(err) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error":err.to_string()}).to_string(),
                    )
                    .await
                }
            }
        }
        ("GET", route)
            if route.starts_with("/api/linked-folders/") && route.contains("/notes/") =>
        {
            let remainder = route.trim_start_matches("/api/linked-folders/");
            let Some((name, id)) = remainder.split_once("/notes/") else {
                unreachable!()
            };
            match crate::read_linked_folder_note(&percent_decode(name), &percent_decode(id)) {
                Ok(content) => {
                    send_json_response(socket, "200 OK", &json!({"content":content}).to_string())
                        .await
                }
                Err(err) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error":err.to_string()}).to_string(),
                    )
                    .await
                }
            }
        }
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

        _ => {
            send_json_response(
                socket,
                "404 Not Found",
                "{\"error\":\"unknown linked-folder route\"}",
            )
            .await
        }
    }
}

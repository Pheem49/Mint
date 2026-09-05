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
        request_bytes,
        header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
        ("POST", "/api/active-model") => {
            #[derive(Deserialize)]
            struct ActiveModelReq {
                provider: String,
                model: Option<String>,
            }
            if let Ok(req) = serde_json::from_str::<ActiveModelReq>(body)
                && let Ok(mut config) = load_config()
            {
                if let Ok(display_name) =
                    config.set_active_model(&req.provider, req.model.as_deref())
                {
                    send_json_response(
                        socket,
                        "200 OK",
                        &serde_json::json!({ "status": "ok", "displayName": display_name })
                            .to_string(),
                    )
                    .await;
                    return;
                }
            }
            send_json_response(
                socket,
                "500 Internal Server Error",
                "{\"status\":\"error\"}",
            )
            .await;
        }

        ("GET", "/api/pictures") => match list_saved_pictures() {
            Ok(mut pictures) => {
                for picture in &mut pictures {
                    picture.url = Some(format!("/api/pictures/{}", picture.filename));
                }
                if let Ok(json_str) = serde_json::to_string(&pictures) {
                    send_json_response(socket, "200 OK", &json_str).await;
                } else {
                    send_json_response(socket, "500 Internal Server Error", "[]").await;
                }
            }
            Err(_) => send_json_response(socket, "500 Internal Server Error", "[]").await,
        },

        ("GET", route) if route.starts_with("/api/pictures/") => {
            let filename = percent_decode(route.trim_start_matches("/api/pictures/"));
            match picture_bytes(&filename) {
                Ok((mime_type, bytes)) => {
                    send_binary_response(socket, "200 OK", &mime_type, &bytes).await
                }
                Err(_) => {
                    send_json_response(socket, "404 Not Found", "{\"error\":\"picture not found\"}")
                        .await
                }
            }
        }

        ("DELETE", route) if route.starts_with("/api/pictures/") => {
            let id = percent_decode(route.trim_start_matches("/api/pictures/"));
            match delete_saved_picture(&id) {
                Ok(_) => {
                    send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                }
                Err(err) => {
                    let err_msg = serde_json::json!({ "error": err.to_string() }).to_string();
                    send_json_response(socket, "400 Bad Request", &err_msg).await;
                }
            }
        }

        ("GET", route) if route.starts_with("/api/thumbnails/") => {
            let filename = percent_decode(route.trim_start_matches("/api/thumbnails/"));
            match thumbnail_bytes(&filename) {
                Ok((mime_type, bytes)) => {
                    send_binary_response(socket, "200 OK", &mime_type, &bytes).await
                }
                Err(_) => {
                    send_json_response(
                        socket,
                        "404 Not Found",
                        "{\"error\":\"thumbnail not found\"}",
                    )
                    .await
                }
            }
        }

        ("GET", "/api/config") => {
            let config = load_config().unwrap_or_default();
            if let Ok(json_str) = serde_json::to_string(&config) {
                send_json_response(socket, "200 OK", &json_str).await;
            } else {
                send_json_response(socket, "500 Internal Server Error", "{}").await;
            }
        }

        ("POST", "/api/config") => {
            if let Ok(new_config) = serde_json::from_str::<MintConfig>(body)
                && save_config(&new_config).is_ok()
            {
                send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                return;
            }
            send_json_response(
                socket,
                "400 Bad Request",
                "{\"status\":\"invalid config json\"}",
            )
            .await;
        }

        ("GET", "/api/weather") => {
            let city = query_param(query, "city").unwrap_or_default();
            match weather(&city).await {
                Ok(report) => {
                    if let Ok(json_str) = serde_json::to_string(&report) {
                        send_json_response(socket, "200 OK", &json_str).await;
                    } else {
                        send_json_response(socket, "500 Internal Server Error", "{}").await;
                    }
                }
                Err(error) => {
                    let err_json = json!({ "error": error.to_string() });
                    send_json_response(socket, "500 Internal Server Error", &err_json.to_string())
                        .await;
                }
            }
        }

        ("POST", "/api/action") => {
            if let Ok(action) = serde_json::from_str::<ApiAction>(body) {
                let config = load_config().unwrap_or_default();
                match execute_api_action(&config, action) {
                    Ok(value) => send_json_response(socket, "200 OK", &value.to_string()).await,
                    Err(error) => {
                        let err_json = json!({ "success": false, "message": error });
                        send_json_response(socket, "400 Bad Request", &err_json.to_string()).await;
                    }
                }
            } else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"success\":false,\"message\":\"invalid action body\"}",
                )
                .await;
            }
        }

        ("POST", "/api/uploads") => {
            // Accept raw body bytes and optional filename query param.
            // Body is the raw file bytes (client should POST the file as the request body).
            let filename_param = query_param(query, "filename");
            let header_end_idx = header_end as usize;
            // extract raw bytes for body
            let body_bytes = &request_bytes[header_end_idx + 4..];

            if body_bytes.is_empty() {
                send_json_response(socket, "400 Bad Request", "{\"error\":\"empty body\"}").await;
                return;
            }

            // Determine mime type from filename or fallback to video/mp4
            let (mime_type, _extension) = filename_param
                .as_deref()
                .and_then(|f| f.rsplit_once('.'))
                .map(|(_, ext)| {
                    let e = ext.to_ascii_lowercase();
                    match e.as_str() {
                        "mp4" => ("video/mp4", "mp4"),
                        "webm" => ("video/webm", "webm"),
                        "mov" => ("video/quicktime", "mov"),
                        "mkv" => ("video/x-matroska", "mkv"),
                        "avi" => ("video/x-msvideo", "avi"),
                        _ => ("application/octet-stream", "bin"),
                    }
                })
                .unwrap_or(("video/mp4", "mp4"));

            // Build data URI and reuse save_chat_images helper to persist and index file
            let encoded = BASE64.encode(body_bytes);
            let data_uri = format!("data:{};base64,{}", mime_type, encoded);
            match save_chat_images(
                vec![data_uri],
                Some("upload".into()),
                Some("uploaded".into()),
            ) {
                Ok(saved) => {
                    if let Some(entry) = saved.into_iter().next() {
                        let res_json = json!({ "url": format!("/api/pictures/{}", entry.filename), "filename": entry.filename });
                        send_json_response(socket, "200 OK", &res_json.to_string()).await;
                        return;
                    }
                }
                Err(err) => {
                    let err_json = json!({ "error": err.to_string() });
                    send_json_response(socket, "500 Internal Server Error", &err_json.to_string())
                        .await;
                    return;
                }
            }
            send_json_response(
                socket,
                "500 Internal Server Error",
                "{\"error\":\"failed to save upload\"}",
            )
            .await;
        }

        ("GET", "/api/checkpoints") => {
            let chat_id = query_param(query, "chatId").unwrap_or_default();
            let checkpoints = crate::git::list_checkpoints(&chat_id);
            if let Ok(json_str) = serde_json::to_string(&checkpoints) {
                send_json_response(socket, "200 OK", &json_str).await;
            } else {
                send_json_response(socket, "500 Internal Server Error", "[]").await;
            }
        }

        ("POST", "/api/checkpoints/rollback") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct RollbackReq {
                chat_id: String,
                step: usize,
                workspace_path: Option<String>,
            }
            if let Ok(req) = serde_json::from_str::<RollbackReq>(body) {
                let root = req
                    .workspace_path
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                match crate::git::rollback_to_step(&root, &req.chat_id, req.step) {
                    Ok(msg) => {
                        let res = json!({ "status": "ok", "message": msg });
                        send_json_response(socket, "200 OK", &res.to_string()).await;
                        return;
                    }
                    Err(err) => {
                        let res = json!({ "status": "error", "message": err });
                        send_json_response(socket, "400 Bad Request", &res.to_string()).await;
                        return;
                    }
                }
            }
            send_json_response(
                socket,
                "400 Bad Request",
                "{\"status\":\"error\",\"message\":\"invalid request body\"}",
            )
            .await;
        }

        ("GET", "/api/file/read") => {
            let file_path = query_param(query, "path").unwrap_or_default();
            if file_path.is_empty() {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"missing file path query parameter\"}",
                )
                .await;
                return;
            }
            match std::fs::read_to_string(&file_path) {
                Ok(content) => {
                    let res = json!({
                        "path": file_path,
                        "content": content
                    });
                    send_json_response(socket, "200 OK", &res.to_string()).await;
                }
                Err(err) => {
                    let res = json!({
                        "error": format!("unable to read file: {err}")
                    });
                    send_json_response(socket, "404 Not Found", &res.to_string()).await;
                }
            }
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::misc::execute: {method} {route}"
        ),
    }
}

use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

struct MediaModelQuery {
    provider: String,
    api_key: String,
}

impl MediaModelQuery {
    fn parse(query: &str) -> Self {
        Self {
            provider: query_param(query, "provider").unwrap_or_default(),
            api_key: query_param(query, "apiKey").unwrap_or_default(),
        }
    }
}

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
        ("GET", "/api/workspace-history") => {
            let root = query_param(query, "root")
                .map(|value| percent_decode(&value))
                .unwrap_or_default();
            let result = (|| -> Result<_, String> {
                if root.trim().is_empty() {
                    return Err("Workspace root is required".into());
                }
                let config = load_config().map_err(|e| e.to_string())?;
                let root = crate::assert_path_capability(
                    std::path::Path::new(&root),
                    crate::Capability::Read,
                    &config,
                )
                .map_err(|e| e.to_string())?;
                crate::system::workspace_history::list(&root)
            })();
            match result {
                Ok(entries) => {
                    send_json_response(socket, "200 OK", &json!(entries).to_string()).await
                }
                Err(message) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({ "message": message }).to_string(),
                    )
                    .await
                }
            }
        }
        ("POST", "/api/workspace-history/undo") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct UndoWorkspaceRequest {
                root: String,
                revision: u64,
                expected_id: Option<String>,
            }
            let result = (|| -> Result<_, String> {
                let request: UndoWorkspaceRequest =
                    serde_json::from_str(body).map_err(|e| e.to_string())?;
                let config = load_config().map_err(|e| e.to_string())?;
                let root = crate::assert_path_capability(
                    std::path::Path::new(&request.root),
                    crate::Capability::Write,
                    &config,
                )
                .map_err(|e| e.to_string())?;
                crate::workspace::undo_last(&root, request.revision, request.expected_id.as_deref())
            })();
            match result {
                Ok(snapshot) => {
                    send_json_response(socket, "200 OK", &json!(snapshot).to_string()).await
                }
                Err(message) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({ "message": message }).to_string(),
                    )
                    .await
                }
            }
        }
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

        ("GET", route) if route == "/api/proxy-image" || route.starts_with("/api/proxy-image") => {
            let target_url = query_param(query, "url").unwrap_or_default();
            if target_url.trim().is_empty() {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"missing url query parameter\"}",
                )
                .await;
                return;
            }

            let cache_dir = crate::media::image_proxy::default_cache_dir();
            match crate::media::image_proxy::fetch_and_cache_image(&cache_dir, &target_url).await {
                Ok((mime_type, bytes)) => {
                    send_binary_response(socket, "200 OK", &mime_type, &bytes).await;
                }
                Err(err) => {
                    let status = match &err {
                        crate::media::image_proxy::ImageProxyError::InvalidUrl(_)
                        | crate::media::image_proxy::ImageProxyError::SsrfBlocked(_) => {
                            "400 Bad Request"
                        }
                        crate::media::image_proxy::ImageProxyError::BadStatus(code)
                            if *code == 404 =>
                        {
                            "404 Not Found"
                        }
                        _ => "502 Bad Gateway",
                    };
                    let err_msg = serde_json::json!({ "error": err.to_string() }).to_string();
                    send_json_response(socket, status, &err_msg).await;
                }
            }
        }

        ("GET", "/api/media") => {
            let path_param = query_param(query, "path")
                .map(|p| percent_decode(&p))
                .unwrap_or_default();
            if path_param.is_empty() {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"missing path query parameter\"}",
                )
                .await;
                return;
            }
            let target_path = std::path::Path::new(&path_param);
            let Ok(canonical) = target_path.canonicalize() else {
                send_json_response(
                    socket,
                    "404 Not Found",
                    "{\"error\":\"media file not found\"}",
                )
                .await;
                return;
            };
            if !canonical.is_file() {
                send_json_response(
                    socket,
                    "404 Not Found",
                    "{\"error\":\"media file not found\"}",
                )
                .await;
                return;
            }
            let Some(mime_type) = (match canonical
                .extension()
                .and_then(|ext| ext.to_str())
                .unwrap_or("")
                .to_lowercase()
                .as_str()
            {
                "png" => Some("image/png"),
                "jpg" | "jpeg" => Some("image/jpeg"),
                "webp" => Some("image/webp"),
                "gif" => Some("image/gif"),
                "svg" => Some("image/svg+xml"),
                "mp4" => Some("video/mp4"),
                "webm" => Some("video/webm"),
                "mov" => Some("video/quicktime"),
                "mkv" => Some("video/x-matroska"),
                "mp3" => Some("audio/mpeg"),
                "wav" => Some("audio/wav"),
                "ogg" => Some("audio/ogg"),
                _ => None,
            }) else {
                send_json_response(
                    socket,
                    "403 Forbidden",
                    "{\"error\":\"unsupported media type\"}",
                )
                .await;
                return;
            };
            match std::fs::read(&canonical) {
                Ok(bytes) => send_binary_response(socket, "200 OK", mime_type, &bytes).await,
                Err(e) => {
                    let err_msg = serde_json::json!({ "error": e.to_string() }).to_string();
                    send_json_response(socket, "500 Internal Server Error", &err_msg).await;
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

        ("POST", "/api/checkpoints/undo") => {
            #[derive(Deserialize)]
            #[serde(rename_all = "camelCase")]
            struct UndoReq {
                workspace_path: Option<String>,
            }
            let req = serde_json::from_str::<UndoReq>(body).unwrap_or(UndoReq {
                workspace_path: None,
            });
            let root = req
                .workspace_path
                .as_deref()
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            match crate::git::undo_rollback(&root) {
                Ok(msg) => {
                    let res = json!({ "status": "ok", "message": msg });
                    send_json_response(socket, "200 OK", &res.to_string()).await;
                }
                Err(err) => {
                    let res = json!({ "status": "error", "message": err });
                    send_json_response(socket, "400 Bad Request", &res.to_string()).await;
                }
            }
        }

        ("GET", "/api/file/media") => {
            let path = query_param(query, "path").unwrap_or_default();
            let root = query_param(query, "workspace").unwrap_or_default();
            let prepared = tokio::task::spawn_blocking(move || {
                if root.trim().is_empty() {
                    return Err("Select a source workspace".to_string());
                }
                crate::workspace::prepare_media(
                    std::path::Path::new(&root),
                    &path,
                    &load_config().map_err(|error| error.to_string())?,
                )
            })
            .await
            .map_err(|error| error.to_string())
            .and_then(|value| value);
            match prepared {
                Ok(media) => match tokio::fs::File::open(&media.path).await {
                    Ok(mut file) => {
                        let headers = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nX-Content-Type-Options: nosniff\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n",
                            media.mime_type, media.byte_length
                        );
                        let mut socket = socket;
                        if socket.write_all(headers.as_bytes()).await.is_ok() {
                            let _ = tokio::io::copy(&mut file, &mut socket).await;
                        }
                    }
                    Err(error) => {
                        send_json_response(
                            socket,
                            "404 Not Found",
                            &json!({"error": error.to_string()}).to_string(),
                        )
                        .await
                    }
                },
                Err(error) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error": error}).to_string(),
                    )
                    .await
                }
            }
        }

        ("GET", "/api/file/image") => {
            let path = query_param(query, "path").unwrap_or_default();
            let root = query_param(query, "workspace").unwrap_or_default();
            let result = tokio::task::spawn_blocking(move || {
                if root.trim().is_empty() {
                    return Err("Select a source workspace".to_string());
                }
                let config = load_config().map_err(|error| error.to_string())?;
                crate::workspace::read_image(std::path::Path::new(&root), &path, &config)
            })
            .await;
            match result {
                Ok(Ok(data_url)) => {
                    send_json_response(socket, "200 OK", &json!({"dataUrl": data_url}).to_string())
                        .await
                }
                other => {
                    let error = match other {
                        Ok(Err(error)) => error,
                        Err(error) => error.to_string(),
                        _ => unreachable!(),
                    };
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        &json!({"error": error}).to_string(),
                    )
                    .await;
                }
            }
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
            let workspace = query_param(query, "workspace");
            let ws_path = workspace.as_deref().map(std::path::Path::new);
            let resolved = crate::files::resolve_readable_path(&file_path, ws_path);
            match std::fs::read_to_string(&resolved) {
                Ok(content) => {
                    let res = json!({
                        "path": resolved.display().to_string(),
                        "content": content
                    });
                    send_json_response(socket, "200 OK", &res.to_string()).await;
                }
                Err(err) => {
                    let res = json!({
                        "error": format!("unable to read file '{}': {err}", resolved.display())
                    });
                    send_json_response(socket, "404 Not Found", &res.to_string()).await;
                }
            }
        }

        ("GET", "/api/models") => {
            // Query params: provider=<id>&apiKey=<key>&baseUrl=<url>
            let provider = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("provider="))
                .map(|v| percent_decode(v))
                .unwrap_or_default();
            let api_key = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("apiKey="))
                .map(|v| percent_decode(v))
                .unwrap_or_default();
            let base_url_opt = query
                .split('&')
                .find_map(|kv| kv.strip_prefix("baseUrl="))
                .map(|v| percent_decode(v));

            let models = match load_config() {
                Ok(cfg) => {
                    crate::slash::models::model_options_for_provider_with_credentials_async(
                        &cfg,
                        &provider,
                        &api_key,
                        base_url_opt.as_deref(),
                    )
                    .await
                }
                Err(_) => Vec::new(),
            };

            send_json_response(
                socket,
                "200 OK",
                &serde_json::json!({ "models": models }).to_string(),
            )
            .await;
        }

        ("GET", "/api/live-models") => {
            let MediaModelQuery { api_key, .. } = MediaModelQuery::parse(query);
            let config = load_config().unwrap_or_default();
            let models = crate::model_catalog::live_models(&config, &api_key).await;

            send_json_response(
                socket,
                "200 OK",
                &serde_json::json!({ "models": models }).to_string(),
            )
            .await;
        }

        ("GET", "/api/image-models") => {
            let MediaModelQuery { provider, api_key } = MediaModelQuery::parse(query);
            let config = load_config().unwrap_or_default();
            let models = crate::model_catalog::image_models(&config, &provider, &api_key).await;

            send_json_response(
                socket,
                "200 OK",
                &serde_json::json!({ "models": models }).to_string(),
            )
            .await;
        }

        ("GET", "/api/video-models") => {
            let MediaModelQuery { provider, api_key } = MediaModelQuery::parse(query);
            let config = load_config().unwrap_or_default();
            let models = crate::model_catalog::video_models(&config, &provider, &api_key).await;

            send_json_response(
                socket,
                "200 OK",
                &serde_json::json!({ "models": models }).to_string(),
            )
            .await;
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::misc::execute: {method} {route}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_model_query_preserves_literal_plus_percent_and_utf8() {
        let mut url = reqwest::Url::parse("http://localhost/api/image-models").unwrap();
        url.query_pairs_mut()
            .append_pair("provider", "OpEnAi")
            .append_pair("apiKey", "key+part%2Fก");
        let options = MediaModelQuery::parse(url.query().unwrap());
        assert_eq!(options.provider, "OpEnAi");
        assert_eq!(options.api_key, "key+part%2Fก");
    }
}

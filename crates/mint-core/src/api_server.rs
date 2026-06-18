use std::{
    net::SocketAddr,
    path::PathBuf,
    process::{Command, Stdio},
};

use serde::Deserialize;
use serde_json::{Value, json};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

use crate::{
    AgentProgress, ApprovalOutcome, ChatRequest, ChatResponse, DEFAULT_CONVERSATION_ID,
    MemoryStore, MintConfig, config_path, create_folder, find_paths, list_saved_pictures,
    load_config, orchestrate_agent_loop, orchestrate_chat_stream_with_fallback,
    orchestrate_chat_with_fallback, save_chat_images, save_config, weather,
};

const MAX_API_REQUEST_BYTES: usize = 32 * 1024 * 1024;

pub async fn start_api_server(port: u16) -> Result<(), std::io::Error> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!(
        "\x1b[32m       Mint Local API Server running at http://{}\x1b[0m",
        addr
    );
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");

    // Start background messaging bridges (Telegram, Discord, Slack)
    crate::start_channels();

    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(val) => val,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            let mut request_bytes = Vec::with_capacity(8192);
            let mut chunk = [0_u8; 8192];
            let mut expected_len: Option<usize> = None;

            loop {
                let n = match socket.read(&mut chunk).await {
                    Ok(n) if n > 0 => n,
                    _ => break,
                };
                request_bytes.extend_from_slice(&chunk[..n]);
                if request_bytes.len() > MAX_API_REQUEST_BYTES {
                    send_json_response(
                        socket,
                        "413 Payload Too Large",
                        "{\"provider\":\"error\",\"model\":\"error\",\"text\":\"Request is too large. Try a smaller image or fewer images.\"}",
                    )
                    .await;
                    return;
                }

                let headers_str = String::from_utf8_lossy(&request_bytes);
                if expected_len.is_none() && headers_str.contains("\r\n\r\n") {
                    expected_len = headers_str
                        .to_lowercase()
                        .find("content-length:")
                        .and_then(|content_length_pos| {
                            let sub = &headers_str[content_length_pos..];
                            let line_end = sub.find("\r\n")?;
                            sub["content-length:".len()..line_end]
                                .trim()
                                .parse::<usize>()
                                .ok()
                        })
                        .and_then(|content_len| {
                            let header_len = headers_str.find("\r\n\r\n")? + 4;
                            Some(header_len + content_len)
                        });
                }

                if let Some(total_len) = expected_len {
                    if request_bytes.len() >= total_len {
                        break;
                    }
                } else if headers_str.contains("\r\n\r\n") {
                    break;
                }
            }

            if request_bytes.is_empty() {
                return;
            }

            let request_str = String::from_utf8_lossy(&request_bytes);
            let lines: Vec<&str> = request_str.split("\r\n").collect();
            if lines.is_empty() {
                return;
            }

            let req_line: Vec<&str> = lines[0].split_whitespace().collect();
            if req_line.len() < 2 {
                return;
            }

            let method = req_line[0];
            let path = req_line[1];

            let header_end = match request_str.find("\r\n\r\n") {
                Some(idx) => idx,
                None => return,
            };
            let body = &request_str[header_end + 4..];

            if method == "OPTIONS" {
                let response = "HTTP/1.1 200 OK\r\n\
                                Access-Control-Allow-Origin: *\r\n\
                                Access-Control-Allow-Headers: Content-Type\r\n\
                                Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
                                Content-Length: 0\r\n\
                                Connection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                return;
            }

            let (route, query) = path.split_once('?').unwrap_or((path, ""));

            match (method, route) {
                ("GET", "/api/status") => {
                    let config = load_config().unwrap_or_default();
                    let path_str = config_path()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let active = config.ai_provider.clone();
                    let available: Vec<String> = config
                        .available_providers()
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect();
                    let status_json = serde_json::json!({
                        "backend": "rust-api-server",
                        "configPath": path_str,
                        "activeProvider": active,
                        "availableProviders": available,
                        "integrations": {},
                        "localIp": get_local_ip()
                    });
                    send_json_response(socket, "200 OK", &status_json.to_string()).await;
                }
                ("GET", "/api/system-info") => {
                    send_json_response(socket, "200 OK", &system_info().to_string()).await;
                }
                ("GET", "/api/smart-context") => {
                    send_json_response(socket, "200 OK", &smart_context().to_string()).await;
                }
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

                    if let Ok(req) = serde_json::from_str::<RenameRequest>(body) {
                        if let Ok(memory) = MemoryStore::open_default() {
                            let updated = memory
                                .rename_chat_session(&req.chat_id, &req.new_title)
                                .unwrap_or(0);
                            let response =
                                serde_json::json!({ "status": "ok", "updated": updated });
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
                ("GET", "/api/profile") => {
                    let key = query_param(query, "key").unwrap_or_default();
                    if let Ok(memory) = MemoryStore::open_default() {
                        let value = memory.get_profile(&key).unwrap_or(None).unwrap_or_default();
                        send_json_response(
                            socket,
                            "200 OK",
                            &serde_json::json!({ "value": value }).to_string(),
                        )
                        .await;
                        return;
                    }
                    send_json_response(socket, "500 Internal Server Error", "{\"value\":\"\"}")
                        .await;
                }
                ("POST", "/api/profile") => {
                    #[derive(Deserialize)]
                    struct ProfileRequest {
                        key: String,
                        value: String,
                    }
                    if let Ok(req) = serde_json::from_str::<ProfileRequest>(body) {
                        if let Ok(memory) = MemoryStore::open_default() {
                            let _ = memory.set_profile(&req.key, &req.value);
                            send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
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
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"error\":\"picture not found\"}",
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
                    if let Ok(new_config) = serde_json::from_str::<MintConfig>(body) {
                        if save_config(&new_config).is_ok() {
                            send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                            return;
                        }
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
                            send_json_response(
                                socket,
                                "500 Internal Server Error",
                                &err_json.to_string(),
                            )
                            .await;
                        }
                    }
                }
                ("POST", "/api/action") => {
                    if let Ok(action) = serde_json::from_str::<ApiAction>(body) {
                        let config = load_config().unwrap_or_default();
                        match execute_api_action(&config, action) {
                            Ok(value) => {
                                send_json_response(socket, "200 OK", &value.to_string()).await
                            }
                            Err(error) => {
                                let err_json = json!({ "success": false, "message": error });
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    &err_json.to_string(),
                                )
                                .await;
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
                ("POST", "/api/chat") => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct ApiChatRequest {
                        message: String,
                        system_instruction: Option<String>,
                        chat_id: Option<String>,
                        image_data_uri: Option<String>,
                        audio_data_uri: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let mut chat_req = ChatRequest {
                            message: req.message,
                            system_instruction: req.system_instruction.unwrap_or_default(),
                            chat_id: req.chat_id,
                            image_data_uri: req.image_data_uri,
                            audio_data_uri: req.audio_data_uri,
                            document_attachment: None,
                            workspace_path: None,
                        };
                        let sent_image = chat_req.image_data_uri.clone();
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
                                if let Some(image) = sent_image {
                                    let _ = save_chat_images(
                                        image
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
                                eprintln!("API Chat error: {:?}", e);
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
                ("POST", "/api/chat-stream") => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct ApiChatRequest {
                        message: String,
                        system_instruction: Option<String>,
                        chat_id: Option<String>,
                        image_data_uri: Option<String>,
                        audio_data_uri: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let mut chat_req = ChatRequest {
                            message: req.message,
                            system_instruction: req.system_instruction.unwrap_or_default(),
                            chat_id: req.chat_id,
                            image_data_uri: req.image_data_uri,
                            audio_data_uri: req.audio_data_uri,
                            document_attachment: None,
                            workspace_path: None,
                        };
                        let sent_image = chat_req.image_data_uri.clone();
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
                                       Access-Control-Allow-Headers: Content-Type\r\n\
                                       Content-Type: application/x-ndjson\r\n\
                                       Cache-Control: no-cache\r\n\
                                       Connection: close\r\n\r\n";
                        if socket.write_all(headers.as_bytes()).await.is_ok() {
                            let _ = socket.flush().await;

                            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();

                            {
                                let tx_progress = tx.clone();
                                let progress_cb = move |progress: AgentProgress| {
                                    if let Ok(json_val) =
                                        serde_json::to_string(&serde_json::json!({
                                            "type": "progress",
                                            "progress": progress
                                        }))
                                    {
                                        let _ = tx_progress.send(format!("{}\n", json_val));
                                    }
                                };

                                let tx_chunk = tx.clone();
                                let on_chunk = move |chunk: String| {
                                    if let Ok(json_val) =
                                        serde_json::to_string(&serde_json::json!({
                                            "type": "chunk",
                                            "chunk": chunk
                                        }))
                                    {
                                        let _ = tx_chunk.send(format!("{}\n", json_val));
                                    }
                                };

                                if is_chat {
                                    let tx_chunk_inner = tx.clone();
                                    let config_clone = config.clone();
                                    let chat_req_clone = chat_req.clone();
                                    let tx_done = tx.clone();
                                    tokio::spawn(async move {
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
                                                    let _ = tx_chunk_inner
                                                        .send(format!("{}\n", json_val));
                                                }
                                            },
                                        )
                                        .await;

                                        match result {
                                            Ok((response, _)) => {
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
                                                eprintln!("API Chat Stream error: {:?}", e);
                                                let err_json = serde_json::json!({
                                                    "type": "done",
                                                    "response": {
                                                        "provider": "error",
                                                        "model": "error",
                                                        "text": format!("Error orchestrating chat: {e}")
                                                    }
                                                });
                                                let _ = tx_done
                                                    .send(format!("{}\n", err_json.to_string()));
                                            }
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
                                    let message = chat_req.message.clone();
                                    let image_data_uri = chat_req.image_data_uri.clone();

                                    tokio::spawn(async move {
                                        let result = orchestrate_agent_loop(
                                            &config_clone,
                                            &message,
                                            &root,
                                            image_data_uri,
                                            chat_id.as_deref(),
                                            fast_mode,
                                            |_| Ok(ApprovalOutcome::Denied),
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
                                                let _ = tx_done
                                                    .send(format!("{}\n", err_json.to_string()));
                                            }
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
                _ => {
                    send_json_response(socket, "404 Not Found", "{\"error\":\"Not Found\"}").await;
                }
            }
        });
    }
}

async fn run_web_agent_loop(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, String> {
    let root = std::env::current_dir().map_err(|error| error.to_string())?;
    let fast_mode = config
        .extra
        .get("enableFastMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let result = orchestrate_agent_loop(
        config,
        &request.message,
        &root,
        request.image_data_uri.clone(),
        request.chat_id.as_deref(),
        fast_mode,
        |_| Ok(ApprovalOutcome::Denied),
        |_| {},
        |_| {},
    )
    .await
    .map_err(|error| error.to_string())?;

    Ok(ChatResponse {
        provider: result.provider,
        model: result.model,
        text: result.summary,
        fallback_provider: result.fallback,
    })
}

fn default_chat_system_instruction() -> String {
    "You are Mint, a warm and helpful Thai assistant. Speak naturally and politely. \
     If the user writes Thai, answer in Thai and refer to yourself as มิ้น. \
     You may discuss mature personal topics at a non-explicit level, including adult relationships, sex education, intimacy, emotions, and feelings. \
     Keep the tone respectful and supportive, avoid graphic sexual detail, and do not engage with sexual content involving minors, coercion, exploitation, or sexual violence."
        .into()
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ApiAction {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    args: Value,
}

fn execute_api_action(config: &MintConfig, action: ApiAction) -> Result<Value, String> {
    match action.kind.as_str() {
        "none" => Ok(success_json("no action requested")),
        "system_info" => Ok(success_json(&system_info().to_string())),
        "open_url" => {
            if !(action.target.starts_with("https://")
                || action.target.starts_with("http://")
                || action.target.starts_with("file://"))
            {
                return Err("only http, https, and file URLs may be opened".into());
            }
            spawn_detached("xdg-open", &[&action.target])?;
            Ok(success_json("opened URL"))
        }
        "search" => {
            let query = action.target.trim();
            if query.is_empty() {
                return Err("search query is required".into());
            }
            let url = format!("https://www.google.com/search?q={}", encode_query(query));
            spawn_detached("xdg-open", &[&url])?;
            Ok(success_json("opened web search"))
        }
        "open_app" => {
            let app = action.target.trim();
            if app.is_empty()
                || !app
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | '.'))
            {
                return Err("application name contains unsupported characters".into());
            }
            spawn_detached(app, &[])?;
            Ok(success_json("opened application"))
        }
        "find_path" => {
            let roots = action.args["roots"]
                .as_array()
                .map(|roots| {
                    roots
                        .iter()
                        .filter_map(Value::as_str)
                        .map(PathBuf::from)
                        .collect::<Vec<_>>()
                })
                .filter(|roots| !roots.is_empty())
                .unwrap_or_else(default_search_roots);
            let limit = action.args["limit"].as_u64().unwrap_or(20).min(100) as usize;
            serde_json::to_value(find_paths(&action.target, &roots, limit, config))
                .map(|matches| json!({ "success": true, "message": matches.to_string(), "matches": matches }))
                .map_err(|error| error.to_string())
        }
        "create_folder" => create_folder(std::path::Path::new(&action.target), config)
            .map(|path| success_json(&format!("created {}", path.display())))
            .map_err(|error| error.to_string()),
        other => Err(format!("local API action '{other}' is not supported")),
    }
}

pub fn get_local_ip() -> Option<String> {
    use std::net::UdpSocket;
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    socket.local_addr().ok().map(|addr| addr.ip().to_string())
}

fn system_info() -> Value {
    json!({
        "backend": "rust-api-server",
        "os": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "family": std::env::consts::FAMILY,
        "host": hostname(),
        "localIp": get_local_ip(),
        "currentDir": std::env::current_dir().ok().map(|path| path.display().to_string()),
        "configPath": config_path().ok().map(|path| path.display().to_string()),
    })
}

fn smart_context() -> Value {
    let active_window = active_window();
    let current_app = active_window.as_ref().map(|window| {
        json!({
            "name": window["appName"],
            "processName": window["processName"],
            "pid": window["pid"]
        })
    });
    json!({
        "capturedAt": unix_timestamp().to_string(),
        "platform": std::env::consts::OS,
        "host": hostname(),
        "activeWindow": active_window,
        "currentApp": current_app,
        "browser": Value::Null,
        "selectedText": selected_text(),
    })
}

fn active_window() -> Option<Value> {
    let id = command_output("xdotool", &["getactivewindow"])?;
    let title = command_output("xdotool", &["getwindowname", &id]).unwrap_or_default();
    let pid = command_output("xdotool", &["getwindowpid", &id]).unwrap_or_default();
    let process_name = command_output("ps", &["-p", &pid, "-o", "comm="]).unwrap_or_default();
    Some(json!({
        "id": id,
        "title": title,
        "appName": process_name,
        "processName": process_name,
        "pid": pid.parse::<u32>().ok(),
        "platform": std::env::consts::OS
    }))
}

fn selected_text() -> String {
    [
        ("wl-paste", vec!["--primary", "--no-newline"]),
        ("xclip", vec!["-selection", "primary", "-out"]),
        ("xsel", vec!["--primary", "--output"]),
    ]
    .into_iter()
    .find_map(|(program, args)| command_output(program, &args))
    .unwrap_or_default()
    .chars()
    .take(2000)
    .collect()
}

fn picture_bytes(filename: &str) -> Result<(String, Vec<u8>), String> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("invalid picture path".into());
    }
    let picture = list_saved_pictures()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|entry| entry.filename == filename)
        .ok_or_else(|| "picture not found".to_string())?;
    let bytes = std::fs::read(&picture.path).map_err(|error| error.to_string())?;
    Ok((picture.mime_type, bytes))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (percent_decode(name) == key).then(|| percent_decode(value))
    })
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let Ok(hex) = u8::from_str_radix(&raw[index + 1..index + 3], 16) {
                output.push(hex);
                index += 3;
                continue;
            }
        }
        output.push(if bytes[index] == b'+' {
            b' '
        } else {
            bytes[index]
        });
        index += 1;
    }
    String::from_utf8_lossy(&output).into_owned()
}

fn default_search_roots() -> Vec<PathBuf> {
    let mut roots = vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
    if let Some(home) = dirs::home_dir() {
        roots.push(home);
    }
    roots
}

fn success_json(message: &str) -> Value {
    json!({ "success": true, "message": message })
}

fn spawn_detached(program: &str, args: &[&str]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("unable to start '{program}': {error}"))
}

fn command_output(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|output| !output.is_empty())
}

fn hostname() -> String {
    command_output("hostname", &[]).unwrap_or_else(|| "unknown".into())
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn encode_query(query: &str) -> String {
    query
        .bytes()
        .flat_map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                vec![byte as char]
            }
            b' ' => vec!['+'],
            _ => format!("%{byte:02X}").chars().collect(),
        })
        .collect()
}

async fn send_json_response(mut socket: tokio::net::TcpStream, status: &str, body_json: &str) {
    let response = format!(
        "HTTP/1.1 {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Content-Type: application/json\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n\
         {}",
        status,
        body_json.len(),
        body_json
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.flush().await;
}

async fn send_binary_response(
    mut socket: tokio::net::TcpStream,
    status: &str,
    content_type: &str,
    body: &[u8],
) {
    let response = format!(
        "HTTP/1.1 {}\r\n\
         Access-Control-Allow-Origin: *\r\n\
         Access-Control-Allow-Headers: Content-Type\r\n\
         Access-Control-Allow-Methods: GET, POST, OPTIONS\r\n\
         Content-Type: {}\r\n\
         Content-Length: {}\r\n\
         Connection: close\r\n\r\n",
        status,
        content_type,
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.write_all(body).await;
    let _ = socket.flush().await;
}

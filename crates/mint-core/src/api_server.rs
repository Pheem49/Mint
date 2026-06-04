use std::net::SocketAddr;
use tokio::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use serde::Deserialize;
use crate::{
    load_config, save_config, config_path, MintConfig, ChatRequest,
    orchestrate_chat_with_fallback, MemoryStore
};

pub async fn start_api_server(port: u16) -> Result<(), std::io::Error> {
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = TcpListener::bind(addr).await?;
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[32m       Mint Local API Server running at http://{}\x1b[0m", addr);
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");

    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(val) => val,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            let mut buf = [0; 65536];
            let mut read_bytes = 0;
            
            loop {
                let n = match socket.read(&mut buf[read_bytes..]).await {
                    Ok(n) if n > 0 => n,
                    _ => break,
                };
                read_bytes += n;
                if read_bytes >= buf.len() {
                    break;
                }
                let headers_str = String::from_utf8_lossy(&buf[..read_bytes]);
                if headers_str.contains("\r\n\r\n") {
                    if let Some(content_length_pos) = headers_str.to_lowercase().find("content-length:") {
                        let sub = &headers_str[content_length_pos..];
                        if let Some(line_end) = sub.find("\r\n") {
                            let len_str = sub["content-length:".len()..line_end].trim();
                            if let Ok(content_len) = len_str.parse::<usize>() {
                                let header_len = headers_str.find("\r\n\r\n").unwrap() + 4;
                                if read_bytes >= header_len + content_len {
                                    break;
                                }
                                continue;
                            }
                        }
                    }
                    if !headers_str.to_uppercase().contains("POST") {
                        break;
                    }
                }
            }

            if read_bytes == 0 {
                return;
            }

            let request_str = String::from_utf8_lossy(&buf[..read_bytes]);
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

            match (method, path) {
                ("GET", "/api/status") => {
                    let config = load_config().unwrap_or_default();
                    let path_str = config_path().map(|p| p.display().to_string()).unwrap_or_default();
                    let active = config.ai_provider.clone();
                    let available: Vec<String> = config.available_providers().into_iter().map(|s| s.to_string()).collect();
                    let status_json = serde_json::json!({
                        "backend": "rust-api-server",
                        "configPath": path_str,
                        "activeProvider": active,
                        "availableProviders": available,
                        "integrations": {}
                    });
                    send_json_response(socket, "200 OK", &status_json.to_string()).await;
                }
                ("GET", "/api/interactions") => {
                    if let Ok(memory) = MemoryStore::open_default() {
                        let list = memory.recent_interactions(50).unwrap_or_default();
                        if let Ok(json_str) = serde_json::to_string(&list) {
                            send_json_response(socket, "200 OK", &json_str).await;
                            return;
                        }
                    }
                    send_json_response(socket, "500 Internal Server Error", "[]").await;
                }
                ("POST", "/api/interactions/clear") => {
                    if let Ok(memory) = MemoryStore::open_default() {
                        let _ = memory.clear_interactions();
                        send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                    } else {
                        send_json_response(socket, "500 Internal Server Error", "{\"status\":\"error\"}").await;
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
                    send_json_response(socket, "400 Bad Request", "{\"status\":\"invalid config json\"}").await;
                }
                ("POST", "/api/chat") => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct ApiChatRequest {
                        message: String,
                        system_instruction: Option<String>,
                        image_data_uri: Option<String>,
                        audio_data_uri: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let chat_req = ChatRequest {
                            message: req.message,
                            system_instruction: req.system_instruction.unwrap_or_default(),
                            image_data_uri: req.image_data_uri,
                            audio_data_uri: req.audio_data_uri,
                        };

                        match orchestrate_chat_with_fallback(&config, &chat_req).await {
                            Ok((resp, _)) => {
                                if let Ok(json_str) = serde_json::to_string(&resp) {
                                    send_json_response(socket, "200 OK", &json_str).await;
                                    return;
                                }
                            }
                            Err(e) => {
                                let err_json = serde_json::json!({
                                    "provider": "error",
                                    "model": "error",
                                    "text": format!("Error orchestrating chat: {}", e)
                                });
                                send_json_response(socket, "500 Internal Server Error", &err_json.to_string()).await;
                                return;
                            }
                        }
                    }
                    send_json_response(socket, "400 Bad Request", "{\"status\":\"invalid chat request body\"}").await;
                }
                _ => {
                    send_json_response(socket, "404 Not Found", "{\"error\":\"Not Found\"}").await;
                }
            }
        });
    }
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

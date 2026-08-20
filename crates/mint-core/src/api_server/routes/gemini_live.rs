use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, mut socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query,
        body: _body,
        request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
        ("GET", "/api/gemini-live") => {
            let Some(ws_key) = get_header(&request_str, "Sec-WebSocket-Key") else {
                send_json_response(
                    socket,
                    "400 Bad Request",
                    "{\"error\":\"missing Sec-WebSocket-Key\"}",
                )
                .await;
                return;
            };

            // A native browser WebSocket can't set an Authorization header, so the
            // token travels as a query param instead.
            let authorized = query_param(query, "token")
                .and_then(|token| session_user_id(token.trim()))
                .is_some();
            if !authorized {
                send_json_response(socket, "401 Unauthorized", "{\"error\":\"unauthorized\"}")
                    .await;
                return;
            }

            let accept_key = websocket_accept_header(&ws_key);
            let handshake_response = format!(
                "HTTP/1.1 101 Switching Protocols\r\n\
                 Upgrade: websocket\r\n\
                 Connection: Upgrade\r\n\
                 Sec-WebSocket-Accept: {accept_key}\r\n\r\n"
            );
            if socket
                .write_all(handshake_response.as_bytes())
                .await
                .is_err()
            {
                return;
            }
            let _ = socket.flush().await;

            // No handshake I/O is performed here — the raw bytes were already
            // consumed off `socket` by the connection loop above, and we just
            // answered the upgrade by hand.
            let ws_stream = WebSocketStream::from_raw_socket(socket, Role::Server, None).await;
            let (mut ws_write, mut ws_read) = ws_stream.split();

            let config = load_config().unwrap_or_default();
            let root = query_param(query, "workspacePath")
                .filter(|path| !path.trim().is_empty())
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
            let chat_id = query_param(query, "chatId").unwrap_or_default();

            let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<String>();
            let handle = crate::gemini_live::start_session(
                config,
                root,
                chat_id,
                |_| Ok(ApprovalOutcome::Denied),
                move |event| {
                    if let Ok(json_val) = serde_json::to_string(&event) {
                        let _ = tx.send(json_val);
                    }
                },
            );

            let writer_task = tokio::spawn(async move {
                while let Some(json_val) = rx.recv().await {
                    if ws_write.send(Message::Text(json_val.into())).await.is_err() {
                        break;
                    }
                }
                let _ = ws_write.close().await;
            });

            while let Some(msg) = ws_read.next().await {
                let Ok(msg) = msg else { break };
                match msg {
                    Message::Text(text) => {
                        if let Ok(payload) = serde_json::from_str::<Value>(&text)
                            && let Some(data) = payload["data"].as_str()
                            && let Ok(pcm) = BASE64.decode(data)
                        {
                            let _ = handle.push_audio(pcm);
                        }
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }

            drop(handle);
            let _ = writer_task.await;
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::gemini_live::execute: {method} {route}"
        ),
    }
}

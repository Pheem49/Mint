use std::{
    net::SocketAddr,
    path::PathBuf,
    process::{Command, Stdio},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{Value, json};
use sha1::{Digest, Sha1};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio_tungstenite::{
    WebSocketStream,
    tungstenite::{Message, protocol::Role},
};

use crate::{
    AgentApproval,
    AgentProgress,
    // Video editing & Speech & Subtitles & Auto Shorts
    AiEditVideoRequest,
    ApprovalOutcome,
    AuthError,
    BurnSubtitleRequest,
    ChatRequest,
    ChatResponse,
    CropRequest,
    DEFAULT_CONVERSATION_ID,
    DetectSilenceRequest,
    ExportRequest,
    ExtractAudioRequest,
    ImageGenRequest,
    MakeShortsRequest,
    MemoryStore,
    MergeRequest,
    MintConfig,
    RemoveSilenceRequest,
    RenderTimelineRequest,
    ResizeRequest,
    TranscribeRequest,
    TranslateSubtitleRequest,
    TrimRequest,
    VideoGenRequest,
    ai_edit_video,
    burn_subtitles,
    config_path,
    create_folder,
    create_session,
    delete_saved_picture,
    destroy_session,
    detect_silence,
    find_paths,
    generate_images,
    generate_srt,
    generate_video,
    get_user,
    list_saved_pictures,
    load_config,
    login_user,
    make_shorts,
    orchestrate_agent_loop,
    orchestrate_chat_stream_with_fallback,
    orchestrate_chat_with_fallback,
    profile_pictures_dir,
    register_user,
    render_timeline,
    save_avatar_file,
    save_chat_images,
    save_config,
    session_user_id,
    transcribe,
    translate_subtitles,
    update_profile,
    video_crop,
    video_export,
    video_extract_audio,
    video_load,
    video_merge,
    video_remove_silence,
    video_resize,
    video_trim,
    weather,
};

const MAX_API_REQUEST_BYTES: usize = 32 * 1024 * 1024;

pub(crate) fn log_api_req(method: &str, route: &str, status: &str, info: Option<&str>) {
    let timestamp = chrono::Local::now().format("%H:%M:%S");
    let status_str = if status.starts_with('2') {
        format!("\x1b[32m{}\x1b[0m", status)
    } else if status.starts_with('4') || status.starts_with('5') {
        format!("\x1b[1;31m{}\x1b[0m", status)
    } else {
        format!("\x1b[33m{}\x1b[0m", status)
    };
    if let Some(detail) = info {
        println!(
            "\x1b[90m[{}]\x1b[0m \x1b[1;36m[API]\x1b[0m \x1b[1m{}\x1b[0m {} -> {} \x1b[90m({})\x1b[0m",
            timestamp, method, route, status_str, detail
        );
    } else {
        println!(
            "\x1b[90m[{}]\x1b[0m \x1b[1;36m[API]\x1b[0m \x1b[1m{}\x1b[0m {} -> {}",
            timestamp, method, route, status_str
        );
    }
}

pub(crate) fn log_api_err(context: &str, error: &dyn std::fmt::Display) {
    let timestamp = chrono::Local::now().format("%H:%M:%S");
    eprintln!(
        "\x1b[90m[{}]\x1b[0m \x1b[1;31m[ERROR]\x1b[0m \x1b[1;33m{}\x1b[0m: {}",
        timestamp, context, error
    );
}

/// Binds the local HTTP API on `port` and blocks forever serving requests.
/// Also starts the messaging bridges and cron scheduler as a side effect —
/// callers (`mint api`, `mint gateway start --api-port`, ...) must not call
/// `channels::start_channels`/`start_cron_scheduler` again themselves, or
/// every bridge loop ends up spawned twice (duplicate Telegram polling,
/// duplicate Discord gateway connections, duplicate replies to the owner).
mod routes;

pub async fn start_api_server(port: u16) -> Result<(), std::io::Error> {
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    let listener = TcpListener::bind(addr).await?;
    // API server banner removed to prevent duplicate output

    // Start background messaging bridges (Telegram, Discord, Slack)
    crate::start_channels();
    // Start the cron scheduler so scheduled agent tasks fire while this server is up
    crate::start_cron_scheduler();

    loop {
        let (mut socket, _) = match listener.accept().await {
            Ok(val) => val,
            Err(_) => continue,
        };

        tokio::spawn(async move {
            // Isolates a panic anywhere in this connection's handling (a malformed
            // request, an unexpected `None` on some edge-case input, ...) so it
            // only drops this one connection instead of leaving it hung with no
            // response and no trace in the logs — same reasoning as
            // `channels::restarting_loop`, which does this for the bridge loops.
            let outcome = std::panic::AssertUnwindSafe(async move {
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
                                Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
                                Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
                                Content-Length: 0\r\n\
                                Connection: close\r\n\r\n";
                let _ = socket.write_all(response.as_bytes()).await;
                let _ = socket.flush().await;
                return;
            }

            let (route, query) = path.split_once('?').unwrap_or((path, ""));

            if !api_auth_ok(&request_str) {
                send_json_response(
                    socket,
                    "401 Unauthorized",
                    "{\"message\":\"Missing or invalid API token. Send it as `Authorization: Bearer <apiAuthToken>`.\"}",
                )
                .await;
                return;
            }

            let auth_label = match authorized_user_id(&request_str) {
                Some(user_id) => format!("auth:{}", &user_id[..user_id.len().min(8)]),
                None => "auth:anonymous".to_string(),
            };

            if route.starts_with("/api/")
                && !route.starts_with("/api/pictures/")
                && !route.starts_with("/api/thumbnails/")
                && route != "/api/chat"
                && route != "/api/chat-stream"
                && route != "/api/image-generate"
                && route != "/api/video-generate"
                && route != "/api/action"
                && route != "/api/config"
                && route != "/api/gemini-live"
            {
                log_api_req(method, route, "200 OK", Some(&auth_label));
            }

            match (method, route) {
                                ("GET", "/api/status") => {
                    routes::status_health::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/gateway/health") => {
                    routes::status_health::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/auth/register") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/auth/login") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/auth/logout") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/auth/session") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/avatar") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("PUT", "/api/auth/profile") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/auth/avatar") => {
                    routes::auth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/detect-tools") => {
                    routes::status_health::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/system-info") => {
                    routes::status_health::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/smart-context") => {
                    routes::status_health::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/interactions") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/chat-sessions") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/chat-sessions/delete") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/chat-sessions/rename") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/learned-skills") => {
                    routes::skills_subagents::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/subagents") => {
                    routes::skills_subagents::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/subagents") => {
                    routes::skills_subagents::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("DELETE", route) if route.starts_with("/api/subagents/") => {
                    routes::skills_subagents::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/mcp/reauth") => {
                    routes::cron_mcp::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/cron") => {
                    routes::cron_mcp::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/cron") => {
                    routes::cron_mcp::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("DELETE", route) if route.starts_with("/api/cron/") => {
                    routes::cron_mcp::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", route)
                    if route.starts_with("/api/cron/") && route.ends_with("/enable") => {
                    routes::cron_mcp::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", route)
                    if route.starts_with("/api/cron/") && route.ends_with("/disable") => {
                    routes::cron_mcp::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/linked-folders") => {
                    routes::linked_folders::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/linked-folders") => {
                    routes::linked_folders::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("DELETE", route) if route.starts_with("/api/linked-folders/") => {
                    routes::linked_folders::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/profile") => {
                    routes::profile_oauth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/profile") => {
                    routes::profile_oauth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/oauth/start") => {
                    routes::profile_oauth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/oauth/callback") => {
                    routes::profile_oauth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/oauth/status") => {
                    routes::profile_oauth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/oauth/revoke") => {
                    routes::profile_oauth::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/active-model") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/interactions/clear") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/interactions") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/interactions/agent-activity") => {
                    routes::sessions::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/pictures") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", route) if route.starts_with("/api/pictures/") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("DELETE", route) if route.starts_with("/api/pictures/") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", route) if route.starts_with("/api/thumbnails/") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/config") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/config") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/weather") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/action") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/slash") => {
                    routes::slash::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/uploads") => {
                    routes::misc::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/chat") => {
                    routes::chat::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/cancel-chat") => {
                    routes::chat::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/chat-stream") => {
                    routes::chat::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/submit-approval") => {
                    routes::chat::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/image-generate") => {
                    routes::media_gen::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("POST", "/api/video-generate") => {
                    routes::media_gen::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                                ("GET", "/api/gemini-live") => {
                    routes::gemini_live::execute(
                        routes::RequestCtx {
                            method,
                            route,
                            query,
                            body,
                            request_str: &request_str,
                            request_bytes: &request_bytes,
                            header_end,
                            auth_label: auth_label.clone(),
                        },
                        socket,
                    )
                    .await;
                }
                _ => {
                    send_json_response(socket, "404 Not Found", "{\"error\":\"Not Found\"}").await;
                }
            }
            })
            .catch_unwind()
            .await;
            if let Err(payload) = outcome {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] api_server connection handler panicked: {message}");
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
    // Same pre-scoping as `/api/chat-stream`'s agent-mode branch in
    // routes/chat.rs and for the same reason: `root` above deliberately
    // stays this server process's own cwd (not the client's workspace), so
    // `orchestrate_agent_loop`'s self-derivation from `root` alone can't
    // distinguish workspaces here — scope from the client's `workspace_path`
    // explicitly instead. Idempotent/safe either way (see `scoped_chat_id`).
    let scoped_chat_id = crate::agent::memory::scoped_chat_id(
        request
            .chat_id
            .as_deref()
            .unwrap_or(DEFAULT_CONVERSATION_ID),
        request.workspace_path.as_deref(),
    );
    let result = orchestrate_agent_loop(
        config,
        &request.message,
        &root,
        request.image_data_uri.clone(),
        request.audio_data_uri.clone(),
        request.video_data_uri.clone(),
        Some(scoped_chat_id.as_str()),
        request.agent_id.as_deref(),
        None,
        request.pinned_mcp_server.as_deref(),
        fast_mode,
        false,
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
        fallback_reason: None,
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
    })
}

fn default_chat_system_instruction() -> String {
    crate::prompts::chat::default_chat_system_instruction()
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

fn thumbnail_bytes(filename: &str) -> Result<(String, Vec<u8>), String> {
    if filename.contains('/') || filename.contains('\\') || filename.contains("..") {
        return Err("invalid thumbnail path".into());
    }
    let id = filename
        .strip_suffix(".thumb.png")
        .ok_or_else(|| "invalid thumbnail name".to_string())?;
    let picture = list_saved_pictures()
        .map_err(|error| error.to_string())?
        .into_iter()
        .find(|entry| entry.id == id)
        .ok_or_else(|| "picture not found".to_string())?;
    let thumb_path = picture
        .thumbnail_path
        .ok_or_else(|| "thumbnail not found".to_string())?;
    let bytes = std::fs::read(&thumb_path).map_err(|error| error.to_string())?;
    Ok(("image/png".to_string(), bytes))
}

fn query_param(query: &str, key: &str) -> Option<String> {
    query.split('&').find_map(|pair| {
        let (name, value) = pair.split_once('=')?;
        (percent_decode(name) == key).then(|| percent_decode(value))
    })
}

/// Computes the `Sec-WebSocket-Accept` response header per RFC 6455: base64(SHA1(key + GUID)).
fn websocket_accept_header(client_key: &str) -> String {
    const WS_GUID: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut hasher = Sha1::new();
    hasher.update(client_key.as_bytes());
    hasher.update(WS_GUID.as_bytes());
    BASE64.encode(hasher.finalize())
}

/// Reads a header value from the raw request text (headers + body, as
/// assembled in the connection loop above).
fn get_header(request_str: &str, header_name: &str) -> Option<String> {
    let header_end = request_str.find("\r\n\r\n").unwrap_or(request_str.len());
    let headers = &request_str[..header_end];
    let needle = format!("{header_name}:");
    headers.lines().find_map(|line| {
        if line.len() > needle.len() && line[..needle.len()].eq_ignore_ascii_case(&needle) {
            Some(line[needle.len()..].trim().to_string())
        } else {
            None
        }
    })
}

/// Opt-in shared-secret gate for the *whole* API server (every route except
/// the CORS preflight handled earlier, which returns before this runs).
/// Unset by default (no `apiAuthToken` in config), so existing local-only
/// setups — the desktop app, `mint web` — keep working exactly as before.
/// An operator who exposes this port beyond localhost (e.g. a VPS reachable
/// over an SSH tunnel/Tailscale) can set `apiAuthToken` via
/// `mint config set apiAuthToken <token>` to require every request to carry
/// it as `Authorization: Bearer <token>` — a second layer on top of the
/// tunnel itself, since only 3 of this server's ~60 routes otherwise check
/// `authorized_user_id` (that login system is per-user profile data, not a
/// general access-control boundary for the rest of the API).
fn api_auth_ok(request_str: &str) -> bool {
    let Some(expected_token) = load_config().ok().and_then(|config| {
        config
            .extra
            .get("apiAuthToken")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    }) else {
        return true;
    };
    token_matches(request_str, &expected_token)
}

/// Checks the request's `Authorization: Bearer <token>` header against the
/// expected token. Split out from `api_auth_ok` so this comparison — the
/// actual security check — is testable without touching the real on-disk
/// config that `api_auth_ok` reads `expected_token` from.
fn token_matches(request_str: &str, expected_token: &str) -> bool {
    get_header(request_str, "Authorization")
        .and_then(|header| {
            header
                .strip_prefix("Bearer ")
                .map(|token| token.trim().to_owned())
        })
        .is_some_and(|provided_token| provided_token == expected_token)
}

/// Resolves the logged-in user id (web mode) from the `Authorization: Bearer
/// <token>` header, if present and valid.
fn authorized_user_id(request_str: &str) -> Option<String> {
    let header = get_header(request_str, "Authorization")?;
    let token = header.strip_prefix("Bearer ")?;
    session_user_id(token.trim())
}

fn percent_decode(raw: &str) -> String {
    let bytes = raw.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(hex) = u8::from_str_radix(&raw[index + 1..index + 3], 16)
        {
            output.push(hex);
            index += 3;
            continue;
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
         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
         Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
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
         Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
         Access-Control-Allow-Methods: GET, POST, PUT, DELETE, OPTIONS\r\n\
         Cache-Control: public, max-age=86400\r\n\
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

#[cfg(test)]
mod tests {
    use super::*;

    // -- get_header --------------------------------------------------

    #[test]
    fn get_header_finds_header_case_insensitively() {
        let req =
            "GET /api/status HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer abc123\r\n\r\n";
        assert_eq!(
            get_header(req, "authorization").as_deref(),
            Some("Bearer abc123")
        );
        assert_eq!(
            get_header(req, "AUTHORIZATION").as_deref(),
            Some("Bearer abc123")
        );
    }

    #[test]
    fn get_header_trims_leading_whitespace() {
        let req = "GET / HTTP/1.1\r\nX-Test:    spaced-value  \r\n\r\n";
        assert_eq!(get_header(req, "X-Test").as_deref(), Some("spaced-value"));
    }

    #[test]
    fn get_header_missing_returns_none() {
        let req = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert_eq!(get_header(req, "Authorization"), None);
    }

    #[test]
    fn get_header_does_not_match_suffix_header_names() {
        // "Authorization-Extra" must not be picked up when asking for "Authorization".
        let req = "GET / HTTP/1.1\r\nAuthorization-Extra: nope\r\n\r\n";
        assert_eq!(get_header(req, "Authorization"), None);
    }

    #[test]
    fn get_header_ignores_body_content() {
        // A header-looking line in the body (after \r\n\r\n) must not be read.
        let req = "POST / HTTP/1.1\r\nHost: localhost\r\n\r\nAuthorization: Bearer body-smuggled";
        assert_eq!(get_header(req, "Authorization"), None);
    }

    // -- token_matches (the actual gateway auth-gate comparison) ------

    #[test]
    fn token_matches_accepts_correct_bearer_token() {
        let req = "GET / HTTP/1.1\r\nAuthorization: Bearer secret-token\r\n\r\n";
        assert!(token_matches(req, "secret-token"));
    }

    #[test]
    fn token_matches_rejects_wrong_token() {
        let req = "GET / HTTP/1.1\r\nAuthorization: Bearer wrong-token\r\n\r\n";
        assert!(!token_matches(req, "secret-token"));
    }

    #[test]
    fn token_matches_rejects_missing_header() {
        let req = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(!token_matches(req, "secret-token"));
    }

    #[test]
    fn token_matches_rejects_header_without_bearer_prefix() {
        let req = "GET / HTTP/1.1\r\nAuthorization: secret-token\r\n\r\n";
        assert!(!token_matches(req, "secret-token"));
    }

    #[test]
    fn token_matches_trims_surrounding_whitespace_on_token() {
        let req = "GET / HTTP/1.1\r\nAuthorization: Bearer   secret-token  \r\n\r\n";
        assert!(token_matches(req, "secret-token"));
    }

    #[test]
    fn token_matches_rejects_empty_expected_token() {
        // Callers should never reach here with an empty expected_token (api_auth_ok
        // filters those out before calling), but the comparison itself must not
        // treat "no token provided" as matching "no token expected".
        let req = "GET / HTTP/1.1\r\nHost: localhost\r\n\r\n";
        assert!(!token_matches(req, ""));
    }

    // -- query_param / percent_decode ---------------------------------

    #[test]
    fn percent_decode_handles_percent_and_plus_encoding() {
        assert_eq!(percent_decode("hello%20world"), "hello world");
        assert_eq!(percent_decode("a+b"), "a b");
        assert_eq!(percent_decode("100%25"), "100%");
    }

    #[test]
    fn percent_decode_leaves_invalid_escapes_intact() {
        assert_eq!(percent_decode("50%"), "50%");
        assert_eq!(percent_decode("50%zz"), "50%zz");
    }

    #[test]
    fn query_param_extracts_decoded_value() {
        let query = "path=%2Fhome%2Fuser&limit=10";
        assert_eq!(query_param(query, "path").as_deref(), Some("/home/user"));
        assert_eq!(query_param(query, "limit").as_deref(), Some("10"));
        assert_eq!(query_param(query, "missing"), None);
    }

    #[test]
    fn encode_query_percent_encodes_reserved_characters() {
        assert_eq!(encode_query("a b"), "a+b");
        assert_eq!(encode_query("100%"), "100%25");
        assert_eq!(encode_query("safe-Value_1.0~"), "safe-Value_1.0~");
    }

    #[test]
    fn encode_query_round_trips_through_percent_decode() {
        let original = "hello world/ünïcode? 100%";
        assert_eq!(percent_decode(&encode_query(original)), original);
    }

    // -- websocket_accept_header ---------------------------------------

    #[test]
    fn websocket_accept_header_matches_rfc6455_example() {
        // Worked example straight from RFC 6455 section 1.3.
        assert_eq!(
            websocket_accept_header("dGhlIHNhbXBsZSBub25jZQ=="),
            "s3pPLMBiTxaQ9kYGzzhZRbK+xOo="
        );
    }

    // -- misc small helpers ---------------------------------------------

    #[test]
    fn success_json_shape() {
        let value = success_json("done");
        assert_eq!(value["success"], true);
        assert_eq!(value["message"], "done");
    }

    #[test]
    fn unix_timestamp_is_recent_and_monotonic_enough() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let ts = unix_timestamp();
        assert!(ts >= before);
        assert!(ts < before + 5);
    }
}

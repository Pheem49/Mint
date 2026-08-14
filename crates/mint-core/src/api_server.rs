use std::{
    net::SocketAddr,
    path::PathBuf,
    process::{Command, Stdio},
};

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use futures_util::{SinkExt, StreamExt};
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
                ("POST", "/api/auth/register") => {
                    #[derive(Deserialize)]
                    struct RegisterRequest {
                        #[serde(default)]
                        name: Option<String>,
                        email: String,
                        password: String,
                    }
                    let Ok(req) = serde_json::from_str::<RegisterRequest>(body) else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"message\":\"Invalid request body.\"}",
                        )
                        .await;
                        return;
                    };
                    match register_user(req.name, &req.email, &req.password) {
                        Ok(user) => {
                            let token = create_session(&user.id);
                            send_json_response(
                                socket,
                                "201 Created",
                                &json!({ "token": token, "user": user }).to_string(),
                            )
                            .await;
                        }
                        Err(err) => {
                            let status = match err {
                                AuthError::EmailTaken => "409 Conflict",
                                AuthError::MissingCredentials | AuthError::PasswordTooShort => {
                                    "400 Bad Request"
                                }
                                _ => "500 Internal Server Error",
                            };
                            send_json_response(
                                socket,
                                status,
                                &json!({ "message": err.to_string() }).to_string(),
                            )
                            .await;
                        }
                    }
                    return;
                }
                ("POST", "/api/auth/login") => {
                    #[derive(Deserialize)]
                    struct LoginRequest {
                        email: String,
                        password: String,
                    }
                    let Ok(req) = serde_json::from_str::<LoginRequest>(body) else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"message\":\"Invalid request body.\"}",
                        )
                        .await;
                        return;
                    };
                    match login_user(&req.email, &req.password) {
                        Ok(user) => {
                            let token = create_session(&user.id);
                            send_json_response(
                                socket,
                                "200 OK",
                                &json!({ "token": token, "user": user }).to_string(),
                            )
                            .await;
                        }
                        Err(_) => {
                            send_json_response(
                                socket,
                                "401 Unauthorized",
                                "{\"message\":\"Invalid email or password.\"}",
                            )
                            .await;
                        }
                    }
                    return;
                }
                ("POST", "/api/auth/logout") => {
                    if let Some(header) = get_header(&request_str, "Authorization")
                        && let Some(token) = header.strip_prefix("Bearer ")
                    {
                        destroy_session(token.trim());
                    }
                    send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                    return;
                }
                ("GET", "/api/auth/session") => {
                    let user = authorized_user_id(&request_str)
                        .and_then(|id| get_user(&id).ok().flatten());
                    send_json_response(socket, "200 OK", &json!({ "user": user }).to_string())
                        .await;
                    return;
                }
                ("GET", "/api/avatar") => {
                    let key = query_param(query, "key").unwrap_or_default();
                    // Only ever serve a bare filename from the shared profile
                    // pictures directory — never treat `key` as a path.
                    let filename = PathBuf::from(&key)
                        .file_name()
                        .map(|name| name.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let content_type = match filename.rsplit('.').next().unwrap_or("") {
                        "jpg" | "jpeg" => "image/jpeg",
                        "webp" => "image/webp",
                        "gif" => "image/gif",
                        _ => "image/png",
                    };
                    let file_path = profile_pictures_dir()
                        .ok()
                        .filter(|_| !filename.is_empty())
                        .map(|dir| dir.join(&filename));
                    match file_path.and_then(|path| std::fs::read(path).ok()) {
                        Some(bytes) => {
                            send_binary_response(socket, "200 OK", content_type, &bytes).await
                        }
                        None => {
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"message\":\"Avatar not found\"}",
                            )
                            .await
                        }
                    }
                    return;
                }
                ("PUT", "/api/auth/profile") => {
                    #[derive(Deserialize)]
                    struct ProfileUpdateRequest {
                        #[serde(default)]
                        name: Option<String>,
                        #[serde(default)]
                        image: Option<String>,
                    }
                    let Some(user_id) = authorized_user_id(&request_str) else {
                        send_json_response(
                            socket,
                            "401 Unauthorized",
                            "{\"message\":\"Unauthorized\"}",
                        )
                        .await;
                        return;
                    };
                    let Ok(req) = serde_json::from_str::<ProfileUpdateRequest>(body) else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"message\":\"Invalid request body.\"}",
                        )
                        .await;
                        return;
                    };
                    match update_profile(&user_id, req.name, req.image) {
                        Ok(user) => {
                            send_json_response(
                                socket,
                                "200 OK",
                                &json!({ "user": user }).to_string(),
                            )
                            .await;
                        }
                        Err(err) => {
                            send_json_response(
                                socket,
                                "500 Internal Server Error",
                                &json!({ "message": err.to_string() }).to_string(),
                            )
                            .await;
                        }
                    }
                    return;
                }
                ("POST", "/api/auth/avatar") => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct AvatarUploadRequest {
                        file_name: String,
                        data_base64: String,
                    }
                    let Some(user_id) = authorized_user_id(&request_str) else {
                        send_json_response(
                            socket,
                            "401 Unauthorized",
                            "{\"message\":\"Unauthorized\"}",
                        )
                        .await;
                        return;
                    };
                    let Ok(req) = serde_json::from_str::<AvatarUploadRequest>(body) else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"message\":\"Invalid request body.\"}",
                        )
                        .await;
                        return;
                    };
                    let Ok(bytes) = BASE64.decode(req.data_base64.as_bytes()) else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"message\":\"Invalid image data.\"}",
                        )
                        .await;
                        return;
                    };
                    let extension = req
                        .file_name
                        .rsplit('.')
                        .next()
                        .unwrap_or("png")
                        .to_lowercase();
                    match save_avatar_file(&bytes, &extension)
                        .and_then(|url| update_profile(&user_id, None, Some(url)))
                    {
                        Ok(user) => {
                            send_json_response(
                                socket,
                                "200 OK",
                                &json!({ "user": user }).to_string(),
                            )
                            .await;
                        }
                        Err(err) => {
                            send_json_response(
                                socket,
                                "500 Internal Server Error",
                                &json!({ "message": err.to_string() }).to_string(),
                            )
                            .await;
                        }
                    }
                    return;
                }
                ("GET", "/api/detect-tools") => {
                    let tools_json = serde_json::json!({
                        "docker": crate::config::which("docker"),
                        "git": crate::config::which("git"),
                        "gh": crate::config::which("gh"),
                        "node": crate::config::which("node")
                    });
                    send_json_response(socket, "200 OK", &tools_json.to_string()).await;
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
                ("GET", "/api/learned-skills") => {
                    let mut skills = match MemoryStore::open_default() {
                        Ok(m) => m.learned_skills(100).unwrap_or_default(),
                        Err(_) => Vec::new(),
                    };

                    if let Some(home) = dirs::home_dir() {
                        let global_agents_path =
                            home.join(".gemini").join("config").join("AGENTS.md");
                        crate::skills::load_agent_rules_file(&global_agents_path, &mut skills);

                        let global_skills_path =
                            home.join(".config").join("mint").join("mint-skills");
                        crate::skills::load_skills_from_dir(&global_skills_path, &mut skills);
                    }

                    if let Ok(current_dir) = std::env::current_dir() {
                        let workspace_agents_path1 = current_dir.join(".agents").join("AGENTS.md");
                        crate::skills::load_agent_rules_file(&workspace_agents_path1, &mut skills);
                        let workspace_agents_path2 = current_dir.join("AGENTS.md");
                        crate::skills::load_agent_rules_file(&workspace_agents_path2, &mut skills);

                        let workspace_skills_path1 = current_dir.join(".agents").join("skills");
                        crate::skills::load_skills_from_dir(&workspace_skills_path1, &mut skills);
                        let workspace_skills_path2 = current_dir.join("skills");
                        crate::skills::load_skills_from_dir(&workspace_skills_path2, &mut skills);

                        if let Ok(canonical_cwd) = current_dir.canonicalize() {
                            for s in &mut skills {
                                if let Ok(p) = std::path::Path::new(&s.source_path).canonicalize() {
                                    if p.starts_with(&canonical_cwd) {
                                        s.is_workspace = true;
                                    }
                                }
                            }
                        }
                    }

                    let mut unique_skills = std::collections::BTreeMap::new();
                    for s in skills {
                        unique_skills.insert(s.name.clone(), s);
                    }

                    let list: Vec<_> = unique_skills.into_values().collect();
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
                ("POST", "/api/mcp/reauth") => {
                    #[derive(Deserialize)]
                    struct ReauthRequest {
                        #[serde(rename = "serverName")]
                        server_name: String,
                    }
                    match serde_json::from_str::<ReauthRequest>(body) {
                        Ok(req) => {
                            let server_name = req.server_name;
                            let result = tokio::task::spawn_blocking(move || {
                                crate::reauth_mcp_server(&server_name)
                            })
                            .await;
                            match result {
                                Ok(Ok(success)) => {
                                    send_json_response(
                                        socket,
                                        "200 OK",
                                        &json!({ "success": success }).to_string(),
                                    )
                                    .await;
                                }
                                Ok(Err(err)) => {
                                    let err_msg = json!({ "error": err.to_string() }).to_string();
                                    send_json_response(socket, "400 Bad Request", &err_msg).await;
                                }
                                Err(err) => {
                                    let err_msg =
                                        json!({ "error": format!("reauth task failed: {err}") })
                                            .to_string();
                                    send_json_response(socket, "500 Internal Server Error", &err_msg)
                                        .await;
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
                ("GET", "/api/cron") => {
                    let jobs = crate::CronStore::open_default()
                        .and_then(|store| store.list())
                        .unwrap_or_default();
                    send_json_response(
                        socket,
                        "200 OK",
                        &serde_json::to_string(&jobs).unwrap_or_default(),
                    )
                    .await;
                }
                ("POST", "/api/cron") => {
                    match serde_json::from_str::<crate::CronJobDraft>(body) {
                        Ok(draft) => match crate::CronStore::open_default() {
                            Ok(store) => {
                                match store.add(draft.name, draft.schedule, draft.task, draft.workspace)
                                {
                                    Ok(job) => {
                                        send_json_response(
                                            socket,
                                            "200 OK",
                                            &serde_json::to_string(&job).unwrap_or_default(),
                                        )
                                        .await;
                                    }
                                    Err(err) => {
                                        let err_msg = json!({ "error": err.to_string() }).to_string();
                                        send_json_response(socket, "400 Bad Request", &err_msg).await;
                                    }
                                }
                            }
                            Err(err) => {
                                let err_msg = json!({ "error": err.to_string() }).to_string();
                                send_json_response(socket, "400 Bad Request", &err_msg).await;
                            }
                        },
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
                ("DELETE", route) if route.starts_with("/api/cron/") => {
                    let id = percent_decode(route.trim_start_matches("/api/cron/"));
                    match crate::CronStore::open_default().and_then(|store| store.remove(&id)) {
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
                ("POST", route)
                    if route.starts_with("/api/cron/") && route.ends_with("/enable") =>
                {
                    let id = percent_decode(
                        route
                            .trim_start_matches("/api/cron/")
                            .trim_end_matches("/enable"),
                    );
                    match crate::CronStore::open_default()
                        .and_then(|store| store.set_enabled(&id, true))
                    {
                        Ok(Some(job)) => {
                            send_json_response(
                                socket,
                                "200 OK",
                                &serde_json::to_string(&job).unwrap_or_default(),
                            )
                            .await;
                        }
                        Ok(None) => {
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"error\":\"No cron job with that id.\"}",
                            )
                            .await;
                        }
                        Err(err) => {
                            let err_msg = json!({ "error": err.to_string() }).to_string();
                            send_json_response(socket, "400 Bad Request", &err_msg).await;
                        }
                    }
                }
                ("POST", route)
                    if route.starts_with("/api/cron/") && route.ends_with("/disable") =>
                {
                    let id = percent_decode(
                        route
                            .trim_start_matches("/api/cron/")
                            .trim_end_matches("/disable"),
                    );
                    match crate::CronStore::open_default()
                        .and_then(|store| store.set_enabled(&id, false))
                    {
                        Ok(Some(job)) => {
                            send_json_response(
                                socket,
                                "200 OK",
                                &serde_json::to_string(&job).unwrap_or_default(),
                            )
                            .await;
                        }
                        Ok(None) => {
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"error\":\"No cron job with that id.\"}",
                            )
                            .await;
                        }
                        Err(err) => {
                            let err_msg = json!({ "error": err.to_string() }).to_string();
                            send_json_response(socket, "400 Bad Request", &err_msg).await;
                        }
                    }
                }
                ("GET", "/api/linked-folders") => {
                    let folders = load_config()
                        .ok()
                        .and_then(|config| {
                            crate::linked_folders::configured_linked_folders(&config).ok()
                        })
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
                            match crate::add_linked_folder(
                                &draft.name,
                                &draft.path,
                                draft.description,
                            ) {
                                Ok(()) => {
                                    send_json_response(
                                        socket,
                                        "200 OK",
                                        "{\"status\":\"ok\"}",
                                    )
                                    .await;
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
                    if let Ok(req) = serde_json::from_str::<ProfileRequest>(body)
                        && let Ok(memory) = MemoryStore::open_default()
                    {
                        let _ = memory.set_profile(&req.key, &req.value);
                        send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                        return;
                    }
                    send_json_response(
                        socket,
                        "500 Internal Server Error",
                        "{\"status\":\"error\"}",
                    )
                    .await;
                }
                ("GET", "/api/oauth/start") => {
                    let provider =
                        query_param(query, "provider").unwrap_or_else(|| "google".to_string());
                    let redirect_uri = format!("http://localhost:3000/api/oauth/callback");
                    let config = load_config().unwrap_or_default();

                    let custom_client_id = match provider.as_str() {
                        "google" | "gmail" | "google_calendar" | "youtube_music" => config
                            .extra
                            .get("gmailClientId")
                            .and_then(Value::as_str)
                            .or_else(|| {
                                config
                                    .extra
                                    .get("googleCalendarClientId")
                                    .and_then(Value::as_str)
                            }),
                        "spotify" => config.extra.get("spotifyClientId").and_then(Value::as_str),
                        "github" => config.extra.get("githubClientId").and_then(Value::as_str),
                        "vercel" => config.extra.get("vercelClientId").and_then(Value::as_str),
                        "notion" => config.extra.get("notionApiKey").and_then(Value::as_str),
                        _ => None,
                    };

                    if let Some((auth_url, state)) =
                        crate::oauth::build_auth_url(&provider, &redirect_uri, custom_client_id)
                    {
                        send_json_response(
                            socket,
                            "200 OK",
                            &serde_json::json!({
                                "status": "ok",
                                "auth_url": auth_url,
                                "state": state,
                                "redirect_uri": redirect_uri
                            })
                            .to_string(),
                        )
                        .await;
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"Invalid provider\"}",
                        )
                        .await;
                    }
                    return;
                }
                ("GET", "/api/oauth/callback") => {
                    let code = query_param(query, "code").unwrap_or_default();
                    let state = query_param(query, "state").unwrap_or_default();
                    let provider = state.split('-').next().unwrap_or("google");
                    let redirect_uri = format!("http://localhost:3000/api/oauth/callback");
                    let config = load_config().unwrap_or_default();

                    let (custom_client_id, custom_client_secret) = match provider {
                        "google" | "gmail" | "google_calendar" | "youtube_music" => (
                            config
                                .extra
                                .get("gmailClientId")
                                .and_then(Value::as_str)
                                .or_else(|| {
                                    config
                                        .extra
                                        .get("googleCalendarClientId")
                                        .and_then(Value::as_str)
                                }),
                            config
                                .extra
                                .get("gmailClientSecret")
                                .and_then(Value::as_str)
                                .or_else(|| {
                                    config
                                        .extra
                                        .get("googleCalendarClientSecret")
                                        .and_then(Value::as_str)
                                }),
                        ),
                        "spotify" => (
                            config.extra.get("spotifyClientId").and_then(Value::as_str),
                            config
                                .extra
                                .get("spotifyClientSecret")
                                .and_then(Value::as_str),
                        ),
                        "github" => (
                            config.extra.get("githubClientId").and_then(Value::as_str),
                            config
                                .extra
                                .get("githubClientSecret")
                                .and_then(Value::as_str),
                        ),
                        "vercel" => (
                            config.extra.get("vercelClientId").and_then(Value::as_str),
                            config
                                .extra
                                .get("vercelClientSecret")
                                .and_then(Value::as_str),
                        ),
                        "notion" => (
                            config.extra.get("notionApiKey").and_then(Value::as_str),
                            None,
                        ),
                        _ => (None, None),
                    };

                    let result = crate::oauth::exchange_code(
                        provider,
                        &code,
                        &state,
                        &redirect_uri,
                        custom_client_id,
                        custom_client_secret,
                    )
                    .await;

                    let html_body = match result {
                        Ok(tokens) => format!(
                            "<!DOCTYPE html><html><head><title>Mint Agent Connected</title><style>body {{ font-family: system-ui, sans-serif; background: #0f172a; color: #fff; text-align: center; padding: 40px; }} .card {{ background: #1e293b; padding: 30px; border-radius: 12px; display: inline-block; box-shadow: 0 4px 20px rgba(0,0,0,0.5); }} h2 {{ color: #10b981; }}</style></head><body><div class='card'><h2>🟢 Connection Successful!</h2><p>Mint Agent has successfully connected to <strong>{}</strong> ({}).</p><p>You can close this tab now and return to Mint.</p></div><script>setTimeout(() => window.close(), 2500);</script></body></html>",
                            tokens.provider,
                            tokens.account_email.as_deref().unwrap_or("Connected")
                        ),
                        Err(err) => format!(
                            "<!DOCTYPE html><html><head><title>Mint OAuth Error</title><style>body {{ font-family: system-ui, sans-serif; background: #0f172a; color: #fff; text-align: center; padding: 40px; }} .card {{ background: #1e293b; padding: 30px; border-radius: 12px; display: inline-block; border: 1px solid #ef4444; }} h2 {{ color: #ef4444; }}</style></head><body><div class='card'><h2>❌ Connection Failed</h2><p>Error: {}</p></div></body></html>",
                            err
                        ),
                    };

                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        html_body.len(),
                        html_body
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                    return;
                }
                ("GET", "/api/oauth/status") => {
                    let statuses = crate::oauth::list_oauth_statuses();
                    send_json_response(
                        socket,
                        "200 OK",
                        &serde_json::json!({ "statuses": statuses }).to_string(),
                    )
                    .await;
                    return;
                }
                ("POST", "/api/oauth/revoke") => {
                    #[derive(Deserialize)]
                    struct RevokeReq {
                        provider: String,
                    }
                    if let Ok(req) = serde_json::from_str::<RevokeReq>(body) {
                        let _ = crate::oauth::revoke_oauth_tokens(&req.provider);
                        send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                        return;
                    }
                    send_json_response(socket, "400 Bad Request", "{\"status\":\"error\"}").await;
                    return;
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
                                    send_json_response(socket, "200 OK", &res_json.to_string())
                                        .await;
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
                            .set_interaction_agent_activity_json(
                                payload.interaction_id,
                                &activity_json,
                            )
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
                ("DELETE", route) if route.starts_with("/api/pictures/") => {
                    let id = percent_decode(route.trim_start_matches("/api/pictures/"));
                    match delete_saved_picture(&id) {
                        Ok(_) => {
                            send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                        }
                        Err(err) => {
                            let err_msg =
                                serde_json::json!({ "error": err.to_string() }).to_string();
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
                ("POST", "/api/uploads") => {
                    // Accept raw body bytes and optional filename query param.
                    // Body is the raw file bytes (client should POST the file as the request body).
                    let filename_param = query_param(query, "filename");
                    let header_end_idx = header_end as usize;
                    // extract raw bytes for body
                    let body_bytes = &request_bytes[header_end_idx + 4..];

                    if body_bytes.is_empty() {
                        send_json_response(socket, "400 Bad Request", "{\"error\":\"empty body\"}")
                            .await;
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
                            send_json_response(
                                socket,
                                "500 Internal Server Error",
                                &err_json.to_string(),
                            )
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

                ("POST", "/api/chat") => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct ApiChatRequest {
                        message: String,
                        system_instruction: Option<String>,
                        chat_id: Option<String>,
                        image_data_uri: Option<String>,
                        audio_data_uri: Option<String>,
                        video_data_uri: Option<String>,
                        document_attachment: Option<crate::chat::DocumentAttachment>,
                        agent_id: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let chat_req = ChatRequest {
                            message: req.message,
                            system_instruction: req.system_instruction.unwrap_or_default(),
                            chat_id: req.chat_id,
                            image_data_uri: req.image_data_uri,
                            audio_data_uri: req.audio_data_uri,
                            video_data_uri: req.video_data_uri,
                            document_attachment: req.document_attachment,
                            workspace_path: None,
                            agent_id: req.agent_id,
                            plan_mode: false,
                            messages: None,
                            tools: None,
                        };
                        let mut chat_req = match chat_req.with_document_context(&config) {
                            Ok(req) => req,
                            Err(error) => {
                                log_api_err("API /api/chat document error", &error);
                                let err_json = serde_json::json!({
                                    "text": format!("Could not read the attached document: {error}")
                                });
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    &err_json.to_string(),
                                )
                                .await;
                                return;
                            }
                        };
                        let sent_image = chat_req.image_data_uri.clone();
                        let sent_video = chat_req.video_data_uri.clone();
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
                                log_api_req(
                                    "POST",
                                    "/api/chat",
                                    "200 OK",
                                    Some(&format!(
                                        "Model: {} | {}",
                                        config.ai_provider, auth_label
                                    )),
                                );
                                if let Some(image) = sent_image {
                                    let _ = save_chat_images(
                                        image
                                            .split_whitespace()
                                            .map(str::to_owned)
                                            .collect::<Vec<_>>(),
                                        Some("web".into()),
                                        Some(sent_message.clone()),
                                    );
                                }
                                if let Some(video) = sent_video {
                                    let _ = save_chat_images(
                                        video
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
                                log_api_err("API /api/chat error", &e);
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
                ("POST", "/api/cancel-chat") => {
                    #[derive(Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct CancelRequest {
                        chat_id: String,
                    }
                    if let Ok(req) = serde_json::from_str::<CancelRequest>(body) {
                        crate::cancel_agent(&req.chat_id);
                        send_json_response(socket, "200 OK", "{\"status\":\"ok\"}").await;
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"status\":\"invalid cancel body\"}",
                        )
                        .await;
                    }
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
                        video_data_uri: Option<String>,
                        document_attachment: Option<crate::chat::DocumentAttachment>,
                        agent_id: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<ApiChatRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let chat_req = ChatRequest {
                            message: req.message,
                            system_instruction: req.system_instruction.unwrap_or_default(),
                            chat_id: req.chat_id,
                            image_data_uri: req.image_data_uri,
                            audio_data_uri: req.audio_data_uri,
                            video_data_uri: req.video_data_uri,
                            document_attachment: req.document_attachment,
                            workspace_path: None,
                            agent_id: req.agent_id,
                            plan_mode: false,
                            messages: None,
                            tools: None,
                        };
                        let mut chat_req = match chat_req.with_document_context(&config) {
                            Ok(req) => req,
                            Err(error) => {
                                log_api_err("API /api/chat-stream document error", &error);
                                let err_json = serde_json::json!({
                                    "text": format!("Could not read the attached document: {error}")
                                });
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    &err_json.to_string(),
                                )
                                .await;
                                return;
                            }
                        };
                        let sent_image = chat_req.image_data_uri.clone();
                        let sent_video = chat_req.video_data_uri.clone();
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
                                       Access-Control-Allow-Headers: Content-Type, Authorization\r\n\
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
                                    let chat_id_str = chat_req.chat_id.clone().unwrap_or_default();
                                    let auth_label_clone = auth_label.clone();
                                    let join_handle = tokio::spawn(async move {
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
                                                log_api_req(
                                                    "POST",
                                                    "/api/chat-stream",
                                                    "200 OK",
                                                    Some(&format!(
                                                        "Provider: {} | {}",
                                                        config_clone.ai_provider, auth_label_clone
                                                    )),
                                                );
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
                                                log_api_err("API /api/chat-stream error", &e);
                                                let err_json = serde_json::json!({
                                                    "type": "done",
                                                    "response": {
                                                        "provider": "error",
                                                        "model": "error",
                                                        "text": format!("Error orchestrating chat: {e}")
                                                    }
                                                });
                                                let _ = tx_done.send(format!("{}\n", err_json));
                                            }
                                        }
                                    });

                                    let abort_handle = join_handle.abort_handle();
                                    if !chat_id_str.is_empty() {
                                        crate::ACTIVE_AGENTS
                                            .lock()
                                            .unwrap()
                                            .insert(chat_id_str.clone(), abort_handle);
                                    }

                                    let chat_id_str_cleanup = chat_id_str.clone();
                                    tokio::spawn(async move {
                                        let _ = join_handle.await;
                                        if !chat_id_str_cleanup.is_empty() {
                                            crate::ACTIVE_AGENTS
                                                .lock()
                                                .unwrap()
                                                .remove(&chat_id_str_cleanup);
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
                                    let chat_id_str = chat_id.clone().unwrap_or_default();
                                    let message = chat_req.message.clone();
                                    let image_data_uri = chat_req.image_data_uri.clone();
                                    let audio_data_uri = chat_req.audio_data_uri.clone();
                                    let video_data_uri = chat_req.video_data_uri.clone();
                                    let agent_id = chat_req.agent_id.clone();

                                    let join_handle = tokio::spawn(async move {
                                        let result = orchestrate_agent_loop(
                                            &config_clone,
                                            &message,
                                            &root,
                                            image_data_uri,
                                            audio_data_uri,
                                            video_data_uri,
                                            chat_id.as_deref(),
                                            agent_id.as_deref(),
                                            None,
                                            fast_mode,
                                            false,
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
                                                    tool_calls: None,
                                                    stop_reason: None,
                                                    total_tokens: None,
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
                                                let _ = tx_done.send(format!("{}\n", err_json));
                                            }
                                        }
                                    });

                                    let abort_handle = join_handle.abort_handle();
                                    if !chat_id_str.is_empty() {
                                        crate::ACTIVE_AGENTS
                                            .lock()
                                            .unwrap()
                                            .insert(chat_id_str.clone(), abort_handle);
                                    }

                                    let chat_id_str_cleanup = chat_id_str.clone();
                                    tokio::spawn(async move {
                                        let _ = join_handle.await;
                                        if !chat_id_str_cleanup.is_empty() {
                                            crate::ACTIVE_AGENTS
                                                .lock()
                                                .unwrap()
                                                .remove(&chat_id_str_cleanup);
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
                                    Some(sent_message.clone()),
                                );
                            }
                            if let Some(video) = sent_video {
                                let _ = save_chat_images(
                                    video
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
                ("POST", "/api/image-generate") => {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct ImageGenApiRequest {
                        prompt: String,
                        #[serde(default)]
                        negative_prompt: Option<String>,
                        #[serde(default)]
                        aspect_ratio: Option<String>,
                        #[serde(default)]
                        num_images: Option<u8>,
                        #[serde(default)]
                        model: Option<String>,
                        /// Which image provider to use (overrides config.image_gen_provider).
                        #[serde(default)]
                        provider: Option<String>,
                        #[serde(default)]
                        image_data_uri: Option<String>,
                        #[serde(default)]
                        mask_data_uri: Option<String>,
                        #[serde(default)]
                        mode: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<ImageGenApiRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let gen_request = ImageGenRequest {
                            prompt: req.prompt.clone(),
                            negative_prompt: req.negative_prompt,
                            aspect_ratio: req.aspect_ratio,
                            num_images: req.num_images,
                            model: req.model,
                            provider: req.provider,
                            image_data_uri: req.image_data_uri,
                            mask_data_uri: req.mask_data_uri,
                            mode: req.mode,
                        };
                        match generate_images(&config, &gen_request).await {
                            Ok(result) => {
                                log_api_req(
                                    "POST",
                                    "/api/image-generate",
                                    "200 OK",
                                    Some(&format!(
                                        "Provider: {} | {}",
                                        result.provider, auth_label
                                    )),
                                );
                                let data_uris: Vec<String> = result
                                    .images
                                    .iter()
                                    .map(|img| img.data_uri.clone())
                                    .collect();
                                let mut saved = save_chat_images(
                                    data_uris,
                                    Some(result.provider.clone()),
                                    Some(req.prompt.clone()),
                                )
                                .unwrap_or_default();
                                for picture in &mut saved {
                                    picture.url =
                                        Some(format!("/api/pictures/{}", picture.filename));
                                    picture.thumbnail_url =
                                        Some(format!("/api/pictures/{}", picture.filename));
                                }
                                let response = json!({
                                    "images": saved,
                                    "model": result.model,
                                    "provider": result.provider,
                                    "prompt": result.prompt,
                                    "description": result.description
                                });
                                send_json_response(socket, "200 OK", &response.to_string()).await;
                            }
                            Err(e) => {
                                log_api_err("API /api/image-generate error", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid image generation request body\"}",
                        )
                        .await;
                    }
                }
                ("POST", "/api/video-generate") => {
                    #[derive(serde::Deserialize)]
                    #[serde(rename_all = "camelCase")]
                    struct VideoGenApiRequest {
                        prompt: String,
                        #[serde(default)]
                        negative_prompt: Option<String>,
                        #[serde(default)]
                        aspect_ratio: Option<String>,
                        #[serde(default)]
                        duration: Option<u32>,
                        #[serde(default)]
                        model: Option<String>,
                        #[serde(default)]
                        provider: Option<String>,
                    }

                    if let Ok(req) = serde_json::from_str::<VideoGenApiRequest>(body) {
                        let config = load_config().unwrap_or_default();
                        let gen_request = VideoGenRequest {
                            prompt: req.prompt.clone(),
                            negative_prompt: req.negative_prompt,
                            aspect_ratio: req.aspect_ratio.unwrap_or_else(|| "16:9".to_string()),
                            duration: req.duration.unwrap_or(5),
                            model: req.model,
                            provider: req.provider.unwrap_or_else(|| "veo".to_string()),
                        };
                        match generate_video(&config, &gen_request).await {
                            Ok(result) => {
                                let mut response =
                                    serde_json::to_value(&result).unwrap_or(json!({}));
                                if let Some(videos) =
                                    response.get_mut("videos").and_then(|v| v.as_array_mut())
                                {
                                    for picture in videos {
                                        let filename = picture
                                            .get("filename")
                                            .and_then(|f| f.as_str())
                                            .map(|s| s.to_string());
                                        if let Some(filename) = filename {
                                            picture.as_object_mut().unwrap().insert(
                                                "url".to_string(),
                                                json!(format!("/api/pictures/{}", filename)),
                                            );
                                        }
                                        let id = picture
                                            .get("id")
                                            .and_then(|i| i.as_str())
                                            .map(|s| s.to_string());
                                        if let Some(id) = id {
                                            let has_thumb = picture.get("thumbnailPath").is_some()
                                                || picture.get("thumbnailUrl").is_some();
                                            if has_thumb {
                                                picture.as_object_mut().unwrap().insert(
                                                    "thumbnailUrl".to_string(),
                                                    json!(format!(
                                                        "/api/thumbnails/{}.thumb.png",
                                                        id
                                                    )),
                                                );
                                            }
                                        }
                                    }
                                }
                                log_api_req(
                                    "POST",
                                    "/api/video-generate",
                                    "200 OK",
                                    Some(&format!(
                                        "Provider: {} | {}",
                                        result.provider, auth_label
                                    )),
                                );
                                send_json_response(socket, "200 OK", &response.to_string()).await;
                            }
                            Err(e) => {
                                log_api_err("API /api/video-generate error", &e);
                                let err = json!({ "error": e.to_string() });
                                send_json_response(
                                    socket,
                                    "500 Internal Server Error",
                                    &err.to_string(),
                                )
                                .await;
                            }
                        }
                    } else {
                        send_json_response(
                            socket,
                            "400 Bad Request",
                            "{\"error\":\"invalid video generation request body\"}",
                        )
                        .await;
                    }
                }
                (_, route) if route.starts_with("/api/video/") && method == "POST" => {
                    // ── Video Editing Routes ────────────────────────────────────────────
                    match route {
                        "/api/video/load" => {
                            #[derive(serde::Deserialize)]
                            struct VideoLoadReq {
                                path: String,
                            }
                            if let Ok(req) = serde_json::from_str::<VideoLoadReq>(body) {
                                match video_load(&req.path) {
                                    Ok(info) => {
                                        let res = serde_json::to_string(&info).unwrap_or_default();
                                        log_api_req("POST", "/api/video/load", "200 OK", None);
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/load", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"expected {\\\"path\\\":\\\"...\\\"}\" }",
                                )
                                .await;
                            }
                        }
                        "/api/video/trim" => {
                            if let Ok(req) = serde_json::from_str::<TrimRequest>(body) {
                                match video_trim(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req("POST", "/api/video/trim", "200 OK", None);
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/trim", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid trim request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/crop" => {
                            if let Ok(req) = serde_json::from_str::<CropRequest>(body) {
                                match video_crop(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req("POST", "/api/video/crop", "200 OK", None);
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/crop", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid crop request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/resize" => {
                            if let Ok(req) = serde_json::from_str::<ResizeRequest>(body) {
                                match video_resize(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req("POST", "/api/video/resize", "200 OK", None);
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/resize", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid resize request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/merge" => {
                            if let Ok(req) = serde_json::from_str::<MergeRequest>(body) {
                                match video_merge(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req("POST", "/api/video/merge", "200 OK", None);
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/merge", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid merge request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/extract-audio" => {
                            if let Ok(req) = serde_json::from_str::<ExtractAudioRequest>(body) {
                                match video_extract_audio(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/video/extract-audio",
                                            "200 OK",
                                            None,
                                        );
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/extract-audio", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid extract-audio request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/remove-silence" => {
                            if let Ok(req) = serde_json::from_str::<RemoveSilenceRequest>(body) {
                                match video_remove_silence(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/video/remove-silence",
                                            "200 OK",
                                            None,
                                        );
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/remove-silence", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid remove-silence request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/export" => {
                            if let Ok(req) = serde_json::from_str::<ExportRequest>(body) {
                                match video_export(&req) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req("POST", "/api/video/export", "200 OK", None);
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/export", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid export request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/render-timeline" => {
                            if let Ok(req) = serde_json::from_str::<RenderTimelineRequest>(body) {
                                match render_timeline(&req.timeline) {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/video/render-timeline",
                                            "200 OK",
                                            Some(&format!("{} clips", r.clips_rendered)),
                                        );
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/render-timeline", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid timeline request\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/make-shorts" => {
                            if let Ok(req) = serde_json::from_str::<MakeShortsRequest>(body) {
                                let config = load_config().unwrap_or_default();
                                match make_shorts(&config, &req).await {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/video/make-shorts",
                                            "200 OK",
                                            Some(&format!("{} shorts clips", r.clips.len())),
                                        );
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/make-shorts", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid make-shorts request body\"}",
                                )
                                .await;
                            }
                        }
                        "/api/video/ai-edit" => {
                            if let Ok(req) = serde_json::from_str::<AiEditVideoRequest>(body) {
                                let config = load_config().unwrap_or_default();
                                match ai_edit_video(&config, &req).await {
                                    Ok(r) => {
                                        let res = serde_json::to_string(&r).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/video/ai-edit",
                                            "200 OK",
                                            Some(&format!(
                                                "AI executed prompt: {}",
                                                req.instruction
                                            )),
                                        );
                                        send_json_response(socket, "200 OK", &res).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/video/ai-edit", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid ai-edit request body\"}",
                                )
                                .await;
                            }
                        }
                        _ => {
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"error\":\"unknown video route\"}",
                            )
                            .await;
                        }
                    }
                }
                (_, route) if route.starts_with("/api/speech/") && method == "POST" => {
                    // ── Speech Routes ───────────────────────────────────────────────────
                    match route {
                        "/api/speech/transcribe" => {
                            if let Ok(req) = serde_json::from_str::<TranscribeRequest>(body) {
                                let config = load_config().unwrap_or_default();
                                match transcribe(&config, &req).await {
                                    Ok(res) => {
                                        let json_str =
                                            serde_json::to_string(&res).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/speech/transcribe",
                                            "200 OK",
                                            None,
                                        );
                                        send_json_response(socket, "200 OK", &json_str).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/speech/transcribe", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid transcribe request body\"}",
                                )
                                .await;
                            }
                        }
                        "/api/speech/detect-silence" => {
                            if let Ok(req) = serde_json::from_str::<DetectSilenceRequest>(body) {
                                match detect_silence(&req) {
                                    Ok(ranges) => {
                                        let json_str =
                                            serde_json::to_string(&ranges).unwrap_or_default();
                                        log_api_req(
                                            "POST",
                                            "/api/speech/detect-silence",
                                            "200 OK",
                                            Some(&format!("{} ranges", ranges.len())),
                                        );
                                        send_json_response(socket, "200 OK", &json_str).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/speech/detect-silence", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid detect-silence request body\"}",
                                )
                                .await;
                            }
                        }
                        _ => {
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"error\":\"unknown speech route\"}",
                            )
                            .await;
                        }
                    }
                }
                (_, route) if route.starts_with("/api/subtitle/") && method == "POST" => {
                    // ── Subtitle Routes ────────────────────────────────────────────────
                    match route {
                        "/api/subtitle/generate" => {
                            #[derive(serde::Deserialize)]
                            struct GenSubReq {
                                segments: Vec<crate::speech::TranscriptSegment>,
                            }
                            if let Ok(req) = serde_json::from_str::<GenSubReq>(body) {
                                let srt = generate_srt(&req.segments);
                                let res = json!({ "srt": srt });
                                log_api_req("POST", "/api/subtitle/generate", "200 OK", None);
                                send_json_response(socket, "200 OK", &res.to_string()).await;
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid subtitle generate request body\"}",
                                )
                                .await;
                            }
                        }
                        "/api/subtitle/translate" => {
                            if let Ok(req) = serde_json::from_str::<TranslateSubtitleRequest>(body)
                            {
                                let config = load_config().unwrap_or_default();
                                match translate_subtitles(&config, &req).await {
                                    Ok(srt) => {
                                        let res = json!({ "srt": srt });
                                        log_api_req(
                                            "POST",
                                            "/api/subtitle/translate",
                                            "200 OK",
                                            None,
                                        );
                                        send_json_response(socket, "200 OK", &res.to_string())
                                            .await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/subtitle/translate", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid subtitle translate request body\"}",
                                )
                                .await;
                            }
                        }
                        "/api/subtitle/burn" => {
                            if let Ok(req) = serde_json::from_str::<BurnSubtitleRequest>(body) {
                                match burn_subtitles(&req) {
                                    Ok(res) => {
                                        let json_str =
                                            serde_json::to_string(&res).unwrap_or_default();
                                        log_api_req("POST", "/api/subtitle/burn", "200 OK", None);
                                        send_json_response(socket, "200 OK", &json_str).await;
                                    }
                                    Err(e) => {
                                        log_api_err("/api/subtitle/burn", &e);
                                        let err = json!({ "error": e.to_string() });
                                        send_json_response(
                                            socket,
                                            "500 Internal Server Error",
                                            &err.to_string(),
                                        )
                                        .await;
                                    }
                                }
                            } else {
                                send_json_response(
                                    socket,
                                    "400 Bad Request",
                                    "{\"error\":\"invalid subtitle burn request body\"}",
                                )
                                .await;
                            }
                        }
                        _ => {
                            send_json_response(
                                socket,
                                "404 Not Found",
                                "{\"error\":\"unknown subtitle route\"}",
                            )
                            .await;
                        }
                    }
                }
                (_, "/api/video-gen/providers")
                | (_, "/api/video/providers")
                | ("GET", "/api/image-gen/providers") => {
                    let config = load_config().unwrap_or_default();
                    let mut available: Vec<String> = Vec::new();
                    if !config.api_key.trim().is_empty() {
                        available.push("nanobanana".into());
                    }
                    if !config.openai_api_key.trim().is_empty() {
                        available.push("dalle".into());
                    }
                    if !config.stability_api_key.trim().is_empty() {
                        available.push("stability".into());
                    }
                    if !config.ideogram_api_key.trim().is_empty() {
                        available.push("ideogram".into());
                    }
                    if !config.replicate_api_key.trim().is_empty() {
                        available.push("replicate".into());
                    }
                    if !config.bfl_api_key.trim().is_empty() {
                        available.push("bfl".into());
                    }
                    let active = if available.contains(&config.image_gen_provider) {
                        config.image_gen_provider.clone()
                    } else {
                        available
                            .first()
                            .cloned()
                            .unwrap_or_else(|| "nanobanana".into())
                    };
                    let response = json!({ "active": active, "available": available });
                    send_json_response(socket, "200 OK", &response.to_string()).await;
                }
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
                        send_json_response(
                            socket,
                            "401 Unauthorized",
                            "{\"error\":\"unauthorized\"}",
                        )
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
                    let ws_stream =
                        WebSocketStream::from_raw_socket(socket, Role::Server, None).await;
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
        request.audio_data_uri.clone(),
        request.video_data_uri.clone(),
        request.chat_id.as_deref(),
        request.agent_id.as_deref(),
        None,
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
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
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

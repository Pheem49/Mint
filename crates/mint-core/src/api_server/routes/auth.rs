use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query,
        body,
        request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
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
            let user = authorized_user_id(&request_str).and_then(|id| get_user(&id).ok().flatten());
            send_json_response(socket, "200 OK", &json!({ "user": user }).to_string()).await;
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
                Some(bytes) => send_binary_response(socket, "200 OK", content_type, &bytes).await,
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
                send_json_response(socket, "401 Unauthorized", "{\"message\":\"Unauthorized\"}")
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
                    send_json_response(socket, "200 OK", &json!({ "user": user }).to_string())
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
                send_json_response(socket, "401 Unauthorized", "{\"message\":\"Unauthorized\"}")
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
                    send_json_response(socket, "200 OK", &json!({ "user": user }).to_string())
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

        _ => unreachable!(
            "api_server routed an unhandled route into routes::auth::execute: {method} {route}"
        ),
    }
}

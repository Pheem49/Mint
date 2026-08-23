use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, mut socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
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
            send_json_response(socket, "500 Internal Server Error", "{\"value\":\"\"}").await;
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
            let provider = query_param(query, "provider").unwrap_or_else(|| "google".to_string());
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

        _ => unreachable!(
            "api_server routed an unhandled route into routes::profile_oauth::execute: {method} {route}"
        ),
    }
}

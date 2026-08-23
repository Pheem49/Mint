use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query: _query,
        body: _body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
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

        ("GET", "/api/gateway/health") => {
            // Unauthenticated like `/api/status` above — this is operational
            // status (which bridges are alive), not user data, and it's the
            // whole point of the endpoint: check it remotely (over an SSH
            // tunnel/Tailscale) without needing a login round-trip first.
            let config = load_config().unwrap_or_default();
            let bridges = [
                ("enableTelegramBridge", "telegram"),
                ("enableDiscordBridge", "discord"),
                ("enableSlackBridge", "slack"),
                ("enableLineBridge", "line"),
                ("enableWhatsappBridge", "whatsapp"),
                ("enableSignalBridge", "signal"),
                ("enableEmailBridge", "email"),
            ];
            let snapshot = crate::bridge_health::snapshot();
            let items: Vec<Value> = bridges
                .iter()
                .map(|&(enabled_key, name)| {
                    let enabled = config
                        .extra
                        .get(enabled_key)
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let health = snapshot.iter().find(|entry| entry.name == name);
                    json!({
                        "name": name,
                        "enabled": enabled,
                        "startedAt": health.and_then(|h| h.started_at),
                        "lastSuccessAt": health.and_then(|h| h.last_success_at),
                        "lastErrorAt": health.and_then(|h| h.last_error_at),
                        "lastError": health.and_then(|h| h.last_error.clone()),
                        "consecutiveFailures": health.map(|h| h.consecutive_failures).unwrap_or(0),
                    })
                })
                .collect();
            send_json_response(socket, "200 OK", &json!({ "bridges": items }).to_string()).await;
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

        _ => unreachable!(
            "api_server routed an unhandled route into routes::status_health::execute: {method} {route}"
        ),
    }
}

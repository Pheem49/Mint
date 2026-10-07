use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

/// `POST /api/slash` — run a slash command through the shared engine
/// (`crate::slash`). Body: `{ "input": "/cron", "cwd": "/path" }`.
pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let owner = match background_request_owner(ctx.request_str) {
        Ok(owner) => owner,
        Err(message) => {
            send_json_response(
                socket,
                "401 Unauthorized",
                &json!({"message": message}).to_string(),
            )
            .await;
            return;
        }
    };
    let RequestCtx {
        method,
        route,
        body,
        ..
    } = ctx;
    match (method, route) {
        ("POST", "/api/slash") => {
            let mut req = match serde_json::from_str::<crate::slash::SlashRequest>(body) {
                Ok(req) => req,
                Err(_) => {
                    send_json_response(
                        socket,
                        "400 Bad Request",
                        "{\"error\":\"Invalid request body.\"}",
                    )
                    .await;
                    return;
                }
            };

            req.surface = Some("web".to_string());
            // Never fall back to `MintConfig::default()` here: `execute` mutates
            // `config` in place and we `save_config` it below, so a default on a
            // transient load failure would overwrite the user's real config
            // (API keys included) with blanks.
            let mut config = match load_config() {
                Ok(config) => config,
                Err(err) => {
                    let msg =
                        json!({ "error": format!("could not load config: {err}") }).to_string();
                    send_json_response(socket, "500 Internal Server Error", &msg).await;
                    return;
                }
            };
            let trimmed = req.input.trim();
            let (token, rest) = trimmed
                .split_once(char::is_whitespace)
                .unwrap_or((trimmed, ""));
            let response = if token == "/shells" {
                let cwd = req
                    .cwd
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                match crate::bg_shell::shells_command(rest.trim(), &cwd, &owner, &config) {
                    Ok(markdown) => crate::slash::SlashResponse::Message { markdown },
                    Err(error) => crate::slash::SlashResponse::Message {
                        markdown: error.to_string(),
                    },
                }
            } else if token == "/preview" {
                let cwd = req
                    .cwd
                    .as_deref()
                    .map(std::path::PathBuf::from)
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
                match crate::system::html_preview::command(&cwd, rest, owner, &config) {
                    Ok(markdown) => crate::slash::SlashResponse::Message { markdown },
                    Err(markdown) => crate::slash::SlashResponse::Message { markdown },
                }
            } else {
                crate::slash::execute_async(&req, &mut config).await
            };
            if slash_persists_config(&response) {
                let _ = save_config(&config);
            }

            match serde_json::to_string(&response) {
                Ok(json) => send_json_response(socket, "200 OK", &json).await,
                Err(err) => {
                    let msg = json!({ "error": err.to_string() }).to_string();
                    send_json_response(socket, "500 Internal Server Error", &msg).await;
                }
            }
        }

        _ => unreachable!(
            "api_server routed an unhandled route into routes::slash::execute: {method} {route}"
        ),
    }
}

/// Whether the engine reported a config mutation that this host must persist.
fn slash_persists_config(response: &crate::slash::SlashResponse) -> bool {
    use crate::slash::{SlashEffect, SlashResponse};
    match response {
        SlashResponse::Applied { effects, .. } => effects.iter().any(|e| {
            matches!(
                e,
                SlashEffect::ConfigChanged
                    | SlashEffect::ProviderChanged { .. }
                    | SlashEffect::FastModeChanged { .. }
                    | SlashEffect::MultiAgentChanged { .. }
                    | SlashEffect::WorkspaceChanged { .. }
            )
        }),
        _ => false,
    }
}

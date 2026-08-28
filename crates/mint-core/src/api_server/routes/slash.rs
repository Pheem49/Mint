use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

/// `POST /api/slash` — run a slash command through the shared engine
/// (`crate::slash`). Body: `{ "input": "/cron", "cwd": "/path" }`.
pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
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
            let response = crate::slash::execute(&req, &mut config);
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

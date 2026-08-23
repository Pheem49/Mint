use super::RequestCtx;
use tokio::net::TcpStream;

use super::super::*;

pub(in crate::api_server) async fn execute(ctx: RequestCtx<'_>, socket: TcpStream) {
    let RequestCtx {
        method,
        route,
        query: _query,
        body,
        request_str: _request_str,
        request_bytes: _request_bytes,
        header_end: _header_end,
        auth_label: _auth_label,
    } = ctx;
    match (method, route) {
        ("POST", "/api/mcp/reauth") => {
            #[derive(Deserialize)]
            struct ReauthRequest {
                #[serde(rename = "serverName")]
                server_name: String,
            }
            match serde_json::from_str::<ReauthRequest>(body) {
                Ok(req) => {
                    let server_name = req.server_name;
                    let result =
                        tokio::task::spawn_blocking(move || crate::reauth_mcp_server(&server_name))
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
                            let err_msg = json!({ "error": format!("reauth task failed: {err}") })
                                .to_string();
                            send_json_response(socket, "500 Internal Server Error", &err_msg).await;
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

        ("POST", "/api/cron") => match serde_json::from_str::<crate::CronJobDraft>(body) {
            Ok(draft) => match crate::CronStore::open_default() {
                Ok(store) => {
                    match store.add(draft.name, draft.schedule, draft.task, draft.workspace) {
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
        },

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

        ("POST", route) if route.starts_with("/api/cron/") && route.ends_with("/enable") => {
            let id = percent_decode(
                route
                    .trim_start_matches("/api/cron/")
                    .trim_end_matches("/enable"),
            );
            match crate::CronStore::open_default().and_then(|store| store.set_enabled(&id, true)) {
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

        ("POST", route) if route.starts_with("/api/cron/") && route.ends_with("/disable") => {
            let id = percent_decode(
                route
                    .trim_start_matches("/api/cron/")
                    .trim_end_matches("/disable"),
            );
            match crate::CronStore::open_default().and_then(|store| store.set_enabled(&id, false)) {
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

        _ => unreachable!(
            "api_server routed an unhandled route into routes::cron_mcp::execute: {method} {route}"
        ),
    }
}

//! Target-pinned CDP transport. Agent/workflow sessions retain sockets so
//! session-scoped Chrome settings survive across actions. A disconnected input
//! is never resent automatically.

use crate::MintConfig;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use std::{collections::BTreeMap, future::Future, sync::Arc};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite::Message};
type Socket = WebSocketStream<MaybeTlsStream<tokio::net::TcpStream>>;
type Pool = Arc<tokio::sync::Mutex<BTreeMap<String, Socket>>>;
tokio::task_local! {static SOCKETS: Pool;}
pub(super) async fn run_transport<F: Future>(future: F) -> F::Output {
    SOCKETS
        .scope(Arc::new(tokio::sync::Mutex::new(BTreeMap::new())), future)
        .await
}

async fn exchange(
    socket: &mut Socket,
    id: u64,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    socket
        .send(Message::Text(
            json!({"id":id,"method":method,"params":params})
                .to_string()
                .into(),
        ))
        .await
        .map_err(|e| format!("browser_disconnected: {e}; input completion may be unconfirmed"))?;
    while let Some(message) = socket.next().await {
        let Message::Text(raw) = message.map_err(|e| e.to_string())? else {
            continue;
        };
        let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if value["id"] == id {
            return Ok(value);
        }
    }
    Err(
        "browser_disconnected: socket closed before response; observe before repeating input"
            .into(),
    )
}

use super::overlay::build_overlay_script;

/// Calls are pinned to the session target and bounded, including the handshake.
pub(super) async fn cdp_call(
    config: &MintConfig,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    // The overlay is cosmetic; its failure must not turn an action into a retry.
    if method != "Runtime.evaluate" {
        let _ = cdp_call_raw(
            config,
            "Runtime.evaluate",
            json!({
                "expression": build_overlay_script(), "returnByValue": false
            }),
        )
        .await;
    }
    cdp_call_raw(config, method, params).await
}

pub(super) async fn cdp_call_raw(
    config: &MintConfig,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        let pages = fetch_pages(config).await?;
        let target = super::selected_tab();
        let page = pages
            .into_iter()
            .find(|p| {
                p["type"] == "page"
                    && target
                        .as_ref()
                        .is_none_or(|id| p["id"].as_str() == Some(id.as_str()))
            })
            .ok_or_else(|| {
                if target.is_some() {
                    "tab_unavailable"
                } else {
                    "No browser page is open"
                }
                .to_string()
            })?;
        let socket_url = page["webSocketDebuggerUrl"]
            .as_str()
            .ok_or("Missing browser websocket URL")?;
        let pool = SOCKETS.try_with(Arc::clone).ok();
        let mut reused = if let Some(pool) = &pool {
            pool.lock().await.remove(socket_url)
        } else {
            None
        };
        let mut socket = if let Some(socket) = reused.take() {
            socket
        } else {
            let (mut socket, _) = connect_async(socket_url)
                .await
                .map_err(|e| format!("browser_disconnected: {e}"))?;
            if config.extra.get("__managedBrowser") == Some(&json!(true)) {
                let width = config
                    .extra
                    .get("browserViewportWidth")
                    .and_then(Value::as_u64)
                    .unwrap_or(1440)
                    .clamp(320, 7680);
                let height = config
                    .extra
                    .get("browserViewportHeight")
                    .and_then(Value::as_u64)
                    .unwrap_or(900)
                    .clamp(240, 4320);
                let reply = exchange(
                    &mut socket,
                    0,
                    "Emulation.setDeviceMetricsOverride",
                    json!({"width":width,"height":height,"deviceScaleFactor":1,"mobile":false}),
                )
                .await?;
                validate_response(&reply)?;
            }
            socket
        };
        let value = exchange(&mut socket, 1, method, params).await?;
        if let Some(pool) = pool {
            pool.lock().await.insert(socket_url.to_owned(), socket);
        }
        validate_response(&value)?;
        Ok(value)
    })
    .await
    .map_err(|_| {
        "browser_timeout: outcome may be ambiguous; observe before acting again".to_string()
    })?
}

pub(super) fn validate_response(value: &Value) -> Result<(), String> {
    if value.get("error").is_some() {
        return Err(response_error(value));
    }
    if let Some(exception) = value["result"].get("exceptionDetails") {
        return Err(format!(
            "browser_script_error: {}",
            exception["exception"]["description"]
                .as_str()
                .or_else(|| exception["text"].as_str())
                .unwrap_or("JavaScript evaluation failed")
        ));
    }
    Ok(())
}

/// Debug endpoint of the managed headless browser for this run. Mint never
/// borrows a browser on a well-known port: without a managed run the fallback
/// is an unreachable address so calls fail instead of hijacking another browser.
pub(super) fn debug_endpoint(config: &MintConfig) -> &str {
    config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:0/json/list")
}

pub(super) async fn fetch_pages(config: &MintConfig) -> Result<Vec<Value>, String> {
    fetch_pages_endpoint(debug_endpoint(config)).await
}

pub(super) async fn fetch_pages_endpoint(endpoint: &str) -> Result<Vec<Value>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1000))
        .connect_timeout(std::time::Duration::from_millis(500))
        .build()
        .unwrap_or_else(|_| crate::HTTP_CLIENT.clone());

    let value: Value = client
        .get(endpoint)
        .send()
        .await
        .map_err(|e| format!("unable to reach Chrome DevTools at {endpoint}: {e}"))?
        .error_for_status()
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;

    value
        .as_array()
        .cloned()
        .ok_or_else(|| "Chrome DevTools response was not a page list".into())
}

pub(super) fn response_error(response: &Value) -> String {
    response["error"]["message"]
        .as_str()
        .unwrap_or("Chrome DevTools returned an unexpected response")
        .to_owned()
}

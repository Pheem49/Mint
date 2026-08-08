//! Transport layer: a hand-rolled Chrome DevTools Protocol client over a raw
//! WebSocket. Every call here opens a fresh connection, sends one JSON-RPC
//! request, and reads until the matching `id` comes back — there is no
//! connection pooling or session reuse, so every higher-level action pays a
//! full websocket handshake + page-list lookup.

use crate::MintConfig;
use futures_util::{SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

use super::overlay::build_overlay_script;

/// Full CDP call — injects the aura+cursor overlay before the actual call.
pub(super) async fn cdp_call(
    config: &MintConfig,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let page = fetch_pages(config)
        .await?
        .into_iter()
        .find(|page| page["type"] == "page")
        .ok_or("Chrome DevTools did not report an open browser page")?;
    let socket_url = page["webSocketDebuggerUrl"]
        .as_str()
        .ok_or("Chrome DevTools page does not expose a websocket URL")?;
    let (mut socket, _) = connect_async(socket_url)
        .await
        .map_err(|e| format!("unable to connect to Chrome DevTools websocket: {e}"))?;

    let is_overlay = method == "Runtime.evaluate"
        && params["expression"]
            .as_str()
            .map(|s| s.contains("mint-browser-aura"))
            .unwrap_or(false);

    if !is_overlay {
        let overlay = build_overlay_script();
        let _ = socket
            .send(Message::Text(
                json!({
                    "id": 999,
                    "method": "Runtime.evaluate",
                    "params": { "expression": overlay, "returnByValue": false }
                })
                .to_string()
                .into(),
            ))
            .await;
    }

    socket
        .send(Message::Text(
            json!({ "id": 1, "method": method, "params": params })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(message) = socket.next().await {
        let message = message.map_err(|e| e.to_string())?;
        let Message::Text(raw) = message else {
            continue;
        };
        let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if value["id"] == 1 {
            return Ok(value);
        }
    }
    Err("Chrome DevTools websocket closed before returning a response".into())
}

/// Lightweight CDP call — no overlay injection. Used for Input.* and Page.captureScreenshot.
pub(super) async fn cdp_call_raw(
    config: &MintConfig,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    let page = fetch_pages(config)
        .await?
        .into_iter()
        .find(|page| page["type"] == "page")
        .ok_or("Chrome DevTools did not report an open browser page")?;
    let socket_url = page["webSocketDebuggerUrl"]
        .as_str()
        .ok_or("Chrome DevTools page does not expose a websocket URL")?;
    let (mut socket, _) = connect_async(socket_url)
        .await
        .map_err(|e| format!("unable to connect to Chrome DevTools websocket: {e}"))?;

    socket
        .send(Message::Text(
            json!({ "id": 1, "method": method, "params": params })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|e| e.to_string())?;

    while let Some(message) = socket.next().await {
        let message = message.map_err(|e| e.to_string())?;
        let Message::Text(raw) = message else {
            continue;
        };
        let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
        if value["id"] == 1 {
            return Ok(value);
        }
    }
    Err("Chrome DevTools websocket closed before returning a response".into())
}

pub(super) async fn fetch_pages(config: &MintConfig) -> Result<Vec<Value>, String> {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");
    fetch_pages_endpoint(endpoint).await
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

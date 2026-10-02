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
        let (mut socket, _) = connect_async(socket_url)
            .await
            .map_err(|e| format!("browser_disconnected: {e}"))?;
        socket
            .send(Message::Text(
                json!({ "id": 1, "method": method, "params": params })
                    .to_string()
                    .into(),
            ))
            .await
            .map_err(|e| e.to_string())?;
        while let Some(message) = socket.next().await {
            let Message::Text(raw) = message.map_err(|e| e.to_string())? else {
                continue;
            };
            let value: Value = serde_json::from_str(&raw).map_err(|e| e.to_string())?;
            if value["id"] == 1 {
                validate_response(&value)?;
                return Ok(value);
            }
        }
        Err("browser_disconnected: websocket closed before response".into())
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

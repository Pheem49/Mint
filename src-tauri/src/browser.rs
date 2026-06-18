use futures_util::{SinkExt, StreamExt};
use mint_core::MintConfig;
use serde::Serialize;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
}

pub async fn list_tabs(config: &MintConfig) -> Result<Vec<BrowserTab>, String> {
    Ok(fetch_pages(config)
        .await?
        .into_iter()
        .filter_map(|page| {
            Some(BrowserTab {
                id: page["id"].as_str()?.to_owned(),
                title: page["title"].as_str().unwrap_or_default().to_owned(),
                url: page["url"].as_str().unwrap_or_default().to_owned(),
            })
        })
        .collect())
}

pub async fn navigate(config: &MintConfig, url: &str) -> Result<String, String> {
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("browser navigation only supports http and https URLs".into());
    }
    let response = cdp_call(config, "Page.navigate", json!({ "url": url })).await?;
    response["result"]["frameId"]
        .as_str()
        .map(|_| format!("navigating to {url}"))
        .ok_or_else(|| response_error(&response))
}

pub async fn read_page_text(config: &MintConfig) -> Result<String, String> {
    let response = cdp_call(
        config,
        "Runtime.evaluate",
        json!({
            "expression": "document.body ? document.body.innerText.substring(0, 12000) : ''",
            "returnByValue": true
        }),
    )
    .await?;
    response["result"]["result"]["value"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| response_error(&response))
}

pub async fn click(config: &MintConfig, selector: &str) -> Result<String, String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > 500 {
        return Err("browser selector must contain between 1 and 500 characters".into());
    }
    let selector = serde_json::to_string(selector).map_err(|error| error.to_string())?;
    let response = cdp_call(
        config,
        "Runtime.evaluate",
        json!({
            "expression": format!(
                "(() => {{ const element = document.querySelector({selector}); if (!element) return 'not-found'; element.click(); return 'clicked'; }})()"
            ),
            "returnByValue": true
        }),
    )
    .await?;
    match response["result"]["result"]["value"].as_str() {
        Some("clicked") => Ok("clicked".into()),
        Some("not-found") => Err(format!("browser selector not found: {selector}")),
        _ => Err(response_error(&response)),
    }
}

async fn cdp_call(config: &MintConfig, method: &str, params: Value) -> Result<Value, String> {
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
        .map_err(|error| format!("unable to connect to Chrome DevTools websocket: {error}"))?;
    socket
        .send(Message::Text(
            json!({ "id": 1, "method": method, "params": params })
                .to_string()
                .into(),
        ))
        .await
        .map_err(|error| error.to_string())?;
    while let Some(message) = socket.next().await {
        let message = message.map_err(|error| error.to_string())?;
        let Message::Text(raw) = message else {
            continue;
        };
        let value: Value = serde_json::from_str(&raw).map_err(|error| error.to_string())?;
        if value["id"] == 1 {
            return Ok(value);
        }
    }
    Err("Chrome DevTools websocket closed before returning a response".into())
}

async fn fetch_pages(config: &MintConfig) -> Result<Vec<Value>, String> {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");
    let value: Value = mint_core::HTTP_CLIENT
        .clone()
        .get(endpoint)
        .send()
        .await
        .map_err(|error| format!("unable to reach Chrome DevTools at {endpoint}: {error}"))?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    value
        .as_array()
        .cloned()
        .ok_or_else(|| "Chrome DevTools response was not a page list".into())
}

fn response_error(response: &Value) -> String {
    response["error"]["message"]
        .as_str()
        .unwrap_or("Chrome DevTools returned an unexpected response")
        .to_owned()
}

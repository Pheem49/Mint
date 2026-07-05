use crate::MintConfig;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

fn log_action(action: &str, details: &str) {
    if let Some(config_dir) = dirs::config_dir() {
        let log_dir = config_dir.join("mint");
        let log_file = log_dir.join("browser-automation.log");
        let _ = std::fs::create_dir_all(&log_dir);
        if let Ok(mut file) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            use std::io::Write;
            let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
            let _ = writeln!(file, "[{}] [{}] {}", now, action, details);
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
}

pub async fn list_tabs(config: &MintConfig) -> Result<Vec<BrowserTab>, String> {
    if !is_browser_running(config).await {
        return Err("Browser automation is not running. Please run 'mint auto' first.".to_string());
    }
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
        log_action(
            "NAVIGATE_ERROR",
            "Browser navigation only supports http and https URLs",
        );
        return Err("browser navigation only supports http and https URLs".into());
    }
    log_action("NAVIGATE", &format!("Navigating to {url}"));
    ensure_page_open(config).await?;
    match cdp_call(config, "Page.navigate", json!({ "url": url })).await {
        Ok(response) => {
            if response["result"]["frameId"].as_str().is_some() {
                log_action(
                    "NAVIGATE_SUCCESS",
                    &format!("Successfully navigated to {url}"),
                );
                Ok(format!("navigating to {url}"))
            } else {
                let err = response_error(&response);
                log_action("NAVIGATE_ERROR", &format!("Failed: {err}"));
                Err(err)
            }
        }
        Err(e) => {
            log_action("NAVIGATE_ERROR", &format!("Websocket error: {e}"));
            Err(e)
        }
    }
}

pub async fn read_page_text(config: &MintConfig) -> Result<String, String> {
    log_action("READ", "Reading page text content");
    ensure_page_open(config).await?;
    match cdp_call(
        config,
        "Runtime.evaluate",
        json!({
            "expression": "document.body ? document.body.innerText.substring(0, 12000) : ''",
            "returnByValue": true
        }),
    )
    .await
    {
        Ok(response) => {
            if let Some(val) = response["result"]["result"]["value"].as_str() {
                log_action(
                    "READ_SUCCESS",
                    &format!("Successfully read {} characters", val.len()),
                );
                Ok(val.to_owned())
            } else {
                let err = response_error(&response);
                log_action("READ_ERROR", &format!("Failed: {err}"));
                Err(err)
            }
        }
        Err(e) => {
            log_action("READ_ERROR", &format!("Websocket error: {e}"));
            Err(e)
        }
    }
}

pub async fn click(config: &MintConfig, selector: &str) -> Result<String, String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > 500 {
        log_action(
            "CLICK_ERROR",
            "Browser selector must contain between 1 and 500 characters",
        );
        return Err("browser selector must contain between 1 and 500 characters".into());
    }
    log_action("CLICK", &format!("Clicking element '{selector}'"));
    ensure_page_open(config).await?;
    let selector_json = serde_json::to_string(selector).map_err(|error| error.to_string())?;
    match cdp_call(
        config,
        "Runtime.evaluate",
        json!({
            "expression": format!(
                "(() => {{ const element = document.querySelector({selector_json}); if (!element) return 'not-found'; element.click(); return 'clicked'; }})()"
            ),
            "returnByValue": true
        }),
    )
    .await {
        Ok(response) => {
            match response["result"]["result"]["value"].as_str() {
                Some("clicked") => {
                    log_action("CLICK_SUCCESS", &format!("Successfully clicked element '{selector}'"));
                    Ok("clicked".into())
                }
                Some("not-found") => {
                    let err = format!("browser selector not found: {selector}");
                    log_action("CLICK_ERROR", &err);
                    Err(err)
                }
                _ => {
                    let err = response_error(&response);
                    log_action("CLICK_ERROR", &err);
                    Err(err)
                }
            }
        }
        Err(e) => {
            log_action("CLICK_ERROR", &format!("Websocket error: {e}"));
            Err(e)
        }
    }
}

pub async fn type_text(config: &MintConfig, selector: &str, text: &str) -> Result<String, String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > 500 {
        log_action(
            "TYPE_ERROR",
            "Browser selector must contain between 1 and 500 characters",
        );
        return Err("browser selector must contain between 1 and 500 characters".into());
    }
    log_action("TYPE", &format!("Typing text into element '{selector}'"));
    ensure_page_open(config).await?;
    let selector_json = serde_json::to_string(selector).map_err(|error| error.to_string())?;
    let text_json = serde_json::to_string(text).map_err(|error| error.to_string())?;
    match cdp_call(
        config,
        "Runtime.evaluate",
        json!({
            "expression": format!(
                "(() => {{ \
                    const element = document.querySelector({selector_json}); \
                    if (!element) return 'not-found'; \
                    element.value = {text_json}; \
                    element.dispatchEvent(new Event('input', {{ bubbles: true }})); \
                    element.dispatchEvent(new Event('change', {{ bubbles: true }})); \
                    return 'typed'; \
                }})()"
            ),
            "returnByValue": true
        }),
    )
    .await
    {
        Ok(response) => match response["result"]["result"]["value"].as_str() {
            Some("typed") => {
                log_action(
                    "TYPE_SUCCESS",
                    &format!("Successfully typed into element '{selector}'"),
                );
                Ok("typed".into())
            }
            Some("not-found") => {
                let err = format!("browser selector not found: {selector}");
                log_action("TYPE_ERROR", &err);
                Err(err)
            }
            _ => {
                let err = response_error(&response);
                log_action("TYPE_ERROR", &err);
                Err(err)
            }
        },
        Err(e) => {
            log_action("TYPE_ERROR", &format!("Websocket error: {e}"));
            Err(e)
        }
    }
}

pub async fn is_browser_running(config: &MintConfig) -> bool {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");
    fetch_pages_endpoint(endpoint).await.is_ok()
}

pub async fn spawn_automation_browser(config: &MintConfig) -> Result<(), String> {
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");

    if fetch_pages_endpoint(endpoint).await.is_ok() {
        return Ok(());
    }

    let browser_name = config
        .extra
        .get("automationBrowser")
        .and_then(Value::as_str)
        .unwrap_or("chromium");

    let profile_dir = dirs::config_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("mint")
        .join("automation-profile");
    let profile_arg = format!("--user-data-dir={}", profile_dir.to_string_lossy());

    let args = [
        "--remote-debugging-port=9222".to_owned(),
        "--no-first-run".to_owned(),
        "--no-default-browser-check".to_owned(),
        profile_arg,
    ];

    let mut spawned = false;
    let executables = if cfg!(target_os = "windows") {
        vec!["chrome.exe", "chromium.exe", "msedge.exe"]
    } else if cfg!(target_os = "macos") {
        vec![
            "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
            "/Applications/Chromium.app/Contents/MacOS/Chromium",
            "chromium",
            "google-chrome",
        ]
    } else {
        vec![
            "chromium",
            "google-chrome-stable",
            "google-chrome",
            "chrome",
            "chromium-browser",
        ]
    };

    for exe in executables {
        if std::process::Command::new(exe)
            .args(&args)
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            spawned = true;
            break;
        }
    }

    if !spawned {
        return Err(format!(
            "Could not find or spawn browser '{browser_name}' with remote debugging. Please verify it is installed."
        ));
    }

    for _ in 0..20 {
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
        if fetch_pages_endpoint(endpoint).await.is_ok() {
            return Ok(());
        }
    }

    Err("Browser spawned but remote debugging port 9222 did not become available.".to_string())
}

pub async fn ensure_page_open(config: &MintConfig) -> Result<(), String> {
    if !is_browser_running(config).await {
        return Err("Browser automation is not running. Please run 'mint auto' first.".to_string());
    }
    let endpoint = config
        .extra
        .get("browserDebugUrl")
        .and_then(Value::as_str)
        .unwrap_or("http://127.0.0.1:9222/json/list");

    let pages = fetch_pages_endpoint(endpoint).await?;
    let has_page = pages.iter().any(|p| p["type"] == "page");
    if !has_page {
        let base_url = endpoint.replace("/json/list", "/json/new");
        let client = crate::HTTP_CLIENT.clone();
        let _ = client
            .put(&base_url)
            .send()
            .await
            .map_err(|e| format!("failed to open new tab: {e}"))?;
    }
    Ok(())
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
    fetch_pages_endpoint(endpoint).await
}

async fn fetch_pages_endpoint(endpoint: &str) -> Result<Vec<Value>, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_millis(1000))
        .connect_timeout(std::time::Duration::from_millis(500))
        .build()
        .unwrap_or_else(|_| crate::HTTP_CLIENT.clone());

    let value: Value = client
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

use crate::MintConfig;
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

// ---------------------------------------------------------------------------
// Logging
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserTab {
    pub id: String,
    pub title: String,
    pub url: String,
}

// ---------------------------------------------------------------------------
// Public API — navigation / reading
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Public API — click / type (selector-based, with native CDP events)
// ---------------------------------------------------------------------------

/// Click a CSS selector element using native CDP mouse events.
/// Supports:
///   - Standard CSS selectors: `button.submit`, `#id`, `[attr=val]`
///   - Text match: `text=Login`, `contains=Submit`
///   - XPath: `xpath=//button[@type='submit']`
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

    // Ensure overlay exists before doing any visual interaction
    inject_overlay(config).await;

    // Build a JS expression that finds the element using selector or text/xpath
    let find_expr = selector_to_js_find(selector);

    let expression = format!(
        r#"(() => {{
            {find_expr}
            if (!el) return null;
            const r = el.getBoundingClientRect();
            return JSON.stringify({{ x: r.left + r.width / 2, y: r.top + r.height / 2 }});
        }})()"#
    );

    let coord_result = cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({ "expression": expression, "returnByValue": true }),
    )
    .await;

    if let Ok(response) = coord_result {
        let val = &response["result"]["result"];
        if let Some(val_str) = val["value"].as_str() {
            if let Ok(parsed) = serde_json::from_str::<Value>(val_str) {
                if let (Some(x), Some(y)) = (parsed["x"].as_f64(), parsed["y"].as_f64()) {
                    log_action(
                        "CLICK",
                        &format!("Coordinates ({x:.0},{y:.0}) for '{selector}'"),
                    );
                    let result = mouse_click(config, x, y, "left").await?;
                    log_action(
                        "CLICK_SUCCESS",
                        &format!("Clicked '{selector}' at ({x:.0},{y:.0})"),
                    );
                    return Ok(result);
                }
            }
        }
    }

    // Fallback: JS .click() directly
    let click_expr = format!(
        r#"(() => {{
            {find_expr}
            if (!el) return 'not-found';
            el.scrollIntoView({{behavior:'instant',block:'center'}});
            el.click();
            return 'clicked';
        }})()"#
    );
    match cdp_call(
        config,
        "Runtime.evaluate",
        json!({ "expression": click_expr, "returnByValue": true }),
    )
    .await
    {
        Ok(response) => match response["result"]["result"]["value"].as_str() {
            Some("clicked") => {
                log_action(
                    "CLICK_SUCCESS",
                    &format!("JS-fallback clicked '{selector}'"),
                );
                Ok("clicked".into())
            }
            Some("not-found") => {
                let err = format!("element not found for selector: {selector}");
                log_action("CLICK_ERROR", &err);
                Err(err)
            }
            _ => {
                let err = response_error(&response);
                log_action("CLICK_ERROR", &err);
                Err(err)
            }
        },
        Err(e) => {
            log_action("CLICK_ERROR", &format!("Websocket error: {e}"));
            Err(e)
        }
    }
}

/// Type text into a CSS selector element using native CDP keyboard events.
/// Focuses the element first (via click), then sends Input.insertText.
pub async fn type_text(config: &MintConfig, selector: &str, text: &str) -> Result<String, String> {
    let selector = selector.trim();
    if selector.is_empty() || selector.len() > 500 {
        log_action(
            "TYPE_ERROR",
            "Browser selector must contain between 1 and 500 characters",
        );
        return Err("browser selector must contain between 1 and 500 characters".into());
    }
    log_action("TYPE", &format!("Typing into '{selector}'"));
    ensure_page_open(config).await?;

    // Focus the element by clicking it
    let _ = click(config, selector).await;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Native keyboard input
    type_text_native(config, text).await
}

// ---------------------------------------------------------------------------
// Public API — native mouse control
// ---------------------------------------------------------------------------

/// Move the real browser mouse cursor to (x, y) via CDP Input.dispatchMouseEvent.
/// Also ensures overlay exists and updates cursor position.
pub async fn mouse_move(config: &MintConfig, x: f64, y: f64) -> Result<String, String> {
    log_action("MOUSE_MOVE", &format!("Moving mouse to ({x:.0},{y:.0})"));
    ensure_page_open(config).await?;

    // Ensure overlay exists
    inject_overlay(config).await;

    match cdp_call_raw(
        config,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseMoved",
            "x": x,
            "y": y,
            "button": "none",
            "buttons": 0,
            "clickCount": 0,
            "modifiers": 0
        }),
    )
    .await
    {
        Ok(_) => {
            // Update the visual cursor overlay position
            let cursor_script = format!(
                r#"(() => {{
                    const c = document.getElementById('mint-cursor-overlay');
                    if (c) {{ c.style.left = '{x}px'; c.style.top = '{y}px'; }}
                }})()"#
            );
            let _ = cdp_call_raw(
                config,
                "Runtime.evaluate",
                json!({ "expression": cursor_script, "returnByValue": false }),
            )
            .await;
            log_action("MOUSE_MOVE_SUCCESS", &format!("Moved to ({x:.0},{y:.0})"));
            Ok(format!("mouse moved to ({x:.0},{y:.0})"))
        }
        Err(e) => {
            log_action("MOUSE_MOVE_ERROR", &format!("Failed: {e}"));
            Err(e)
        }
    }
}

/// Click at absolute screen coordinates (x, y) using native CDP mouse events.
/// Dispatches mouseMoved → mousePressed → mouseReleased with animation feedback.
pub async fn mouse_click(
    config: &MintConfig,
    x: f64,
    y: f64,
    button: &str,
) -> Result<String, String> {
    log_action(
        "MOUSE_CLICK",
        &format!("Clicking ({x:.0},{y:.0}) btn={button}"),
    );
    ensure_page_open(config).await?;

    let button_str = match button {
        "right" => "right",
        "middle" => "middle",
        _ => "left",
    };

    // Move cursor to target first
    let _ = mouse_move(config, x, y).await;
    tokio::time::sleep(std::time::Duration::from_millis(60)).await;

    // Cursor click animation
    let anim = r#"(() => {
        const c = document.getElementById('mint-cursor-overlay');
        if (c) {
            c.style.transition = 'transform 0.08s ease, opacity 0.08s ease';
            c.style.transform = 'scale(0.82)';
            c.style.opacity = '0.65';
            setTimeout(() => { c.style.transform = 'scale(1)'; c.style.opacity = '1'; }, 120);
        }
    })()"#;
    let _ = cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({ "expression": anim, "returnByValue": false }),
    )
    .await;

    // mousePressed
    cdp_call_raw(
        config,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mousePressed",
            "x": x, "y": y,
            "button": button_str,
            "buttons": 1,
            "clickCount": 1,
            "modifiers": 0
        }),
    )
    .await
    .map_err(|e| {
        log_action("MOUSE_CLICK_ERROR", &format!("mousePressed: {e}"));
        e
    })?;

    tokio::time::sleep(std::time::Duration::from_millis(50)).await;

    // mouseReleased
    cdp_call_raw(
        config,
        "Input.dispatchMouseEvent",
        json!({
            "type": "mouseReleased",
            "x": x, "y": y,
            "button": button_str,
            "buttons": 0,
            "clickCount": 1,
            "modifiers": 0
        }),
    )
    .await
    .map_err(|e| {
        log_action("MOUSE_CLICK_ERROR", &format!("mouseReleased: {e}"));
        e
    })?;

    log_action(
        "MOUSE_CLICK_SUCCESS",
        &format!("Clicked ({x:.0},{y:.0}) {button_str}"),
    );
    Ok(format!(
        "clicked at ({x:.0},{y:.0}) with {button_str} button"
    ))
}

// ---------------------------------------------------------------------------
// Public API — native keyboard control
// ---------------------------------------------------------------------------

/// Type text using CDP Input.insertText (native, supports any Unicode text).
pub async fn type_text_native(config: &MintConfig, text: &str) -> Result<String, String> {
    log_action(
        "TYPE",
        &format!("Native typing: '{}'", &text[..text.len().min(60)]),
    );
    ensure_page_open(config).await?;

    match cdp_call_raw(config, "Input.insertText", json!({ "text": text })).await {
        Ok(_) => {
            log_action(
                "TYPE_SUCCESS",
                &format!("Typed {} characters natively", text.len()),
            );
            Ok(format!("typed {} characters", text.len()))
        }
        Err(e) => {
            log_action("TYPE_ERROR", &format!("Native type failed: {e}"));
            Err(e)
        }
    }
}

/// Press a single key using CDP Input.dispatchKeyEvent.
///
/// Supported special keys: Enter, Tab, Escape, Backspace, Delete,
/// ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Home, End,
/// PageUp, PageDown, Space, F1–F12.
/// Also accepts single printable characters.
pub async fn key_press(config: &MintConfig, key: &str) -> Result<String, String> {
    log_action("KEY_PRESS", &format!("Pressing key '{key}'"));
    ensure_page_open(config).await?;

    let (key_code, code) = key_to_cdp_params(key);

    // keyDown
    cdp_call_raw(
        config,
        "Input.dispatchKeyEvent",
        json!({
            "type": "keyDown",
            "key": key,
            "code": code,
            "windowsVirtualKeyCode": key_code,
            "nativeVirtualKeyCode": key_code,
            "modifiers": 0,
            "isSystemKey": false,
            "location": 0
        }),
    )
    .await
    .map_err(|e| {
        log_action("KEY_PRESS_ERROR", &format!("keyDown failed: {e}"));
        e
    })?;

    // char event for printable single characters
    if key.chars().count() == 1 && !key.chars().next().map(|c| c.is_control()).unwrap_or(true) {
        let _ = cdp_call_raw(
            config,
            "Input.dispatchKeyEvent",
            json!({
                "type": "char",
                "key": key,
                "code": code,
                "windowsVirtualKeyCode": key_code,
                "modifiers": 0
            }),
        )
        .await;
    }

    tokio::time::sleep(std::time::Duration::from_millis(30)).await;

    // keyUp
    cdp_call_raw(
        config,
        "Input.dispatchKeyEvent",
        json!({
            "type": "keyUp",
            "key": key,
            "code": code,
            "windowsVirtualKeyCode": key_code,
            "nativeVirtualKeyCode": key_code,
            "modifiers": 0,
            "isSystemKey": false,
            "location": 0
        }),
    )
    .await
    .map_err(|e| {
        log_action("KEY_PRESS_ERROR", &format!("keyUp failed: {e}"));
        e
    })?;

    log_action("KEY_PRESS_SUCCESS", &format!("Pressed '{key}'"));
    Ok(format!("pressed key '{key}'"))
}

// ---------------------------------------------------------------------------
// Public API — screenshot
// ---------------------------------------------------------------------------

/// Capture a PNG screenshot of the current browser page.
/// Returns base64-encoded PNG string.
pub async fn screenshot(config: &MintConfig) -> Result<String, String> {
    log_action("SCREENSHOT", "Capturing screenshot");
    ensure_page_open(config).await?;

    match cdp_call_raw(
        config,
        "Page.captureScreenshot",
        json!({
            "format": "png",
            "fromSurface": true,
            "captureBeyondViewport": false
        }),
    )
    .await
    {
        Ok(response) => {
            if let Some(data) = response["result"]["data"].as_str() {
                log_action(
                    "SCREENSHOT_SUCCESS",
                    &format!("Captured {} base64 bytes", data.len()),
                );
                Ok(data.to_owned())
            } else {
                let err = response_error(&response);
                log_action("SCREENSHOT_ERROR", &format!("Failed: {err}"));
                Err(err)
            }
        }
        Err(e) => {
            log_action("SCREENSHOT_ERROR", &format!("Websocket error: {e}"));
            Err(e)
        }
    }
}

// ---------------------------------------------------------------------------
// Public API — element coordinates
// ---------------------------------------------------------------------------

/// Get the viewport-relative center (x, y) of an element by CSS selector.
/// Supports text=, contains=, xpath= prefixes in addition to CSS selectors.
pub async fn get_element_coordinates(
    config: &MintConfig,
    selector: &str,
) -> Result<(f64, f64), String> {
    let find_expr = selector_to_js_find(selector);
    let expression = format!(
        r#"(() => {{
            {find_expr}
            if (!el) return null;
            const r = el.getBoundingClientRect();
            return JSON.stringify({{ x: r.left + r.width / 2, y: r.top + r.height / 2 }});
        }})()"#
    );
    match cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({ "expression": expression, "returnByValue": true }),
    )
    .await
    {
        Ok(response) => {
            let val = &response["result"]["result"];
            if val["type"] == "null" || val["value"].is_null() {
                return Err(format!("element not found: {selector}"));
            }
            if let Some(val_str) = val["value"].as_str() {
                let parsed: Value = serde_json::from_str(val_str).map_err(|e| e.to_string())?;
                let x = parsed["x"].as_f64().ok_or("missing x")?;
                let y = parsed["y"].as_f64().ok_or("missing y")?;
                Ok((x, y))
            } else {
                Err(response_error(&response))
            }
        }
        Err(e) => Err(e),
    }
}

/// Convert a selector (CSS / text= / contains= / xpath=) into a JS let statement:
/// `let el = <expression>;`
fn selector_to_js_find(selector: &str) -> String {
    if let Some(text) = selector.strip_prefix("text=") {
        // Exact text match (trimmed)
        let escaped = text.replace('\\', "\\\\").replace('`', "\\`");
        format!(
            "const el = Array.from(document.querySelectorAll('*')).find(\
             e => e.childElementCount === 0 && e.textContent.trim() === `{escaped}`) \
             || Array.from(document.querySelectorAll('*')).find(\
             e => e.textContent.trim() === `{escaped}`);"
        )
    } else if let Some(text) = selector.strip_prefix("contains=") {
        // Partial text match
        let escaped = text.replace('\\', "\\\\").replace('`', "\\`");
        format!(
            "const el = Array.from(document.querySelectorAll('*')).find(\
             e => e.childElementCount === 0 && e.textContent.includes(`{escaped}`)) \
             || Array.from(document.querySelectorAll('*')).find(\
             e => e.textContent.includes(`{escaped}`));"
        )
    } else if let Some(xpath) = selector.strip_prefix("xpath=") {
        let escaped = xpath.replace('\\', "\\\\").replace('"', "\\\"");
        format!(
            "const el = document.evaluate(`{escaped}`, document, null, \
             XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;"
        )
    } else {
        // Standard CSS selector — wrap in try/catch so invalid selectors don't throw
        let escaped = selector.replace('\\', "\\\\").replace('`', "\\`");
        format!(
            "let el; try {{ el = document.querySelector(`{escaped}`); }} \
             catch(e) {{ \
               el = Array.from(document.querySelectorAll('*')).find(\
                 e => e.textContent.trim() === `{escaped}`); \
             }}"
        )
    }
}

/// Inject the green aura + animated cursor overlay into the current page.
/// Best-effort: errors are silently ignored.
async fn inject_overlay(config: &MintConfig) {
    let script = build_overlay_script();
    let _ = cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({ "expression": script, "returnByValue": false }),
    )
    .await;
}

// ---------------------------------------------------------------------------
// Browser lifecycle
// ---------------------------------------------------------------------------

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
        .unwrap_or_else(std::env::temp_dir)
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
    let executables: Vec<&str> = if cfg!(target_os = "windows") {
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
            "Could not find or spawn browser '{browser_name}' with remote debugging. \
             Please verify it is installed."
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

// ---------------------------------------------------------------------------
// Overlay injection script
// ---------------------------------------------------------------------------

fn build_overlay_script() -> &'static str {
    r##"
    (() => {
        // ── Green aura border ────────────────────────────────────────────────
        if (!document.getElementById('mint-browser-aura')) {
            const style = document.createElement('style');
            style.id = 'mint-browser-aura-style';
            style.textContent = `
                @keyframes mint-pulse {
                    0%   { box-shadow: inset 0 0 15px rgba(16,185,129,0.4); border-color: rgba(16,185,129,0.6); }
                    100% { box-shadow: inset 0 0 30px rgba(16,185,129,0.8); border-color: rgba(16,185,129,1);   }
                }
            `;
            document.head.appendChild(style);

            const aura = document.createElement('div');
            aura.id = 'mint-browser-aura';
            aura.style.cssText = `
                position:fixed; top:0; left:0; width:100vw; height:100vh;
                border:5px solid rgba(16,185,129,0.6);
                pointer-events:none; z-index:2147483646;
                box-sizing:border-box;
                animation:mint-pulse 1.5s infinite alternate;
            `;
            document.body.appendChild(aura);
        }

        // ── Animated mouse cursor overlay ─────────────────────────────────────
        if (!document.getElementById('mint-cursor-overlay')) {
            const cursor = document.createElement('div');
            cursor.id = 'mint-cursor-overlay';
            cursor.innerHTML = `<svg xmlns="http://www.w3.org/2000/svg" width="28" height="28" viewBox="0 0 28 28">
                <defs>
                    <filter id="mint-cs" x="-30%" y="-30%" width="160%" height="160%">
                        <feDropShadow dx="1" dy="2" stdDeviation="2.5" flood-color="rgba(0,0,0,0.5)"/>
                    </filter>
                </defs>
                <path d="M5 2 L5 22 L9.5 17 L13.5 24.5 L16 23 L12 15.5 L19 15.5 Z"
                      fill="white" stroke="#10b981" stroke-width="1.6"
                      stroke-linejoin="round" filter="url(#mint-cs)"/>
            </svg>`;
            cursor.style.cssText = `
                position:fixed; left:0; top:0; pointer-events:none;
                z-index:2147483647;
                transition:left 0.06s ease-out, top 0.06s ease-out, transform 0.08s ease, opacity 0.08s ease;
                transform-origin:5px 2px;
            `;
            document.body.appendChild(cursor);
        }
    })()
    "##
}

// ---------------------------------------------------------------------------
// CDP helpers
// ---------------------------------------------------------------------------

/// Full CDP call — injects the aura+cursor overlay before the actual call.
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
async fn cdp_call_raw(config: &MintConfig, method: &str, params: Value) -> Result<Value, String> {
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

fn response_error(response: &Value) -> String {
    response["error"]["message"]
        .as_str()
        .unwrap_or("Chrome DevTools returned an unexpected response")
        .to_owned()
}

// ---------------------------------------------------------------------------
// Key-to-CDP mapping
// ---------------------------------------------------------------------------

/// Returns (windowsVirtualKeyCode, code string) for a key name.
fn key_to_cdp_params(key: &str) -> (i32, &'static str) {
    match key {
        "Enter" | "\n" | "\r" => (13, "Enter"),
        "Tab" | "\t" => (9, "Tab"),
        "Escape" | "Esc" => (27, "Escape"),
        "Backspace" => (8, "Backspace"),
        "Delete" | "Del" => (46, "Delete"),
        "ArrowUp" | "Up" => (38, "ArrowUp"),
        "ArrowDown" | "Down" => (40, "ArrowDown"),
        "ArrowLeft" | "Left" => (37, "ArrowLeft"),
        "ArrowRight" | "Right" => (39, "ArrowRight"),
        "Home" => (36, "Home"),
        "End" => (35, "End"),
        "PageUp" => (33, "PageUp"),
        "PageDown" => (34, "PageDown"),
        "Space" | " " => (32, "Space"),
        "F1" => (112, "F1"),
        "F2" => (113, "F2"),
        "F3" => (114, "F3"),
        "F4" => (115, "F4"),
        "F5" => (116, "F5"),
        "F6" => (117, "F6"),
        "F7" => (118, "F7"),
        "F8" => (119, "F8"),
        "F9" => (120, "F9"),
        "F10" => (121, "F10"),
        "F11" => (122, "F11"),
        "F12" => (123, "F12"),
        _ => {
            let code = key.chars().next().map(|c| c as i32).unwrap_or(0);
            (code, "Unidentified")
        }
    }
}

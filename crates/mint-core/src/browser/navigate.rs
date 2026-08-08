//! Navigating the automated page and reading its state back (tab list, page
//! text, screenshots). Not selector-driven interaction — see `interact`.

use crate::MintConfig;
use serde_json::json;

use super::BrowserTab;
use super::cdp::{cdp_call, cdp_call_raw, fetch_pages, response_error};
use super::lifecycle::{ensure_page_open, is_browser_running};
use super::logging::log_action;

pub async fn list_tabs(config: &MintConfig) -> Result<Vec<BrowserTab>, String> {
    if !is_browser_running(config).await {
        return Err("Browser automation is not running. Please run 'mint auto' first.".to_string());
    }
    Ok(fetch_pages(config)
        .await?
        .into_iter()
        // Chrome's `/json/list` also reports internal UI surfaces (the
        // omnibox popup, etc.) as `browser_ui` entries alongside real tabs —
        // without this filter they show up as noise (e.g. "Omnibox Popup —
        // chrome://omnibox-popup.top-chrome/") in any tab switcher built on
        // top of this, and `.first()`-style consumers can grab one of them
        // instead of the actual page. `ensure_page_open`/`cdp_call` already
        // filter the same way when picking which page to act on.
        .filter(|page| page["type"] == "page")
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
                wait_for_page_load(config).await;
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

/// Poll `document.readyState` until the page finishes loading, or until a
/// timeout elapses.
///
/// Called after `navigate()` (whose `Page.navigate` CDP call only
/// acknowledges that navigation *started*) — and also, from `interact` and
/// `input`, after any click or Enter keypress that might have triggered a
/// navigation itself (a link, a form submit). Those never get an equivalent
/// "navigation started" acknowledgment at all, so this starts with a short
/// grace delay to give a just-triggered navigation a moment to actually kick
/// off before the first readiness check — otherwise it can read the *old*
/// document's `"complete"` state and return immediately, missing the
/// navigation entirely. Without this being called after click/key_press, the
/// very next `browser_read`/`browser_click`/`browser_screenshot` could race a
/// still-loading (or about-to-navigate) page and see an empty, half-built, or
/// stale DOM. Best effort: some pages (SPAs with long-polling connections,
/// etc.) never settle to `"complete"`, so this gives up after ~8s and lets
/// the caller proceed rather than blocking the agent loop indefinitely.
pub(super) async fn wait_for_page_load(config: &MintConfig) {
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    for _ in 0..40 {
        if let Ok(response) = cdp_call_raw(
            config,
            "Runtime.evaluate",
            json!({ "expression": "document.readyState", "returnByValue": true }),
        )
        .await
        {
            if response["result"]["result"]["value"].as_str() == Some("complete") {
                return;
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
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

//! Selector-driven interaction: resolving a CSS/text/xpath selector to an
//! element and clicking or typing into it. Built on top of the native mouse
//! and keyboard primitives in `input`.

use crate::MintConfig;
use serde_json::{Value, json};

use super::cdp::{cdp_call, cdp_call_raw, response_error};
use super::input::{mouse_click, type_text_native};
use super::lifecycle::ensure_page_open;
use super::logging::log_action;
use super::navigate::wait_for_page_load;
use super::overlay::inject_overlay;

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
                // The coordinate-based path already waits inside
                // `mouse_click`; this JS `.click()` fallback bypasses that,
                // so it needs its own wait — see `wait_for_page_load`'s doc.
                wait_for_page_load(config).await;
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

    // Focus the element by clicking it first. Propagate failure instead of
    // swallowing it: `Input.insertText` types into whatever currently has
    // focus, so if the click silently failed to find the element, the old
    // behavior would type into the wrong field (or nowhere) with no error
    // surfaced back to the agent to recover from.
    click(config, selector).await?;
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Native keyboard input
    type_text_native(config, text).await
}

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

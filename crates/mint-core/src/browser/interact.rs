//! Selector-driven interaction: resolving a CSS/text/xpath selector to an
//! element and clicking or typing into it. Built on top of the native mouse
//! and keyboard primitives in `input`.

use crate::MintConfig;
use serde_json::{Value, json};

use super::cdp::{cdp_call_raw, response_error};
use super::input::{mouse_click, type_text_native};
use super::lifecycle::ensure_page_open;
use super::logging::log_action;
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

    let (x, y) = get_element_coordinates(config, selector).await?;
    mouse_click(config, x, y, "left").await
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

    edit_text(config, selector, text, false).await
}

/// Pin one DOM node for the entire edit; never resolve a replacement selector.
pub(super) async fn edit_text(
    config: &MintConfig,
    selector: &str,
    text: &str,
    replace: bool,
) -> Result<String, String> {
    let token = uuid::Uuid::new_v4().to_string();
    let find = selector_to_js_find(selector);
    eval(config, format!("(() => {{ {find} if (!el || !el.isConnected) throw new Error('stale_reference'); if (el.readOnly || !(el.tagName === 'TEXTAREA' || (el.tagName === 'INPUT' && ['text','search','url','tel','password','email','number'].includes(el.type)))) throw new Error('unsupported_edit_target'); (globalThis.__mintEditable ||= new Map()).set({},el); return true; }})()",json!(token))).await?;
    let pinned = format!("mint-edit={token}");
    let find = selector_to_js_find(&pinned);
    let check = format!(
        "{find} if (!el || !el.isConnected) throw new Error('stale_reference'); if (el.disabled || el.readOnly || el.getAttribute('aria-disabled') === 'true' || !(el.tagName === 'TEXTAREA' || (el.tagName === 'INPUT' && ['text','search','url','tel','password','email','number'].includes(el.type)))) throw new Error('focus_changed: original target is no longer editable'); if (el.getRootNode().activeElement !== el) throw new Error('focus_changed');"
    );
    let result = async {
        click(config, &pinned).await?;
        let previous = eval(config,format!("(() => {{ {check} {} return el.value; }})()",if replace {""} else {"if (typeof el.selectionStart === 'number') el.setSelectionRange(el.value.length,el.value.length);"})).await?;
        if replace {
            let platform = eval(config,"navigator.platform".into()).await?;
            let modifiers = if platform.as_str().unwrap_or("").contains("Mac") {4} else {2};
            for kind in ["keyDown", "keyUp"] {
                cdp_call_raw(config,"Input.dispatchKeyEvent",json!({"type":kind,"key":"a","code":"KeyA","windowsVirtualKeyCode":65,"nativeVirtualKeyCode":65,"modifiers":modifiers})).await?;
            }
        } else {
            super::key_press(config,"End").await?;
        }
        // Key handlers may redirect focus or replace the node. Stop before text input.
        eval(config,format!("(() => {{ {check} return true; }})()")).await?;
        if replace && text.is_empty() { super::key_press(config,"Backspace").await?; }
        else { type_text_native(config,text).await?; }
        let value = eval(config,format!("(() => {{ {find} if (!el?.isConnected) throw new Error('stale_reference'); return el.value; }})()")).await?;
        let expected = if replace {text.to_owned()} else {format!("{}{text}",previous.as_str().ok_or("unsupported_edit_target")?)};
        if value.as_str() != Some(expected.as_str()) {return Err("field_value_mismatch".into());}
        Ok("value_verified".into())
    }.await;
    let _ = eval(
        config,
        format!("globalThis.__mintEditable?.delete({})", json!(token)),
    )
    .await;
    result
}

async fn eval(config: &MintConfig, expression: String) -> Result<Value, String> {
    Ok(cdp_call_raw(
        config,
        "Runtime.evaluate",
        json!({"expression":expression,"returnByValue":true}),
    )
    .await?["result"]["result"]["value"]
        .clone())
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
            if (!el || !el.isConnected) throw new Error('element_not_found_or_stale');
            el.scrollIntoView({{behavior:'instant',block:'center',inline:'center'}});
            const r = el.getBoundingClientRect(), style = getComputedStyle(el);
            if (!r.width || !r.height || style.visibility === 'hidden' || style.display === 'none') throw new Error('element_not_visible');
            if (el.disabled || el.getAttribute('aria-disabled') === 'true') throw new Error('element_disabled');
            const x = r.left + r.width / 2, y = r.top + r.height / 2;
            let hit = document.elementFromPoint(x,y);
            while (hit && hit.shadowRoot) {{ const deeper = hit.shadowRoot.elementFromPoint(x,y); if (!deeper || deeper === hit) break; hit = deeper; }}
            if (hit !== el && !el.contains(hit)) throw new Error('element_obstructed');
            return JSON.stringify({{ x, y }});
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
pub(super) fn selector_to_js_find(selector: &str) -> String {
    let quoted = |s: &str| serde_json::to_string(s).expect("string serialization");
    if selector == "mint-focus=" {
        "let el=document.activeElement; while (el?.shadowRoot?.activeElement) el=el.shadowRoot.activeElement;".into()
    } else if let Some(point) = selector.strip_prefix("mint-point=") {
        let (x, y) = point.split_once(',').unwrap_or(("0", "0"));
        let x = x.parse::<f64>().unwrap_or(0.0);
        let y = y.parse::<f64>().unwrap_or(0.0);
        format!(
            "let el=document.elementFromPoint({x},{y}); while(el?.shadowRoot) {{const hit=el.shadowRoot.elementFromPoint({x},{y});if (!hit || hit===el) break;el=hit;}}"
        )
    } else if selector == "mint-attempt=" {
        "const el = globalThis.__mintAttemptTarget;".into()
    } else if let Some(token) = selector.strip_prefix("mint-edit=") {
        format!(
            "const el = globalThis.__mintEditable?.get({});",
            quoted(token)
        )
    } else if let Some(reference) = selector.strip_prefix("mint-ref=") {
        format!(
            "const el = globalThis.__mintObservation?.elements.get({});",
            quoted(reference)
        )
    } else if let Some(text) = selector.strip_prefix("text=") {
        format!(
            "const el = Array.from(document.querySelectorAll('*')).find(e => e.childElementCount === 0 && e.textContent.trim() === {});",
            quoted(text)
        )
    } else if let Some(text) = selector.strip_prefix("contains=") {
        format!(
            "const el = Array.from(document.querySelectorAll('*')).find(e => e.childElementCount === 0 && e.textContent.includes({}));",
            quoted(text)
        )
    } else if let Some(xpath) = selector.strip_prefix("xpath=") {
        format!(
            "const el = document.evaluate({}, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null).singleNodeValue;",
            quoted(xpath)
        )
    } else {
        format!("const el = document.querySelector({});", quoted(selector))
    }
}

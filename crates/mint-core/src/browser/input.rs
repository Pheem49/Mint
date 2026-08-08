//! Native mouse and keyboard control via raw CDP `Input.*` events — the
//! low-level primitives that `interact::click`/`interact::type_text` build on
//! top of, and that are also exposed directly for coordinate-driven control
//! (e.g. clicking a point found via `browser_screenshot`).

use crate::MintConfig;
use serde_json::json;

use super::cdp::cdp_call_raw;
use super::lifecycle::ensure_page_open;
use super::logging::log_action;
use super::navigate::wait_for_page_load;
use super::overlay::inject_overlay;

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
    // The click may have landed on a link or submit button and triggered a
    // full page navigation — see `wait_for_page_load`'s doc comment.
    wait_for_page_load(config).await;
    Ok(format!(
        "clicked at ({x:.0},{y:.0}) with {button_str} button"
    ))
}

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

    // keyDown. For Enter specifically, `text`/`unmodifiedText` must be set to
    // "\r" or Chrome dispatches the DOM `keydown` event but skips its native
    // default action — implicit form submission — entirely; a synthetic Enter
    // sent without it looks like it worked (no error) but silently does
    // nothing on any real `<form>`. Confirmed empirically against a live
    // Chrome instance: identical event but for this field, submit vs no-op.
    let mut key_down = json!({
        "type": "keyDown",
        "key": key,
        "code": code,
        "windowsVirtualKeyCode": key_code,
        "nativeVirtualKeyCode": key_code,
        "modifiers": 0,
        "isSystemKey": false,
        "location": 0
    });
    if matches!(key, "Enter" | "\n" | "\r") {
        key_down["text"] = json!("\r");
        key_down["unmodifiedText"] = json!("\r");
    }
    cdp_call_raw(config, "Input.dispatchKeyEvent", key_down)
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
    if matches!(key, "Enter" | "\n" | "\r") {
        // Enter commonly submits the focused form — see `wait_for_page_load`'s
        // doc comment. Other keys (Tab, Escape, arrows, ...) don't trigger
        // navigation, so there's nothing to wait for.
        wait_for_page_load(config).await;
    }
    Ok(format!("pressed key '{key}'"))
}

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

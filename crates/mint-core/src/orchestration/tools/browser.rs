use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to browser.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    _root: &Path,
    config: &MintConfig,
    _chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "browser_open" => {
            let url = if !input.url.is_empty() {
                &input.url
            } else if !input.path.is_empty() {
                &input.path
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_open requires 'url'".into(),
                ));
            };
            if crate::is_browser_running(config).await {
                let result = crate::browser::navigate(config, url)
                    .await
                    .map_err(OrchestrationError::Agent)?;
                Ok(result)
            } else {
                let opened = if cfg!(target_os = "macos") {
                    std::process::Command::new("open").arg(url).spawn().is_ok()
                } else if cfg!(target_os = "windows") {
                    std::process::Command::new("cmd")
                        .args(["/C", "start", url])
                        .spawn()
                        .is_ok()
                } else {
                    std::process::Command::new("xdg-open")
                        .arg(url)
                        .spawn()
                        .is_ok()
                };
                if opened {
                    Ok(format!(
                        "Mint Auto is not active. Opened {url} in your default browser instead."
                    ))
                } else {
                    Err(OrchestrationError::Agent(
                        "Failed to open URL in default browser.".into(),
                    ))
                }
            }
        }
        "browser_click" => {
            let selector = if !input.selector.is_empty() {
                &input.selector
            } else if !input.path.is_empty() {
                &input.path
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_click requires 'selector'".into(),
                ));
            };
            let result = crate::browser::click(config, selector)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_type" => {
            let selector = if !input.selector.is_empty() {
                &input.selector
            } else if !input.path.is_empty() {
                &input.path
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_type requires 'selector'".into(),
                ));
            };
            let text = if !input.text.is_empty() {
                &input.text
            } else if !input.query.is_empty() {
                &input.query
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_type requires 'text'".into(),
                ));
            };
            let result = crate::browser::type_text(config, selector, text)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_read" => {
            let result = crate::browser::read_page_text(config)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_mouse_move" => {
            let x = input.x.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_move requires 'x'".into())
            })?;
            let y = input.y.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_move requires 'y'".into())
            })?;
            let result = crate::browser::mouse_move(config, x, y)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_mouse_click" => {
            let x = input.x.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_click requires 'x'".into())
            })?;
            let y = input.y.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_click requires 'y'".into())
            })?;
            let button = if input.button.is_empty() {
                "left"
            } else {
                &input.button
            };
            let result = crate::browser::mouse_click(config, x, y, button)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_key_press" => {
            let key = if !input.key.is_empty() {
                &input.key
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_key_press requires 'key'".into(),
                ));
            };
            let result = crate::browser::key_press(config, key)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_screenshot" => {
            let data = crate::browser::screenshot(config)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(format!("data:image/png;base64,{data}"))
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::browser::execute: {action}"
        ),
    }
}

use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use image::ImageFormat;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use crate::integrations::call_mcp_tool;
use crate::system::run_system_automation;
use mint_core::{KnowledgeStore, create_folder, find_paths};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAction {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Deserialize)]
pub struct CaptureRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

pub fn open_desktop_window(app: &AppHandle, kind: &str) -> Result<(), String> {
    let (label, route, width, height, always_on_top, skip_taskbar) = match kind {
        "settings" => ("settings", "settings", 1020.0, 720.0, false, false),
        "spotlight" => ("spotlight", "spotlight", 600.0, 80.0, true, true),
        "widget" => ("widget", "widget", 150.0, 150.0, true, true),
        "screen-picker" => ("screen-picker", "screen-picker", 1280.0, 800.0, true, true),
        other => return Err(format!("unsupported desktop window '{other}'")),
    };

    if let Some(window) = app.get_webview_window(label) {
        window.show().map_err(|error| error.to_string())?;
        window.set_focus().map_err(|error| error.to_string())?;
        return Ok(());
    }

    let url = WebviewUrl::App(format!("index.html#/{route}").into());
    let mut builder = WebviewWindowBuilder::new(app, label, url)
        .title(format!("Mint {kind}"))
        .inner_size(width, height)
        .decorations(false)
        .transparent(true)
        .always_on_top(always_on_top)
        .skip_taskbar(skip_taskbar);
    if kind == "screen-picker" {
        builder = builder.fullscreen(true);
    }
    builder.build().map_err(|error| error.to_string())?;
    Ok(())
}

pub fn hide_window(app: &AppHandle, label: &str) -> Result<(), String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' is not open"))?
        .hide()
        .map_err(|error| error.to_string())
}

pub fn close_window(app: &AppHandle, label: &str) -> Result<(), String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' is not open"))?
        .close()
        .map_err(|error| error.to_string())
}

pub fn resize_window(app: &AppHandle, label: &str, width: u32, height: u32) -> Result<(), String> {
    app.get_webview_window(label)
        .ok_or_else(|| format!("window '{label}' is not open"))?
        .set_size(PhysicalSize::new(width, height))
        .map_err(|error| error.to_string())
}

pub fn position_widget(app: &AppHandle) {
    let Some(widget) = app.get_webview_window("widget") else {
        return;
    };
    let Ok(Some(monitor)) = widget.primary_monitor() else {
        return;
    };
    let size = monitor.size();
    let x = size.width.saturating_sub(190) as i32;
    let _ = widget.set_position(PhysicalPosition::new(x, 40));
}

pub fn execute_action(
    config: &mint_core::MintConfig,
    action: DesktopAction,
) -> Result<ActionResult, String> {
    match action.kind.as_str() {
        "none" => Ok(success("no action requested")),
        "system_info" => Ok(success(&format!(
            "os={} arch={} family={}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        ))),
        "open_url" => {
            if !(action.target.starts_with("https://") || action.target.starts_with("http://")) {
                return Err("only http and https URLs may be opened".into());
            }
            spawn_detached("xdg-open", &[&action.target])?;
            Ok(success("opened URL"))
        }
        "search" => {
            let query = action.target.trim();
            if query.is_empty() {
                return Err("search query is required".into());
            }
            let url = format!("https://www.google.com/search?q={}", encode_query(query));
            spawn_detached("xdg-open", &[&url])?;
            Ok(success("opened web search"))
        }
        "open_app" => {
            let app = action.target.trim();
            if app.is_empty()
                || !app
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | '.'))
            {
                return Err("application name contains unsupported characters".into());
            }
            spawn_detached(app, &[])?;
            Ok(success("opened application"))
        }
        "clipboard_write" => Err("clipboard actions are handled by the renderer".into()),
        "mcp_tool" => call_mcp_tool(config, &action.server, &action.target, action.args)
            .map(|result| success(&result.to_string())),
        "system_automation" => {
            run_system_automation(&action.target, action.approved).map(|message| success(&message))
        }
        "create_folder" => create_folder(std::path::Path::new(&action.target), config)
            .map(|path| success(&format!("created {}", path.display())))
            .map_err(|error| error.to_string()),
        "find_path" => {
            let roots = action.args["roots"]
                .as_array()
                .map(|roots| {
                    roots
                        .iter()
                        .filter_map(Value::as_str)
                        .map(PathBuf::from)
                        .collect::<Vec<_>>()
                })
                .filter(|roots| !roots.is_empty())
                .unwrap_or_else(|| {
                    let mut roots =
                        vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
                    if let Some(home) = dirs::home_dir() {
                        roots.push(home);
                    }
                    roots
                });
            let limit = action.args["limit"].as_u64().unwrap_or(20).min(100) as usize;
            serde_json::to_string(&find_paths(&action.target, &roots, limit, config))
                .map(|message| success(&message))
                .map_err(|error| error.to_string())
        }
        "learn_file" => KnowledgeStore::open_default()
            .map_err(|error| error.to_string())?
            .index_file(std::path::Path::new(&action.target), config)
            .map(|chunks| success(&format!("indexed {chunks} knowledge chunks")))
            .map_err(|error| error.to_string()),
        other => Err(format!(
            "desktop action '{other}' has not migrated to the allowlisted Rust executor"
        )),
    }
}

pub fn capture_screen() -> Result<String, String> {
    capture_screen_bytes().map(|bytes| format!("data:image/png;base64,{}", STANDARD.encode(bytes)))
}

pub async fn translate_screen_region(
    config: &mint_core::MintConfig,
    rect: CaptureRect,
) -> Result<String, String> {
    let bytes = capture_screen_bytes()?;
    let image = image::load_from_memory(&bytes).map_err(|error| error.to_string())?;
    if rect.width == 0 || rect.height == 0 || rect.x >= image.width() || rect.y >= image.height() {
        return Err("screen capture rectangle is invalid".into());
    }
    let width = rect.width.min(image.width() - rect.x);
    let height = rect.height.min(image.height() - rect.y);
    let cropped = image.crop_imm(rect.x, rect.y, width, height);
    let mut jpeg = std::io::Cursor::new(Vec::new());
    cropped
        .write_to(&mut jpeg, ImageFormat::Jpeg)
        .map_err(|error| error.to_string())?;
    let api_key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if api_key.trim().is_empty() {
        return Err("Gemini API key is required for live translation".into());
    }
    let value: Value = Client::new()
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={api_key}",
            config.gemini_model
        ))
        .json(&json!({
            "contents": [{
                "role": "user",
                "parts": [
                    { "text": "Translate visible text in this image into Thai. Return only the translated text. If there is no readable text, return an empty string." },
                    { "inlineData": { "mimeType": "image/jpeg", "data": STANDARD.encode(jpeg.into_inner()) } }
                ]
            }]
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    value["candidates"][0]["content"]["parts"][0]["text"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| "Gemini translation response did not include text".into())
}

pub(crate) fn capture_screen_bytes() -> Result<Vec<u8>, String> {
    let path = std::env::temp_dir().join(format!("mint-screen-{}.png", std::process::id()));
    let commands = [
        ("grim", vec![path_string(&path)]),
        ("gnome-screenshot", vec!["-f".into(), path_string(&path)]),
        (
            "spectacle",
            vec!["-b".into(), "-n".into(), "-o".into(), path_string(&path)],
        ),
        ("scrot", vec![path_string(&path)]),
        (
            "import",
            vec!["-window".into(), "root".into(), path_string(&path)],
        ),
    ];
    let mut attempted = Vec::new();
    for (program, args) in commands {
        attempted.push(program);
        let result = Command::new(program)
            .args(&args)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if result.is_ok_and(|status| status.success()) && path.exists() {
            let bytes = fs::read(&path).map_err(|error| error.to_string())?;
            let _ = fs::remove_file(&path);
            return Ok(bytes);
        }
    }
    Err(format!(
        "screen capture requires one of these commands: {}",
        attempted.join(", ")
    ))
}

pub fn integration_status(config: &mint_core::MintConfig) -> Value {
    let mcp_servers = config
        .extra
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(|servers| servers.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    json!({
        "automation": {
            "supportedActions": ["open_url", "open_app", "search", "system_info", "system_automation", "find_path", "create_folder", "learn_file"],
            "approvalRequired": true
        },
        "mcp": {
            "configuredServers": mcp_servers,
            "execution": "native-stdio"
        },
        "plugins": {
            "migrated": ["desktop-actions", "dev_tools", "docker", "obsidian", "spotify", "system_metrics"],
            "legacyBridge": false
        }
    })
}

pub fn emit_to_main(app: &AppHandle, event: &str, payload: impl Serialize + Clone) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit(event, payload);
        let _ = main.show();
        let _ = main.set_focus();
    }
}

fn success(message: &str) -> ActionResult {
    ActionResult {
        success: true,
        message: message.into(),
    }
}

fn spawn_detached(program: &str, args: &[&str]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("unable to start '{program}': {error}"))
}

fn path_string(path: &PathBuf) -> String {
    path.to_string_lossy().into_owned()
}

fn encode_query(query: &str) -> String {
    query
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_search_queries_for_urls() {
        assert_eq!(encode_query("mint cli"), "mint+cli");
        assert_eq!(encode_query("a/b"), "a%2Fb");
    }
}

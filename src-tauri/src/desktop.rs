use std::{
    fs,
    path::PathBuf,
    process::{Command, Stdio},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindowBuilder,
};

use crate::integrations::call_mcp_tool;

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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
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
        other => Err(format!(
            "desktop action '{other}' has not migrated to the allowlisted Rust executor"
        )),
    }
}

pub fn capture_screen() -> Result<String, String> {
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
            return Ok(format!("data:image/png;base64,{}", STANDARD.encode(bytes)));
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
            "supportedActions": ["open_url", "open_app"],
            "approvalRequired": true
        },
        "mcp": {
            "configuredServers": mcp_servers,
            "execution": "native-stdio"
        },
        "plugins": {
            "migrated": ["desktop-actions"],
            "legacyBridge": true
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

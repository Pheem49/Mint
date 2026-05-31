mod desktop;
mod integrations;
mod workflows;

use desktop::{
    ActionResult, DesktopAction, capture_screen, close_window, emit_to_main, execute_action,
    hide_window, integration_status, open_desktop_window, position_widget, resize_window,
};
use integrations::{configured_mcp_servers, list_plugins};
use mint_core::{
    ChatRequest, ChatResponse, InteractionMemory, MemoryStore, MintConfig, classify_shell_command,
    config_path, load_config, orchestrate_chat, save_config, stream_chunks,
};
use serde::Serialize;
use serde_json::Value;
use std::process::Command;
use tauri::{
    AppHandle, Emitter, Manager,
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use workflows::{load_workflows, start_monitor, workflows_path};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    backend: &'static str,
    config_path: String,
    active_provider: String,
    available_providers: Vec<String>,
    integrations: Value,
}

#[tauri::command]
fn get_runtime_status() -> Result<RuntimeStatus, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    Ok(RuntimeStatus {
        backend: "rust",
        config_path: config_path()
            .map_err(|error| error.to_string())?
            .display()
            .to_string(),
        active_provider: config.ai_provider.clone(),
        available_providers: config
            .available_providers()
            .into_iter()
            .map(str::to_owned)
            .collect(),
        integrations: integration_status(&config),
    })
}

#[tauri::command]
fn get_config() -> Result<MintConfig, String> {
    load_config().map_err(|error| error.to_string())
}

#[tauri::command]
fn update_config(app: AppHandle, config: MintConfig) -> Result<(), String> {
    save_config(&config).map_err(|error| error.to_string())?;
    let _ = app.emit("settings-changed", &config);
    if config.show_desktop_widget {
        let _ = open_desktop_window(&app, "widget");
        position_widget(&app);
    } else if app.get_webview_window("widget").is_some() {
        let _ = close_window(&app, "widget");
    }
    Ok(())
}

#[tauri::command]
fn inspect_shell_command(command: String) -> mint_core::ShellClassification {
    classify_shell_command(&command)
}

#[tauri::command]
async fn send_chat_message(request: ChatRequest) -> Result<ChatResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    orchestrate_chat(&config, &request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn stream_chat_message(
    request: ChatRequest,
    on_event: Channel<String>,
) -> Result<ChatResponse, String> {
    let response = send_chat_message(request).await?;
    for chunk in stream_chunks(&response.text) {
        on_event.send(chunk).map_err(|error| error.to_string())?;
    }
    Ok(response)
}

#[tauri::command]
fn get_recent_interactions(limit: Option<usize>) -> Result<Vec<InteractionMemory>, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.recent_interactions(limit.unwrap_or(5)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn open_window(app: AppHandle, kind: String) -> Result<(), String> {
    open_desktop_window(&app, &kind)?;
    if kind == "widget" {
        position_widget(&app);
    }
    Ok(())
}

#[tauri::command]
fn hide_desktop_window(app: AppHandle, label: String) -> Result<(), String> {
    hide_window(&app, &label)
}

#[tauri::command]
fn close_desktop_window(app: AppHandle, label: String) -> Result<(), String> {
    close_window(&app, &label)
}

#[tauri::command]
fn resize_desktop_window(
    app: AppHandle,
    label: String,
    width: u32,
    height: u32,
) -> Result<(), String> {
    resize_window(&app, &label, width, height)
}

#[tauri::command]
fn run_desktop_action(action: DesktopAction) -> Result<ActionResult, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    execute_action(&config, action)
}

#[tauri::command]
fn get_integration_inventory() -> Result<Value, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "mcpServers": configured_mcp_servers(&config)?.keys().collect::<Vec<_>>(),
        "plugins": list_plugins()
    }))
}

#[tauri::command]
fn capture_silent_screen() -> Result<String, String> {
    capture_screen()
}

#[tauri::command]
fn start_screen_capture(app: AppHandle) -> Result<(), String> {
    open_desktop_window(&app, "screen-picker")
}

#[tauri::command]
fn submit_screen_selection(app: AppHandle, image: String) {
    emit_to_main(&app, "vision-ready", image);
    let _ = close_window(&app, "screen-picker");
}

#[tauri::command]
fn submit_spotlight(app: AppHandle, query: String) {
    emit_to_main(&app, "spotlight-to-chat", query);
    let _ = hide_window(&app, "spotlight");
}

#[tauri::command]
fn set_ai_state(app: AppHandle, state: String) {
    if let Some(widget) = app.get_webview_window("widget") {
        let _ = widget.emit("widget-state", state);
    }
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn open_workflows_file() -> Result<ActionResult, String> {
    load_workflows()?;
    let path = workflows_path()?;
    Command::new("xdg-open")
        .arg(path)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(ActionResult {
        success: true,
        message: "opened workflows file".into(),
    })
}

#[tauri::command]
fn reload_custom_workflows() -> Result<Value, String> {
    let workflows = load_workflows()?;
    Ok(serde_json::json!({
        "success": true,
        "count": workflows.len(),
        "workflows": workflows
    }))
}

fn install_tray(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(app, "show", "Show Mint", true, None::<&str>)?;
    let settings = MenuItem::with_id(app, "settings", "Settings", true, None::<&str>)?;
    let spotlight = MenuItem::with_id(app, "spotlight", "Spotlight", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &spotlight, &quit])?;
    let mut builder = TrayIconBuilder::new()
        .tooltip("Mint AI Assistant")
        .menu(&menu)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "show" => {
                let _ = app.get_webview_window("main").map(|window| window.show());
            }
            "settings" => {
                let _ = open_desktop_window(app, "settings");
            }
            "spotlight" => {
                let _ = open_desktop_window(app, "spotlight");
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = if window.is_visible().unwrap_or(false) {
                        window.hide()
                    } else {
                        window.show()
                    };
                }
            }
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            install_tray(app.handle())?;
            start_monitor(app.handle().clone());
            if load_config()
                .map(|config| config.show_desktop_widget)
                .unwrap_or(false)
            {
                let _ = open_desktop_window(app.handle(), "widget");
                position_widget(app.handle());
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_runtime_status,
            get_config,
            update_config,
            inspect_shell_command,
            send_chat_message,
            stream_chat_message,
            get_recent_interactions,
            open_window,
            hide_desktop_window,
            close_desktop_window,
            resize_desktop_window,
            run_desktop_action,
            get_integration_inventory,
            capture_silent_screen,
            start_screen_capture,
            submit_screen_selection,
            submit_spotlight,
            set_ai_state,
            exit_app,
            open_workflows_file,
            reload_custom_workflows
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mint desktop");
}

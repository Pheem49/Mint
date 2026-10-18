#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
mod compact;

use futures_util::{SinkExt, StreamExt};
use mint_companion_protocol::{Command, Event, HostDescriptor, VERSION};
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU64, Ordering},
    },
};
use tauri::{Emitter, Manager};
use tokio::{net::TcpStream, sync::mpsc};
use tokio_tungstenite::{
    connect_async,
    tungstenite::{Message, client::IntoClientRequest},
};

fn valid_endpoint(endpoint: &str) -> bool {
    endpoint
        .strip_prefix("ws://")
        .and_then(|v| v.strip_suffix("/companion/v1"))
        .and_then(|addr| addr.parse::<std::net::SocketAddr>().ok())
        .is_some_and(|addr| addr.ip().is_loopback() && addr.port() != 0)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Settings {
    host_id: String,
    host_identity: String,
    host_peers: Vec<String>,
    speech_auto: bool,
    speech_muted: bool,
    speech_volume: f64,
    chat_id: String,
    scale: f64,
    offset_y: f64,
    expression: i32,
    accessory: u32,
    locked: bool,
    guide: bool,
    always_on_top: bool,
    normal_window: bool,
    panel_open: bool,
    x: Option<i32>,
    y: Option<i32>,
    width: u32,
    height: u32,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            host_id: String::new(),
            host_identity: String::new(),
            host_peers: Vec::new(),
            speech_auto: false,
            speech_muted: false,
            speech_volume: 1.0,
            chat_id: String::new(),
            scale: 1.0,
            offset_y: 0.0,
            expression: -1,
            accessory: 0,
            locked: false,
            guide: false,
            always_on_top: true,
            normal_window: false,
            panel_open: true,
            x: None,
            y: None,
            width: 480,
            height: 800,
        }
    }
}
#[derive(Default)]
struct Client {
    sender: Arc<Mutex<Option<mpsc::Sender<Command>>>>,
    generation: Arc<AtomicU64>,
    task: Mutex<Option<tauri::async_runtime::JoinHandle<()>>>,
    settings: Mutex<Settings>,
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Host {
    host_id: String,
    name: String,
    identity: Option<String>,
    surface: Option<mint_companion_protocol::Surface>,
    speech: bool,
}
fn settings_path() -> Result<PathBuf, String> {
    dirs::config_dir()
        .map(|p| p.join("mint-companion/settings.json"))
        .ok_or_else(|| "User config directory unavailable".into())
}
fn read_settings() -> Settings {
    settings_path()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}
fn persist(settings: &Settings) -> Result<(), String> {
    let path = settings_path()?;
    std::fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    std::fs::write(
        path,
        serde_json::to_vec_pretty(settings).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn descriptor(host: &str) -> Result<HostDescriptor, String> {
    let id = uuid::Uuid::parse_str(host).map_err(|_| "Invalid host ID")?;
    let path = mint_companion_protocol::hosts_dir()
        .ok_or("User config directory unavailable")?
        .join(format!("{id}.json"));
    let descriptor: HostDescriptor = serde_json::from_slice(
        &std::fs::read(path)
            .map_err(|_| "Mint is offline. Open Mint or choose another instance.")?,
    )
    .map_err(|_| "Invalid host descriptor")?;
    if descriptor.version != VERSION {
        return Err("Update Mint and Companion to compatible versions.".into());
    }
    if descriptor.host_id != host
        || !valid_endpoint(&descriptor.endpoint)
        || descriptor.token.len() != 32
    {
        return Err("Invalid local host descriptor".into());
    }
    Ok(descriptor)
}
async fn open(
    host: &HostDescriptor,
) -> Result<tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>, String>
{
    let mut request = host
        .endpoint
        .clone()
        .into_client_request()
        .map_err(|e| e.to_string())?;
    request.headers_mut().insert(
        "Authorization",
        format!("Bearer {}", host.token)
            .parse()
            .map_err(|_| "Invalid credentials")?,
    );
    tokio::time::timeout(std::time::Duration::from_secs(2), connect_async(request))
        .await
        .map_err(|_| "Mint did not respond")?
        .map(|(socket, _)| socket)
        .map_err(|_| "Could not connect to Mint".to_string())
}
#[tauri::command]
async fn discover_hosts() -> Result<Vec<Host>, String> {
    let Some(dir) = mint_companion_protocol::hosts_dir() else {
        return Ok(Vec::new());
    };
    let Ok(files) = std::fs::read_dir(dir) else {
        return Ok(Vec::new());
    };
    let ids: Vec<_> = files
        .flatten()
        .filter_map(|entry| {
            entry
                .path()
                .file_stem()
                .map(|s| s.to_string_lossy().into_owned())
        })
        .collect();
    let probes = ids.into_iter().map(|id| async move {
        let host = descriptor(&id).ok()?;
        let mut socket = open(&host).await.ok()?;
        let _ = socket
            .send(Message::Text(
                serde_json::to_string(&Command::ListSessions).ok()?.into(),
            ))
            .await;
        let response = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
            .await
            .ok()??
            .ok()?;
        let event: Event = serde_json::from_str(&response.to_string()).ok()?;
        let _ = socket.close(None).await;
        match event {
            Event::Snapshot {
                host_id,
                version: VERSION,
                surface,
                speech,
                ..
            } if host_id == id => Some(Host {
                host_id: id,
                name: host.name,
                identity: host.identity,
                surface,
                speech,
            }),
            _ => None,
        }
    });
    let mut hosts: Vec<_> = futures_util::future::join_all(probes)
        .await
        .into_iter()
        .flatten()
        .collect();
    hosts.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(hosts)
}
fn connection(app: &tauri::AppHandle, host_id: &str, online: bool, message: &str) {
    let _ = app.emit(
        "companion-connection",
        serde_json::json!({"hostId": host_id, "online": online, "message": message}),
    );
}
fn replace_sender(
    sender: &Mutex<Option<mpsc::Sender<Command>>>,
    generation: &AtomicU64,
    expected: u64,
    replacement: Option<mpsc::Sender<Command>>,
) -> bool {
    let mut current = sender.lock().unwrap();
    if generation.load(Ordering::Acquire) != expected {
        return false;
    }
    *current = replacement;
    true
}
#[tauri::command]
fn connect_host(
    app: tauri::AppHandle,
    state: tauri::State<'_, Client>,
    host_id: String,
    chat_id: String,
) {
    let expected = state.generation.fetch_add(1, Ordering::AcqRel) + 1;
    if let Some(task) = state.task.lock().unwrap().take() {
        task.abort();
    }
    replace_sender(&state.sender, &state.generation, expected, None);
    if host_id.is_empty() {
        return;
    }
    let sender = Arc::clone(&state.sender);
    let generation = Arc::clone(&state.generation);
    let chat = chat_id;
    let task = tauri::async_runtime::spawn(async move {
        let mut wait = 1;
        let mut selected = if chat.is_empty() { None } else { Some(chat) };
        loop {
            let result = async {
                let host = descriptor(&host_id)?;
                let mut socket = open(&host).await?;
                socket.send(Message::Text(serde_json::to_string(&Command::ListSessions).unwrap().into())).await.map_err(|_| "Connection lost")?;
                if let Some(chat_id) = selected.clone() { socket.send(Message::Text(serde_json::to_string(&Command::Subscribe { chat_id }).unwrap().into())).await.map_err(|_| "Connection lost")?; }
                let (tx, mut rx) = mpsc::channel::<Command>(16);
                if !replace_sender(&sender, &generation, expected, Some(tx)) { return Ok(()); }
                connection(&app, &host_id, true, ""); wait = 1;
                loop {
                    tokio::select! {
                        incoming = socket.next() => {
                            let Some(Ok(message)) = incoming else { return Err("Mint disconnected. Submitted messages will not be retried automatically.".to_string()); };
                            match message {
                                Message::Text(text) => { let event: Event = serde_json::from_str(&text).map_err(|_| "Invalid companion response")?; let _ = app.emit("companion-event", event); }
                                Message::Ping(bytes) => { socket.send(Message::Pong(bytes)).await.map_err(|_| "Connection lost")?; }
                                Message::Close(_) => return Err("Mint is offline".into()),
                                _ => {},
                            }
                        }
                        command = rx.recv() => {
                            let Some(command) = command else { return Ok(()); };
                            if let Command::Subscribe { chat_id } = &command { selected = Some(chat_id.clone()); }
                            socket.send(Message::Text(serde_json::to_string(&command).unwrap().into())).await.map_err(|_| "Connection lost before confirmation. Messages will not be retried automatically.")?;
                        }
                    }
                }
            }.await;
            if !replace_sender(&sender, &generation, expected, None) {
                return;
            }
            connection(
                &app,
                &host_id,
                false,
                result
                    .as_ref()
                    .err()
                    .map(String::as_str)
                    .unwrap_or("Mint is offline"),
            );
            tokio::time::sleep(std::time::Duration::from_secs(wait)).await;
            wait = (wait * 2).min(30);
        }
    });
    *state.task.lock().unwrap() = Some(task);
}
#[tauri::command]
async fn open_conversation(
    host_id: String,
    chat_id: String,
    new_terminal: bool,
) -> Result<mint_companion_protocol::NavigationOutcome, String> {
    let host = descriptor(&host_id)?;
    tokio::time::timeout(std::time::Duration::from_secs(12), async {
        let mut socket = open(&host).await?;
        let command = Command::OpenChat {
            chat_id,
            new_terminal,
        };
        socket
            .send(Message::Text(
                serde_json::to_string(&command)
                    .map_err(|error| error.to_string())?
                    .into(),
            ))
            .await
            .map_err(|error| error.to_string())?;
        while let Some(message) = socket.next().await {
            let message = message.map_err(|error| error.to_string())?;
            if let Message::Text(text) = message {
                let event: Event =
                    serde_json::from_str(&text).map_err(|error| error.to_string())?;
                match event {
                    Event::Navigation { outcome } => {
                        let _ = socket.close(None).await;
                        return Ok(outcome);
                    }
                    Event::Error { message, .. } => return Err(message),
                    _ => {}
                }
            }
        }
        Err("Mint disconnected before confirming navigation. Try again after checking Mint.".into())
    })
    .await
    .map_err(|_| "Mint did not confirm navigation. Check Mint before trying again.".to_string())?
}

#[tauri::command]
async fn speech_request(
    host_id: String,
    request: mint_companion_protocol::SpeechRequest,
) -> Result<mint_companion_protocol::SpeechReply, String> {
    let host = descriptor(&host_id)?;
    tokio::time::timeout(std::time::Duration::from_secs(15), async {
        let mut socket = open(&host).await?;
        let command = Command::Speech { request };
        socket
            .send(Message::Text(
                serde_json::to_string(&command)
                    .map_err(|e| e.to_string())?
                    .into(),
            ))
            .await
            .map_err(|e| e.to_string())?;
        while let Some(message) = socket.next().await {
            let message = message.map_err(|e| e.to_string())?;
            if let Message::Text(text) = message {
                match serde_json::from_str::<Event>(&text).map_err(|e| e.to_string())? {
                    Event::Speech { reply } => {
                        let _ = socket.close(None).await;
                        return Ok(reply);
                    }
                    Event::Error { message, .. } => return Err(message),
                    _ => {}
                }
            }
        }
        Err("Mint disconnected while preparing speech".into())
    })
    .await
    .map_err(|_| "Mint speech request timed out".to_string())?
}

#[tauri::command]
async fn send_command(state: tauri::State<'_, Client>, command: Command) -> Result<(), String> {
    let sender = state
        .sender
        .lock()
        .unwrap()
        .clone()
        .ok_or("Mint is offline")?;
    sender
        .send(command)
        .await
        .map_err(|_| "Mint disconnected".into())
}
#[tauri::command]
fn load_settings(state: tauri::State<'_, Client>) -> Settings {
    state.settings.lock().unwrap().clone()
}
#[tauri::command]
fn save_settings(
    app: tauri::AppHandle,
    state: tauri::State<'_, Client>,
    mut settings: Settings,
) -> Result<(), String> {
    settings.speech_volume = if settings.speech_volume.is_finite() {
        settings.speech_volume.clamp(0.0, 1.0)
    } else {
        1.0
    };
    settings.scale = settings.scale.clamp(0.5, 1.5);
    settings.expression = settings.expression.clamp(-1, 5);
    settings.accessory = settings.accessory.min(3);
    let previous = state.settings.lock().unwrap().clone();
    if let Some(window) = app.get_webview_window("main") {
        if previous.always_on_top != settings.always_on_top {
            window
                .set_always_on_top(settings.always_on_top)
                .map_err(|e| e.to_string())?;
        }
        if previous.normal_window != settings.normal_window {
            window
                .set_decorations(settings.normal_window)
                .map_err(|e| e.to_string())?;
        }
        settings.x = previous.x;
        settings.y = previous.y;
        settings.width = previous.width;
        settings.height = previous.height;
    }
    persist(&settings)?;
    *state.settings.lock().unwrap() = settings;
    Ok(())
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelativeCursorPosition {
    pub x: f64,
    pub y: f64,
}

pub fn calculate_relative_cursor(
    cursor_x: f64,
    cursor_y: f64,
    win_x: i32,
    win_y: i32,
    scale_factor: f64,
) -> RelativeCursorPosition {
    let scale = if scale_factor <= 0.0 || !scale_factor.is_finite() {
        1.0
    } else {
        scale_factor
    };
    RelativeCursorPosition {
        x: (cursor_x - win_x as f64) / scale,
        y: (cursor_y - win_y as f64) / scale,
    }
}

#[tauri::command]
fn get_cursor_position(window: tauri::WebviewWindow) -> Option<RelativeCursorPosition> {
    let cursor = window.cursor_position().ok()?;
    let win_pos = window.outer_position().ok()?;
    let scale = window.scale_factor().unwrap_or(1.0);
    Some(calculate_relative_cursor(
        cursor.x, cursor.y, win_pos.x, win_pos.y, scale,
    ))
}
fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = compact::set_compact(window.clone(), false);
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit("companion-visible", true);
}
fn main() {
    tauri::Builder::default()
        .manage(Client::default())
        .manage(compact::CompactWindow::default())
        .setup(|app| {
            let settings = read_settings();
            if let Some(window) = app.get_webview_window("main") {
                window.set_always_on_top(settings.always_on_top)?;
                window.set_decorations(settings.normal_window)?;
                if settings.x.is_some() {
                    window.set_size(tauri::PhysicalSize::new(
                        settings.width.clamp(340, 3200),
                        settings.height.clamp(560, 3200),
                    ))?;
                } else {
                    window.set_size(tauri::LogicalSize::new(480.0, 800.0))?;
                }
                if let (Some(x), Some(y)) = (settings.x, settings.y)
                    && window.available_monitors()?.iter().any(|m| {
                        let p = m.position();
                        let size = m.size();
                        x >= p.x
                            && y >= p.y
                            && x + 100 < p.x + size.width as i32
                            && y + 100 < p.y + size.height as i32
                    })
                {
                    window.set_position(tauri::PhysicalPosition::new(x, y))?;
                }
            }
            *app.state::<Client>().settings.lock().unwrap() = settings;
            let visibility_app = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let mut previous = true;
                loop {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let Some(window) = visibility_app.get_webview_window("main") else {
                        break;
                    };
                    let visible = window.is_visible().unwrap_or(false)
                        && !window.is_minimized().unwrap_or(false);
                    if visible != previous {
                        let _ = visibility_app.emit("companion-visible", visible);
                        previous = visible;
                    }
                }
            });
            let show_item =
                tauri::menu::MenuItem::with_id(app, "show", "Show Companion", true, None::<&str>)?;
            let quit_item =
                tauri::menu::MenuItem::with_id(app, "quit", "Quit Companion", true, None::<&str>)?;
            let menu = tauri::menu::Menu::with_items(app, &[&show_item, &quit_item])?;
            tauri::tray::TrayIconBuilder::new()
                .icon(app.default_window_icon().unwrap().clone())
                .tooltip("Mint Companion")
                .menu(&menu)
                .on_menu_event(|app, event| {
                    if event.id.as_ref() == "show" {
                        show(app);
                    } else if event.id.as_ref() == "quit" {
                        app.exit(0);
                    }
                })
                .build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            tauri::WindowEvent::Moved(position) => {
                if window
                    .state::<compact::CompactWindow>()
                    .active
                    .load(Ordering::Acquire)
                {
                    return;
                }
                let state = window.state::<Client>();
                let mut settings = state.settings.lock().unwrap();
                settings.x = Some(position.x);
                settings.y = Some(position.y);
                let _ = persist(&settings);
            }
            tauri::WindowEvent::Resized(size) if size.width >= 340 && size.height >= 560 => {
                if window
                    .state::<compact::CompactWindow>()
                    .active
                    .load(Ordering::Acquire)
                {
                    return;
                }
                let state = window.state::<Client>();
                let mut settings = state.settings.lock().unwrap();
                settings.width = size.width;
                settings.height = size.height;
                let _ = persist(&settings);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            compact::set_compact,
            discover_hosts,
            open_conversation,
            speech_request,
            connect_host,
            send_command,
            load_settings,
            save_settings,
            quit_app,
            get_cursor_position
        ])
        .run(tauri::generate_context!())
        .expect("Failed to launch Mint Companion");
}
#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn stale_disconnect_does_not_clear_the_new_hosts_sender() {
        let sender = Mutex::new(None);
        let generation = AtomicU64::new(2);
        let (tx, mut rx) = mpsc::channel(1);
        replace_sender(&sender, &generation, 2, Some(tx));
        replace_sender(&sender, &generation, 1, None);
        let current = sender.lock().unwrap().clone();
        assert!(
            current.is_some(),
            "old host callback disconnected the new host"
        );
        current.unwrap().send(Command::ListSessions).await.unwrap();
        assert!(matches!(rx.recv().await, Some(Command::ListSessions)));
    }
    #[test]
    fn discovery_only_connects_to_the_local_companion_path() {
        assert!(valid_endpoint("ws://127.0.0.1:1234/companion/v1"));
        for endpoint in [
            "ws://example.com:1234/companion/v1",
            "ws://127.0.0.1:1234/api",
            "ws://127.0.0.1:1234/companion/v1?token=secret",
            "http://127.0.0.1:1234/companion/v1",
        ] {
            assert!(!valid_endpoint(endpoint));
        }
    }
    #[test]
    fn relative_cursor_calculation_handles_offsets_and_dpi_scale() {
        let origin = calculate_relative_cursor(1440.0, 100.0, 1440, 100, 1.0);
        assert_eq!(origin, RelativeCursorPosition { x: 0.0, y: 0.0 });

        let left = calculate_relative_cursor(440.0, 50.0, 1440, 100, 1.0);
        assert_eq!(
            left,
            RelativeCursorPosition {
                x: -1000.0,
                y: -50.0
            }
        );

        let scaled = calculate_relative_cursor(1640.0, 300.0, 1440, 100, 2.0);
        assert_eq!(scaled, RelativeCursorPosition { x: 100.0, y: 100.0 });

        let fallback = calculate_relative_cursor(1500.0, 200.0, 1440, 100, 0.0);
        assert_eq!(fallback, RelativeCursorPosition { x: 60.0, y: 100.0 });
    }
    #[test]
    fn settings_deserialization_defaults_offset_y() {
        let json = r#"{"hostId":"","chatId":"","scale":1.2}"#;
        let s: Settings = serde_json::from_str(json).unwrap();
        assert_eq!(s.offset_y, 0.0);
        assert_eq!(s.scale, 1.2);
    }
}

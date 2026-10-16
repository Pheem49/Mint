#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
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
    chat_id: String,
    scale: f64,
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
            chat_id: String::new(),
            scale: 1.0,
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
                ..
            } if host_id == id => Some(Host {
                host_id: id,
                name: host.name,
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
fn hide_window(app: tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.hide();
    }
    let _ = app.emit("companion-visible", false);
}
#[tauri::command]
fn quit_app(app: tauri::AppHandle) {
    app.exit(0);
}
fn show(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    let _ = app.emit("companion-visible", true);
}
fn main() {
    tauri::Builder::default()
        .manage(Client::default())
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
                let state = window.state::<Client>();
                let mut settings = state.settings.lock().unwrap();
                settings.x = Some(position.x);
                settings.y = Some(position.y);
                let _ = persist(&settings);
            }
            tauri::WindowEvent::Resized(size) if size.width > 0 && size.height > 0 => {
                let state = window.state::<Client>();
                let mut settings = state.settings.lock().unwrap();
                settings.width = size.width;
                settings.height = size.height;
                let _ = persist(&settings);
            }
            _ => {}
        })
        .invoke_handler(tauri::generate_handler![
            discover_hosts,
            connect_host,
            send_command,
            load_settings,
            save_settings,
            hide_window,
            quit_app
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
}

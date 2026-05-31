use std::{
    fs,
    net::{SocketAddr, TcpStream},
    path::Path,
    thread,
    time::Duration,
};

use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

pub fn start_system_events(app: AppHandle) {
    thread::spawn(move || {
        let mut last_battery = None;
        let mut last_online = None;
        loop {
            let battery = battery_level(Path::new("/sys/class/power_supply"));
            if let Some(level) = battery
                .filter(|level| *level <= 20 && last_battery.is_none_or(|previous| previous > 20))
            {
                emit_notification(
                    &app,
                    json!({ "message": format!("Battery is low ({level}%). Please plug in your charger."), "type": "warning" }),
                );
            }
            last_battery = battery;

            let online = network_online();
            if last_online.is_some_and(|previous| previous != online) {
                emit_notification(
                    &app,
                    json!({
                        "message": if online { "Internet connection restored." } else { "Internet connection lost." },
                        "type": "info"
                    }),
                );
            }
            last_online = Some(online);
            thread::sleep(Duration::from_secs(60));
        }
    });
}

fn emit_notification(app: &AppHandle, payload: serde_json::Value) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit("proactive-notification", payload);
    }
}

fn battery_level(root: &Path) -> Option<u8> {
    fs::read_dir(root).ok()?.flatten().find_map(|entry| {
        let name = entry.file_name().to_string_lossy().to_ascii_uppercase();
        if !name.starts_with("BAT") {
            return None;
        }
        fs::read_to_string(entry.path().join("capacity"))
            .ok()?
            .trim()
            .parse()
            .ok()
    })
}

fn network_online() -> bool {
    "1.1.1.1:53"
        .parse::<SocketAddr>()
        .ok()
        .and_then(|address| TcpStream::connect_timeout(&address, Duration::from_secs(2)).ok())
        .is_some()
}

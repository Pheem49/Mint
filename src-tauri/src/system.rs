use std::process::{Command, Stdio};

use mint_core::load_config;
use serde::Serialize;
use serde_json::{Value, json};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SmartContext {
    pub captured_at: String,
    pub platform: &'static str,
    pub host: String,
    pub active_window: Option<Value>,
    pub current_app: Option<Value>,
    pub browser: Option<Value>,
    pub selected_text: String,
}

pub async fn smart_context() -> SmartContext {
    let active_window = linux_active_window();
    let current_app = active_window.as_ref().map(|window| {
        json!({
            "name": window["appName"],
            "processName": window["processName"],
            "pid": window["pid"]
        })
    });
    SmartContext {
        captured_at: unix_timestamp().to_string(),
        platform: std::env::consts::OS,
        host: hostname(),
        browser: browser_context(&active_window).await,
        active_window,
        current_app,
        selected_text: selected_text(),
    }
}

async fn browser_context(active_window: &Option<Value>) -> Option<Value> {
    let is_chromium = active_window
        .as_ref()
        .and_then(|window| window["appName"].as_str())
        .map(str::to_ascii_lowercase)
        .is_some_and(|name| {
            ["chrome", "chromium", "brave", "edge"]
                .iter()
                .any(|browser| name.contains(browser))
        });
    if is_chromium && let Some(context) = chromium_context().await {
        return Some(context);
    }
    browser_extension_context().await
}

async fn chromium_context() -> Option<Value> {
    let endpoint = load_config()
        .ok()
        .and_then(|config| {
            config
                .extra
                .get("browserDebugUrl")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "http://127.0.0.1:9222/json/list".into());
    let pages: Value = mint_core::HTTP_CLIENT
        .clone()
        .get(endpoint)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    pages
        .as_array()?
        .iter()
        .find(|page| page["type"] == "page")
        .map(|page| {
            json!({
                "title": page["title"],
                "url": page["url"],
                "source": "chromium-remote-debug"
            })
        })
}

async fn browser_extension_context() -> Option<Value> {
    let endpoint = load_config()
        .ok()
        .and_then(|config| {
            config
                .extra
                .get("browserExtensionContextUrl")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| "http://127.0.0.1:3212/context".into());
    if !(endpoint.starts_with("http://127.0.0.1:") || endpoint.starts_with("http://localhost:")) {
        return None;
    }
    let context: Value = mint_core::HTTP_CLIENT
        .clone()
        .get(endpoint)
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;
    Some(json!({
        "title": context["title"],
        "url": context["url"],
        "source": "browser-extension"
    }))
}

pub fn run_system_automation(target: &str, approved: bool) -> Result<String, String> {
    let (command, value) = target.split_once(':').unwrap_or((target, ""));
    match command {
        "volume" => {
            let value = percent(value)?;
            run_first(&[
                ("pactl", vec!["set-sink-volume", "@DEFAULT_SINK@", &value]),
                ("amixer", vec!["-D", "pulse", "sset", "Master", &value]),
            ])?;
            Ok(format!("Volume set to {value}"))
        }
        "mute" => {
            run_first(&[
                ("pactl", vec!["set-sink-mute", "@DEFAULT_SINK@", "toggle"]),
                ("amixer", vec!["-D", "pulse", "sset", "Master", "toggle"]),
            ])?;
            Ok("Volume toggled".into())
        }
        "brightness" => {
            let value = percent(value)?;
            run_first(&[
                ("brightnessctl", vec!["set", &value]),
                ("xbacklight", vec!["-set", value.trim_end_matches('%')]),
            ])?;
            Ok(format!("Brightness set to {value}"))
        }
        "minimize_all" => {
            run("xdotool", &["key", "Super+d"])?;
            Ok("Minimized all windows".into())
        }
        "sleep" => {
            run("systemctl", &["suspend"])?;
            Ok("Suspend requested".into())
        }
        "restart" | "shutdown" if !approved => {
            Err(format!("'{command}' requires explicit approval"))
        }
        "restart" => {
            run("systemctl", &["reboot"])?;
            Ok("Restart requested".into())
        }
        "shutdown" => {
            run("systemctl", &["poweroff"])?;
            Ok("Shutdown requested".into())
        }
        _ => Err(format!("unsupported system automation command '{command}'")),
    }
}

fn linux_active_window() -> Option<Value> {
    let id = output("xdotool", &["getactivewindow"])?;
    let title = output("xdotool", &["getwindowname", &id]).unwrap_or_default();
    let pid = output("xdotool", &["getwindowpid", &id]).unwrap_or_default();
    let process_name = output("ps", &["-p", &pid, "-o", "comm="]).unwrap_or_default();
    Some(json!({
        "id": id,
        "title": title,
        "appName": process_name,
        "processName": process_name,
        "pid": pid.parse::<u32>().ok(),
        "platform": "linux"
    }))
}

fn selected_text() -> String {
    [
        ("wl-paste", vec!["--primary", "--no-newline"]),
        ("xclip", vec!["-selection", "primary", "-out"]),
        ("xsel", vec!["--primary", "--output"]),
    ]
    .into_iter()
    .find_map(|(program, args)| output(program, &args))
    .unwrap_or_default()
    .chars()
    .take(2000)
    .collect()
}

fn hostname() -> String {
    output("hostname", &[]).unwrap_or_else(|| "unknown".into())
}

fn unix_timestamp() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn percent(raw: &str) -> Result<String, String> {
    raw.parse::<u8>()
        .ok()
        .filter(|value| *value <= 100)
        .map(|value| format!("{value}%"))
        .ok_or_else(|| "percentage must be between 0 and 100".into())
}

fn run_first(commands: &[(&str, Vec<&str>)]) -> Result<(), String> {
    let mut errors = Vec::new();
    for (program, args) in commands {
        match run(program, args) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    Err(errors.join(" | "))
}

fn run(program: &str, args: &[&str]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|error| format!("unable to run '{program}': {error}"))
        .and_then(|status| {
            status
                .success()
                .then_some(())
                .ok_or_else(|| format!("'{program}' exited with {status}"))
        })
}

fn output(program: &str, args: &[&str]) -> Option<String> {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .map(|output| String::from_utf8_lossy(&output.stdout).trim().to_owned())
        .filter(|output| !output.is_empty())
}

use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use mint_core::{config_path, load_config};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

pub fn workflows_path() -> Result<PathBuf, String> {
    Ok(config_path()
        .map_err(|error| error.to_string())?
        .with_file_name("workflows.json"))
}

pub fn load_workflows() -> Result<Vec<Value>, String> {
    let path = workflows_path()?;
    if !path.exists() {
        save_default_workflows(&path)?;
    }
    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("unable to read {}: {error}", path.display()))?;
    serde_json::from_str(&raw)
        .map_err(|error| format!("unable to parse {}: {error}", path.display()))
}

fn save_default_workflows(path: &PathBuf) -> Result<(), String> {
    let directory = path
        .parent()
        .ok_or_else(|| "workflow directory is unavailable".to_string())?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let workflows = json!([
        {
            "id": "wf-1",
            "name": "Check Mic on Zoom",
            "trigger": { "type": "process_running", "processName": "zoom" },
            "action": {
                "type": "system_info",
                "message": "Looks like you opened Zoom. Should I check your system resources?",
                "target": ""
            }
        },
        {
            "id": "wf-2",
            "name": "Coding Time",
            "trigger": { "type": "process_running", "processName": "code" },
            "action": {
                "type": "open_app",
                "message": "Coding time. Want me to open Spotify?",
                "target": "spotify"
            }
        }
    ]);
    let raw = serde_json::to_string_pretty(&workflows).map_err(|error| error.to_string())?;
    fs::write(path, format!("{raw}\n")).map_err(|error| error.to_string())
}

pub fn start_monitor(app: AppHandle) {
    thread::spawn(move || {
        let mut last_triggered = BTreeMap::<String, Instant>::new();
        loop {
            thread::sleep(Duration::from_secs(15));
            if load_config()
                .ok()
                .and_then(|config| config.extra.get("enableCustomWorkflows").cloned())
                .and_then(|value| value.as_bool())
                == Some(false)
            {
                continue;
            }
            let Ok(workflows) = load_workflows() else {
                continue;
            };
            let Ok(output) = Command::new("ps").args(["-A", "-o", "comm="]).output() else {
                continue;
            };
            let processes = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|line| line.trim().to_ascii_lowercase())
                .collect::<Vec<_>>();
            for workflow in workflows {
                let Some(id) = workflow["id"].as_str() else {
                    continue;
                };
                let Some(process_name) = workflow["trigger"]["processName"].as_str() else {
                    continue;
                };
                if !processes
                    .iter()
                    .any(|process| process == &process_name.to_ascii_lowercase())
                {
                    continue;
                }
                if last_triggered
                    .get(id)
                    .is_some_and(|last| last.elapsed() < Duration::from_secs(60 * 60))
                {
                    continue;
                }
                last_triggered.insert(id.into(), Instant::now());
                if let Some(main) = app.get_webview_window("main") {
                    let payload = json!({
                        "message": workflow["action"]["message"]
                            .as_str()
                            .unwrap_or("Automation workflow triggered"),
                        "suggestions": [
                            { "label": "Yes, please", "action": workflow["action"] },
                            { "label": "Dismiss", "action": { "type": "none" } }
                        ]
                    });
                    let _ = main.emit("proactive-suggestion", payload);
                }
            }
        }
    });
}

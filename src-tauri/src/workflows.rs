use std::{
    collections::BTreeMap,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use mint_core::{load_config, load_workflows};
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager};

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

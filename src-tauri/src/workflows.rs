use std::{
    collections::BTreeMap,
    process::Command,
    thread,
    time::{Duration, Instant},
};

use mint_core::{load_config, load_workflows, ChatRequest, send_chat};
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
                    let static_message = workflow["action"]["message"]
                        .as_str()
                        .unwrap_or("Automation workflow triggered");

                    let final_message = if let Ok(config) = load_config() {
                        let prompt = format!(
                            "You are Mint, a friendly desktop AI assistant. The user has triggered a workflow called '{}'. \
                            The original notification message is: '{}'. \
                            Please rewrite this notification message to be extremely friendly, cute, and natural. \
                            Write in the language matching the user's configured language: '{}'. \
                            Keep the length under 20 words and always end with a question (e.g. asking if they want to launch the action). \
                            Only reply with the rewritten message itself, do not include any quotes, markdown formatting, or extra explanations.",
                            workflow["name"].as_str().unwrap_or(""),
                            static_message,
                            config.language
                        );

                        let chat_req = ChatRequest {
                            message: prompt,
                            system_instruction: format!("You are Mint, a helpful desktop assistant. Write only the friendly suggestion in the language matching '{}'. No quotes, no markdown, no other text.", config.language),
                            chat_id: None,
                            image_data_uri: None,
                            audio_data_uri: None,
                            document_attachment: None,
                            workspace_path: None,
                        };

                        tauri::async_runtime::block_on(async {
                            match send_chat(&config, &chat_req).await {
                                Ok(response) => {
                                    let cleaned = response.text.trim().trim_matches('"').to_string();
                                    if !cleaned.is_empty() {
                                        cleaned
                                    } else {
                                        static_message.to_string()
                                    }
                                }
                                Err(err) => {
                                    eprintln!("Failed to generate dynamic suggestion via AI: {:?}", err);
                                    static_message.to_string()
                                }
                            }
                        })
                    } else {
                        static_message.to_string()
                    };

                    let payload = json!({
                        "message": final_message,
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

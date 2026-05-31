use std::{
    collections::BTreeMap,
    fs,
    path::PathBuf,
    sync::{
        LazyLock, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use mint_core::{MintConfig, config_path, load_config};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::desktop::capture_screen_bytes;

const DEFAULT_INTERVAL_SECONDS: u64 = 60;
const DEFAULT_COOLDOWN_SECONDS: u64 = 120;
const MAX_CONTEXT_HISTORY: usize = 20;
const PROMPT: &str = r#"You are Mint's desktop suggestion engine. Analyze the screenshot and return only JSON:
{"context":"short English description","message":"short Thai message or null","suggestions":[{"label":"1-3 words","action":{"type":"open_url|open_app|search|none","target":"..."}}]}
Return an empty suggestions array when there is no clear useful opportunity. Provide at most 4 suggestions. Never suggest shell commands."#;

static ENABLED: AtomicBool = AtomicBool::new(false);
static LAST_SUGGESTION: LazyLock<Mutex<Option<(Instant, String)>>> =
    LazyLock::new(|| Mutex::new(None));

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BehaviorMemory {
    #[serde(default)]
    app_frequency: BTreeMap<String, u64>,
    #[serde(default)]
    context_history: Vec<BehaviorEntry>,
    #[serde(default)]
    last_updated: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BehaviorEntry {
    context: String,
    time: String,
}

pub fn set_enabled(enabled: bool) {
    ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn record_behavior(context: &str) -> Result<(), String> {
    let context = context.trim();
    if context.is_empty() {
        return Ok(());
    }
    let mut memory = load_behavior()?;
    memory.context_history.insert(
        0,
        BehaviorEntry {
            context: context.into(),
            time: timestamp(),
        },
    );
    memory.context_history.truncate(MAX_CONTEXT_HISTORY);
    for app in [
        "YouTube", "Chrome", "Firefox", "VS Code", "Spotify", "Terminal", "Google", "Discord",
        "Slack", "Gmail", "GitHub", "Figma", "Notion",
    ] {
        if context
            .to_ascii_lowercase()
            .contains(&app.to_ascii_lowercase())
        {
            *memory.app_frequency.entry(app.into()).or_default() += 1;
        }
    }
    memory.last_updated = Some(timestamp());
    save_behavior(&memory)
}

pub fn start_loop(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        loop {
            if !ENABLED.load(Ordering::Relaxed) {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
            let config = match load_config() {
                Ok(config) => config,
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(DEFAULT_INTERVAL_SECONDS)).await;
                    continue;
                }
            };
            if let Ok(Some(suggestion)) = analyze(&config).await {
                if let Some(context) = suggestion["context"].as_str() {
                    let _ = record_behavior(context);
                }
                if let Some(main) = app.get_webview_window("main") {
                    let _ = main.emit("proactive-suggestion", suggestion);
                }
            }
            tokio::time::sleep(Duration::from_secs(config_u64(
                &config,
                "proactiveInterval",
                DEFAULT_INTERVAL_SECONDS,
            )))
            .await;
        }
    });
}

async fn analyze(config: &MintConfig) -> Result<Option<Value>, String> {
    let key = if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    };
    if key.trim().is_empty() || cooling_down(config) {
        return Ok(None);
    }
    let image = capture_screen_bytes()?;
    let response: Value = Client::new()
        .post(format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={key}",
            config.gemini_model
        ))
        .json(&json!({
            "systemInstruction": { "parts": [{ "text": PROMPT }] },
            "contents": [{ "role": "user", "parts": [
                { "text": format!("Behavior context: {}", behavior_summary()?) },
                { "inlineData": { "mimeType": "image/png", "data": STANDARD.encode(image) } }
            ]}],
            "generationConfig": { "responseMimeType": "application/json" }
        }))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let Some(raw) = response["candidates"][0]["content"]["parts"][0]["text"].as_str() else {
        return Ok(None);
    };
    let mut suggestion: Value = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    let has_message = suggestion["message"].as_str().is_some();
    let Some(items) = suggestion["suggestions"].as_array_mut() else {
        return Ok(None);
    };
    items.retain(|item| {
        item["action"]["type"]
            .as_str()
            .is_some_and(|kind| matches!(kind, "open_url" | "open_app" | "search" | "none"))
    });
    items.truncate(4);
    if !has_message || items.is_empty() {
        return Ok(None);
    }
    let context = suggestion["context"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let mut last = LAST_SUGGESTION.lock().map_err(|error| error.to_string())?;
    if last
        .as_ref()
        .is_some_and(|(_, previous)| previous == &context)
    {
        return Ok(None);
    }
    *last = Some((Instant::now(), context));
    Ok(Some(suggestion))
}

fn cooling_down(config: &MintConfig) -> bool {
    LAST_SUGGESTION
        .lock()
        .ok()
        .and_then(|value| value.as_ref().map(|(time, _)| time.elapsed()))
        .is_some_and(|elapsed| {
            elapsed
                < Duration::from_secs(config_u64(
                    config,
                    "proactiveCooldown",
                    DEFAULT_COOLDOWN_SECONDS,
                ))
        })
}

fn behavior_summary() -> Result<String, String> {
    let memory = load_behavior()?;
    let recent = memory
        .context_history
        .iter()
        .take(3)
        .map(|entry| entry.context.as_str())
        .collect::<Vec<_>>()
        .join(" | ");
    let mut apps = memory.app_frequency.into_iter().collect::<Vec<_>>();
    apps.sort_by(|left, right| right.1.cmp(&left.1));
    let apps = apps
        .into_iter()
        .take(5)
        .map(|(app, count)| format!("{app} ({count}x)"))
        .collect::<Vec<_>>()
        .join(", ");
    Ok(format!(
        "Frequent apps: {apps}. Recent activities: {recent}."
    ))
}

fn behavior_path() -> Result<PathBuf, String> {
    Ok(config_path()
        .map_err(|error| error.to_string())?
        .with_file_name("behavior_memory.json"))
}

fn load_behavior() -> Result<BehaviorMemory, String> {
    let path = behavior_path()?;
    if !path.exists() {
        return Ok(BehaviorMemory::default());
    }
    serde_json::from_str(&fs::read_to_string(&path).map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn save_behavior(memory: &BehaviorMemory) -> Result<(), String> {
    let path = behavior_path()?;
    let directory = path
        .parent()
        .ok_or_else(|| "behavior memory directory is unavailable".to_string())?;
    fs::create_dir_all(directory).map_err(|error| error.to_string())?;
    let raw = serde_json::to_string_pretty(memory).map_err(|error| error.to_string())?;
    fs::write(path, format!("{raw}\n")).map_err(|error| error.to_string())
}

fn config_u64(config: &MintConfig, key: &str, default: u64) -> u64 {
    config
        .extra
        .get(key)
        .and_then(Value::as_u64)
        .unwrap_or(default)
        .max(5)
}

fn timestamp() -> String {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}

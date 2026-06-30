mod browser;
mod desktop;
mod discord_rpc;
mod events;
mod headless;
mod integrations;
mod plugins;
mod proactive;
mod system;
mod updater;
mod webhooks;
mod workflows;


use browser::{
    BrowserTab, click as browser_click, list_tabs as browser_list_tabs,
    navigate as browser_navigate, read_page_text,
};

use desktop::{
    ActionResult, CaptureRect, DesktopAction, capture_screen, close_window, emit_to_main,
    execute_action, hide_window, integration_status, open_desktop_window, position_widget,
    resize_window, translate_screen_region,
};
use events::start_system_events;
use headless::{run_next_task, start_headless_queue};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tokio::sync::oneshot;

use integrations::{channel_inventory, list_plugins};
use mint_core::{
    AgentApproval, AgentProgress, AppliedCodeEdit, ApprovalOutcome, ChatRequest, ChatResponse,
    ChatSession, CodeEdit, CodeEditProposal, ImageGenRequest, InteractionMemory, MemoryStore,
    MintConfig, PictureEntry, TtsUrl, WeatherReport, apply_code_edits, classify_shell_command,
    config_path, google_tts_urls, list_saved_pictures, load_config,
    load_workflows, save_workflows, orchestrate_agent_loop, orchestrate_chat_stream_with_fallback,
    orchestrate_chat_with_fallback, propose_code_edits, save_chat_images, save_config,
    start_channels, weather, workflows_path,
};
use plugins::execute_plugin;

pub struct ApprovalsState {
    pub pending: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

static COUNTER: AtomicU64 = AtomicU64::new(1);
use proactive::{
    record_behavior, set_enabled as set_proactive_enabled, start_loop as start_proactive_loop,
};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use system::{SmartContext, smart_context};
use tauri::{
    AppHandle, Emitter, Manager,
    ipc::Channel,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};
use updater::{
    AvailableUpdate, UpdateChannelStatus, check as check_update, install as install_update,
    status as updater_status,
};
use webhooks::start_webhooks;
use workflows::start_monitor;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    backend: &'static str,
    config_path: String,
    active_provider: String,
    available_providers: Vec<String>,
    integrations: Value,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceTreeEntry {
    name: String,
    path: String,
    kind: &'static str,
    children: Vec<WorkspaceTreeEntry>,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum DesktopStreamEvent {
    Chunk { chunk: String },
    Progress { progress: AgentProgress },
}
const WORKSPACE_TREE_MAX_DEPTH: usize = 9;
const WORKSPACE_TREE_MAX_CHILDREN: usize = 400;
const WORKSPACE_TREE_COLLAPSED_DIRS: &[&str] = &[
    ".antigravitycli",
    ".cargo_home",
    ".git",
    ".rustup",
    ".rustup_copy",
    ".rustup_home",
    "build",
    "coverage",
    "dist",
    "node_modules",
    "out",
    "target",
];



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
async fn get_workspace_tree(path: Option<String>) -> Result<WorkspaceTreeEntry, String> {
    tokio::task::spawn_blocking(move || build_workspace_tree(path))
        .await
        .map_err(|error| format!("workspace tree task failed: {error}"))?
}

fn build_workspace_tree(path: Option<String>) -> Result<WorkspaceTreeEntry, String> {
    let root = workspace_root(path.as_deref())?;
    let name = root
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string());
    Ok(WorkspaceTreeEntry {
        name,
        path: root.display().to_string(),
        kind: "directory",
        children: workspace_children(&root, &root, 0)?,
    })
}

#[tauri::command]
async fn select_workspace_directory() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(select_workspace_directory_blocking)
        .await
        .map_err(|error| format!("workspace picker task failed: {error}"))?
}

fn select_workspace_directory_blocking() -> Result<Option<String>, String> {
    for (program, args) in [
        (
            "zenity",
            vec!["--file-selection", "--directory", "--title=Select Project"],
        ),
        ("kdialog", vec!["--getexistingdirectory", "."]),
    ] {
        let Ok(output) = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            return Ok(None);
        }
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            return Ok(None);
        }
        return Ok(Some(workspace_root(Some(&selected))?.display().to_string()));
    }
    Ok(None)
}

fn workspace_root(path: Option<&str>) -> Result<PathBuf, String> {
    let root = match path.map(str::trim).filter(|path| !path.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err(format!("workspace is not a directory: {}", root.display()));
    }
    Ok(root)
}

fn workspace_children(
    root: &Path,
    directory: &Path,
    depth: usize,
) -> Result<Vec<WorkspaceTreeEntry>, String> {
    if depth >= WORKSPACE_TREE_MAX_DEPTH {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .flatten()
        .filter_map(|entry| {
            let file_type = entry.file_type().ok()?;
            if file_type.is_symlink() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            Some((name, entry.path(), file_type.is_dir()))
        })
        .collect::<Vec<_>>();

    entries.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));
    entries.truncate(WORKSPACE_TREE_MAX_CHILDREN);

    entries
        .into_iter()
        .map(|(name, path, is_dir)| {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .to_string();
            let children = if is_dir && !WORKSPACE_TREE_COLLAPSED_DIRS.contains(&name.as_str()) {
                workspace_children(root, &path, depth + 1)?
            } else {
                Vec::new()
            };
            Ok(WorkspaceTreeEntry {
                name,
                path: relative,
                kind: if is_dir { "directory" } else { "file" },
                children,
            })
        })
        .collect()
}

#[tauri::command]
fn get_config() -> Result<MintConfig, String> {
    load_config().map_err(|error| error.to_string())
}

#[tauri::command]
fn get_updater_status() -> Result<UpdateChannelStatus, String> {
    Ok(updater_status(
        &load_config().map_err(|error| error.to_string())?,
    ))
}

#[tauri::command]
async fn check_for_updates(app: AppHandle) -> Result<AvailableUpdate, String> {
    check_update(&app, &load_config().map_err(|error| error.to_string())?).await
}

#[tauri::command]
async fn install_available_update(app: AppHandle, approved: bool) -> Result<String, String> {
    install_update(
        &app,
        &load_config().map_err(|error| error.to_string())?,
        approved,
    )
    .await
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
async fn send_chat_message(app: AppHandle, request: ChatRequest) -> Result<ChatResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    let request = request.with_document_context(&config)?;

    if request.message.starts_with("/chat ") {
        let mut clean_request = request.clone();
        clean_request.message = request.message.strip_prefix("/chat ").unwrap().to_owned();

        let (response, _) = orchestrate_chat_with_fallback(&config, &clean_request)
            .await
            .map_err(|error| error.to_string())?;
        return Ok(response);
    }

    let root = workspace_root(request.workspace_path.as_deref())?;
    let fast_mode = config
        .extra
        .get("enableFastMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let app_clone = app.clone();
    let approve_cb = move |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
        let (tx, rx) = oneshot::channel();
        let token = format!("tok-{}", COUNTER.fetch_add(1, Ordering::SeqCst));

        let state = app_clone.state::<ApprovalsState>();
        state.pending.lock().unwrap().insert(token.clone(), tx);

        app_clone
            .emit(
                "tool-approval-requested",
                serde_json::json!({
                    "token": token,
                    "approval": approval
                }),
            )
            .map_err(|e| e.to_string())?;

        let approved =
            tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(rx))
                .unwrap_or(false);

        if approved {
            Ok(ApprovalOutcome::Approved)
        } else {
            Ok(ApprovalOutcome::Denied)
        }
    };

    let progress_cb = |_| {};
    let on_chunk = |_| {};

    let res = orchestrate_agent_loop(
        &config,
        &request.message,
        &root,
        request.image_data_uri.clone(),
        request.chat_id.as_deref(),
        fast_mode,
        approve_cb,
        progress_cb,
        on_chunk,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(ChatResponse {
        provider: res.provider,
        model: res.model,
        text: res.summary,
        fallback_provider: res.fallback,
    })
}

#[tauri::command]
async fn stream_chat_message(
    app: AppHandle,
    request: ChatRequest,
    on_event: Channel<DesktopStreamEvent>,
) -> Result<ChatResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    let request = request.with_document_context(&config)?;

    if request.message.starts_with("/chat ") {
        let mut clean_request = request.clone();
        clean_request.message = request.message.strip_prefix("/chat ").unwrap().to_owned();

        let (response, _) =
            orchestrate_chat_stream_with_fallback(&config, &clean_request, |chunk| {
                let _ = on_event.send(DesktopStreamEvent::Chunk { chunk });
            })
            .await
            .map_err(|error| error.to_string())?;
        return Ok(response);
    }

    let root = workspace_root(request.workspace_path.as_deref())?;
    let fast_mode = config
        .extra
        .get("enableFastMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);

    let app_clone = app.clone();
    let approve_cb = move |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
        let (tx, rx) = oneshot::channel();
        let token = format!("tok-{}", COUNTER.fetch_add(1, Ordering::SeqCst));

        let state = app_clone.state::<ApprovalsState>();
        state.pending.lock().unwrap().insert(token.clone(), tx);

        app_clone
            .emit(
                "tool-approval-requested",
                serde_json::json!({
                    "token": token,
                    "approval": approval
                }),
            )
            .map_err(|e| e.to_string())?;

        let approved =
            tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(rx))
                .unwrap_or(false);

        if approved {
            Ok(ApprovalOutcome::Approved)
        } else {
            Ok(ApprovalOutcome::Denied)
        }
    };

    let on_progress_event = on_event.clone();
    let progress_cb = move |progress| {
        let _ = on_progress_event.send(DesktopStreamEvent::Progress { progress });
    };

    let on_event_clone = on_event.clone();
    let on_chunk = move |summary: String| {
        let chars: Vec<char> = summary.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let end = (i + 4).min(chars.len());
            let chunk: String = chars[i..end].iter().collect();
            let _ = on_event_clone.send(DesktopStreamEvent::Chunk { chunk });
            i = end;
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
    };

    let res = orchestrate_agent_loop(
        &config,
        &request.message,
        &root,
        request.image_data_uri.clone(),
        request.chat_id.as_deref(),
        fast_mode,
        approve_cb,
        progress_cb,
        on_chunk,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(ChatResponse {
        provider: res.provider,
        model: res.model,
        text: res.summary,
        fallback_provider: res.fallback,
    })
}

#[tauri::command]
fn save_interaction_agent_activity(
    interaction_id: i64,
    activity: Vec<AgentProgress>,
) -> Result<(), String> {
    let activity_json = serde_json::to_string(&activity).map_err(|error| error.to_string())?;
    MemoryStore::open_default()
        .and_then(|memory| {
            memory
                .set_interaction_agent_activity_json(interaction_id, &activity_json)
                .map(|_| ())
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn save_system_interaction(
    chat_id: String,
    user_text: String,
    provider: String,
    model: String,
) -> Result<i64, String> {
    MemoryStore::open_default()
        .and_then(|memory| {
            memory.add_interaction_for_chat(&chat_id, &user_text, "", &provider, &model)
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_recent_interactions(
    limit: Option<usize>,
    chat_id: Option<String>,
) -> Result<Vec<InteractionMemory>, String> {
    MemoryStore::open_default()
        .and_then(|memory| {
            memory.recent_interactions_for_chat(
                chat_id
                    .as_deref()
                    .unwrap_or(mint_core::DEFAULT_CONVERSATION_ID),
                limit.unwrap_or(5),
            )
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_chat_sessions() -> Result<Vec<ChatSession>, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.list_chat_sessions())
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_chat_session(chat_id: String) -> Result<usize, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.delete_chat_session(&chat_id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn rename_chat_session(chat_id: String, new_title: String) -> Result<usize, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.rename_chat_session(&chat_id, &new_title))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_profile_value(key: String) -> Result<Option<String>, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.get_profile(&key))
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn set_profile_value(key: String, value: String) -> Result<(), String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.set_profile(&key, &value))
        .map_err(|error| error.to_string())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LearnedSkillDto {
    id: i64,
    name: String,
    source_path: String,
    content: String,
    updated_at: String,
}

#[tauri::command]
fn list_learned_skills() -> Result<Vec<LearnedSkillDto>, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    let skills = store.learned_skills(100).map_err(|e| e.to_string())?;
    let dtos = skills
        .into_iter()
        .map(|s| LearnedSkillDto {
            id: s.id,
            name: s.name,
            source_path: s.source_path,
            content: s.content,
            updated_at: s.created_at,
        })
        .collect();
    Ok(dtos)
}

#[tauri::command]
fn add_learned_skill(name: String, content: String) -> Result<LearnedSkillDto, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    let skill = store
        .add_learned_skill(&name, "ui_manual", &content)
        .map_err(|e| e.to_string())?;
    Ok(LearnedSkillDto {
        id: skill.id,
        name: skill.name,
        source_path: skill.source_path,
        content: skill.content,
        updated_at: skill.created_at,
    })
}

#[tauri::command]
fn delete_learned_skill(name: String) -> Result<usize, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    store.delete_learned_skill(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn clear_chat_history(chat_id: Option<String>) -> Result<usize, String> {
    MemoryStore::open_default()
        .and_then(|memory| {
            memory.clear_interactions_for_chat(
                chat_id
                    .as_deref()
                    .unwrap_or(mint_core::DEFAULT_CONVERSATION_ID),
            )
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn submit_tool_approval(
    state: tauri::State<'_, ApprovalsState>,
    token: String,
    approved: bool,
) -> Result<(), String> {
    let mut pending = state.pending.lock().map_err(|error| error.to_string())?;
    if let Some(tx) = pending.remove(&token) {
        let _ = tx.send(approved);
        Ok(())
    } else {
        Err("No pending approval found for this token".into())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct DesktopImageGenResponse {
    images: Vec<PictureEntry>,
    model: String,
    provider: String,
    prompt: String,
    description: Option<String>,
}

#[tauri::command]
async fn generate_images(request: ImageGenRequest) -> Result<DesktopImageGenResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;

    // Call core generate_images logic
    let result = mint_core::generate_images(&config, &request)
        .await
        .map_err(|error| error.to_string())?;

    // Save images to Pictures library (just like api_server does)
    let data_uris: Vec<String> = result
        .images
        .iter()
        .map(|img| img.data_uri.clone())
        .collect();
    let saved = save_chat_images(
        data_uris,
        Some("nanobanana".into()),
        Some(request.prompt.clone()),
    )
    .map_err(|error| error.to_string())?;

    Ok(DesktopImageGenResponse {
        images: saved,
        model: result.model,
        provider: result.provider,
        prompt: result.prompt,
        description: result.description,
    })
}

#[tauri::command]
fn list_pictures() -> Result<Vec<PictureEntry>, String> {
    list_saved_pictures().map_err(|error| error.to_string())
}

#[tauri::command]
fn save_pictures(
    images: Vec<String>,
    source: Option<String>,
    message: Option<String>,
) -> Result<Vec<PictureEntry>, String> {
    save_chat_images(images, source, message).map_err(|error| error.to_string())
}

#[tauri::command]
fn open_folder(path: String) -> Result<ActionResult, String> {
    let target_path = PathBuf::from(path.trim());
    if target_path.as_os_str().is_empty() {
        return Err("folder path is required".into());
    }

    let folder = if target_path.is_dir() {
        target_path
    } else {
        target_path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "could not resolve containing folder".to_string())?
    };

    Command::new("xdg-open")
        .arg(&folder)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(ActionResult {
        success: true,
        message: format!("opened {}", folder.display()),
    })
}

#[tauri::command]
fn get_tts_urls(text: String) -> Result<Vec<TtsUrl>, String> {
    let language = load_config().map_err(|error| error.to_string())?.language;
    Ok(google_tts_urls(&text, &language))
}

#[tauri::command]
async fn get_weather(city: String) -> Result<WeatherReport, String> {
    weather(&city).await.map_err(|error| error.to_string())
}

#[tauri::command]
fn propose_desktop_code_edits(
    root: String,
    edits: Vec<CodeEdit>,
) -> Result<CodeEditProposal, String> {
    propose_code_edits(
        std::path::Path::new(&root),
        &edits,
        &load_config().map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
fn apply_desktop_code_edits(
    root: String,
    edits: Vec<CodeEdit>,
    approval_token: String,
) -> Result<Vec<AppliedCodeEdit>, String> {
    apply_code_edits(
        std::path::Path::new(&root),
        &edits,
        &approval_token,
        &load_config().map_err(|error| error.to_string())?,
    )
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
        "mcpServers": mint_core::configured_mcp_servers(&config)
            .map_err(|error| error.to_string())?
            .keys()
            .collect::<Vec<_>>(),
        "plugins": list_plugins(&config),
        "channels": channel_inventory(&config)
    }))
}

#[tauri::command]
async fn run_native_plugin(name: String, instruction: String) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    execute_plugin(&config, &name, &instruction).await
}

#[tauri::command]
fn capture_silent_screen() -> Result<String, String> {
    capture_screen()
}

#[tauri::command]
fn read_clipboard_image() -> Result<String, String> {
    desktop::read_clipboard_image()
}

#[tauri::command]
async fn translate_capture_region(rect: CaptureRect) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    translate_screen_region(&config, rect).await
}

#[tauri::command]
async fn get_smart_context() -> SmartContext {
    smart_context().await
}

#[tauri::command]
async fn get_browser_tabs() -> Result<Vec<BrowserTab>, String> {
    browser_list_tabs(&load_config().map_err(|error| error.to_string())?).await
}

#[tauri::command]
async fn navigate_browser(url: String) -> Result<String, String> {
    browser_navigate(&load_config().map_err(|error| error.to_string())?, &url).await
}

#[tauri::command]
async fn read_browser_page() -> Result<String, String> {
    read_page_text(&load_config().map_err(|error| error.to_string())?).await
}

#[tauri::command]
async fn click_browser_selector(selector: String) -> Result<String, String> {
    browser_click(
        &load_config().map_err(|error| error.to_string())?,
        &selector,
    )
    .await
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
fn toggle_proactive(enabled: bool) {
    set_proactive_enabled(enabled);
}

#[tauri::command]
fn save_behavior_context(context: String) -> Result<(), String> {
    record_behavior(&context)
}

#[tauri::command]
async fn run_next_queued_task(app: AppHandle) -> Result<Option<mint_core::Task>, String> {
    run_next_task(&app).await
}

#[tauri::command]
fn exit_app(app: AppHandle) {
    app.exit(0);
}

#[tauri::command]
fn open_workflows_file() -> Result<ActionResult, String> {
    load_workflows().map_err(|error| error.to_string())?;
    let path = workflows_path().map_err(|error| error.to_string())?;
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
    let workflows = load_workflows().map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "success": true,
        "count": workflows.len(),
        "workflows": workflows
    }))
}

#[tauri::command]
fn save_custom_workflows(workflows: Vec<serde_json::Value>) -> Result<ActionResult, String> {
    save_workflows(&workflows).map_err(|error| error.to_string())?;
    Ok(ActionResult {
        success: true,
        message: "workflows saved successfully".into(),
    })
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

fn install_shortcuts(app: &AppHandle) -> tauri::Result<()> {
    let main_shortcut = Shortcut::new(Some(Modifiers::CONTROL | Modifiers::SHIFT), Code::Space);
    let spotlight_shortcut = Shortcut::new(Some(Modifiers::ALT), Code::Space);
    let main_handler = main_shortcut.clone();
    let spotlight_handler = spotlight_shortcut.clone();
    app.plugin(
        tauri_plugin_global_shortcut::Builder::new()
            .with_handler(move |app, shortcut, event| {
                if event.state() != ShortcutState::Pressed {
                    return;
                }
                if shortcut == &main_handler {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = if window.is_visible().unwrap_or(false) {
                            window.hide()
                        } else {
                            window.show()
                        };
                    }
                } else if shortcut == &spotlight_handler {
                    let _ = open_desktop_window(app, "spotlight");
                }
            })
            .build(),
    )?;
    let _ = app.global_shortcut().register(main_shortcut);
    let _ = app.global_shortcut().register(spotlight_shortcut);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ApprovalsState {
            pending: Mutex::new(HashMap::new()),
        })
        .setup(|app| {
            install_tray(app.handle())?;
            install_shortcuts(app.handle())?;
            start_monitor(app.handle().clone());
            start_system_events(app.handle().clone());
            start_headless_queue(app.handle().clone());
            start_proactive_loop(app.handle().clone());
            start_channels();
            start_webhooks();
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
            get_workspace_tree,
            generate_images,
            select_workspace_directory,
            get_config,
            get_updater_status,
            check_for_updates,
            install_available_update,
            update_config,
            inspect_shell_command,
            send_chat_message,
            stream_chat_message,
            submit_tool_approval,
            get_recent_interactions,
            save_interaction_agent_activity,
            list_chat_sessions,
            delete_chat_session,
            rename_chat_session,
            get_profile_value,
            set_profile_value,
            clear_chat_history,
            list_learned_skills,
            add_learned_skill,
            delete_learned_skill,
            list_pictures,
            save_pictures,
            open_folder,
            get_tts_urls,
            get_weather,
            propose_desktop_code_edits,
            apply_desktop_code_edits,
            open_window,
            hide_desktop_window,
            close_desktop_window,
            resize_desktop_window,
            run_desktop_action,
            get_integration_inventory,
            run_native_plugin,
            capture_silent_screen,
            read_clipboard_image,
            translate_capture_region,
            get_smart_context,
            get_browser_tabs,
            navigate_browser,
            read_browser_page,
            click_browser_selector,
            start_screen_capture,
            submit_screen_selection,
            submit_spotlight,
            set_ai_state,
            toggle_proactive,
            save_behavior_context,
            run_next_queued_task,
            exit_app,
            open_workflows_file,
            reload_custom_workflows,
            save_custom_workflows,
            save_system_interaction
        ])
        .run(tauri::generate_context!())
        .expect("error while running Mint desktop");
}

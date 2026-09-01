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

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mint_core::browser::{
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
    AgentApproval, AgentProgress, AppliedCodeEdit, ApprovalOutcome, AuthUser, ChatRequest,
    ChatResponse, ChatSession, CodeEdit, CodeEditProposal, CronJob, CronJobDraft, CronStore,
    GeminiLiveEvent, GeminiLiveHandle, ImageGenRequest, InteractionMemory, LinkedFolder,
    LinkedFolderDraft, MemoryStore, MicRecordingHandle, MintConfig, PictureEntry,
    SubagentDefinition, SubagentDraft, TtsUrl, VideoGenRequest, VideoGenResponse, WeatherReport,
    apply_code_edits, classify_shell_command, config_path, delete_saved_picture,
    delete_subagent as core_delete_subagent, get_user, google_tts_urls, list_saved_pictures,
    list_subagents as core_list_subagents, load_config, login_user, orchestrate_agent_loop,
    orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback, propose_code_edits,
    reauth_mcp_server as core_reauth_mcp_server, register_user, save_avatar_file, save_chat_images,
    save_config, save_subagent as core_save_subagent, start_channels, start_cron_scheduler,
    start_gemini_live_session as core_start_gemini_live_session,
    start_recording as core_start_mic_recording, stop_recording as core_stop_mic_recording,
    transcribe_recording as core_transcribe_mic_recording, update_profile, weather,
};
use plugins::execute_plugin;

pub struct ApprovalsState {
    pub pending: Mutex<HashMap<String, oneshot::Sender<ApprovalOutcome>>>,
}

#[derive(Default)]
pub struct GeminiLiveState {
    pub sessions: Mutex<HashMap<String, GeminiLiveHandle>>,
}

#[derive(Default)]
pub struct MicRecordingState {
    pub active: Mutex<Option<MicRecordingHandle>>,
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct RuntimeStatus {
    backend: &'static str,
    config_path: String,
    active_provider: String,
    active_model: String,
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
        active_model: config.active_model().to_string(),
        available_providers: config.available_providers(),
        integrations: integration_status(&config),
    })
}

#[tauri::command]
async fn get_workspace_tree(path: Option<String>) -> Result<WorkspaceTreeEntry, String> {
    tokio::task::spawn_blocking(move || build_workspace_tree(path))
        .await
        .map_err(|error| format!("workspace tree task failed: {error}"))?
}

/// Re-runs a configured MCP server's OAuth login in the foreground (fixes an
/// expired/invalid refresh token, e.g. `invalid_grant` from a Gmail MCP
/// server). The underlying core call is blocking (spawns a child process and
/// waits on it while streaming its output), so it runs on a blocking thread
/// pool task rather than the async runtime.
#[tauri::command]
async fn reauth_mcp_server(server_name: String) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || core_reauth_mcp_server(&server_name))
        .await
        .map_err(|error| format!("reauth task failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn list_mcp_server_tools(name: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || mint_core::mcp_server_tool_names(&name))
        .await
        .map_err(|error| format!("list-tools task failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
async fn create_workspace_file(path: String) -> Result<(), String> {
    std::fs::write(&path, "").map_err(|error| error.to_string())
}

#[tauri::command]
async fn create_workspace_folder(path: String) -> Result<(), String> {
    std::fs::create_dir_all(&path).map_err(|error| error.to_string())
}

#[tauri::command]
async fn delete_workspace_item(path: String) -> Result<(), String> {
    let path_buf = std::path::PathBuf::from(path);
    if path_buf.is_dir() {
        std::fs::remove_dir_all(path_buf).map_err(|error| error.to_string())
    } else {
        std::fs::remove_file(path_buf).map_err(|error| error.to_string())
    }
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
    let root = if root.ends_with("src-tauri") {
        root.parent().unwrap_or(&root).to_path_buf()
    } else {
        root
    };
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
fn set_active_model(
    app: AppHandle,
    provider: String,
    model: Option<String>,
) -> Result<String, String> {
    let mut config = load_config().map_err(|error| error.to_string())?;
    let display_name = config
        .set_active_model(&provider, model.as_deref())
        .map_err(|error| error.to_string())?;
    let _ = app.emit("settings-changed", &config);
    Ok(display_name)
}

#[tauri::command]
fn inspect_shell_command(command: String) -> mint_core::ShellClassification {
    classify_shell_command(&command)
}

#[tauri::command]
async fn send_chat_message(app: AppHandle, request: ChatRequest) -> Result<ChatResponse, String> {
    let mut config = load_config().map_err(|error| error.to_string())?;
    if let Some(ref path) = request.workspace_path {
        if !path.trim().is_empty()
            && config
                .extra
                .get("activeWorkspacePath")
                .and_then(Value::as_str)
                != Some(path.as_str())
        {
            config
                .extra
                .insert("activeWorkspacePath".into(), Value::String(path.clone()));
            let _ = save_config(&config);
        }
    }
    let request = request.with_document_context(&config)?;

    if request.message.starts_with("/chat ") {
        let mut clean_request = request.clone();
        clean_request.message = request.message.strip_prefix("/chat ").unwrap().to_owned();
        let config_clone = config.clone();
        let chat_id_str = request.chat_id.clone().unwrap_or_default();

        let join_handle = tokio::spawn(async move {
            orchestrate_chat_with_fallback(&config_clone, &clean_request).await
        });

        let abort_handle = join_handle.abort_handle();
        if !chat_id_str.is_empty() {
            mint_core::ACTIVE_AGENTS
                .lock()
                .unwrap()
                .insert(chat_id_str.clone(), abort_handle);
        }

        let res = join_handle.await;

        if !chat_id_str.is_empty() {
            mint_core::ACTIVE_AGENTS
                .lock()
                .unwrap()
                .remove(&chat_id_str);
        }

        let (response, _) = match res {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => return Err(e.to_string()),
            Err(e) if e.is_cancelled() => {
                return Err("Chat execution cancelled by user".to_string());
            }
            Err(e) => return Err(format!("Task panicked: {}", e)),
        };
        return Ok(response);
    }

    let root = workspace_root(request.workspace_path.as_deref())?;
    let fast_mode = config
        .extra
        .get("enableFastMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let plan_mode = request.plan_mode;

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

        let outcome =
            tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(rx))
                .unwrap_or(ApprovalOutcome::Denied);
        Ok(outcome)
    };

    let progress_cb = |_| {};
    let on_chunk = |_| {};

    let chat_id_str = request.chat_id.clone().unwrap_or_default();
    let config_clone = config.clone();
    let message_clone = request.message.clone();
    let root_clone = root.clone();
    let image_data_uri_clone = request.image_data_uri.clone();
    let audio_data_uri_clone = request.audio_data_uri.clone();
    let video_data_uri_clone = request.video_data_uri.clone();
    let chat_id_clone = request.chat_id.clone();
    let agent_id_clone = request.agent_id.clone();
    let pinned_mcp_server_clone = request.pinned_mcp_server.clone();

    let join_handle = tokio::spawn(async move {
        orchestrate_agent_loop(
            &config_clone,
            &message_clone,
            &root_clone,
            image_data_uri_clone,
            audio_data_uri_clone,
            video_data_uri_clone,
            chat_id_clone.as_deref(),
            agent_id_clone.as_deref(),
            None,
            pinned_mcp_server_clone.as_deref(),
            fast_mode,
            plan_mode,
            approve_cb,
            progress_cb,
            on_chunk,
        )
        .await
    });

    let abort_handle = join_handle.abort_handle();
    if !chat_id_str.is_empty() {
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .insert(chat_id_str.clone(), abort_handle);
    }

    let res = join_handle.await;

    if !chat_id_str.is_empty() {
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .remove(&chat_id_str);
    }

    let res = match res {
        Ok(Ok(val)) => val,
        Ok(Err(e)) => return Err(e.to_string()),
        Err(e) if e.is_cancelled() => return Err("Agent execution cancelled by user".to_string()),
        Err(e) => return Err(format!("Task panicked: {}", e)),
    };

    Ok(ChatResponse {
        provider: res.provider,
        model: res.model,
        text: res.summary,
        fallback_provider: res.fallback,
        fallback_reason: None,
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
    })
}

#[tauri::command]
async fn stream_chat_message(
    app: AppHandle,
    request: ChatRequest,
    on_event: Channel<DesktopStreamEvent>,
) -> Result<ChatResponse, String> {
    let mut config = load_config().map_err(|error| error.to_string())?;
    if let Some(ref path) = request.workspace_path {
        if !path.trim().is_empty()
            && config
                .extra
                .get("activeWorkspacePath")
                .and_then(Value::as_str)
                != Some(path.as_str())
        {
            config
                .extra
                .insert("activeWorkspacePath".into(), Value::String(path.clone()));
            let _ = save_config(&config);
        }
    }
    let request = request.with_document_context(&config)?;

    if request.message.starts_with("/chat ") {
        let mut clean_request = request.clone();
        clean_request.message = request.message.strip_prefix("/chat ").unwrap().to_owned();
        let config_clone = config.clone();
        let on_event_clone = on_event.clone();
        let chat_id_str = request.chat_id.clone().unwrap_or_default();

        let join_handle = tokio::spawn(async move {
            orchestrate_chat_stream_with_fallback(&config_clone, &clean_request, move |chunk| {
                let _ = on_event_clone.send(DesktopStreamEvent::Chunk { chunk });
            })
            .await
        });

        let abort_handle = join_handle.abort_handle();
        if !chat_id_str.is_empty() {
            mint_core::ACTIVE_AGENTS
                .lock()
                .unwrap()
                .insert(chat_id_str.clone(), abort_handle);
        }

        let res = join_handle.await;

        if !chat_id_str.is_empty() {
            mint_core::ACTIVE_AGENTS
                .lock()
                .unwrap()
                .remove(&chat_id_str);
        }

        let (response, _) = match res {
            Ok(Ok(val)) => val,
            Ok(Err(e)) => return Err(e.to_string()),
            Err(e) if e.is_cancelled() => {
                return Err("Chat execution cancelled by user".to_string());
            }
            Err(e) => return Err(format!("Task panicked: {}", e)),
        };
        return Ok(response);
    }

    let root = workspace_root(request.workspace_path.as_deref())?;
    let fast_mode = config
        .extra
        .get("enableFastMode")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let plan_mode = request.plan_mode;

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

        let outcome =
            tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(rx))
                .unwrap_or(ApprovalOutcome::Denied);
        Ok(outcome)
    };

    let on_progress_event = on_event.clone();
    let avatar_app = app.clone();
    let progress_cb = move |progress| {
        avatar_app
            .state::<mint_core::avatar_bridge::AvatarBridge>()
            .on_agent_progress(&progress);
        let _ = on_progress_event.send(DesktopStreamEvent::Progress { progress });
    };

    let on_event_clone = on_event.clone();
    let chunk_app = app.clone();
    let on_chunk = move |summary: String| {
        let bridge = chunk_app.state::<mint_core::avatar_bridge::AvatarBridge>();
        bridge.on_talking(true);
        let chars: Vec<char> = summary.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let end = (i + 4).min(chars.len());
            let chunk: String = chars[i..end].iter().collect();
            let _ = on_event_clone.send(DesktopStreamEvent::Chunk { chunk });
            i = end;
            std::thread::sleep(std::time::Duration::from_millis(15));
        }
        bridge.on_talking(false);
    };

    let chat_id_str = request.chat_id.clone().unwrap_or_default();
    let config_clone = config.clone();
    let message_clone = request.message.clone();
    let root_clone = root.clone();
    let image_data_uri_clone = request.image_data_uri.clone();
    let audio_data_uri_clone = request.audio_data_uri.clone();
    let video_data_uri_clone = request.video_data_uri.clone();
    let chat_id_clone = request.chat_id.clone();
    let agent_id_clone = request.agent_id.clone();
    let pinned_mcp_server_clone = request.pinned_mcp_server.clone();

    let join_handle = tokio::spawn(async move {
        orchestrate_agent_loop(
            &config_clone,
            &message_clone,
            &root_clone,
            image_data_uri_clone,
            audio_data_uri_clone,
            video_data_uri_clone,
            chat_id_clone.as_deref(),
            agent_id_clone.as_deref(),
            None,
            pinned_mcp_server_clone.as_deref(),
            fast_mode,
            plan_mode,
            approve_cb,
            progress_cb,
            on_chunk,
        )
        .await
    });

    let abort_handle = join_handle.abort_handle();
    if !chat_id_str.is_empty() {
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .insert(chat_id_str.clone(), abort_handle);
    }

    let res = join_handle.await;

    if !chat_id_str.is_empty() {
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .remove(&chat_id_str);
    }

    let avatar_bridge = app.state::<mint_core::avatar_bridge::AvatarBridge>();
    let res = match res {
        Ok(Ok(val)) => {
            avatar_bridge.on_turn_end(true);
            val
        }
        Ok(Err(e)) => {
            avatar_bridge.on_turn_end(false);
            return Err(e.to_string());
        }
        Err(e) if e.is_cancelled() => {
            avatar_bridge.on_turn_end(false);
            return Err("Agent execution cancelled by user".to_string());
        }
        Err(e) => {
            avatar_bridge.on_turn_end(false);
            return Err(format!("Task panicked: {}", e));
        }
    };

    Ok(ChatResponse {
        provider: res.provider,
        model: res.model,
        text: res.summary,
        fallback_provider: res.fallback,
        fallback_reason: None,
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
    })
}

#[tauri::command]
async fn cancel_chat_message(chat_id: String) -> Result<(), String> {
    mint_core::cancel_agent(&chat_id);
    Ok(())
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct GeminiLiveStartRequest {
    #[serde(default)]
    workspace_path: Option<String>,
    #[serde(default)]
    chat_id: Option<String>,
}

/// Starts a Gemini Live realtime voice session (beta, opt-in voice mode) and returns a
/// session id used by `send_gemini_live_audio_chunk`/`stop_gemini_live_session`. Session
/// events (audio replies, transcripts, tool-call status) stream back through `on_event`.
/// Tool calls triggered by voice go through the same approval flow as typed chat, reusing
/// the existing `tool-approval-requested` event / `ApprovalsState` bridge.
#[tauri::command]
async fn start_gemini_live_session(
    app: AppHandle,
    state: tauri::State<'_, GeminiLiveState>,
    request: GeminiLiveStartRequest,
    on_event: Channel<GeminiLiveEvent>,
) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    let root = workspace_root(request.workspace_path.as_deref())?;
    let chat_id = request.chat_id.unwrap_or_default();
    let session_id = format!("gemini-live-{}", COUNTER.fetch_add(1, Ordering::SeqCst));

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

        let outcome =
            tokio::task::block_in_place(move || tokio::runtime::Handle::current().block_on(rx))
                .unwrap_or(ApprovalOutcome::Denied);
        Ok(outcome)
    };

    let handle = core_start_gemini_live_session(config, root, chat_id, approve_cb, move |event| {
        let _ = on_event.send(event);
    });

    state
        .sessions
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session_id.clone(), handle);
    Ok(session_id)
}

#[tauri::command]
async fn send_gemini_live_audio_chunk(
    state: tauri::State<'_, GeminiLiveState>,
    session_id: String,
    chunk_base64: String,
) -> Result<(), String> {
    let pcm = BASE64
        .decode(chunk_base64)
        .map_err(|e| format!("invalid audio chunk: {e}"))?;
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let handle = sessions
        .get(&session_id)
        .ok_or_else(|| "Gemini Live session not found".to_string())?;
    handle.push_audio(pcm)
}

#[tauri::command]
async fn stop_gemini_live_session(
    state: tauri::State<'_, GeminiLiveState>,
    session_id: String,
) -> Result<(), String> {
    state
        .sessions
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&session_id);
    Ok(())
}

/// Starts native push-to-talk mic recording (Rust-side, via `cpal`) for the desktop
/// build's voice input button. Call `stop_mic_recording_and_transcribe` to stop and
/// get the transcript back.
#[tauri::command]
fn start_mic_recording(state: tauri::State<'_, MicRecordingState>) -> Result<(), String> {
    let mut active = state.active.lock().map_err(|e| e.to_string())?;
    if active.is_some() {
        return Err("A recording is already in progress".into());
    }
    let handle = core_start_mic_recording().map_err(|e| e.to_string())?;
    *active = Some(handle);
    Ok(())
}

/// Stops the in-progress recording and transcribes it using whichever provider is
/// configured for chat (`MintConfig.ai_provider`). Rejects with a clear message if
/// that provider doesn't support audio input.
#[tauri::command]
async fn stop_mic_recording_and_transcribe(
    state: tauri::State<'_, MicRecordingState>,
) -> Result<String, String> {
    let handle = state
        .active
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| "No recording is in progress".to_string())?;

    // stop_recording() blocks joining the recorder thread — run it off the async
    // executor so it doesn't stall this command's tokio task.
    let wav_bytes = tokio::task::spawn_blocking(move || core_stop_mic_recording(handle))
        .await
        .map_err(|e| format!("recording thread panicked: {e}"))?
        .map_err(|e| e.to_string())?;

    let config = load_config().map_err(|error| error.to_string())?;
    core_transcribe_mic_recording(&config, wav_bytes)
        .await
        .map_err(|e| e.to_string())
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
    ai_text: Option<String>,
    provider: String,
    model: String,
) -> Result<i64, String> {
    MemoryStore::open_default()
        .and_then(|memory| {
            memory.add_interaction_for_chat(
                &chat_id,
                &user_text,
                ai_text.as_deref().unwrap_or(""),
                &provider,
                &model,
            )
        })
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn get_recent_interactions(
    limit: Option<usize>,
    chat_id: Option<String>,
    workspace_path: Option<String>,
) -> Result<Vec<InteractionMemory>, String> {
    let scoped_chat_id = mint_core::scoped_chat_id(
        chat_id
            .as_deref()
            .unwrap_or(mint_core::DEFAULT_CONVERSATION_ID),
        workspace_path.as_deref(),
    );
    MemoryStore::open_default()
        .and_then(|memory| memory.recent_interactions_for_chat(&scoped_chat_id, limit.unwrap_or(5)))
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

/// Path to the file that remembers which shared-store user is currently
/// logged into the desktop app (`~/.config/mint/session.json`). Desktop is a
/// single trusted process per launch, so a bearer token isn't needed here —
/// unlike the web-mode API server (see mint_core::auth session tokens).
fn desktop_session_path() -> Result<PathBuf, String> {
    Ok(config_path()
        .map_err(|error| error.to_string())?
        .with_file_name("session.json"))
}

fn write_desktop_session(user_id: &str) -> Result<(), String> {
    let path = desktop_session_path()?;
    let contents = serde_json::json!({ "userId": user_id }).to_string();
    fs::write(path, contents).map_err(|error| error.to_string())
}

fn read_desktop_session_user_id() -> Option<String> {
    let path = desktop_session_path().ok()?;
    let contents = fs::read_to_string(path).ok()?;
    let value: Value = serde_json::from_str(&contents).ok()?;
    value.get("userId")?.as_str().map(str::to_string)
}

#[tauri::command]
fn auth_register(
    name: Option<String>,
    email: String,
    password: String,
) -> Result<AuthUser, String> {
    let user = register_user(name, &email, &password).map_err(|error| error.to_string())?;
    write_desktop_session(&user.id)?;
    Ok(user)
}

#[tauri::command]
fn auth_login(email: String, password: String) -> Result<AuthUser, String> {
    let user = login_user(&email, &password).map_err(|error| error.to_string())?;
    write_desktop_session(&user.id)?;
    Ok(user)
}

#[tauri::command]
fn auth_logout() -> Result<(), String> {
    let path = desktop_session_path()?;
    if path.exists() {
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn auth_current_user() -> Result<Option<AuthUser>, String> {
    let Some(user_id) = read_desktop_session_user_id() else {
        return Ok(None);
    };
    get_user(&user_id).map_err(|error| error.to_string())
}

#[tauri::command]
fn auth_update_profile(name: Option<String>, image: Option<String>) -> Result<AuthUser, String> {
    let Some(user_id) = read_desktop_session_user_id() else {
        return Err("Not logged in".to_string());
    };
    update_profile(&user_id, name, image).map_err(|error| error.to_string())
}

#[tauri::command]
fn auth_upload_avatar(file_name: String, data_base64: String) -> Result<AuthUser, String> {
    let Some(user_id) = read_desktop_session_user_id() else {
        return Err("Not logged in".to_string());
    };
    let bytes = BASE64
        .decode(data_base64.as_bytes())
        .map_err(|_| "Invalid image data".to_string())?;
    let extension = file_name.rsplit('.').next().unwrap_or("png").to_lowercase();
    let url = save_avatar_file(&bytes, &extension).map_err(|error| error.to_string())?;
    update_profile(&user_id, None, Some(url)).map_err(|error| error.to_string())
}

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
struct LearnedSkillDto {
    id: i64,
    name: String,
    source_path: String,
    content: String,
    updated_at: String,
    location: String,
}

#[tauri::command]
fn list_learned_skills(workspace_path: Option<String>) -> Result<Vec<LearnedSkillDto>, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    let db_skills = store.learned_skills(100).map_err(|e| e.to_string())?;

    let mut global_skills = Vec::new();
    if let Some(home) = dirs::home_dir() {
        let global_agents_path = home.join(".gemini").join("config").join("AGENTS.md");
        mint_core::skills::load_agent_rules_file(&global_agents_path, &mut global_skills);

        let global_skills_path = home.join(".config").join("mint").join("mint-skills");
        if !global_skills_path.exists() {
            let _ = std::fs::create_dir_all(&global_skills_path);
        }
        mint_core::skills::load_skills_from_dir(&global_skills_path, &mut global_skills);
    }

    let mut workspace_skills = Vec::new();
    if let Ok(root) = workspace_root(workspace_path.as_deref()) {
        let workspace_agents_path1 = root.join(".agents").join("AGENTS.md");
        mint_core::skills::load_agent_rules_file(&workspace_agents_path1, &mut workspace_skills);

        let workspace_agents_path2 = root.join("AGENTS.md");
        mint_core::skills::load_agent_rules_file(&workspace_agents_path2, &mut workspace_skills);

        let workspace_skills_path1 = root.join(".agents").join("skills");
        mint_core::skills::load_skills_from_dir(&workspace_skills_path1, &mut workspace_skills);

        let workspace_skills_path2 = root.join("skills");
        mint_core::skills::load_skills_from_dir(&workspace_skills_path2, &mut workspace_skills);
    }

    let mut unique_skills = std::collections::BTreeMap::new();
    for s in db_skills {
        unique_skills.insert(s.name.clone(), (s, "database"));
    }
    for s in global_skills {
        unique_skills.insert(s.name.clone(), (s, "global"));
    }
    for s in workspace_skills {
        unique_skills.insert(s.name.clone(), (s, "workspace"));
    }

    let dtos = unique_skills
        .into_values()
        .map(|(s, loc)| LearnedSkillDto {
            id: s.id,
            name: s.name,
            source_path: s.source_path,
            content: s.content,
            updated_at: s.created_at,
            location: loc.to_string(),
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
        location: "database".to_string(),
    })
}

#[tauri::command]
fn delete_learned_skill(name: String) -> Result<usize, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    store.delete_learned_skill(&name).map_err(|e| e.to_string())
}

#[tauri::command]
fn list_subagents(workspace_path: Option<String>) -> Result<Vec<SubagentDefinition>, String> {
    let root = workspace_root(workspace_path.as_deref()).ok();
    Ok(core_list_subagents(root.as_deref()))
}

#[tauri::command]
fn save_subagent(
    draft: SubagentDraft,
    workspace_path: Option<String>,
) -> Result<SubagentDefinition, String> {
    let root = workspace_root(workspace_path.as_deref()).ok();
    core_save_subagent(&draft, root.as_deref())
}

#[tauri::command]
fn delete_subagent(source_path: String) -> Result<(), String> {
    core_delete_subagent(&source_path)
}

#[tauri::command]
fn run_slash_command(
    app: AppHandle,
    input: String,
    cwd: Option<String>,
) -> Result<mint_core::slash::SlashResponse, String> {
    use mint_core::slash::{SlashEffect, SlashResponse};
    let mut config = load_config().map_err(|error| error.to_string())?;
    let request = mint_core::slash::SlashRequest {
        input,
        cwd,
        surface: Some("desktop".to_string()),
    };
    let response = mint_core::slash::execute(&request, &mut config);

    let persists_config = matches!(
        &response,
        SlashResponse::Applied { effects, .. }
            if effects.iter().any(|e| !matches!(e, SlashEffect::HistoryCleared))
    );
    if persists_config {
        save_config(&config).map_err(|error| error.to_string())?;
        let _ = app.emit("settings-changed", &config);
    }
    Ok(response)
}

#[tauri::command]
fn list_cron_jobs() -> Result<Vec<CronJob>, String> {
    CronStore::open_default()
        .and_then(|store| store.list())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_cron_job(draft: CronJobDraft) -> Result<CronJob, String> {
    let store = CronStore::open_default().map_err(|e| e.to_string())?;
    store
        .add(draft.name, draft.schedule, draft.task, draft.workspace)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_cron_job(id: String) -> Result<bool, String> {
    CronStore::open_default()
        .and_then(|store| store.remove(&id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn set_cron_job_enabled(id: String, enabled: bool) -> Result<Option<CronJob>, String> {
    CronStore::open_default()
        .and_then(|store| store.set_enabled(&id, enabled))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn list_linked_folders() -> Result<std::collections::BTreeMap<String, LinkedFolder>, String> {
    mint_core::list_linked_folders().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_linked_folder(draft: LinkedFolderDraft) -> Result<(), String> {
    mint_core::add_linked_folder(&draft.name, &draft.path, draft.description)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_linked_folder(name: String) -> Result<bool, String> {
    mint_core::remove_linked_folder(&name).map_err(|e| e.to_string())
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
    answer: Option<String>,
) -> Result<(), String> {
    let mut pending = state.pending.lock().map_err(|error| error.to_string())?;
    if let Some(tx) = pending.remove(&token) {
        let outcome = if let Some(ans) = answer {
            if !ans.trim().is_empty() {
                ApprovalOutcome::Intercepted(ans)
            } else {
                ApprovalOutcome::Denied
            }
        } else if approved {
            ApprovalOutcome::Approved
        } else {
            ApprovalOutcome::Denied
        };
        let _ = tx.send(outcome);
        Ok(())
    } else {
        Err("No pending approval found for this token".into())
    }
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTools {
    pub docker: bool,
    pub git: bool,
    pub gh: bool,
    pub node: bool,
}

#[tauri::command]
async fn detect_system_tools() -> Result<DetectedTools, String> {
    Ok(DetectedTools {
        docker: mint_core::config::which("docker"),
        git: mint_core::config::which("git"),
        gh: mint_core::config::which("gh"),
        node: mint_core::config::which("node"),
    })
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
async fn generate_video(request: VideoGenRequest) -> Result<VideoGenResponse, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    mint_core::generate_video(&config, &request)
        .await
        .map_err(|error| error.to_string())
}

#[tauri::command]
fn list_pictures() -> Result<Vec<PictureEntry>, String> {
    list_saved_pictures().map_err(|error| error.to_string())
}

#[tauri::command]
fn delete_picture(id: String) -> Result<(), String> {
    delete_saved_picture(&id).map_err(|error| error.to_string())
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
fn upload_file(filename: String, data_b64: String) -> Result<String, String> {
    // Determine mime from filename
    let mime = filename
        .rsplit_once('.')
        .map(|(_, ext)| match ext.to_ascii_lowercase().as_str() {
            "mp4" => "video/mp4",
            "webm" => "video/webm",
            "mov" => "video/quicktime",
            "mkv" => "video/x-matroska",
            "avi" => "video/x-msvideo",
            _ => "application/octet-stream",
        })
        .unwrap_or("application/octet-stream");

    let data_uri = format!("data:{};base64,{}", mime, data_b64);
    match save_chat_images(
        vec![data_uri],
        Some("upload".into()),
        Some("uploaded".into()),
    ) {
        Ok(mut saved) => {
            if let Some(entry) = saved.pop() {
                return Ok(format!("/api/pictures/{}", entry.filename));
            }
            Err("no entry saved".into())
        }
        Err(e) => Err(e.to_string()),
    }
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
async fn type_in_browser(selector: String, text: String) -> Result<String, String> {
    mint_core::browser::type_text(
        &load_config().map_err(|error| error.to_string())?,
        &selector,
        &text,
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

/// WebKitGTK denies every `permission-request` (microphone, camera, geolocation, ...) by
/// default unless something handles the signal — Tauri/wry doesn't wire this up on Linux,
/// which is why `getUserMedia()` rejects with `NotAllowedError` even though the user never
/// saw (or could act on) a prompt. Auto-allow only mic/camera requests here; everything
/// else falls through to WebKit's default (deny).
#[cfg(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
))]
fn allow_media_permission_requests(window: &tauri::WebviewWindow) {
    use webkit2gtk::{PermissionRequestExt, WebViewExt, glib::prelude::ObjectExt};
    let _ = window.with_webview(|platform_webview| {
        let webview = platform_webview.inner();
        webview.connect_permission_request(|_webview, request| {
            if request.is::<webkit2gtk::UserMediaPermissionRequest>() {
                request.allow();
                true
            } else {
                false
            }
        });
    });
}

#[cfg(not(any(
    target_os = "linux",
    target_os = "dragonfly",
    target_os = "freebsd",
    target_os = "netbsd",
    target_os = "openbsd"
)))]
fn allow_media_permission_requests(_window: &tauri::WebviewWindow) {}

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
                emit_to_main(app, "open-settings", ());
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
    let main_handler = main_shortcut;
    let spotlight_handler = spotlight_shortcut;
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
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ApprovalsState {
            pending: Mutex::new(HashMap::new()),
        })
        .manage(GeminiLiveState::default())
        .manage(MicRecordingState::default())
        .manage(mint_core::avatar_bridge::AvatarBridge::new(
            load_config()
                .map(|c| mint_core::avatar_bridge::AvatarBridgeConfig::from_mint_config(&c))
                .unwrap_or_else(|_| mint_core::avatar_bridge::AvatarBridgeConfig::from_env()),
        ))
        .setup(|app| {
            if let Some(main_window) = app.get_webview_window("main") {
                allow_media_permission_requests(&main_window);
            }
            install_tray(app.handle())?;
            install_shortcuts(app.handle())?;
            start_system_events(app.handle().clone());
            start_headless_queue(app.handle().clone());
            start_proactive_loop(app.handle().clone());
            start_channels();
            start_cron_scheduler();
            start_webhooks();
            tauri::async_runtime::spawn(async {
                let _ = mint_core::start_api_server(3000).await;
            });
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
            detect_system_tools,
            reauth_mcp_server,
            list_mcp_server_tools,
            get_workspace_tree,
            create_workspace_file,
            create_workspace_folder,
            delete_workspace_item,
            generate_images,
            generate_video,
            select_workspace_directory,
            get_config,
            get_updater_status,
            check_for_updates,
            install_available_update,
            update_config,
            set_active_model,
            inspect_shell_command,
            send_chat_message,
            stream_chat_message,
            cancel_chat_message,
            start_gemini_live_session,
            send_gemini_live_audio_chunk,
            stop_gemini_live_session,
            start_mic_recording,
            stop_mic_recording_and_transcribe,
            submit_tool_approval,
            get_recent_interactions,
            save_interaction_agent_activity,
            list_chat_sessions,
            delete_chat_session,
            rename_chat_session,
            get_profile_value,
            set_profile_value,
            auth_register,
            auth_login,
            auth_logout,
            auth_current_user,
            auth_update_profile,
            auth_upload_avatar,
            clear_chat_history,
            list_learned_skills,
            add_learned_skill,
            delete_learned_skill,
            list_subagents,
            save_subagent,
            delete_subagent,
            run_slash_command,
            list_cron_jobs,
            add_cron_job,
            remove_cron_job,
            set_cron_job_enabled,
            list_linked_folders,
            add_linked_folder,
            remove_linked_folder,
            list_pictures,
            delete_picture,
            save_pictures,
            upload_file,
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
            type_in_browser,
            start_screen_capture,
            submit_screen_selection,
            submit_spotlight,
            set_ai_state,
            toggle_proactive,
            save_behavior_context,
            run_next_queued_task,
            exit_app,
            save_system_interaction
        ])
        .build(tauri::generate_context!())
        .expect("error while running Mint desktop")
        .run(|_app_handle, event| {
            // Kill any stdio MCP child processes we spawned before the process
            // goes away — `SESSIONS` is a `static`, so `McpSession::Drop` never
            // runs at exit on its own. Fires for tray "Quit", `exit_app`, and
            // the last window closing.
            if let tauri::RunEvent::Exit = event {
                mint_core::close_all_mcp_sessions();
            }
        });
}

pub use super::approvals::ApprovalsState;
use crate::commands::approvals;
use crate::commands::workspace::{background_owner, bind_background_owner, workspace_root};
use mint_core::{
    AgentProgress, ChatRequest, ChatResponse, ChatSession, ConversationChanges,
    ConversationSnapshot, InteractionMemory, MemoryStore, load_config, orchestrate_agent_loop,
    orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback, save_config,
    with_turn_start_listener,
};
use serde::Serialize;
use serde_json::Value;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum DesktopStreamEvent {
    Started {
        #[serde(rename = "interactionId")]
        interaction_id: i64,
    },
    Chunk {
        chunk: String,
    },
    Progress {
        progress: AgentProgress,
    },
}

#[tauri::command]
pub(crate) async fn send_chat_message(
    app: AppHandle,
    request: ChatRequest,
    owner_token: Option<String>,
) -> Result<ChatResponse, String> {
    bind_background_owner(&request, owner_token)?;
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

        let registered_task_id = join_handle.id();
        let abort_handle = join_handle.abort_handle();
        if !chat_id_str.is_empty() {
            mint_core::ACTIVE_AGENTS
                .lock()
                .unwrap()
                .insert(chat_id_str.clone(), abort_handle);
        }

        let res = join_handle.await;

        if !chat_id_str.is_empty() {
            mint_core::unregister_agent(&chat_id_str, registered_task_id);
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

    let approve_cb = approvals::callback(app.clone(), request.chat_id.clone().unwrap_or_default());

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

    let registered_task_id = join_handle.id();
    let abort_handle = join_handle.abort_handle();
    if !chat_id_str.is_empty() {
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .insert(chat_id_str.clone(), abort_handle);
    }

    let res = join_handle.await;

    if !chat_id_str.is_empty() {
        mint_core::unregister_agent(&chat_id_str, registered_task_id);
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
        fallback_reason: res.fallback_reason,
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
        thought: None,
    })
}

#[tauri::command]
pub(crate) async fn stream_chat_message(
    app: AppHandle,
    request: ChatRequest,
    on_event: Channel<DesktopStreamEvent>,
    owner_token: Option<String>,
) -> Result<ChatResponse, String> {
    bind_background_owner(&request, owner_token)?;
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
        let on_event_started = on_event.clone();
        let chat_id_str = request.chat_id.clone().unwrap_or_default();

        let join_handle = tokio::spawn(async move {
            let expected_chat_id = clean_request
                .chat_id
                .clone()
                .unwrap_or_else(|| mint_core::DEFAULT_CONVERSATION_ID.to_owned());
            with_turn_start_listener(
                expected_chat_id,
                move |id| {
                    let _ =
                        on_event_started.send(DesktopStreamEvent::Started { interaction_id: id });
                },
                orchestrate_chat_stream_with_fallback(
                    &config_clone,
                    &clean_request,
                    move |chunk| {
                        let _ = on_event_clone.send(DesktopStreamEvent::Chunk { chunk });
                    },
                ),
            )
            .await
        });

        let registered_task_id = join_handle.id();
        let abort_handle = join_handle.abort_handle();
        if !chat_id_str.is_empty() {
            mint_core::ACTIVE_AGENTS
                .lock()
                .unwrap()
                .insert(chat_id_str.clone(), abort_handle);
        }

        let res = join_handle.await;

        if !chat_id_str.is_empty() {
            mint_core::unregister_agent(&chat_id_str, registered_task_id);
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

    let approve_cb = approvals::callback(app.clone(), request.chat_id.clone().unwrap_or_default());

    let on_progress_event = on_event.clone();
    let avatar_app = app.clone();
    let progress_cb = move |progress| {
        avatar_app
            .state::<mint_core::avatar_bridge::AvatarBridge>()
            .on_agent_progress(&progress);
        let _ = on_progress_event.send(DesktopStreamEvent::Progress { progress });
    };

    let on_event_clone = on_event.clone();
    let on_event_started = on_event.clone();
    let chunk_app = app.clone();
    let on_chunk = move |chunk: String| {
        let bridge = chunk_app.state::<mint_core::avatar_bridge::AvatarBridge>();
        bridge.on_talking(true);
        let _ = on_event_clone.send(DesktopStreamEvent::Chunk { chunk });
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
        let expected_chat_id = chat_id_clone
            .clone()
            .unwrap_or_else(|| mint_core::DEFAULT_CONVERSATION_ID.to_owned());
        with_turn_start_listener(
            expected_chat_id,
            move |id| {
                let _ = on_event_started.send(DesktopStreamEvent::Started { interaction_id: id });
            },
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
            ),
        )
        .await
    });

    let registered_task_id = join_handle.id();
    let abort_handle = join_handle.abort_handle();
    if !chat_id_str.is_empty() {
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .insert(chat_id_str.clone(), abort_handle);
    }

    let res = join_handle.await;

    if !chat_id_str.is_empty() {
        mint_core::unregister_agent(&chat_id_str, registered_task_id);
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
        fallback_reason: res.fallback_reason,
        tool_calls: None,
        stop_reason: None,
        total_tokens: None,
        input_tokens: None,
        output_tokens: None,
        thought: None,
    })
}

#[tauri::command]
pub(crate) async fn cancel_chat_message(chat_id: String) -> Result<(), String> {
    mint_core::cancel_agent(&chat_id);
    Ok(())
}

#[tauri::command]
pub(crate) fn save_interaction_agent_activity(
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
pub(crate) fn save_system_interaction(
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
pub(crate) fn update_chat_session_workspace(
    chat_id: String,
    workspace_path: Option<String>,
) -> Result<(), String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.set_chat_session_workspace(&chat_id, workspace_path.as_deref()))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_recent_interactions(
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
pub(crate) fn get_conversation_snapshot(
    chat_id: String,
    before_id: Option<i64>,
    limit: Option<usize>,
) -> Result<ConversationSnapshot, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.conversation_snapshot(&chat_id, before_id, limit.unwrap_or(50)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_conversation_changes(
    chat_id: String,
    after: i64,
    limit: Option<usize>,
) -> Result<ConversationChanges, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.conversation_changes(&chat_id, after, limit.unwrap_or(100)))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn list_chat_sessions() -> Result<Vec<ChatSession>, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.list_chat_sessions())
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn delete_chat_session(chat_id: String) -> Result<usize, String> {
    mint_core::cancel_agent(&chat_id);
    MemoryStore::open_default()
        .and_then(|memory| memory.delete_chat_session(&chat_id))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn rename_chat_session(chat_id: String, new_title: String) -> Result<usize, String> {
    MemoryStore::open_default()
        .and_then(|memory| memory.rename_chat_session(&chat_id, &new_title))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn clear_chat_history(chat_id: Option<String>) -> Result<usize, String> {
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
pub(crate) async fn submit_tool_approval(
    state: tauri::State<'_, ApprovalsState>,
    token: String,
    approved: bool,
    answer: Option<String>,
) -> Result<(), String> {
    if state.resolve(&token, approved, answer) {
        Ok(())
    } else {
        Err("No pending approval found for this token".into())
    }
}

#[tauri::command]
pub(crate) async fn run_slash_command(
    app: AppHandle,
    input: String,
    cwd: Option<String>,
    chat_id: Option<String>,
    owner_token: Option<String>,
) -> Result<mint_core::slash::SlashResponse, String> {
    use mint_core::slash::{SlashEffect, SlashResponse};
    let mut config = load_config().map_err(|error| error.to_string())?;
    let request = mint_core::slash::SlashRequest {
        input,
        cwd,
        surface: Some("desktop".to_string()),
        chat_id,
    };
    let trimmed = request.input.trim();
    let (token, rest) = trimmed
        .split_once(char::is_whitespace)
        .unwrap_or((trimmed, ""));
    let response = if token == "/shells" {
        let owner = background_owner(owner_token)?;
        let cwd = request
            .cwd
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        match mint_core::bg_shell::shells_command(rest.trim(), &cwd, &owner, &config) {
            Ok(markdown) => SlashResponse::Message { markdown },
            Err(error) => SlashResponse::Message {
                markdown: error.to_string(),
            },
        }
    } else if token == "/mint-web" {
        let owner = background_owner(owner_token)?;
        let cwd = request
            .cwd
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        let markdown =
            mint_core::system::mint_web::command(&cwd, rest, owner, &config).unwrap_or_else(|e| e);
        SlashResponse::Message { markdown }
    } else if token == "/preview" {
        let owner = background_owner(owner_token)?;
        let cwd = request
            .cwd
            .as_deref()
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_default());
        match mint_core::system::html_preview::command(&cwd, rest, owner, &config) {
            Ok(markdown) => SlashResponse::Message { markdown },
            Err(markdown) => SlashResponse::Message { markdown },
        }
    } else {
        mint_core::slash::execute_async(&request, &mut config).await
    };

    let persists_config = matches!(
        &response,
        SlashResponse::Applied { effects, .. }
            if effects.iter().any(|e| !matches!(e, SlashEffect::HistoryCleared | SlashEffect::PlanModeChanged { .. }))
    );
    if persists_config {
        save_config(&config).map_err(|error| error.to_string())?;
        let _ = app.emit("settings-changed", &config);
    }
    Ok(response)
}

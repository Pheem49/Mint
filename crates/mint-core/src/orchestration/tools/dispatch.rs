use std::collections::BTreeMap;
use std::path::Path;

use futures_util::StreamExt;
use serde_json::Value;

use crate::chat::{ChatMessage, ChatRole, ContentBlock};
use crate::config::MintConfig;
use crate::{Capability, assert_path_capability};

use super::super::context_render::format_trajectory_step;
use super::super::decision_parsing::*;
use super::super::types::*;
use super::super::workspace_helpers::*;
use super::super::{
    is_internal_cot, mcp_review, model_tool_result, plan_mode_allows, reliability, run_control,
    tool_progress_input,
};

type LoadedImage = (String, Option<(Vec<u8>, &'static str, String)>);

#[allow(clippy::too_many_arguments)]
pub(crate) async fn run_parallel_read_only_batch(
    decisions: &[(String, AgentDecision)],
    step: usize,
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    action_counts: &mut BTreeMap<String, usize>,
    trajectory: &mut Vec<String>,
    step_images: &mut std::collections::HashMap<String, Vec<String>>,
    hooks: &[crate::hooks::HookEntry],
    pinned_mcp_server: Option<&str>,
    fast_mode: bool,
) -> Vec<ToolResultEntry> {
    let approve_mutex = std::sync::Mutex::new(approve);
    let progress_mutex = std::sync::Mutex::new(progress);

    let mut tasks = Vec::with_capacity(decisions.len());
    for (index, (call_id, decision)) in decisions.iter().enumerate() {
        let call_id = call_id.clone();
        let thought = decision.thought.clone();
        let action = decision.action.clone();
        let input_val = tool_progress_input(&root, &decision);
        let action_key = action_fingerprint(decision);
        let approve_mutex = &approve_mutex;
        let progress_mutex = &progress_mutex;
        let decision = decision;

        tasks.push(async move {
            let activity_call_id=call_id.clone();
            if !fast_mode && !thought.trim().is_empty() {
                let t = thought.trim();
                let mut guard = progress_mutex.lock().unwrap();
                if is_internal_cot(t) {
                    (*guard)(AgentProgress::ExtendedThinking {
                        id: None,
                        thought: t.to_owned(),
                        elapsed_ms: None,
                    });
                } else {
                    (*guard)(AgentProgress::Thought {
                        thought: t.to_owned(),
                    });
                }
            }

            {
                let mut guard = progress_mutex.lock().unwrap();
                (*guard)(AgentProgress::ToolStart { call_id: Some(activity_call_id.clone()),
                    action: action.clone(),
                    input: input_val.clone(),
                    subagent: None,
                });
            }

            let mut images=Vec::new();
            let mut typed_status=None;
            let result: String = if pinned_mcp_server.is_some_and(|p| {
                (action == "mcp_tool" || action == "mcp_list_tools") && decision.input.server != p
            }) {
                let p = pinned_mcp_server.unwrap();
                format!(
                    "Blocked: this turn is pinned to the \"{p}\" MCP server only (selected via @{p} in the composer). Retry with \"server\":\"{p}\", or use a different (non-MCP) tool."
                )
            } else {
                match crate::hooks::run_pre_tool_hooks(hooks, &action, &input_val, root) {
                    crate::hooks::PreHookOutcome::Blocked(reason) => {
                        format!(
                            "Blocked by hook: {}\n\n[System Tip: A configured PreToolUse hook rejected this action. Adjust your approach or ask the user for guidance.]",
                            reason
                        )
                    }
                    crate::hooks::PreHookOutcome::Allowed => {
                        let mut approve_adapter =
                            |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                                let mut guard = approve_mutex.lock().unwrap();
                                (*guard)(approval)
                            };
                        let mut progress_adapter = |event: AgentProgress| {
                            let mut guard = progress_mutex.lock().unwrap();
                            (*guard)(event);
                        };

                        let (tool_result, success) = match execute_tool_outcome(
                            root,
                            config,
                            &decision,
                            chat_id,
                            &mut approve_adapter,
                            &mut progress_adapter,
                            &activity_call_id,
                        )
                        .await
                        {
                            Ok(outcome) => {images=outcome.images;typed_status=Some(outcome.status);(outcome.text,outcome.status==crate::mcp_result::ToolStatus::Success)},
                            Err(err) => (format!("Error: {}", err), false),
                        };

                        let hook_messages = crate::hooks::run_post_tool_hooks(
                            hooks,
                            &action,
                            &input_val,
                            &tool_result,
                            success,
                            root,
                        );

                        if hook_messages.is_empty() {
                            tool_result
                        } else {
                            format!(
                                "{}\n\n{}",
                                tool_result,
                                hook_messages
                                    .iter()
                                    .map(|msg| format!("[Hook] {}", msg))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                            )
                        }
                    }
                }
            };

            {
                let mut guard = progress_mutex.lock().unwrap();
                (*guard)(AgentProgress::ToolEnd { elapsed_ms: None, call_id: Some(activity_call_id.clone()), status: typed_status,
                    action: action.clone(),
                    input: input_val.clone(),
                    result: result.clone(),
                    subagent: None,
                });
            }

            (
                index, call_id, thought, action, input_val, action_key, result, images, typed_status,
            )
        });
    }

    let mut results = futures_util::stream::iter(tasks)
        .buffer_unordered(PARALLEL_READ_ONLY_LIMIT)
        .collect::<Vec<_>>()
        .await;

    results.sort_by_key(|(index, ..)| *index);

    let mut step_tool_results = Vec::with_capacity(results.len());
    for (_, call_id, thought, action, input_val, action_key, result, images, typed_status) in
        results
    {
        if !images.is_empty() {
            step_images.insert(call_id.clone(), images);
        }
        let action_count = {
            let count = action_counts.entry(action_key).or_insert(0);
            *count += 1;
            *count
        };

        let mut final_result = if result.starts_with("data:image/")
            && matches!(
                action.as_str(),
                "video_filmstrip" | "video_waveform" | "view_image"
            ) {
            step_images.insert(call_id.clone(), vec![result.clone()]);
            match action.as_str() {
                "video_filmstrip" => {
                    "[Filmstrip generated — see attached image: sampled frames across the video timeline]".to_string()
                }
                "video_waveform" => {
                    "[Waveform generated — see attached image: audio amplitude over time]".to_string()
                }
                "view_image" => "[Image loaded — see attached image]".to_string(),
                _ => "[Screenshot captured — see attached image]".to_string(),
            }
        } else {
            model_tool_result(Some(root), &action, &result)
        };

        if action_count >= 3 {
            final_result.push_str(
                "\n\n[System Tip: You repeated the same tool action three or more times. \
                 Stop repeating it. If you already have enough information or the requested edit is done, \
                 use the finish action now. Otherwise choose a different necessary action.]",
            );
        }

        trajectory.push(format_trajectory_step(
            step,
            &thought,
            &action,
            &final_result,
        ));

        let mut entry = ToolResultEntry::from((call_id, action, input_val, final_result));
        if let Some(status) = typed_status {
            entry.status = status;
        }
        step_tool_results.push(entry);
    }

    step_tool_results
}

// Takes a trait object rather than being generic over `Approve` like it used
// to be: `dispatch_subagent` recurses back into `orchestrate_agent_loop`
// (which is itself generic over its own `Approve`), and reborrowing a generic
// `&mut Approve` into that recursive call makes the type grow by one `&mut`
// layer per nesting level during monomorphization — `&mut Approve`, then
// `&mut &mut Approve`, then `&mut &mut &mut Approve`, forever, since the type
// system has no way to know the runtime depth limit that
// `tool_catalog`/`allow_subagent_dispatch` enforces. A `&mut dyn FnMut(...)`
// trait object is a single fixed type regardless of recursion depth, so it
// doesn't hit that.

pub(crate) async fn execute_tool(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<String, OrchestrationError> {
    Ok(execute_tool_outcome(
        root,
        config,
        decision,
        chat_id,
        approve,
        progress,
        &uuid::Uuid::new_v4().to_string(),
    )
    .await?
    .text)
}

pub(crate) async fn execute_tool_outcome(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    call_id: &str,
) -> Result<crate::integrations::mcp_result::ToolOutcome, OrchestrationError> {
    execute_tool_outcome_reviewed(
        root, config, decision, chat_id, approve, progress, call_id, None,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn execute_tool_outcome_reviewed(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    call_id: &str,
    expected: Option<&mcp_review::McpReview>,
) -> Result<crate::integrations::mcp_result::ToolOutcome, OrchestrationError> {
    run_control::check()?;
    let origin = MOD_ORIGIN.try_with(Clone::clone).ok().flatten();
    let input = serde_json::to_value(&decision.input).unwrap_or(Value::Null);
    crate::mods::event(
        root,
        chat_id,
        "pre_tool_use",
        serde_json::json!({"callId":call_id,"action":decision.action,"input":input}),
        origin.as_deref(),
    )
    .map_err(OrchestrationError::Agent)?;
    let execution = execute_tool_outcome_reviewed_inner(
        root, config, decision, chat_id, approve, progress, call_id, expected,
    );
    let result = if matches!(
        decision.action.as_str(),
        "run_shell" | "verify" | "run_tests" | "run_typecheck" | "run_linter"
    ) {
        // Foreground shell cancellation joins process cleanup and imports any
        // checkpointed screenshots before the turn is marked interrupted.
        execution.await
    } else {
        run_control::run(Box::pin(execution)).await
    };
    let _ = crate::mods::event(
        root,
        chat_id,
        "post_tool_use",
        serde_json::json!({"callId":call_id,"action":decision.action,"input":input,"result":result.as_ref().map(|r|r.text.clone()).unwrap_or_else(|e|e.to_string()),"success":result.is_ok()}),
        origin.as_deref(),
    );
    result
}

tokio::task_local! { static MOD_ORIGIN: Option<String>; }

/// Run queued requests through the same policy and approval path as agent tools.
pub async fn execute_mod_requests(
    root: &Path,
    chat: &str,
    config: &MintConfig,
    plan_mode: bool,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    selected: Option<&[String]>,
) -> Result<(), String> {
    let requests = match selected {
        Some(ids) => crate::mods::take_selected_requests(root, chat, ids)?,
        None => crate::mods::take_requests(root, chat)?,
    };
    let (remembered_plan, pin) = crate::mods::turn_policy(root, chat);
    let plan_mode = plan_mode || remembered_plan;
    for request in requests {
        let action = request["action"].as_str().unwrap_or("");
        let id = request["modId"].as_str().unwrap_or("");
        let input = match decode_agent_input(request["input"].clone()) {
            Ok(input) => input,
            Err(error) => {
                crate::mods::complete_request(
                    root,
                    chat,
                    id,
                    request["hash"].as_str().unwrap_or(""),
                    serde_json::json!({"requestId":request["id"],"action":action,"error":error}),
                )?;
                continue;
            }
        };
        let allowed = crate::prompts::agent::base_allowed_actions().contains(&action)
            && !action.starts_with("mod_")
            && !config.disabled_tools.iter().any(|t| t == action)
            && (!plan_mode || plan_mode_allows(action, &input))
            && pin.as_deref().is_none_or(|p| {
                !matches!(action, "mcp_tool" | "mcp_list_tools") || input.server == p
            });
        let result = if allowed {
            let decision = AgentDecision {
                thought: String::new(),
                action: action.into(),
                input,
            };
            MOD_ORIGIN
                .scope(
                    Some(id.into()),
                    execute_tool_outcome_reviewed(
                        root,
                        config,
                        &decision,
                        chat,
                        approve,
                        progress,
                        request["id"].as_str().unwrap_or(""),
                        None,
                    ),
                )
                .await
                .map(|r| r.text)
                .map_err(|e| e.to_string())
        } else {
            Err("This tool is unavailable to mods in the current mode".into())
        };
        crate::mods::complete_request(
            root,
            chat,
            id,
            request["hash"].as_str().unwrap_or(""),
            serde_json::json!({"requestId":request["id"],"action":action,"result":result.as_ref().ok(),"error":result.as_ref().err()}),
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn execute_tool_outcome_reviewed_inner(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    call_id: &str,
    expected: Option<&mcp_review::McpReview>,
) -> Result<crate::integrations::mcp_result::ToolOutcome, OrchestrationError> {
    run_control::check()?;
    if decision.action == "view_image" {
        use crate::media::tool_images::{self, ViewTarget};
        let target =
            tool_images::resolve_view_target(&decision.input.path, &decision.input.artifact_id)
                .map_err(OrchestrationError::Agent)?;
        let root = root.to_path_buf();
        let config = config.clone();
        let chat = chat_id.to_owned();
        let call = call_id.to_owned();
        let (path, artifact_id) = match target {
            ViewTarget::Artifact(id) => (String::new(), id.to_owned()),
            ViewTarget::Path(path) => (path.to_owned(), String::new()),
        };
        // Phase 1: validate and identify the image without saving anything, so a
        // repeat request can be refused before it creates another chat artifact.
        let loaded = {
            let (chat, artifact_id) = (chat.clone(), artifact_id.clone());
            tokio::task::spawn_blocking(move || -> Result<LoadedImage, String> {
                if !artifact_id.is_empty() {
                    let (entry, bytes) = crate::mcp_result::read_artifact(&artifact_id)?;
                    if entry.chat_id != chat || entry.kind != "image" {
                        return Err("Image artifact is not available in this conversation.".into());
                    }
                    return Ok((tool_images::content_key(&bytes), None));
                }
                let path = agent_read_path(&root, &path, &config).map_err(|e| e.to_string())?;
                let path = assert_path_capability(&path, Capability::Read, &config)
                    .map_err(|e| e.to_string())?;
                if std::fs::metadata(&path).map_err(|e| e.to_string())?.len()
                    > tool_images::ORIGINAL_LIMIT as u64
                {
                    return Err("Image exceeds 20 MiB".into());
                }
                let mime = match path
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("")
                    .to_ascii_lowercase()
                    .as_str()
                {
                    "png" => "image/png",
                    "jpg" | "jpeg" => "image/jpeg",
                    "gif" => "image/gif",
                    "webp" => "image/webp",
                    "svg" => "image/svg+xml",
                    _ => return Err("Unsupported image type".into()),
                };
                let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
                let name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Image")
                    .to_owned();
                Ok((tool_images::content_key(&bytes), Some((bytes, mime, name))))
            })
            .await
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?
            .map_err(OrchestrationError::Agent)?
        };
        let (key, file) = loaded;
        if !reliability::first_image_view(&key) {
            return Ok(crate::mcp_result::ToolOutcome::from(
                "Image already in context: this exact image was shown earlier in this turn, so it \
                 was not attached again. Use what you saw, capture something different, or call finish."
                    .to_string(),
            ));
        }
        // Phase 2: attach (and, for files, persist) the image.
        let out = tokio::task::spawn_blocking(
            move || -> Result<crate::mcp_result::ToolOutcome, String> {
                match file {
                    None => tool_images::view_artifact(&artifact_id, &chat),
                    Some((bytes, mime, name)) => {
                        tool_images::save(&bytes, mime, &name, &chat, &call, "view_image", None)
                    }
                }
            },
        )
        .await
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?
        .map_err(OrchestrationError::Agent)?;
        progress(AgentProgress::ToolArtifacts {
            call_id: call_id.into(),
            artifacts: out.artifacts.clone(),
            warnings: out.warnings.clone(),
        });
        return Ok(out);
    }
    if matches!(decision.action.as_str(), "mcp_tool" | "mcp_list_tools") {
        let mut out = super::plugins_mcp::execute_reviewed(
            &decision.action,
            &decision.input,
            config,
            chat_id,
            call_id,
            expected,
            approve,
            progress,
        )
        .await?;
        if !out.images.is_empty()
            && config.extra.get("mcpImageInput").and_then(Value::as_bool) == Some(false)
        {
            out.images.clear();
            let warning="Model image input is disabled for MCP. Use attachment metadata only; do not claim visual inspection.".to_string();
            out.text.push_str(&format!("\nWarning: {warning}"));
            out.warnings.push(warning);
        }
        if !out.artifacts.is_empty() || !out.warnings.is_empty() {
            progress(AgentProgress::ToolArtifacts {
                call_id: call_id.into(),
                artifacts: out.artifacts.clone(),
                warnings: out.warnings.clone(),
            });
        }
        Ok(out)
    } else {
        let mut tool_config = config.clone();
        let bridge = if decision.action == "run_shell" && !decision.input.background {
            crate::browser::workflow::ShellBridge::prepare(
                &decision.input.command,
                root,
                &mut tool_config,
            )
            .map_err(OrchestrationError::Agent)?
        } else {
            None
        };
        let checkpoint = reliability::checkpoint();
        let text = match execute_tool_text(root, &tool_config, decision, chat_id, approve, progress)
            .await
        {
            Ok(text) => text,
            Err(error) => {
                reliability::complete_tool(&decision.action, call_id, checkpoint, false);
                return Err(error);
            }
        };
        let mut out = crate::mcp_result::ToolOutcome::from(text);
        let mut workflow_verified = None;
        if let Some(bridge) = bridge {
            match bridge.images(chat_id, call_id) {
                Ok((images, verified)) => {
                    workflow_verified = Some(verified);
                    out.text.push_str(&images.text);
                    out.images.extend(images.images);
                    out.artifacts.extend(images.artifacts);
                    out.warnings.extend(images.warnings);
                    progress(AgentProgress::ToolArtifacts {
                        call_id: call_id.into(),
                        artifacts: out.artifacts.clone(),
                        warnings: out.warnings.clone(),
                    });
                }
                Err(error) => {
                    out.text.push_str(&format!("\nWarning: {error}"));
                    out.warnings.push(error);
                    workflow_verified = Some(false);
                }
            }
        }
        let mut entries = reliability::complete_tool(
            &decision.action,
            call_id,
            checkpoint,
            out.status == crate::mcp_result::ToolStatus::Success,
        );
        if let Some(verified) = workflow_verified {
            entries = reliability::mark_workflow(call_id, verified);
        }
        if decision.action == "shell_output" {
            entries = reliability::for_job(&decision.input.job_id);
        }
        if !entries.is_empty() {
            out.status = if entries
                .iter()
                .any(|e| e.state == reliability::ExecutionState::Cancelled)
            {
                crate::mcp_result::ToolStatus::Cancelled
            } else if entries
                .iter()
                .any(|e| e.state == reliability::ExecutionState::Failed)
            {
                crate::mcp_result::ToolStatus::Failed
            } else if entries
                .iter()
                .any(|e| e.state == reliability::ExecutionState::Running)
            {
                crate::mcp_result::ToolStatus::Running
            } else if entries
                .iter()
                .any(|e| e.state == reliability::ExecutionState::Unknown)
            {
                crate::mcp_result::ToolStatus::Unknown
            } else {
                crate::mcp_result::ToolStatus::Success
            };
            progress(AgentProgress::ExecutionUpdate {
                call_id: call_id.into(),
                executions: entries,
            });
        }
        Ok(out)
    }
}

async fn execute_tool_text(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<String, OrchestrationError> {
    if let Some(permissions) = crate::swarm::current_permissions() {
        if !permissions.allows(&decision.action) {
            return Err(OrchestrationError::Agent(format!(
                "action '{}' is not permitted by this subagent's permissions",
                decision.action
            )));
        }
    }
    let input = &decision.input;
    match decision.action.as_str() {
        "mod_help" | "mod_list" | "mod_validate" | "mod_publish" => {
            super::mods::execute(&decision.action, input, root, config, chat_id, approve_cb).await
        }
        "list_files" | "read_file" | "note_write" | "apply_patch" | "write_file" => {
            super::files::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "search_code" | "symbols" | "find_definition" | "find_references" | "repo_map"
        | "semantic_index" | "semantic_search" => {
            super::code_search::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "knowledge_search" | "web_search" | "image_search" => {
            super::web::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "weather" | "stock" | "calculation" | "memory_recall" => {
            super::misc::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "git_status" | "git_diff" | "git_log" | "git_branch" | "git_checkpoint"
        | "git_rollback" | "git_restore_file" | "git_commit" | "git_create_branch" => {
            super::git::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "create_plan" | "update_plan" | "request_user_approval" | "ask_user" => {
            super::planning::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
                progress,
            )
            .await
        }
        "detect_project" | "list_tests" | "read_diagnostics" | "view_image" | "search_docs"
        | "create_project_doc" => {
            super::project::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "avatar_signal" => super::avatar::execute(input).await,
        "run_plugin" | "dispatch_agents" | "dispatch_subagent" | "mcp_tool" | "mcp_list_tools" => {
            super::plugins_mcp::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
                progress,
            )
            .await
        }
        "run_shell" | "shell_output" | "kill_shell" | "verify" | "run_tests" | "run_typecheck"
        | "run_linter" => {
            super::shell::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "video_trim"
        | "video.trim"
        | "video_remove_silence"
        | "video.remove_silence"
        | "video_resize"
        | "video_merge"
        | "video_export"
        | "video.export"
        | "video_extract_audio"
        | "video_filmstrip"
        | "video.filmstrip"
        | "video_waveform"
        | "video.waveform"
        | "speech_transcribe"
        | "subtitle_generate"
        | "subtitle.generate"
        | "subtitle_translate"
        | "subtitle.translate"
        | "subtitle_burn"
        | "timeline_reorder"
        | "timeline.reorder"
        | "effect_zoom_on_speaker"
        | "effect.zoom_on_speaker"
        | "audio_duck_music"
        | "audio.duck_music"
        | "make_shorts"
        | "video.make_shorts"
        | "generate_image"
        | "image_studio.generate"
        | "image_generate"
        | "generate_video"
        | "veo.generate"
        | "video_generate" => {
            super::media::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        other => Err(OrchestrationError::Agent(format!(
            "unsupported code-agent action '{}'",
            other
        ))),
    }
}

/// Bridge for callers outside this module (e.g. the Gemini Live realtime voice
/// session) that receive an action name and raw JSON input rather than a parsed
/// `AgentDecision`, and so can't construct one directly since its fields are
/// private to this module.
pub(crate) async fn execute_tool_from_json<Approve>(
    root: &Path,
    config: &MintConfig,
    action: &str,
    input: Value,
    chat_id: &str,
    approve_cb: &mut Approve,
) -> Result<String, OrchestrationError>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send,
{
    let decision = AgentDecision {
        thought: String::new(),
        action: action.to_string(),
        input: parse_tool_input(action, input)?,
    };
    // No progress channel from the realtime voice session — tool-call
    // activity isn't surfaced there the way it is in the CLI/GUI.
    execute_tool(root, config, &decision, chat_id, approve_cb, &mut |_| {}).await
}

pub(crate) fn append_native_tool_results(
    messages: &mut Vec<ChatMessage>,
    response_text: &str,
    results: &[ToolResultEntry],
    signatures: &std::collections::HashMap<String, String>,
    images: &std::collections::HashMap<String, Vec<String>>,
) {
    let mut assistant_content: Vec<ContentBlock> = Vec::new();
    if !response_text.is_empty() {
        assistant_content.push(ContentBlock::Text {
            text: response_text.to_string(),
        });
    }
    let mut tool_result_content: Vec<ContentBlock> = Vec::new();
    for ToolResultEntry {
        call_id,
        action,
        input: input_value,
        text: final_result,
        status,
    } in results
    {
        // Activity annotations are for the UI, not parameters of the provider tool schema.
        let mut tool_input = input_value.clone();
        if let Some(object) = tool_input.as_object_mut() {
            object.remove("skill_name");
        }
        assistant_content.push(ContentBlock::ToolUse {
            id: call_id.clone(),
            name: action.clone(),
            input: tool_input,
            thought_signature: signatures.get(call_id).cloned(),
        });
        tool_result_content.push(ContentBlock::ToolResult {
            tool_use_id: call_id.clone(),
            content: final_result.clone(),
            is_error: matches!(
                *status,
                crate::mcp_result::ToolStatus::Failed
                    | crate::mcp_result::ToolStatus::TimedOut
                    | crate::mcp_result::ToolStatus::Cancelled
            ),
        });
        if let Some(data_uris) = images.get(call_id) {
            for data_uri in data_uris {
                tool_result_content.push(ContentBlock::Image {
                    data_uri: data_uri.clone(),
                });
            }
        }
    }
    if !assistant_content.is_empty() {
        messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: assistant_content,
        });
    }
    if !tool_result_content.is_empty() {
        messages.push(ChatMessage {
            role: ChatRole::Tool,
            content: tool_result_content,
        });
    }
}

pub(crate) fn extract_modified_paths(results: &[ToolResultEntry]) -> Vec<String> {
    let mut paths = Vec::new();
    for entry in results {
        if matches!(entry.status, crate::mcp_result::ToolStatus::Success) {
            match entry.action.as_str() {
                "apply_patch" => {
                    let path_opt = entry
                        .input
                        .get("patch")
                        .and_then(|p| p.get("path"))
                        .and_then(|p| p.as_str())
                        .or_else(|| entry.input.get("path").and_then(|p| p.as_str()));
                    if let Some(path) = path_opt {
                        let trimmed = path.trim();
                        if !trimmed.is_empty() {
                            paths.push(trimmed.to_string());
                        }
                    }
                }
                "write_file" | "apply_code_edits" | "patch_file" | "delete_file" => {
                    if let Some(path) = entry.input.get("path").and_then(|p| p.as_str()) {
                        let trimmed = path.trim();
                        if !trimmed.is_empty() {
                            paths.push(trimmed.to_string());
                        }
                    }
                }
                _ => {}
            }
        }
    }
    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn extract_modified_paths_captures_apply_patch_and_write_file() {
        let results = vec![
            ToolResultEntry {
                call_id: "c1".into(),
                action: "apply_patch".into(),
                input: json!({
                    "patch": {
                        "path": "crates/mint-core/src/lib.rs",
                        "hunks": []
                    }
                }),
                text: "success".into(),
                status: crate::mcp_result::ToolStatus::Success,
            },
            ToolResultEntry {
                call_id: "c2".into(),
                action: "write_file".into(),
                input: json!({
                    "path": "README.md",
                    "fileContent": "new content"
                }),
                text: "created".into(),
                status: crate::mcp_result::ToolStatus::Success,
            },
            ToolResultEntry {
                call_id: "c3".into(),
                action: "read_file".into(),
                input: json!({"path": "src/main.rs"}),
                text: "read".into(),
                status: crate::mcp_result::ToolStatus::Success,
            },
            ToolResultEntry {
                call_id: "c4".into(),
                action: "apply_patch".into(),
                input: json!({"patch": {"path": "failed.rs"}}),
                text: "failed".into(),
                status: crate::mcp_result::ToolStatus::Failed,
            },
        ];

        let modified = extract_modified_paths(&results);
        assert_eq!(modified, vec!["crates/mint-core/src/lib.rs", "README.md"]);
    }
}

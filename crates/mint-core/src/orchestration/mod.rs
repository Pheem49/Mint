use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;

pub mod approvals;
#[cfg(test)]
mod image_regressions;
mod mcp_review;
#[cfg(test)]
mod mod_authoring_regressions;
mod progress_clock;
pub mod reliability;
pub use reliability::{ExecutionEvidence, ExecutionState};
pub mod diagnostics;
pub mod loop_guard;
pub mod trajectory_schema;
pub mod truncation;
pub mod types;
pub use trajectory_schema::{StandardTrajectoryRun, StandardTrajectoryStep, StepTokens};
pub use types::*;
mod repeated_failures;
#[cfg(all(test, unix))]
mod review_regressions;
pub(crate) mod run_control;
use repeated_failures::RepeatedFailures;

use crate::chat::{
    ChatMessage, ChatRole, ChatStreamEvent, ContentBlock, send_chat_with_fallback,
    stream_chat_events_with_fallback, stream_chat_with_fallback,
};
use crate::code_tools::{
    CodeEdit, apply_code_edits, build_code_patch, list_code_files, propose_code_edits,
    read_code_file, search_code,
};
use crate::config::ToolCallingMode;
use crate::knowledge::KnowledgeStore;
use crate::plugins::execute_native_plugin;
use crate::prompts::tool_catalog::tool_catalog;
use crate::semantic::{index_semantic_code, search_semantic_code};
use crate::symbols::build_symbol_index;
use crate::{
    Capability, ChatError, ChatRequest, ChatResponse, DEFAULT_CONVERSATION_ID, MemoryError,
    MemoryStore, MintConfig, assert_path_capability, classify_shell_command, send_chat,
    stream_chat,
};

pub(crate) const CONTEXT_LIMIT: usize = 15;
/// Per-message cap (in `chars`, not bytes — Thai text is multi-byte UTF-8)
/// applied to each recalled `user_text`/`ai_text` when building the "recent
/// conversation context" injected into a new task's opening system prompt or
/// structured `native_messages`.
/// Sized at 4,000 characters to comfortably accommodate rich code snippets,
/// full multi-line instructions, and Thai conversational text without truncation.
pub(crate) const MAX_CONTEXT_MESSAGE_CHARS: usize = 4000;

pub async fn resolve_github_links(message: &str, config: &MintConfig) -> String {
    // Check if a GitHub MCP server is configured in Settings
    let github_mcp_configured = crate::mcp::configured_mcp_servers(config)
        .ok()
        .map(|servers| servers.contains_key("github"))
        .unwrap_or(false);

    if github_mcp_configured {
        // If GitHub MCP is active, we let it handle the repo via tool calls
        // to avoid duplicate/redundant context.
        return message.to_string();
    }

    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"https?://(?:www\.)?github\.com/([a-zA-Z0-9\-_.]+)/([a-zA-Z0-9\-_.]+)")
            .unwrap()
    });

    let mut resolved_msg = message.to_string();
    let mut resolved_repos = std::collections::HashSet::new();

    for caps in re.captures_iter(message) {
        if let (Some(owner_match), Some(repo_match)) = (caps.get(1), caps.get(2)) {
            let owner = owner_match.as_str();
            let mut repo = repo_match.as_str().to_string();
            if repo.ends_with(".git") {
                repo = repo[..repo.len() - 4].to_string();
            }
            let repo_clean: String = repo
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
                .collect();

            let repo_key = format!("{owner}/{repo_clean}");
            if resolved_repos.insert(repo_key.clone())
                && let Ok(summary) =
                    crate::code_tools::fetch_github_repo_summary(owner, &repo_clean).await
            {
                resolved_msg.push_str(&format!(
                        "\n\n--- Auto-Resolved GitHub Metadata for {} ---\n{}\n--------------------------------------------",
                        repo_key, summary
                    ));
            }
        }
    }
    resolved_msg
}

pub async fn orchestrate_chat(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    if let Some(workspace) = request
        .workspace_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        memory.set_chat_session_workspace(&request_chat_id(request), Some(workspace.trim()))?;
    }
    let mut turn = TurnLease::start(&memory, &request_chat_id(request), &request.message).await?;
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let response = match send_chat(config, &enriched).await {
        Ok(response) => response,
        Err(error) => {
            turn.fail();
            return Err(error.into());
        }
    };
    turn.complete(&response)?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        request.workspace_path.clone(),
        request_chat_id(request),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        turn.id,
    );
    Ok(response)
}

pub async fn orchestrate_chat_stream<F>(
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: F,
) -> Result<ChatResponse, OrchestrationError>
where
    F: FnMut(String),
{
    let memory = MemoryStore::open_default()?;
    if let Some(workspace) = request
        .workspace_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        memory.set_chat_session_workspace(&request_chat_id(request), Some(workspace.trim()))?;
    }
    let mut turn = TurnLease::start(&memory, &request_chat_id(request), &request.message).await?;
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let response = match {
        let mut on_chunk = on_chunk;
        let chat = turn.chat_id.clone();
        let id = turn.id;
        stream_chat(config, &enriched, move |chunk: String| {
            crate::companion::chunk(&chat, id, &chunk);
            on_chunk(chunk);
        })
    }
    .await
    {
        Ok(response) => response,
        Err(error) => {
            turn.fail();
            return Err(error.into());
        }
    };
    turn.complete(&response)?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        request.workspace_path.clone(),
        request_chat_id(request),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        turn.id,
    );
    Ok(response)
}

pub async fn orchestrate_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    if let Some(workspace) = request
        .workspace_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        memory.set_chat_session_workspace(&request_chat_id(request), Some(workspace.trim()))?;
    }
    let mut turn = TurnLease::start(&memory, &request_chat_id(request), &request.message).await?;
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let (response, fallback) = match send_chat_with_fallback(config, &enriched).await {
        Ok(result) => result,
        Err(error) => {
            turn.fail();
            return Err(error.into());
        }
    };
    turn.complete(&response)?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        request.workspace_path.clone(),
        request_chat_id(request),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        turn.id,
    );
    Ok((response, fallback))
}

pub async fn orchestrate_chat_stream_with_fallback<F>(
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: F,
) -> Result<(ChatResponse, Option<String>), OrchestrationError>
where
    F: FnMut(String),
{
    let memory = MemoryStore::open_default()?;
    if let Some(workspace) = request
        .workspace_path
        .as_deref()
        .filter(|path| !path.trim().is_empty())
    {
        memory.set_chat_session_workspace(&request_chat_id(request), Some(workspace.trim()))?;
    }
    let mut turn = TurnLease::start(&memory, &request_chat_id(request), &request.message).await?;
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let (response, fallback) = match {
        let mut on_chunk = on_chunk;
        let chat = turn.chat_id.clone();
        let id = turn.id;
        stream_chat_with_fallback(config, &enriched, move |chunk: String| {
            crate::companion::chunk(&chat, id, &chunk);
            on_chunk(chunk);
        })
    }
    .await
    {
        Ok(result) => result,
        Err(error) => {
            turn.fail();
            return Err(error.into());
        }
    };
    turn.complete(&response)?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        request.workspace_path.clone(),
        request_chat_id(request),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
        turn.id,
    );
    Ok((response, fallback))
}

/// Read-only tools whose *outcome* (not just input) identifies a stuck loop:
/// image/page observations and headless-Chrome captures run through the shell.
fn is_observation_action(action: &str, command: &str) -> bool {
    if action == "view_image" {
        return true;
    }
    action == "run_shell" && {
        let command = command.to_ascii_lowercase();
        command.contains("chrome") || command.contains("chromium")
    }
}

// Full skill reads must reach the model exactly; other tools keep bounded output.
// Large outputs (> 3,000 chars) are safely offloaded to .mint/scratch/tool_outputs/ when root is available.
pub(super) fn model_tool_result(root: Option<&Path>, action: &str, result: &str) -> String {
    if action == "read_file" && result.starts_with("[Loaded skill: ") {
        result.to_owned()
    } else {
        truncation::truncate_tool_output(
            root,
            action,
            result,
            truncation::TruncationConfig::default(),
        )
    }
}

pub(super) fn tool_progress_input(root: &Path, decision: &AgentDecision) -> Value {
    let mut input = serde_json::to_value(&decision.input).unwrap_or(Value::Null);
    if decision.action == "read_file"
        && let Some(skill) = crate::skills::skill_for_read_path(root, &decision.input.path)
        && let Some(object) = input.as_object_mut()
    {
        object.insert("skill_name".into(), Value::String(skill.name));
    }
    input
}

/// Truncates `text` to at most `max_chars` characters (not bytes, so this
/// never splits a multi-byte UTF-8 character) for injection into a "recent
/// context" summary. Cheap no-op for the common case of a short message.
pub(super) fn truncate_for_context(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push_str("... [truncated]");
    truncated
}

/// Deduplicates building user content blocks across plain chat enrichment and agent turn initialization.
pub(crate) fn build_user_content_blocks(
    text: String,
    image_data: Option<&str>,
    audio_data: Option<&str>,
    video_data: Option<&str>,
) -> Vec<ContentBlock> {
    let mut content = vec![ContentBlock::Text { text }];
    if let Some(image_data) = image_data {
        for img in image_data.split_whitespace() {
            content.push(ContentBlock::Image {
                data_uri: img.to_owned(),
            });
        }
    }
    if let Some(audio_data) = audio_data {
        for aud in audio_data.split_whitespace() {
            content.push(ContentBlock::Audio {
                data_uri: aud.to_owned(),
            });
        }
    }
    if let Some(video_data) = video_data {
        for vid in video_data.split_whitespace() {
            content.push(ContentBlock::Video {
                data_uri: vid.to_owned(),
            });
        }
    }
    content
}

fn construct_system_prompt(
    config: &MintConfig,
    memory: &MemoryStore,
    chat_id: &str,
    root: &Path,
    resolved_task: &str,
    plan_mode: bool,
    system_prompt_native: bool,
    user_name: Option<&str>,
    pinned_mcp_server: Option<&str>,
) -> String {
    let mut system_prompt = build_system_prompt(
        config,
        plan_mode,
        system_prompt_native,
        user_name,
        pinned_mcp_server,
    );
    append_memory_context(
        &mut system_prompt,
        memory,
        chat_id,
        Some(root.to_string_lossy().as_ref()),
        Some(resolved_task),
        config.semantic_fact_recall,
        !system_prompt_native,
    );
    if config.memory_recall
        && let Some(recall) = render_recalled_messages(chat_id, resolved_task)
    {
        system_prompt = format!("{}\n\n{}", system_prompt.trim(), recall);
    }
    system_prompt
}

fn enrich_request(
    config: &MintConfig,
    memory: &MemoryStore,
    request: &ChatRequest,
) -> Result<ChatRequest, MemoryError> {
    let mut enriched = request.clone();

    // If structured messages were not explicitly provided, build conversation history messages
    // so multi-turn context is preserved natively in plain chat mode across all native providers.
    let is_native = config.tool_calling_mode() == ToolCallingMode::Native;
    if is_native && enriched.messages.is_none() {
        let history = build_conversation_history_messages(
            memory,
            &request_chat_id(request),
            CONTEXT_LIMIT,
            MAX_CONTEXT_MESSAGE_CHARS,
        );
        if !history.is_empty() {
            let mut messages = history;
            let content = build_user_content_blocks(
                request.message.clone(),
                request.image_data_uri.as_deref(),
                request.audio_data_uri.as_deref(),
                request.video_data_uri.as_deref(),
            );
            messages.push(ChatMessage {
                role: ChatRole::User,
                content,
            });
            enriched.messages = Some(messages);
        }
    }

    let mut profile_instructions = String::new();
    if let Ok(Some(name)) = memory.get_profile("name")
        && !name.trim().is_empty()
    {
        profile_instructions.push_str(&format!("User Name: {}\n", name.trim()));
    }
    if let Ok(Some(preferences)) = memory.get_profile("preferences")
        && !preferences.trim().is_empty()
    {
        profile_instructions.push_str(&format!(
            "User Preferences & Profile:\n{}\n",
            preferences.trim()
        ));
    }

    if !profile_instructions.is_empty() {
        enriched.system_instruction = format!(
            "{}\n\nUser Profile Information:\n{}",
            enriched.system_instruction.trim(),
            profile_instructions.trim()
        )
        .trim()
        .to_owned();
    }

    if let Some(facts) = render_memory_facts(
        memory,
        request.workspace_path.as_deref(),
        crate::subagent_name(&request_chat_id(request)),
        Some(&request.message),
        config.semantic_fact_recall,
    ) {
        enriched.system_instruction = format!(
            "{}\n\nRemembered facts:\n{}",
            enriched.system_instruction.trim(),
            facts
        )
        .trim()
        .to_owned();
    }

    if config.memory_recall
        && let Some(recall) = render_recalled_messages(&request_chat_id(request), &request.message)
    {
        enriched.system_instruction =
            format!("{}\n\n{}", enriched.system_instruction.trim(), recall)
                .trim()
                .to_owned();
    }

    // Inject active AI model/provider context to system instructions
    let active_model_info = format!(
        "\n\n[Active Environment Context]\n\
         You are running on: {}\n\
         Using AI Model: {}\n",
        config.ai_provider,
        config.active_model()
    );
    enriched.system_instruction.push_str(&active_model_info);

    // If structured messages are not in use (e.g. no messages array populated),
    // fallback to injecting recent transcript into system prompt if available.
    if enriched.messages.is_none()
        && let Some(transcript) = render_conversation_transcript(
            memory,
            &request_chat_id(request),
            CONTEXT_LIMIT,
            MAX_CONTEXT_MESSAGE_CHARS,
        )
    {
        enriched.system_instruction = format!(
            "{}\n\nRecent conversation context:\n{}",
            enriched.system_instruction.trim(),
            transcript
        )
        .trim()
        .to_owned();
    }
    Ok(enriched)
}

fn request_chat_id(request: &ChatRequest) -> String {
    let raw = request
        .chat_id
        .as_deref()
        .map(str::trim)
        .filter(|chat_id| !chat_id.is_empty())
        .unwrap_or(DEFAULT_CONVERSATION_ID);
    crate::agent::memory::scoped_chat_id(raw, request.workspace_path.as_deref())
}

use crate::prompts::agent::build_system_prompt;

mod context_render;
pub mod decision_parsing;
mod memory_skill;
mod tools;
mod verification;
mod workspace_helpers;

use context_render::*;
use decision_parsing::*;
pub use decision_parsing::{SwarmTaskInput, is_dummy_value};
pub(crate) use memory_skill::*;
pub use tools::execute_mod_requests;
pub(crate) use tools::execute_tool_from_json;
use tools::*;
use verification::*;
use workspace_helpers::*;

/// Hard ceiling on tool-call round-trips per task. Each step resends the
/// whole accumulated conversation, so total tokens for one task scale
/// roughly with step count. Was lowered from 32 to 24 to cap runaway/looping
/// tasks earlier, but 24 turned out too tight for legitimate multi-file
/// work, cutting it off mid-task — now configurable via `MintConfig::agent_max_steps` (default 40).
// A tool gets its own correction budget, renewed after successful execution.
const MAX_OBSERVATION_BYTES: usize = 16_000;
/// Compact `native_messages` once a step's reported token usage crosses this
/// fraction of `MintConfig::context_window_tokens`. Every step resends the whole
/// accumulated history, so this is the main lever on real per-task token volume
/// (prompt caching only discounts price, not what's counted). Compaction fires
/// the moment the threshold is crossed and shrinks the history before the next
/// request, so `0.75` keeps a generous verbatim window without letting the next
/// request grow past the real limit.
const COMPACTION_TRIGGER_RATIO: f64 = 0.75;
/// Number of most-recent Assistant/Tool step-pairs kept verbatim (uncompacted)
/// in `native_messages`.
const COMPACTION_KEEP_RECENT_STEPS: usize = 3;
/// Number of most-recent Tool step-pairs that retain full `ContentBlock::Image` pixels.
/// Older tool turns have image blocks pruned to reduce prompt cache and token usage.
const MAX_RETAINED_IMAGE_STEPS: usize = 2;
/// Adaptive token threshold to compact context in long-running agent tasks (>= 20 steps),
/// preventing cumulative prompt cache and token bloat before hitting the full window ratio.
const ADAPTIVE_COMPACTION_TOKEN_CEILING: u32 = 40_000;
const ADAPTIVE_COMPACTION_MIN_STEPS: usize = 20;
/// How many times a step retries after every configured provider comes back
/// `ChatError::NetworkUnavailable` before finally giving up — swapping
/// providers is pointless when the network itself is down, so this waits and
/// tries the same request again instead, with `AgentProgress::WaitingForNetwork`
/// keeping the user informed of which attempt it's on.
const NETWORK_RETRY_ATTEMPTS: usize = 4;
const NETWORK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

fn resolve_agent_config(
    config: &MintConfig,
    agent_id: Option<&str>,
    trajectory: &[String],
) -> (MintConfig, String, Option<String>, Option<String>) {
    if !config.enable_agent_collaboration {
        return (config.clone(), "".to_string(), None, None);
    }

    let enabled_agents: Vec<&crate::config::AgentConfig> =
        config.agents.iter().filter(|a| a.enabled).collect();
    if enabled_agents.is_empty() {
        return (config.clone(), "".to_string(), None, None);
    }

    let active_agent = if let Some(id) = agent_id {
        enabled_agents.iter().find(|a| a.id == id).copied()
    } else {
        // Multi-Agent Pipeline Collaboration (Planner -> Coder -> Reviewer)
        let plan_created = trajectory
            .iter()
            .any(|step| step.contains("- Action: create_plan"));
        if !plan_created {
            if let Some(planner) = enabled_agents.iter().find(|a| a.id == "planner") {
                Some(*planner)
            } else {
                enabled_agents.iter().find(|a| a.id == "coder").copied()
            }
        } else {
            // Check if edits have been made to verify
            let edits_made = trajectory.iter().any(|step| {
                step.contains("- Action: write_file") || step.contains("- Action: apply_patch")
            });
            if edits_made {
                if let Some(last_step) = trajectory.last() {
                    if last_step.contains("- Action: write_file")
                        || last_step.contains("- Action: apply_patch")
                    {
                        if let Some(reviewer) = enabled_agents.iter().find(|a| a.id == "reviewer") {
                            Some(*reviewer)
                        } else {
                            enabled_agents.iter().find(|a| a.id == "coder").copied()
                        }
                    } else {
                        enabled_agents.iter().find(|a| a.id == "coder").copied()
                    }
                } else {
                    enabled_agents.iter().find(|a| a.id == "coder").copied()
                }
            } else {
                enabled_agents.iter().find(|a| a.id == "coder").copied()
            }
        }
    };

    let Some(agent) = active_agent else {
        return (config.clone(), "".to_string(), None, None);
    };

    let mut cfg_clone = config.clone();
    cfg_clone.ai_provider = agent.provider.clone();
    cfg_clone.gemini_model = agent.model.clone();
    cfg_clone.openai_model = agent.model.clone();
    cfg_clone.anthropic_model = agent.model.clone();
    cfg_clone.openrouter_model = agent.model.clone();
    cfg_clone.deepseek_model = agent.model.clone();
    cfg_clone.hf_model = agent.model.clone();
    cfg_clone.local_model_name = agent.model.clone();
    cfg_clone.ollama_model = agent.model.clone();

    if let Some(key) = &agent.api_key
        && !key.trim().is_empty()
    {
        match agent.provider.as_str() {
            "gemini" => cfg_clone.api_key = key.clone(),
            "openai" => cfg_clone.openai_api_key = key.clone(),
            "anthropic" => cfg_clone.anthropic_api_key = key.clone(),
            "openrouter" => cfg_clone.openrouter_api_key = key.clone(),
            "deepseek" => cfg_clone.deepseek_api_key = key.clone(),
            "huggingface" => cfg_clone.hf_api_key = key.clone(),
            _ => {}
        }
    }

    (
        cfg_clone,
        agent.system_instruction.clone(),
        Some(agent.name.clone()),
        Some(agent.model.clone()),
    )
}

enum AgentStreamChunk {
    DirectText(String),
    FinalSummary(String),
}

async fn stream_chat_with_network_retry(
    config: &MintConfig,
    request: &ChatRequest,
    thought_id: &str,
    thought_started: Instant,
    fast_mode: bool,
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    stream_json_prompt: bool,
    allow_final_stream: bool,
    on_stream_chunk: &mut (dyn FnMut(AgentStreamChunk) + Send),
) -> Result<(ChatResponse, Option<String>), ChatError> {
    let mut attempt = 0;
    let mut summary_stream = FinishSummaryStream::default();
    loop {
        let result = stream_chat_events_with_fallback(config, request, |event| match event {
            ChatStreamEvent::ReasoningDelta { delta } if !fast_mode && !delta.is_empty() => {
                progress(AgentProgress::ThinkingDelta {
                    id: thought_id.to_owned(),
                    delta,
                    elapsed_ms: thought_started.elapsed().as_millis() as u64,
                });
            }
            ChatStreamEvent::TextDelta { delta } if allow_final_stream => {
                if stream_json_prompt {
                    summary_stream.push_json_prompt(&delta, &mut |chunk| {
                        on_stream_chunk(AgentStreamChunk::FinalSummary(chunk));
                    });
                }
            }
            ChatStreamEvent::ToolCallDelta {
                index,
                name,
                arguments,
                input,
            } if allow_final_stream => summary_stream.push_tool_call(
                index,
                name.as_deref(),
                arguments.as_deref(),
                input.as_ref(),
                &mut |chunk| on_stream_chunk(AgentStreamChunk::FinalSummary(chunk)),
            ),
            _ => {}
        })
        .await;
        match result {
            Err(ChatError::NetworkUnavailable) if attempt < NETWORK_RETRY_ATTEMPTS => {
                attempt += 1;
                progress(AgentProgress::WaitingForNetwork {
                    attempt,
                    max_attempts: NETWORK_RETRY_ATTEMPTS,
                });
                tokio::time::sleep(NETWORK_RETRY_DELAY).await;
            }
            Ok((response, fallback)) => {
                if !stream_json_prompt
                    && allow_final_stream
                    && !provider_truncated_response(response.stop_reason.as_deref())
                    && response
                        .tool_calls
                        .as_ref()
                        .is_none_or(|calls| calls.is_empty())
                    && !response.text.is_empty()
                {
                    // Plain text from a native tool-calling response is only a final
                    // answer if the completed model turn contains no tool calls. Buffer
                    // it until then so an assistant preamble doesn't appear under
                    // "Mint:" while the requested tools are still about to run.
                    on_stream_chunk(AgentStreamChunk::DirectText(response.text.clone()));
                }
                return Ok((response, fallback));
            }
            other => return other,
        }
    }
}

#[derive(Default)]
struct FinishSummaryStream {
    tool_names: std::collections::HashMap<usize, String>,
    tool_arguments: std::collections::HashMap<usize, String>,
    json_prompt: String,
    emitted: String,
}

impl FinishSummaryStream {
    fn push_tool_call(
        &mut self,
        index: usize,
        name_delta: Option<&str>,
        arguments_delta: Option<&str>,
        input: Option<&serde_json::Value>,
        on_chunk: &mut (dyn FnMut(String) + Send),
    ) {
        if let Some(name) = name_delta {
            self.tool_names.insert(index, name.to_owned());
        }
        if let Some(arguments) = arguments_delta {
            self.tool_arguments
                .entry(index)
                .or_default()
                .push_str(arguments);
        }
        if let Some(input) = input {
            self.tool_arguments.insert(index, input.to_string());
        }
        if self
            .tool_names
            .get(&index)
            .is_some_and(|name| name == "finish")
            && let Some(arguments) = self.tool_arguments.get(&index)
            && let Some(summary) = json_string_field_prefix(arguments, "summary")
        {
            self.emit_new_text(&summary, on_chunk);
        }
    }

    fn push_json_prompt(&mut self, delta: &str, on_chunk: &mut (dyn FnMut(String) + Send)) {
        self.json_prompt.push_str(delta);
        if json_string_field_prefix(&self.json_prompt, "action").as_deref() == Some("finish")
            && let Some(summary) = json_string_field_prefix(&self.json_prompt, "summary")
        {
            self.emit_new_text(&summary, on_chunk);
        }
    }

    fn emit_new_text(&mut self, text: &str, on_chunk: &mut (dyn FnMut(String) + Send)) {
        if !text.starts_with(&self.emitted) || text.len() == self.emitted.len() {
            return;
        }
        let delta = text[self.emitted.len()..].to_owned();
        self.emitted.push_str(&delta);
        on_chunk(delta);
    }
}

fn unstreamed_summary_remainder<'a>(
    summary: &'a str,
    streamed_finish: &str,
    streamed_direct: &str,
) -> &'a str {
    let streamed_finish = streamed_finish.trim();
    if !streamed_finish.is_empty()
        && let Some(remainder) = summary.strip_prefix(streamed_finish)
    {
        return remainder;
    }

    let streamed_direct = streamed_direct.trim();
    if !streamed_direct.is_empty()
        && let Some(remainder) = summary.strip_prefix(streamed_direct)
    {
        return remainder;
    }

    summary
}

/// Reads a JSON string-valued property even while its value is still arriving.
/// It skips quoted values and only recognizes actual object keys, so a mention
/// of `"summary"` inside a thought or another string cannot leak to the chat.
fn json_string_field_prefix(input: &str, field: &str) -> Option<String> {
    let bytes = input.as_bytes();
    let mut cursor = 0;
    while cursor < bytes.len() {
        if bytes[cursor] != b'"' {
            cursor += 1;
            continue;
        }
        let start = cursor;
        cursor += 1;
        let mut escaped = false;
        let mut end = None;
        while cursor < bytes.len() {
            match (bytes[cursor], escaped) {
                (b'"', false) => {
                    end = Some(cursor);
                    cursor += 1;
                    break;
                }
                (b'\\', false) => escaped = true,
                (_, true) => escaped = false,
                _ => {}
            }
            cursor += 1;
        }
        if end.is_none() {
            break;
        }
        let Ok(key) = serde_json::from_str::<String>(&input[start..cursor]) else {
            continue;
        };
        if key != field {
            continue;
        }
        let mut value_start = cursor;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if value_start >= bytes.len() || bytes[value_start] != b':' {
            continue;
        }
        value_start += 1;
        while value_start < bytes.len() && bytes[value_start].is_ascii_whitespace() {
            value_start += 1;
        }
        if value_start >= bytes.len() || bytes[value_start] != b'"' {
            continue;
        }
        let value_content_start = value_start + 1;
        let mut value_cursor = value_content_start;
        let mut value_escaped = false;
        while value_cursor < bytes.len() {
            match (bytes[value_cursor], value_escaped) {
                (b'"', false) => {
                    return serde_json::from_str::<String>(&input[value_start..value_cursor + 1])
                        .ok();
                }
                (b'\\', false) => value_escaped = true,
                (_, true) => value_escaped = false,
                _ => {}
            }
            value_cursor += 1;
        }
        // Close a temporary copy of an incomplete string. Invalid partial
        // escapes (e.g. half a `\\uXXXX`) are held until the next provider delta.
        let partial = format!("\"{}\"", &input[value_content_start..]);
        return serde_json::from_str::<String>(&partial).ok();
    }
    None
}

/// Returns a boxed `dyn Future` trait object rather than being a plain
/// `async fn` (whose return type would otherwise be an opaque, compiler-
/// inferred type). This is required, not just a style choice: `execute_tool`
/// (called from this function's own loop body) can itself recurse back into
/// `orchestrate_agent_loop` via the `dispatch_subagent` tool, and Rust cannot
/// check whether an opaque `async fn` return type satisfies `Send` from code
/// that lives inside that same function's own defining scope — it's a
/// "cannot check whether the hidden type of opaque type satisfies auto
/// traits" compile error. Making the return type concrete (not opaque) up
/// front sidesteps that entirely. `.await` works identically on this as on a
/// plain `async fn`, so none of this function's existing callers need to
/// change.
pub fn orchestrate_agent_loop<'a, Approve, Progress, Chunk>(
    config: &'a MintConfig,
    task: &'a str,
    root: &'a Path,
    image_data_uri: Option<String>,
    audio_data_uri: Option<String>,
    video_data_uri: Option<String>,
    chat_id: Option<&'a str>,
    agent_id: Option<&'a str>,
    user_name: Option<&'a str>,
    pinned_mcp_server: Option<&'a str>,
    fast_mode: bool,
    plan_mode: bool,
    mut approve: Approve,
    mut progress: Progress,
    mut on_chunk: Chunk,
) -> Pin<Box<dyn Future<Output = Result<AgentResult, OrchestrationError>> + Send + 'a>>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send + 'a,
    Progress: FnMut(AgentProgress) + Send + 'a,
    Chunk: FnMut(String) + Send + 'a,
{
    Box::pin(run_control::scope(
        chat_id
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .unwrap_or(DEFAULT_CONVERSATION_ID),
        async move {
            run_control::check()?;
            let started_at = Instant::now();
            let root = root.canonicalize().map_err(|e| {
                OrchestrationError::Agent(format!(
                    "unable to resolve workspace root {}: {}",
                    root.display(),
                    e
                ))
            })?;
            let chat_id = chat_id
                .map(str::trim)
                .filter(|chat_id| !chat_id.is_empty())
                .unwrap_or(DEFAULT_CONVERSATION_ID);
            // `root` is always resolved to a real, canonicalized directory above
            // (never optional here, unlike the plain-chat path's `ChatRequest.
            // workspace_path`), so an agent-mode conversation is always scoped to
            // *some* workspace — it just never falls back to the plain global
            // "cli" bucket the way plain chat can. Idempotent on chat ids that
            // are already scoped or aren't "cli" at all (see `scoped_chat_id`).
            let chat_id =
                crate::agent::memory::scoped_chat_id(chat_id, Some(&root.to_string_lossy()));
            let chat_id = chat_id.as_str();
            let memory = MemoryStore::open_default()?;
            // Agent requests use a real workspace even when the provider fails.
            // Persist it before the first turn so all session lists group it correctly.
            memory.set_chat_session_workspace(chat_id, Some(root.to_string_lossy().as_ref()))?;
            let mut turn = run_control::run(TurnLease::start(&memory, chat_id, task)).await?;
            let record_activity = Arc::clone(&turn.activity);
            let progress_chat_id = turn.chat_id.clone();
            let companion_turn_id = turn.id;
            let chunk_chat_id = turn.chat_id.clone();
            let mut on_chunk = move |chunk: String| {
                crate::companion::chunk(&chunk_chat_id, companion_turn_id, &chunk);
                on_chunk(chunk);
            };
            let approval_chat_id = turn.chat_id.clone();
            let mut approve = move |request: &AgentApproval| {
                crate::companion::update(
                    &approval_chat_id,
                    companion_turn_id,
                    mint_companion_protocol::Status::Waiting,
                    None,
                );
                let result = approve(request);
                crate::companion::update(
                    &approval_chat_id,
                    companion_turn_id,
                    mint_companion_protocol::Status::Thinking,
                    None,
                );
                result
            };
            let mut progress_clock = progress_clock::ProgressClock::default();
            let mut progress = move |mut event: AgentProgress| {
                progress_clock.stamp(&mut event, Instant::now());
                crate::companion::progress(&progress_chat_id, companion_turn_id, &event);
                if let Ok(mut lock) = record_activity.lock() {
                    lock.push(event.clone());
                }
                progress(event);
            };
            let _ = crate::mods::event(
                &root,
                chat_id,
                "turn_start",
                serde_json::json!({"turnId":companion_turn_id}),
                None,
            );
            let mut mod_usage = crate::mods::UsageTracker::new(&root, chat_id, companion_turn_id);
            let resolved_task = resolve_github_links(task, config).await;
            let agent_result = reliability::scope(Box::pin(async {
        // Subagent runs use a synthetic `{parent_chat_id}::subagent::{name}` chat id
        // (see the `dispatch_subagent` arm in `execute_tool`) so their own memory
        // interaction doesn't leak into the parent conversation's history. That
        // same marker doubles as the depth-limit signal here: a subagent's own
        // nested loop never offers `dispatch_subagent` in its tool catalog, so it
        // can't recurse into further subagents.
        let allow_subagent_dispatch = !chat_id.contains("::subagent::");
        let skills =
            crate::skills::learned_skills_context(Some(&root), Some(chat_id)).unwrap_or_default();
        let mut observation = initial_observation(&resolved_task, &root, "See the current skill catalog in system instructions.");
        let mut pending_image = image_data_uri;
        let mut pending_audio = audio_data_uri;
        let mut pending_video = video_data_uri;

        let mut plan_mode = plan_mode;
        // A pin only means anything if it names a server that's actually configured
        // and enabled — a stale/hand-typed `@name`, or one for a server since
        // disabled in Settings, is silently dropped rather than restricting the
        // turn to a server that can't be reached.
        let pinned_mcp_server = pinned_mcp_server.filter(|p| {
            crate::mcp::list_mcp_servers()
                .map(|m| m.get(*p).is_some_and(|server| !server.disabled))
                .unwrap_or(false)
        });
        // Determined once from the base `config` (not the per-step `active_config`
        // multi-agent collaboration can substitute) — matches how `system_prompt`
        // itself is only rebuilt on plan-mode transitions, not every step.
        let system_prompt_native = config.tool_calling_mode() == ToolCallingMode::Native;
        let mut system_prompt = construct_system_prompt(
            config,
            &memory,
            chat_id,
            &root,
            &resolved_task,
            plan_mode,
            system_prompt_native,
            user_name,
            pinned_mcp_server,
        );
        let hooks = crate::hooks::list_hooks(config);

        // Structured history for native tool-calling providers, pre-populated with
        // completed conversation turns so multi-turn context (e.g. earlier questions/answers)
        // is preserved natively in the model's message sequence.
        let mut native_messages = build_conversation_history_messages(
            &memory,
            chat_id,
            CONTEXT_LIMIT,
            MAX_CONTEXT_MESSAGE_CHARS,
        );
        let history_prefix_len = native_messages.len();
        let mut current_turn_added_to_native = false;

        // Rough token estimate for the very first request going out — lets
        // the CLI seed its live counter before step 1's real response comes
        // back and there's nothing else to go on yet (~4 chars/token, a
        // widely-used approximation; ignores the tool catalog's own size —
        // built per-step further down, not worth duplicating here just for
        // this — so it undercounts a bit for native tool-calling, but only
        // needs to be in the right ballpark for a still-updating live label,
        // not exact).
        let history_chars: usize = native_messages
            .iter()
            .flat_map(|m| &m.content)
            .map(|b| match b {
                ContentBlock::Text { text } => text.chars().count(),
                _ => 0,
            })
            .sum();
        let estimated_first_step_tokens =
            ((system_prompt.chars().count() + observation.chars().count() + history_chars) / 4) as u64;

        #[allow(unused_assignments)]
        let mut final_provider = config.ai_provider.clone();
        #[allow(unused_assignments)]
        let mut final_model = "".to_string();
        let mut final_fallback = None;
        let mut final_fallback_reason = None;
        let mut action_counts = BTreeMap::<String, usize>::new();
        let mut invalid_tool_calls = ToolArgumentRetries::default();
        let mut repeated_failures = RepeatedFailures::default();
        let mut streamed_finish_summary = String::new();
        let mut streamed_direct_text = String::new();
        // Used only for retry telemetry; finish validation uses the typed ledger.
        let mut last_verify_failed: Option<bool> = None;
        let run_id = format!("run-{}", chrono::Local::now().format("%Y%m%d%H%M%S"));
        let mut trajectory_run = trajectory_schema::StandardTrajectoryRun::new(
            &run_id,
            chat_id,
            &resolved_task,
            config.active_model(),
            &config.ai_provider,
        );
        let mut trajectory: Vec<String> = Vec::new();
        // Gemini's `thoughtSignature` (see `ToolCall::thought_signature`) arrives on
        // `response.tool_calls`, keyed by call id, but the `ContentBlock::ToolUse`
        // that gets pushed onto `native_messages` for this call is only rebuilt
        // later from `step_tool_results` (itself derived from `decisions`, which
        // drops the original `ToolCall`). Stashing it here by call id — persisting
        // across the whole loop, since every past turn must keep replaying its own
        // signature on every subsequent request — avoids threading it through
        // `AgentDecision` just for this one Gemini-specific quirk.
        let mut step_thought_signatures: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut warned_json_prompt_fallback = false;
        // Last known context-window usage, as a percentage — updated after
        // every step's response (see below) so the *next* step's `Thinking`
        // progress event can show a live number instead of only learning
        // about it retroactively once `COMPACTION_TRIGGER_RATIO` is crossed.
        let mut last_context_pct: Option<u8> = None;
        // Sum of `total_tokens` reported by every step's API response this
        // turn — each step resends the full accumulated history (see the
        // module-level "resend everything" note), so this is the actual
        // billed token volume for the whole turn, not just the final step's
        // context size (`last_context_pct` above tracks that separately).
        let mut turn_total_tokens: u64 = 0;
        // The `↑ context · ↓ generated` split of `turn_total_tokens`:
        // `last_input_tokens` is the *most recent* step's prompt size (not
        // summed — it tracks context-window fill, and the resent history
        // would otherwise be counted once per step), `turn_generated_tokens`
        // sums every step's completion since each step's output is new work.
        let mut last_input_tokens: u64 = 0;
        let mut turn_generated_tokens: u64 = 0;
        let mut executed_tools: Vec<ToolExecutionRecord> = Vec::new();
        let mut mod_truncation_retries = 0;
        let mut mcp_review = mcp_review::McpReview::default();
        let mut files_modified: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        let mut files_created: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        let mut repetition_guard = loop_guard::RepetitionGuard::default();
        let max_steps = config.resolved_agent_max_steps();
        let warning_threshold = max_steps.saturating_sub(5);

        'steps: for step in 1..=max_steps {
            crate::mods::set_turn_policy(&root, chat_id, plan_mode, pinned_mcp_server);
            let _ = execute_mod_requests(&root, chat_id, config, plan_mode, &mut approve, &mut progress, None).await;
            if run_control::cancelled() { turn.interrupt(); return Err(OrchestrationError::Agent("Agent turn canceled by user".into())); }
            let thought_id = if let Some(name) = chat_id.split("::subagent::").nth(1) {
                format!("subagent-{name}-step-{step}")
            } else {
                format!("root-step-{step}")
            };
            let thought_started = Instant::now();
            let (active_config, agent_instruction, active_agent_name, active_model_name) =
                resolve_agent_config(config, agent_id, &trajectory);

            progress(AgentProgress::Thinking {
                elapsed_secs: started_at.elapsed().as_secs(),
                agent_name: active_agent_name,
                model_name: active_model_name.clone(),
                context_pct: last_context_pct,
                tokens_used: turn_total_tokens,
                input_tokens: last_input_tokens,
                generated_tokens: turn_generated_tokens,
                estimated_tokens: estimated_first_step_tokens,
            });

            let mut active_system_prompt = system_prompt.clone();
            let mcp_context = mcp_review.context();
            if !mcp_context.is_empty() { active_system_prompt.push_str(&format!("\n\n{mcp_context}")); }
            // Pin the catalog and only successfully read, unchanged skill bodies
            // to every request, including after JSON history rebuilds/compaction.
            let visible_history = if active_config.tool_calling_mode() == ToolCallingMode::Native { render_messages_as_text(&native_messages) } else { observation.clone() };
            let skill_context = crate::skills::learned_skills_context_with_history(Some(&root), Some(chat_id), &visible_history)
                .unwrap_or_default();
            if !skill_context.is_empty() {
                active_system_prompt.push_str(&format!("\n\n[Available skills and retained instructions]\n{skill_context}"));
            }
            if let Some(ref model_name) = active_model_name {
                active_system_prompt.push_str(&format!(
                    "\n\n[Active Environment Context]\n\
                 You are running on: {}\n\
                 Using AI Model: {}\n",
                    active_config.ai_provider, model_name
                ));
            }
            if !agent_instruction.is_empty() {
                active_system_prompt.push_str(&format!(
                    "\n\nYour Current Role & System Instructions:\n{}",
                    agent_instruction
                ));
            }

            if step >= warning_threshold {
                let remaining = max_steps.saturating_sub(step);
                if remaining == 0 {
                    active_system_prompt.push_str(&format!(
                        "\n\n[CRITICAL STEP LIMIT REACHED - FINAL STEP]\n\
                         You are at step {step} of {max_steps} (the absolute final step). \
                         Do NOT execute any new tool calls or start new exploration. \
                         You MUST call 'finish' right now, synthesize all findings, results, and links discovered so far, \
                         and provide a complete, clear, and comprehensive answer to the user."
                    ));
                } else {
                    active_system_prompt.push_str(&format!(
                        "\n\n[STEP BUDGET WARNING]\n\
                         You are at step {step} of {max_steps} ({remaining} step{} remaining). \
                         Do not initiate new broad exploratory tasks or repeat failed attempts. \
                         Consolidate your findings and call 'finish' with your final report before the budget is exhausted.",
                        if remaining == 1 { "" } else { "s" }
                    ));
                }
            }

            let tool_mode = active_config.tool_calling_mode();
            // A finish can still be rejected after the provider finishes
            // streaming when edited files have not been verified. Don't show
            // that provisional summary to the user as if the run were done.
            let allow_final_stream = reliability::completion("verified", "").is_ok();

            if tool_mode == ToolCallingMode::JsonPrompt
                && active_config.ai_provider == "ollama"
                && !warned_json_prompt_fallback
            {
                warned_json_prompt_fallback = true;
                progress(AgentProgress::Thought {
                    thought: format!(
                        "[Warning] Model '{}' is not on the verified native tool-calling allowlist; \
                     falling back to prompt-based JSON mode, which is less reliable than native \
                     tool calling. Switch to a Llama 3.1+/Qwen2.5+-family model for more reliable \
                     agent runs.",
                        active_config.ollama_model
                    ),
                });
            }

            let (response, fallback) = if tool_mode == ToolCallingMode::Native {
                if !current_turn_added_to_native {
                    current_turn_added_to_native = true;
                    // Native tool-calling providers build the request from `messages`
                    // and ignore ChatRequest.image_data_uri/audio_data_uri/video_data_uri
                    // entirely, so any pending attachment must be embedded as content
                    // blocks on the first user turn here, not left for the (unused)
                    // top-level fields — otherwise Agent Mode would silently drop
                    // attachments that work fine with Agent Mode off.
                    let content = build_user_content_blocks(
                        observation.clone(),
                        pending_image.take().as_deref(),
                        pending_audio.take().as_deref(),
                        pending_video.take().as_deref(),
                    );
                    native_messages.push(ChatMessage {
                        role: ChatRole::User,
                        content,
                    });
                }
                {
                    let mut emit_stream_chunk = |chunk| match chunk {
                        AgentStreamChunk::DirectText(delta) => {
                            streamed_direct_text.push_str(&delta);
                            on_chunk(delta);
                        }
                        AgentStreamChunk::FinalSummary(delta) => {
                            streamed_finish_summary.push_str(&delta);
                            on_chunk(delta);
                        }
                    };
                    run_control::run(stream_chat_with_network_retry(
                        &active_config,
                        &ChatRequest {
                            message: String::new(),
                            system_instruction: active_system_prompt.clone(),
                            chat_id: Some(chat_id.to_owned()),
                            image_data_uri: None,
                            audio_data_uri: None,
                            video_data_uri: None,
                            document_attachment: None,
                            workspace_path: None,
                            agent_id: None,
                            plan_mode: false,
                            pinned_mcp_server: None,
                            messages: Some(native_messages.clone()),
                            tools: Some(tool_catalog(
                                &active_config,
                                plan_mode,
                                &root,
                                allow_subagent_dispatch,
                            )),
                            temperature: active_config.temperature,
                            ..Default::default()
                        },
                        &thought_id,
                        thought_started,
                        fast_mode,
                        &mut progress,
                        false,
                        allow_final_stream,
                        &mut emit_stream_chunk,
                    ))
                    .await?
                }
            } else {
                {
                    let mut emit_stream_chunk = |chunk| match chunk {
                        AgentStreamChunk::DirectText(delta) => {
                            streamed_direct_text.push_str(&delta);
                            on_chunk(delta);
                        }
                        AgentStreamChunk::FinalSummary(delta) => {
                            streamed_finish_summary.push_str(&delta);
                            on_chunk(delta);
                        }
                    };
                    run_control::run(stream_chat_with_network_retry(
                        &active_config,
                        &ChatRequest {
                            message: observation.clone(),
                            system_instruction: active_system_prompt.clone(),
                            chat_id: Some(chat_id.to_owned()),
                            image_data_uri: pending_image.take(),
                            audio_data_uri: pending_audio.take(),
                            video_data_uri: pending_video.take(),
                            document_attachment: None,
                            workspace_path: None,
                            agent_id: None,
                            plan_mode: false,
                            pinned_mcp_server: None,
                            messages: None,
                            tools: None,
                            temperature: active_config.temperature,
                            ..Default::default()
                        },
                        &thought_id,
                        thought_started,
                        fast_mode,
                        &mut progress,
                        true,
                        allow_final_stream,
                        &mut emit_stream_chunk,
                    ))
                    .await?
                }
            };

            final_provider = response.provider.clone();
            final_model = response.model.clone();
            turn_total_tokens += response.total_tokens.unwrap_or(0) as u64;
            let usage_payload = mod_usage.record(&active_config, &response);
            let _ = crate::mods::event(&root, chat_id, "usage_update", usage_payload, None);
            turn_generated_tokens += response.output_tokens.unwrap_or(0) as u64;
            if let Some(input) = response.input_tokens {
                last_input_tokens = input as u64;
            }
            if fallback.is_some() {
                // Track the original provider that failed over (held in `response.fallback_provider`),
                // so UI and logs correctly show `<original> unavailable, fell back to <current>`.
                final_fallback = response.fallback_provider.clone();
                final_fallback_reason = response.fallback_reason.clone();
                if let Some(reason) = &response.fallback_reason {
                    progress(AgentProgress::Thought {
                        thought: format!(
                            "⚠️ Switched provider: {reason} — continuing with {}.",
                            response.provider
                        ),
                    });
                }
            }
            if let Some(calls) = &response.tool_calls {
                for call in calls {
                    if let Some(signature) = &call.thought_signature {
                        step_thought_signatures.insert(call.id.clone(), signature.clone());
                    }
                }
            }

            // A provider-limited design draft is not a completed mod. Recover
            // only after successful mod preparation, before publication, and
            // within a per-turn budget. All subsequent tools keep their normal
            // plan-mode, capability and approval checks.
            if tool_mode == ToolCallingMode::Native
                && !plan_mode
                && !active_config.disabled_tools.iter().any(|tool| tool == "mod_publish")
                && response.tool_calls.as_ref().is_none_or(|calls| calls.is_empty())
                && provider_truncated_response(response.stop_reason.as_deref())
                && mod_truncation_retries < 2
                && executed_tools.iter().any(|tool| tool.success && matches!(tool.action.as_str(), "mod_help" | "mod_validate"))
                && !executed_tools.iter().any(|tool| tool.success && tool.action == "mod_publish")
            {
                mod_truncation_retries += 1;
                let instruction = "Your previous response reached the provider output-length limit before completing the request. Use the already loaded Mint Mods guide and choose reasonable defaults. If the user requested a mod, send the source through mod_validate and then mod_publish using their tool arguments, with at most one short progress sentence. Follow the existing permissions and approvals. If the request cannot be completed, call finish with a concise explanation. Otherwise answer the original question concisely. Avoid repeating the design discussion.";
                native_messages.push(ChatMessage {
                    role: ChatRole::User,
                    content: vec![ContentBlock::Text { text: instruction.into() }],
                });
                trajectory.push(format_trajectory_step(step, "", "mod_authoring_recovery", instruction));
                progress(AgentProgress::Thought {
                    thought: "Mod preparation reached the provider output limit; continuing with the authoring tools.".into(),
                });
                continue 'steps;
            }

            // `decisions` is normally a single `(call_id, AgentDecision)`, matching the
            // original one-action-per-step design. Native tool-calling can return
            // several tool calls in one model turn, in which case they're executed
            // sequentially and all their results are fed back before the next call —
            // `finish` never appears alongside real tool calls (see below), so this
            // never conflicts with the early-return finish handling.
            let mut step_tool_results: Vec<ToolResultEntry> = Vec::new();
            let decisions: Vec<(String, AgentDecision)> = if tool_mode == ToolCallingMode::Native {
                match response.tool_calls.clone() {
                    Some(calls) if !calls.is_empty() => {
                        let (decisions, errors) = prepare_native_tool_calls(
                            calls, response.text.trim(), &mut invalid_tool_calls,
                        )?;
                        for ToolResultEntry{action,input,text:error,..} in &errors {
                            progress(AgentProgress::ToolStart { call_id: None, action: action.clone(), input: input.clone(), subagent: None });
                            progress(AgentProgress::ToolEnd { elapsed_ms: None, call_id: None, status: None, action: action.clone(), input: input.clone(), result: error.clone(), subagent: None });
                            trajectory.push(format_trajectory_step(step, "", action, error));
                            trajectory_run.record_step(StandardTrajectoryStep {
                                step,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                thought: String::new(),
                                action: action.clone(),
                                input: input.clone(),
                                observation: error.clone(),
                                status: "failed".to_string(),
                                duration_ms: thought_started.elapsed().as_millis() as u64,
                                tokens: StepTokens {
                                    input_tokens: last_input_tokens,
                                    generated_tokens: response.output_tokens.unwrap_or(0) as u64,
                                },
                            });
                        }
                        step_tool_results.extend(errors);
                        decisions
                    }
                    // No tool calls means the model answered directly — treat exactly
                    // like the JSON-prompt path's fallback for plain, non-JSON text
                    // (see `parse_decision_or_finish`): finish with that text as the
                    // summary. Unlike the JSON-prompt path, a truncated answer here
                    // passes through as perfectly valid plain text (there's no JSON
                    // structure to fail parsing and trigger a repair retry), so a
                    // response cut off by the provider's own output-token cap would
                    // otherwise print as if it were a complete answer — see
                    // `provider_truncated_response`.
                    _ => {
                        let mut summary = response.text.trim().to_string();
                        if provider_truncated_response(response.stop_reason.as_deref()) {
                            summary.push_str(
                                "\n\n[System note: this response was cut off by the model \
                                 provider's own output-length limit, not by Mint — it may be \
                                 incomplete. Ask to continue or retry to get the rest.]",
                            );
                        }
                        let thought = response.thought.clone().unwrap_or_default();
                        vec![(
                            format!("call_{step}_finish"),
                            AgentDecision {
                                thought,
                                action: "finish".to_string(),
                                input: AgentInput {
                                    summary,
                                    ..AgentInput::default()
                                },
                            },
                        )]
                    }
                }
            } else {
                let mut decision = match parse_decision_or_finish(&response.text) {
                    Ok(decision) => decision,
                    Err(_) => {
                        let (repaired, _) = run_control::run(send_chat_with_fallback(
                            &active_config,
                            &ChatRequest {
                                message: format!(
                                    "Your previous response was not valid Mint agent JSON.\n\
                                     Return exactly one corrected JSON object with an action and input. \
                                     Do not use markdown.\n\nPrevious response:\n{}",
                                    truncate(&response.text)
                                ),
                                system_instruction: active_system_prompt.clone(),
                                chat_id: Some(chat_id.to_owned()),
                                image_data_uri: None,
                                audio_data_uri: None,
                                video_data_uri: None,
                                document_attachment: None,
                                workspace_path: None,
                                agent_id: None,
                                plan_mode: false,
                                pinned_mcp_server: None,
                                messages: None,
                                tools: None,
                                temperature: active_config.temperature,
                                ..Default::default()
                            },
                        ))
                        .await?;
                        parse_decision_or_finish(&repaired.text).map_err(|e| {
                            OrchestrationError::Agent(format!(
                                "unable to repair invalid agent response: {}",
                                e
                            ))
                        })?
                    }
                };
                // If the model's inline JSON `thought` is empty but the API
                // sent separate thinking tokens (reasoning models), emit them
                // as `ExtendedThinking` so the UI can display them in a
                // dedicated collapsible block instead of the short-thought
                // timeline.  Still copy into `decision.thought` so the text
                // is available downstream, but the *progress event* uses the
                // correct variant.
                if decision.thought.trim().is_empty() {
                    if let Some(t) = &response.thought {
                        decision.thought = t.trim().to_string();
                    }
                }
                vec![(format!("call_{step}"), decision)]
            };

            let mut extended_emitted = false;
            if !fast_mode
                && let Some(t) = &response.thought
                && !t.trim().is_empty()
            {
                progress(AgentProgress::ExtendedThinking {
                    id: Some(thought_id.clone()),
                    thought: t.trim().to_owned(),
                    elapsed_ms: Some(thought_started.elapsed().as_millis() as u64),
                });
                extended_emitted = true;
            }

            // Screenshots (and any future binary/image tool result) are kept out of
            // `step_tool_results`'s plain-text `final_result` — that string is what
            // both the text `trajectory` and the JSON-prompt `observation` are built
            // from, and a multi-hundred-KB base64 PNG would blow straight through
            // `MAX_OBSERVATION_BYTES` and get silently chopped mid-base64. The full
            // data URI is stashed here by `call_id` instead, and re-attached as a
            // real `ContentBlock::Image` when building `native_messages` below, so
            // the model actually sees the pixels instead of a truncated text blob.
            let mut step_images: std::collections::HashMap<String, Vec<String>> =
                std::collections::HashMap::new();

            if decisions_are_parallel_read_only_batch(&decisions) {
                let completed = run_parallel_read_only_batch(
                    &decisions,
                    step,
                    &root,
                    config,
                    chat_id,
                    &mut approve,
                    &mut progress,
                    &mut action_counts,
                    &mut trajectory,
                    &mut step_images,
                    &hooks,
                    pinned_mcp_server,
                    fast_mode,
                )
                .await;
                invalid_tool_calls.observe_completed(&completed);
                step_tool_results.extend(completed);
            } else {
                for (call_id, decision) in decisions {
                    if run_control::cancelled() { turn.interrupt(); return Err(OrchestrationError::Agent("Agent turn canceled by user".into())); }
                    let d_thought = decision.thought.trim();
                    if !fast_mode && !d_thought.is_empty() {
                        let already_extended = extended_emitted
                            && response.thought.as_deref().map(str::trim) == Some(d_thought);
                        if !already_extended {
                            if is_internal_cot(d_thought) {
                                progress(AgentProgress::ExtendedThinking {
                                    id: Some(thought_id.clone()),
                                    thought: d_thought.to_owned(),
                                    elapsed_ms: Some(thought_started.elapsed().as_millis() as u64),
                                });
                            } else {
                                progress(AgentProgress::Thought {
                                    thought: d_thought.to_owned(),
                                });
                            }
                        }
                    }

                    if decision.action == "finish" {
                        let (inline_summary_thought, cleaned_summary) =
                            crate::chat::extract_think_tag(decision.input.summary.trim());
                        if let Some(th) = inline_summary_thought
                            && !fast_mode
                            && !extended_emitted
                        {
                            progress(AgentProgress::ExtendedThinking {
                                id: Some(thought_id.clone()),
                                thought: th,
                                elapsed_ms: Some(thought_started.elapsed().as_millis() as u64),
                            });
                        }
                        let mut summary = cleaned_summary.trim().to_owned();
                        if summary.is_empty() {
                            let err_msg = "Error: Your finish action summary was empty. \
                                       You MUST provide a final answer, explanation, or response to the user's query \
                                       in the 'summary' field of the 'finish' action input. Do not leave it empty.";
                            trajectory.push(format_trajectory_step(
                                step,
                                &decision.thought,
                                &decision.action,
                                err_msg,
                            ));
                            rebuild_observation(task, &root, &trajectory, &mut observation);
                            reject_native_finish(
                                tool_mode,
                                &mut native_messages,
                                &response.text,
                                err_msg,
                            );
                            continue 'steps;
                        }
                        let completion_status = match reliability::completion(&decision.input.completion_status, &decision.input.verification) {
                            Ok(status) => status,
                            Err(error) => {
                                trajectory.push(format_trajectory_step(step, &decision.thought, &decision.action, &error));
                                rebuild_observation(task, &root, &trajectory, &mut observation);
                                reject_native_finish(tool_mode, &mut native_messages, &response.text, &error);
                                continue 'steps;
                            }
                        };
                        if completion_status != "verified" {
                            let notice = format!("\n\nCompletion: {}. {}", completion_status, decision.input.verification.trim());
                            summary.push_str(&notice);
                        }
                        // Auto-append generated media (image/video) and model feedback to summary if LLM omitted it
                        let mut media_blocks = Vec::new();
                        for step in trajectory.iter() {
                            for line in step.lines() {
                                let trimmed = line.trim();
                                if trimmed.starts_with("<video")
                                    || trimmed.starts_with("✓ Video generated successfully")
                                {
                                    if !summary.contains(trimmed) {
                                        media_blocks.push(trimmed.to_string());
                                    }
                                }
                            }
                        }
                        if !media_blocks.is_empty() {
                            summary.push_str("\n\n");
                            summary.push_str(&media_blocks.join("\n\n"));
                        }

                        let verification =
                            meaningful_verification(&decision.input.verification).to_owned();

                        let remainder = unstreamed_summary_remainder(
                            &summary,
                            &streamed_finish_summary,
                            &streamed_direct_text,
                        );
                        if !remainder.is_empty() {
                            on_chunk(remainder.to_owned());
                        }

                        if config.auto_skill_writing && allow_subagent_dispatch && !run_control::cancelled() && looks_skill_worthy(step, &action_counts) {
                            match auto_write_skill(config, task, &summary, &root, &skills, &mut approve).await {
                                Ok(Some(message)) => {
                                    on_chunk(format!("\n\n{message}"));
                                    summary.push_str(&format!("\n\n{message}"));
                                }
                                Ok(None) => {}
                                Err(error) => {
                                    let message = format!("Skill proposal was not saved: {error}");
                                    on_chunk(format!("\n\n{message}"));
                                    summary.push_str(&format!("\n\n{message}"));
                                }
                            }
                        }
                        memory.save_workspace_session(
                            &root.to_string_lossy(),
                            &summary,
                            &verification,
                        )?;
                        spawn_auto_memory_update(
                            config.clone(),
                            task.to_string(),
                            summary.clone(),
                            Some(root.to_string_lossy().into_owned()),
                            chat_id.to_string(),
                        );
                        crate::linked_folders::spawn_linked_folder_note(
                            config.clone(),
                            task.to_string(),
                            summary.clone(),
                            turn.id,
                        );
                        trajectory_run.finish(
                            completion_status.to_ascii_uppercase(),
                            turn_total_tokens,
                            started_at.elapsed().as_secs_f64(),
                            files_created.iter().cloned().collect(),
                            files_modified.iter().cloned().collect(),
                        );
                        let run_summary = RunTelemetrySummary {
                            run_id: run_id.clone(),
                            task: task.to_string(),
                            outcome: completion_status.to_ascii_uppercase(),
                            total_tokens: turn_total_tokens as usize,
                            tool_calls_count: executed_tools.len(),
                            files_changed: files_created
                                .iter()
                                .chain(files_modified.iter())
                                .cloned()
                                .collect(),
                            files_created: files_created.into_iter().collect(),
                            duration_secs: started_at.elapsed().as_secs_f64(),
                            tool_timeline: executed_tools.clone(),
                            retries_count: executed_tools.iter().filter(|t| t.retried).count(),
                            trajectory: Some(trajectory_run),
                        };
                        let _ = execute_mod_requests(&root, chat_id, config, plan_mode, &mut approve, &mut progress, None).await;
                        progress(AgentProgress::RunCompleted {
                            summary: run_summary,
                        });

                        turn.complete_with_summary(
                            &summary,
                            &final_provider,
                            &final_model,
                            final_fallback.as_deref(),
                        )?;

                        return Ok(AgentResult {
                            provider: final_provider,
                            model: final_model,
                            summary,
                            verification,
                            completion_status,
                            fallback: final_fallback,
                            fallback_reason: final_fallback_reason,
                            total_tokens: turn_total_tokens,
                            input_tokens: last_input_tokens,
                            generated_tokens: turn_generated_tokens,
                        });
                    }

                    if decision.action == "mcp_tool" && mcp_review.blocks_repeat(&decision.input.server,&decision.input.tool,&decision.input.arguments) {
                        let message="Error: Automatic repeat blocked: this Device command's completion is unconfirmed. Read its status or wait for a new user instruction; do not send the command again this turn.".to_string();
                        trajectory.push(format_trajectory_step(step,&decision.thought,&decision.action,&message));
                        trajectory_run.record_step(StandardTrajectoryStep {
                            step,
                            timestamp: chrono::Utc::now().to_rfc3339(),
                            thought: decision.thought.clone(),
                            action: decision.action.clone(),
                            input: tool_progress_input(&root,&decision),
                            observation: message.clone(),
                            status: "intercepted".to_string(),
                            duration_ms: thought_started.elapsed().as_millis() as u64,
                            tokens: StepTokens {
                                input_tokens: last_input_tokens,
                                generated_tokens: response.output_tokens.unwrap_or(0) as u64,
                            },
                        });
                        step_tool_results.push((call_id.clone(),decision.action.clone(),tool_progress_input(&root,&decision),message).into());
                        continue;
                    }

                    if decision.action == "mcp_tool" && !plan_mode
                        && pinned_mcp_server.is_none_or(|p|p==decision.input.server)
                        && let Some(preparation) = tools::plugins_mcp::prepare(
                            &decision.input,config,chat_id,&mut mcp_review,&mut approve,&mut progress,
                        ).await? {
                            if preparation.status == crate::mcp_result::ToolStatus::Cancelled || run_control::cancelled() {
                                progress(AgentProgress::Thought{thought:preparation.text.clone()});
                                turn.interrupt();return Err(OrchestrationError::Agent(preparation.text));
                            }
                            trajectory.push(format_trajectory_step(step,&decision.thought,&decision.action,&preparation.text));
                            trajectory_run.record_step(StandardTrajectoryStep {
                                step,
                                timestamp: chrono::Utc::now().to_rfc3339(),
                                thought: decision.thought.clone(),
                                action: decision.action.clone(),
                                input: tool_progress_input(&root,&decision),
                                observation: preparation.text.clone(),
                                status: "intercepted".to_string(),
                                duration_ms: thought_started.elapsed().as_millis() as u64,
                                tokens: StepTokens {
                                    input_tokens: last_input_tokens,
                                    generated_tokens: response.output_tokens.unwrap_or(0) as u64,
                                },
                            });
                            step_tool_results.push(ToolResultEntry{call_id:call_id.clone(),action:decision.action.clone(),input:tool_progress_input(&root,&decision),text:preparation.text,status:preparation.status});
                            continue;
                    }

                    let action_key = action_fingerprint(&decision);
                    let action_count = {
                        let count = action_counts.entry(action_key).or_insert(0);
                        *count += 1;
                        *count
                    };

                    // Set only on the real `execute_tool` path below (PreHookOutcome::Allowed);
                    // stays false for plan-mode/hook blocks, since
                    // those don't actually run anything and shouldn't count toward verification.
                    let mut action_succeeded = false;
                    let mut outcome_status = None;
                    // Provider tool IDs may restart at call_0 on the next model step.
                    // Activity and artifact IDs must identify this invocation across the chat.
                    let activity_call_id = uuid::Uuid::new_v4().to_string();
                    let target_file_existed = match decision.action.as_str() {
                        "write_file" => {
                            !decision.input.path.is_empty()
                                && root.join(&decision.input.path).exists()
                        }
                        "note_write" => {
                            !decision.input.name.is_empty()
                                && root
                                    .join(format!(".config/mint/notes/{}", decision.input.name))
                                    .exists()
                        }
                        "apply_patch" => {
                            if let Some(patch) = &decision.input.patch {
                                root.join(&patch.path).exists()
                            } else {
                                true
                            }
                        }
                        _ => true,
                    };
                    let result = if decision.action == "enter_plan_mode" {
                        let reason = decision.input.reason.trim().to_owned();
                        match approve(&AgentApproval::EnterPlanMode {
                            reason: reason.clone(),
                        }) {
                            Ok(ApprovalOutcome::Approved) => {
                                plan_mode = true;
                                system_prompt = construct_system_prompt(
                                    config,
                                    &memory,
                                    chat_id,
                                    &root,
                                    &resolved_task,
                                    plan_mode,
                                    system_prompt_native,
                                    user_name,
                                    pinned_mcp_server,
                                );
                                progress(AgentProgress::Thought {
                                    thought: format!(
                                        "[Plan] Switched to plan mode before acting: {}",
                                        if reason.is_empty() {
                                            "this task looked risky or complex enough to investigate first."
                                        } else {
                                            reason.as_str()
                                        }
                                    ),
                                });
                                "Plan mode is now ON. Investigate read-only (list_files, read_file, search_code, etc.) and once you have a clear implementation plan, call exit_plan_mode with the full plan; the user will approve or reject it.".to_string()
                            }
                            Ok(ApprovalOutcome::Denied) => {
                                "The user declined to enter plan mode. Plan mode stays OFF — proceed with the task directly. If new information later makes this feel too risky to continue without a plan, you may call enter_plan_mode again.".to_string()
                            }
                            Ok(ApprovalOutcome::Intercepted(feedback)) => {
                                format!(
                                    "The user did not approve entering plan mode and left this feedback: {}\n\nPlan mode stays OFF. Take this feedback into account and proceed with the task.",
                                    feedback
                                )
                            }
                            Err(error) => format!("Error requesting plan-mode approval: {}", error),
                        }
                    } else if decision.action == "exit_plan_mode" {
                        let plan_text = decision.input.plan.trim().to_owned();
                        match approve(&AgentApproval::ExitPlanMode {
                    plan: plan_text.clone(),
                }) {
                    Ok(ApprovalOutcome::Approved) => {
                        plan_mode = false;
                        system_prompt = construct_system_prompt(
                            config,
                            &memory,
                            chat_id,
                            &root,
                            &resolved_task,
                            plan_mode,
                            system_prompt_native,
                            user_name,
                            pinned_mcp_server,
                        );
                        "Plan approved by the user. Plan mode is now OFF — write_file, apply_patch, run_shell, and other previously blocked tools are now available. Proceed with implementing the plan.".to_string()
                    }
                    Ok(ApprovalOutcome::Denied) => {
                        "The user rejected this plan. Plan mode is still ON. Continue investigating or revise the plan, then call exit_plan_mode again when ready.".to_string()
                    }
                    Ok(ApprovalOutcome::Intercepted(feedback)) => {
                        format!(
                            "The user did not approve the plan yet and left this feedback: {}\n\nPlan mode is still ON. Revise the plan accordingly and call exit_plan_mode again when ready.",
                            feedback
                        )
                    }
                    Err(error) => format!("Error requesting plan approval: {}", error),
                }
                    } else if plan_mode && !plan_mode_allows(&decision.action, &decision.input) {
                        format!(
                            "Blocked: '{}' is not available in plan mode (read-only investigation only). Call exit_plan_mode with your proposed plan once you are ready to implement it; the user will approve or reject it.",
                            decision.action
                        )
                    } else if pinned_mcp_server.is_some_and(|p| {
                        (decision.action == "mcp_tool" || decision.action == "mcp_list_tools")
                            && decision.input.server != p
                    }) {
                        let p = pinned_mcp_server.unwrap();
                        format!(
                            "Blocked: this turn is pinned to the \"{p}\" MCP server only (selected via @{p} in the composer). Retry with \"server\":\"{p}\", or use a different (non-MCP) tool."
                        )
                    } else {
                        let input_val =
                            tool_progress_input(&root, &decision);
                        let loop_decision = repetition_guard.check(&decision.action, &input_val);
                        progress(AgentProgress::ToolStart { call_id: Some(activity_call_id.clone()),
                            action: decision.action.clone(),
                            input: input_val.clone(),
                            subagent: None,
                        });

                        match loop_decision {
                            loop_guard::LoopGuardDecision::InterceptStuckLoop { ref action, ref reason, ref instruction } => {
                                outcome_status = Some(crate::integrations::mcp_result::ToolStatus::Failed);
                                action_succeeded = false;
                                progress(AgentProgress::Thought {
                                    thought: format!("[Stuck-Loop Guard] Intercepted repeated call to '{action}': {reason}"),
                                });
                                instruction.clone()
                            }
                            _ => {
                                match crate::hooks::run_pre_tool_hooks(
                                    &hooks,
                                    &decision.action,
                                    &input_val,
                                    &root,
                                ) {
                                    crate::hooks::PreHookOutcome::Blocked(reason) => {
                                        format!(
                                            "Blocked by hook: {}\n\n[System Tip: A configured PreToolUse hook rejected this action. Adjust your approach or ask the user for guidance.]",
                                            reason
                                        )
                                    }
                                    crate::hooks::PreHookOutcome::Allowed => {
                                        if matches!(
                                            decision.action.as_str(),
                                            "write_file" | "apply_patch" | "note_write"
                                        ) {
                                            let target_path = if !decision.input.path.is_empty() {
                                                Some(decision.input.path.as_str())
                                            } else {
                                                None
                                            };
                                            let _ = crate::git::create_checkpoint(
                                                &root,
                                                chat_id,
                                                step,
                                                &decision.action,
                                                target_path,
                                                &format!("Before Step {step}: {}", decision.action),
                                            );
                                        }

                                        let (mut tool_result, success) = match execute_tool_outcome_reviewed(
                                            &root,
                                            config,
                                            &decision,
                                            chat_id,
                                            &mut approve,
                                            &mut progress,
                                            &activity_call_id,
                                            Some(&mcp_review),
                                        )
                                        .await
                                        {
                                            Ok(outcome) => {
                                                if decision.action=="mcp_tool" {mcp_review.record_result(&decision.input.server,&decision.input.tool,&decision.input.arguments,&outcome);}
                                                outcome_status = Some(outcome.status);
                                                if !outcome.images.is_empty() { step_images.insert(call_id.clone(), outcome.images); }
                                                let success = outcome.status == crate::integrations::mcp_result::ToolStatus::Success;
                                                (outcome.text, success)
                                            },
                                            Err(error) => (format!("Error: {}", error), false),
                                        };
                                        action_succeeded = success;
                                        if success {
                                            invalid_tool_calls.succeeded(&decision.action);
                                        }
                                        if let loop_guard::LoopGuardDecision::WarnConsecutiveRepeat { ref notice, .. } = loop_decision {
                                            tool_result.push_str(&format!("\n\n{notice}"));
                                        }
                                        if step >= warning_threshold {
                                            let remaining = max_steps.saturating_sub(step);
                                            if remaining == 0 {
                                                tool_result.push_str("\n\n[CRITICAL STEP LIMIT REACHED - FINAL STEP]: Step budget exhausted. Do not call more tools. Call 'finish' now with your summary.");
                                            } else {
                                                tool_result.push_str(&format!(
                                                    "\n\n[Step Budget Warning]: Step {}/{} ({} step{} remaining). Please begin consolidating your findings and call 'finish'.",
                                                    step, max_steps, remaining, if remaining == 1 { "" } else { "s" }
                                                ));
                                            }
                                        }
                                        let hook_messages = crate::hooks::run_post_tool_hooks(
                                            &hooks,
                                            &decision.action,
                                            &input_val,
                                            &tool_result,
                                            success,
                                            &root,
                                        );
                                        if hook_messages.is_empty() {
                                            tool_result
                                        } else {
                                            format!(
                                                "{}\n\n{}",
                                                tool_result,
                                                hook_messages
                                                    .iter()
                                                    .map(|message| format!("[Hook] {}", message))
                                                    .collect::<Vec<_>>()
                                                    .join("\n")
                                            )
                                        }
                                    }
                                }
                            }
                        }
                    };

                    if outcome_status.is_none() && matches!(decision.action.as_str(), "mcp_tool" | "mcp_list_tools") {
                        outcome_status = Some(if action_succeeded {
                            crate::integrations::mcp_result::ToolStatus::Success
                        } else {
                            crate::integrations::mcp_result::ToolStatus::Failed
                        });
                    }
                    if run_control::cancelled() {outcome_status = Some(crate::mcp_result::ToolStatus::Cancelled);}
                    progress(AgentProgress::ToolEnd { elapsed_ms: None, call_id: Some(activity_call_id), status: outcome_status,
                        action: decision.action.clone(),
                        input: tool_progress_input(&root, &decision),
                        result: result.clone(),
                        subagent: None,
                    });

                    let success = outcome_status.map(|s| s == crate::mcp_result::ToolStatus::Success).unwrap_or(action_succeeded);
                    let target = if !decision.input.path.is_empty() {
                        decision.input.path.clone()
                    } else if !decision.input.command.is_empty() {
                        decision.input.command.clone()
                    } else if !decision.input.query.is_empty() {
                        decision.input.query.clone()
                    } else if !decision.input.symbol.is_empty() {
                        decision.input.symbol.clone()
                    } else {
                        String::new()
                    };
                    let retried = success && (action_count > 1 || last_verify_failed == Some(true));
                    executed_tools.push(ToolExecutionRecord {
                        step,
                        action: decision.action.clone(),
                        target,
                        success,
                        retried,
                    });

                    if outcome_status == Some(crate::integrations::mcp_result::ToolStatus::Cancelled) {
                        turn.interrupt();
                        return Err(OrchestrationError::Agent(result));
                    }

                    if action_succeeded {
                        match decision.action.as_str() {
                            "apply_patch" | "write_file" | "note_write" => {
                                repetition_guard.notify_state_change();
                                let path = if let Some(patch) = &decision.input.patch {
                                    patch.path.to_string_lossy().into_owned()
                                } else if !decision.input.path.is_empty() {
                                    decision.input.path.clone()
                                } else if !decision.input.name.is_empty() {
                                    format!(".config/mint/notes/{}", decision.input.name)
                                } else {
                                    String::new()
                                };
                                if !path.is_empty() {
                                    if !files_created.contains(&path)
                                        && (target_file_existed || files_modified.contains(&path))
                                    {
                                        files_modified.insert(path);
                                    } else {
                                        files_created.insert(path);
                                    }
                                }
                            }
                            // Counts even if the commands it ran failed — an attempted check
                            // still counts as verification having been attempted; whether it
                            // actually passed is tracked separately in `last_verify_failed`
                            // and enforced by `unacknowledged_verify_failure` at finish time.
                            "verify" | "run_tests" | "run_typecheck" | "run_linter" => {
                                last_verify_failed = Some(!success);
                            }
                            _ => {}
                        }
                    }

                    let mut final_result = if result.starts_with("data:image/")
                        && matches!(
                            decision.action.as_str(),
                            "video_filmstrip"
                                | "video_waveform"
                                | "view_image"
                        ) {
                        step_images.insert(call_id.clone(), vec![result.clone()]);
                        match decision.action.as_str() {
                            "video_filmstrip" => "[Filmstrip generated — see attached image: \
                                sampled frames across the video timeline]"
                                .to_string(),
                            "video_waveform" => "[Waveform generated — see attached image: \
                                audio amplitude over time]"
                                .to_string(),
                            _ => "[Image loaded — see attached image]".to_string(),
                        }
                    } else {
                        model_tool_result(Some(&root), &decision.action, &result)
                    };
                    if matches!(
                        decision.action.as_str(),
                        "run_shell" | "verify" | "run_tests" | "run_typecheck" | "run_linter"
                    ) {
                        if shell_result_failed(&result) {
                            if let Some(diag) = diagnostics::diagnose_command_failure(&decision.input.command, &result, None) {
                                final_result.push_str(&format!("\n\n{diag}"));
                            } else {
                                final_result.push_str(
                                    "\n\n[System Tip: The command failed with a non-zero exit code. \
                                     Analyze the stdout/stderr above to locate the error, read the offending files, \
                                     apply corrected edits (using apply_patch), and run the verification command again. \
                                     If checks cannot pass, finish with completionStatus=blocked/failed/unverified and explain the actual result; never claim they passed.]"
                                );
                            }
                        }
                    }
                    if decision.action == "apply_patch" || decision.action == "write_file" {
                        final_result.push_str(
                    "\n\n[System Tip: The file edit was approved and applied successfully. \
                     Before finishing, verify this change with the verify tool (build/test/lint, \
                     whatever fits this project). If no check applies, finish with completionStatus=unverified \
                     and explain why in verification. A written explanation is not a passed check. Do not broaden the scope, do not make \
                     additional unrelated edits, and do not reread the same file unless you need one \
                     concise verification read.]",
                );
                    }
                    if is_observation_action(&decision.action, &decision.input.command) {
                        let mut observed = result.clone();
                        if let Some(images) = step_images.get(&call_id) {
                            for image in images {
                                observed.push_str(image);
                            }
                        }
                        if let Some(notice) = repetition_guard.observe_result(&decision.action, &observed) {
                            final_result.push_str("\n\n");
                            final_result.push_str(&notice);
                        }
                    }
                    if action_count >= 3 {
                        final_result.push_str(
                    "\n\n[System Tip: You repeated the same tool action three or more times. \
                     Stop repeating it. If you already have enough information or the requested edit is done, \
                     use the finish action now. Otherwise choose a different necessary action.]",
                );
                    }

                    trajectory.push(format_trajectory_step(
                        step,
                        &decision.thought,
                        &decision.action,
                        &final_result,
                    ));

                    let step_input_val = serde_json::to_value(&decision.input).unwrap_or(Value::Null);
                    let step_status_str = match outcome_status {
                        Some(crate::integrations::mcp_result::ToolStatus::Success) => "success",
                        Some(crate::integrations::mcp_result::ToolStatus::Failed) => "failed",
                        Some(crate::integrations::mcp_result::ToolStatus::TimedOut) => "timed_out",
                        Some(crate::integrations::mcp_result::ToolStatus::Cancelled) => "cancelled",
                        Some(crate::integrations::mcp_result::ToolStatus::Skipped) => "skipped",
                        _ => if success { "success" } else { "failed" },
                    };
                    trajectory_run.record_step(StandardTrajectoryStep {
                        step,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        thought: decision.thought.clone(),
                        action: decision.action.clone(),
                        input: step_input_val,
                        observation: final_result.clone(),
                        status: step_status_str.to_string(),
                        duration_ms: thought_started.elapsed().as_millis() as u64,
                        tokens: StepTokens {
                            input_tokens: last_input_tokens,
                            generated_tokens: response.output_tokens.unwrap_or(0) as u64,
                        },
                    });

                    step_tool_results.push(ToolResultEntry {
                        call_id, action:decision.action.clone(), input:serde_json::to_value(&decision.input).unwrap_or(Value::Null),
                        text:final_result, status:outcome_status.unwrap_or(if success {crate::mcp_result::ToolStatus::Success}else{crate::mcp_result::ToolStatus::Failed}),
                    });
                } // end `for (call_id, decision) in decisions`
            } // end `else` (sequential path)

            if run_control::cancelled() { turn.interrupt(); return Err(OrchestrationError::Agent("Agent turn canceled by user".into())); }

            // All interfaces and both native/JSON tool modes share this guard.
            // Inspect failures, not action counts: legitimate repeated reads
            // and successful edits must not be stopped as loops.
            match repeated_failures.observe(step, &mut step_tool_results) {
                Ok(warnings) => {
                    for warning in warnings {
                        progress(AgentProgress::Thought { thought: warning.clone() });
                        trajectory.push(format!("Step {step} loop warning: {warning}"));
                    }
                }
                Err(error) => {
                    progress(AgentProgress::Thought { thought: error.to_string() });
                    return Err(error);
                }
            }

            if tool_mode == ToolCallingMode::JsonPrompt && !step_images.is_empty() {
                pending_image = Some(step_images.values().flatten().cloned().collect::<Vec<_>>().join(" "));
            }
            if tool_mode == ToolCallingMode::Native {
                append_native_tool_results(
                    &mut native_messages,
                    response.text.trim(),
                    &step_tool_results,
                    &step_thought_signatures,
                    &step_images,
                );
                prune_historic_images_from_native_messages(
                    &mut native_messages,
                    MAX_RETAINED_IMAGE_STEPS,
                );
                let modified = extract_modified_paths(&step_tool_results);
                if !modified.is_empty() {
                    supersede_stale_file_reads(&mut native_messages, &modified, step);
                }
                prune_historic_tool_results_from_native_messages(
                    &mut native_messages,
                    3,
                    7,
                );

                if let Some(total_tokens) = response.total_tokens {
                    let window = active_config.context_window_tokens();
                    last_context_pct = Some(
                        ((total_tokens as f64 / window as f64) * 100.0).clamp(0.0, 255.0) as u8,
                    );
                    let compaction_needed = (total_tokens as f64) >= (window as f64) * COMPACTION_TRIGGER_RATIO
                        || (step >= ADAPTIVE_COMPACTION_MIN_STEPS && total_tokens >= ADAPTIVE_COMPACTION_TOKEN_CEILING);
                    if compaction_needed {
                        let eligible = native_messages.len().saturating_sub(history_prefix_len + 1) / 2 > COMPACTION_KEEP_RECENT_STEPS;
                        if let Ok(Some((compacted, _))) = run_control::run(report_context_compaction(
                            eligible,
                            total_tokens,
                            window,
                            compact_native_messages(&active_config, &native_messages, history_prefix_len),
                            &mut progress,
                        )).await {
                            native_messages = compacted;
                            let payload = mod_usage.compacted();
                            let _ = crate::mods::event(&root, chat_id, "context_compacted", payload, None);
                            // Wait for actual usage from the next request, rather than
                            // displaying the pre-compaction percentage as current.
                            last_context_pct = None;
                        }
                    }
                }
            }

            rebuild_observation(task, &root, &trajectory, &mut observation);
        }

        progress(AgentProgress::Thought {
            thought: format!(
                "[Step Limit Reached] Reached maximum allowed steps ({max_steps}). Synthesizing final findings for user..."
            ),
        });

        let (wrapup_config, _, _, _) = resolve_agent_config(config, agent_id, &trajectory);
        let mut final_streamed_summary = String::new();
        let mut emit_wrapup_chunk = |chunk| match chunk {
            AgentStreamChunk::DirectText(delta) | AgentStreamChunk::FinalSummary(delta) => {
                final_streamed_summary.push_str(&delta);
                on_chunk(delta);
            }
        };

        let synthesis_instruction = format!(
            "You are an AI assistant completing a multi-step task. The step limit of {max_steps} steps has been reached. \
             Synthesize all findings, observations, errors encountered, and links discovered so far. \
             Provide a comprehensive, direct, and well-structured final answer to the user in their requested language."
        );

        let synthesis_succeeded = if wrapup_config.tool_calling_mode() == ToolCallingMode::Native {
            let mut final_native = native_messages.clone();
            final_native.push(ChatMessage {
                role: ChatRole::User,
                content: vec![ContentBlock::Text {
                    text: format!(
                        "Maximum step limit of {max_steps} reached. Do not call any tools. Please output your final answer and summary of findings now."
                    ),
                }],
            });
            let wrapup_req = ChatRequest {
                message: String::new(),
                system_instruction: synthesis_instruction,
                chat_id: Some(chat_id.to_owned()),
                image_data_uri: None,
                audio_data_uri: None,
                video_data_uri: None,
                document_attachment: None,
                workspace_path: None,
                agent_id: None,
                plan_mode: false,
                pinned_mcp_server: None,
                messages: Some(final_native),
                tools: None,
                temperature: wrapup_config.temperature,
                ..Default::default()
            };
            run_control::run(stream_chat_with_network_retry(
                &wrapup_config,
                &wrapup_req,
                &format!("step-limit-wrapup-{chat_id}"),
                Instant::now(),
                fast_mode,
                &mut progress,
                false,
                true,
                &mut emit_wrapup_chunk,
            ))
            .await
            .is_ok()
        } else {
            let wrapup_req = ChatRequest {
                message: format!(
                    "Observation: Maximum step limit of {max_steps} reached. Please synthesize your findings and output your final answer now.\nPrevious observation:\n{observation}"
                ),
                system_instruction: synthesis_instruction,
                chat_id: Some(chat_id.to_owned()),
                image_data_uri: None,
                audio_data_uri: None,
                video_data_uri: None,
                document_attachment: None,
                workspace_path: None,
                agent_id: None,
                plan_mode: false,
                pinned_mcp_server: None,
                messages: None,
                tools: None,
                temperature: wrapup_config.temperature,
                ..Default::default()
            };
            run_control::run(stream_chat_with_network_retry(
                &wrapup_config,
                &wrapup_req,
                &format!("step-limit-wrapup-{chat_id}"),
                Instant::now(),
                fast_mode,
                &mut progress,
                false,
                true,
                &mut emit_wrapup_chunk,
            ))
            .await
            .is_ok()
        };

        let summary = if synthesis_succeeded && !final_streamed_summary.trim().is_empty() {
            final_streamed_summary
        } else {
            format!(
                "Task concluded after reaching the step limit ({} steps). Recent activity:\n{}",
                max_steps,
                trajectory.iter().rev().take(5).cloned().collect::<Vec<_>>().join("\n")
            )
        };

        trajectory_run.finish(
            "COMPLETED",
            turn_total_tokens,
            started_at.elapsed().as_secs_f64(),
            files_created.iter().cloned().collect(),
            files_modified.iter().cloned().collect(),
        );
        let run_summary = RunTelemetrySummary {
            run_id: run_id.clone(),
            task: task.to_string(),
            outcome: "COMPLETED".to_string(),
            total_tokens: turn_total_tokens as usize,
            tool_calls_count: executed_tools.len(),
            files_changed: files_created
                .iter()
                .chain(files_modified.iter())
                .cloned()
                .collect(),
            files_created: files_created.into_iter().collect(),
            duration_secs: started_at.elapsed().as_secs_f64(),
            tool_timeline: executed_tools.clone(),
            retries_count: executed_tools.iter().filter(|t| t.retried).count(),
            trajectory: Some(trajectory_run),
        };
        progress(AgentProgress::RunCompleted {
            summary: run_summary,
        });

        turn.complete_with_summary(
            &summary,
            &final_provider,
            &final_model,
            final_fallback.as_deref(),
        )?;

        return Ok(AgentResult {
            provider: final_provider,
            model: final_model,
            summary,
            verification: String::new(),
            completion_status: "step_limit_reached".to_string(),
            fallback: final_fallback,
            fallback_reason: final_fallback_reason,
            total_tokens: turn_total_tokens,
            input_tokens: last_input_tokens,
            generated_tokens: turn_generated_tokens,
        });
        })).await;
            if agent_result.is_err() && !turn.finished {
                if run_control::cancelled() {
                    turn.interrupt();
                } else {
                    turn.fail();
                }
            }
            let mut mod_turn_end = mod_usage.finish();
            mod_turn_end["success"] = serde_json::json!(agent_result.is_ok());
            mod_turn_end["cancelled"] = serde_json::json!(run_control::cancelled());
            let _ = crate::mods::event(&root, chat_id, "turn_end", mod_turn_end, None);
            if run_control::cancelled() {
                crate::mods::close_chat(&root, chat_id);
            }
            agent_result
        },
    ))
}

/// Whether `action` may run while plan mode is active. `run_shell` gets a
/// special case: only commands the safety classifier already tags as
/// read-only are allowed, everything else requires exiting plan mode first.
/// `finish` is always allowed regardless of plan mode. The rest of the
/// allowlist is shared with the system-prompt builder and the native
/// tool-calling catalog via `crate::prompts::agent::PLAN_MODE_ALLOWED_ACTIONS`
/// so the three can never drift on which actions are plan-mode-safe.
/// Whether a `ChatResponse::stop_reason` indicates the provider cut the
/// response off at its own output-token cap rather than the model choosing
/// to stop — `"length"` (OpenAI-compatible: openai/deepseek/openrouter/
/// local_openai), `"max_tokens"` (Anthropic), `"MAX_TOKENS"` (Gemini).
/// Providers that don't report a stop reason at all (huggingface, ollama)
/// can't be checked this way and are treated as not truncated.
fn provider_truncated_response(stop_reason: Option<&str>) -> bool {
    matches!(
        stop_reason,
        Some("length") | Some("max_tokens") | Some("MAX_TOKENS")
    )
}

pub(super) fn plan_mode_allows(action: &str, input: &AgentInput) -> bool {
    if action == "run_shell" {
        return classify_shell_command(&input.command).mode.as_str() == "readOnly";
    }
    action == "finish" || crate::prompts::agent::PLAN_MODE_ALLOWED_ACTIONS.contains(&action)
}

/// Runs a swarm batch of sub-agents to completion and returns the aggregated dossier.
pub(super) async fn dispatch_swarm(
    root: &Path,
    config: &MintConfig,
    parent_chat_id: &str,
    tasks: Vec<crate::swarm::SwarmTask>,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<String, OrchestrationError> {
    let (is_plan_mode, _) = crate::mods::turn_policy(root, parent_chat_id);
    let effective_cap = crate::swarm::effective_max_parallel(Some(config.swarm_max_parallel));
    let profiles = |name: &str| crate::subagents::find_subagent(name, Some(root));
    let ctx = crate::swarm::BatchCtx {
        parent_chat_id,
        max_parallel: effective_cap,
        profiles: &profiles,
        plan_mode: is_plan_mode,
    };

    let approve_mutex = std::sync::Mutex::new(approve_cb);
    let progress_mutex = std::sync::Mutex::new(progress);

    let runner = |run: crate::swarm::AgentRun, emitter: crate::swarm::Emitter| {
        let approve_mutex = &approve_mutex;
        Box::pin(async move {
            let mut sub_config = config.clone();
            if let Some(provider) = &run.provider {
                sub_config.ai_provider = provider.clone();
            }
            if let Some(model) = &run.model {
                match sub_config.ai_provider.as_str() {
                    "anthropic" => sub_config.anthropic_model = model.clone(),
                    "openai" => sub_config.openai_model = model.clone(),
                    "openrouter" => sub_config.openrouter_model = model.clone(),
                    "deepseek" => sub_config.deepseek_model = model.clone(),
                    "huggingface" => sub_config.hf_model = model.clone(),
                    "local_openai" => sub_config.local_model_name = model.clone(),
                    "ollama" => sub_config.ollama_model = model.clone(),
                    "gemini" => sub_config.gemini_model = model.clone(),
                    _ => {}
                }
            }

            let allows_run_shell = run.permissions.allows("run_shell");
            let backend = run.sandbox.as_deref().unwrap_or(&config.sandbox_backend);
            let docker_started = if backend == "docker" && allows_run_shell {
                match crate::docker_sandbox::start_session(&run.chat_id, root, &sub_config) {
                    Ok(()) => true,
                    Err(error) if config.sandbox_mode.trim().eq_ignore_ascii_case("enforce") => {
                        return Err(format!(
                            "agent '{}' requires docker sandbox (sandboxMode=enforce) but failed: {error}",
                            run.label
                        ));
                    }
                    Err(_) => false,
                }
            } else {
                false
            };

            let subagent_label = run.label.clone();
            let sub_task = format!(
                "{}\n\nTask from parent agent: {}",
                run.system_prompt, run.instruction
            );
            let run_chat_id = run.chat_id.clone();
            let permissions = run.permissions.clone();

            let steps_counter = std::sync::atomic::AtomicU32::new(1);
            let mut nested_progress = |event: AgentProgress| {
                if matches!(event, AgentProgress::ToolStart { .. }) {
                    steps_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                }
                let tagged = match event {
                    AgentProgress::ToolStart {
                        action,
                        input,
                        call_id,
                        ..
                    } => AgentProgress::ToolStart {
                        call_id,
                        action,
                        input,
                        subagent: Some(subagent_label.clone()),
                    },
                    AgentProgress::ToolEnd {
                        elapsed_ms,
                        action,
                        input,
                        result,
                        call_id,
                        status,
                        ..
                    } => AgentProgress::ToolEnd {
                        elapsed_ms,
                        call_id,
                        status,
                        action,
                        input,
                        result,
                        subagent: Some(subagent_label.clone()),
                    },
                    AgentProgress::ContextCompaction {
                        status, message, ..
                    } => AgentProgress::ContextCompaction {
                        status,
                        message,
                        subagent: Some(subagent_label.clone()),
                    },
                    other => other,
                };
                emitter.progress(tagged);
            };

            let mut nested_approve = |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                let mut guard = approve_mutex.lock().unwrap();
                (*guard)(approval)
            };

            let loop_fut = Box::pin(orchestrate_agent_loop(
                &sub_config,
                &sub_task,
                root,
                None,
                None,
                None,
                Some(&run_chat_id),
                None,
                None,
                None,
                true,
                false,
                &mut nested_approve,
                &mut nested_progress,
                |_| {},
            ));

            let res = crate::swarm::scope_permissions(Some(permissions), loop_fut).await;

            if docker_started {
                crate::docker_sandbox::stop_session(&run_chat_id);
            }

            match res {
                Ok(agent_result) => Ok(crate::swarm::RunnerOutput {
                    text: agent_result.summary,
                    steps: steps_counter.load(std::sync::atomic::Ordering::Relaxed),
                }),
                Err(err) => Err(err.to_string()),
            }
        })
    };

    let mut on_event = |event: crate::swarm::SwarmEvent| {
        let mut guard = progress_mutex.lock().unwrap();
        (*guard)(event.into());
    };

    let outcomes = crate::swarm::run_batch(&ctx, tasks, runner, &mut on_event).await;
    Ok(crate::swarm::format_batch_result(&outcomes))
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod full_skill_result_tests;

#[cfg(all(test, unix))]
mod mcp_outcome_tests;

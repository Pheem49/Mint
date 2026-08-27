use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::time::Instant;
use thiserror::Error;

use crate::chat::{
    ChatMessage, ChatRole, ContentBlock, send_chat_with_fallback, stream_chat_with_fallback,
};
use crate::code_tools::{
    CodeEdit, CodePatchHunk, apply_code_edits, build_code_patch, list_code_files,
    propose_code_edits, read_code_file, search_code,
};
use crate::config::ToolCallingMode;
use crate::knowledge::KnowledgeStore;
use crate::plugins::execute_native_plugin;
use crate::prompts::tool_catalog::tool_catalog;
use crate::semantic::{index_semantic_code, search_semantic_code};
use crate::shell::run_shell_command;
use crate::symbols::build_symbol_index;
use crate::{
    Capability, ChatError, ChatRequest, ChatResponse, DEFAULT_CONVERSATION_ID, MemoryError,
    MemoryStore, MintConfig, assert_path_capability, classify_shell_command, send_chat,
    stream_chat,
};

const CONTEXT_LIMIT: usize = 3;
/// Per-message cap (in `chars`, not bytes — Thai text is multi-byte UTF-8)
/// applied to each recalled `user_text`/`ai_text` when building the "recent
/// conversation context" injected into a new task's opening system prompt.
/// Without this, a single unusually long past answer (a multi-KB agent
/// summary, a big code dump) gets replayed *in full* into every subsequent
/// unrelated turn for as long as it stays within the last `CONTEXT_LIMIT`
/// interactions — this exists purely to nudge the model with recent
/// continuity, not to re-litigate a whole previous answer.
const MAX_CONTEXT_MESSAGE_CHARS: usize = 200;

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Chat(#[from] ChatError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error("agent error: {0}")]
    Agent(String),
}

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
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let response = send_chat(config, &enriched).await?;
    memory.add_interaction_for_chat_with_fallback(
        &request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
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
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let response = stream_chat(config, &enriched, on_chunk).await?;
    memory.add_interaction_for_chat_with_fallback(
        &request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    Ok(response)
}

pub async fn orchestrate_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), OrchestrationError> {
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let (response, fallback) = send_chat_with_fallback(config, &enriched).await?;
    memory.add_interaction_for_chat_with_fallback(
        &request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
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
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let (response, fallback) = stream_chat_with_fallback(config, &enriched, on_chunk).await?;
    memory.add_interaction_for_chat_with_fallback(
        &request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    Ok((response, fallback))
}

/// Truncates `text` to at most `max_chars` characters (not bytes, so this
/// never splits a multi-byte UTF-8 character) for injection into a "recent
/// context" summary. Cheap no-op for the common case of a short message.
fn truncate_for_context(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut truncated: String = text.chars().take(max_chars).collect();
    truncated.push_str("... [truncated]");
    truncated
}

fn enrich_request(
    config: &MintConfig,
    memory: &MemoryStore,
    request: &ChatRequest,
) -> Result<ChatRequest, MemoryError> {
    let mut interactions =
        memory.recent_interactions_for_chat(&request_chat_id(request), CONTEXT_LIMIT)?;
    interactions.reverse();
    let transcript = interactions
        .into_iter()
        .map(|item| {
            format!(
                "User: {}\nAssistant: {}",
                truncate_for_context(&item.user_text, MAX_CONTEXT_MESSAGE_CHARS),
                truncate_for_context(&item.ai_text, MAX_CONTEXT_MESSAGE_CHARS)
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut enriched = request.clone();

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

    // Inject active AI model/provider context to system instructions
    let active_model_info = format!(
        "\n\n[Active Environment Context]\n\
         You are running on: {}\n\
         Using AI Model: {}\n",
        config.ai_provider,
        config.active_model()
    );
    enriched.system_instruction.push_str(&active_model_info);

    if !transcript.is_empty() {
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
mod decision_parsing;
mod memory_skill;
mod tools;
mod verification;
mod workspace_helpers;
use context_render::*;
use decision_parsing::*;
pub(crate) use memory_skill::*;
use verification::*;
use workspace_helpers::*;

/// Hard ceiling on tool-call round-trips per task. Each step resends the
/// whole accumulated conversation, so total tokens for one task scale
/// roughly with step count. Was lowered from 32 to 24 to cap runaway/looping
/// tasks earlier, but 24 turned out too tight for legitimate multi-file
/// work, cutting it off mid-task — raised to 40 as a middle ground.
const MAX_STEPS: usize = 40;
const MAX_OBSERVATION_BYTES: usize = 16_000;
/// Compact `native_messages` once reported token usage crosses this fraction
/// of the active model's context window. Every step resends the whole
/// accumulated history (see `MAX_STEPS`'s doc comment above), so this is the
/// main lever on real per-task token volume — not just cost, since provider
/// prompt caching (automatic for OpenAI-compatible providers, explicit
/// `cache_control` breakpoints for Anthropic) only discounts *price* on a
/// resend, it doesn't shrink what's actually sent/counted. Lowered from 0.8,
/// then from 0.6 to compact considerably earlier still — a task observed
/// hitting 80,949/128,000 tokens on a single step at the 0.6 ratio (63% of
/// window) was well past the point of real benefit from keeping that much
/// verbatim.
const COMPACTION_TRIGGER_RATIO: f64 = 0.4;
/// Number of most-recent Assistant/Tool step-pairs kept verbatim (uncompacted)
/// in `native_messages`.
const COMPACTION_KEEP_RECENT_STEPS: usize = 3;
/// How many times a step retries after every configured provider comes back
/// `ChatError::NetworkUnavailable` before finally giving up — swapping
/// providers is pointless when the network itself is down, so this waits and
/// tries the same request again instead, with `AgentProgress::WaitingForNetwork`
/// keeping the user informed of which attempt it's on.
const NETWORK_RETRY_ATTEMPTS: usize = 4;
const NETWORK_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(5);

/// A single pick-a-choice option offered by the `ask_user` tool, with an
/// optional one-line explanation shown under the label in both the CLI
/// picker and the desktop/web approval card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskUserOption {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentApproval {
    WriteFile {
        path: String,
        content: String,
        diff: String,
    },
    ApplyPatch {
        path: String,
        hunks: Vec<CodePatchHunk>,
        diff: String,
    },
    RunShell {
        command: String,
        mode: String,
        background: bool,
    },
    NoteWrite {
        path: String,
        content: String,
    },
    RunPlugin {
        name: String,
        instruction: String,
    },
    McpTool {
        server: String,
        tool: String,
        arguments: Value,
    },
    UserApproval {
        title: String,
        prompt: String,
    },
    AskUser {
        question: String,
        #[serde(default)]
        options: Vec<AskUserOption>,
        #[serde(default)]
        header: Option<String>,
        #[serde(default, rename = "multiSelect")]
        multi_select: bool,
    },
    ExitPlanMode {
        plan: String,
    },
    EnterPlanMode {
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalOutcome {
    Approved,
    Denied,
    Intercepted(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentProgress {
    Thinking {
        elapsed_secs: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model_name: Option<String>,
        /// Context-window usage as of the last completed step, 0-100 — the
        /// last thing we actually know, since usage is only reported in a
        /// step's *response*, not predictable mid-generation. `None` until
        /// the first step of a task has completed at least once.
        #[serde(skip_serializing_if = "Option::is_none")]
        context_pct: Option<u8>,
        /// Running sum of `total_tokens` across every step completed so far
        /// this turn — same accumulator as `AgentResult::total_tokens`, just
        /// surfaced live instead of only once the turn finishes, so the CLI
        /// can count it up next to the "Thinking…" label the way Claude Code
        /// does. 0 until the first step has completed.
        #[serde(default)]
        tokens_used: u64,
        /// Prompt/context tokens the *most recent* completed step processed
        /// (system prompt + tool schemas + the whole resent history) — the
        /// `↑` half of the live counter. Latest-step, not summed: it tracks
        /// how full the context window is, and summing would count the resent
        /// history once per step. 0 until the first step has completed.
        #[serde(default)]
        input_tokens: u64,
        /// Running sum of completion tokens the model generated across every
        /// step this turn — the `↓` half. Summed (unlike `input_tokens`),
        /// since each step's output is distinct work. 0 until the first step
        /// has completed.
        #[serde(default)]
        generated_tokens: u64,
        /// Rough char-count-based estimate of the very first request's size
        /// — constant across every step of the turn. Only meaningful to a
        /// consumer while `tokens_used` is still 0 (i.e. step 1's response
        /// hasn't come back yet), as a stand-in until the real number
        /// exists; ignored once `tokens_used` is nonzero.
        #[serde(default)]
        estimated_tokens: u64,
    },
    Thought {
        thought: String,
    },
    /// Emitted while retrying a step after every configured provider was
    /// unreachable (`ChatError::NetworkUnavailable`) — distinct from
    /// `Thinking` because there's nothing to wait *on* here except the
    /// network itself coming back, not a model generating a reply.
    WaitingForNetwork {
        attempt: usize,
        max_attempts: usize,
    },
    ToolStart {
        action: String,
        input: Value,
        /// Name of the subagent this tool call happened inside, if any —
        /// `None` for the top-level agent's own calls. Set by
        /// `dispatch_one_subagent` wrapping the nested loop's `progress` so
        /// the CLI/GUI can render a subagent's own tool calls nested under
        /// its `dispatch_subagent` call instead of indistinguishable from
        /// the parent's.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent: Option<String>,
    },
    ToolEnd {
        action: String,
        input: Value,
        result: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResult {
    pub provider: String,
    pub model: String,
    pub summary: String,
    pub verification: String,
    pub fallback: Option<String>,
    /// Sum of `total_tokens` across every step's API response this turn —
    /// see the doc comment on `turn_total_tokens` where it's accumulated.
    pub total_tokens: u64,
    /// Prompt/context tokens the final step processed (latest-step, not summed
    /// — see `AgentProgress::Thinking::input_tokens`). The `↑` half of the
    /// turn footer's token counter.
    #[serde(default)]
    pub input_tokens: u64,
    /// Sum of completion tokens generated across every step this turn — the
    /// `↓` half of the turn footer's token counter.
    #[serde(default)]
    pub generated_tokens: u64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentDirectoryEntry {
    name: String,
    path: PathBuf,
    kind: &'static str,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AgentDecision {
    #[serde(default)]
    thought: String,
    action: String,
    #[serde(default, deserialize_with = "deserialize_agent_input")]
    input: AgentInput,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentInput {
    #[serde(default)]
    path: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    options: Vec<AskUserOptionInput>,
    #[serde(default)]
    header: String,
    #[serde(default)]
    multi_select: bool,
    #[serde(default)]
    city: String,
    #[serde(default)]
    expression: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    background: bool,
    #[serde(default)]
    job_id: String,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    steps: Vec<String>,
    #[serde(default)]
    file_content: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    verification: String,
    #[serde(default)]
    plan: String,
    #[serde(default)]
    reason: String,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    patch: Option<AgentPatch>,
    #[serde(default)]
    server: String,
    #[serde(default)]
    tool: String,
    #[serde(default)]
    arguments: Value,
    #[serde(default)]
    note_path: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    instruction: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    selector: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    button: String,
    #[serde(default)]
    key: String,
    // Video tools input fields
    #[serde(default)]
    input: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    width: Option<i32>,
    #[serde(default)]
    height: Option<i32>,
    #[serde(default)]
    frame_count: Option<u32>,
    #[serde(default)]
    columns: Option<u32>,
    #[serde(default)]
    threshold_db: Option<f64>,
    #[serde(default)]
    min_silence_secs: Option<f64>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    target_language: Option<String>,
    #[serde(default)]
    srt_content: Option<String>,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    max_clips: Option<u32>,
    #[serde(default)]
    target_duration: Option<f64>,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    order: Vec<usize>,
    #[serde(default)]
    music_input: Option<String>,
    #[serde(default)]
    video_input: Option<String>,
    #[serde(default)]
    music_volume: Option<f32>,
    #[serde(default)]
    zoom_factor: Option<f32>,
    // Image & Video generation input fields
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    aspect_ratio: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    duration: Option<f64>,
    // avatar_signal input fields — Rust-side `avatar_` prefix only to read
    // unambiguously in this shared flat struct; explicit renames keep the
    // wire/JSON keys unprefixed (matching the relay's AvatarEvent schema —
    // see `crate::avatar_bridge`).
    #[serde(default, rename = "emotions")]
    avatar_emotions: std::collections::HashMap<String, String>,
    #[serde(default, rename = "action")]
    avatar_action: String,
    #[serde(default, rename = "prop")]
    avatar_prop: String,
    #[serde(default, rename = "intensity")]
    avatar_intensity: String,
    #[serde(default, rename = "color")]
    avatar_color: String,
    #[serde(default, rename = "talking")]
    avatar_talking: Option<bool>,
}

/// Decode shim for `ask_user`'s `options`: accepts either a bare string
/// (the legacy shape, and what a model may still emit even after the schema
/// documents the object shape) or `{label, description}`.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
enum AskUserOptionInput {
    Plain(String),
    Detailed {
        label: String,
        #[serde(default)]
        description: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentPatch {
    path: PathBuf,
    #[serde(default)]
    hunks: Vec<CodePatchHunk>,
}

fn deserialize_agent_input<'de, D>(deserializer: D) -> Result<AgentInput, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<AgentInput>::deserialize(deserializer)?.unwrap_or_default())
}

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

/// Calls `send_chat_with_fallback`, retrying up to `NETWORK_RETRY_ATTEMPTS`
/// times (waiting `NETWORK_RETRY_DELAY` between each) when every configured
/// provider comes back `ChatError::NetworkUnavailable` — swapping providers
/// is pointless when the network itself is down, so this waits for it to
/// come back instead of burning through the provider list, keeping the user
/// informed via `AgentProgress::WaitingForNetwork` in the meantime. Any other
/// error (including `NetworkUnavailable` after the last attempt) is returned
/// immediately, unchanged.
async fn send_chat_with_network_retry(
    config: &MintConfig,
    request: &ChatRequest,
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<(ChatResponse, Option<String>), ChatError> {
    let mut attempt = 0;
    loop {
        match send_chat_with_fallback(config, request).await {
            Err(ChatError::NetworkUnavailable) if attempt < NETWORK_RETRY_ATTEMPTS => {
                attempt += 1;
                progress(AgentProgress::WaitingForNetwork {
                    attempt,
                    max_attempts: NETWORK_RETRY_ATTEMPTS,
                });
                tokio::time::sleep(NETWORK_RETRY_DELAY).await;
            }
            other => return other,
        }
    }
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
    Box::pin(async move {
        let started_at = Instant::now();
        let root = root.canonicalize().map_err(|e| {
            OrchestrationError::Agent(format!(
                "unable to resolve workspace root {}: {}",
                root.display(),
                e
            ))
        })?;
        let resolved_task = resolve_github_links(task, config).await;
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
        let chat_id = crate::agent::memory::scoped_chat_id(chat_id, Some(&root.to_string_lossy()));
        let chat_id = chat_id.as_str();
        // Subagent runs use a synthetic `{parent_chat_id}::subagent::{name}` chat id
        // (see the `dispatch_subagent` arm in `execute_tool`) so their own memory
        // interaction doesn't leak into the parent conversation's history. That
        // same marker doubles as the depth-limit signal here: a subagent's own
        // nested loop never offers `dispatch_subagent` in its tool catalog, so it
        // can't recurse into further subagents.
        let allow_subagent_dispatch = !chat_id.contains("::subagent::");
        let skills =
            crate::skills::learned_skills_context(Some(&root), Some(chat_id)).unwrap_or_default();
        let mut observation = initial_observation(&resolved_task, &root, &skills);
        let mut pending_image = image_data_uri;
        let mut pending_audio = audio_data_uri;
        let mut pending_video = video_data_uri;

        let mut plan_mode = plan_mode;
        // A pin only means anything if it names a server that's actually configured —
        // a stale/hand-typed `@name` (e.g. from a since-disabled server) is silently
        // dropped rather than restricting the turn to a server that doesn't exist.
        let pinned_mcp_server = pinned_mcp_server.filter(|p| {
            crate::mcp::list_mcp_servers()
                .map(|m| m.contains_key(*p))
                .unwrap_or(false)
        });
        // Determined once from the base `config` (not the per-step `active_config`
        // multi-agent collaboration can substitute) — matches how `system_prompt`
        // itself is only rebuilt on plan-mode transitions, not every step.
        let system_prompt_native = config.tool_calling_mode() == ToolCallingMode::Native;
        let mut system_prompt = build_system_prompt(
            config,
            plan_mode,
            system_prompt_native,
            user_name,
            pinned_mcp_server,
        );
        let hooks = crate::hooks::list_hooks(config);

        append_memory_context(&mut system_prompt, chat_id);
        // Rough token estimate for the very first request going out — lets
        // the CLI seed its live counter before step 1's real response comes
        // back and there's nothing else to go on yet (~4 chars/token, a
        // widely-used approximation; ignores the tool catalog's own size —
        // built per-step further down, not worth duplicating here just for
        // this — so it undercounts a bit for native tool-calling, but only
        // needs to be in the right ballpark for a still-updating live label,
        // not exact).
        let estimated_first_step_tokens =
            ((system_prompt.chars().count() + observation.chars().count()) / 4) as u64;

        #[allow(unused_assignments)]
        let mut final_provider = config.ai_provider.clone();
        #[allow(unused_assignments)]
        let mut final_model = "".to_string();
        let mut final_fallback = None;
        let mut action_counts = BTreeMap::<String, usize>::new();
        // Track the most recent step (if any) that successfully modified a file
        // (`apply_patch`/`write_file`) and the most recent step that ran `verify`,
        // so `finish` can be rejected when code was changed but never checked —
        // see the gate right before the `finish` handling block below.
        let mut last_modify_step: Option<usize> = None;
        let mut last_verify_step: Option<usize> = None;
        // Whether the most recent `verify` call actually passed — separate from
        // `last_verify_step`, which only records that one *ran*. Lets `finish` be
        // rejected when the agent ignores a real failure and claims success anyway;
        // see `unacknowledged_verify_failure` right before the `finish` handling block.
        let mut last_verify_failed: Option<bool> = None;
        let mut trajectory: Vec<String> = Vec::new();
        // Structured history for native tool-calling providers, maintained alongside
        // `trajectory`/`observation` (which remain the source of truth for the
        // JSON-prompt fallback path and for the web-search/media-append scans in the
        // finish handler below, which operate on flattened text either way).
        let mut native_messages: Vec<ChatMessage> = Vec::new();
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

        'steps: for step in 1..=MAX_STEPS {
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

            let tool_mode = active_config.tool_calling_mode();

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
                if native_messages.is_empty() {
                    // Native tool-calling providers build the request from `messages`
                    // and ignore ChatRequest.image_data_uri/audio_data_uri/video_data_uri
                    // entirely, so any pending attachment must be embedded as content
                    // blocks on the first user turn here, not left for the (unused)
                    // top-level fields — otherwise Agent Mode would silently drop
                    // attachments that work fine with Agent Mode off.
                    let mut content = vec![ContentBlock::Text {
                        text: observation.clone(),
                    }];
                    if let Some(image_data) = pending_image.take() {
                        for img in image_data.split_whitespace() {
                            content.push(ContentBlock::Image {
                                data_uri: img.to_owned(),
                            });
                        }
                    }
                    if let Some(audio_data) = pending_audio.take() {
                        for aud in audio_data.split_whitespace() {
                            content.push(ContentBlock::Audio {
                                data_uri: aud.to_owned(),
                            });
                        }
                    }
                    if let Some(video_data) = pending_video.take() {
                        for vid in video_data.split_whitespace() {
                            content.push(ContentBlock::Video {
                                data_uri: vid.to_owned(),
                            });
                        }
                    }
                    native_messages.push(ChatMessage {
                        role: ChatRole::User,
                        content,
                    });
                }
                send_chat_with_network_retry(
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
                    },
                    &mut progress,
                )
                .await?
            } else {
                send_chat_with_network_retry(
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
                    },
                    &mut progress,
                )
                .await?
            };

            final_provider = response.provider.clone();
            final_model = response.model.clone();
            turn_total_tokens += response.total_tokens.unwrap_or(0) as u64;
            turn_generated_tokens += response.output_tokens.unwrap_or(0) as u64;
            if let Some(input) = response.input_tokens {
                last_input_tokens = input as u64;
            }
            if fallback.is_some() {
                // `fallback` (this function's own return value) is the provider
                // that actually served this response; `response.fallback_provider`
                // is a same-shaped but differently-populated field that
                // `send_chat_with_fallback` sets to the *original* provider that
                // failed over — using it here showed e.g. "gemini → fallback:
                // gemini • Qwen..." in the CLI badge instead of "gemini →
                // fallback: huggingface • Qwen...".
                final_fallback = fallback.clone();
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

            // `decisions` is normally a single `(call_id, AgentDecision)`, matching the
            // original one-action-per-step design. Native tool-calling can return
            // several tool calls in one model turn, in which case they're executed
            // sequentially and all their results are fed back before the next call —
            // `finish` never appears alongside real tool calls (see below), so this
            // never conflicts with the early-return finish handling.
            let decisions: Vec<(String, AgentDecision)> = if tool_mode == ToolCallingMode::Native {
                match response.tool_calls.clone() {
                    Some(calls) if !calls.is_empty() => calls
                        .into_iter()
                        .enumerate()
                        .map(|(index, call)| {
                            let thought = if index == 0 {
                                response.text.trim().to_string()
                            } else {
                                String::new()
                            };
                            let input: AgentInput =
                                serde_json::from_value(call.input).unwrap_or_default();
                            (
                                call.id,
                                AgentDecision {
                                    thought,
                                    action: call.name,
                                    input,
                                },
                            )
                        })
                        .collect(),
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
                        vec![(
                            format!("call_{step}_finish"),
                            AgentDecision {
                                thought: String::new(),
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
                let decision = match parse_decision_or_finish(&response.text) {
                    Ok(decision) => decision,
                    Err(_) => {
                        let (repaired, _) = send_chat_with_fallback(
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
                        },
                    )
                    .await?;
                        parse_decision_or_finish(&repaired.text).map_err(|e| {
                            OrchestrationError::Agent(format!(
                                "unable to repair invalid agent response: {}",
                                e
                            ))
                        })?
                    }
                };
                vec![(format!("call_{step}"), decision)]
            };

            let mut step_tool_results: Vec<(String, String, Value, String)> = Vec::new();
            // Screenshots (and any future binary/image tool result) are kept out of
            // `step_tool_results`'s plain-text `final_result` — that string is what
            // both the text `trajectory` and the JSON-prompt `observation` are built
            // from, and a multi-hundred-KB base64 PNG would blow straight through
            // `MAX_OBSERVATION_BYTES` and get silently chopped mid-base64. The full
            // data URI is stashed here by `call_id` instead, and re-attached as a
            // real `ContentBlock::Image` when building `native_messages` below, so
            // the model actually sees the pixels instead of a truncated text blob.
            let mut step_images: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            if decisions_are_parallel_subagent_batch(&decisions) {
                step_tool_results = run_parallel_subagent_batch(
                    &decisions,
                    step,
                    &root,
                    config,
                    chat_id,
                    &mut approve,
                    &mut progress,
                    &mut action_counts,
                    &mut trajectory,
                )
                .await;
            } else {
                for (call_id, decision) in decisions {
                    if !fast_mode
                        && decision.action != "finish"
                        && !decision.thought.trim().is_empty()
                    {
                        progress(AgentProgress::Thought {
                            thought: decision.thought.trim().to_owned(),
                        });
                    }

                    if decision.action == "finish" {
                        let mut summary = decision.input.summary.trim().to_owned();
                        let is_thai_task =
                            task.chars().any(|c| ('\u{0e00}'..='\u{0e7f}').contains(&c));
                        if let Some(err_line) = observation
                            .lines()
                            .find(|l| l.contains("Web search error:"))
                        {
                            let clean_err = err_line
                                .replace("Web search error: ", "")
                                .replace("Web search is currently unavailable.", "")
                                .trim()
                                .to_string();
                            if summary.is_empty() {
                                if is_thai_task {
                                    summary = format!(
                                        "การค้นหาข้อมูลจากเว็บล้มเหลวเนื่องจากข้อผิดพลาด: {}\nมิ้นท์ขออภัยด้วยนะคะที่ไม่สามารถค้นหาข้อมูลเรียลไทม์ให้ได้ในขณะนี้ค่ะ",
                                        clean_err
                                    );
                                } else {
                                    summary = format!(
                                        "Web search failed due to error: {}\nI apologize, but I cannot retrieve real-time information at the moment.",
                                        clean_err
                                    );
                                }
                            } else {
                                let err_lower = clean_err.to_lowercase();
                                let summary_lower = summary.to_lowercase();
                                let already_mentions_error = if is_thai_task {
                                    summary_lower.contains("ล้มเหลว")
                                        || summary_lower.contains("ข้อผิดพลาด")
                                        || summary_lower.contains(&err_lower)
                                } else {
                                    summary_lower.contains("fail")
                                        || summary_lower.contains("error")
                                        || summary_lower.contains(&err_lower)
                                };
                                if !already_mentions_error {
                                    if is_thai_task {
                                        summary.push_str(&format!(
                                            "\n\n(การค้นหาเว็บล้มเหลวเนื่องจากข้อผิดพลาด: {})",
                                            clean_err
                                        ));
                                    } else {
                                        summary.push_str(&format!(
                                            "\n\n(Web search failed due to error: {})",
                                            clean_err
                                        ));
                                    }
                                }
                            }
                        } else {
                            if summary.is_empty() {
                                let err_msg = "Error: Your finish action summary was empty. \
                                       You MUST provide a final answer, explanation, or response to the user's query \
                                       in the 'summary' field of the 'finish' action input. Do not leave it empty.";
                                trajectory.push(format!(
                                    "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                                    decision.thought.trim(),
                                    decision.action,
                                    err_msg
                                ));
                                let history_str = trajectory.join("\n\n");
                                observation = format!(
                                    "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                                    root.display(),
                                    history_str
                                );
                                reject_native_finish(
                                    tool_mode,
                                    &mut native_messages,
                                    &response.text,
                                    err_msg,
                                );
                                continue 'steps;
                            }
                            if unverified_modification(
                                last_modify_step,
                                last_verify_step,
                                &decision.input.verification,
                            ) {
                                let err_msg = "Error: You modified a file (apply_patch/write_file) in this \
                                       run but finished without verifying it. Call the verify tool \
                                       with build/test/lint commands appropriate for this project \
                                       before finishing. If no check genuinely applies (e.g. no test \
                                       suite, documentation-only change), say so explicitly in the \
                                       finish action's 'verification' field and finish again.";
                                trajectory.push(format!(
                                    "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                                    decision.thought.trim(),
                                    decision.action,
                                    err_msg
                                ));
                                let history_str = trajectory.join("\n\n");
                                observation = format!(
                                    "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                                    root.display(),
                                    history_str
                                );
                                reject_native_finish(
                                    tool_mode,
                                    &mut native_messages,
                                    &response.text,
                                    err_msg,
                                );
                                continue 'steps;
                            }
                            if unacknowledged_verify_failure(
                                last_verify_failed,
                                &decision.input.verification,
                            ) {
                                let err_msg = "Error: Your last verify call reported a failure \
                                       (non-zero exit code), but you're finishing without \
                                       addressing it. Read the stdout/stderr from that verify \
                                       call, fix the actual problem, and run verify again until \
                                       it passes. Do not report success in the finish summary \
                                       while a real check is failing — if the failure is genuinely \
                                       unrelated to your change (e.g. pre-existing), say so \
                                       explicitly in the finish action's 'verification' field and \
                                       finish again.";
                                trajectory.push(format!(
                                    "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                                    decision.thought.trim(),
                                    decision.action,
                                    err_msg
                                ));
                                let history_str = trajectory.join("\n\n");
                                observation = format!(
                                    "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                                    root.display(),
                                    history_str
                                );
                                reject_native_finish(
                                    tool_mode,
                                    &mut native_messages,
                                    &response.text,
                                    err_msg,
                                );
                                continue 'steps;
                            }
                            let mut provider_used = None;
                            for line in observation.lines() {
                                if line.contains("Web search succeeded using Google Search") {
                                    provider_used = Some("Google");
                                } else if line.contains("Web search succeeded using Brave Search") {
                                    provider_used = Some("Brave");
                                }
                            }
                            if let Some(prov) = provider_used {
                                let summary_lower = summary.to_lowercase();
                                if !summary_lower.contains("google")
                                    && !summary_lower.contains("brave")
                                {
                                    if is_thai_task {
                                        summary.push_str(&format!(
                                            "\n\n(มิ้นท์หาข้อมูลนี้มาจาก {} Search นะคะ 💖)",
                                            prov
                                        ));
                                    } else {
                                        summary.push_str(&format!(
                                            "\n\n(Information retrieved via {} Search 💖)",
                                            prov
                                        ));
                                    }
                                }
                            }
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

                        on_chunk(summary.clone());

                        let memory = MemoryStore::open_default()?;
                        memory.add_interaction_for_chat_with_fallback(
                            chat_id,
                            task,
                            &summary,
                            &final_provider,
                            &final_model,
                            final_fallback.as_deref(),
                        )?;
                        memory.save_workspace_session(
                            &root.to_string_lossy(),
                            &summary,
                            &verification,
                        )?;
                        spawn_auto_memory_update(config.clone(), task.to_string(), summary.clone());
                        crate::linked_folders::spawn_linked_folder_note(
                            config.clone(),
                            task.to_string(),
                            summary.clone(),
                        );
                        if config.auto_skill_writing && looks_skill_worthy(step, &action_counts) {
                            spawn_auto_skill_write(
                                config.clone(),
                                task.to_string(),
                                summary.clone(),
                                root.clone(),
                                skills.clone(),
                            );
                        }

                        return Ok(AgentResult {
                            provider: final_provider,
                            model: final_model,
                            summary,
                            verification,
                            fallback: final_fallback,
                            total_tokens: turn_total_tokens,
                            input_tokens: last_input_tokens,
                            generated_tokens: turn_generated_tokens,
                        });
                    }

                    let action_key = action_fingerprint(&decision);
                    let action_count = {
                        let count = action_counts.entry(action_key).or_insert(0);
                        *count += 1;
                        *count
                    };

                    // Set only on the real `execute_tool` path below (PreHookOutcome::Allowed);
                    // stays false for plan-mode/hook blocks and the duplicate-shell skip, since
                    // those don't actually run anything and shouldn't count toward verification.
                    let mut action_succeeded = false;
                    let result = if decision.action == "enter_plan_mode" {
                        let reason = decision.input.reason.trim().to_owned();
                        match approve(&AgentApproval::EnterPlanMode {
                            reason: reason.clone(),
                        }) {
                            Ok(ApprovalOutcome::Approved) => {
                                plan_mode = true;
                                system_prompt = build_system_prompt(
                                    config,
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
                        system_prompt = build_system_prompt(config, plan_mode, system_prompt_native, user_name, pinned_mcp_server);
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
                    } else if decision.action == "run_shell" && action_count > 1 {
                        format!(
                            "Skipped duplicate shell command: {}\n\n[System Tip: This exact shell command already ran once in this task. Do not run it again. Use the finish action now and tell the user the action was completed.]",
                            decision.input.command.trim()
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
                            serde_json::to_value(&decision.input).unwrap_or(Value::Null);
                        progress(AgentProgress::ToolStart {
                            action: decision.action.clone(),
                            input: input_val.clone(),
                            subagent: None,
                        });

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
                                let (tool_result, success) = match execute_tool(
                                    &root,
                                    config,
                                    &decision,
                                    chat_id,
                                    &mut approve,
                                    &mut progress,
                                )
                                .await
                                {
                                    Ok(result) => (result, true),
                                    Err(error) => (format!("Error: {}", error), false),
                                };
                                action_succeeded = success;
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
                    };

                    progress(AgentProgress::ToolEnd {
                        action: decision.action.clone(),
                        input: serde_json::to_value(&decision.input).unwrap_or(Value::Null),
                        result: result.clone(),
                        subagent: None,
                    });

                    if action_succeeded {
                        match decision.action.as_str() {
                            "apply_patch" | "write_file" => last_modify_step = Some(step),
                            // Counts even if the commands it ran failed — an attempted check
                            // still counts as verification having been attempted; whether it
                            // actually passed is tracked separately in `last_verify_failed`
                            // and enforced by `unacknowledged_verify_failure` at finish time.
                            "verify" => {
                                last_verify_step = Some(step);
                                last_verify_failed = Some(shell_result_failed(&result));
                            }
                            _ => {}
                        }
                    }

                    let mut final_result = if result.starts_with("data:image/")
                        && matches!(
                            decision.action.as_str(),
                            "browser_screenshot"
                                | "video_filmstrip"
                                | "video_waveform"
                                | "view_image"
                        ) {
                        step_images.insert(call_id.clone(), result.clone());
                        match decision.action.as_str() {
                            "video_filmstrip" => "[Filmstrip generated — see attached image: \
                                sampled frames across the video timeline]"
                                .to_string(),
                            "video_waveform" => "[Waveform generated — see attached image: \
                                audio amplitude over time]"
                                .to_string(),
                            "view_image" => "[Image loaded — see attached image]".to_string(),
                            _ => "[Screenshot captured — see attached image]".to_string(),
                        }
                    } else {
                        truncate(&result)
                    };
                    if decision.action == "run_shell" || decision.action == "verify" {
                        if shell_result_failed(&result) {
                            final_result.push_str(
                        "\n\n[System Tip: The command failed with a non-zero exit code. \
                         Analyze the stdout/stderr above to locate the error, read the offending files, \
                         apply corrected edits (using apply_patch), and run the verification command again. \
                         Do not finish or stop until the compilation or test errors are resolved!]"
                    );
                        }
                    }
                    if decision.action == "apply_patch" || decision.action == "write_file" {
                        final_result.push_str(
                    "\n\n[System Tip: The file edit was approved and applied successfully. \
                     Before finishing, verify this change with the verify tool (build/test/lint, \
                     whatever fits this project) — finish will be rejected until you do, unless you \
                     state in the finish action's verification field why no check applies (e.g. no \
                     test suite, documentation-only change). Do not broaden the scope, do not make \
                     additional unrelated edits, and do not reread the same file unless you need one \
                     concise verification read.]",
                );
                    }
                    if action_count >= 3 {
                        final_result.push_str(
                    "\n\n[System Tip: You repeated the same tool action three or more times. \
                     Stop repeating it. If you already have enough information or the requested edit is done, \
                     use the finish action now. Otherwise choose a different necessary action.]",
                );
                    }

                    trajectory.push(format!(
                        "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                        decision.thought.trim(),
                        decision.action,
                        final_result
                    ));

                    step_tool_results.push((
                        call_id,
                        decision.action.clone(),
                        serde_json::to_value(&decision.input).unwrap_or(Value::Null),
                        final_result,
                    ));
                } // end `for (call_id, decision) in decisions`
            } // end `else` (sequential path)

            if tool_mode == ToolCallingMode::Native {
                let mut assistant_content: Vec<ContentBlock> = Vec::new();
                let response_text = response.text.trim();
                if !response_text.is_empty() {
                    assistant_content.push(ContentBlock::Text {
                        text: response_text.to_string(),
                    });
                }
                let mut tool_result_content: Vec<ContentBlock> = Vec::new();
                for (call_id, action, input_value, final_result) in &step_tool_results {
                    assistant_content.push(ContentBlock::ToolUse {
                        id: call_id.clone(),
                        name: action.clone(),
                        input: input_value.clone(),
                        thought_signature: step_thought_signatures.get(call_id).cloned(),
                    });
                    tool_result_content.push(ContentBlock::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: final_result.clone(),
                        is_error: false,
                    });
                    if let Some(data_uri) = step_images.get(call_id) {
                        tool_result_content.push(ContentBlock::Image {
                            data_uri: data_uri.clone(),
                        });
                    }
                }
                if !assistant_content.is_empty() {
                    native_messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: assistant_content,
                    });
                }
                if !tool_result_content.is_empty() {
                    native_messages.push(ChatMessage {
                        role: ChatRole::Tool,
                        content: tool_result_content,
                    });
                }

                if let Some(total_tokens) = response.total_tokens {
                    let window = active_config.context_window_tokens();
                    last_context_pct = Some(
                        ((total_tokens as f64 / window as f64) * 100.0).clamp(0.0, 255.0) as u8,
                    );
                    if (total_tokens as f64) >= (window as f64) * COMPACTION_TRIGGER_RATIO {
                        match compact_native_messages(&active_config, &native_messages).await {
                            Ok(Some((compacted, fallback_provider))) => {
                                native_messages = compacted;
                                let fallback_note = fallback_provider
                                    .as_deref()
                                    .map(|p| {
                                        format!(
                                            " (summary generated via fallback provider \
                                             \"{p}\" — the primary provider failed for this \
                                             call)"
                                        )
                                    })
                                    .unwrap_or_default();
                                progress(AgentProgress::Thought {
                                    thought: format!(
                                        "[Context] Compacted earlier steps to stay under the \
                                     context window ({total_tokens}/{window} tokens before \
                                     compaction){fallback_note}."
                                    ),
                                });
                            }
                            // Nothing worth compacting yet (too little history) — routine, no warning.
                            Ok(None) => {}
                            Err(error) => {
                                progress(AgentProgress::Thought {
                                    thought: format!(
                                        "[Context] Context is approaching the model's window \
                                     ({total_tokens}/{window} tokens) but compaction failed: \
                                     {error}. Continuing without compacting this step."
                                    ),
                                });
                            }
                        }
                    }
                }
            }

            let history_str = trajectory.join("\n\n");
            observation = format!(
                "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                root.display(),
                history_str
            );
        }

        Err(OrchestrationError::Agent(format!(
            "code agent reached the limit of {} steps",
            MAX_STEPS
        )))
    })
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

fn plan_mode_allows(action: &str, input: &AgentInput) -> bool {
    if action == "run_shell" {
        return classify_shell_command(&input.command).mode.as_str() == "readOnly";
    }
    action == "finish" || crate::prompts::agent::PLAN_MODE_ALLOWED_ACTIONS.contains(&action)
}

/// Runs one subagent to completion and returns its formatted result text.
/// Extracted from `execute_tool`'s `"dispatch_subagent"` arm so both the plain
/// sequential path (a single subagent call, or one mixed in with other actions)
/// and the parallel path (`decisions_are_parallel_subagent_batch`, driven by
/// `buffer_unordered` in the main step loop) share one implementation. Takes
/// the same `&mut dyn FnMut(...)` trait object as `execute_tool` — see the
/// comment on that function for why a trait object rather than a generic —
/// which lets the parallel path pass a small Mutex-backed adapter closure so
/// concurrently-running subagents still share one real approval gate.
async fn dispatch_one_subagent(
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    name: &str,
    task: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<String, OrchestrationError> {
    let Some(definition) = crate::subagents::find_subagent(name, Some(root)) else {
        return Err(OrchestrationError::Agent(format!(
            "no subagent named '{name}' found (check .agents/subagents/ or \
             ~/.config/mint/mint-agents/)"
        )));
    };

    let mut sub_config = config.clone();
    if let Some(provider) = &definition.provider {
        sub_config.ai_provider = provider.clone();
    }
    if let Some(model) = &definition.model {
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

    // The subagent's own persona/instructions are folded into the task
    // framing rather than replacing Mint's system prompt (which
    // `orchestrate_agent_loop` builds internally and isn't a parameter),
    // so the subagent still follows the same tool-use protocol and
    // safety rules as any other agent run.
    let sub_task = format!(
        "{}\n\nTask from parent agent: {task}",
        definition.system_prompt
    );
    let sub_chat_id = format!("{chat_id}::subagent::{}", definition.name);

    // Docker sandbox: a subagent whose resolved backend is "docker" gets one
    // container for its whole run, started here and torn down unconditionally
    // below (success or failure) — never per individual `run_shell` call. See
    // `docker_sandbox`'s module doc for why this is a session-scoped resource
    // rather than something `run_shell_command` starts itself. Subagents with
    // no `run_shell` in their tool list (e.g. `explorer`/`plan`) never need a
    // container, so skip starting one for them.
    let allows_run_shell = definition
        .tools
        .as_ref()
        .is_none_or(|tools| tools.iter().any(|t| t == "run_shell"));
    let backend = definition
        .sandbox
        .as_deref()
        .unwrap_or(&config.sandbox_backend);
    let docker_started = if backend == "docker" && allows_run_shell {
        match crate::docker_sandbox::start_session(&sub_chat_id, root, &sub_config) {
            Ok(()) => true,
            Err(error) if config.sandbox_mode.trim().eq_ignore_ascii_case("enforce") => {
                return Err(OrchestrationError::Agent(format!(
                    "subagent '{}' requires the docker sandbox (sandboxMode=enforce) but it \
                     failed to start: {error}",
                    definition.name
                )));
            }
            // "prefer" (or any other non-"enforce" value): fall back to the
            // subagent's shell commands running unconfined/OS-sandboxed,
            // matching `run_shell_command`'s own fallback semantics for a
            // missing OS sandbox binary.
            Err(_) => false,
        }
    } else {
        false
    };

    // Recursing into `orchestrate_agent_loop` from inside `execute_tool`
    // (which it itself calls) requires boxing this one call — Rust
    // can't compute a finite size for a directly self-referential
    // async fn cycle otherwise. `approve_cb` is reborrowed rather than
    // moved so the subagent's mutating actions still go through the
    // same approval gate as the caller's; `chunk` stays a no-op so the
    // subagent's own streamed answer text never lands in the parent's chat
    // — only its final summary (returned below) does. `progress` is *not*
    // a no-op: `ToolStart`/`ToolEnd` are re-tagged with this subagent's name
    // and forwarded to the caller's real `progress`, so the CLI/GUI can
    // render the subagent's own tool calls nested under its
    // `dispatch_subagent` call — this only affects what's shown in the UI,
    // never what reaches the parent model's context (that isolation comes
    // from `sub_chat_id`/`native_messages` staying local to this call, not
    // from suppressing progress).
    // `orchestrate_agent_loop` itself returns a boxed `dyn Future`
    // (see its doc comment) specifically so this recursive call can
    // just be awaited directly, with no manual boxing needed here.
    let subagent_name = definition.name.clone();
    let mut nested_progress = |event: AgentProgress| {
        let tagged = match event {
            AgentProgress::ToolStart { action, input, .. } => AgentProgress::ToolStart {
                action,
                input,
                subagent: Some(subagent_name.clone()),
            },
            AgentProgress::ToolEnd {
                action,
                input,
                result,
                ..
            } => AgentProgress::ToolEnd {
                action,
                input,
                result,
                subagent: Some(subagent_name.clone()),
            },
            other => other,
        };
        progress(tagged);
    };
    let result = orchestrate_agent_loop(
        &sub_config,
        &sub_task,
        root,
        None,
        None,
        None,
        Some(&sub_chat_id),
        None,
        None,
        None,
        true,
        false,
        &mut *approve_cb,
        &mut nested_progress,
        |_| {},
    )
    .await;

    let outcome = match result {
        Ok(agent_result) => Ok(format!(
            "[Subagent '{}' result]\n{}",
            definition.name, agent_result.summary
        )),
        Err(error) => Err(OrchestrationError::Agent(format!(
            "subagent '{}' failed: {error}",
            definition.name
        ))),
    };
    // Unconditional teardown — runs on both the Ok and Err arms above, since
    // an orphaned container left running after a failed subagent is exactly
    // the failure mode a session-scoped sandbox needs to avoid.
    if docker_started {
        crate::docker_sandbox::stop_session(&sub_chat_id);
    }
    outcome
}

/// Runs a step's `dispatch_subagent` decisions concurrently (see
/// `decisions_are_parallel_subagent_batch` for when this is used instead of
/// the normal one-at-a-time loop), capped at `PARALLEL_SUBAGENT_LIMIT` in
/// flight at once. `progress`/`action_counts`/`trajectory` are only touched
/// after every future has completed — back in single-threaded code, in the
/// decisions' original order — so this never needs to share those across
/// concurrent tasks. `approve` is the one resource genuinely needed *during*
/// concurrent execution (a subagent's own risky actions still need real user
/// approval), so it's wrapped in a `std::sync::Mutex` for the batch: each
/// concurrent subagent gets a small adapter closure over a shared `&Mutex`,
/// serializing just the moment of invoking it rather than the whole call.
#[allow(clippy::too_many_arguments)]
async fn run_parallel_subagent_batch(
    decisions: &[(String, AgentDecision)],
    step: usize,
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    action_counts: &mut BTreeMap<String, usize>,
    trajectory: &mut Vec<String>,
) -> Vec<(String, String, Value, String)> {
    let approve_mutex = std::sync::Mutex::new(approve);
    // Concurrently-running subagents share one real `progress` sink the same
    // way they share one real `approve` gate above — each gets a small
    // Mutex-backed adapter closure so their nested `ToolStart`/`ToolEnd`
    // events (tagged with their own subagent name in `dispatch_one_subagent`)
    // still reach the CLI/GUI, interleaved but not corrupted, instead of the
    // batch staying silent until every item in it finishes.
    let progress_mutex = std::sync::Mutex::new(progress);

    let mut dispatches = Vec::with_capacity(decisions.len());
    for (index, (call_id, decision)) in decisions.iter().enumerate() {
        let call_id = call_id.clone();
        let thought = decision.thought.clone();
        let action = decision.action.clone();
        let input_val = serde_json::to_value(&decision.input).unwrap_or(Value::Null);
        let action_key = action_fingerprint(decision);
        let name = decision.input.name.clone();
        let task_text = decision.input.instruction.clone();
        let approve_mutex = &approve_mutex;
        let progress_mutex = &progress_mutex;
        dispatches.push(async move {
            let result: Result<String, OrchestrationError> = if name.trim().is_empty() {
                Err(OrchestrationError::Agent("name is required".into()))
            } else if task_text.trim().is_empty() {
                Err(OrchestrationError::Agent("instruction is required".into()))
            } else {
                let mut approve_adapter =
                    |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                        let mut guard = approve_mutex.lock().unwrap();
                        (*guard)(approval)
                    };
                let mut progress_adapter = |event: AgentProgress| {
                    let mut guard = progress_mutex.lock().unwrap();
                    (*guard)(event);
                };
                dispatch_one_subagent(
                    root,
                    config,
                    chat_id,
                    &name,
                    &task_text,
                    &mut approve_adapter,
                    &mut progress_adapter,
                )
                .await
            };
            (
                index, call_id, thought, action, input_val, action_key, result,
            )
        });
    }

    let mut results = futures_util::stream::iter(dispatches)
        .buffer_unordered(PARALLEL_SUBAGENT_LIMIT)
        .collect::<Vec<_>>()
        .await;
    results.sort_by_key(|(index, ..)| *index);
    // All `dispatches` futures (the only borrowers of `progress_mutex`) have
    // finished by now, so this is the sole remaining handle — safe to unwrap
    // back into a plain `&mut dyn FnMut` for the rest of this function.
    let progress = progress_mutex.into_inner().unwrap();

    let mut step_tool_results = Vec::with_capacity(results.len());
    for (_, call_id, thought, action, input_val, action_key, result) in results {
        progress(AgentProgress::ToolStart {
            action: action.clone(),
            input: input_val.clone(),
            subagent: None,
        });
        let tool_result = match result {
            Ok(text) => text,
            Err(error) => format!("Error: {}", error),
        };
        progress(AgentProgress::ToolEnd {
            action: action.clone(),
            input: input_val.clone(),
            result: tool_result.clone(),
            subagent: None,
        });

        let action_count = {
            let count = action_counts.entry(action_key).or_insert(0);
            *count += 1;
            *count
        };
        let mut final_result = truncate(&tool_result);
        if action_count >= 3 {
            final_result.push_str(
                "\n\n[System Tip: You repeated the same tool action three or more times. \
                 Stop repeating it. If you already have enough information or the requested edit is done, \
                 use the finish action now. Otherwise choose a different necessary action.]",
            );
        }

        trajectory.push(format!(
            "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
            thought.trim(),
            action,
            final_result
        ));
        step_tool_results.push((call_id, action, input_val, final_result));
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

async fn execute_tool(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<String, OrchestrationError> {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" | "read_file" | "note_write" | "apply_patch" | "write_file" => {
            tools::files::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "search_code" | "symbols" | "semantic_index" | "semantic_search" => {
            tools::code_search::execute(
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
            tools::web::execute(
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
            tools::misc::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "browser_open"
        | "browser_click"
        | "browser_type"
        | "browser_read"
        | "browser_mouse_move"
        | "browser_mouse_click"
        | "browser_key_press"
        | "browser_screenshot" => {
            tools::browser::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "git_status" | "git_diff" | "git_log" | "git_branch" => {
            tools::git::execute(
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
            tools::planning::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "detect_project" | "list_tests" | "read_diagnostics" | "view_image" => {
            tools::project::execute(
                decision.action.as_str(),
                input,
                root,
                config,
                chat_id,
                approve_cb,
            )
            .await
        }
        "avatar_signal" => tools::avatar::execute(input).await,
        "run_plugin" | "dispatch_subagent" | "mcp_tool" | "mcp_list_tools" => {
            tools::plugins_mcp::execute(
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
        "run_shell" | "shell_output" | "kill_shell" | "verify" => {
            tools::shell::execute(
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
            tools::media::execute(
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
        input: serde_json::from_value(input).unwrap_or_default(),
    };
    // No progress channel from the realtime voice session — tool-call
    // activity isn't surfaced there the way it is in the CLI/GUI.
    execute_tool(root, config, &decision, chat_id, approve_cb, &mut |_| {}).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;

    #[test]
    fn truncate_for_context_leaves_short_text_untouched() {
        assert_eq!(truncate_for_context("hello", 400), "hello");
    }

    #[test]
    fn truncate_for_context_truncates_long_text_with_a_marker() {
        let long = "a".repeat(500);
        let result = truncate_for_context(&long, 400);
        assert_eq!(
            result.chars().count(),
            400 + "... [truncated]".chars().count()
        );
        assert!(result.starts_with(&"a".repeat(400)));
        assert!(result.ends_with("... [truncated]"));
    }

    #[test]
    fn truncate_for_context_never_splits_a_multibyte_char() {
        // Thai text is multi-byte UTF-8; a byte-index slice here would panic
        // or produce invalid UTF-8 if it landed mid-character.
        let thai = "สวัสดี".repeat(200);
        let result = truncate_for_context(&thai, 5);
        assert_eq!(
            result.chars().count(),
            5 + "... [truncated]".chars().count()
        );
    }

    #[test]
    fn slugify_lowercases_and_collapses_separators() {
        assert_eq!(
            slugify("Retry Flaky Playwright Tests!!"),
            "retry-flaky-playwright-tests"
        );
        assert_eq!(
            slugify("  leading/trailing --dashes--  "),
            "leading-trailing-dashes"
        );
        assert_eq!(slugify("already-a-slug"), "already-a-slug");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn skill_revision_defaults_to_zero_without_a_revisions_line() {
        assert_eq!(skill_revision("no frontmatter at all"), 0);
        assert_eq!(skill_revision("---\ndescription: a skill\n---\nbody"), 0);
    }

    #[test]
    fn skill_revision_reads_back_what_set_skill_revision_wrote() {
        let original = "---\ndescription: a skill\n---\nstep-by-step body";
        let bumped = set_skill_revision(original, 1);
        assert_eq!(skill_revision(&bumped), 1);
        assert!(
            bumped.contains("description: a skill"),
            "must preserve existing frontmatter fields: {bumped}"
        );
        assert!(
            bumped.contains("step-by-step body"),
            "must preserve the body: {bumped}"
        );

        // Refining again must replace the old count, not accumulate a
        // second `revisions:` line alongside it.
        let bumped_again = set_skill_revision(&bumped, 2);
        assert_eq!(skill_revision(&bumped_again), 2);
        assert_eq!(
            bumped_again.matches("revisions:").count(),
            1,
            "expected exactly one revisions line, got: {bumped_again}"
        );
    }

    #[test]
    fn set_skill_revision_adds_a_frontmatter_block_when_content_has_none() {
        let result = set_skill_revision("just a body, no frontmatter", 1);
        assert_eq!(skill_revision(&result), 1);
        assert!(result.contains("just a body, no frontmatter"));
    }

    #[test]
    fn existing_workspace_skill_bodies_reports_none_yet_for_a_fresh_workspace() {
        let root = std::env::temp_dir().join("mint-memory-skill-test-no-skills-yet");
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(existing_workspace_skill_bodies(&root), "(none yet)");
    }

    #[test]
    fn existing_workspace_skill_bodies_includes_full_content_by_slug() {
        let root = std::env::temp_dir().join("mint-memory-skill-test-existing-bodies");
        let _ = std::fs::remove_dir_all(&root);
        let skill_dir = root
            .join(".agents")
            .join("skills")
            .join("retry-flaky-tests");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: retry flaky tests\n---\nrun with --retries=3",
        )
        .unwrap();

        let bodies = existing_workspace_skill_bodies(&root);
        assert!(bodies.contains("--- retry-flaky-tests ---"));
        assert!(bodies.contains("run with --retries=3"));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn skill_worthy_requires_enough_steps_and_substantive_work() {
        let mut counts = BTreeMap::new();
        counts.insert("read_file:foo.rs".to_string(), 1);
        counts.insert("apply_patch:foo.rs".to_string(), 1);

        // Too few steps, even with a substantive action.
        assert!(!looks_skill_worthy(2, &counts));
        // Enough steps, has a substantive action.
        assert!(looks_skill_worthy(3, &counts));

        let mut read_only = BTreeMap::new();
        read_only.insert("read_file:foo.rs".to_string(), 5);
        read_only.insert("web_search::rust patterns".to_string(), 2);
        // Enough steps, but nothing substantive happened.
        assert!(!looks_skill_worthy(5, &read_only));
    }

    #[test]
    fn shell_result_failed_reads_the_exit_line() {
        assert!(!shell_result_failed(
            "exit: 0\nmode: normal\nsandboxed: true\nstdout:\nok\nstderr:\n"
        ));
        assert!(shell_result_failed(
            "exit: 1\nmode: normal\nsandboxed: true\nstdout:\nfail\nstderr:\n"
        ));
        // A killed/unknown-status process isn't treated as a confirmed failure.
        assert!(!shell_result_failed(
            "exit: unknown\nmode: normal\nsandboxed: true\nstdout:\nstderr:\n"
        ));
    }

    #[test]
    fn shell_result_failed_catches_a_later_command_failing_in_a_multi_command_verify() {
        // A multi-command `verify` joins each command's own `run_shell` block with
        // "\n\n" — the first command here passes, the second fails, and the scan
        // must not stop after the first "exit: " line it finds.
        let joined = "exit: 0\nmode: normal\nsandboxed: true\nstdout:\ncargo check ok\nstderr:\n\n\
                       exit: 1\nmode: normal\nsandboxed: true\nstdout:\n\nstderr:\n2 tests failed";
        assert!(shell_result_failed(joined));
    }

    #[test]
    fn unverified_modification_requires_a_verify_call_after_the_last_edit() {
        // No edit at all this run — nothing to verify.
        assert!(!unverified_modification(None, None, ""));
        // Edited, never verified, no explanation given.
        assert!(unverified_modification(Some(2), None, ""));
        // Edited, verified *before* the edit (stale) — still unverified.
        assert!(unverified_modification(Some(2), Some(1), ""));
        // Edited, verified after — satisfied regardless of what verify found.
        assert!(!unverified_modification(Some(2), Some(3), ""));
        // Edited, never verified, but explicitly explained why no check applies.
        assert!(!unverified_modification(
            Some(2),
            None,
            "Docs-only change, no test suite."
        ));
    }

    #[test]
    fn unacknowledged_verify_failure_requires_the_agent_to_address_a_real_failure() {
        // No verify ran yet, or it passed — nothing to block on.
        assert!(!unacknowledged_verify_failure(None, ""));
        assert!(!unacknowledged_verify_failure(Some(false), ""));
        // Verify failed and the agent said nothing about it — this is the
        // "claims success while a check is actually failing" case.
        assert!(unacknowledged_verify_failure(Some(true), ""));
        assert!(unacknowledged_verify_failure(Some(true), "n/a"));
        // Verify failed, but the agent explicitly addressed it in the finish
        // action's verification field (e.g. explaining it's pre-existing).
        assert!(!unacknowledged_verify_failure(
            Some(true),
            "2 pre-existing test failures unrelated to this change; see notes."
        ));
    }

    #[test]
    fn preserves_request_without_history() {
        let store = MemoryStore::open(
            std::env::temp_dir().join(format!("mint-orchestrator-{}.sqlite", std::process::id())),
        );
        let request = ChatRequest {
            message: "hello".into(),
            system_instruction: "system".into(),
            chat_id: None,
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
        };
        let config = MintConfig::default();
        assert!(
            enrich_request(&config, &store, &request)
                .unwrap()
                .system_instruction
                .starts_with("system")
        );
    }

    #[test]
    fn agent_decision_allows_null_input() {
        let decision = parse_decision(r#"{"thought":"done","action":"finish","input":null}"#)
            .expect("null input should parse as default input");

        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());
    }

    #[test]
    fn agent_decision_allows_missing_input() {
        let decision = parse_decision(r#"{"thought":"done","action":"finish"}"#)
            .expect("missing input should parse as default input");

        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());
    }

    #[test]
    fn shorthand_finish_allows_null_or_missing_finish() {
        let decision = parse_decision(r#"{"thought":"done","finish":null}"#)
            .expect("null finish should parse");
        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());

        let decision =
            parse_decision(r#"{"thought":"done"}"#).expect("missing finish should parse");
        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());
    }

    #[test]
    fn shorthand_finish_allows_string_finish_as_summary() {
        let decision = parse_decision(r#"{"thought":"done","finish":"all done!"}"#)
            .expect("string finish should parse as summary");
        assert_eq!(decision.action, "finish");
        assert_eq!(decision.input.summary, "all done!");
    }

    #[test]
    fn write_file_policy_rejects_existing_workspace_file() {
        let root =
            std::env::temp_dir().join(format!("mint-write-file-policy-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("existing.txt");
        std::fs::write(&target, "already here").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };

        let result = validate_new_workspace_file(&root, &config, Path::new("existing.txt"));

        assert!(
            matches!(result, Err(OrchestrationError::Agent(message)) if message.contains("Use apply_patch"))
        );
        let _ = std::fs::remove_file(target);
        let _ = std::fs::remove_dir(root);
    }

    #[test]
    fn agent_list_files_includes_directories() {
        let root = std::env::temp_dir().join(format!(
            "mint-agent-list-directories-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("Bunny Girl")).unwrap();
        std::fs::write(root.join("note.txt"), "hello").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };

        let entries = list_directory_entries(&root, 100, &config).unwrap();

        assert!(entries.iter().any(|entry| {
            entry.name == "Bunny Girl" && entry.kind == "directory" && entry.size.is_none()
        }));
        assert!(entries.iter().any(|entry| {
            entry.name == "note.txt" && entry.kind == "file" && entry.size == Some(5)
        }));
        let _ = std::fs::remove_file(root.join("note.txt"));
        let _ = std::fs::remove_dir(root.join("Bunny Girl"));
        let _ = std::fs::remove_dir(root);
    }

    #[test]
    fn grep_is_classified_as_read_only() {
        let classification = classify_shell_command("ls ~/Downloads | grep -F \"Bunny Girl\"");

        assert_eq!(classification.mode.as_str(), "readOnly");
    }

    #[test]
    fn test_resolve_agent_config() {
        let mut config = MintConfig::default();
        config.enable_agent_collaboration = true;
        config.agents = vec![
            AgentConfig {
                id: "planner".into(),
                name: "Planner".into(),
                provider: "openai".into(),
                model: "gpt-4o".into(),
                api_key: Some("test-key".into()),
                system_instruction: "Planner instruction".into(),
                enabled: true,
            },
            AgentConfig {
                id: "coder".into(),
                name: "Coder".into(),
                provider: "gemini".into(),
                model: "gemini-2.5-flash".into(),
                api_key: None,
                system_instruction: "Coder instruction".into(),
                enabled: true,
            },
        ];

        let (cfg, instr, name, model) = resolve_agent_config(&config, Some("planner"), &[]);
        assert_eq!(cfg.ai_provider, "openai");
        assert_eq!(cfg.openai_model, "gpt-4o");
        assert_eq!(cfg.openai_api_key, "test-key");
        assert_eq!(instr, "Planner instruction");
        assert_eq!(name, Some("Planner".to_string()));
        assert_eq!(model, Some("gpt-4o".to_string()));

        let (cfg, instr, name, _model) = resolve_agent_config(&config, None, &[]);
        assert_eq!(cfg.ai_provider, "openai");
        assert_eq!(instr, "Planner instruction");
        assert_eq!(name, Some("Planner".to_string()));

        let trajectory =
            vec!["Step 1:\n- Action: create_plan\n- Observation: all planned".to_string()];
        let (cfg, instr, name, model) = resolve_agent_config(&config, None, &trajectory);
        assert_eq!(cfg.ai_provider, "gemini");
        assert_eq!(instr, "Coder instruction");
        assert_eq!(name, Some("Coder".to_string()));
        assert_eq!(model, Some("gemini-2.5-flash".to_string()));
    }

    fn step_pair(index: usize) -> [ChatMessage; 2] {
        [
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: format!("call_{index}"),
                    name: "read_file".into(),
                    input: serde_json::json!({ "path": format!("file{index}.rs") }),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: format!("call_{index}"),
                    content: format!("contents of file{index}.rs"),
                    is_error: false,
                }],
            },
        ]
    }

    fn native_messages_with_steps(step_count: usize) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::text(ChatRole::User, "do the task")];
        for i in 0..step_count {
            messages.extend(step_pair(i));
        }
        messages
    }

    #[tokio::test]
    async fn compact_native_messages_is_a_noop_when_history_is_short() {
        // COMPACTION_KEEP_RECENT_STEPS = 3, so exactly 3 step-pairs is nothing to compact yet.
        let messages = native_messages_with_steps(3);
        let config = MintConfig::default();
        let result = compact_native_messages(&config, &messages).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn compact_native_messages_is_a_noop_for_empty_history() {
        let config = MintConfig::default();
        let result = compact_native_messages(&config, &[]).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn render_messages_as_text_includes_tool_calls_and_results() {
        let messages = native_messages_with_steps(1);
        let rendered = render_messages_as_text(&messages);
        assert!(rendered.contains("Called read_file with"));
        assert!(rendered.contains("Result: contents of file0.rs"));
        assert!(rendered.contains("do the task"));
    }

    #[test]
    fn no_modification_means_finish_never_needs_verification() {
        // A pure Q&A run that never touched apply_patch/write_file can finish
        // immediately, regardless of what (if anything) is in `verification`.
        assert!(!unverified_modification(None, None, ""));
        assert!(!unverified_modification(None, Some(1), ""));
    }

    #[test]
    fn modification_without_any_verify_call_blocks_finish() {
        assert!(unverified_modification(Some(2), None, ""));
    }

    #[test]
    fn verify_called_before_the_modification_does_not_count() {
        // Editing again after the last verify invalidates that earlier check.
        assert!(unverified_modification(Some(3), Some(1), ""));
    }

    #[test]
    fn verify_called_after_the_modification_satisfies_the_gate() {
        assert!(!unverified_modification(Some(2), Some(3), ""));
    }

    #[test]
    fn verify_on_the_same_step_as_the_modification_satisfies_the_gate() {
        // Both markers can land on the same step if a single turn issues
        // multiple tool calls (apply_patch then verify back to back).
        assert!(!unverified_modification(Some(2), Some(2), ""));
    }

    #[test]
    fn explicit_written_justification_satisfies_the_gate_without_a_verify_call() {
        assert!(!unverified_modification(
            Some(2),
            None,
            "Documentation-only change; no test suite in this repo."
        ));
    }

    #[test]
    fn placeholder_verification_text_does_not_satisfy_the_gate() {
        // Mirrors `meaningful_verification`'s own placeholder filter — "n/a" etc.
        // must not be treated as a real justification.
        assert!(unverified_modification(Some(2), None, "n/a"));
        assert!(unverified_modification(Some(2), None, "not run"));
    }

    fn decision_with_action(action: &str) -> (String, AgentDecision) {
        (
            "call_0".to_string(),
            AgentDecision {
                thought: String::new(),
                action: action.to_string(),
                input: AgentInput::default(),
            },
        )
    }

    #[test]
    fn two_or_more_subagent_only_decisions_qualify_for_the_parallel_batch() {
        let decisions = vec![
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
        ];
        assert!(decisions_are_parallel_subagent_batch(&decisions));

        let decisions = vec![
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
        ];
        assert!(decisions_are_parallel_subagent_batch(&decisions));
    }

    #[test]
    fn a_lone_subagent_dispatch_stays_sequential() {
        let decisions = vec![decision_with_action("dispatch_subagent")];
        assert!(!decisions_are_parallel_subagent_batch(&decisions));
    }

    #[test]
    fn subagent_dispatch_mixed_with_another_action_stays_sequential() {
        let decisions = vec![
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
            decision_with_action("read_file"),
        ];
        assert!(!decisions_are_parallel_subagent_batch(&decisions));
    }

    #[test]
    fn empty_decisions_never_qualify_for_the_parallel_batch() {
        assert!(!decisions_are_parallel_subagent_batch(&[]));
    }
}

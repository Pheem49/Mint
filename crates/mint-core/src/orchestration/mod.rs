use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;

mod mcp_review;
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

struct TurnStartListener {
    chat_id: String,
    callback: Arc<dyn Fn(i64) + Send + Sync>,
}

tokio::task_local! {
    static TURN_START_LISTENER: TurnStartListener;
}

/// Reports the persisted turn ID to the initiating transport without changing
/// the agent/chat interfaces used by CLI, jobs, and background integrations.
pub async fn with_turn_start_listener<F, R>(
    chat_id: String,
    callback: impl Fn(i64) + Send + Sync + 'static,
    future: F,
) -> R
where
    F: Future<Output = R>,
{
    TURN_START_LISTENER
        .scope(
            TurnStartListener {
                chat_id,
                callback: Arc::new(callback),
            },
            future,
        )
        .await
}

/// Owns one persisted turn, including its cross-process queue lease. Dropping a
/// cancelled task marks its prompt interrupted instead of leaving it running.
struct TurnLease {
    memory: MemoryStore,
    id: i64,
    heartbeat: tokio::task::JoinHandle<()>,
    finished: bool,
    activity: Arc<std::sync::Mutex<Vec<AgentProgress>>>,
}

impl TurnLease {
    async fn start(memory: &MemoryStore, chat_id: &str, text: &str) -> Result<Self, MemoryError> {
        let id = memory.start_turn(chat_id, text)?;
        let _ = TURN_START_LISTENER.try_with(|listener| {
            if listener.chat_id == chat_id {
                (listener.callback)(id);
            }
        });
        let heartbeat_memory = memory.clone();
        let heartbeat = tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(10)).await;
                if heartbeat_memory.renew_turn(id).is_err() {
                    break;
                }
            }
        });
        let mut lease = Self {
            memory: memory.clone(),
            id,
            heartbeat,
            finished: false,
            activity: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        loop {
            match memory.claim_turn(chat_id, id) {
                Ok(true) => return Ok(lease),
                Ok(false) => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
                Err(error) => {
                    lease.fail();
                    return Err(error);
                }
            }
        }
    }

    fn persist_activity(&self) {
        if let Ok(lock) = self.activity.lock() {
            let compacted = compact_agent_progress(&lock);
            if !compacted.is_empty() {
                if let Ok(activity_json) = serde_json::to_string(&compacted) {
                    let _ = self
                        .memory
                        .set_interaction_agent_activity_json(self.id, &activity_json);
                }
            }
        }
    }

    fn complete_with_summary(
        &mut self,
        summary: &str,
        provider: &str,
        model: &str,
        fallback_provider: Option<&str>,
    ) -> Result<(), MemoryError> {
        self.persist_activity();
        self.memory
            .finish_turn(self.id, summary, provider, model, fallback_provider)?;
        self.finished = true;
        Ok(())
    }

    fn complete(&mut self, response: &ChatResponse) -> Result<(), MemoryError> {
        self.complete_with_summary(
            &response.text,
            &response.provider,
            &response.model,
            response.fallback_provider.as_deref(),
        )
    }

    fn fail(&mut self) {
        self.persist_activity();
        let _ = self.memory.end_turn(self.id, "failed");
        self.finished = true;
    }

    fn interrupt(&mut self) {
        self.persist_activity();
        let _ = self.memory.end_turn(self.id, "interrupted");
        self.finished = true;
    }
}

impl Drop for TurnLease {
    fn drop(&mut self) {
        self.heartbeat.abort();
        if !self.finished {
            self.persist_activity();
            let _ = self.memory.end_turn(self.id, "interrupted");
        }
    }
}

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
    let response = match stream_chat(config, &enriched, on_chunk).await {
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
    let (response, fallback) = match stream_chat_with_fallback(config, &enriched, on_chunk).await {
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

// Full skill reads must reach the model exactly; other tools keep bounded output.
fn model_tool_result(action: &str, result: &str) -> String {
    if action.starts_with("browser_")
        || (action == "read_file" && result.starts_with("[Loaded skill: "))
    {
        result.to_owned()
    } else {
        truncate(result)
    }
}

fn tool_progress_input(root: &Path, decision: &AgentDecision) -> Value {
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
        memory.recent_completed_interactions_for_chat(&request_chat_id(request), CONTEXT_LIMIT)?;
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
// A tool gets its own correction budget, renewed after successful execution.
const MAX_TOOL_ARGUMENT_RETRIES: usize = 5;
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

/// The `answer` string a surface returns from an [`AgentApproval::McpTool`]
/// prompt to mean "run this call **and** persist `allowedMcpTools[server] =
/// ["*"]`, so this server never prompts again". Travels through the normal
/// `Intercepted(answer)` channel. The CLI approval picker
/// (`crates/mint-cli/src/agent.rs`) and the web/desktop `ApprovalCard`
/// (`src/renderer/shared/components/ApprovalCard.tsx`) send this exact string.
pub const MCP_ALLOW_ALL_SENTINEL: &str = "__mcp_allow_all__";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentProgress {
    ToolDiscovery {
        server: String,
        tool: String,
        message: String,
    },
    DeviceState {
        #[serde(rename = "callId")]
        call_id: String,
        server: String,
        tool: String,
        update: crate::mcp::device::DeviceUpdate,
    },
    ToolProgress {
        #[serde(rename = "callId")]
        call_id: String,
        server: String,
        tool: String,
        update: crate::mcp::McpProgress,
    },
    ToolArtifacts {
        #[serde(rename = "callId")]
        call_id: String,
        artifacts: Vec<crate::integrations::mcp_result::McpArtifact>,
        warnings: Vec<String>,
    },
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
    /// Incremental model reasoning for the currently active agent step. These
    /// events are live-only; consumers replace them with the matching final
    /// `ExtendedThinking` record instead of persisting every delta.
    ThinkingDelta {
        id: String,
        delta: String,
        elapsed_ms: u64,
    },
    /// Extended reasoning / chain-of-thought from the model's API thinking
    /// tokens (e.g. Claude `<thinking>`, Gemini Thinking, DeepSeek Reasoner,
    /// or `<think>` tags). Semantically distinct from `Thought`, which
    /// carries short, one-line agent notes (fallback warnings, decision
    /// summaries). The CLI renders this as a collapsible block; the web/
    /// desktop UI shows it in a dedicated "Extended Thinking" panel.
    ExtendedThinking {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        id: Option<String>,
        thought: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        elapsed_ms: Option<u64>,
    },
    /// Lifecycle of native context summarization, including actionable failures.
    ContextCompaction {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent: Option<String>,
        status: String,
        message: String,
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
        #[serde(default, rename = "callId", skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
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
        #[serde(default, rename = "callId", skip_serializing_if = "Option::is_none")]
        call_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        status: Option<crate::integrations::mcp_result::ToolStatus>,
        action: String,
        input: Value,
        result: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        subagent: Option<String>,
    },
    PlanUpdated {
        plan: ActivePlan,
    },
    RunCompleted {
        summary: RunTelemetrySummary,
    },
}

/// Determines whether a thought string represents long-form / internal chain-of-thought
/// (reasoning tokens, <think> tags, or multi-line reasoning) versus a brief agent note.
pub fn is_internal_cot(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.starts_with("<think>") || trimmed.contains("</think>") {
        return true;
    }
    let line_count = trimmed.lines().count();
    let char_count = trimmed.chars().count();
    if trimmed.contains("\n\n") || line_count > 2 || char_count > 180 {
        return true;
    }
    let lower = trimmed.to_lowercase();
    if lower.starts_with("the user ")
        || lower.starts_with("let's think")
        || lower.starts_with("let me analyze")
        || lower.starts_with("per rule")
        || lower.starts_with("note the ")
        || lower.contains("thinking process:")
    {
        return true;
    }
    false
}

/// Compacts an agent progress event stream for storage, removing transport-only
/// streaming deltas (`ThinkingDelta`) while preserving complete `ExtendedThinking`
/// blocks, or synthesizing an `ExtendedThinking` block for any interrupted delta sequence.
pub fn compact_agent_progress(progress: &[AgentProgress]) -> Vec<AgentProgress> {
    let completed_ids: std::collections::HashSet<String> = progress
        .iter()
        .filter_map(|event| match event {
            AgentProgress::ExtendedThinking { id: Some(id), .. } => Some(id.clone()),
            _ => None,
        })
        .collect();

    let mut result = Vec::new();
    let mut interrupted_order: Vec<String> = Vec::new();
    let mut interrupted: std::collections::HashMap<String, (String, Option<u64>)> =
        std::collections::HashMap::new();

    let latest_progress: std::collections::HashMap<(u8, &str), usize> = progress
        .iter()
        .enumerate()
        .filter_map(|(i, event)| match event {
            AgentProgress::ToolProgress { call_id, .. } => Some(((0, call_id.as_str()), i)),
            AgentProgress::DeviceState {
                call_id, update, ..
            } if !matches!(update.state, crate::mcp::device::DevicePhase::Accepted) => {
                Some(((1, call_id.as_str()), i))
            }
            _ => None,
        })
        .collect();
    for (index, event) in progress.iter().enumerate() {
        let key = match event {
            AgentProgress::ToolProgress { call_id, .. } => Some((0, call_id.as_str())),
            AgentProgress::DeviceState {
                call_id, update, ..
            } if !matches!(update.state, crate::mcp::device::DevicePhase::Accepted) => {
                Some((1, call_id.as_str()))
            }
            _ => None,
        };
        if key.is_some_and(|key| latest_progress.get(&key) != Some(&index)) {
            continue;
        }
        match event {
            AgentProgress::ThinkingDelta {
                id,
                delta,
                elapsed_ms,
            } => {
                if !completed_ids.contains(id) {
                    if !interrupted.contains_key(id) {
                        interrupted_order.push(id.clone());
                    }
                    interrupted
                        .entry(id.clone())
                        .and_modify(|(text, elapsed)| {
                            text.push_str(delta);
                            *elapsed = Some(*elapsed_ms);
                        })
                        .or_insert_with(|| (delta.clone(), Some(*elapsed_ms)));
                }
            }
            _ => {
                result.push(event.clone());
            }
        }
    }

    for id in interrupted_order {
        if let Some((thought, elapsed_ms)) = interrupted.remove(&id) {
            result.push(AgentProgress::ExtendedThinking {
                id: Some(id),
                thought,
                elapsed_ms,
            });
        }
    }

    result
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ToolExecutionRecord {
    pub step: usize,
    pub action: String,
    pub target: String,
    pub success: bool,
    pub retried: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RunTelemetrySummary {
    pub run_id: String,
    pub task: String,
    pub outcome: String, // "SUCCESS" | "FAILED" | "ROLLED_BACK"
    pub total_tokens: usize,
    pub tool_calls_count: usize,
    pub files_changed: Vec<String>,
    #[serde(default)]
    pub files_created: Vec<String>,
    pub duration_secs: f64,
    pub tool_timeline: Vec<ToolExecutionRecord>,
    pub retries_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanTaskItem {
    pub id: String,
    pub title: String,
    pub status: String, // "pending", "in_progress", "completed", "failed"
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct ActivePlan {
    pub objective: String,
    pub tasks: Vec<PlanTaskItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResult {
    pub provider: String,
    pub model: String,
    pub summary: String,
    pub verification: String,
    pub fallback: Option<String>,
    #[serde(default)]
    pub fallback_reason: Option<String>,
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
    #[serde(
        default,
        alias = "q",
        alias = "keyword",
        alias = "search",
        alias = "searchTerm",
        alias = "search_term"
    )]
    query: String,
    #[serde(default, alias = "ticker")]
    symbol: String,
    #[serde(default)]
    filter: String,
    #[serde(default)]
    options: Vec<AskUserOptionInput>,
    #[serde(default)]
    header: String,
    #[serde(default, alias = "multi_select")]
    multi_select: bool,
    #[serde(default, alias = "location", alias = "place")]
    city: String,
    #[serde(default, alias = "expr", alias = "math")]
    expression: String,
    #[serde(default, alias = "cmd")]
    command: String,
    #[serde(default)]
    background: bool,
    #[serde(default, alias = "job_id")]
    job_id: String,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    steps: Vec<String>,
    #[serde(default, alias = "file_content")]
    file_content: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    verification: String,
    #[serde(default)]
    plan: String,
    #[serde(default)]
    reason: String,
    #[serde(default, alias = "start_line")]
    start_line: Option<usize>,
    #[serde(default, alias = "end_line")]
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
    #[serde(default, alias = "note_path")]
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
    step: Option<usize>,
    #[serde(default)]
    kind: String,
    #[serde(default)]
    content: String,
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
    #[serde(default)]
    element_ref: String,
    #[serde(default)]
    tab_id: String,
    #[serde(default)]
    operation: String,
    #[serde(default)]
    condition: String,
    #[serde(default)]
    value: String,
    #[serde(default)]
    timeout_ms: Option<u64>,
    #[serde(default)]
    text_offset: Option<u64>,
    #[serde(default)]
    element_offset: Option<u64>,
    #[serde(default)]
    takeover_complete: bool,
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
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(decode_agent_input)
        .transpose()
        .map_err(serde::de::Error::custom)
        .map(|input| input.unwrap_or_default())
}

// Some providers encode the structured patch twice. Decode only that field;
// retain strict typing for all other arguments and never infer a target file.
fn decode_agent_input(mut value: Value) -> Result<AgentInput, String> {
    let outer_path = value.get("path").and_then(Value::as_str).map(str::to_owned);
    if let Some(patch) = value.get_mut("patch").filter(|patch| !patch.is_null()) {
        if let Value::String(encoded) = patch {
            *patch = serde_json::from_str(encoded).map_err(|_| {
                "patch must contain a valid JSON object with path and hunks".to_owned()
            })?;
        }
        let object = patch
            .as_object_mut()
            .ok_or_else(|| "patch must be an object with path and hunks".to_owned())?;
        if !object.contains_key("path") {
            if let Some(path) = &outer_path {
                object.insert("path".into(), Value::String(path.clone()));
            }
        }
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| {
                "patch.path is required; supply the target file explicitly".to_owned()
            })?;
        if outer_path
            .as_deref()
            .is_some_and(|outer| !outer.is_empty() && outer != path)
        {
            return Err("path and patch.path must name the same target file".into());
        }
        serde_json::from_value::<AgentPatch>(patch.clone())
            .map_err(|_| "patch requires a string path and hunks containing string oldText/newText and optional boolean replaceAll".to_owned())?;
    }
    serde_json::from_value(value).map_err(|error| {
        let message = error.to_string();
        if message.chars().count() > 400 {
            format!(
                "{}… (argument value omitted)",
                message.chars().take(400).collect::<String>()
            )
        } else {
            message
        }
    })
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
            let mut progress = move |event: AgentProgress| {
                if let Ok(mut lock) = record_activity.lock() {
                    lock.push(event.clone());
                }
                progress(event);
            };
            let resolved_task = resolve_github_links(task, config).await;
            let agent_result = crate::browser::run_session(async {
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
        let mut system_prompt = build_system_prompt(
            config,
            plan_mode,
            system_prompt_native,
            user_name,
            pinned_mcp_server,
        );
        let hooks = crate::hooks::list_hooks(config);

        append_memory_context(
            &mut system_prompt,
            chat_id,
            Some(root.to_string_lossy().as_ref()),
            Some(&resolved_task),
            config.semantic_fact_recall,
        );
        if config.memory_recall
            && let Some(recall) = render_recalled_messages(chat_id, &resolved_task)
        {
            system_prompt = format!("{}\n\n{}", system_prompt.trim(), recall);
        }
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
        let mut final_fallback_reason = None;
        let mut action_counts = BTreeMap::<String, usize>::new();
        let mut invalid_tool_calls = ToolArgumentRetries::default();
        let mut repeated_failures = RepeatedFailures::default();
        let mut streamed_finish_summary = String::new();
        let mut streamed_direct_text = String::new();
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
        let mut executed_tools: Vec<ToolExecutionRecord> = Vec::new();
        let mut mcp_review = mcp_review::McpReview::default();
        let mut files_modified: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();
        let mut files_created: std::collections::BTreeSet<String> =
            std::collections::BTreeSet::new();

        'steps: for step in 1..=MAX_STEPS {
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

            let tool_mode = active_config.tool_calling_mode();
            // A finish can still be rejected after the provider finishes
            // streaming when edited files have not been verified. Don't show
            // that provisional summary to the user as if the run were done.
            let allow_final_stream =
                !unverified_modification(last_modify_step, last_verify_step, "")
                    && last_verify_failed != Some(true);

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
                            progress(AgentProgress::ToolEnd { call_id: None, status: None, action: action.clone(), input: input.clone(), result: error.clone(), subagent: None });
                            trajectory.push(format_trajectory_step(step, "", action, error));
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

            if decisions_are_parallel_subagent_batch(&decisions) {
                let completed = run_parallel_subagent_batch(
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
                invalid_tool_calls.observe_completed(&completed);
                step_tool_results.extend(completed);
            } else if decisions_are_parallel_read_only_batch(&decisions) {
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
                        let mut summary = decision.input.summary.trim().to_owned();
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
                        if crate::browser::session_finish_evidence().await && meaningful_verification(&decision.input.verification).is_empty() {
                            let error = "Browser tasks require observed evidence in finish.verification. Describe the observed outcome, or explicitly state that completion is unverified or blocked.";
                            trajectory.push(format_trajectory_step(step, &decision.thought, &decision.action, error));
                            rebuild_observation(task, &root, &trajectory, &mut observation);
                            reject_native_finish(tool_mode, &mut native_messages, &response.text, error);
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
                        if config.auto_skill_writing && looks_skill_worthy(step, &action_counts) {
                            spawn_auto_skill_write(
                                config.clone(),
                                task.to_string(),
                                summary.clone(),
                                root.clone(),
                                skills.clone(),
                            );
                        }
                        let run_summary = RunTelemetrySummary {
                            run_id: format!("run-{}", chrono::Local::now().format("%Y%m%d%H%M%S")),
                            task: task.to_string(),
                            outcome: "SUCCESS".to_string(),
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
                            verification,
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
                    // stays false for plan-mode/hook blocks and the duplicate-shell skip, since
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
                            tool_progress_input(&root, &decision);
                        progress(AgentProgress::ToolStart { call_id: Some(activity_call_id.clone()),
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

                                let (tool_result, success) = match execute_tool_outcome_reviewed(
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
                                        if matches!(decision.action.as_str(),"mcp_tool"|"mcp_list_tools") { outcome_status = Some(outcome.status); }
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

                    if outcome_status.is_none() && matches!(decision.action.as_str(), "mcp_tool" | "mcp_list_tools") {
                        outcome_status = Some(if action_succeeded {
                            crate::integrations::mcp_result::ToolStatus::Success
                        } else {
                            crate::integrations::mcp_result::ToolStatus::Failed
                        });
                    }
                    if run_control::cancelled() {outcome_status = Some(crate::mcp_result::ToolStatus::Cancelled);}
                    progress(AgentProgress::ToolEnd { call_id: Some(activity_call_id), status: outcome_status,
                        action: decision.action.clone(),
                        input: tool_progress_input(&root, &decision),
                        result: result.clone(),
                        subagent: None,
                    });

                    let success = action_succeeded && !shell_result_failed(&result);
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
                                last_modify_step = Some(step);
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
                        step_images.insert(call_id.clone(), vec![result.clone()]);
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
                        model_tool_result(&decision.action, &result)
                    };
                    if matches!(
                        decision.action.as_str(),
                        "run_shell" | "verify" | "run_tests" | "run_typecheck" | "run_linter"
                    ) {
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
                    if action_count >= 3 && !decision.action.starts_with("browser_") {
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

                    step_tool_results.push(ToolResultEntry {
                        call_id, action:decision.action.clone(), input:tool_progress_input(&root,&decision),
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

                if let Some(total_tokens) = response.total_tokens {
                    let window = active_config.context_window_tokens();
                    last_context_pct = Some(
                        ((total_tokens as f64 / window as f64) * 100.0).clamp(0.0, 255.0) as u8,
                    );
                    if (total_tokens as f64) >= (window as f64) * COMPACTION_TRIGGER_RATIO {
                        let eligible = native_messages.len().saturating_sub(1) / 2 > COMPACTION_KEEP_RECENT_STEPS;
                        if let Ok(Some((compacted, _))) = run_control::run(report_context_compaction(
                            eligible,
                            total_tokens,
                            window,
                            compact_native_messages(&active_config, &native_messages),
                            &mut progress,
                        )).await {
                            native_messages = compacted;
                            // Wait for actual usage from the next request, rather than
                            // displaying the pre-compaction percentage as current.
                            last_context_pct = None;
                        }
                    }
                }
            }

            rebuild_observation(task, &root, &trajectory, &mut observation);
        }

        let run_summary = RunTelemetrySummary {
            run_id: format!("run-{}", chrono::Local::now().format("%Y%m%d%H%M%S")),
            task: task.to_string(),
            outcome: "FAILED".to_string(),
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
        };
        progress(AgentProgress::RunCompleted {
            summary: run_summary,
        });

        turn.fail();
        Err(OrchestrationError::Agent(format!(
            "code agent reached the limit of {} steps; task incomplete. Last browser state: {}",
            MAX_STEPS, crate::browser::session_last_state().await
        )))
        }).await;
            if agent_result.is_err() && !turn.finished {
                if run_control::cancelled() {
                    turn.interrupt();
                } else {
                    turn.fail();
                }
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
            AgentProgress::ToolStart {
                action,
                input,
                call_id,
                ..
            } => AgentProgress::ToolStart {
                call_id,
                action,
                input,
                subagent: Some(subagent_name.clone()),
            },
            AgentProgress::ToolEnd {
                action,
                input,
                result,
                call_id,
                status,
                ..
            } => AgentProgress::ToolEnd {
                call_id,
                status,
                action,
                input,
                result,
                subagent: Some(subagent_name.clone()),
            },
            AgentProgress::ContextCompaction {
                status, message, ..
            } => AgentProgress::ContextCompaction {
                status,
                message,
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
) -> Vec<ToolResultEntry> {
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
        let input_val = tool_progress_input(&root, &decision);
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
            call_id: None,
            action: action.clone(),
            input: input_val.clone(),
            subagent: None,
        });
        let tool_result = match result {
            Ok(text) => text,
            Err(error) => format!("Error: {}", error),
        };
        progress(AgentProgress::ToolEnd {
            call_id: None,
            status: None,
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
        let mut final_result = model_tool_result(&action, &tool_result);
        if action_count >= 3 && !action.starts_with("browser_") {
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
        step_tool_results.push((call_id, action, input_val, final_result).into());
    }
    step_tool_results
}

#[allow(clippy::too_many_arguments)]
async fn run_parallel_read_only_batch(
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
                (*guard)(AgentProgress::ToolStart { call_id: None,
                    action: action.clone(),
                    input: input_val.clone(),
                    subagent: None,
                });
            }

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

                        let (tool_result, success) = match execute_tool(
                            root,
                            config,
                            &decision,
                            chat_id,
                            &mut approve_adapter,
                            &mut progress_adapter,
                        )
                        .await
                        {
                            Ok(res) => (res, true),
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
                (*guard)(AgentProgress::ToolEnd { call_id: None, status: None,
                    action: action.clone(),
                    input: input_val.clone(),
                    result: result.clone(),
                    subagent: None,
                });
            }

            (
                index, call_id, thought, action, input_val, action_key, result,
            )
        });
    }

    let mut results = futures_util::stream::iter(tasks)
        .buffer_unordered(PARALLEL_READ_ONLY_LIMIT)
        .collect::<Vec<_>>()
        .await;

    results.sort_by_key(|(index, ..)| *index);

    let mut step_tool_results = Vec::with_capacity(results.len());
    for (_, call_id, thought, action, input_val, action_key, result) in results {
        let action_count = {
            let count = action_counts.entry(action_key).or_insert(0);
            *count += 1;
            *count
        };

        let mut final_result = if result.starts_with("data:image/")
            && matches!(
                action.as_str(),
                "browser_screenshot" | "video_filmstrip" | "video_waveform" | "view_image"
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
            model_tool_result(&action, &result)
        };

        if action_count >= 3 && !action.starts_with("browser_") {
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

        step_tool_results.push((call_id, action, input_val, final_result).into());
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

async fn execute_tool_outcome(
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
async fn execute_tool_outcome_reviewed(
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
    if matches!(decision.action.as_str(), "mcp_tool" | "mcp_list_tools") {
        let mut out = tools::plugins_mcp::execute_reviewed(
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
        execute_tool_text(root, config, decision, chat_id, approve, progress)
            .await
            .map(Into::into)
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
        "search_code" | "symbols" | "find_definition" | "find_references" | "repo_map"
        | "semantic_index" | "semantic_search" => {
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
        "conversation_summary" => Ok(serde_json::json!({
            "status": "acknowledged",
            "message": "Previous steps summary is already recorded in context."
        })
        .to_string()),
        "browser_open"
        | "browser_click"
        | "browser_type"
        | "browser_read"
        | "browser_mouse_move"
        | "browser_mouse_click"
        | "browser_key_press"
        | "browser_screenshot"
        | "browser_tabs"
        | "browser_observe"
        | "browser_fill"
        | "browser_select"
        | "browser_scroll"
        | "browser_wait" => {
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
        "git_status" | "git_diff" | "git_log" | "git_branch" | "git_checkpoint"
        | "git_rollback" | "git_restore_file" | "git_commit" | "git_create_branch" => {
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
                progress,
            )
            .await
        }
        "detect_project" | "list_tests" | "read_diagnostics" | "view_image" | "search_docs"
        | "create_project_doc" => {
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
        "run_shell" | "shell_output" | "kill_shell" | "verify" | "run_tests" | "run_typecheck"
        | "run_linter" => {
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
        input: parse_tool_input(action, input)?,
    };
    // No progress channel from the realtime voice session — tool-call
    // activity isn't surfaced there the way it is in the CLI/GUI.
    execute_tool(root, config, &decision, chat_id, approve_cb, &mut |_| {}).await
}

fn parse_tool_input(action: &str, input: Value) -> Result<AgentInput, OrchestrationError> {
    decode_agent_input(input).map_err(|error| {
        OrchestrationError::Agent(format!("invalid arguments for tool '{action}': {error}"))
    })
}

fn append_native_tool_results(
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
            is_error: *status != crate::mcp_result::ToolStatus::Success,
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

#[derive(Debug, Clone)]
struct ToolResultEntry {
    call_id: String,
    action: String,
    input: Value,
    text: String,
    status: crate::mcp_result::ToolStatus,
}

// Legacy tools determine their outcome once at this adapter; MCP supplies its
// explicit status directly, regardless of the wording of server text.
impl From<(String, String, Value, String)> for ToolResultEntry {
    fn from((call_id, action, input, text): (String, String, Value, String)) -> Self {
        let mut status = crate::mcp_result::ToolOutcome::from(text.clone()).status;
        if text.starts_with("Skipped")
            || (matches!(
                action.as_str(),
                "run_shell" | "verify" | "run_tests" | "run_typecheck" | "run_linter"
            ) && shell_result_failed(&text))
        {
            status = crate::mcp_result::ToolStatus::Failed;
        }
        Self {
            call_id,
            action,
            input,
            text,
            status,
        }
    }
}
type PreparedToolCalls = (Vec<(String, AgentDecision)>, Vec<ToolResultEntry>);

#[derive(Debug, Default)]
struct ToolArgumentRetries {
    failures: BTreeMap<String, usize>,
}

impl ToolArgumentRetries {
    fn succeeded(&mut self, tool: &str) {
        self.failures.remove(tool);
    }

    fn observe_completed(&mut self, results: &[ToolResultEntry]) {
        for ToolResultEntry {
            action: tool,
            status,
            ..
        } in results
        {
            // Parallel batches encode execution errors and hook blocks in
            // their results. Neither is a successful execution.
            if *status == crate::mcp_result::ToolStatus::Success {
                self.succeeded(tool);
            }
        }
    }
}

/// Invalid calls never become executable decisions. Return their original
/// arguments and call IDs so the next model turn can repair them.
fn prepare_native_tool_calls(
    calls: Vec<crate::chat::ToolCall>,
    thought: &str,
    retries: &mut ToolArgumentRetries,
) -> Result<PreparedToolCalls, OrchestrationError> {
    let mut decisions = Vec::new();
    let mut errors = Vec::new();
    for (index, call) in calls.into_iter().enumerate() {
        match parse_tool_input(&call.name, call.input.clone()) {
            Ok(input) => decisions.push((
                call.id,
                AgentDecision {
                    thought: if index == 0 {
                        thought.to_owned()
                    } else {
                        String::new()
                    },
                    action: call.name,
                    input,
                },
            )),
            Err(error) => {
                let invalid_count = retries.failures.entry(call.name.clone()).or_default();
                if *invalid_count >= MAX_TOOL_ARGUMENT_RETRIES {
                    return Err(OrchestrationError::Agent(format!(
                        "Tool '{}' exhausted its {MAX_TOOL_ARGUMENT_RETRIES} argument-correction retries without a successful execution. Other tools have independent budgets. {error}",
                        call.name
                    )));
                }
                *invalid_count += 1;
                errors.push((call.id, call.name, call.input, format!(
                    "Error: {error}. No tool was executed for this call. Correct the arguments to match the tool schema, or choose another suitable tool. Correction retries for this tool: {}/{MAX_TOOL_ARGUMENT_RETRIES}. This counter resets only after this tool executes successfully; changing arguments alone does not reset it. Other tools have independent counters. Continued failures for this tool will stop the run.",
                    *invalid_count,
                )).into());
            }
        }
    }
    Ok((decisions, errors))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;

    #[test]
    fn stringified_patch_uses_explicit_outer_target() {
        let patch =
            serde_json::json!({"hunks": [{"oldText": "old", "newText": "new"}]}).to_string();
        let input = parse_tool_input(
            "apply_patch",
            serde_json::json!({"path": "style.css", "patch": patch}),
        )
        .unwrap();
        let patch = input.patch.unwrap();
        assert_eq!(patch.path, PathBuf::from("style.css"));
        assert_eq!(patch.hunks.len(), 1);
    }

    #[tokio::test]
    async fn stringified_patch_executes_only_after_approval_and_can_be_undone() {
        let root = std::env::temp_dir().join(format!("mint-patch-repro-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("style.css"), "body { color: black; }").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };
        let input = serde_json::json!({"path": "style.css", "patch": serde_json::json!({"hunks": [{"oldText": "black", "newText": "orange"}]}).to_string()});
        execute_tool_from_json(
            &root,
            &config,
            "apply_patch",
            input.clone(),
            "patch-repro",
            &mut |_| Ok(ApprovalOutcome::Denied),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("style.css")).unwrap(),
            "body { color: black; }"
        );
        execute_tool_from_json(
            &root,
            &config,
            "apply_patch",
            input,
            "patch-repro",
            &mut |_| Ok(ApprovalOutcome::Approved),
        )
        .await
        .unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("style.css")).unwrap(),
            "body { color: orange; }"
        );
        crate::system::workspace_history::undo(&root, None).unwrap();
        assert_eq!(
            std::fs::read_to_string(root.join("style.css")).unwrap(),
            "body { color: black; }"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn patch_decoding_preserves_strict_types_and_rejects_ambiguous_targets() {
        let patch = serde_json::json!({"path": "style.css", "hunks": [{"oldText": "old", "newText": "new"}]});
        assert!(parse_tool_input("apply_patch", serde_json::json!({"patch": patch})).is_ok());
        assert!(
            parse_tool_input(
                "apply_patch",
                serde_json::json!({"path": "other.css", "patch": patch.to_string()})
            )
            .is_err()
        );
        assert!(
            parse_tool_input(
                "apply_patch",
                serde_json::json!({"patch": "{\"hunks\": []}"})
            )
            .is_err()
        );
        assert!(parse_tool_input("apply_patch", serde_json::json!({"patch": "not JSON"})).is_err());
        assert!(parse_tool_input("list_files", serde_json::json!({"limit": "100"})).is_err());
        let huge = "private-code-marker".repeat(1000);
        let error = parse_tool_input(
            "apply_patch",
            serde_json::json!({"patch": {"path": "style.css", "hunks": huge}}),
        )
        .unwrap_err()
        .to_string();
        assert!(error.len() < 600);
        assert!(!error.contains("private-code-marker"));
        let decision: AgentDecision = serde_json::from_value(serde_json::json!({"action": "apply_patch", "input": {"path": "style.css", "patch": serde_json::json!({"hunks": [{"oldText": "old", "newText": "new"}]}).to_string()}})).unwrap();
        assert_eq!(
            decision.input.patch.unwrap().path,
            PathBuf::from("style.css")
        );
    }

    fn test_list_call(id: &str, input: Value) -> crate::chat::ToolCall {
        crate::chat::ToolCall {
            id: id.into(),
            name: "list_files".into(),
            input,
            thought_signature: Some("signature".into()),
        }
    }

    #[tokio::test]
    async fn native_argument_error_is_returned_and_corrected_call_can_execute() {
        let raw = serde_json::json!({"path": "Downloads/YOUTUBE_DL/playlist", "limit": "100"});
        let mut count = ToolArgumentRetries::default();
        let (decisions, errors) =
            prepare_native_tool_calls(vec![test_list_call("bad", raw.clone())], "", &mut count)
                .unwrap();
        assert!(decisions.is_empty());
        assert_eq!(errors.len(), 1);
        let mut messages = Vec::new();
        let signatures = std::collections::HashMap::from([("bad".into(), "signature".into())]);
        append_native_tool_results(&mut messages, "", &errors, &signatures, &Default::default());
        assert!(
            matches!(&messages[0].content[0], ContentBlock::ToolUse { id, input, thought_signature, .. }
            if id == "bad" && input == &raw && thought_signature.as_deref() == Some("signature"))
        );
        assert!(
            matches!(&messages[1].content[0], ContentBlock::ToolResult { tool_use_id, content, is_error }
            if tool_use_id == "bad" && *is_error && content.contains("Correct the arguments"))
        );

        let root =
            std::env::temp_dir().join(format!("mint-tool-recovery-{}", uuid::Uuid::new_v4()));
        let nested = root.join("Downloads/YOUTUBE_DL/playlist");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("song.txt"), "song").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };
        let (decisions, errors) = prepare_native_tool_calls(
            vec![test_list_call(
                "fixed",
                serde_json::json!({"path": "Downloads/YOUTUBE_DL/playlist", "limit": 100}),
            )],
            "",
            &mut count,
        )
        .unwrap();
        assert!(errors.is_empty());
        let output = execute_tool(
            &root,
            &config,
            &decisions[0].1,
            "regression",
            &mut |_| panic!("listing must not request approval"),
            &mut |_| {},
        )
        .await
        .unwrap();
        let entries: Vec<Value> = serde_json::from_str(&output).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(
            entries[0]["path"],
            serde_json::json!(nested.join("song.txt"))
        );
        // Parsing the repaired call alone must not renew its budget.
        assert_eq!(count.failures["list_files"], 1);
        count.observe_completed(&[
            ("fixed".into(), "list_files".into(), Value::Null, output).into()
        ]);
        assert!(!count.failures.contains_key("list_files"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn native_invalid_calls_are_bounded_without_blocking_valid_alternatives() {
        let mut count = ToolArgumentRetries::default();
        for _ in 0..MAX_TOOL_ARGUMENT_RETRIES {
            let (decisions, errors) = prepare_native_tool_calls(
                vec![test_list_call(
                    "bad",
                    serde_json::json!({"path": "~/Downloads/YOUTUBE_DL", "limit": "100"}),
                )],
                "",
                &mut count,
            )
            .unwrap();
            assert!(decisions.is_empty());
            assert_eq!(errors.len(), 1);
        }
        let alternative = crate::chat::ToolCall {
            id: "alternative".into(),
            name: "run_shell".into(),
            input: serde_json::json!({"command": "ls ~/Downloads/YOUTUBE_DL"}),
            thought_signature: None,
        };
        let (decisions, errors) =
            prepare_native_tool_calls(vec![alternative], "", &mut count).unwrap();
        assert_eq!(decisions[0].1.action, "run_shell");
        assert!(errors.is_empty());
        assert_eq!(count.failures["list_files"], MAX_TOOL_ARGUMENT_RETRIES);
        let error =
            prepare_native_tool_calls(vec![test_list_call("bad", Value::Null)], "", &mut count)
                .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("exhausted its 5 argument-correction retries")
        );
        // One exhausted tool must not consume another tool's corrections.
        let other = crate::chat::ToolCall {
            id: "other-bad".into(),
            name: "read_file".into(),
            input: serde_json::json!({"path": 123}),
            thought_signature: None,
        };
        let (_, errors) = prepare_native_tool_calls(vec![other], "", &mut count).unwrap();
        assert_eq!(errors.len(), 1);
        assert_eq!(count.failures["read_file"], 1);
        for result in [
            "Error: execution failed",
            "Blocked by hook",
            "Skipped duplicate",
        ] {
            count.observe_completed(&[(
                "x".into(),
                "list_files".into(),
                Value::Null,
                result.into(),
            )
                .into()]);
            assert_eq!(count.failures["list_files"], MAX_TOOL_ARGUMENT_RETRIES);
        }
        count.succeeded("list_files");
        assert_eq!(count.failures["read_file"], 1);
        assert!(!count.failures.contains_key("list_files"));
        assert!(
            prepare_native_tool_calls(vec![test_list_call("fresh", Value::Null)], "", &mut count)
                .is_ok()
        );
    }

    #[test]
    fn native_mixed_batch_keeps_valid_calls_and_matches_each_error_to_its_id() {
        let mut count = ToolArgumentRetries::default();
        let (decisions, errors) = prepare_native_tool_calls(
            vec![
                test_list_call("bad", Value::Null),
                test_list_call("valid-1", serde_json::json!({"path": "."})),
                test_list_call("valid-2", serde_json::json!({"path": "src"})),
            ],
            "",
            &mut count,
        )
        .unwrap();
        assert_eq!(decisions.len(), 2);
        assert!(decisions_are_parallel_read_only_batch(&decisions));
        assert_eq!(errors[0].call_id, "bad");
        let mut results = errors;
        results.extend(decisions.iter().map(|(id, d)| {
            (
                id.clone(),
                d.action.clone(),
                serde_json::to_value(&d.input).unwrap(),
                "[]".into(),
            )
                .into()
        }));
        let mut messages = Vec::new();
        append_native_tool_results(
            &mut messages,
            "",
            &results,
            &Default::default(),
            &Default::default(),
        );
        assert_eq!(messages[0].content.len(), 3);
        assert_eq!(messages[1].content.len(), 3);
        assert!(
            matches!(&messages[1].content[0], ContentBlock::ToolResult { tool_use_id, is_error: true, .. } if tool_use_id == "bad")
        );
        assert!(
            matches!(&messages[1].content[1], ContentBlock::ToolResult { tool_use_id, is_error: false, .. } if tool_use_id == "valid-1")
        );
    }

    #[tokio::test]
    async fn list_files_rejects_invalid_arguments_instead_of_listing_workspace() {
        let config = MintConfig::default();
        // A nonexistent root ensures no filesystem lookup occurs for bad input.
        let root = Path::new("/nonexistent-mint-list-files-regression");
        for input in [
            serde_json::json!({"path": "~/Downloads/YOUTUBE_DL", "limit": "100"}),
            serde_json::json!({"path": "~/Downloads/YOUTUBE_DL", "limit": -1}),
            serde_json::json!({"path": null}),
            Value::String(r#"{"path":"~/Downloads/YOUTUBE_DL""#.into()),
            Value::Null,
        ] {
            let error = execute_tool_from_json(
                root,
                &config,
                "list_files",
                input,
                "regression",
                &mut |_| panic!("invalid arguments must not request approval"),
            )
            .await
            .unwrap_err();
            assert!(
                error
                    .to_string()
                    .contains("invalid arguments for tool 'list_files'")
            );
        }
    }

    #[tokio::test]
    async fn list_files_preserves_nested_paths_and_default_directory() {
        let root =
            std::env::temp_dir().join(format!("mint-list-files-paths-{}", std::process::id()));
        let nested = root.join("Downloads/YOUTUBE_DL/playlist");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("song.txt"), "song").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };
        for input in [
            serde_json::json!({"path": "Downloads/YOUTUBE_DL/playlist", "limit": 100}),
            serde_json::json!({"path": nested}),
            serde_json::json!({}),
        ] {
            let expected = if input.get("path").is_some() {
                &nested
            } else {
                &root
            };
            let output = execute_tool_from_json(
                &root,
                &config,
                "list_files",
                input,
                "regression",
                &mut |_| panic!("listing must not request approval"),
            )
            .await
            .unwrap();
            let entries: Vec<Value> = serde_json::from_str(&output).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(
                Path::new(entries[0]["path"].as_str().unwrap()).parent(),
                Some(expected.as_path())
            );
        }
        std::fs::remove_dir_all(root).unwrap();
    }

    #[tokio::test]
    async fn turn_start_listener_reports_only_the_matching_chat() {
        let memory = MemoryStore::open(std::env::temp_dir().join(format!(
            "mint-turn-start-listener-{}.sqlite",
            uuid::Uuid::new_v4()
        )));
        let reported = Arc::new(std::sync::Mutex::new(Vec::new()));
        let callback_reported = reported.clone();
        with_turn_start_listener(
            "cli::one".to_string(),
            move |id| callback_reported.lock().unwrap().push(id),
            async {
                let first = TurnLease::start(&memory, "cli::one", "hello")
                    .await
                    .unwrap();
                let other = TurnLease::start(&memory, "cli::two", "other")
                    .await
                    .unwrap();
                assert_eq!(*reported.lock().unwrap(), vec![first.id]);
                drop(first);
                drop(other);
            },
        )
        .await;
    }

    #[test]
    fn test_compact_agent_progress_cleans_deltas_and_preserves_tools() {
        let events = vec![
            AgentProgress::ToolStart {
                call_id: None,
                action: "read_file".into(),
                input: serde_json::json!({ "path": "src/main.rs" }),
                subagent: None,
            },
            AgentProgress::ThinkingDelta {
                id: "cot-1".into(),
                delta: "Reading ".into(),
                elapsed_ms: 100,
            },
            AgentProgress::ThinkingDelta {
                id: "cot-1".into(),
                delta: "the file...".into(),
                elapsed_ms: 200,
            },
            AgentProgress::ExtendedThinking {
                id: Some("cot-1".into()),
                thought: "Reading the file...".into(),
                elapsed_ms: Some(200),
            },
            AgentProgress::ToolEnd {
                call_id: None,
                status: None,
                action: "read_file".into(),
                input: serde_json::json!({ "path": "src/main.rs" }),
                result: "fn main() {}".into(),
                subagent: None,
            },
            AgentProgress::ThinkingDelta {
                id: "cot-interrupted".into(),
                delta: "Let's check ".into(),
                elapsed_ms: 300,
            },
            AgentProgress::ThinkingDelta {
                id: "cot-interrupted".into(),
                delta: "more lines".into(),
                elapsed_ms: 350,
            },
        ];

        let compacted = compact_agent_progress(&events);
        assert!(
            !compacted
                .iter()
                .any(|e| matches!(e, AgentProgress::ThinkingDelta { id, .. } if id == "cot-1"))
        );
        assert!(compacted.iter().any(
            |e| matches!(e, AgentProgress::ToolStart { action, .. } if action == "read_file")
        ));
        assert!(
            compacted.iter().any(
                |e| matches!(e, AgentProgress::ToolEnd { action, .. } if action == "read_file")
            )
        );
        assert!(compacted.iter().any(|e| matches!(e, AgentProgress::ExtendedThinking { id: Some(id), thought, .. } if id == "cot-1" && thought == "Reading the file...")));
        assert!(compacted.iter().any(|e| matches!(e, AgentProgress::ExtendedThinking { id: Some(id), thought, .. } if id == "cot-interrupted" && thought == "Let's check more lines")));
    }

    #[tokio::test]
    async fn test_turn_lease_persists_agent_activity_on_complete() {
        let memory = MemoryStore::open(std::env::temp_dir().join(format!(
            "mint-turn-lease-complete-{}.sqlite",
            uuid::Uuid::new_v4()
        )));
        let mut turn = TurnLease::start(&memory, "cli::test_complete", "hello")
            .await
            .unwrap();
        let turn_id = turn.id;
        turn.activity
            .lock()
            .unwrap()
            .push(AgentProgress::ToolStart {
                call_id: None,
                action: "run_shell".into(),
                input: serde_json::json!({ "command": "ls -la" }),
                subagent: None,
            });
        turn.activity.lock().unwrap().push(AgentProgress::ToolEnd {
            call_id: None,
            status: None,
            action: "run_shell".into(),
            input: serde_json::json!({ "command": "ls -la" }),
            result: "file.txt".into(),
            subagent: None,
        });

        turn.complete_with_summary("Done listing files", "test_provider", "test_model", None)
            .unwrap();

        let rows = memory.interactions_for_chat("cli::test_complete").unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.id, turn_id);
        assert_eq!(row.status, "completed");
        assert_eq!(row.ai_text, "Done listing files");
        assert!(row.agent_activity.is_some());
        let activity_val = row.agent_activity.as_ref().unwrap();
        let activity_arr = activity_val.as_array().unwrap();
        assert_eq!(activity_arr.len(), 2);
        assert_eq!(activity_arr[0]["type"], "ToolStart");
        assert_eq!(activity_arr[1]["type"], "ToolEnd");
    }

    #[tokio::test]
    async fn test_turn_lease_persists_partial_activity_on_interrupt() {
        let memory = MemoryStore::open(std::env::temp_dir().join(format!(
            "mint-turn-lease-interrupt-{}.sqlite",
            uuid::Uuid::new_v4()
        )));
        let turn = TurnLease::start(&memory, "cli::test_interrupt", "run something")
            .await
            .unwrap();
        let turn_id = turn.id;
        turn.activity
            .lock()
            .unwrap()
            .push(AgentProgress::ToolStart {
                call_id: None,
                action: "read_file".into(),
                input: serde_json::json!({ "path": "test.txt" }),
                subagent: None,
            });
        drop(turn);

        let rows = memory.interactions_for_chat("cli::test_interrupt").unwrap();
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.id, turn_id);
        assert_eq!(row.status, "interrupted");
        assert!(row.agent_activity.is_some());
        let activity_val = row.agent_activity.as_ref().unwrap();
        let activity_arr = activity_val.as_array().unwrap();
        assert_eq!(activity_arr.len(), 1);
        assert_eq!(activity_arr[0]["type"], "ToolStart");
    }

    #[test]
    fn extracts_a_summary_incrementally_without_matching_text_inside_other_strings() {
        let mut stream = FinishSummaryStream::default();
        let mut output = String::new();
        let mut emit = |delta: String| output.push_str(&delta);

        stream.push_tool_call(0, Some("read_file"), None, None, &mut emit);
        stream.push_tool_call(
            0,
            None,
            Some(r#"{"thought":"the \"summary\" key is data","path":"x"}"#),
            None,
            &mut emit,
        );
        assert!(stream.emitted.is_empty());

        stream.push_tool_call(1, Some("fin"), None, None, &mut emit);
        stream.push_tool_call(1, Some("finish"), None, None, &mut emit);
        stream.push_tool_call(1, None, Some(r#"{"summary":"สวัส"#), None, &mut emit);
        stream.push_tool_call(
            1,
            None,
            Some(r#"ดี\nโลก","verification":""}"#),
            None,
            &mut emit,
        );

        assert_eq!(output, "สวัสดี\nโลก");
    }

    #[test]
    fn extracts_json_prompt_summary_as_chunks_arrive() {
        let mut stream = FinishSummaryStream::default();
        let mut output = String::new();
        let mut emit = |delta: String| output.push_str(&delta);
        stream.push_json_prompt(
            r#"{"thought":"done","action":"finish","input":{"summary":"Hello"#,
            &mut emit,
        );
        assert_eq!(stream.emitted, "Hello");
        stream.push_json_prompt(r#" world"}}"#, &mut emit);
        assert_eq!(output, "Hello world");
    }

    #[test]
    fn direct_native_text_is_not_sent_again_as_a_completed_summary() {
        assert_eq!(
            unstreamed_summary_remainder("A streamed answer.", "", "A streamed answer."),
            ""
        );
    }

    #[test]
    fn direct_native_text_appends_only_the_unstreamed_suffix() {
        assert_eq!(
            unstreamed_summary_remainder("A streamed answer, continued.", "", "A streamed answer"),
            ", continued."
        );
    }

    #[test]
    fn streamed_finish_summary_takes_precedence_over_direct_text() {
        assert_eq!(
            unstreamed_summary_remainder("Final answer.", "Final", "unrelated preamble"),
            " answer."
        );
    }

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
            temperature: None,
            ..Default::default()
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
        assert!(!decisions_are_parallel_read_only_batch(&[]));
    }

    #[test]
    fn two_or_more_read_only_decisions_qualify_for_the_parallel_batch() {
        let decisions = vec![
            decision_with_action("read_file"),
            decision_with_action("search_code"),
        ];
        assert!(decisions_are_parallel_read_only_batch(&decisions));

        let decisions = vec![
            decision_with_action("read_file"),
            decision_with_action("web_search"),
            decision_with_action("git_diff"),
        ];
        assert!(decisions_are_parallel_read_only_batch(&decisions));
    }

    #[test]
    fn a_lone_read_only_decision_stays_sequential() {
        let decisions = vec![decision_with_action("read_file")];
        assert!(!decisions_are_parallel_read_only_batch(&decisions));
    }

    #[test]
    fn read_only_mixed_with_mutating_action_stays_sequential() {
        let decisions = vec![
            decision_with_action("read_file"),
            decision_with_action("write_file"),
        ];
        assert!(!decisions_are_parallel_read_only_batch(&decisions));

        let decisions = vec![
            decision_with_action("read_file"),
            decision_with_action("run_shell"),
        ];
        assert!(!decisions_are_parallel_read_only_batch(&decisions));
    }

    #[test]
    fn rebuild_observation_reuses_buffer_and_formats_history() {
        let root = Path::new("/workspace/test-project");
        let task = "Fix the login bug";
        let step1 = format_trajectory_step(1, "Inspecting files", "list_files", "src/main.rs");
        let step2 = format_trajectory_step(2, "Reading code", "read_file", "fn main() {}");
        let trajectory = vec![step1, step2];

        let mut buffer = String::new();
        rebuild_observation(task, root, &trajectory, &mut buffer);

        assert!(buffer.contains("Task: Fix the login bug"));
        assert!(buffer.contains("Workspace: /workspace/test-project"));
        assert!(buffer.contains("Step 1:\n- Thought: Inspecting files"));
        assert!(buffer.contains("Step 2:\n- Thought: Reading code"));
        assert!(buffer.ends_with(
            "Proceed to the next step. If you have completed the task, use the 'finish' action."
        ));

        // Ensure buffer can be reused cleanly for subsequent steps
        let step3 = format_trajectory_step(3, "All done", "finish", "Fixed");
        let mut extended_trajectory = trajectory;
        extended_trajectory.push(step3);
        rebuild_observation(task, root, &extended_trajectory, &mut buffer);

        assert!(buffer.contains("Step 3:\n- Thought: All done"));
    }
}

#[cfg(test)]
mod full_skill_result_tests {
    use super::*;
    #[test]
    fn large_loaded_skill_results_reach_the_model_without_losing_middle_instructions() {
        let content = format!(
            "[Loaded skill: review]\n{}\nREQUIRED-MIDDLE-INSTRUCTION\n{}",
            "a".repeat(40_000),
            "z".repeat(40_000)
        );
        assert_eq!(model_tool_result("read_file", &content), content);
        assert!(model_tool_result("run_shell", &"x".repeat(80_000)).len() < 20_000);
    }
}

#[cfg(all(test, unix))]
mod mcp_outcome_tests {
    use super::*;
    use std::time::Duration;
    fn generic_server(tool: &str, response: Value) -> String {
        format!(
            r#"
import sys,json
response=json.loads({:?})
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':[{{'name':{:?},'inputSchema':{{'type':'object'}}}}]}}
 elif m=='tools/call': result=response
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
            response.to_string(),
            tool
        )
    }

    #[test]
    fn compact_device_progress_keeps_acceptance_and_latest_observation() {
        let events=["accepted","running","running","verified"].into_iter().enumerate().map(|(seq,state)|serde_json::from_value(serde_json::json!({"type":"DeviceState","data":{"callId":"one","server":"motor","tool":"move","update":{"state":state,"target":{"rpm":100},"actual":{"rpm":seq},"observationSeq":seq,"elapsedSecs":seq,"message":"observed"}}})).unwrap()).collect::<Vec<AgentProgress>>();
        let compacted = compact_agent_progress(&events);
        assert_eq!(
            compacted.len(),
            2,
            "saved history should retain acceptance and the final fresh observation"
        );
        assert!(matches!(
            &compacted[1],
            AgentProgress::DeviceState {
                update: crate::mcp::device::DeviceUpdate {
                    state: crate::mcp::device::DevicePhase::Verified,
                    ..
                },
                ..
            }
        ));
    }

    #[tokio::test]
    async fn mcp_definitions_reach_native_and_json_models_before_any_command() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for native in [true, false] {
            let marker =
                std::env::temp_dir().join(format!("mint-model-command-{}", uuid::Uuid::new_v4()));
            let server = format!("model-mcp-{}", uuid::Uuid::new_v4());
            let mut cfg = MintConfig::default();
            let script=generic_server("move",serde_json::json!({"content":[{"type":"text","text":"moved"}]})).replace("elif m=='tools/call': result=response",&format!("elif m=='tools/call':\n  open({:?},'a').write('called\\n')\n  result=response",marker.to_string_lossy())).replace("'name':\"move\",'inputSchema'","'name':\"move\",'description':'Move the mock axis. RPM are integer units.', 'inputSchema'");
            cfg.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":5}}));
            cfg.extra.insert(
                "allowedMcpTools".into(),
                serde_json::json!({server.clone():["*"]}),
            );
            cfg.ai_provider = "ollama".into();
            cfg.ollama_model = "qwen2.5".into();
            cfg.extra
                .insert("forceJsonPromptMode".into(), serde_json::json!(!native));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            cfg.ollama_host = format!("http://{}", listener.local_addr().unwrap());
            let recorded = Arc::new(std::sync::Mutex::new(Vec::<Value>::new()));
            let requests = Arc::clone(&recorded);
            let observed_marker = marker.clone();
            let name = server.clone();
            let provider = tokio::spawn(async move {
                for step in 0..3 {
                    let (mut socket, _) = listener.accept().await.unwrap();
                    let mut bytes = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let (end, len) = loop {
                        let n = socket.read(&mut chunk).await.unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&bytes[..end]);
                            let len = header
                                .lines()
                                .find_map(|l| {
                                    l.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse::<usize>().ok())
                                })
                                .unwrap();
                            break (end + 4, len);
                        }
                    };
                    while bytes.len() < end + len {
                        let n = socket.read(&mut chunk).await.unwrap();
                        assert!(n > 0);
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    requests
                        .lock()
                        .unwrap()
                        .push(serde_json::from_slice(&bytes[end..end + len]).unwrap());
                    if step == 1 {
                        assert!(
                            !observed_marker.exists(),
                            "initial preparation sent a tools/call"
                        );
                    }
                    let (action, input) = if step < 2 {
                        (
                            "mcp_tool",
                            serde_json::json!({"server":name,"tool":"move","arguments":{}}),
                        )
                    } else {
                        (
                            "finish",
                            serde_json::json!({"summary":"done","verification":"Mock command verified"}),
                        )
                    };
                    let message = if native {
                        serde_json::json!({"role":"assistant","content":"","tool_calls":[{"function":{"name":action,"arguments":input}}]})
                    } else {
                        serde_json::json!({"role":"assistant","content":serde_json::json!({"thought":"test","action":action,"input":input}).to_string()})
                    };
                    let body = format!(
                        "{}\n",
                        serde_json::json!({"model":"qwen2.5","message":message,"done":true,"prompt_eval_count":100,"eval_count":10})
                    );
                    socket.write_all(format!("HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",body.len()).as_bytes()).await.unwrap();
                }
            });
            let chat = format!("model-review-{}", uuid::Uuid::new_v4());
            let mut events = Vec::new();
            let outcome = tokio::time::timeout(
                Duration::from_secs(15),
                orchestrate_agent_loop(
                    &cfg,
                    "Use the mock MCP move tool",
                    Path::new("."),
                    None,
                    None,
                    None,
                    Some(&chat),
                    None,
                    None,
                    None,
                    true,
                    false,
                    |_| Ok(ApprovalOutcome::Denied),
                    |e| events.push(e),
                    |_| {},
                ),
            )
            .await;
            provider.abort();
            crate::mcp::close_mcp_session(&server);
            assert!(
                matches!(outcome, Ok(Ok(_))),
                "native={native}, {outcome:?}; requests={}, events={:?}",
                recorded.lock().unwrap().len(),
                events.iter().rev().take(5).collect::<Vec<_>>()
            );
            let requests = recorded.lock().unwrap();
            assert_eq!(requests.len(), 3);
            assert!(
                requests[1].to_string().contains("Move the mock axis"),
                "full tool definition missing from model request: {}",
                requests[1]
            );
            assert!(
                requests[2].to_string().contains("Move the mock axis"),
                "definition must remain in later requests"
            );
            assert_eq!(std::fs::read_to_string(&marker).unwrap().lines().count(), 1);
            std::fs::remove_file(marker).unwrap();
            assert_eq!(
                events
                    .iter()
                    .filter(
                        |e| matches!(e,AgentProgress::ToolStart {action,..} if action=="mcp_tool")
                    )
                    .count(),
                1
            );
            let memory = MemoryStore::open_default().unwrap();
            let _ = memory.delete_chat_session(&chat);
        }
    }

    #[tokio::test]
    async fn device_command_waits_for_a_fresh_verified_observation() {
        let server = format!("device-{}", uuid::Uuid::new_v4());
        let mut cfg = MintConfig::default();
        cfg.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"node","args":[Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/device-mcp/server.mjs")],"timeoutSecs":5}}));
        cfg.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({server.clone():["*"]}),
        );
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock-device","rpm":100}}})).unwrap();
        let mut events = Vec::new();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "device-test",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |e| events.push(e),
            "device-command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Success
        );
        let events = serde_json::to_value(events).unwrap();
        assert!(
            events
                .as_array()
                .unwrap()
                .iter()
                .any(|e| e["type"] == "DeviceState" && e["data"]["update"]["state"] == "verified"),
            "receipt must not be considered a verified device state: {events}"
        );
    }

    #[tokio::test]
    async fn device_capture_preserves_verified_images_and_explicit_stop_confirms_zero() {
        let server = format!("device-photo-{}", uuid::Uuid::new_v4());
        let mut cfg = MintConfig::default();
        cfg.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"node","args":[Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/device-mcp/server.mjs")],"timeoutSecs":10}}));
        cfg.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({server.clone():["*"]}),
        );
        let photo:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"capture_photo","arguments":{"deviceId":"mock-device"}}})).unwrap();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &photo,
            "photo-test",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |_| {},
            "photo-command",
        )
        .await
        .unwrap();
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Success
        );
        assert_eq!(out.images.len(), 1);
        assert!(out.warnings.is_empty());
        remove_test_artifacts(&out.artifacts);
        crate::mcp::call_mcp_tool_async(
            &cfg,
            &server,
            "set_speed",
            serde_json::json!({"deviceId":"mock-device","rpm":1000}),
            "photo-test",
            |_| {},
        )
        .await
        .unwrap();
        let stop:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"stop","arguments":{"deviceId":"mock-device"}}})).unwrap();
        let mut events = Vec::new();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &stop,
            "photo-test",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |e| events.push(e),
            "stop-command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Success
        );
        assert!(events.iter().any(|e|matches!(e,AgentProgress::DeviceState {update,..} if matches!(update.state,crate::mcp::device::DevicePhase::Verified) && update.actual["rpm"]==0)));
    }

    fn device_fixture(status: Value, timeout: u64) -> (MintConfig, String, PathBuf) {
        let name = format!("device-fault-{}", uuid::Uuid::new_v4());
        let log = std::env::temp_dir().join(format!("mint-device-calls-{}", uuid::Uuid::new_v4()));
        let schema = serde_json::json!({"type":"object","properties":{"deviceId":{"type":"string"},"operationId":{"type":"string"},"rpm":{"type":"integer"}},"required":["deviceId"],"additionalProperties":false});
        let tools = serde_json::json!([{"name":"set_speed","inputSchema":schema,"outputSchema":{"type":"object"},"_meta":{"mint/device":{"version":1,"statusTool":"get_status"}}},{"name":"get_status","inputSchema":schema,"outputSchema":{"type":"object"}}]);
        let receipt = serde_json::json!({"deviceId":"mock","operationId":"one","state":"accepted","observationSeq":1,"target":{"rpm":100},"actual":{"rpm":0}});
        let script = format!(
            r#"
import sys,json
tools=json.loads({:?});receipt=json.loads({:?});status=json.loads({:?})
for line in sys.stdin:
 r=json.loads(line);m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':tools}}
 elif m=='tools/call':
  with open({:?},'a') as f: f.write(json.dumps(r['params'])+'\n')
  result={{'structuredContent':receipt if r['params']['name']=='set_speed' else status,'content':[]}}
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
            tools.to_string(),
            receipt.to_string(),
            status.to_string(),
            log.to_string_lossy()
        );
        let mut cfg = MintConfig::default();
        cfg.extra.insert("mcpServers".into(),serde_json::json!({name.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":timeout}}));
        cfg.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({name.clone():["*"]}),
        );
        (cfg, name, log)
    }

    #[tokio::test]
    async fn device_stale_observations_cannot_confirm_completion() {
        let (cfg, server, log) = device_fixture(
            serde_json::json!({"deviceId":"mock","operationId":"one","state":"completed","observationSeq":1,"target":{"rpm":100},"actual":{"rpm":100}}),
            1,
        );
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "stale-test",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |_| {},
            "stale-command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        let calls = std::fs::read_to_string(&log).unwrap();
        std::fs::remove_file(log).unwrap();
        assert_eq!(calls.lines().filter(|l| l.contains("set_speed")).count(), 1);
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::TimedOut
        );
        assert!(
            out.completion_unconfirmed,
            "a dispatched command remains uncertain after stale observations"
        );
    }

    #[tokio::test]
    async fn review_device_session_loss_preserves_receipt_without_reconnecting_or_repeating() {
        let (mut cfg, server, log) = device_fixture(
            serde_json::json!({"deviceId":"mock","operationId":"one","state":"running","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":50}}),
            5,
        );
        let starts = log.with_extension("starts");
        let script = cfg.extra["mcpServers"][&server]["args"][2]
            .as_str()
            .unwrap()
            .replace(
                "import sys,json",
                &format!(
                    "import sys,json,os\nopen({:?},'a').write('start\\n')",
                    starts.to_string_lossy()
                ),
            );
        cfg.extra.get_mut("mcpServers").unwrap()[&server]["args"][2] = serde_json::json!(format!(
            "{script}\n if m=='tools/call' and r['params']['name']=='get_status': os._exit(0)\n"
        ));
        let decision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
        let mut events = Vec::new();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "review-device-loss",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |e| events.push(e),
            "command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        let calls = std::fs::read_to_string(&log).unwrap();
        assert_eq!(std::fs::read_to_string(&starts).unwrap(), "start\n");
        let _ = std::fs::remove_file(log);
        let _ = std::fs::remove_file(starts);
        assert_eq!(calls.lines().filter(|l| l.contains("set_speed")).count(), 1);
        assert_eq!(
            calls.lines().filter(|l| l.contains("get_status")).count(),
            1
        );
        assert_eq!(out.status, crate::mcp_result::ToolStatus::Failed);
        assert!(out.completion_unconfirmed);
        assert!(
            out.text.contains("one"),
            "operation receipt lost: {}",
            out.text
        );
        assert!(events.iter().any(|e|matches!(e,AgentProgress::DeviceState{update,..} if matches!(update.state,crate::mcp::device::DevicePhase::Unconfirmed) && update.operation_id.as_deref()==Some("one"))));
        assert!(!events.iter().any(|e|matches!(e,AgentProgress::DeviceState{update,..} if matches!(update.state,crate::mcp::device::DevicePhase::Verified))));
    }

    #[tokio::test]
    async fn device_wrong_operation_and_physical_failure_never_report_success() {
        for status in [
            serde_json::json!({"deviceId":"mock","operationId":"other","state":"completed","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":100}}),
            serde_json::json!({"deviceId":"mock","operationId":"one","state":"failed","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":0},"error":"jammed"}),
        ] {
            let (cfg, server, log) = device_fixture(status, 3);
            let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
            let out = execute_tool_outcome(
                Path::new("."),
                &cfg,
                &decision,
                "wrong-operation",
                &mut |_| Ok(ApprovalOutcome::Denied),
                &mut |_| {},
                "wrong-command",
            )
            .await
            .unwrap();
            crate::mcp::close_mcp_session(&server);
            std::fs::remove_file(log).unwrap();
            assert_eq!(
                out.status,
                crate::integrations::mcp_result::ToolStatus::Failed
            );
            assert!(
                out.text.starts_with("Error:"),
                "Native tool results must carry an error marker for failed verification: {}",
                out.text
            );
            assert!(
                !out.text.contains("no tool was executed"),
                "verification errors must acknowledge the already dispatched command: {}",
                out.text
            );
        }
    }

    #[tokio::test]
    async fn device_failed_receipt_is_terminal_without_status_polling() {
        let (mut cfg, server, log) = device_fixture(
            serde_json::json!({"deviceId":"mock","operationId":"one","state":"completed","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":100}}),
            3,
        );
        let script = cfg.extra["mcpServers"][&server]["args"][2]
            .as_str()
            .unwrap()
            .replace("accepted", "failed");
        cfg.extra.get_mut("mcpServers").unwrap()[&server]["args"][2] = serde_json::json!(script);
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "failed-receipt",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |_| {},
            "failed-command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        let calls = std::fs::read_to_string(&log).unwrap();
        std::fs::remove_file(log).unwrap();
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Failed
        );
        assert!(
            !calls.contains("get_status"),
            "failed receipt must not be overwritten by a later status"
        );
    }

    #[tokio::test]
    async fn device_denied_status_access_blocks_movement_before_dispatch() {
        let (mut cfg, server, log) = device_fixture(serde_json::json!({}), 3);
        cfg.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({server.clone():["set_speed"]}),
        );
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
        let mut approvals = Vec::new();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "denied-status",
            &mut |a| {
                approvals.push(serde_json::to_value(a).unwrap());
                Ok(ApprovalOutcome::Denied)
            },
            &mut |_| {},
            "blocked-command",
        )
        .await
        .unwrap();
        crate::mcp::close_mcp_session(&server);
        assert!(
            !log.exists(),
            "command was dispatched without access to status verification"
        );
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Failed
        );
        assert!(
            serde_json::to_string(&approvals)
                .unwrap()
                .contains("get_status")
        );
    }

    #[tokio::test]
    async fn canceling_between_device_observations_does_not_issue_another_command() {
        let (cfg, server, log) = device_fixture(
            serde_json::json!({"deviceId":"mock","operationId":"one","state":"running","observationSeq":2,"target":{"rpm":100},"actual":{"rpm":20}}),
            15,
        );
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"set_speed","arguments":{"deviceId":"mock","rpm":100}}})).unwrap();
        let chat = format!("cancel-between-{}", uuid::Uuid::new_v4());
        let name = chat.clone();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let mut tx = Some(tx);
        let running = tokio::spawn(async move {
            execute_tool_outcome(
                Path::new("."),
                &cfg,
                &decision,
                &name,
                &mut |_| Ok(ApprovalOutcome::Denied),
                &mut |e| {
                    if matches!(
                        e,
                        AgentProgress::DeviceState {
                            update: crate::mcp::device::DeviceUpdate {
                                state: crate::mcp::device::DevicePhase::Running,
                                ..
                            },
                            ..
                        }
                    ) && let Some(tx) = tx.take()
                    {
                        let _ = tx.send(());
                    }
                },
                "cancel-command",
            )
            .await
            .unwrap()
        });
        tokio::time::timeout(Duration::from_secs(10), rx)
            .await
            .unwrap()
            .unwrap();
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert!(crate::mcp::cancel_mcp_calls(&chat) > 0);
        let out = tokio::time::timeout(Duration::from_secs(2), running)
            .await
            .unwrap()
            .unwrap();
        crate::mcp::close_mcp_session(&server);
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Cancelled
        );
        let calls = std::fs::read_to_string(&log).unwrap();
        std::fs::remove_file(log).unwrap();
        assert_eq!(calls.lines().filter(|l| l.contains("set_speed")).count(), 1);
        assert!(!calls.contains("\"stop\""));
    }

    #[tokio::test]
    async fn approval_feedback_does_not_mark_an_unexecuted_mcp_call_successful() {
        let decision: AgentDecision = serde_json::from_value(serde_json::json!({
            "action":"mcp_tool", "input":{"server":"feedback-server","tool":"render","arguments":{}}
        }))
        .unwrap();
        let mut config = MintConfig::default();
        config.extra.insert("mcpServers".into(),serde_json::json!({"feedback-server":{"command":"python3","args":["-u","-c",generic_server("render",serde_json::json!({}))]}}));
        let out = execute_tool_outcome(
            Path::new("."),
            &config,
            &decision,
            "feedback",
            &mut |_| {
                Ok(ApprovalOutcome::Intercepted(
                    "Use a different device".into(),
                ))
            },
            &mut |_| {},
            "not-executed",
        )
        .await
        .unwrap();
        assert!(out.text.contains("Use a different device"));
        assert!(out.text.contains("no command was executed"));
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Failed
        );
    }

    #[tokio::test]
    async fn text_only_model_keeps_artifacts_without_receiving_image_blocks() {
        use base64::Engine as _;
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let encoded = base64::engine::general_purpose::STANDARD.encode(bytes.into_inner());
        let response =
            serde_json::json!({"content":[{"type":"image","mimeType":"image/png","data":encoded}]})
                .to_string();
        let script = generic_server("preview", serde_json::from_str(&response).unwrap());
        let server = format!("text-model-{}", uuid::Uuid::new_v4());
        let mut cfg = MintConfig::default();
        cfg.extra.insert(
            "mcpServers".into(),
            serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",script]}}),
        );
        cfg.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({server.clone():["preview"]}),
        );
        cfg.extra.insert("mcpImageInput".into(), Value::Bool(false));
        let decision:AgentDecision=serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"preview","arguments":{}}})).unwrap();
        let out = execute_tool_outcome(
            Path::new("."),
            &cfg,
            &decision,
            "text-only",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |_| {},
            "preview",
        )
        .await
        .unwrap();
        assert!(out.images.is_empty());
        assert_eq!(out.artifacts.len(), 1);
        assert!(out.text.contains("do not claim visual inspection"));
        crate::mcp::close_mcp_session(&server);
        remove_test_artifacts(&out.artifacts);
    }

    #[test]
    fn multiple_images_attach_to_the_correct_native_tool_result() {
        let images = std::collections::HashMap::from([(
            "call-2".to_string(),
            vec![
                "data:image/png;base64,first".into(),
                "data:image/png;base64,second".into(),
            ],
        )]);
        let results = vec![
            (
                "call-1".into(),
                "mcp_tool".into(),
                Value::Null,
                "one".into(),
            )
                .into(),
            (
                "call-2".into(),
                "mcp_tool".into(),
                Value::Null,
                "two".into(),
            )
                .into(),
        ];
        let mut messages = Vec::new();
        append_native_tool_results(&mut messages, "", &results, &Default::default(), &images);
        assert!(
            matches!(&messages[1].content[1],ContentBlock::ToolResult {tool_use_id,..} if tool_use_id=="call-2")
        );
        assert!(
            matches!(&messages[1].content[2],ContentBlock::Image {data_uri} if data_uri.ends_with("first"))
        );
        assert!(
            matches!(&messages[1].content[3],ContentBlock::Image {data_uri} if data_uri.ends_with("second"))
        );
    }

    #[tokio::test]
    async fn mcp_images_do_not_enter_the_text_observation() {
        let server = format!("image-{}", uuid::Uuid::new_v4());
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::DynamicImage::new_rgb8(1, 1)
            .write_to(&mut bytes, image::ImageFormat::Png)
            .unwrap();
        let png = BASE64_STANDARD.encode(bytes.into_inner());
        let result = serde_json::json!({"content":[{"type":"text","text":"preview"},{"type":"image","mimeType":"image/png","data":png}]}).to_string();
        let script = generic_server("preview", serde_json::from_str(&result).unwrap());
        let mut config = MintConfig::default();
        config.extra.insert(
            "mcpServers".into(),
            serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",script]}}),
        );
        let decision: AgentDecision = serde_json::from_value(serde_json::json!({"action":"mcp_tool","input":{"server":server,"tool":"preview","arguments":{}}})).unwrap();
        let mut events = Vec::new();
        let out = execute_tool_outcome(
            Path::new("."),
            &config,
            &decision,
            "image-test",
            &mut |_| Ok(ApprovalOutcome::Approved),
            &mut |event| events.push(event),
            "image-invocation",
        )
        .await
        .unwrap();
        assert!(
            !out.text.contains(&png),
            "binary data must be removed from the model's text observation"
        );
        assert_eq!(
            out.status,
            crate::integrations::mcp_result::ToolStatus::Success
        );
        assert_eq!(out.images.len(), 1);
        assert_eq!(out.artifacts[0].call_id, "image-invocation");
        assert!(out.text.contains("preview"));
        assert!(!serde_json::to_string(&events).unwrap().contains(&png));
        assert!(
            !crate::mcp::is_mcp_tool_allowed(&config, &server, "preview"),
            "one-call approval must not persist a grant"
        );
        crate::mcp::close_mcp_session(&server);
        remove_test_artifacts(&out.artifacts);
    }

    fn remove_test_artifacts(artifacts: &[crate::integrations::mcp_result::McpArtifact]) {
        let directory = crate::integrations::mcp_result::artifact_directory().unwrap();
        for artifact in artifacts {
            std::fs::remove_file(directory.join(&artifact.id)).unwrap();
            std::fs::remove_file(directory.join(format!("{}.json", artifact.id))).unwrap();
        }
    }

    // A transport-successful tool failure must not become a successful agent step.
    #[tokio::test]
    async fn mcp_tool_failure_is_visible_to_the_agent() {
        let server = format!("outcome-{}", uuid::Uuid::new_v4());
        let mut config = MintConfig::default();
        config.extra.insert("mcpServers".into(),serde_json::json!({server.clone():{"command":"python3","args":["-u","-c",generic_server("render",serde_json::json!({"isError":true,"content":[{"type":"text","text":"render failed"}]}))]}}));
        config.extra.insert(
            "allowedMcpTools".into(),
            serde_json::json!({server.clone():["render"]}),
        );
        let decision: AgentDecision = serde_json::from_value(serde_json::json!({
            "thought":"render", "action":"mcp_tool", "input":{"server":server,"tool":"render","arguments":{}}
        })).unwrap();
        let result = execute_tool(
            Path::new("."),
            &config,
            &decision,
            "outcome-test",
            &mut |_| Ok(ApprovalOutcome::Denied),
            &mut |_| {},
        )
        .await
        .unwrap();
        assert!(
            result.starts_with("Error:"),
            "tool failure must reach the model as a failure: {result}"
        );
        assert!(result.contains("render failed"));
        crate::mcp::close_mcp_session(&server);
    }
}

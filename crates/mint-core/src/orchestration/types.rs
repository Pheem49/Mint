use std::future::Future;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::CodePatchHunk;
use crate::agent::chat::{ChatError, ChatResponse};
use crate::memory::{MemoryError, MemoryStore};

use super::reliability;
use super::trajectory_schema;

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
pub(super) struct TurnLease {
    pub(super) chat_id: String,
    memory: MemoryStore,
    pub(super) id: i64,
    heartbeat: tokio::task::JoinHandle<()>,
    pub(super) finished: bool,
    pub(super) activity: Arc<std::sync::Mutex<Vec<AgentProgress>>>,
}

impl TurnLease {
    pub(super) async fn start(
        memory: &MemoryStore,
        chat_id: &str,
        text: &str,
    ) -> Result<Self, MemoryError> {
        let id = memory.start_turn(chat_id, text)?;
        crate::tts::playback::cancel_chat(memory, chat_id);
        crate::companion::queued(chat_id, id, text);
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
            chat_id: chat_id.to_owned(),
            memory: memory.clone(),
            id,
            heartbeat,
            finished: false,
            activity: Arc::new(std::sync::Mutex::new(Vec::new())),
        };
        loop {
            match memory.claim_turn(chat_id, id) {
                Ok(true) => {
                    crate::companion::update(
                        chat_id,
                        id,
                        mint_companion_protocol::Status::Thinking,
                        None,
                    );
                    return Ok(lease);
                }
                Ok(false) => tokio::time::sleep(std::time::Duration::from_millis(250)).await,
                Err(error) => {
                    lease.fail();
                    return Err(error);
                }
            }
        }
    }

    pub(super) fn persist_activity(&self) {
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

    pub(super) fn complete_with_summary(
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
        crate::companion::update(
            &self.chat_id,
            self.id,
            mint_companion_protocol::Status::Completed,
            Some(summary.to_owned()),
        );
        Ok(())
    }

    pub(super) fn complete(&mut self, response: &ChatResponse) -> Result<(), MemoryError> {
        self.complete_with_summary(
            &response.text,
            &response.provider,
            &response.model,
            response.fallback_provider.as_deref(),
        )
    }

    pub(super) fn fail(&mut self) {
        self.persist_activity();
        let _ = self.memory.end_turn(self.id, "failed");
        crate::companion::update(
            &self.chat_id,
            self.id,
            mint_companion_protocol::Status::Failed,
            None,
        );
        self.finished = true;
    }

    pub(super) fn interrupt(&mut self) {
        self.persist_activity();
        let _ = self.memory.end_turn(self.id, "interrupted");
        crate::companion::update(
            &self.chat_id,
            self.id,
            mint_companion_protocol::Status::Interrupted,
            None,
        );
        self.finished = true;
    }
}

impl Drop for TurnLease {
    fn drop(&mut self) {
        self.heartbeat.abort();
        if !self.finished {
            self.persist_activity();
            let _ = self.memory.end_turn(self.id, "interrupted");
            crate::companion::update(
                &self.chat_id,
                self.id,
                mint_companion_protocol::Status::Interrupted,
                None,
            );
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

/// A selectable option presented to the user during an [`AgentApproval::AskUser`]
/// prompt, carrying a primary label and optional detail text for both the CLI
/// picker and the desktop/web approval card.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AskUserOption {
    pub label: String,
    #[serde(default)]
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentApproval {
    /// Persistent agent instructions need a decision for each concrete diff.
    SkillWrite {
        path: String,
        content: String,
        diff: String,
    },
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
    ExecutionUpdate {
        #[serde(rename = "callId")]
        call_id: String,
        executions: Vec<reliability::ExecutionEvidence>,
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
        #[serde(default, alias = "elapsedMs", skip_serializing_if = "Option::is_none")]
        elapsed_ms: Option<u64>,
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
    SubagentStarted {
        subagent: String,
        profile: String,
        task: String,
    },
    SubagentFinished {
        subagent: String,
        status: String,
        #[serde(default, alias = "elapsed_ms", rename = "elapsedMs")]
        elapsed_ms: u64,
        steps: u32,
        #[serde(default, alias = "summary_preview", rename = "summaryPreview")]
        summary_preview: String,
    },
}

impl From<crate::swarm::SwarmEvent> for AgentProgress {
    fn from(event: crate::swarm::SwarmEvent) -> Self {
        match event {
            crate::swarm::SwarmEvent::Started {
                label,
                profile,
                task,
            } => AgentProgress::SubagentStarted {
                subagent: label,
                profile,
                task,
            },
            crate::swarm::SwarmEvent::Progress(p) => p,
            crate::swarm::SwarmEvent::Finished {
                label,
                status,
                elapsed_ms,
                steps,
                summary_preview,
            } => AgentProgress::SubagentFinished {
                subagent: label,
                status: status.as_str().to_string(),
                elapsed_ms,
                steps,
                summary_preview,
            },
        }
    }
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trajectory: Option<trajectory_schema::StandardTrajectoryRun>,
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
    #[serde(default)]
    pub completion_status: String,
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

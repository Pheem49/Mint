//! Swarm: runs a *batch* of isolated sub-agents for the main agent.
//!
//! The main agent hands the harness one `dispatch_agents` call carrying a list
//! of tasks; this module owns everything about running them — unique labels,
//! permission scoping, the concurrency cap, the write lane, lifecycle events,
//! and capping what flows back into the main agent's context. Callers see only
//! [`run_batch`]; the real agent loop is injected as a `runner` so the whole
//! module is testable without a model provider.
//!
//! Design rules enforced here:
//! - A sub-agent's permissions come from its **base profile only** — the model
//!   can name a role, never grant itself more tools (see [`AgentPermissions`]).
//! - Agents that can mutate state run one at a time (the "write lane");
//!   read-only agents run in parallel up to the configured cap.
//! - Every task yields exactly one [`SwarmOutcome`], in task order, and exactly
//!   one `Started` + one `Finished` event — success, failure and cancellation
//!   included.

mod batch;
mod dossier;
mod labels;
mod permissions;

pub use batch::{Emitter, run_batch};
pub use dossier::{
    MAX_DOSSIER_CHARS, cap_dossier, format_batch_result, format_elapsed_ms, format_outcome,
    summary_preview,
};
pub use labels::{LabelAllocator, sanitize_label};
pub use permissions::{
    AgentPermissions, READ_ONLY_ACTIONS, current_permissions, scope_permissions,
};

use crate::orchestration::AgentProgress;
use crate::subagents::SubagentDefinition;

/// Parallelism used when the user has not configured `swarmMaxParallel`.
pub const DEFAULT_MAX_PARALLEL: usize = 4;
const MIN_MAX_PARALLEL: usize = 1;
const MAX_MAX_PARALLEL: usize = 8;
/// Base profile for a task that names none — deliberately the read-only one.
pub const DEFAULT_BASE: &str = "explorer";

/// The parallelism cap to use: `None` (unset) means [`DEFAULT_MAX_PARALLEL`];
/// an explicit value is clamped to `1..=8` so a typo cannot hammer the
/// provider's rate limit or disable concurrency entirely.
pub fn effective_max_parallel(configured: Option<usize>) -> usize {
    configured
        .map(|n| n.clamp(MIN_MAX_PARALLEL, MAX_MAX_PARALLEL))
        .unwrap_or(DEFAULT_MAX_PARALLEL)
}

/// One unit of work the main agent asked for.
#[derive(Debug, Clone, Default)]
pub struct SwarmTask {
    /// Display name + focus the model gave this agent (optional).
    pub role: Option<String>,
    /// Profile that decides the permissions (optional; defaults to
    /// [`DEFAULT_BASE`]).
    pub base: Option<String>,
    pub instruction: String,
}

/// Everything a runner needs to execute one agent.
#[derive(Debug, Clone)]
pub struct AgentRun {
    /// Unique within the batch; used in the chat id and UI.
    pub label: String,
    /// `{parent}::subagent::{label}` — the marker many modules key off.
    pub chat_id: String,
    pub profile: String,
    pub role: Option<String>,
    pub instruction: String,
    pub system_prompt: String,
    pub permissions: AgentPermissions,
    pub provider: Option<String>,
    pub model: Option<String>,
    pub sandbox: Option<String>,
}

/// What a runner returns on success.
#[derive(Debug, Clone)]
pub struct RunnerOutput {
    pub text: String,
    pub steps: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Completed,
    Failed,
    Cancelled,
}

impl AgentStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }
}

/// The final record of one agent, handed back to the main agent.
#[derive(Debug, Clone)]
pub struct SwarmOutcome {
    pub label: String,
    pub profile: String,
    pub status: AgentStatus,
    pub elapsed_ms: u64,
    pub steps: u32,
    /// Dossier (success) or error text (failure), already capped.
    pub text: String,
}

/// Lifecycle + progress stream for the UI.
#[derive(Debug, Clone)]
pub enum SwarmEvent {
    Started {
        label: String,
        profile: String,
        task: String,
    },
    /// A nested progress event from inside an agent (tool calls etc.).
    Progress(AgentProgress),
    Finished {
        label: String,
        status: AgentStatus,
        elapsed_ms: u64,
        steps: u32,
        summary_preview: String,
    },
}

/// Per-batch inputs that are not tasks.
pub struct BatchCtx<'a> {
    pub parent_chat_id: &'a str,
    /// Already-effective cap (see [`effective_max_parallel`]).
    pub max_parallel: usize,
    /// Resolves a base profile name (builtin or user-authored file).
    pub profiles: &'a (dyn Fn(&str) -> Option<SubagentDefinition> + Sync),
    /// True if the parent turn is currently in Plan Mode.
    pub plan_mode: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_max_parallel_defaults_to_four_and_clamps_explicit_values() {
        assert_eq!(effective_max_parallel(None), 4);
        assert_eq!(effective_max_parallel(Some(3)), 3);
        assert_eq!(effective_max_parallel(Some(0)), 1);
        assert_eq!(effective_max_parallel(Some(100)), 8);
    }
}

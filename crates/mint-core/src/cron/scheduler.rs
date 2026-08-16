use std::path::PathBuf;
use std::time::Duration;

use chrono::{DateTime, Utc};

use crate::config::{MintConfig, PermissionDecision, load_config};
use crate::orchestration::{self, AgentApproval, AgentProgress, ApprovalOutcome};

use super::store::{CronJob, CronStore};

const TICK_INTERVAL: Duration = Duration::from_secs(60);

/// Starts the cron scheduler as a self-healing background loop, mirroring
/// [`crate::channels::start_channels`]'s runtime-detection dance: if a Tokio
/// runtime is already driving the calling process, ride along on it;
/// otherwise spin up a dedicated thread + runtime so the scheduler still runs
/// from contexts (like a plain `fn main`) that haven't started one yet.
/// There is no cron-specific "run as a daemon" command — the scheduler simply
/// lives as long as whichever long-running Mint process started it (the
/// interactive CLI, `mint api`/`mint web`, or the desktop app), exactly like
/// the messaging-channel bridges already do.
pub fn start_cron_scheduler() {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(restarting_loop());
    } else {
        std::thread::spawn(|| {
            if let Ok(runtime) = tokio::runtime::Runtime::new() {
                runtime.block_on(restarting_loop());
            }
        });
    }
}

async fn restarting_loop() {
    loop {
        tick().await;
        tokio::time::sleep(TICK_INTERVAL).await;
    }
}

async fn tick() {
    let Ok(config) = load_config() else { return };
    let Ok(store) = CronStore::open_default() else {
        return;
    };
    let Ok(jobs) = store.list() else { return };
    let now = Utc::now();

    for job in jobs {
        if !job.enabled {
            continue;
        }
        let Ok(due_at) = DateTime::parse_from_rfc3339(&job.next_run) else {
            continue;
        };
        if due_at.with_timezone(&Utc) > now {
            continue;
        }

        // Advance `next_run` before running: if this job's execution outlasts
        // the tick interval, the next tick will see a future `next_run` and
        // skip it, instead of firing it a second time while it's still busy.
        // If there's no further occurrence (e.g. a one-time job whose fixed
        // year has arrived), this due run is its last one ever — disable it
        // so it stops being "due" every tick from here on, but still run it
        // below instead of silently dropping its only/final firing.
        if store.advance_next_run(&job.id).is_err() {
            let _ = store.set_enabled(&job.id, false);
        }

        run_job(&config, &store, &job).await;
    }
}

/// A scheduled task's own conversation thread — same id scheme
/// `CronStore::add` uses to register the chat session up front (see there
/// for why: each task keeps its own history instead of all tasks sharing
/// one thread, so a task's context on each run is its own past runs, not
/// whatever unrelated task happened to run most recently).
pub(super) fn chat_id_for_job(job_id: &str) -> String {
    format!("cron::{job_id}")
}

async fn run_job(config: &MintConfig, store: &CronStore, job: &CronJob) {
    let chat_id = chat_id_for_job(&job.id);
    let root = job.workspace.clone();
    let approve_cb = cron_approve_callback(config.clone(), root.clone());
    let progress_cb = |_progress: AgentProgress| {};
    let on_chunk = |_chunk: String| {};

    let result = orchestration::orchestrate_agent_loop(
        config,
        &job.task,
        &root,
        None,
        None,
        None,
        Some(&chat_id),
        None,
        None,
        false,
        false,
        approve_cb,
        progress_cb,
        on_chunk,
    )
    .await;

    let (status, summary) = match result {
        Ok(agent_result) => ("success", Some(agent_result.summary)),
        Err(error) => ("failed", Some(error.to_string())),
    };
    let _ = store.record_run(&job.id, status, summary);
}

/// Maps an [`AgentApproval`] to the `(tool, subject)` pair
/// [`MintConfig::permission_decision`] matches against — the same mapping the
/// interactive CLI's approval prompt uses when a user picks "Always allow"
/// (see `crates/mint-cli/src/agent.rs`'s `confirm_with_persistence` call
/// sites), so a rule saved once works the same way whether it's consulted
/// interactively or by an unattended cron run.
fn approval_subject(approval: &AgentApproval) -> Option<(&'static str, String)> {
    match approval {
        AgentApproval::WriteFile { path, .. } => Some(("write_file", path.clone())),
        AgentApproval::ApplyPatch { path, .. } => Some(("apply_patch", path.clone())),
        AgentApproval::RunShell { command, .. } => Some(("run_shell", command.clone())),
        AgentApproval::NoteWrite { path, .. } => Some(("note_write", path.clone())),
        AgentApproval::RunPlugin { name, instruction } => {
            Some(("run_plugin", format!("{name}: {instruction}")))
        }
        AgentApproval::McpTool {
            server,
            tool,
            arguments,
        } => Some(("mcp_tool", format!("{server}:{tool}:{arguments}"))),
        AgentApproval::UserApproval { .. }
        | AgentApproval::ExitPlanMode { .. }
        | AgentApproval::EnterPlanMode { .. }
        | AgentApproval::AskUser { .. } => None,
    }
}

/// Cron jobs run unattended, so by design every action is auto-approved
/// *except* one explicitly saved "always deny" permission rule — there is
/// nobody present to answer a live prompt, so `AskUser` is declined rather
/// than hanging, matching the empty-answer behavior the interactive prompt
/// already falls back to.
fn cron_approve_callback(
    config: MintConfig,
    root: PathBuf,
) -> impl FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> {
    move |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
        if matches!(approval, AgentApproval::AskUser { .. }) {
            return Ok(ApprovalOutcome::Denied);
        }
        match approval_subject(approval) {
            Some((tool, subject)) => match config.permission_decision(tool, &subject, &root) {
                Some(PermissionDecision::Deny) => Ok(ApprovalOutcome::Denied),
                _ => Ok(ApprovalOutcome::Approved),
            },
            None => Ok(ApprovalOutcome::Approved),
        }
    }
}

//! The batch scheduler: resolves tasks into runs, schedules them under the
//! concurrency cap + write lane, and streams lifecycle events.
use std::future::Future;
use std::time::Instant;

use futures_util::stream::{self, StreamExt};
use tokio::sync::mpsc::{self, UnboundedSender};

use super::{
    AgentRun, AgentStatus, BatchCtx, DEFAULT_BASE, LabelAllocator, RunnerOutput, SwarmEvent,
    SwarmOutcome, SwarmTask, cap_dossier, sanitize_label, summary_preview, AgentPermissions,
};
use crate::orchestration::{AgentProgress, run_control};

const MAX_ROLE_CHARS: usize = 80;

/// Handle a runner uses to report nested progress (tool calls etc.) for the
/// agent it is running.
#[derive(Clone)]
pub struct Emitter {
    tx: UnboundedSender<SwarmEvent>,
}

impl Emitter {
    pub fn progress(&self, progress: AgentProgress) {
        let _ = self.tx.send(SwarmEvent::Progress(progress));
    }
}

enum Planned {
    Ready(Box<AgentRun>),
    Rejected {
        label: String,
        profile: String,
        task: String,
        reason: String,
    },
}

impl Planned {
    fn is_write_capable(&self) -> bool {
        match self {
            Planned::Ready(run) => run.permissions.is_write_capable(),
            Planned::Rejected { .. } => false,
        }
    }
}

fn one_line_role(role: &str) -> String {
    let flat = role.split_whitespace().collect::<Vec<_>>().join(" ");
    flat.chars().take(MAX_ROLE_CHARS).collect()
}

fn plan(ctx: &BatchCtx<'_>, tasks: Vec<SwarmTask>) -> Vec<Planned> {
    let mut labels = LabelAllocator::new();
    tasks
        .into_iter()
        .map(|task| {
            let base = task
                .base
                .as_deref()
                .map(str::trim)
                .filter(|b| !b.is_empty())
                .unwrap_or(DEFAULT_BASE)
                .to_string();
            let role = task
                .role
                .as_deref()
                .map(one_line_role)
                .filter(|r| !r.is_empty() && !r.eq_ignore_ascii_case("null"));
            let seed = [role.as_deref().unwrap_or(""), base.as_str()]
                .iter()
                .map(|s| sanitize_label(s))
                .find(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
                .unwrap_or_else(|| "agent".to_string());
            let label = labels.allocate(&seed);
            let instruction = task.instruction.trim().to_string();
            if instruction.is_empty() || instruction.eq_ignore_ascii_case("null") {
                return Planned::Rejected {
                    label,
                    profile: base,
                    task: String::new(),
                    reason: "instruction is required and cannot be empty or 'null'".into(),
                };
            }
            let Some(profile) = (ctx.profiles)(&base) else {
                return Planned::Rejected {
                    label,
                    profile: base.clone(),
                    task: instruction,
                    reason: format!("unknown base profile '{base}'"),
                };
            };
            let mut system_prompt = profile.system_prompt.clone();
            if let Some(role) = &role {
                system_prompt.push_str(&format!("\n\nYour role for this task: {role}."));
            }
            Planned::Ready(Box::new(AgentRun {
                chat_id: format!("{}::subagent::{label}", ctx.parent_chat_id),
                label,
                profile: base,
                role,
                instruction,
                system_prompt,
                permissions: AgentPermissions::from_tools(profile.tools.as_deref()),
                provider: profile.provider.clone(),
                model: profile.model.clone(),
                sandbox: profile.sandbox.clone(),
            }))
        })
        .collect()
}

fn emit(tx: &UnboundedSender<SwarmEvent>, event: SwarmEvent) {
    let _ = tx.send(event);
}

fn finish(
    tx: &UnboundedSender<SwarmEvent>,
    label: String,
    profile: String,
    status: AgentStatus,
    elapsed_ms: u64,
    steps: u32,
    text: String,
) -> SwarmOutcome {
    let text = cap_dossier(&text);
    emit(
        tx,
        SwarmEvent::Finished {
            label: label.clone(),
            status,
            elapsed_ms,
            steps,
            summary_preview: summary_preview(&text),
        },
    );
    SwarmOutcome {
        label,
        profile,
        status,
        elapsed_ms,
        steps,
        text,
    }
}

async fn execute_one<R, Fut>(
    idx: usize,
    planned: Planned,
    runner: &R,
    tx: &UnboundedSender<SwarmEvent>,
) -> (usize, SwarmOutcome)
where
    R: Fn(AgentRun, Emitter) -> Fut + Sync,
    Fut: Future<Output = Result<RunnerOutput, String>> + Send,
{
    let run = match planned {
        Planned::Rejected {
            label,
            profile,
            task,
            reason,
        } => {
            emit(
                tx,
                SwarmEvent::Started {
                    label: label.clone(),
                    profile: profile.clone(),
                    task,
                },
            );
            let outcome = finish(tx, label, profile, AgentStatus::Failed, 0, 0, reason);
            return (idx, outcome);
        }
        Planned::Ready(run) => *run,
    };
    emit(
        tx,
        SwarmEvent::Started {
            label: run.label.clone(),
            profile: run.profile.clone(),
            task: run.instruction.clone(),
        },
    );
    let (label, profile) = (run.label.clone(), run.profile.clone());
    if run_control::cancelled() {
        let outcome = finish(
            tx,
            label,
            profile,
            AgentStatus::Cancelled,
            0,
            0,
            "Not started: the turn was canceled.".into(),
        );
        return (idx, outcome);
    }
    let started = Instant::now();
    let result = runner(run, Emitter { tx: tx.clone() }).await;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let outcome = match result {
        Ok(out) => finish(
            tx,
            label,
            profile,
            AgentStatus::Completed,
            elapsed_ms,
            out.steps,
            out.text,
        ),
        Err(msg) => {
            let status = if run_control::cancelled() {
                AgentStatus::Cancelled
            } else {
                AgentStatus::Failed
            };
            finish(tx, label, profile, status, elapsed_ms, 0, msg)
        }
    };
    (idx, outcome)
}

/// Runs every task and returns one outcome per task, in task order.
pub async fn run_batch<R, Fut>(
    ctx: &BatchCtx<'_>,
    tasks: Vec<SwarmTask>,
    runner: R,
    on_event: &mut (dyn FnMut(SwarmEvent) + Send),
) -> Vec<SwarmOutcome>
where
    R: Fn(AgentRun, Emitter) -> Fut + Sync,
    Fut: Future<Output = Result<RunnerOutput, String>> + Send,
{
    let max = ctx.max_parallel.max(1);
    let planned: Vec<(usize, Planned)> = plan(ctx, tasks).into_iter().enumerate().collect();
    let (tx, mut rx) = mpsc::unbounded_channel::<SwarmEvent>();

    let runner = &runner;
    let txr = &tx;
    let work = async move {
        let mut results: Vec<(usize, SwarmOutcome)> = Vec::new();
        if max == 1 {
            let all = stream::iter(
                planned
                    .into_iter()
                    .map(|(i, p)| Box::pin(execute_one(i, p, runner, txr))),
            )
            .buffer_unordered(1)
            .collect::<Vec<_>>()
            .await;
            results.extend(all);
        } else {
            let (writers, readers): (Vec<_>, Vec<_>) =
                planned.into_iter().partition(|(_, p)| p.is_write_capable());
            let read_cap = if writers.is_empty() { max } else { max - 1 };
            let write_lane = stream::iter(
                writers
                    .into_iter()
                    .map(|(i, p)| Box::pin(execute_one(i, p, runner, txr))),
            )
            .buffer_unordered(1)
            .collect::<Vec<_>>();
            let read_lane = stream::iter(
                readers
                    .into_iter()
                    .map(|(i, p)| Box::pin(execute_one(i, p, runner, txr))),
            )
            .buffer_unordered(read_cap)
            .collect::<Vec<_>>();
            let (w, r) = tokio::join!(write_lane, read_lane);
            results.extend(w);
            results.extend(r);
        }
        results
    };
    tokio::pin!(work);
    let mut results = loop {
        tokio::select! {
            biased;
            Some(event) = rx.recv() => on_event(event),
            res = &mut work => break res,
        }
    };
    while let Ok(event) = rx.try_recv() {
        on_event(event);
    }
    results.sort_by_key(|(i, _)| *i);
    results.into_iter().map(|(_, o)| o).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subagents::{SubagentDefinition, builtin_subagents};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use tokio_util::sync::CancellationToken;

    fn builtin(name: &str) -> Option<SubagentDefinition> {
        builtin_subagents().into_iter().find(|d| d.name == name)
    }

    fn task(role: Option<&str>, base: Option<&str>, instruction: &str) -> SwarmTask {
        SwarmTask {
            role: role.map(String::from),
            base: base.map(String::from),
            instruction: instruction.into(),
        }
    }

    #[derive(Default)]
    struct Probe {
        active: AtomicUsize,
        max_active: AtomicUsize,
        writers: AtomicUsize,
        max_writers: AtomicUsize,
        calls: AtomicUsize,
    }

    fn bump(cur: &AtomicUsize, max: &AtomicUsize) {
        let now = cur.fetch_add(1, Ordering::SeqCst) + 1;
        max.fetch_max(now, Ordering::SeqCst);
    }

    async fn probed(probe: Arc<Probe>, run: AgentRun, delay_ms: u64) -> Result<RunnerOutput, String> {
        probe.calls.fetch_add(1, Ordering::SeqCst);
        bump(&probe.active, &probe.max_active);
        let writer = run.permissions.is_write_capable();
        if writer {
            bump(&probe.writers, &probe.max_writers);
        }
        tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        if writer {
            probe.writers.fetch_sub(1, Ordering::SeqCst);
        }
        probe.active.fetch_sub(1, Ordering::SeqCst);
        Ok(RunnerOutput {
            text: format!("done {}", run.label),
            steps: 2,
        })
    }

    async fn go<R, Fut>(
        tasks: Vec<SwarmTask>,
        max: usize,
        runner: R,
    ) -> (Vec<SwarmOutcome>, Vec<SwarmEvent>)
    where
        R: Fn(AgentRun, Emitter) -> Fut + Sync,
        Fut: Future<Output = Result<RunnerOutput, String>> + Send,
    {
        let ctx = BatchCtx {
            parent_chat_id: "cli::abc",
            max_parallel: max,
            profiles: &builtin,
        };
        let mut events = Vec::new();
        let outcomes = run_batch(&ctx, tasks, runner, &mut |e| events.push(e)).await;
        (outcomes, events)
    }

    #[tokio::test]
    async fn parallelism_is_capped() {
        let probe = Arc::new(Probe::default());
        let p = probe.clone();
        let tasks = (0..6).map(|i| task(None, None, &format!("t{i}"))).collect();
        let (out, _) = go(tasks, 4, move |run, _| probed(p.clone(), run, 40)).await;
        assert_eq!(out.len(), 6);
        assert_eq!(probe.calls.load(Ordering::SeqCst), 6);
        assert_eq!(probe.max_active.load(Ordering::SeqCst), 4);
    }

    #[tokio::test]
    async fn write_capable_agents_run_one_at_a_time_while_reads_overlap() {
        let probe = Arc::new(Probe::default());
        let p = probe.clone();
        let tasks = vec![
            task(None, Some("debugger"), "a"),
            task(None, Some("debugger"), "b"),
            task(None, Some("explorer"), "c"),
            task(None, Some("explorer"), "d"),
            task(None, Some("explorer"), "e"),
        ];
        let (out, _) = go(tasks, 4, move |run, _| probed(p.clone(), run, 40)).await;
        assert_eq!(out.len(), 5);
        assert_eq!(probe.max_writers.load(Ordering::SeqCst), 1);
        assert!(probe.max_active.load(Ordering::SeqCst) >= 2);
    }

    #[tokio::test]
    async fn max_one_is_fully_sequential() {
        let probe = Arc::new(Probe::default());
        let p = probe.clone();
        let tasks = (0..3).map(|i| task(None, None, &format!("t{i}"))).collect();
        go(tasks, 1, move |run, _| probed(p.clone(), run, 10)).await;
        assert_eq!(probe.max_active.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn outcomes_follow_task_order_not_finish_order() {
        let tasks = (0..3).map(|i| task(None, None, &format!("{i}"))).collect();
        let (out, _) = go(tasks, 4, |run, _| async move {
            let n: u64 = run.instruction.parse().unwrap();
            tokio::time::sleep(Duration::from_millis(60 - n * 25)).await;
            Ok(RunnerOutput { text: run.instruction, steps: 1 })
        })
        .await;
        let texts: Vec<_> = out.iter().map(|o| o.text.as_str()).collect();
        assert_eq!(texts, ["0", "1", "2"]);
    }

    #[tokio::test]
    async fn duplicate_profiles_get_unique_labels_and_chat_ids() {
        let tasks = vec![task(None, None, "a"), task(None, None, "b")];
        let ids = Arc::new(std::sync::Mutex::new(Vec::new()));
        let i = ids.clone();
        let (out, _) = go(tasks, 4, move |run, _| {
            i.lock().unwrap().push(run.chat_id.clone());
            async { Ok(RunnerOutput { text: "x".into(), steps: 1 }) }
        })
        .await;
        assert_eq!(out[0].label, "explorer");
        assert_eq!(out[1].label, "explorer-2");
        let ids = ids.lock().unwrap();
        assert!(ids.contains(&"cli::abc::subagent::explorer".to_string()));
        assert!(ids.contains(&"cli::abc::subagent::explorer-2".to_string()));
    }

    #[tokio::test]
    async fn ad_hoc_role_inherits_the_read_only_default_base() {
        let seen = Arc::new(std::sync::Mutex::new(None));
        let s = seen.clone();
        let tasks = vec![task(Some("Stripe Webhook Tracer"), None, "trace it")];
        let (out, _) = go(tasks, 4, move |run, _| {
            *s.lock().unwrap() = Some(run.clone());
            async { Ok(RunnerOutput { text: "x".into(), steps: 1 }) }
        })
        .await;
        assert_eq!(out[0].label, "stripe-webhook-tracer");
        assert_eq!(out[0].profile, "explorer");
        let run = seen.lock().unwrap().clone().unwrap();
        assert!(!run.permissions.is_write_capable());
        assert!(!run.permissions.allows("write_file"));
        assert!(run.system_prompt.contains("Stripe Webhook Tracer"));
    }

    #[tokio::test]
    async fn unknown_base_and_empty_instruction_fail_without_running() {
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let tasks = vec![task(None, Some("nope"), "x"), task(None, None, "   ")];
        let (out, events) = go(tasks, 4, move |_, _| {
            c.fetch_add(1, Ordering::SeqCst);
            async { Ok(RunnerOutput { text: String::new(), steps: 0 }) }
        })
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(out.iter().all(|o| o.status == AgentStatus::Failed));
        assert!(out[0].text.contains("unknown base profile 'nope'"));
        assert!(out[1].text.contains("instruction is required"));
        assert_eq!(events.len(), 4);
    }

    #[tokio::test]
    async fn runner_error_becomes_failed_with_lifecycle_events() {
        let (out, events) = go(vec![task(None, None, "x")], 4, |_, _| async {
            Err::<RunnerOutput, _>("boom".to_string())
        })
        .await;
        assert_eq!(out[0].status, AgentStatus::Failed);
        assert!(out[0].text.contains("boom"));
        assert!(matches!(events.first(), Some(SwarmEvent::Started { .. })));
        assert!(matches!(
            events.last(),
            Some(SwarmEvent::Finished { status: AgentStatus::Failed, .. })
        ));
    }

    #[tokio::test]
    async fn events_arrive_started_then_progress_then_finished() {
        let (_, events) = go(vec![task(None, None, "x")], 4, |run, em| async move {
            em.progress(AgentProgress::ToolStart {
                call_id: None,
                action: "read_file".into(),
                input: serde_json::json!({}),
                subagent: Some(run.label.clone()),
            });
            Ok(RunnerOutput { text: "ok".into(), steps: 1 })
        })
        .await;
        assert!(matches!(events[0], SwarmEvent::Started { .. }));
        assert!(matches!(
            &events[1],
            SwarmEvent::Progress(AgentProgress::ToolStart { subagent: Some(s), .. }) if s == "explorer"
        ));
        assert!(matches!(events[2], SwarmEvent::Finished { status: AgentStatus::Completed, .. }));
    }

    #[tokio::test]
    async fn a_pre_cancelled_turn_starts_nothing() {
        let token = CancellationToken::new();
        token.cancel();
        let calls = Arc::new(AtomicUsize::new(0));
        let c = calls.clone();
        let (out, events) = run_control::scope_with_token(
            "cli::abc",
            token,
            go(vec![task(None, None, "a"), task(None, None, "b")], 4, move |_, _| {
                c.fetch_add(1, Ordering::SeqCst);
                async { Ok(RunnerOutput { text: String::new(), steps: 0 }) }
            }),
        )
        .await;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(out.iter().all(|o| o.status == AgentStatus::Cancelled));
        assert_eq!(events.len(), 4);
    }

    #[tokio::test]
    async fn cancelling_mid_run_reports_cancelled() {
        let token = CancellationToken::new();
        let t = token.clone();
        let (out, _) = run_control::scope_with_token(
            "cli::abc",
            token,
            go(vec![task(None, None, "a")], 4, move |_, _| {
                let t = t.clone();
                async move {
                    t.cancel();
                    Err::<RunnerOutput, _>("interrupted".to_string())
                }
            }),
        )
        .await;
        assert_eq!(out[0].status, AgentStatus::Cancelled);
    }

    #[tokio::test]
    async fn huge_results_are_capped() {
        let (out, _) = go(vec![task(None, None, "a")], 4, |_, _| async {
            Ok(RunnerOutput { text: "z".repeat(20_000), steps: 1 })
        })
        .await;
        assert!(out[0].text.ends_with("... [truncated]"));
    }

    #[tokio::test]
    async fn null_or_empty_instruction_is_rejected_cleanly() {
        let (out, _) = go(
            vec![
                task(Some("tui-tracer"), None, "null"),
                task(Some("null"), None, "VALID"),
            ],
            4,
            |_, _| async {
                Ok(RunnerOutput { text: "ok".into(), steps: 1 })
            },
        )
        .await;
        assert_eq!(out[0].status, AgentStatus::Failed);
        assert!(out[0].text.contains("cannot be empty or 'null'"));
        assert_eq!(out[1].status, AgentStatus::Completed);
        assert_eq!(out[1].label, "explorer"); // "null" role was filtered out, fallback to explorer
    }
}

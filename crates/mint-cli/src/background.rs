//! Background query jobs for the interactive CLI: `/bg <query>` spawns an agent
//! run without blocking the input prompt, `/jobs` lists/inspects/cancels them.
//!
//! Jobs run fully non-interactively — any action that would normally need an
//! approval prompt (writing files, running shell commands, plugins, MCP tools)
//! is auto-denied, since there is no way to safely prompt over the same stdin
//! the foreground prompt is reading from. This makes `/bg` best suited for
//! research/analysis-style queries rather than workspace-modifying tasks.

use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mint_core::{AgentApproval, ApprovalOutcome, MintConfig, orchestrate_agent_loop};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum JobStatus {
    Running,
    Done,
    Failed,
    Cancelled,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            JobStatus::Running => "running",
            JobStatus::Done => "done",
            JobStatus::Failed => "failed",
            JobStatus::Cancelled => "cancelled",
        };
        f.write_str(s)
    }
}

#[derive(Clone)]
pub struct JobRecord {
    pub id: u32,
    pub query: String,
    pub status: JobStatus,
    pub result: Option<String>,
    pub started_at: Instant,
    /// Seconds the job took, frozen once it stops running. While `status` is
    /// `Running` this is `None` and callers should use `started_at.elapsed()`
    /// instead so the displayed time keeps ticking live.
    pub finished_secs: Option<u64>,
}

impl JobRecord {
    /// Seconds elapsed: live if still running, frozen at completion otherwise.
    pub fn elapsed_secs(&self) -> u64 {
        self.finished_secs
            .unwrap_or_else(|| self.started_at.elapsed().as_secs())
    }
}

struct Inner {
    next_id: u32,
    jobs: Vec<JobRecord>,
    notices: Vec<String>,
}

/// Cheaply cloneable handle shared between the foreground prompt loop and
/// however many background job tasks are currently running.
#[derive(Clone)]
pub struct BackgroundJobs {
    inner: Arc<Mutex<Inner>>,
}

impl BackgroundJobs {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                next_id: 1,
                jobs: Vec::new(),
                notices: Vec::new(),
            })),
        }
    }

    fn chat_id_for(id: u32) -> String {
        format!("cli-bg-{id}")
    }

    /// Spawn `query` as a background agent run against `config`/`current_dir`.
    /// Returns the new job's id immediately; the caller does not wait on it.
    pub fn spawn(&self, config: &MintConfig, current_dir: &Path, query: &str) -> u32 {
        let id = {
            let mut inner = self.inner.lock().unwrap();
            let id = inner.next_id;
            inner.next_id += 1;
            inner.jobs.push(JobRecord {
                id,
                query: query.to_owned(),
                status: JobStatus::Running,
                result: None,
                started_at: Instant::now(),
                finished_secs: None,
            });
            id
        };

        let config = config.clone();
        let root = current_dir.to_path_buf();
        let query = query.to_owned();
        let chat_id = Self::chat_id_for(id);
        let jobs = self.clone();

        // Spawn the actual agent run directly (not nested inside another
        // spawned task) so its AbortHandle can be registered in
        // ACTIVE_AGENTS synchronously, before `spawn` returns `id` to the
        // caller. Registering it from inside a further-nested spawned task
        // left a window where a `/jobs cancel <id>` issued immediately after
        // `/bg` could run before the handle was registered and spuriously
        // report "no such running job".
        let inner_chat_id = chat_id.clone();
        let inner_query = query.clone();
        let join_handle = tokio::spawn(async move {
            let approve_cb = |_approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                Ok(ApprovalOutcome::Denied)
            };
            let progress_cb = |_progress| {};
            let on_chunk = |_chunk: String| {};

            orchestrate_agent_loop(
                &config,
                &inner_query,
                &root,
                None,
                None,
                None,
                Some(&inner_chat_id),
                None,
                None,
                None,
                false,
                false,
                approve_cb,
                progress_cb,
                on_chunk,
            )
            .await
        });

        let abort_handle = join_handle.abort_handle();
        mint_core::ACTIVE_AGENTS
            .lock()
            .unwrap()
            .insert(chat_id.clone(), abort_handle);

        tokio::spawn(async move {
            let outcome = join_handle.await;

            mint_core::ACTIVE_AGENTS.lock().unwrap().remove(&chat_id);

            let (status, result) = match outcome {
                Ok(Ok(agent_result)) => (JobStatus::Done, Some(agent_result.summary)),
                Ok(Err(error)) => (JobStatus::Failed, Some(error.to_string())),
                Err(join_error) if join_error.is_cancelled() => (JobStatus::Cancelled, None),
                Err(join_error) => (JobStatus::Failed, Some(join_error.to_string())),
            };

            jobs.finish(id, status, result, &query);
        });

        id
    }

    fn finish(&self, id: u32, status: JobStatus, result: Option<String>, query: &str) {
        let mut inner = self.inner.lock().unwrap();
        if let Some(job) = inner.jobs.iter_mut().find(|j| j.id == id) {
            job.finished_secs = Some(job.started_at.elapsed().as_secs());
            job.status = status;
            job.result = result;
        }
        let preview: String = query.chars().take(50).collect();
        let suffix = if query.chars().count() > 50 {
            "…"
        } else {
            ""
        };
        inner
            .notices
            .push(format!("[bg #{id}] {status} — {preview}{suffix}"));
    }

    /// Snapshot of all jobs, most recently started first.
    pub fn list(&self) -> Vec<JobRecord> {
        let inner = self.inner.lock().unwrap();
        let mut jobs: Vec<JobRecord> = inner.jobs.clone();
        jobs.reverse();
        jobs
    }

    pub fn show(&self, id: u32) -> Option<JobRecord> {
        let inner = self.inner.lock().unwrap();
        inner.jobs.iter().find(|j| j.id == id).cloned()
    }

    /// Cancel a running job. Returns `false` if no such running job exists.
    pub fn cancel(&self, id: u32) -> bool {
        let is_running = {
            let inner = self.inner.lock().unwrap();
            inner
                .jobs
                .iter()
                .any(|j| j.id == id && j.status == JobStatus::Running)
        };
        if !is_running {
            return false;
        }
        mint_core::cancel_agent(&Self::chat_id_for(id))
    }

    /// Drain any "job finished" notices for the foreground loop to print.
    pub fn take_notices(&self) -> Vec<String> {
        let mut inner = self.inner.lock().unwrap();
        std::mem::take(&mut inner.notices)
    }
}

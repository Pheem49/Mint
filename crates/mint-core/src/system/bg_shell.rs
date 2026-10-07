//! Tracked background shell jobs.
//!
//! `crate::shell::run_shell_command` always blocks until the process exits,
//! which is fine for quick commands but useless for a long-running one (a
//! dev server, a watcher, ...). This module lets the agent start such a
//! command, keep working, and later poll its buffered stdout/stderr or kill
//! it, via the `run_shell(background: true)` / `shell_output` / `kill_shell`
//! tools wired up in `orchestration.rs`.

use std::{
    cell::RefCell,
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, LazyLock, Mutex},
    time::{Duration, Instant},
};

use crate::{
    Capability, MintConfig, SafetyTier, assert_path_capability, classify_shell_command,
    shell::ShellError, shell_mode_allowed,
};

/// Cap on how much output a single stream (stdout or stderr) retains per
/// job. Oldest bytes are dropped once exceeded, mirroring the truncation
/// convention `orchestration.rs` already uses for tool observations.
const MAX_BUF_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone)]
pub enum JobStatus {
    Running,
    Stopping,
    Exited(i32),
    Killed,
    Failed(String),
}

/// Growable buffer that tracks how much of itself has already been handed
/// back by [`JobRecord`] polling, and caps total retained size so a chatty
/// background process can't grow memory unbounded.
pub(crate) struct CappedBuf {
    pub(crate) data: String,
    read_offset: usize,
    truncated: bool,
}

impl CappedBuf {
    pub(crate) fn new() -> Self {
        Self {
            data: String::new(),
            read_offset: 0,
            truncated: false,
        }
    }

    pub(crate) fn push(&mut self, chunk: &str) {
        self.data.push_str(chunk);
        if self.data.len() > MAX_BUF_BYTES {
            let mut cut = self.data.len() - MAX_BUF_BYTES;
            while !self.data.is_char_boundary(cut) {
                cut += 1;
            }
            self.data.drain(..cut);
            self.read_offset = self.read_offset.saturating_sub(cut);
            self.truncated = true;
        }
    }

    /// Returns everything appended since the last call, and advances the
    /// read cursor so a later call only returns what's new since this one.
    fn drain_new(&mut self) -> (String, bool) {
        let new = self.data[self.read_offset..].to_string();
        self.read_offset = self.data.len();
        (new, self.truncated)
    }
}

pub struct JobRecord {
    id: String,
    command: String,
    cwd: PathBuf,
    pid: Option<u32>,
    status: Mutex<JobStatus>,
    stdout: Arc<Mutex<CappedBuf>>,
    stderr: Arc<Mutex<CappedBuf>>,
    started_at: Instant,
    started_ms: i64,
    ended_ms: Mutex<Option<i64>>,
    context: JobContext,
    server_url: Mutex<Option<String>>,
    stop_complete: std::sync::atomic::AtomicBool,
    stop_error: Mutex<Option<String>>,
    managed_stop: Option<Arc<std::sync::atomic::AtomicBool>>,
}

pub struct StartedJob {
    pub id: String,
    pub pid: Option<u32>,
    pub cwd: PathBuf,
}

/// One row of `list_jobs()` — enough to render a `/shells` listing without
/// touching any job's read cursor.
pub struct JobSummary {
    pub id: String,
    pub command: String,
    pub status: JobStatus,
    pub pid: Option<u32>,
    pub elapsed_secs: u64,
    pub cwd: PathBuf,
    pub workspace: PathBuf,
    pub server_url: Option<String>,
}

/// Full accumulated stdout/stderr for one job, for `/shells show <id>`.
/// Unlike [`poll_output`], this is a read-only peek: it never advances the
/// read cursor `poll_output`/`shell_output` rely on, so a human looking at a
/// job doesn't cause the agent to miss output it hasn't polled yet.
pub struct JobSnapshot {
    pub id: String,
    pub command: String,
    pub cwd: PathBuf,
    pub status: JobStatus,
    pub pid: Option<u32>,
    pub stdout: String,
    pub stderr: String,
    pub truncated: bool,
    pub elapsed_secs: u64,
    pub server_url: Option<String>,
}

static BG_SHELL_JOBS: LazyLock<Mutex<HashMap<String, Arc<JobRecord>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static NEXT_JOB_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));

fn next_job_id() -> String {
    let mut n = NEXT_JOB_ID.lock().unwrap();
    *n += 1;
    format!("bg-{}-{n}", &BACKEND_ID[..8])
}

static SHUTTING_DOWN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
pub struct ShutdownGuard;
impl Drop for ShutdownGuard {
    fn drop(&mut self) {
        shutdown();
    }
}
pub fn is_shutting_down() -> bool {
    SHUTTING_DOWN.load(std::sync::atomic::Ordering::Acquire)
}
static BACKEND_ID: LazyLock<String> = LazyLock::new(|| uuid::Uuid::new_v4().to_string());
static CHAT_OWNERS: LazyLock<Mutex<HashMap<String, Option<String>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
#[derive(Clone, Default)]
pub struct JobContext {
    pub workspace: PathBuf,
    pub chat_id: Option<String>,
    pub owner: Option<String>,
}
thread_local! { static JOB_CONTEXT: RefCell<JobContext> = RefCell::new(JobContext::default()); }
pub fn with_context<T>(context: JobContext, run: impl FnOnce() -> T) -> T {
    struct Restore(JobContext);
    impl Drop for Restore {
        fn drop(&mut self) {
            JOB_CONTEXT.with(|c| c.replace(self.0.clone()));
        }
    }
    let previous = JOB_CONTEXT.with(|c| c.replace(context));
    let _restore = Restore(previous);
    run()
}
fn normalized_chat(chat: &str) -> &str {
    let chat = chat.trim();
    if chat.is_empty() {
        crate::DEFAULT_CONVERSATION_ID
    } else {
        chat
    }
}
pub fn bind_chat_owner(chat: &str, owner: Option<String>) -> bool {
    let chat = normalized_chat(chat);
    let mut owners = CHAT_OWNERS.lock().unwrap();
    let parent = chat.split("::subagent::").next().unwrap_or(chat);
    if let Some(existing) = owners.get(parent).or_else(|| owners.get(chat)) {
        return existing == &owner;
    }
    owners.insert(chat.to_owned(), owner);
    true
}
pub fn chat_context(root: &Path, chat: &str) -> JobContext {
    let chat = normalized_chat(chat);
    let owners = CHAT_OWNERS.lock().unwrap();
    let parent = chat.split("::subagent::").next().unwrap_or(chat);
    let owner = owners
        .get(parent)
        .or_else(|| owners.get(chat))
        .cloned()
        .flatten();
    JobContext {
        workspace: root.to_path_buf(),
        chat_id: Some(chat.to_owned()),
        owner,
    }
}
pub fn accessible(job_id: &str, owner: &Option<String>) -> bool {
    BG_SHELL_JOBS
        .lock()
        .unwrap()
        .get(job_id)
        .is_some_and(|job| &job.context.owner == owner)
}
fn context_for(cwd: &Path) -> JobContext {
    let mut context = JOB_CONTEXT.with(|c| c.borrow().clone());
    if context.workspace.as_os_str().is_empty() {
        context.workspace = cwd.to_path_buf();
    }
    context
}
fn register(job: Arc<JobRecord>) {
    let mut jobs = BG_SHELL_JOBS.lock().unwrap();
    jobs.insert(job.id.clone(), job);
    let mut finished: Vec<_> = jobs
        .values()
        .filter(|j| {
            !matches!(
                *j.status.lock().unwrap(),
                JobStatus::Running | JobStatus::Stopping
            )
        })
        .map(|j| (j.started_at, j.id.clone()))
        .collect();
    finished.sort();
    let remove = finished.len().saturating_sub(100);
    for (_, id) in finished.into_iter().take(remove) {
        jobs.remove(&id);
    }
}
/// Immutable metadata and output for clients; never consumes the agent's cursor.
pub fn job_json(id: &str, include_output: bool) -> Result<serde_json::Value, ShellError> {
    let job = BG_SHELL_JOBS
        .lock()
        .unwrap()
        .get(id)
        .cloned()
        .ok_or_else(|| ShellError::JobNotFound(id.into()))?;
    let status = job.status.lock().unwrap().clone();
    let (state, code, error) = match status {
        JobStatus::Running => ("running", None, None),
        JobStatus::Stopping => ("stopping", None, None),
        JobStatus::Exited(0) => ("completed", Some(0), None),
        JobStatus::Exited(code) => ("failed", Some(code), None),
        JobStatus::Killed => ("stopped", None, None),
        JobStatus::Failed(error) => ("failed", None, Some(error)),
    };
    let ended = *job.ended_ms.lock().unwrap();
    let mut value = serde_json::json!({ "id": job.id, "managedService": job.managed_stop.is_some(), "command": job.command, "cwd": job.cwd, "workspacePath": job.context.workspace, "chatId": job.context.chat_id, "pid": job.pid, "status": state, "exitCode": code, "error": error, "startedAt": job.started_ms, "endedAt": ended, "elapsedSeconds": (ended.unwrap_or_else(|| chrono::Utc::now().timestamp_millis()) - job.started_ms).max(0) / 1000, "serverUrl": if state == "running" { job.server_url.lock().unwrap().clone() } else { None } });
    if include_output {
        let stdout = job.stdout.lock().unwrap();
        let stderr = job.stderr.lock().unwrap();
        value["stdout"] = serde_json::json!(stdout.data);
        value["stderr"] = serde_json::json!(stderr.data);
        value["truncated"] = serde_json::json!(stdout.truncated || stderr.truncated);
    }
    Ok(value)
}
pub fn list_json(owner: &Option<String>, workspace: Option<&Path>) -> Vec<serde_json::Value> {
    let ids: Vec<_> = BG_SHELL_JOBS
        .lock()
        .unwrap()
        .values()
        .filter(|j| &j.context.owner == owner && workspace.is_none_or(|w| j.context.workspace == w))
        .map(|j| j.id.clone())
        .collect();
    let mut jobs: Vec<_> = ids
        .iter()
        .filter_map(|id| job_json(id, false).ok())
        .collect();
    jobs.sort_by_key(|j| {
        (
            j["startedAt"].as_i64().unwrap_or(0),
            j["id"]
                .as_str()
                .and_then(|id| id.rsplit('-').next())
                .and_then(|n| n.parse::<u64>().ok())
                .unwrap_or(0),
        )
    });
    jobs
}
pub fn running_count_for(workspace: &Path) -> usize {
    BG_SHELL_JOBS
        .lock()
        .unwrap()
        .values()
        .filter(|j| {
            j.context.workspace == workspace
                && matches!(
                    *j.status.lock().unwrap(),
                    JobStatus::Running | JobStatus::Stopping
                )
        })
        .count()
}
pub fn shutdown() {
    SHUTTING_DOWN.store(true, std::sync::atomic::Ordering::Release);
    let ids: Vec<_> = BG_SHELL_JOBS.lock().unwrap().keys().cloned().collect();
    for id in &ids {
        let _ = kill_job(id);
    }
    let deadline = Instant::now() + Duration::from_secs(3);
    while running_count() > 0 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(25));
    }
}

/// Plain-text notices ("job X exited with code 0") a UI surface can drain,
/// same shape as [`crate::LINKED_FOLDER_NOTICES`].
static FINISHED_NOTICES: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));

pub fn take_finished_notices() -> Vec<String> {
    std::mem::take(&mut *FINISHED_NOTICES.lock().unwrap())
}

fn push_finished_notice(job: &JobRecord, status: &JobStatus) {
    let message = match status {
        JobStatus::Exited(code) => format!(
            "Background job {} (`{}`) exited with code {code}",
            job.id, job.command
        ),
        JobStatus::Failed(err) => format!(
            "Background job {} (`{}`) failed: {err}",
            job.id, job.command
        ),
        // Killed jobs were stopped intentionally (via kill_shell), and
        // Running is not a terminal state — neither needs a surprise notice.
        JobStatus::Killed | JobStatus::Running | JobStatus::Stopping => return,
    };
    FINISHED_NOTICES.lock().unwrap().push(message);
}

enum StreamKind {
    Stdout,
    Stderr,
}

pub(crate) fn read_stream(mut pipe: impl Read, buffer: &Arc<Mutex<CappedBuf>>) {
    let mut chunk = [0u8; 4096];
    let mut pending = Vec::new();
    loop {
        match pipe.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => pending.extend_from_slice(&chunk[..n]),
        }
        loop {
            match std::str::from_utf8(&pending) {
                Ok(text) => {
                    buffer.lock().unwrap().push(text);
                    pending.clear();
                    break;
                }
                Err(error) => {
                    let valid = error.valid_up_to();
                    buffer
                        .lock()
                        .unwrap()
                        .push(std::str::from_utf8(&pending[..valid]).unwrap());
                    pending.drain(..valid);
                    if let Some(invalid) = error.error_len() {
                        buffer.lock().unwrap().push("�");
                        pending.drain(..invalid);
                    } else {
                        break;
                    }
                }
            }
        }
    }
    if !pending.is_empty() {
        buffer
            .lock()
            .unwrap()
            .push(&String::from_utf8_lossy(&pending));
    }
}
fn spawn_pump(pipe: impl Read + Send + 'static, job: Arc<JobRecord>, kind: StreamKind) {
    std::thread::spawn(move || {
        let buffer = match kind {
            StreamKind::Stdout => &job.stdout,
            StreamKind::Stderr => &job.stderr,
        };
        read_stream(pipe, buffer);
    });
}

/// Track a built-in local service using the same lifetime and Stop controls as shell jobs.
pub(crate) fn start_local_service(
    root: PathBuf,
    owner: Option<String>,
    url: String,
    run: impl FnOnce(Arc<std::sync::atomic::AtomicBool>) -> Result<(), String> + Send + 'static,
) -> Result<String, String> {
    if is_shutting_down() {
        return Err("Mint backend is shutting down".into());
    }
    let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let job = Arc::new(JobRecord {
        id: next_job_id(),
        command: format!("HTML preview server · {}", root.display()),
        cwd: root.clone(),
        pid: None,
        status: Mutex::new(JobStatus::Running),
        stdout: Arc::new(Mutex::new(CappedBuf::new())),
        stderr: Arc::new(Mutex::new(CappedBuf::new())),
        started_at: Instant::now(),
        started_ms: chrono::Utc::now().timestamp_millis(),
        ended_ms: Mutex::new(None),
        context: JobContext {
            workspace: root,
            chat_id: None,
            owner,
        },
        server_url: Mutex::new(Some(url.clone())),
        stop_complete: std::sync::atomic::AtomicBool::new(false),
        stop_error: Mutex::new(None),
        managed_stop: Some(Arc::clone(&stop)),
    });
    job.stdout
        .lock()
        .unwrap()
        .push(&format!("Serving HTML preview at {url}\n"));
    let id = job.id.clone();
    register(Arc::clone(&job));
    if is_shutting_down() {
        stop.store(true, std::sync::atomic::Ordering::Release);
    }
    std::thread::spawn(move || {
        let result =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(Arc::clone(&stop))))
                .unwrap_or_else(|_| Err("HTML preview server panicked".into()));
        let final_status = if stop.load(std::sync::atomic::Ordering::Acquire) {
            JobStatus::Killed
        } else {
            result.map_or_else(JobStatus::Failed, |_| JobStatus::Exited(0))
        };
        *job.status.lock().unwrap() = final_status.clone();
        *job.ended_ms.lock().unwrap() = Some(chrono::Utc::now().timestamp_millis());
        job.stop_complete
            .store(true, std::sync::atomic::Ordering::Release);
        push_finished_notice(&job, &final_status);
        register(job);
    });
    Ok(id)
}

/// Starts `command` detached from the blocking tool-call path and returns
/// immediately with its job id. Applies the same safety classification and
/// working-directory capability check as `run_shell_command`; does not go
/// through the OS sandbox wrapper (`run_in_sandbox`), since streaming a
/// sandboxed process's output live is out of scope for now.
pub fn start_background(
    cwd: &Path,
    config: &MintConfig,
    command: &str,
) -> Result<StartedJob, ShellError> {
    if is_shutting_down() {
        return Err(ShellError::Execute(std::io::Error::other(
            "Mint backend is shutting down",
        )));
    }
    let classification = classify_shell_command(command);
    if classification.tier == SafetyTier::Blocked {
        return Err(ShellError::Blocked {
            command: command.into(),
            reason: classification.reason,
        });
    }
    if config.safety_enabled && !shell_mode_allowed(config, classification.mode) {
        return Err(ShellError::ModeDenied {
            command: command.into(),
            mode: classification.mode.as_str().into(),
        });
    }

    let cwd = assert_path_capability(cwd, Capability::Write, config)?;
    if !cwd.is_dir() {
        return Err(ShellError::InvalidWorkingDirectory(cwd));
    }

    let mut cmd = crate::shell::shell_command(command);
    cmd.current_dir(&cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd.spawn()?;
    let pid = child.id();
    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");

    let job = Arc::new(JobRecord {
        id: next_job_id(),
        command: command.to_owned(),
        cwd: cwd.clone(),
        pid: Some(pid),
        status: Mutex::new(JobStatus::Running),
        stdout: Arc::new(Mutex::new(CappedBuf::new())),
        stderr: Arc::new(Mutex::new(CappedBuf::new())),
        started_at: Instant::now(),
        started_ms: chrono::Utc::now().timestamp_millis(),
        ended_ms: Mutex::new(None),
        context: context_for(&cwd),
        server_url: Mutex::new(None),
        stop_complete: std::sync::atomic::AtomicBool::new(false),
        stop_error: Mutex::new(None),
        managed_stop: None,
    });

    spawn_pump(stdout, Arc::clone(&job), StreamKind::Stdout);
    spawn_pump(stderr, Arc::clone(&job), StreamKind::Stderr);

    {
        let job = Arc::clone(&job);
        std::thread::spawn(move || {
            let job_for_wait = Arc::clone(&job);
            // Isolated so a panic here (an unexpectedly poisoned lock, ...) marks
            // the job Failed instead of leaving it stuck showing `Running`
            // forever in `/shells` with no explanation and — since `child` is
            // never `wait()`-ed on if the panic happens before that call — a
            // zombie process left behind.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                // Always wait, even if `kill_job` already marked this Killed —
                // otherwise the child becomes a zombie.
                let exit_status = child.wait();
                wait_stop_completion(&job_for_wait);
                let mut status = job_for_wait.status.lock().unwrap();
                let was_running = matches!(*status, JobStatus::Running | JobStatus::Stopping);
                let stopping = matches!(*status, JobStatus::Stopping);
                if was_running {
                    *status = match exit_status {
                        Ok(_) if stopping => job_for_wait
                            .stop_error
                            .lock()
                            .unwrap()
                            .clone()
                            .map_or(JobStatus::Killed, JobStatus::Failed),
                        Ok(code) => JobStatus::Exited(code.code().unwrap_or(-1)),
                        Err(e) => JobStatus::Failed(e.to_string()),
                    };
                }
                *job_for_wait.ended_ms.lock().unwrap() =
                    Some(chrono::Utc::now().timestamp_millis());
                let final_status = status.clone();
                drop(status);
                if was_running {
                    push_finished_notice(&job_for_wait, &final_status);
                    register(Arc::clone(&job_for_wait));
                }
            }));
            if let Err(payload) = result {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] bg_shell job reaper thread panicked: {message}");
                let mut status = job.status.lock().unwrap();
                if matches!(*status, JobStatus::Running | JobStatus::Stopping) {
                    *job.ended_ms.lock().unwrap() = Some(chrono::Utc::now().timestamp_millis());
                    *status = JobStatus::Failed(format!("reaper thread panicked: {message}"));
                }
            }
        });
    }

    let started = StartedJob {
        id: job.id.clone(),
        pid: job.pid,
        cwd,
    };
    spawn_readiness(Arc::clone(&job));
    register(job);
    if is_shutting_down() {
        let _ = kill_job(&started.id);
    }
    Ok(started)
}

/// Adopts a process that was running in the foreground `run_shell` path when
/// it hit `shell::SHELL_COMMAND_TIMEOUT`, instead of `shell::run_with_timeout`
/// killing it. `stdout_handle`/`stderr_handle` are the reader threads already
/// draining the child's pipes (their pipe fds were taken when the process was
/// spawned, so they can't be swapped for `spawn_pump`-style incremental
/// readers now) — this job's buffers only receive their combined output once
/// the process actually exits and the threads join, so unlike a job started
/// via `start_background`, there is no live streaming during this handoff.
/// The job is registered as `Running` immediately regardless, so it's never
/// invisible to `/shells` or the status bar even before that happens.
pub(crate) fn promote_timed_out(
    command: &str,
    cwd: PathBuf,
    mut child: Child,
    stdout_handle: std::thread::JoinHandle<Vec<u8>>,
    stderr_handle: std::thread::JoinHandle<Vec<u8>>,
    stdout_buffer: Arc<Mutex<CappedBuf>>,
    stderr_buffer: Arc<Mutex<CappedBuf>>,
) -> StartedJob {
    let pid = child.id();
    let job = Arc::new(JobRecord {
        id: next_job_id(),
        command: command.to_owned(),
        cwd: cwd.clone(),
        pid: Some(pid),
        status: Mutex::new(JobStatus::Running),
        stdout: stdout_buffer,
        stderr: stderr_buffer,
        started_at: Instant::now(),
        started_ms: chrono::Utc::now().timestamp_millis(),
        ended_ms: Mutex::new(None),
        context: context_for(&cwd),
        server_url: Mutex::new(None),
        stop_complete: std::sync::atomic::AtomicBool::new(false),
        stop_error: Mutex::new(None),
        managed_stop: None,
    });

    {
        let job = Arc::clone(&job);
        std::thread::spawn(move || {
            let job_for_wait = Arc::clone(&job);
            // See the matching comment in `start_background`: isolated so a
            // panic here marks the job Failed instead of leaving it stuck as
            // `Running` forever with a zombie child left un-`wait()`-ed.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                // Always wait, even if `kill_job` already marked this Killed —
                // otherwise the child becomes a zombie.
                let exit_status = child.wait();
                wait_stop_completion(&job_for_wait);
                let _ = stdout_handle.join();
                let _ = stderr_handle.join();
                let mut status = job_for_wait.status.lock().unwrap();
                let was_running = matches!(*status, JobStatus::Running | JobStatus::Stopping);
                let stopping = matches!(*status, JobStatus::Stopping);
                if was_running {
                    *status = match exit_status {
                        Ok(_) if stopping => job_for_wait
                            .stop_error
                            .lock()
                            .unwrap()
                            .clone()
                            .map_or(JobStatus::Killed, JobStatus::Failed),
                        Ok(code) => JobStatus::Exited(code.code().unwrap_or(-1)),
                        Err(e) => JobStatus::Failed(e.to_string()),
                    };
                }
                *job_for_wait.ended_ms.lock().unwrap() =
                    Some(chrono::Utc::now().timestamp_millis());
                let final_status = status.clone();
                drop(status);
                if was_running {
                    push_finished_notice(&job_for_wait, &final_status);
                    register(Arc::clone(&job_for_wait));
                }
            }));
            if let Err(payload) = result {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] bg_shell job reaper thread panicked: {message}");
                let mut status = job.status.lock().unwrap();
                if matches!(*status, JobStatus::Running | JobStatus::Stopping) {
                    *job.ended_ms.lock().unwrap() = Some(chrono::Utc::now().timestamp_millis());
                    *status = JobStatus::Failed(format!("reaper thread panicked: {message}"));
                }
            }
        });
    }

    let started = StartedJob {
        id: job.id.clone(),
        pid: job.pid,
        cwd,
    };
    spawn_readiness(Arc::clone(&job));
    register(job);
    if is_shutting_down() {
        let _ = kill_job(&started.id);
    }
    started
}

/// Returns buffered stdout/stderr produced since the last poll of this job,
/// plus its current status.
pub fn poll_output(job_id: &str) -> Result<String, ShellError> {
    let job = BG_SHELL_JOBS
        .lock()
        .unwrap()
        .get(job_id)
        .cloned()
        .ok_or_else(|| ShellError::JobNotFound(job_id.to_owned()))?;

    let (stdout_new, stdout_truncated) = job.stdout.lock().unwrap().drain_new();
    let (stderr_new, stderr_truncated) = job.stderr.lock().unwrap().drain_new();
    let status = job.status.lock().unwrap().clone();
    let truncated_note = if stdout_truncated || stderr_truncated {
        "\n[Note] Older output for this job was dropped to stay under the buffer cap; only recent output is retained."
    } else {
        ""
    };

    Ok(match status {
        JobStatus::Stopping => format!(
            "job_id: {}\nstatus: stopping\nstdout (new):\n{}\nstderr (new):\n{}{}",
            job.id, stdout_new, stderr_new, truncated_note
        ),
        JobStatus::Running => format!(
            "job_id: {}\nstatus: running (elapsed {}s)\ncwd: {}\nstdout (new):\n{}\nstderr (new):\n{}{}",
            job.id,
            job.started_at.elapsed().as_secs(),
            job.cwd.display(),
            stdout_new,
            stderr_new,
            truncated_note
        ),
        JobStatus::Exited(code) => format!(
            "exit: {}\njob_id: {}\nstatus: exited\nstdout (new):\n{}\nstderr (new):\n{}{}",
            code, job.id, stdout_new, stderr_new, truncated_note
        ),
        JobStatus::Killed => format!(
            "job_id: {}\nstatus: killed\nstdout (new):\n{}\nstderr (new):\n{}{}",
            job.id, stdout_new, stderr_new, truncated_note
        ),
        JobStatus::Failed(err) => format!(
            "job_id: {}\nstatus: failed ({err})\nstdout (new):\n{}\nstderr (new):\n{}{}",
            job.id, stdout_new, stderr_new, truncated_note
        ),
    })
}

/// Every tracked job, most-recently-started last — for a human-facing
/// `/shells` listing. Doesn't touch any job's read cursor.
pub fn list_jobs() -> Vec<JobSummary> {
    let mut jobs: Vec<JobSummary> = BG_SHELL_JOBS
        .lock()
        .unwrap()
        .values()
        .map(|job| {
            let status = job.status.lock().unwrap().clone();
            JobSummary {
                id: job.id.clone(),
                command: job.command.clone(),
                status: status.clone(),
                pid: job.pid,
                elapsed_secs: (job
                    .ended_ms
                    .lock()
                    .unwrap()
                    .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
                    - job.started_ms)
                    .max(0) as u64
                    / 1000,
                cwd: job.cwd.clone(),
                workspace: job.context.workspace.clone(),
                server_url: if matches!(status, JobStatus::Running) {
                    job.server_url.lock().unwrap().clone()
                } else {
                    None
                },
            }
        })
        .collect();
    jobs.sort_by_key(|job| {
        job.id
            .rsplit('-')
            .next()
            .and_then(|n| n.parse::<u64>().ok())
            .unwrap_or(0)
    });
    jobs
}

/// How many tracked jobs are currently still running — cheap enough to call
/// on every prompt redraw for a persistent status-bar indicator.
pub fn running_count() -> usize {
    BG_SHELL_JOBS
        .lock()
        .unwrap()
        .values()
        .filter(|job| {
            matches!(
                *job.status.lock().unwrap(),
                JobStatus::Running | JobStatus::Stopping
            )
        })
        .count()
}

/// Read-only peek at everything a job has produced so far, for `/shells show
/// <id>` — see [`JobSnapshot`] for why this doesn't drain like
/// [`poll_output`] does.
pub fn snapshot(job_id: &str) -> Result<JobSnapshot, ShellError> {
    let job = BG_SHELL_JOBS
        .lock()
        .unwrap()
        .get(job_id)
        .cloned()
        .ok_or_else(|| ShellError::JobNotFound(job_id.to_owned()))?;

    let status = job.status.lock().unwrap().clone();
    let stdout = job.stdout.lock().unwrap();
    let stderr = job.stderr.lock().unwrap();
    Ok(JobSnapshot {
        id: job.id.clone(),
        command: job.command.clone(),
        cwd: job.cwd.clone(),
        status: status.clone(),
        pid: job.pid,
        stdout: stdout.data.clone(),
        stderr: stderr.data.clone(),
        truncated: stdout.truncated || stderr.truncated,
        elapsed_secs: (job
            .ended_ms
            .lock()
            .unwrap()
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
            - job.started_ms)
            .max(0) as u64
            / 1000,
        server_url: if matches!(status, JobStatus::Running) {
            job.server_url.lock().unwrap().clone()
        } else {
            None
        },
    })
}

/// Verify a listening socket belongs to the tracked process group before probing it.
#[cfg(target_os = "linux")]
fn owns_port(pid: u32, port: u16) -> bool {
    let mut sockets = std::collections::HashSet::new();
    for table in ["/proc/net/tcp", "/proc/net/tcp6"] {
        for line in std::fs::read_to_string(table)
            .unwrap_or_default()
            .lines()
            .skip(1)
        {
            let fields: Vec<_> = line.split_whitespace().collect();
            if fields.len() > 9
                && fields[3] == "0A"
                && fields[1]
                    .rsplit(':')
                    .next()
                    .is_some_and(|p| u16::from_str_radix(p, 16).ok() == Some(port))
            {
                sockets.insert(format!("socket:[{}]", fields[9]));
            }
        }
    }
    if sockets.is_empty() {
        return false;
    }
    for entry in std::fs::read_dir("/proc").into_iter().flatten().flatten() {
        if entry.file_name().to_string_lossy().parse::<u32>().is_err() {
            continue;
        }
        let stat = std::fs::read_to_string(entry.path().join("stat")).unwrap_or_default();
        // After the parenthesized command: state, parent pid, process group.
        if !stat.rsplit_once(')').is_some_and(|(_, rest)| {
            rest.split_whitespace()
                .nth(2)
                .and_then(|g| g.parse::<u32>().ok())
                == Some(pid)
        }) {
            continue;
        }
        for fd in std::fs::read_dir(entry.path().join("fd"))
            .into_iter()
            .flatten()
            .flatten()
        {
            if std::fs::read_link(fd.path())
                .ok()
                .is_some_and(|target| sockets.contains(target.to_string_lossy().as_ref()))
            {
                return true;
            }
        }
    }
    false
}
#[cfg(target_os = "macos")]
fn owns_port(pid: u32, port: u16) -> bool {
    Command::new("lsof")
        .args([
            "-a",
            "-g",
            &pid.to_string(),
            &format!("-iTCP:{port}"),
            "-sTCP:LISTEN",
            "-t",
        ])
        .output()
        .is_ok_and(|r| r.status.success() && !r.stdout.is_empty())
}
#[cfg(target_os = "windows")]
fn owns_port(pid: u32, port: u16) -> bool {
    let script = format!(
        "$ids=@({pid}); $processes=Get-CimInstance Win32_Process; do {{ $previous=$ids.Count; $ids+=@($processes | Where-Object {{ $ids -contains $_.ParentProcessId -and $ids -notcontains $_.ProcessId }} | ForEach-Object {{ $_.ProcessId }}); }} while ($ids.Count -gt $previous); if (Get-NetTCPConnection -LocalPort {port} -State Listen -ErrorAction SilentlyContinue | Where-Object {{ $ids -contains $_.OwningProcess }}) {{ 'owned' }}"
    );
    Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .output()
        .is_ok_and(|r| r.status.success() && String::from_utf8_lossy(&r.stdout).contains("owned"))
}
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
fn owns_port(_pid: u32, _port: u16) -> bool {
    false
}

fn spawn_readiness(job: Arc<JobRecord>) {
    std::thread::spawn(move || {
        let pattern =
            regex::Regex::new(r"http://(?:localhost|127\.0\.0\.1|0\.0\.0\.0):([0-9]{1,5})")
                .unwrap();
        let python = regex::Regex::new(r"-m\s+http\.server(?:\s+(\d+))?").unwrap();
        let serving = regex::Regex::new(r"Serving HTTP on .* port (\d+)").unwrap();
        let command_port = python.captures(&job.command).and_then(|c| {
            c.get(1)
                .map_or(Some(8000), |p| p.as_str().parse::<u16>().ok())
        });
        loop {
            std::thread::sleep(Duration::from_secs(1));
            if !matches!(*job.status.lock().unwrap(), JobStatus::Running) {
                return;
            }
            let text = format!(
                "{}\n{}",
                job.stdout.lock().unwrap().data,
                job.stderr.lock().unwrap().data
            );
            let port = pattern
                .captures(&text)
                .and_then(|c| c[1].parse::<u16>().ok())
                .or_else(|| {
                    serving
                        .captures(&text)
                        .and_then(|c| c[1].parse::<u16>().ok())
                })
                .or(command_port);
            let Some(port) = port.filter(|p| *p > 0) else {
                continue;
            };
            let owned = job.pid.is_some_and(|pid| owns_port(pid, port));
            let mut verified = false;
            if owned {
                let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
                if let Ok(mut stream) =
                    std::net::TcpStream::connect_timeout(&addr, Duration::from_millis(300))
                {
                    let _ = stream.set_read_timeout(Some(Duration::from_millis(300)));
                    let _ = stream.set_write_timeout(Some(Duration::from_millis(300)));
                    if stream
                        .write_all(b"HEAD / HTTP/1.0\r\nHost: localhost\r\n\r\n")
                        .is_ok()
                    {
                        let mut response = [0; 128];
                        verified = stream
                            .read(&mut response)
                            .is_ok_and(|n| response[..n].starts_with(b"HTTP/"));
                    }
                }
            }
            *job.server_url.lock().unwrap() = verified.then(|| format!("http://127.0.0.1:{port}"));
        }
    });
}

fn wait_stop_completion(job: &JobRecord) {
    if matches!(*job.status.lock().unwrap(), JobStatus::Stopping) {
        while !job.stop_complete.load(std::sync::atomic::Ordering::Acquire) {
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Sends a termination signal to a running job's whole process group
/// (SIGTERM, escalating to SIGKILL shortly after if it's still alive; on
/// Windows, `taskkill /T` kills the whole process tree directly). Does not
/// block waiting for the process to actually exit — the job's waiter thread
/// observes that and finalizes its status.
pub fn kill_job(job_id: &str) -> Result<String, ShellError> {
    let job = BG_SHELL_JOBS
        .lock()
        .unwrap()
        .get(job_id)
        .cloned()
        .ok_or_else(|| ShellError::JobNotFound(job_id.to_owned()))?;

    let mut status = job.status.lock().unwrap();
    if !matches!(*status, JobStatus::Running) {
        return Ok(format!(
            "job_id: {} is already {:?}, nothing to stop",
            job.id, *status
        ));
    }
    if let Some(stop) = &job.managed_stop {
        *status = JobStatus::Stopping;
        stop.store(true, std::sync::atomic::Ordering::Release);
        return Ok(format!("job_id: {job_id} stop requested"));
    }
    let Some(pid) = job.pid else {
        return Err(ShellError::JobNotFound(job_id.into()));
    };
    #[cfg(not(target_os = "windows"))]
    let result = Command::new("kill")
        .args(["-TERM", "--", &format!("-{pid}")])
        .output();
    #[cfg(target_os = "windows")]
    let result = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &pid.to_string()])
        .output();
    let result = result?;
    if !result.status.success() {
        return Err(ShellError::Execute(std::io::Error::other(
            String::from_utf8_lossy(&result.stderr).to_string(),
        )));
    }
    *status = JobStatus::Stopping;
    drop(status);
    #[cfg(target_os = "windows")]
    job.stop_complete
        .store(true, std::sync::atomic::Ordering::Release);
    #[cfg(not(target_os = "windows"))]
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        // The shell may already have exited while a child ignores TERM.
        // Escalate the original process group before the reaper reports Stopped.
        let result = Command::new("kill")
            .args(["-KILL", "--", &format!("-{pid}")])
            .output();
        if result.as_ref().map_or(true, |r| !r.status.success())
            && Command::new("kill")
                .args(["-0", "--", &format!("-{pid}")])
                .output()
                .is_ok_and(|r| r.status.success())
        {
            *job.stop_error.lock().unwrap() =
                Some("Unable to terminate the background process group".into());
        }
        job.stop_complete
            .store(true, std::sync::atomic::Ordering::Release);
    });
    Ok(format!("job_id: {job_id} stop requested (pid {pid})"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::MintConfig;

    fn local_config() -> MintConfig {
        MintConfig {
            safety_enabled: false,
            sandbox_mode: "off".into(),
            ..MintConfig::default()
        }
    }

    /// Polls until `predicate` matches or `timeout` elapses, returning the
    /// last polled output. Avoids flaking under a loaded parallel test run,
    /// where a fixed sleep can be too short.
    fn poll_until(job_id: &str, timeout: Duration, predicate: impl Fn(&str) -> bool) -> String {
        let deadline = Instant::now() + timeout;
        loop {
            let output = poll_output(job_id).unwrap();
            if predicate(&output) || Instant::now() >= deadline {
                return output;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    #[test]
    fn background_job_reports_running_then_exit() {
        let config = local_config();
        let started = start_background(Path::new("."), &config, "sleep 0.1 && echo hello").unwrap();
        assert!(started.pid.is_some());

        let after = poll_until(&started.id, Duration::from_secs(10), |out| {
            out.contains("exit: 0")
        });
        assert!(after.contains("exit: 0"), "got: {after}");
        assert!(after.contains("hello"), "got: {after}");
    }

    #[test]
    fn kill_job_stops_a_running_command() {
        let config = local_config();
        let started = start_background(Path::new("."), &config, "sleep 30").unwrap();

        let result = kill_job(&started.id).unwrap();
        assert!(result.contains("stop requested"));

        let after = poll_until(&started.id, Duration::from_secs(10), |out| {
            out.contains("status: killed") || out.contains("status: exited")
        });
        assert!(after.contains("status: killed") || after.contains("status: exited"));
    }

    struct TestDir(PathBuf);
    impl TestDir {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!("mint-bg-test-{}", uuid::Uuid::new_v4()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for TestDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    struct StopOnDrop(String);
    impl Drop for StopOnDrop {
        fn drop(&mut self) {
            let _ = kill_job(&self.0);
        }
    }
    fn wait_json(id: &str, predicate: impl Fn(&serde_json::Value) -> bool) -> serde_json::Value {
        let deadline = Instant::now() + Duration::from_secs(8);
        loop {
            let value = job_json(id, true).unwrap();
            if predicate(&value) {
                return value;
            }
            assert!(
                Instant::now() < deadline,
                "job did not reach expected state: {value}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }
    #[test]
    fn normalized_and_subagent_chats_keep_parent_ownership() {
        let chat = uuid::Uuid::new_v4().to_string();
        let owner = Some("parent-owner".to_owned());
        assert!(bind_chat_owner(&format!("  {chat}  "), owner.clone()));
        let child = format!("{chat}::subagent::helper");
        assert_eq!(chat_context(Path::new("."), &child).owner, owner);
        assert!(!bind_chat_owner(&child, Some("other-owner".into())));
        assert_eq!(
            chat_context(Path::new("."), &format!(" {chat} ")).chat_id,
            Some(chat)
        );
    }

    #[test]
    fn counts_and_context_are_workspace_and_owner_scoped() {
        let root = TestDir::new();
        let other = TestDir::new();
        let chat = uuid::Uuid::new_v4().to_string();
        assert!(bind_chat_owner(&chat, Some("owner-test".into())));
        assert!(!bind_chat_owner(&chat, Some("different-owner".into())));
        let start = Instant::now();
        let job = with_context(chat_context(root.path(), &chat), || {
            start_background(other.path(), &local_config(), "sleep 30")
        })
        .unwrap();
        let _cleanup = StopOnDrop(job.id.clone());
        assert!(start.elapsed() < Duration::from_secs(2));
        assert_eq!(running_count_for(root.path()), 1);
        assert_eq!(running_count_for(other.path()), 0);
        assert!(accessible(&job.id, &Some("owner-test".into())));
        assert!(!accessible(&job.id, &None));
        assert_eq!(
            list_json(&Some("owner-test".into()), Some(root.path())).len(),
            1
        );
        let info = job_json(&job.id, false).unwrap();
        assert_eq!(info["cwd"], other.path().to_string_lossy().as_ref());
        assert_eq!(info["chatId"], chat);
        assert_eq!(info["id"].as_str().unwrap().split('-').count(), 3);
        kill_job(&job.id).unwrap();
        kill_job(&job.id).unwrap();
        let value = wait_json(&job.id, |v| v["status"] == "stopped");
        assert!(value["endedAt"].is_i64());
        assert_eq!(running_count_for(root.path()), 0);
    }
    #[test]
    fn snapshots_never_consume_incremental_output_and_exit_codes_are_retained() {
        let job = start_background(
            Path::new("."),
            &local_config(),
            "printf hello; printf problem >&2; exit 7",
        )
        .unwrap();
        let value = wait_json(&job.id, |v| {
            v["status"] == "failed" && v["stdout"] == "hello" && v["stderr"] == "problem"
        });
        assert_eq!(value["exitCode"], 7);
        assert_eq!(snapshot(&job.id).unwrap().stdout, "hello");
        assert!(poll_output(&job.id).unwrap().contains("hello"));
        assert!(!poll_output(&job.id).unwrap().contains("hello"));
        assert_eq!(job_json(&job.id, true).unwrap()["stdout"], "hello");
    }
    #[test]
    fn capped_output_preserves_utf8_and_reports_truncation() {
        let mut buffer = CappedBuf::new();
        buffer.push(&"ก".repeat(MAX_BUF_BYTES));
        assert!(buffer.data.len() <= MAX_BUF_BYTES);
        assert!(buffer.truncated);
        assert!(buffer.drain_new().1);
    }
    #[test]
    fn promoted_output_streams_before_exit_and_retains_context() {
        let root = TestDir::new();
        let command = "printf promoted-ready; sleep 30";
        let context = JobContext {
            workspace: root.path().into(),
            chat_id: Some("promoted-test".into()),
            owner: Some("promotion-owner".into()),
        };
        let result = with_context(context, || {
            crate::shell::run_with_timeout(
                crate::shell::shell_command(command),
                Duration::from_millis(100),
                command,
                root.path(),
            )
        })
        .unwrap();
        assert!(result.status_code.is_none());
        let job = list_json(&Some("promotion-owner".into()), Some(root.path()))
            .pop()
            .unwrap();
        let id = job["id"].as_str().unwrap();
        let _cleanup = StopOnDrop(id.to_string());
        let value = wait_json(id, |v| v["stdout"] == "promoted-ready");
        assert_eq!(value["status"], "running");
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn stop_terminates_children_in_the_job_group() {
        let job = start_background(
            Path::new("."),
            &local_config(),
            "sh -c 'trap \"\" TERM; echo $$; sleep 30' & wait",
        )
        .unwrap();
        let _cleanup = StopOnDrop(job.id.clone());
        let value = wait_json(&job.id, |v| {
            v["stdout"].as_str().is_some_and(|s| !s.trim().is_empty())
        });
        let pid: u32 = value["stdout"].as_str().unwrap().trim().parse().unwrap();
        kill_job(&job.id).unwrap();
        wait_json(&job.id, |v| v["status"] == "stopped");
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).unwrap_or_default();
        assert!(
            stat.is_empty()
                || stat
                    .rsplit_once(')')
                    .unwrap()
                    .1
                    .trim_start()
                    .starts_with('Z')
        );
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn local_server_readiness_is_verified_and_occupied_ports_are_rejected() {
        if !crate::config::which("python3") {
            return;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let job = start_background(
            Path::new("."),
            &local_config(),
            &format!("python3 -u -m http.server {port} --bind 127.0.0.1"),
        )
        .unwrap();
        let failed = wait_json(&job.id, |v| v["status"] == "failed");
        assert!(failed["serverUrl"].is_null());
        drop(listener);
        let job = start_background(
            Path::new("."),
            &local_config(),
            &format!("sleep 0.2; python3 -u -m http.server {port} --bind 127.0.0.1"),
        )
        .unwrap();
        let _cleanup = StopOnDrop(job.id.clone());
        let ready = wait_json(&job.id, |v| v["serverUrl"].is_string());
        assert_eq!(ready["serverUrl"], format!("http://127.0.0.1:{port}"));
        kill_job(&job.id).unwrap();
        let stopped = wait_json(&job.id, |v| v["status"] == "stopped");
        assert!(stopped["serverUrl"].is_null());
    }

    #[test]
    fn poll_unknown_job_errors() {
        assert!(matches!(
            poll_output("bg-does-not-exist"),
            Err(ShellError::JobNotFound(_))
        ));
    }
}

/// Human-facing slash output shared by Desktop and Web; CLI keeps its styled renderer.
pub fn shells_command(
    rest: &str,
    workspace: &Path,
    owner: &Option<String>,
    config: &MintConfig,
) -> Result<String, ShellError> {
    if rest.is_empty() || rest == "all" {
        assert_path_capability(workspace, Capability::Read, config)?;
        let jobs = list_json(owner, if rest == "all" { None } else { Some(workspace) });
        let mut lines = vec!["Background terminals".to_string()];
        for job in jobs {
            let path = job["workspacePath"].as_str().unwrap_or("");
            if assert_path_capability(Path::new(path), Capability::Read, config).is_err() {
                continue;
            }
            lines.push(format!(
                "{} · {} · {}s · {}\n{}",
                job["id"].as_str().unwrap_or(""),
                job["status"].as_str().unwrap_or(""),
                job["elapsedSeconds"],
                path,
                job["command"].as_str().unwrap_or("")
            ));
            if let Some(url) = job["serverUrl"].as_str() {
                lines.push(format!("Ready URL: {url}"));
            }
        }
        if lines.len() == 1 {
            lines.push("No background terminals in this workspace.".into());
        }
        return Ok(lines.join("\n\n"));
    }
    let (action, id) = rest
        .split_once(char::is_whitespace)
        .map(|(a, i)| (a, i.trim()))
        .unwrap_or((rest, ""));
    if !matches!(action, "show" | "stop" | "kill") || id.is_empty() {
        return Ok("Usage: /shells [all|show <id>|stop <id>|kill <id>]".into());
    }
    if !accessible(id, owner) {
        return Err(ShellError::JobNotFound(id.into()));
    }
    let job = job_json(id, action == "show")?;
    let path = Path::new(job["workspacePath"].as_str().unwrap_or(""));
    assert_path_capability(
        path,
        if action == "show" || job["managedService"] == true {
            Capability::Read
        } else {
            Capability::Write
        },
        config,
    )?;
    if action != "show" {
        return kill_job(id);
    }
    let readiness = job["serverUrl"]
        .as_str()
        .map(|url| format!("Ready URL: {url}\n"))
        .unwrap_or_default();
    Ok(format!(
        "{readiness}{} · {} · {}s\nWorkspace: {}\nCommand: {}\n{}\nstdout:\n{}\nstderr:\n{}",
        id,
        job["status"].as_str().unwrap_or(""),
        job["elapsedSeconds"],
        path.display(),
        job["command"].as_str().unwrap_or(""),
        if job["truncated"] == true {
            "[Older output truncated]"
        } else {
            ""
        },
        job["stdout"].as_str().unwrap_or(""),
        job["stderr"].as_str().unwrap_or("")
    ))
}

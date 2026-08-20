//! Tracked background shell jobs.
//!
//! `crate::shell::run_shell_command` always blocks until the process exits,
//! which is fine for quick commands but useless for a long-running one (a
//! dev server, a watcher, ...). This module lets the agent start such a
//! command, keep working, and later poll its buffered stdout/stderr or kill
//! it, via the `run_shell(background: true)` / `shell_output` / `kill_shell`
//! tools wired up in `orchestration.rs`.

use std::{
    collections::HashMap,
    io::Read,
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
    Exited(i32),
    Killed,
    Failed(String),
}

/// Growable buffer that tracks how much of itself has already been handed
/// back by [`JobRecord`] polling, and caps total retained size so a chatty
/// background process can't grow memory unbounded.
struct CappedBuf {
    data: String,
    read_offset: usize,
    truncated: bool,
}

impl CappedBuf {
    fn new() -> Self {
        Self {
            data: String::new(),
            read_offset: 0,
            truncated: false,
        }
    }

    fn push(&mut self, chunk: &str) {
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
    stdout: Mutex<CappedBuf>,
    stderr: Mutex<CappedBuf>,
    started_at: Instant,
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
}

static BG_SHELL_JOBS: LazyLock<Mutex<HashMap<String, Arc<JobRecord>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static NEXT_JOB_ID: LazyLock<Mutex<u32>> = LazyLock::new(|| Mutex::new(0));

fn next_job_id() -> String {
    let mut n = NEXT_JOB_ID.lock().unwrap();
    *n += 1;
    format!("bg-{n}")
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
        JobStatus::Killed | JobStatus::Running => return,
    };
    FINISHED_NOTICES.lock().unwrap().push(message);
}

enum StreamKind {
    Stdout,
    Stderr,
}

fn spawn_pump(mut pipe: impl Read + Send + 'static, job: Arc<JobRecord>, kind: StreamKind) {
    std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        loop {
            match pipe.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&chunk[..n]);
                    let mut buf = match kind {
                        StreamKind::Stdout => job.stdout.lock().unwrap(),
                        StreamKind::Stderr => job.stderr.lock().unwrap(),
                    };
                    buf.push(&text);
                }
            }
        }
    });
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
        stdout: Mutex::new(CappedBuf::new()),
        stderr: Mutex::new(CappedBuf::new()),
        started_at: Instant::now(),
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
                let mut status = job_for_wait.status.lock().unwrap();
                let was_running = matches!(*status, JobStatus::Running);
                if was_running {
                    *status = match exit_status {
                        Ok(code) => JobStatus::Exited(code.code().unwrap_or(-1)),
                        Err(e) => JobStatus::Failed(e.to_string()),
                    };
                }
                let final_status = status.clone();
                drop(status);
                if was_running {
                    push_finished_notice(&job_for_wait, &final_status);
                }
            }));
            if let Err(payload) = result {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] bg_shell job reaper thread panicked: {message}");
                let mut status = job.status.lock().unwrap();
                if matches!(*status, JobStatus::Running) {
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
    BG_SHELL_JOBS
        .lock()
        .unwrap()
        .insert(started.id.clone(), job);
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
pub fn promote_timed_out(
    command: &str,
    cwd: PathBuf,
    mut child: Child,
    stdout_handle: std::thread::JoinHandle<Vec<u8>>,
    stderr_handle: std::thread::JoinHandle<Vec<u8>>,
) -> StartedJob {
    let pid = child.id();
    let job = Arc::new(JobRecord {
        id: next_job_id(),
        command: command.to_owned(),
        cwd: cwd.clone(),
        pid: Some(pid),
        status: Mutex::new(JobStatus::Running),
        stdout: Mutex::new(CappedBuf::new()),
        stderr: Mutex::new(CappedBuf::new()),
        started_at: Instant::now(),
    });

    {
        let job = Arc::clone(&job);
        std::thread::spawn(move || {
            let job_for_wait = Arc::clone(&job);
            // See the matching comment in `start_background`: isolated so a
            // panic here marks the job Failed instead of leaving it stuck as
            // `Running` forever with a zombie child left un-`wait()`-ed.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(move || {
                let stdout = stdout_handle.join().unwrap_or_default();
                let stderr = stderr_handle.join().unwrap_or_default();
                job_for_wait
                    .stdout
                    .lock()
                    .unwrap()
                    .push(&String::from_utf8_lossy(&stdout));
                job_for_wait
                    .stderr
                    .lock()
                    .unwrap()
                    .push(&String::from_utf8_lossy(&stderr));

                // Always wait, even if `kill_job` already marked this Killed —
                // otherwise the child becomes a zombie.
                let exit_status = child.wait();
                let mut status = job_for_wait.status.lock().unwrap();
                let was_running = matches!(*status, JobStatus::Running);
                if was_running {
                    *status = match exit_status {
                        Ok(code) => JobStatus::Exited(code.code().unwrap_or(-1)),
                        Err(e) => JobStatus::Failed(e.to_string()),
                    };
                }
                let final_status = status.clone();
                drop(status);
                if was_running {
                    push_finished_notice(&job_for_wait, &final_status);
                }
            }));
            if let Err(payload) = result {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] bg_shell job reaper thread panicked: {message}");
                let mut status = job.status.lock().unwrap();
                if matches!(*status, JobStatus::Running) {
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
    BG_SHELL_JOBS
        .lock()
        .unwrap()
        .insert(started.id.clone(), job);
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
        .map(|job| JobSummary {
            id: job.id.clone(),
            command: job.command.clone(),
            status: job.status.lock().unwrap().clone(),
            pid: job.pid,
            elapsed_secs: job.started_at.elapsed().as_secs(),
        })
        .collect();
    jobs.sort_by(|a, b| a.id.cmp(&b.id));
    jobs
}

/// How many tracked jobs are currently still running — cheap enough to call
/// on every prompt redraw for a persistent status-bar indicator.
pub fn running_count() -> usize {
    BG_SHELL_JOBS
        .lock()
        .unwrap()
        .values()
        .filter(|job| matches!(*job.status.lock().unwrap(), JobStatus::Running))
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

    Ok(JobSnapshot {
        id: job.id.clone(),
        command: job.command.clone(),
        cwd: job.cwd.clone(),
        status: job.status.lock().unwrap().clone(),
        pid: job.pid,
        stdout: job.stdout.lock().unwrap().data.clone(),
        stderr: job.stderr.lock().unwrap().data.clone(),
    })
}

#[cfg(not(target_os = "windows"))]
pub(crate) fn terminate_process_group(pid: u32) {
    let _ = Command::new("kill")
        .args(["-TERM", &format!("-{pid}")])
        .status();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(500));
        let _ = Command::new("kill")
            .args(["-KILL", &format!("-{pid}")])
            .status();
    });
}

#[cfg(target_os = "windows")]
pub(crate) fn terminate_process_group(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/F", "/T", "/PID", &pid.to_string()])
        .status();
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

    let current = job.status.lock().unwrap().clone();
    if !matches!(current, JobStatus::Running) {
        return Ok(format!(
            "job_id: {} is already {:?}, nothing to kill",
            job.id, current
        ));
    }

    let Some(pid) = job.pid else {
        return Ok(format!("job_id: {} has no known pid to kill", job.id));
    };

    terminate_process_group(pid);

    {
        let mut status = job.status.lock().unwrap();
        if matches!(*status, JobStatus::Running) {
            *status = JobStatus::Killed;
        }
    }

    Ok(format!("job_id: {} kill signal sent (pid {pid})", job.id))
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
        assert!(result.contains("kill signal sent"));

        let after = poll_until(&started.id, Duration::from_secs(10), |out| {
            out.contains("status: killed") || out.contains("status: exited")
        });
        assert!(after.contains("status: killed") || after.contains("status: exited"));
    }

    #[test]
    fn poll_unknown_job_errors() {
        assert!(matches!(
            poll_output("bg-does-not-exist"),
            Err(ShellError::JobNotFound(_))
        ));
    }
}

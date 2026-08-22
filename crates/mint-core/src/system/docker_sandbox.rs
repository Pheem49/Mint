//! Docker sandbox backend for `dispatch_subagent`-spawned subagents.
//!
//! One container per subagent *session*, not per command: `start_session`
//! starts a single detached container keyed by `sub_chat_id` (the same id
//! `dispatch_one_subagent` builds as `format!("{chat_id}::subagent::{name}")`),
//! and every subsequent `run_shell` call from that subagent execs into it via
//! `docker exec` instead of paying container-startup latency per command.
//! `stop_session` tears the container down when the subagent run ends,
//! success or failure — see `dispatch_one_subagent` in `orchestration/mod.rs`,
//! the sole call site that brackets a session's lifetime.
//!
//! Modeled directly on `integrations::mcp`'s `SESSIONS` registry (a live,
//! lazily-started external resource keyed by a stable string, looked up
//! across many calls, explicitly torn down) rather than `bg_shell`'s job
//! registry, which is keyed by an unrelated counter and has no start/reuse
//! lifecycle — the wrong shape for "one resource per subagent session."
//!
//! Killing the local `docker` client process does **not** reliably stop the
//! container it's attached to (Docker's signal-proxying only forwards
//! signals sent to the container's PID 1 through an *attached* `docker run`,
//! and a `docker exec` client dying never stops the in-container process at
//! all) — so, unlike `bg_shell::terminate_process_group`, teardown here is
//! always an explicit `docker stop`/`docker rm` by container id, never a
//! process-group signal.

use std::{
    collections::HashMap,
    path::Path,
    process::Command,
    sync::{LazyLock, Mutex},
};

use crate::MintConfig;

use super::shell::{
    CommandResult, SHELL_COMMAND_TIMEOUT, ShellError, run_with_timeout, writable_roots,
};

/// Live Docker sandbox containers, one per subagent session, keyed by
/// `sub_chat_id`. A plain `Mutex<HashMap<..>>` (not `Arc<Mutex<..>>` per
/// entry like `mcp::SESSIONS`) is enough here: unlike an MCP session's stdio
/// pipes, nothing about a `DockerSession` benefits from a finer-grained lock
/// — every operation against it is itself a fresh `docker exec` child
/// process, not a shared in-process resource.
static SESSIONS: LazyLock<Mutex<HashMap<String, DockerSession>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

struct DockerSession {
    container_id: String,
}

impl Drop for DockerSession {
    fn drop(&mut self) {
        // RAII safety net mirroring `mcp::McpSession`'s `Drop`, in case
        // `stop_session` is ever skipped (a panic unwinding past
        // `dispatch_one_subagent`'s match block, ...). Harmless to run again
        // after `stop_session` already removed the container — `docker rm`
        // on a nonexistent id just errors, which is ignored here same as
        // everywhere else in this module.
        let _ = Command::new("docker")
            .args(["rm", "-f", &self.container_id])
            .output();
    }
}

/// Whether the `docker` CLI is present and usable — the Docker analog of
/// `shell::sandbox_availability`.
pub fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .is_ok_and(|output| output.status.success())
}

/// Starts the sandbox container for `sub_chat_id`, or does nothing if one is
/// already registered for it (idempotent — `dispatch_one_subagent` calls
/// this once per subagent run, so a second call for the same id shouldn't
/// normally happen, but tolerating it costs nothing).
///
/// Bind mounts: `writable_roots(config, cwd)` (the same policy bwrap's
/// Linux path uses) mounted read-write, plus `config.allowed_read_paths`
/// mounted read-only. Unlike bwrap's `--ro-bind / /`, the rest of the host
/// filesystem is *not* exposed — a container ships its own `/usr`/`/bin`,
/// so there's no need to also expose the host's, and this keeps Docker's
/// posture at least as tight as `sandbox-exec`'s Seatbelt profile on macOS
/// (which already restricts reads to `allowed_read_paths` plus a short list
/// of system directories) rather than bwrap's looser whole-root-readable
/// concession.
///
/// Network: containers start with Docker's default bridge network attached,
/// matching bwrap/sandbox-exec's existing (also unrestricted) network
/// posture — see the module-level rationale in the implementation plan for
/// why per-command network gating isn't attempted here (`docker exec` has no
/// per-call network override).
pub fn start_session(sub_chat_id: &str, cwd: &Path, config: &MintConfig) -> Result<(), ShellError> {
    if SESSIONS.lock().unwrap().contains_key(sub_chat_id) {
        return Ok(());
    }

    let image = config.docker_sandbox_image.trim();
    let image = if image.is_empty() {
        "debian:bookworm-slim"
    } else {
        image
    };

    let mut cmd = Command::new("docker");
    // No `--rm` here: Docker's own auto-remove-on-stop and `stop_session`'s
    // explicit `docker stop` + `docker rm -f` (below) both racing to remove
    // the same container is exactly what produces a container stuck in
    // Docker's "Dead" state instead of actually going away. Explicit
    // start/stop/rm is the single source of truth for this session's
    // lifecycle; `--rm` would just be a second, competing one.
    cmd.args(["run", "-d", "--network", "bridge"]);
    cmd.args(["--label", &format!("mint-subagent={sub_chat_id}")]);
    let rw_roots = writable_roots(config, cwd);
    for root in &rw_roots {
        cmd.arg("-v")
            .arg(format!("{}:{}", root.display(), root.display()));
    }
    // `allowed_read_paths` and `allowed_write_paths` are the same list by
    // default (see `MintConfig::default`), so without this filter a fresh
    // install would try to `-v` the same host path twice — once rw, once
    // `:ro` — which Docker rejects outright as a duplicate mount point.
    // A path already covered by a writable root already has read access, so
    // only paths that are read-only *and not* also writable get their own
    // `:ro` mount.
    for root in config
        .allowed_read_paths
        .iter()
        .filter(|root| root.exists())
        .filter(|root| !rw_roots.iter().any(|rw| root.starts_with(rw)))
    {
        cmd.arg("-v")
            .arg(format!("{}:{}:ro", root.display(), root.display()));
    }
    cmd.arg("-w").arg(cwd);
    cmd.arg(image);
    cmd.args(["sleep", "infinity"]);

    let output = cmd
        .output()
        .map_err(|e| ShellError::Execute(std::io::Error::other(e.to_string())))?;
    if !output.status.success() {
        return Err(ShellError::Execute(std::io::Error::other(format!(
            "docker run failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ))));
    }
    let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if container_id.is_empty() {
        return Err(ShellError::Execute(std::io::Error::other(
            "docker run produced no container id",
        )));
    }

    SESSIONS
        .lock()
        .unwrap()
        .insert(sub_chat_id.to_string(), DockerSession { container_id });
    Ok(())
}

/// Runs `command` inside `sub_chat_id`'s container, if one is registered.
/// Returns `Ok(None)` when there is none — the common case for every
/// top-level (non-subagent) call and every subagent on the `"os"` backend —
/// so the caller (`run_shell_command`) falls through to its normal
/// OS-sandbox/unconfined path unchanged.
pub(crate) fn run_in_session(
    sub_chat_id: &str,
    command: &str,
    cwd: &Path,
    _config: &MintConfig,
) -> Result<Option<CommandResult>, ShellError> {
    let container_id = {
        let sessions = SESSIONS.lock().unwrap();
        let Some(session) = sessions.get(sub_chat_id) else {
            return Ok(None);
        };
        session.container_id.clone()
    };

    let mut cmd = Command::new("docker");
    cmd.args(["exec", "-w"]).arg(cwd).arg(&container_id);
    cmd.args(["bash", "-lc", command]);

    let output = run_with_timeout(cmd, SHELL_COMMAND_TIMEOUT, command, cwd)?;
    Ok(Some(output))
}

/// Explicit teardown for one session: a graceful `docker stop` (SIGTERM,
/// then SIGKILL after Docker's default grace period) followed by `docker rm
/// -f` as a belt-and-suspenders cleanup, then removes the map entry. Safe to
/// call even if no session is registered for
/// `sub_chat_id`, mirroring `mcp::close_mcp_session`'s tolerance of a
/// missing key. Called unconditionally by `dispatch_one_subagent` after its
/// `orchestrate_agent_loop` call returns, regardless of Ok/Err.
pub fn stop_session(sub_chat_id: &str) {
    if let Some(session) = SESSIONS.lock().unwrap().remove(sub_chat_id) {
        let _ = Command::new("docker")
            .args(["stop", &session.container_id])
            .output();
        let _ = Command::new("docker")
            .args(["rm", "-f", &session.container_id])
            .output();
    }
}

/// Whether a session is currently registered for `sub_chat_id`. Used by
/// `dispatch_one_subagent` for logging/diagnostics only — the actual
/// container lookup for execution happens inside `run_in_session`, reached
/// from deep inside `run_shell_command` where only `chat_id: Option<&str>`
/// is in scope.
pub fn has_session(sub_chat_id: &str) -> bool {
    SESSIONS.lock().unwrap().contains_key(sub_chat_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docker_available_for_test() -> bool {
        docker_available()
    }

    #[test]
    fn docker_sandbox_available_reports_false_when_docker_not_on_path() {
        // Not gated on `docker_available_for_test` — this test's whole point
        // is exercising the "not available" path, so it should run even in
        // environments that genuinely lack Docker (most CI).
        if command_exists_for_test("docker") {
            return;
        }
        assert!(!docker_available());
    }

    fn command_exists_for_test(command: &str) -> bool {
        std::env::var_os("PATH").is_some_and(|path_os| {
            std::env::split_paths(&path_os).any(|dir| dir.join(command).is_file())
        })
    }

    #[test]
    fn run_in_session_returns_none_for_unregistered_chat_id() {
        let config = MintConfig::default();
        let result = run_in_session("no-such-session", "echo hi", Path::new("."), &config).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn stop_session_is_a_noop_for_unknown_session() {
        // Should not panic even though nothing is registered.
        stop_session("no-such-session");
        assert!(!has_session("no-such-session"));
    }

    #[test]
    fn start_session_then_run_in_session_executes_inside_container() {
        if !docker_available_for_test() {
            return;
        }
        let config = MintConfig::default();
        let cwd = std::env::temp_dir();
        let id = "docker-sandbox-test::subagent::probe";
        start_session(id, &cwd, &config).expect("docker run should succeed");
        assert!(has_session(id));

        let output = run_in_session(id, "test -f /.dockerenv && echo yes", &cwd, &config)
            .unwrap()
            .expect("a session is registered for this id");
        assert!(output.success);
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "yes");

        stop_session(id);
        assert!(!has_session(id));

        let ps = Command::new("docker")
            .args(["ps", "-a", "--filter", &format!("label=mint-subagent={id}")])
            .output()
            .unwrap();
        let listed = String::from_utf8_lossy(&ps.stdout);
        assert_eq!(
            listed.lines().count(),
            1,
            "only the header line should remain after stop_session: {listed}"
        );
    }
}

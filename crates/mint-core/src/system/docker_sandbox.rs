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
    /// How many concurrent `dispatch_one_subagent` calls are currently using
    /// this container. `run_parallel_subagent_batch` doesn't dedupe subagent
    /// names, so two dispatches of the *same* name can run concurrently and
    /// share one `sub_chat_id` — without this, whichever one finished first
    /// would tear the container down via `stop_session` while the other was
    /// still mid-`docker exec` against it. `start_session` increments this
    /// instead of starting a second container when one is already running;
    /// `stop_session` decrements it and only actually stops/removes the
    /// container once the count reaches zero.
    ref_count: usize,
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

/// Starts the sandbox container for `sub_chat_id`, or — if one is already
/// running for it, e.g. two concurrent dispatches of the same subagent name
/// (see `DockerSession::ref_count`) — bumps its reference count instead of
/// starting a second one. Callers must pair every successful call with
/// exactly one [`stop_session`] call.
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
    if let Some(session) = SESSIONS.lock().unwrap().get_mut(sub_chat_id) {
        session.ref_count += 1;
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

    // Re-check under the lock: another call for this *same brand-new* id
    // could have raced ahead of the fast path above and already inserted
    // its own container while this call was blocked in `docker run` —
    // possible only the very first time two concurrent dispatches share an
    // id neither has started yet (every call after the first hits the fast
    // path instead). If so, adopt the winner's container as this call's
    // reference and discard the redundant one just started here, rather
    // than leaking an untracked second container for the same session.
    let mut sessions = SESSIONS.lock().unwrap();
    if let Some(session) = sessions.get_mut(sub_chat_id) {
        session.ref_count += 1;
        drop(sessions);
        let _ = Command::new("docker")
            .args(["rm", "-f", &container_id])
            .output();
        return Ok(());
    }
    sessions.insert(
        sub_chat_id.to_string(),
        DockerSession {
            container_id,
            ref_count: 1,
        },
    );
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

/// Releases this caller's reference to one session. Decrements its
/// `ref_count`; only once that reaches zero (every concurrent dispatch that
/// shares this `sub_chat_id` — see `DockerSession::ref_count` — has also
/// called this) does it actually tear the container down: a graceful
/// `docker stop` (SIGTERM, then SIGKILL after Docker's default grace
/// period) followed by `docker rm -f` as a belt-and-suspenders cleanup, then
/// removing the map entry. Safe to call even if no session is registered
/// for `sub_chat_id`, mirroring `mcp::close_mcp_session`'s tolerance of a
/// missing key. Called unconditionally by `dispatch_one_subagent` after its
/// `orchestrate_agent_loop` call returns, regardless of Ok/Err — exactly
/// once per successful [`start_session`] call.
pub fn stop_session(sub_chat_id: &str) {
    let mut sessions = SESSIONS.lock().unwrap();
    let Some(session) = sessions.get_mut(sub_chat_id) else {
        return;
    };
    session.ref_count = session.ref_count.saturating_sub(1);
    if session.ref_count > 0 {
        return;
    }
    let Some(session) = sessions.remove(sub_chat_id) else {
        return;
    };
    drop(sessions);
    let _ = Command::new("docker")
        .args(["stop", &session.container_id])
        .output();
    let _ = Command::new("docker")
        .args(["rm", "-f", &session.container_id])
        .output();
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

    /// Force-remove any container still carrying this test's `mint-subagent`
    /// label, and drop any stale in-memory session for it. A previous run
    /// killed mid-test (machine reboot, `earlyoom` killing `cargo test`,
    /// Ctrl-C) leaves a container behind that the fixed-label `docker ps -a`
    /// assertions below would otherwise count. Run at the start of each
    /// container test so the environment is known-clean regardless of how the
    /// last run ended.
    fn purge_test_containers(id: &str) {
        stop_session(id);
        let ids = Command::new("docker")
            .args([
                "ps",
                "-aq",
                "--filter",
                &format!("label=mint-subagent={id}"),
            ])
            .output()
            .map(|out| {
                String::from_utf8_lossy(&out.stdout)
                    .split_whitespace()
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if !ids.is_empty() {
            let _ = Command::new("docker")
                .arg("rm")
                .arg("-f")
                .args(&ids)
                .output();
        }
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
        purge_test_containers(id);
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

    /// Regression test for the bug `dispatch_subagent` hit in practice:
    /// `run_parallel_subagent_batch` can dispatch the same subagent name
    /// twice concurrently (it doesn't dedupe names), so two callers can
    /// share one `sub_chat_id`. Without ref-counting, whichever one finished
    /// first would tear the shared container down via `stop_session` while
    /// the other was still using it for `run_in_session`.
    #[test]
    fn concurrent_start_sessions_share_a_container_until_every_caller_stops() {
        if !docker_available_for_test() {
            return;
        }
        let config = MintConfig::default();
        let cwd = std::env::temp_dir();
        let id = "docker-sandbox-test::subagent::sibling-refcount";
        purge_test_containers(id);

        start_session(id, &cwd, &config).expect("first caller's docker run should succeed");
        start_session(id, &cwd, &config)
            .expect("second concurrent caller should reuse the first's container");
        assert!(has_session(id));

        // First caller finishes and releases its reference — the container
        // must still be alive for the second caller, which hasn't stopped
        // yet.
        stop_session(id);
        assert!(
            has_session(id),
            "container was torn down while a second caller still held a reference"
        );
        let output = run_in_session(id, "echo still-alive", &cwd, &config)
            .unwrap()
            .expect("the session the second caller is still using should still be registered");
        assert!(output.success);
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            "still-alive"
        );

        // Second (last) caller finishes — now it actually tears down.
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
            "only the header line should remain after the last stop_session: {listed}"
        );
    }
}

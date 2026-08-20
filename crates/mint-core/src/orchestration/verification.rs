use super::*;

/// Cheap pre-filter run before the (costlier) auto-skill-writing reflection call:
/// only tasks that took several steps and did real work (edited files, ran shell
/// commands, drove the browser, or delegated to a subagent) are worth asking the
/// LLM to judge for skill-worthiness. Keeps trivial one-shot chats from spawning an
/// extra reflection call every time `auto_skill_writing` is enabled.
pub(super) fn looks_skill_worthy(step: usize, action_counts: &BTreeMap<String, usize>) -> bool {
    const SUBSTANTIVE_ACTIONS: &[&str] = &[
        "apply_patch",
        "write_file",
        "run_shell",
        "browser_open",
        "browser_click",
        "browser_type",
        "dispatch_subagent",
    ];
    step >= 3
        && action_counts.keys().any(|key| {
            SUBSTANTIVE_ACTIONS
                .iter()
                .any(|action| key.starts_with(action))
        })
}

/// Whether `finish` should be rejected because the run modified a file
/// (`apply_patch`/`write_file`) without a subsequent `verify` call and without
/// an explicit written reason in the `finish` action's `verification` field.
pub(super) fn unverified_modification(
    last_modify_step: Option<usize>,
    last_verify_step: Option<usize>,
    verification_field: &str,
) -> bool {
    let Some(modify_step) = last_modify_step else {
        return false;
    };
    let verified_since = last_verify_step.is_some_and(|verify_step| verify_step >= modify_step);
    !verified_since && meaningful_verification(verification_field).is_empty()
}

/// Whether any `run_shell`/`verify` command in `result` — which may join
/// several `"exit: N\n..."` blocks, one per command in a multi-command
/// `verify` call — reported a non-zero, known exit code. Scans every
/// `"exit: "` line rather than just the first, so a `verify` call where an
/// earlier command passed but a later one failed still counts as a failure.
pub(super) fn shell_result_failed(result: &str) -> bool {
    result.lines().any(|line| {
        line.strip_prefix("exit: ")
            .is_some_and(|code| !matches!(code.trim(), "0" | "unknown"))
    })
}

/// Whether `finish` should be rejected because the most recent `verify` call
/// reported a failure and the agent said nothing about it in the `finish`
/// action's `verification` field. Unlike `unverified_modification` (which
/// only checks that `verify` was *called*), this catches the agent claiming
/// success while ignoring a real failure that's sitting right there in
/// `verify`'s own last result — the specific "reports success when it
/// wasn't" failure mode that matters more here than in an
/// interactively-supervised session, since a scheduled or messaging-bridge
/// run has nobody watching live to catch it.
pub(super) fn unacknowledged_verify_failure(
    last_verify_failed: Option<bool>,
    verification_field: &str,
) -> bool {
    last_verify_failed == Some(true) && meaningful_verification(verification_field).is_empty()
}

pub(super) fn meaningful_verification(value: &str) -> &str {
    let value = value.trim();
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "" | "not run"
            | "not run."
            | "no checks run"
            | "no checks run."
            | "not_required"
            | "not required"
            | "none"
            | "n/a"
    ) {
        ""
    } else {
        value
    }
}

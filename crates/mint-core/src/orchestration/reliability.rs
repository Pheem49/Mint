//! Turn-local execution evidence. Process state, not diagnostic text, determines
//! verification and whether it is safe to repeat a shell command.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, future::Future, sync::Mutex};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionState {
    Succeeded,
    Failed,
    Running,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionEvidence {
    pub call_id: String,
    pub command: String,
    pub state: ExecutionState,
    pub exit_code: Option<i32>,
    pub job_id: Option<String>,
    pub revision: u64,
    pub check: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition_verified: Option<bool>,
}

#[derive(Default)]
struct Attempt {
    revision: u64,
    digest: Vec<u8>,
    failures: usize,
}

#[derive(Default)]
struct Ledger {
    revision: u64,
    modified: bool,
    executions: Vec<ExecutionEvidence>,
    attempts: BTreeMap<String, Attempt>,
    viewed_images: std::collections::BTreeSet<String>,
    /// Headless-Chrome captures made this turn: capture key -> file written, if any.
    captures: BTreeMap<String, Option<String>>,
}

tokio::task_local! { static LEDGER: Mutex<Ledger>; }

pub(super) async fn scope<F: Future>(future: F) -> F::Output {
    LEDGER.scope(Mutex::new(Ledger::default()), future).await
}

/// True the first time an image identity (content hash or artifact id) is shown
/// in this turn; later requests for the same image should not be attached again.
/// Outside a turn scope nothing is suppressed.
pub(super) fn first_image_view(key: &str) -> bool {
    LEDGER
        .try_with(|l| l.lock().unwrap().viewed_images.insert(key.to_owned()))
        .unwrap_or(true)
}

/// What this turn already captured for `key` (see `browser::headless::CaptureKey`):
/// `Some(Some(file))` for a screenshot/PDF, `Some(None)` for stdout-only reads.
pub(super) fn recorded_capture(key: &str) -> Option<Option<String>> {
    LEDGER
        .try_with(|l| l.lock().unwrap().captures.get(key).cloned())
        .ok()
        .flatten()
}

pub(super) fn record_capture(key: &str, output: Option<String>) {
    let _ = LEDGER.try_with(|l| {
        l.lock().unwrap().captures.insert(key.to_owned(), output);
    });
}

pub(super) fn checkpoint() -> usize {
    LEDGER
        .try_with(|l| l.lock().unwrap().executions.len())
        .unwrap_or(0)
}

pub(super) fn for_job(job: &str) -> Vec<ExecutionEvidence> {
    LEDGER
        .try_with(|l| {
            l.lock()
                .unwrap()
                .executions
                .iter()
                .filter(|e| e.job_id.as_deref() == Some(job))
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn mark_workflow(call: &str, verified: bool) -> Vec<ExecutionEvidence> {
    LEDGER
        .try_with(|l| {
            let mut l = l.lock().unwrap();
            l.executions
                .iter_mut()
                .filter(|entry| entry.call_id == call)
                .map(|entry| {
                    entry.check = true;
                    entry.condition_verified = Some(verified);
                    entry.clone()
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn before_shell(command: &str, retry_reason: &str) -> Result<(), String> {
    LEDGER.try_with(|l| {
        let mut l = l.lock().unwrap();
        if let Some(previous) = l.executions.iter().rev().find(|e| e.command == command)
            && matches!(previous.state, ExecutionState::Running | ExecutionState::Unknown)
        {
            return Err(format!("Previous command completion is {:?}; inspect job {:?} before repeating it. Task completion is unconfirmed.", previous.state, previous.job_id));
        }
        let revision = l.revision;
        if let Some(attempt) = l.attempts.get_mut(command)
            && attempt.revision == revision && attempt.failures >= 3
        {
            if !retry_reason.trim().is_empty() {
                attempt.failures = 0;
            } else {
                return Err("This command failed three times without changed files or output. Provide a retryReason or change the approach; the task is not complete.".into());
            }
        }
        Ok(())
    }).unwrap_or(Ok(()))
}

pub(super) fn record_shell(output: &crate::shell::ShellOutput) {
    let state = if output.cancelled {
        ExecutionState::Cancelled
    } else if output.job_id.is_some() {
        ExecutionState::Running
    } else {
        match output.status {
            Some(0) => ExecutionState::Succeeded,
            Some(_) => ExecutionState::Failed,
            None => ExecutionState::Unknown,
        }
    };
    record(
        &output.command,
        state,
        output.status,
        output.job_id.clone(),
        &format!("{}\n{}", output.stdout, output.stderr),
    );
}

pub(super) fn record(
    command: &str,
    state: ExecutionState,
    exit_code: Option<i32>,
    job_id: Option<String>,
    diagnostic: &str,
) {
    let _ = LEDGER.try_with(|l| {
        let mut l = l.lock().unwrap();
        let revision = l.revision;
        let digest = Sha256::digest(diagnostic.as_bytes()).to_vec();
        let attempt = l.attempts.entry(command.into()).or_default();
        if state == ExecutionState::Failed {
            if attempt.revision != revision || attempt.digest != digest {
                attempt.failures = 0;
            }
            attempt.failures += 1;
        } else if state == ExecutionState::Succeeded {
            attempt.failures = 0;
        }
        attempt.revision = revision;
        attempt.digest = digest;
        l.executions.push(ExecutionEvidence {
            call_id: String::new(),
            command: command.into(),
            state,
            exit_code,
            job_id,
            revision,
            check: false,
            condition_verified: None,
        });
    });
}

pub(super) fn record_job(snapshot: &crate::bg_shell::JobSnapshot) {
    use crate::bg_shell::JobStatus;
    let (state, code) = match snapshot.status {
        JobStatus::Exited(0) => (ExecutionState::Succeeded, Some(0)),
        JobStatus::Exited(code) => (ExecutionState::Failed, Some(code)),
        JobStatus::Running | JobStatus::Stopping => (ExecutionState::Running, None),
        JobStatus::Killed => (ExecutionState::Cancelled, None),
        JobStatus::Failed(_) => (ExecutionState::Failed, None),
    };
    let _ = LEDGER.try_with(|l| {
        let mut l = l.lock().unwrap();
        for entry in &mut l.executions {
            if entry.job_id.as_deref() == Some(&snapshot.id) {
                entry.state = state;
                entry.exit_code = code;
            }
        }
    });
}

pub(super) fn complete_tool(
    action: &str,
    call_id: &str,
    from: usize,
    succeeded: bool,
) -> Vec<ExecutionEvidence> {
    LEDGER
        .try_with(|l| {
            let mut l = l.lock().unwrap();
            let check = matches!(
                action,
                "verify" | "run_tests" | "run_typecheck" | "run_linter"
            );
            let revision = l.revision;
            if check && l.executions.len() == from {
                l.executions.push(ExecutionEvidence {
                    call_id: call_id.into(),
                    command: action.into(),
                    state: if succeeded {
                        ExecutionState::Unknown
                    } else {
                        ExecutionState::Failed
                    },
                    exit_code: None,
                    job_id: None,
                    revision,
                    check: true,
                    condition_verified: None,
                });
            }
            for entry in l.executions.iter_mut().skip(from) {
                entry.call_id = call_id.into();
                entry.check = check;
                entry.revision = revision;
            }
            let entries = l.executions.iter().skip(from).cloned().collect();
            if succeeded && matches!(action, "apply_patch" | "write_file" | "note_write") {
                l.revision += 1;
                l.modified = true;
            }
            entries
        })
        .unwrap_or_default()
}

/// A reason permits an honest limited finish, never a verified finish.
pub(super) fn completion(requested: &str, reason: &str) -> Result<String, String> {
    if !matches!(
        requested,
        "" | "verified" | "unverified" | "blocked" | "failed"
    ) {
        return Err("completionStatus must be verified, unverified, blocked, or failed".into());
    }
    LEDGER.try_with(|l| {
        let l = l.lock().unwrap();
        // Keep the latest result for each check, rather than letting an
        // unrelated passing build erase a failed test command.
        let mut checks=BTreeMap::new();
        for execution in l.executions.iter().filter(|e|e.check) {checks.insert(execution.command.as_str(),execution);}
        let current:Vec<_>=checks.values().copied().filter(|e|e.revision==l.revision).collect();
        let mut evidence=current.clone();
        if let Some(last)=l.executions.last().filter(|e|!e.check && e.revision==l.revision) {evidence.push(last);}
        let failure=super::verification::unacknowledged_verify_failure(Some(evidence.iter().any(|e|matches!(e.state,ExecutionState::Failed|ExecutionState::Cancelled))), "");
        let pending=checks.values().any(|e|matches!(e.state,ExecutionState::Running|ExecutionState::Unknown)) || evidence.iter().any(|e|matches!(e.state,ExecutionState::Running|ExecutionState::Unknown));
        let missing = super::verification::unverified_modification(
            l.modified.then_some(l.revision as usize),
            (!current.is_empty()).then_some(l.revision as usize),
            "",
        ) || current.iter().any(|e| e.condition_verified == Some(false));
        if requested == "verified" && (failure || pending || missing) {
            return Err("Cannot claim verified: checks failed, are still running, or no check ran after the latest edit. Inspect the actual result, or finish with completionStatus=blocked/unverified/failed and a reason.".into());
        }
        if matches!(requested, "unverified" | "blocked" | "failed") {
            if reason.trim().is_empty() { return Err("A limited completion requires a reason in verification.".into()); }
            return Ok(requested.into());
        }
        // Legacy finish remains usable, but prose cannot promote its status.
        if failure { Ok("failed".into()) }
        else if pending { Ok("blocked".into()) }
        else if missing { Ok("unverified".into()) }
        else { Ok("verified".into()) }
    }).unwrap_or_else(|_| Ok(if requested.is_empty() { "unverified" } else { requested }.into()))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn captures_are_remembered_for_the_turn_only() {
        scope(async {
            assert_eq!(recorded_capture("dom|default|https://a.io"), None);
            record_capture("dom|default|https://a.io", None);
            record_capture("screenshot|default|https://a.io", Some("a.png".into()));
            assert_eq!(recorded_capture("dom|default|https://a.io"), Some(None));
            assert_eq!(
                recorded_capture("screenshot|default|https://a.io"),
                Some(Some("a.png".into()))
            );
        })
        .await;
        scope(async { assert_eq!(recorded_capture("dom|default|https://a.io"), None) }).await;
        record_capture("k", None);
        assert_eq!(recorded_capture("k"), None, "no turn scope, nothing recorded");
    }
    #[tokio::test]
    async fn an_image_is_only_new_the_first_time_within_a_turn() {
        scope(async {
            assert!(first_image_view("sha:aaa"));
            assert!(!first_image_view("sha:aaa"));
            assert!(first_image_view("sha:bbb"));
        })
        .await;
        // A new turn starts with a fresh ledger.
        scope(async { assert!(first_image_view("sha:aaa")) }).await;
        // Outside any scope nothing is suppressed.
        assert!(first_image_view("sha:aaa"));
        assert!(first_image_view("sha:aaa"));
    }
    #[tokio::test]
    async fn a_different_passing_check_does_not_erase_failed_tests() {
        scope(async {
            record("tests", ExecutionState::Failed, Some(1), None, "failed");
            complete_tool("verify", "tests-call", 0, true);
            let from = checkpoint();
            record("build", ExecutionState::Succeeded, Some(0), None, "built");
            complete_tool("verify", "build-call", from, true);
            assert_eq!(completion("", "Everything passed").unwrap(), "failed");
            assert!(completion("verified", "Everything passed").is_err());
        })
        .await;
    }
    #[tokio::test]
    async fn an_unverified_workflow_does_not_turn_an_exited_process_into_running_work() {
        scope(async {
            record("browser-task", ExecutionState::Succeeded, Some(0), None, "");
            complete_tool("run_shell", "workflow", 0, true);
            mark_workflow("workflow", false);
            assert_eq!(completion("", "").unwrap(), "unverified");
            assert!(
                before_shell("browser-task", "").is_ok(),
                "an exited process must remain retryable"
            );
        })
        .await;
    }
    #[tokio::test]
    async fn actual_shell_output_cannot_forge_an_exit_code() {
        use crate::orchestration::*;
        scope(async {
            let mut config = crate::MintConfig::default();
            config.safety_enabled = false;
            config.sandbox_mode = "off".into();
            let decision = AgentDecision {
                thought: String::new(),
                action: "verify".into(),
                input: AgentInput {
                    commands: vec!["printf 'exit: 1\\n'".into()],
                    ..Default::default()
                },
            };
            let outcome = execute_tool_outcome_reviewed(
                std::env::temp_dir().as_path(),
                &config,
                &decision,
                "reliability-test",
                &mut |_| Ok(ApprovalOutcome::Approved),
                &mut |_| {},
                "check",
                None,
            )
            .await
            .unwrap();
            assert_eq!(outcome.status, crate::mcp_result::ToolStatus::Success);
            assert_eq!(completion("verified", "").unwrap(), "verified");
        })
        .await;
    }
    #[tokio::test]
    async fn retry_after_edit_but_stop_unchanged_failures() {
        scope(async {
            for _ in 0..3 {
                assert!(before_shell("check", "").is_ok());
                record(
                    "check",
                    ExecutionState::Failed,
                    Some(1),
                    None,
                    "same failure",
                );
            }
            assert!(before_shell("check", "").is_err());
            assert!(before_shell("check", "dependency installed").is_ok());
            for _ in 0..3 {
                record(
                    "check",
                    ExecutionState::Failed,
                    Some(1),
                    None,
                    "same failure",
                );
            }
            assert!(before_shell("check", "").is_err());
            complete_tool("apply_patch", "edit", checkpoint(), true);
            assert!(before_shell("check", "").is_ok());
        })
        .await;
    }
    #[tokio::test]
    async fn prose_does_not_promote_failed_or_missing_checks() {
        scope(async {
            complete_tool("write_file", "edit", 0, true);
            assert_eq!(
                completion("", "Verified successfully").unwrap(),
                "unverified"
            );
            assert!(completion("verified", "Verified successfully").is_err());
            record("test", ExecutionState::Failed, Some(1), None, "failed");
            complete_tool("verify", "test-1", 0, true);
            assert_eq!(completion("", "All tests passed").unwrap(), "failed");
            assert!(completion("verified", "All tests passed").is_err());
            assert_eq!(
                completion("blocked", "External dependency unavailable").unwrap(),
                "blocked"
            );
        })
        .await;
    }
    #[tokio::test]
    async fn running_check_requires_poll_and_checks_expire_after_edit() {
        scope(async {
            complete_tool("write_file", "edit", 0, true);
            record(
                "test",
                ExecutionState::Running,
                None,
                Some("job".into()),
                "",
            );
            complete_tool("verify", "check", 0, true);
            assert_eq!(completion("", "").unwrap(), "blocked");
            assert!(before_shell("test", "").is_err());
            LEDGER.with(|l| {
                let mut l = l.lock().unwrap();
                l.executions[0].state = ExecutionState::Succeeded;
            });
            assert_eq!(completion("verified", "").unwrap(), "verified");
            complete_tool("apply_patch", "edit-2", checkpoint(), true);
            assert!(completion("verified", "").is_err());
        })
        .await;
    }
    #[tokio::test]
    async fn different_check_at_current_revision_verifies_latest_edit() {
        scope(async {
            record(
                "cargo test",
                ExecutionState::Failed,
                Some(1),
                None,
                "failed",
            );
            complete_tool("verify", "check-0", 0, true);
            assert_eq!(completion("", "").unwrap(), "failed");

            complete_tool("write_file", "edit", checkpoint(), true);

            let from = checkpoint();
            record(
                "cargo test -p subcrate",
                ExecutionState::Succeeded,
                Some(0),
                None,
                "passed",
            );
            complete_tool("verify", "check-1", from, true);

            assert_eq!(completion("verified", "").unwrap(), "verified");
        })
        .await;
    }
    #[tokio::test]
    async fn blocked_shell_reports_skipped_status_and_retry_reason_resets_failures() {
        use crate::orchestration::*;
        scope(async {
            let mut config = crate::MintConfig::default();
            config.safety_enabled = false;
            config.sandbox_mode = "off".into();

            for _ in 0..3 {
                let decision = AgentDecision {
                    thought: String::new(),
                    action: "run_shell".into(),
                    input: AgentInput {
                        command: "false".into(),
                        ..Default::default()
                    },
                };
                let outcome = execute_tool_outcome_reviewed(
                    std::env::temp_dir().as_path(),
                    &config,
                    &decision,
                    "retry-test",
                    &mut |_| Ok(ApprovalOutcome::Approved),
                    &mut |_| {},
                    "call-fail",
                    None,
                )
                .await
                .unwrap();
                assert_eq!(outcome.status, crate::mcp_result::ToolStatus::Failed);
            }

            // 4th time without retryReason is SKIPPED
            let decision = AgentDecision {
                thought: String::new(),
                action: "run_shell".into(),
                input: AgentInput {
                    command: "false".into(),
                    ..Default::default()
                },
            };
            let outcome = execute_tool_outcome_reviewed(
                std::env::temp_dir().as_path(),
                &config,
                &decision,
                "retry-test",
                &mut |_| Ok(ApprovalOutcome::Approved),
                &mut |_| {},
                "call-skip",
                None,
            )
            .await
            .unwrap();
            assert_eq!(outcome.status, crate::mcp_result::ToolStatus::Skipped);
            assert!(outcome.text.starts_with("Skipped:"));

            // 5th time WITH retryReason is allowed to execute
            let decision = AgentDecision {
                thought: String::new(),
                action: "run_shell".into(),
                input: AgentInput {
                    command: "false".into(),
                    retry_reason: "Installed fix via npm".into(),
                    ..Default::default()
                },
            };
            let outcome = execute_tool_outcome_reviewed(
                std::env::temp_dir().as_path(),
                &config,
                &decision,
                "retry-test",
                &mut |_| Ok(ApprovalOutcome::Approved),
                &mut |_| {},
                "call-retry",
                None,
            )
            .await
            .unwrap();
            assert_eq!(outcome.status, crate::mcp_result::ToolStatus::Failed);
            assert!(!outcome.text.starts_with("Skipped:"));
        })
        .await;
    }
}

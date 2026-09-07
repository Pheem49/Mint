use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to git.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    _config: &MintConfig,
    _chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "git_status" => run_git(root, &["status", "--short", "--branch"]),
        "git_diff" => {
            if input.path.trim().is_empty() {
                run_git(root, &["diff", "--"])
            } else {
                let path = workspace_path(root, &input.path)?;
                let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
                run_git(root, &["diff", "--", relative.as_ref()])
            }
        }
        "git_log" => {
            let limit = input.limit.unwrap_or(5).clamp(1, 50).to_string();
            run_git(root, &["log", "-n", &limit, "--oneline", "--decorate"])
        }
        "git_branch" => run_git(root, &["branch", "--show-current"]),
        "git_checkpoint" => {
            let desc = if !input.summary.trim().is_empty() {
                input.summary.trim()
            } else {
                "Manual checkpoint"
            };
            match crate::git::create_checkpoint(root, _chat_id, 0, "manual", None, desc) {
                Ok(Some(cp)) => Ok(format!(
                    "Checkpoint created: {} (commit: {})",
                    cp.id,
                    &cp.commit_hash[..7.min(cp.commit_hash.len())]
                )),
                Ok(None) => Ok("Not a git repository, checkpoint skipped.".into()),
                Err(e) => Err(OrchestrationError::Agent(format!("Failed to create checkpoint: {e}"))),
            }
        }
        "git_rollback" => {
            if let Some(step) = input.step {
                crate::git::rollback_to_step(root, _chat_id, step)
                    .map_err(OrchestrationError::Agent)
            } else {
                crate::git::rollback_task_changes(root, _chat_id)
                    .map_err(OrchestrationError::Agent)
            }
        }
        "git_restore_file" => {
            let path = required(&input.path, "path")?;
            let git_ref = if !input.command.trim().is_empty() {
                Some(input.command.trim())
            } else {
                None
            };
            crate::git::restore_file(root, path, git_ref)
                .map_err(OrchestrationError::Agent)
        }
        "git_commit" => {
            let msg = if !input.summary.trim().is_empty() {
                input.summary.trim().to_string()
            } else {
                crate::git::generate_commit_message(root, None)
                    .unwrap_or_else(|_| "feat: update project files".to_string())
            };
            crate::git::commit_task_changes(root, &msg)
                .map_err(OrchestrationError::Agent)
        }
        "git_create_branch" => {
            let branch = if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.name.trim().is_empty() {
                input.name.trim()
            } else {
                required(&input.query, "query")?
            };
            crate::git::create_task_branch(root, branch)
                .map_err(OrchestrationError::Agent)
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::git::execute: {action}"
        ),
    }
}

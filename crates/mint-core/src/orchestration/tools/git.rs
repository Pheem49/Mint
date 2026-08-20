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
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::git::execute: {action}"
        ),
    }
}

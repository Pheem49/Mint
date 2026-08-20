use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to project.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    config: &MintConfig,
    _chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "detect_project" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(&detect_project(&path))
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "list_tests" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(&list_tests(&path, config)?)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "read_diagnostics" => {
            let path = workspace_path(root, &input.path)?;
            read_diagnostics(&path, config).await
        }
        "view_image" => {
            let path = workspace_path(root, required(&input.path, "path")?)?;
            view_image(&path, config)
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::project::execute: {action}"
        ),
    }
}

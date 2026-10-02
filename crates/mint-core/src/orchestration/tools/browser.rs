use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to browser.
/// Keeps legacy argument aliases at the orchestration seam; session behavior
/// and browser result verification live in the shared browser module.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    _root: &Path,
    config: &MintConfig,
    _chat_id: &str,
    _approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    let mut args =
        serde_json::to_value(input).map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    // Preserve the established legacy argument aliases.
    if input.url.is_empty() && !input.path.is_empty() {
        args["url"] = input.path.clone().into();
    }
    if input.selector.is_empty() && input.element_ref.is_empty() && !input.path.is_empty() {
        args["selector"] = input.path.clone().into();
    }
    if input.text.is_empty() && !input.query.is_empty() {
        args["text"] = input.query.clone().into();
    }
    crate::browser::execute_action(config, action, &args)
        .await
        .map_err(OrchestrationError::Agent)
}

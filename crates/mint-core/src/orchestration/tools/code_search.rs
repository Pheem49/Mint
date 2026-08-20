use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to code search.
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
        "search_code" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &search_code(
                    &path,
                    required(&input.query, "query")?,
                    input.limit.unwrap_or(20),
                    config,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "symbols" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &build_symbol_index(&path, input.limit.unwrap_or(100), config)
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "semantic_index" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &index_semantic_code(&path, config)
                    .await
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "semantic_search" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &search_semantic_code(
                    &path,
                    required(&input.query, "query")?,
                    input.limit.unwrap_or(5),
                    config,
                )
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::code_search::execute: {action}"
        ),
    }
}

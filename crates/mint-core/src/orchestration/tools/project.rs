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
    chat_id: &str,
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
            read_diagnostics(&path, config, chat_id).await
        }
        "view_image" => {
            let path = workspace_path(root, required(&input.path, "path")?)?;
            view_image(&path, config)
        }
        "search_docs" => {
            let query = required(&input.query, "query")?;
            let hits = crate::system::knowledge_engine::search_project_knowledge(root, query);
            if hits.is_empty() {
                Ok("No relevant documentation found in project docs/. You may search library docs or query the web using search_web.".into())
            } else {
                Ok(serde_json::to_string_pretty(&hits)
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
            }
        }
        "create_project_doc" => {
            let title = required(&input.title, "title")?;
            let content = required(&input.content, "content")?;
            let category = if !input.kind.trim().is_empty() {
                input.kind.trim()
            } else {
                "general"
            };
            let doc_path = crate::system::knowledge_engine::create_project_doc(root, category, title, content)
                .map_err(OrchestrationError::Agent)?;
            let rel = doc_path.strip_prefix(root).unwrap_or(&doc_path).to_string_lossy();
            Ok(format!("Documentation successfully created at: {rel}"))
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::project::execute: {action}"
        ),
    }
}

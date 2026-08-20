use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to planning.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    _root: &Path,
    _config: &MintConfig,
    _chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "create_plan" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "objective": input.summary,
            "steps": input.steps,
        }))
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
        "update_plan" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "steps": input.steps,
            "status": input.status,
        }))
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
        "request_user_approval" => {
            let title = if input.title.trim().is_empty() {
                "User approval"
            } else {
                input.title.trim()
            };
            let prompt = required(&input.summary, "summary")?;
            let approved = approve_cb(&AgentApproval::UserApproval {
                title: title.to_owned(),
                prompt: prompt.to_owned(),
            })
            .map_err(OrchestrationError::Agent)?;
            match approved {
                ApprovalOutcome::Approved => Ok(format!("User approved: {title}")),
                ApprovalOutcome::Denied => Ok(format!("User denied: {title}")),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "ask_user" => {
            let question = required(&input.query, "query")?;
            let options: Vec<String> = input
                .options
                .iter()
                .map(|o| o.trim().to_owned())
                .filter(|o| !o.is_empty())
                .take(3)
                .collect();
            let approved = approve_cb(&AgentApproval::AskUser {
                question: question.to_owned(),
                options,
            })
            .map_err(OrchestrationError::Agent)?;
            match approved {
                ApprovalOutcome::Approved => Ok("User approved the prompt.".into()),
                ApprovalOutcome::Denied => Ok("User declined to answer.".into()),
                ApprovalOutcome::Intercepted(answer) => Ok(format!("User answered: {answer}")),
            }
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::planning::execute: {action}"
        ),
    }
}

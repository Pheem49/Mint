use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to shell.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "run_shell" => {
            let command = required(&input.command, "command")?;
            let mode = classify_shell_command(command).mode.as_str().to_owned();
            let approved = approve_cb(&AgentApproval::RunShell {
                command: command.to_owned(),
                mode,
                background: input.background,
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved if input.background => {
                    let started = crate::bg_shell::start_background(root, config, command)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(format!(
                        "job_id: {}\npid: {}\nstatus: running\nUse the 'shell_output' tool with this job_id to check on it, and 'kill_shell' to stop it.",
                        started.id,
                        started
                            .pid
                            .map_or_else(|| "unknown".to_string(), |p| p.to_string()),
                    ))
                }
                ApprovalOutcome::Approved => run_shell(root, config, chat_id, command).await,
                ApprovalOutcome::Denied => Ok(format!("User denied shell command: {}", command)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "shell_output" => {
            let job_id = required(&input.job_id, "job_id")?;
            crate::bg_shell::poll_output(job_id)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))
        }
        "kill_shell" => {
            let job_id = required(&input.job_id, "job_id")?;
            crate::bg_shell::kill_job(job_id).map_err(|e| OrchestrationError::Agent(e.to_string()))
        }
        "verify" => {
            if input.commands.is_empty() {
                return Err(OrchestrationError::Agent(
                    "verify requires at least one command".into(),
                ));
            }
            let mut output = Vec::new();
            for command in &input.commands {
                output.push(run_shell(root, config, chat_id, command).await?);
            }
            Ok(output.join("\n\n"))
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::shell::execute: {action}"
        ),
    }
}

use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to plugins mcp.
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
        "run_plugin" => {
            let name = required(&input.name, "name")?;
            let instruction = required(&input.instruction, "instruction")?;
            let approved = approve_cb(&AgentApproval::RunPlugin {
                name: name.to_owned(),
                instruction: instruction.to_owned(),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => Ok(execute_native_plugin(config, name, instruction)
                    .await
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => Ok(format!("User denied plugin execution: {}", name)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "dispatch_subagent" => {
            let name = required(&input.name, "name")?;
            let task = required(&input.instruction, "instruction")?;
            dispatch_one_subagent(root, config, chat_id, name, task, approve_cb).await
        }
        "mcp_tool" => {
            let server = required(&input.server, "server")?;
            let tool = required(&input.tool, "tool")?;
            let approved = approve_cb(&AgentApproval::McpTool {
                server: server.to_owned(),
                tool: tool.to_owned(),
                arguments: input.arguments.clone(),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => Ok(serde_json::to_string_pretty(
                    &crate::mcp::call_mcp_tool(config, server, tool, input.arguments.clone())
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => {
                    Ok(format!("User denied MCP tool call: {} {}", server, tool))
                }
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "mcp_list_tools" => {
            let server = required(&input.server, "server")?;
            let approved = approve_cb(&AgentApproval::McpTool {
                server: server.to_owned(),
                tool: "list_tools".to_owned(),
                arguments: serde_json::json!({}),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => Ok(serde_json::to_string_pretty(
                    &crate::mcp::list_server_tools(config, server)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => Ok(format!("User denied MCP list tools: {}", server)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::plugins_mcp::execute: {action}"
        ),
    }
}

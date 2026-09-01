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
    progress: &mut (dyn FnMut(AgentProgress) + Send),
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
            dispatch_one_subagent(root, config, chat_id, name, task, approve_cb, progress).await
        }
        "mcp_tool" => {
            let server = required(&input.server, "server")?;
            let tool = required(&input.tool, "tool")?;
            let args = input.arguments.clone();
            let run = |cfg: &MintConfig| -> Result<String, OrchestrationError> {
                serde_json::to_string_pretty(
                    &crate::mcp::call_mcp_tool(cfg, server, tool, args.clone())
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))
            };
            mcp_approve_then_run(server, tool, &args, config, approve_cb, run)
        }
        "mcp_list_tools" => {
            let server = required(&input.server, "server")?;
            let run = |cfg: &MintConfig| -> Result<String, OrchestrationError> {
                serde_json::to_string_pretty(
                    &crate::mcp::list_server_tools(cfg, server)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))
            };
            mcp_approve_then_run(
                server,
                "list_tools",
                &serde_json::json!({}),
                config,
                approve_cb,
                run,
            )
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::plugins_mcp::execute: {action}"
        ),
    }
}

/// Shared gate for `mcp_tool` / `mcp_list_tools`:
///  * a server already trusted via `allowedMcpTools` runs `run` with no prompt;
///  * otherwise the user is asked. "Allow all (*)" (the
///    [`MCP_ALLOW_ALL_SENTINEL`] answer) persists `allowedMcpTools[server] =
///    ["*"]` and then runs against the freshly reloaded config, so the same
///    server never prompts again;
///  * a plain approve runs once; deny / free-text feedback is returned as-is.
fn mcp_approve_then_run(
    server: &str,
    tool: &str,
    arguments: &serde_json::Value,
    config: &MintConfig,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    run: impl Fn(&MintConfig) -> Result<String, OrchestrationError>,
) -> Result<String, OrchestrationError> {
    if crate::mcp::is_mcp_tool_allowed(config, server, tool) {
        return run(config);
    }

    let outcome = approve_cb(&AgentApproval::McpTool {
        server: server.to_owned(),
        tool: tool.to_owned(),
        arguments: arguments.clone(),
    })
    .map_err(OrchestrationError::Agent)?;

    match outcome {
        ApprovalOutcome::Approved => run(config),
        ApprovalOutcome::Intercepted(answer) if answer == MCP_ALLOW_ALL_SENTINEL => {
            crate::mcp::allow_mcp_tool(server, "*")
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let reloaded =
                crate::load_config().map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            run(&reloaded)
        }
        ApprovalOutcome::Denied => Ok(format!("User denied MCP tool call: {server} {tool}")),
        ApprovalOutcome::Intercepted(obs) => Ok(obs),
    }
}

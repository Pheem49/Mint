use std::path::Path;

use super::super::*;
use crate::integrations::mcp_result::{ToolOutcome, ToolStatus, normalize_mcp_result};

pub(in crate::orchestration) async fn execute_mcp_outcome(
    action: &str,
    input: &AgentInput,
    config: &MintConfig,
    chat_id: &str,
    call_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<ToolOutcome, OrchestrationError> {
    execute_reviewed(
        action, input, config, chat_id, call_id, None, approve, progress,
    )
    .await
}

fn authorize(
    cfg: &mut MintConfig,
    server: &str,
    tool: &str,
    arguments: Value,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<Option<ToolOutcome>, OrchestrationError> {
    super::super::run_control::check()?;
    if crate::mcp::is_mcp_tool_allowed(cfg, server, tool) {
        return Ok(None);
    }
    let approval = approve(&AgentApproval::McpTool {
        server: server.into(),
        tool: tool.into(),
        arguments,
    })
    .map_err(OrchestrationError::Agent)?;
    super::super::run_control::check()?;
    match approval {
        ApprovalOutcome::Approved => {
            crate::mcp::allow_tool_in(cfg, server, tool);
        }
        ApprovalOutcome::Intercepted(answer) if answer == MCP_ALLOW_ALL_SENTINEL => {
            crate::mcp::allow_mcp_tool(server, "*")
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            crate::mcp::allow_tool_in(cfg, server, "*");
        }
        ApprovalOutcome::Denied => {
            return Ok(Some(ToolOutcome::from(format!(
                "User denied MCP tool call: {server} {tool}"
            ))));
        }
        ApprovalOutcome::Intercepted(answer) => {
            return Ok(Some(ToolOutcome {
                text: format!(
                    "MCP call was not approved; no command was executed. User feedback: {answer}"
                ),
                status: ToolStatus::Failed,
                ..Default::default()
            }));
        }
    }
    Ok(None)
}

fn failure(error: crate::mcp::McpError) -> ToolOutcome {
    use crate::mcp::McpError;
    let status = match error {
        McpError::Timeout | McpError::TimeoutUndelivered => ToolStatus::TimedOut,
        McpError::Cancelled | McpError::CancelledUndelivered => ToolStatus::Cancelled,
        _ => ToolStatus::Failed,
    };
    let text = if status == ToolStatus::TimedOut {
        format!(
            "Error: {error}; remote completion is unconfirmed. Do not automatically retry this operation."
        )
    } else {
        format!("Error: {error}")
    };
    ToolOutcome {
        text,
        status,
        ..Default::default()
    }
}

/// Preparation returns an observation, never a command execution record.
pub(in crate::orchestration) async fn prepare(
    input: &AgentInput,
    config: &MintConfig,
    chat: &str,
    review: &mut mcp_review::McpReview,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<Option<ToolOutcome>, OrchestrationError> {
    let server = required(&input.server, "server")?;
    let tool = required(&input.tool, "tool")?;
    // Reading definitions is separately authorized and does not grant execution.
    if !review.discovery_grants.contains(server)
        && !crate::mcp::is_mcp_tool_allowed(config, server, tool)
        && !crate::mcp::is_mcp_tool_allowed(config, server, "list_tools")
    {
        let mut cfg = config.clone();
        if let Some(out) = authorize(
            &mut cfg,
            server,
            "list_tools",
            serde_json::json!({}),
            approve,
        )? {
            return Ok(Some(out));
        }
        review.discovery_grants.insert(server.into());
    }
    let definition = match crate::mcp::catalog::describe_tool(config, server, tool, chat).await {
        Ok(d) => d,
        Err(e) => return Ok(Some(failure(e))),
    };
    let mut read = review.knows(server, tool, &definition.fingerprint, &definition.value);
    if let Some(status) = crate::mcp::device::status_tool(&definition)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?
    {
        let status_definition = crate::mcp::catalog::describe_tool(config, server, &status, chat)
            .await
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
        read = review.knows(
            server,
            &status,
            &status_definition.fingerprint,
            &status_definition.value,
        ) && read;
    }
    if read {
        return Ok(None);
    }
    let message="Tool definition loaded for the next model request. No tools/call was sent. Read its description and inputSchema, then submit the command with valid arguments.".to_string();
    progress(AgentProgress::ToolDiscovery {
        server: server.into(),
        tool: tool.into(),
        message: "Tool definition ready; preparing the command.".into(),
    });
    Ok(Some(ToolOutcome::from(message)))
}

#[allow(clippy::too_many_arguments)]
pub(in crate::orchestration) async fn execute_reviewed(
    action: &str,
    input: &AgentInput,
    config: &MintConfig,
    chat_id: &str,
    call_id: &str,
    expected: Option<&mcp_review::McpReview>,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<ToolOutcome, OrchestrationError> {
    let server = required(&input.server, "server")?;
    let tool = if action == "mcp_tool" {
        required(&input.tool, "tool")?
    } else {
        "list_tools"
    };
    let mut cfg = config.clone();
    if action == "mcp_list_tools" {
        if let Some(out) = authorize(&mut cfg, server, tool, input.arguments.clone(), approve)? {
            return Ok(out);
        }
        return Ok(
            match crate::mcp::catalog::list_tools(&cfg, server, chat_id).await {
                Ok(v) => ToolOutcome::from(serde_json::to_string_pretty(&v).unwrap_or_default()),
                Err(e) => failure(e),
            },
        );
    }
    let definition = match crate::mcp::catalog::describe_tool(&cfg, server, tool, chat_id).await {
        Ok(d) => d,
        Err(e) => return Ok(failure(e)),
    };
    if let Err(e) = definition.validate(&input.arguments) {
        return Ok(failure(e));
    }
    if expected
        .is_some_and(|r| r.fingerprint(server, tool) != Some(definition.fingerprint.as_str()))
    {
        return Ok(failure(crate::mcp::McpError::Preflight(
            "Definition changed; read it again before executing".into(),
        )));
    }
    let status_name = match crate::mcp::device::status_tool(&definition) {
        Ok(s) => s,
        Err(e) => return Ok(failure(e)),
    };
    let status_definition = if let Some(name) = &status_name {
        let d = match crate::mcp::catalog::describe_tool(&cfg, server, name, chat_id).await {
            Ok(d) => d,
            Err(e) => return Ok(failure(e)),
        };
        if expected.is_some_and(|r| r.fingerprint(server, name) != Some(d.fingerprint.as_str())) {
            return Ok(failure(crate::mcp::McpError::Preflight(
                "Status definition changed; read it again before executing".into(),
            )));
        }
        let preflight = (|| {
            definition.output_validator()?;
            d.valid_schema()?;
            d.output_validator()?;
            if crate::mcp::device::status_tool(&d)?.is_some() {
                return Err(crate::mcp::McpError::Preflight(
                    "statusTool must be a read-only observation tool without command metadata"
                        .into(),
                ));
            }
            d.validate(&serde_json::json!({"deviceId":input.arguments["deviceId"],"operationId":uuid::Uuid::new_v4().to_string()}))
        })();
        if let Err(e) = preflight {
            return Ok(failure(e));
        }
        Some(d)
    } else {
        None
    };
    if let Some(out) = authorize(&mut cfg, server, tool, input.arguments.clone(), approve)? {
        return Ok(out);
    }
    if let (Some(name), Some(status_definition)) = (status_name, status_definition) {
        if let Some(out) = authorize(
            &mut cfg,
            server,
            &name,
            serde_json::json!({"deviceId":input.arguments["deviceId"],"operationId":"<operationId returned by this command>","scope":"Read status only for this command until its deadline"}),
            approve,
        )? {
            return Ok(out);
        }
        // Both event callbacks share the sink but are invoked sequentially.
        let sink = std::sync::Mutex::new(progress);
        let result = crate::mcp::device::call_verified(
            &cfg,
            server,
            tool,
            input.arguments.clone(),
            chat_id,
            definition,
            &name,
            status_definition,
            |update| {
                sink.lock().unwrap()(AgentProgress::ToolProgress {
                    call_id: call_id.into(),
                    server: server.into(),
                    tool: tool.into(),
                    update,
                })
            },
            |update| {
                sink.lock().unwrap()(AgentProgress::DeviceState {
                    call_id: call_id.into(),
                    server: server.into(),
                    tool: tool.into(),
                    update,
                })
            },
        )
        .await;
        let mut out = normalize_mcp_result(result.receipt, chat_id, call_id, server);
        out.status = result.status;
        out.completion_unconfirmed = result.completion_unconfirmed;
        out.text.push_str(&format!("\n{}", result.message));
        if out.status != ToolStatus::Success && !out.text.starts_with("Error:") {
            out.text.insert_str(0, "Error: ");
        }
        return Ok(out);
    }
    Ok(
        match crate::mcp::call_mcp_tool_reviewed(
            &cfg,
            server,
            tool,
            input.arguments.clone(),
            chat_id,
            Some(&definition.fingerprint),
            |update| {
                progress(AgentProgress::ToolProgress {
                    call_id: call_id.into(),
                    server: server.into(),
                    tool: tool.into(),
                    update,
                })
            },
        )
        .await
        {
            Ok(v) => normalize_mcp_result(v, chat_id, call_id, server),
            Err(e) => failure(e),
        },
    )
}

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
        "mcp_tool" | "mcp_list_tools" => execute_mcp_outcome(
            action,
            input,
            config,
            chat_id,
            &uuid::Uuid::new_v4().to_string(),
            approve_cb,
            progress,
        )
        .await
        .map(|out| out.text),
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::plugins_mcp::execute: {action}"
        ),
    }
}

use super::super::*;

/// Validates an `avatar_signal` tool call and reports back to the model —
/// purely advisory, no side effect of its own. The actual push to the
/// relay happens in `AvatarBridge::on_agent_progress`, which special-cases
/// this action's `ToolStart` event and runs the exact same
/// `crate::avatar_bridge::parse_avatar_signal` validation on the exact same
/// JSON shape this function builds — see that module's docs for why the
/// side effect can't live here (tool executors have no reference to
/// `AvatarBridge`, which is owned by the Tauri/CLI caller, not orchestration).
pub(in crate::orchestration) async fn execute(
    input: &AgentInput,
) -> Result<String, OrchestrationError> {
    let value = serde_json::to_value(input).unwrap_or(Value::Null);
    match crate::avatar_bridge::parse_avatar_signal(&value) {
        Ok(_) => Ok("Avatar signal sent.".into()),
        // A validation problem is feedback for the model to correct on its
        // next call, not a hard failure — same style as other tools that
        // return a descriptive string rather than erroring out.
        Err(reason) => Ok(reason),
    }
}

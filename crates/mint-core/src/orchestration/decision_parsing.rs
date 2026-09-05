use super::*;

pub(super) fn action_fingerprint(decision: &AgentDecision) -> String {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" | "read_file" | "symbols" => {
            format!("{}:{}", decision.action, input.path.trim())
        }
        "search_code" | "semantic_search" | "web_search" | "knowledge_search" | "memory_recall" => {
            format!(
                "{}:{}:{}",
                decision.action,
                input.path.trim(),
                input.query.trim()
            )
        }
        "git_status" | "git_branch" | "detect_project" | "list_tests" | "read_diagnostics" => {
            format!("{}:{}", decision.action, input.path.trim())
        }
        "git_diff" => format!("git_diff:{}", input.path.trim()),
        "git_log" => format!("git_log:{}", input.limit.unwrap_or(5)),
        "create_plan" | "update_plan" => format!("{}:{}", decision.action, input.steps.join("\n")),
        "request_user_approval" => format!("request_user_approval:{}", input.summary.trim()),
        "ask_user" => format!("ask_user:{}", input.query.trim()),
        "view_image" => format!("view_image:{}", input.path.trim()),
        "run_shell" => format!("run_shell:{}", input.command.trim()),
        "verify" => format!("verify:{}", input.commands.join("\n")),
        "apply_patch" => input
            .patch
            .as_ref()
            .map(|patch| format!("apply_patch:{}", patch.path.display()))
            .unwrap_or_else(|| "apply_patch:<missing>".to_owned()),
        "write_file" => format!("write_file:{}", input.path.trim()),
        other => other.to_owned(),
    }
}

pub(super) fn parse_decision(raw: &str) -> Result<AgentDecision, OrchestrationError> {
    if let Ok(decision) = parse_agent_json(raw) {
        return Ok(decision);
    }
    parse_shorthand_finish(raw).map_err(|e| OrchestrationError::Agent(e.to_string()))
}

pub(super) fn parse_agent_json<T: serde::de::DeserializeOwned>(
    raw: &str,
) -> Result<T, OrchestrationError> {
    serde_json::from_str(raw).or_else(|_| {
        let start = raw
            .find('{')
            .ok_or_else(|| OrchestrationError::Agent("missing JSON object".into()))?;
        let end = raw
            .rfind('}')
            .ok_or_else(|| OrchestrationError::Agent("missing JSON object".into()))?;
        serde_json::from_str(&raw[start..=end])
            .map_err(|error| OrchestrationError::Agent(error.to_string()))
    })
}

pub(super) fn parse_shorthand_finish(raw: &str) -> Result<AgentDecision, serde_json::Error> {
    let value: Value = serde_json::from_str(raw)?;
    let finish = value.get("finish").cloned().unwrap_or(Value::Null);
    let input = match finish {
        Value::Object(_) => serde_json::from_value(finish)?,
        Value::String(s) => AgentInput {
            summary: s,
            ..AgentInput::default()
        },
        _ => AgentInput::default(),
    };
    Ok(AgentDecision {
        thought: value
            .get("thought")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        action: "finish".into(),
        input,
    })
}

pub(super) fn parse_decision_or_finish(raw: &str) -> Result<AgentDecision, OrchestrationError> {
    match parse_decision(raw) {
        Ok(decision) => Ok(decision),
        Err(_) if !raw.trim().is_empty() && !raw.contains("\"action\"") => Ok(AgentDecision {
            thought: String::new(),
            action: "finish".into(),
            input: AgentInput {
                summary: raw.trim().into(),
                ..AgentInput::default()
            },
        }),
        Err(error) => Err(error),
    }
}

/// When a `finish` attempt is rejected (empty summary, missing verification, ...),
/// native tool-calling mode's `messages` history must record both the model's
/// attempted finish and the rejection, or the next request would just resend the
/// same history with no signal anything was wrong: the JSON-prompt path gets the
/// rejection via `observation` (rebuilt from `trajectory` at each call site above),
/// but native mode stops reading `observation` once `native_messages` is non-empty
/// (see the `native_messages.is_empty()` guard near the top of the step loop).
/// No-op outside native mode, where `observation` alone is sufficient.
pub(super) fn reject_native_finish(
    tool_mode: ToolCallingMode,
    native_messages: &mut Vec<ChatMessage>,
    response_text: &str,
    rejection: &str,
) {
    if tool_mode != ToolCallingMode::Native {
        return;
    }
    if !response_text.trim().is_empty() {
        native_messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::Text {
                text: response_text.trim().to_string(),
            }],
        });
    }
    native_messages.push(ChatMessage {
        role: ChatRole::User,
        content: vec![ContentBlock::Text {
            text: rejection.to_string(),
        }],
    });
}

/// Maximum number of `dispatch_subagent` calls run concurrently when a single
/// model turn requests several of them at once (see `orchestrate_agent_loop`'s
/// parallel-dispatch branch). Kept low: each subagent makes its own AI-provider
/// API calls, so a higher cap risks tripping the configured provider's rate
/// limit, and most tasks don't naturally decompose into more than a couple of
/// truly independent pieces anyway.
pub(super) const PARALLEL_SUBAGENT_LIMIT: usize = 2;

/// Whether this step's decisions should run as a concurrency-limited batch of
/// subagent dispatches instead of the normal one-at-a-time loop: 2 or more
/// decisions, every one of them a `dispatch_subagent` call with nothing else
/// mixed in. A lone subagent call, or one mixed with other actions, stays on
/// the sequential path — keeps ordering between subagent results and other
/// tool results simple, and avoids parallelizing tools that were never
/// verified to be safe to run concurrently.
pub(super) fn decisions_are_parallel_subagent_batch(decisions: &[(String, AgentDecision)]) -> bool {
    decisions.len() >= 2
        && decisions
            .iter()
            .all(|(_, d)| d.action == "dispatch_subagent")
}

/// Maximum number of read-only tools run concurrently when a single model turn
/// requests several of them at once.
pub(super) const PARALLEL_READ_ONLY_LIMIT: usize = 6;

/// Whether an action is a pure read-only tool that is safe to run concurrently
/// with other read-only tools in the same step.
pub(super) fn is_parallelizable_read_only_tool(action: &str) -> bool {
    matches!(
        action,
        "read_file"
            | "list_files"
            | "search_code"
            | "symbols"
            | "semantic_search"
            | "knowledge_search"
            | "web_search"
            | "fetch_web_page"
            | "weather"
            | "stock"
            | "calculation"
            | "memory_recall"
            | "git_status"
            | "git_diff"
            | "git_log"
            | "git_branch"
            | "detect_project"
            | "list_tests"
            | "read_diagnostics"
            | "view_image"
            | "video_filmstrip"
            | "video_waveform"
            | "mcp_list_tools"
    )
}

/// Whether this step's decisions should run as a concurrency-limited batch of
/// read-only tool calls instead of the normal sequential loop: 2 or more decisions,
/// every one of them a safe read-only tool with no mutating actions mixed in.
pub(super) fn decisions_are_parallel_read_only_batch(decisions: &[(String, AgentDecision)]) -> bool {
    decisions.len() >= 2
        && decisions
            .iter()
            .all(|(_, d)| is_parallelizable_read_only_tool(&d.action))
}


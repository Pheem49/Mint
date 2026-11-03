use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::CodePatchHunk;
use crate::chat::{ChatMessage, ChatRole, ContentBlock};
use crate::config::ToolCallingMode;

use super::types::OrchestrationError;
use super::verification::shell_result_failed;

pub(super) const MAX_TOOL_ARGUMENT_RETRIES: usize = 5;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentDirectoryEntry {
    pub(super) name: String,
    pub(super) path: PathBuf,
    pub(super) kind: &'static str,
    pub(super) size: Option<u64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AgentDecision {
    #[serde(default)]
    pub(crate) thought: String,
    pub(crate) action: String,
    #[serde(default, deserialize_with = "deserialize_agent_input")]
    pub(crate) input: AgentInput,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SwarmTaskInput {
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub base: Option<String>,
    #[serde(default)]
    pub instruction: String,
}

impl From<SwarmTaskInput> for crate::swarm::SwarmTask {
    fn from(input: SwarmTaskInput) -> Self {
        Self {
            role: input.role,
            base: input.base,
            instruction: input.instruction,
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AgentInput {
    #[serde(default)]
    pub(super) tasks: Vec<SwarmTaskInput>,
    #[serde(default)]
    pub(super) path: String,
    #[serde(default)]
    pub(super) artifact_id: String,
    #[serde(
        default,
        alias = "q",
        alias = "keyword",
        alias = "search",
        alias = "searchTerm",
        alias = "search_term"
    )]
    pub(super) query: String,
    #[serde(default, alias = "ticker")]
    pub(super) symbol: String,
    #[serde(default)]
    pub(super) filter: String,
    #[serde(default)]
    pub(super) options: Vec<AskUserOptionInput>,
    #[serde(default)]
    pub(super) header: String,
    #[serde(default, alias = "multi_select")]
    pub(super) multi_select: bool,
    #[serde(default, alias = "location", alias = "place")]
    pub(super) city: String,
    #[serde(default, alias = "expr", alias = "math")]
    pub(super) expression: String,
    #[serde(
        default,
        alias = "cmd",
        alias = "script",
        alias = "exec",
        alias = "code",
        alias = "bash",
        alias = "sh"
    )]
    pub(super) command: String,
    #[serde(default)]
    pub(super) background: bool,
    #[serde(default, alias = "retryReason")]
    pub(super) retry_reason: String,
    #[serde(default, alias = "job_id")]
    pub(super) job_id: String,
    #[serde(default)]
    pub(super) commands: Vec<String>,
    #[serde(default)]
    pub(super) steps: Vec<String>,
    #[serde(default, alias = "file_content")]
    pub(super) file_content: String,
    #[serde(default)]
    pub(super) summary: String,
    #[serde(default)]
    pub(super) verification: String,
    #[serde(default)]
    pub(super) completion_status: String,
    #[serde(default)]
    pub(super) plan: String,
    #[serde(default)]
    pub(super) reason: String,
    #[serde(default, alias = "start_line")]
    pub(super) start_line: Option<usize>,
    #[serde(default, alias = "end_line")]
    pub(super) end_line: Option<usize>,
    #[serde(default)]
    pub(super) limit: Option<usize>,
    #[serde(default)]
    pub(super) patch: Option<AgentPatch>,
    #[serde(default)]
    pub(super) server: String,
    #[serde(default)]
    pub(super) tool: String,
    #[serde(default)]
    pub(super) arguments: Value,
    #[serde(default, alias = "note_path")]
    pub(super) note_path: String,
    #[serde(default)]
    pub(super) name: String,
    #[serde(default)]
    pub(super) instruction: String,
    #[serde(default)]
    pub(super) title: String,
    #[serde(default)]
    pub(super) status: String,
    #[serde(default)]
    pub(super) step: Option<usize>,
    #[serde(default)]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) content: String,
    #[serde(default)]
    pub(super) url: String,
    #[serde(default)]
    pub(super) selector: String,
    #[serde(default)]
    pub(super) text: String,
    #[serde(default)]
    pub(super) x: Option<f64>,
    #[serde(default)]
    pub(super) y: Option<f64>,
    #[serde(default)]
    pub(super) button: String,
    #[serde(default)]
    pub(super) key: String,
    #[serde(default)]
    pub(super) element_ref: String,
    #[serde(default)]
    pub(super) tab_id: String,
    #[serde(default)]
    pub(super) operation: String,
    #[serde(default)]
    pub(super) mode: String,
    #[serde(default)]
    pub(super) condition: String,
    #[serde(default)]
    pub(super) value: String,
    #[serde(default)]
    pub(super) timeout_ms: Option<u64>,
    #[serde(default)]
    pub(super) text_offset: Option<u64>,
    #[serde(default)]
    pub(super) element_offset: Option<u64>,
    #[serde(default)]
    pub(super) takeover_complete: bool,
    // Video tools input fields
    #[serde(default)]
    pub(super) input: String,
    #[serde(default)]
    pub(super) output: String,
    #[serde(default)]
    pub(super) start: Option<f64>,
    #[serde(default)]
    pub(super) end: Option<f64>,
    #[serde(default)]
    pub(super) width: Option<i32>,
    #[serde(default)]
    pub(super) height: Option<i32>,
    #[serde(default)]
    pub(super) frame_count: Option<u32>,
    #[serde(default)]
    pub(super) columns: Option<u32>,
    #[serde(default)]
    pub(super) threshold_db: Option<f64>,
    #[serde(default)]
    pub(super) min_silence_secs: Option<f64>,
    #[serde(default)]
    pub(super) language: Option<String>,
    #[serde(default)]
    pub(super) target_language: Option<String>,
    #[serde(default)]
    pub(super) srt_content: Option<String>,
    #[serde(default)]
    pub(super) preset: Option<String>,
    #[serde(default)]
    pub(super) max_clips: Option<u32>,
    #[serde(default)]
    pub(super) target_duration: Option<f64>,
    #[serde(default)]
    pub(super) inputs: Vec<String>,
    #[serde(default)]
    pub(super) order: Vec<usize>,
    #[serde(default)]
    pub(super) music_input: Option<String>,
    #[serde(default)]
    pub(super) video_input: Option<String>,
    #[serde(default)]
    pub(super) music_volume: Option<f32>,
    #[serde(default)]
    pub(super) zoom_factor: Option<f32>,
    // Image & Video generation input fields
    #[serde(default)]
    pub(super) prompt: String,
    #[serde(default)]
    pub(super) aspect_ratio: String,
    #[serde(default)]
    pub(super) provider: String,
    #[serde(default)]
    pub(super) duration: Option<f64>,
    #[serde(default, rename = "emotions")]
    pub(super) avatar_emotions: std::collections::HashMap<String, String>,
    #[serde(default, rename = "action")]
    pub(super) avatar_action: String,
    #[serde(default, rename = "prop")]
    pub(super) avatar_prop: String,
    #[serde(default, rename = "intensity")]
    pub(super) avatar_intensity: String,
    #[serde(default, rename = "color")]
    pub(super) avatar_color: String,
    #[serde(default, rename = "talking")]
    pub(super) avatar_talking: Option<bool>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub(super) enum AskUserOptionInput {
    Plain(String),
    Detailed {
        label: String,
        #[serde(default)]
        description: Option<String>,
    },
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentPatch {
    pub(super) path: PathBuf,
    #[serde(default)]
    pub(super) hunks: Vec<CodePatchHunk>,
}

pub(super) fn deserialize_agent_input<'de, D>(deserializer: D) -> Result<AgentInput, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value = Option::<Value>::deserialize(deserializer)?;
    value
        .map(decode_agent_input)
        .transpose()
        .map_err(serde::de::Error::custom)
        .map(|input| input.unwrap_or_default())
}

/// Returns true if a string is an AI dummy placeholder such as `"null"`, `"undefined"`, `"none"`, etc.
pub fn is_dummy_value(s: &str) -> bool {
    let lower = s.trim().to_ascii_lowercase();
    matches!(
        lower.as_str(),
        "null" | "undefined" | "none" | "nil" | "(null)" | "\"\"" | "''"
    )
}

pub(super) fn prune_null_values(value: &mut Value) {
    if let Value::Object(map) = value {
        map.retain(|k, v| {
            if v.is_null() && k != "path" {
                return false;
            }
            if let Value::String(s) = v {
                if is_dummy_value(s) {
                    return false;
                }
            }
            true
        });
        for v in map.values_mut() {
            prune_null_values(v);
        }
    } else if let Value::Array(arr) = value {
        for v in arr.iter_mut() {
            prune_null_values(v);
        }
    }
}

pub(super) fn decode_agent_input(mut value: Value) -> Result<AgentInput, String> {
    if let Value::String(s) = &value {
        let trimmed = s.trim();
        if trimmed.starts_with('{') && trimmed.ends_with('}') {
            if let Ok(parsed) = serde_json::from_str::<Value>(trimmed) {
                if parsed.is_object() {
                    value = parsed;
                }
            }
        }
    }

    prune_null_values(&mut value);

    if let Value::Object(map) = &mut value {
        let has_command = map
            .get("command")
            .or_else(|| map.get("cmd"))
            .or_else(|| map.get("script"))
            .or_else(|| map.get("exec"))
            .or_else(|| map.get("code"))
            .or_else(|| map.get("bash"))
            .or_else(|| map.get("sh"))
            .and_then(Value::as_str)
            .is_some_and(|s| !s.trim().is_empty());

        if !has_command {
            if let Some(args_val) = map.get("arguments").cloned() {
                let inner_obj = match args_val {
                    Value::Object(inner_map) => Some(inner_map),
                    Value::String(inner_str) => serde_json::from_str::<Value>(&inner_str)
                        .ok()
                        .and_then(|v| match v {
                            Value::Object(m) => Some(m),
                            _ => None,
                        }),
                    _ => None,
                };
                if let Some(inner) = inner_obj {
                    let mut wrapped = Value::Object(inner);
                    prune_null_values(&mut wrapped);
                    if let Value::Object(cleaned) = wrapped {
                        for (k, v) in cleaned {
                            if !map.contains_key(&k) && !v.is_null() {
                                map.insert(k, v);
                            }
                        }
                    }
                }
            }
        }
    }

    let outer_path = value.get("path").and_then(Value::as_str).map(str::to_owned);
    if let Some(patch) = value.get_mut("patch").filter(|patch| !patch.is_null()) {
        if let Value::String(encoded) = patch {
            *patch = serde_json::from_str(encoded).map_err(|_| {
                "patch must contain a valid JSON object with path and hunks".to_owned()
            })?;
        }
        let object = patch
            .as_object_mut()
            .ok_or_else(|| "patch must be an object with path and hunks".to_owned())?;
        if !object.contains_key("path") {
            if let Some(path) = &outer_path {
                object.insert("path".into(), Value::String(path.clone()));
            }
        }
        let path = object
            .get("path")
            .and_then(Value::as_str)
            .filter(|path| !path.trim().is_empty())
            .ok_or_else(|| {
                "patch.path is required; supply the target file explicitly".to_owned()
            })?;
        if outer_path
            .as_deref()
            .is_some_and(|outer| !outer.is_empty() && outer != path)
        {
            return Err("path and patch.path must name the same target file".into());
        }
        serde_json::from_value::<AgentPatch>(patch.clone())
            .map_err(|_| "patch requires a string path and hunks containing string oldText/newText and optional boolean replaceAll".to_owned())?;
    }
    serde_json::from_value(value).map_err(|error| {
        let message = error.to_string();
        if message.chars().count() > 400 {
            format!(
                "{}… (argument value omitted)",
                message.chars().take(400).collect::<String>()
            )
        } else {
            message
        }
    })
}

pub(super) fn parse_tool_input(
    action: &str,
    input: Value,
) -> Result<AgentInput, OrchestrationError> {
    let normalized = if action == "run_shell" {
        match &input {
            Value::String(cmd) if !cmd.trim().is_empty() => {
                if let Ok(val) = serde_json::from_str::<Value>(cmd)
                    && val.is_object()
                {
                    val
                } else {
                    serde_json::json!({ "command": cmd.trim() })
                }
            }
            _ => input,
        }
    } else {
        input
    };
    decode_agent_input(normalized).map_err(|error| {
        OrchestrationError::Agent(format!("invalid arguments for tool '{action}': {error}"))
    })
}

#[derive(Debug, Clone)]
pub(super) struct ToolResultEntry {
    pub(super) call_id: String,
    pub(super) action: String,
    pub(super) input: Value,
    pub(super) text: String,
    pub(super) status: crate::mcp_result::ToolStatus,
}

impl From<(String, String, Value, String)> for ToolResultEntry {
    fn from((call_id, action, input, text): (String, String, Value, String)) -> Self {
        let mut status = crate::mcp_result::ToolOutcome::from(text.clone()).status;
        if text.starts_with("Skipped") {
            status = crate::mcp_result::ToolStatus::Skipped;
        } else if matches!(
            action.as_str(),
            "run_shell" | "verify" | "run_tests" | "run_typecheck" | "run_linter"
        ) && shell_result_failed(&text)
        {
            status = crate::mcp_result::ToolStatus::Failed;
        }
        Self {
            call_id,
            action,
            input,
            text,
            status,
        }
    }
}

pub(super) type PreparedToolCalls = (Vec<(String, AgentDecision)>, Vec<ToolResultEntry>);

#[derive(Debug, Default)]
pub(super) struct ToolArgumentRetries {
    pub(super) failures: BTreeMap<String, usize>,
}

impl ToolArgumentRetries {
    pub(super) fn succeeded(&mut self, tool: &str) {
        self.failures.remove(tool);
    }

    pub(super) fn observe_completed(&mut self, results: &[ToolResultEntry]) {
        for ToolResultEntry {
            action: tool,
            status,
            ..
        } in results
        {
            if *status == crate::mcp_result::ToolStatus::Success {
                self.succeeded(tool);
            }
        }
    }
}

pub(super) fn prepare_native_tool_calls(
    calls: Vec<crate::chat::ToolCall>,
    thought: &str,
    retries: &mut ToolArgumentRetries,
) -> Result<PreparedToolCalls, OrchestrationError> {
    let mut decisions = Vec::new();
    let mut errors = Vec::new();
    for (index, call) in calls.into_iter().enumerate() {
        match parse_tool_input(&call.name, call.input.clone()) {
            Ok(input) => decisions.push((
                call.id,
                AgentDecision {
                    thought: if index == 0 {
                        thought.to_owned()
                    } else {
                        String::new()
                    },
                    action: call.name,
                    input,
                },
            )),
            Err(error) => {
                let invalid_count = retries.failures.entry(call.name.clone()).or_default();
                if *invalid_count >= MAX_TOOL_ARGUMENT_RETRIES {
                    return Err(OrchestrationError::Agent(format!(
                        "Tool '{}' exhausted its {MAX_TOOL_ARGUMENT_RETRIES} argument-correction retries without a successful execution. Other tools have independent budgets. {error}",
                        call.name
                    )));
                }
                *invalid_count += 1;
                errors.push((call.id, call.name, call.input, format!(
                    "Error: {error}. No tool was executed for this call. Correct the arguments to match the tool schema, or choose another suitable tool. Correction retries for this tool: {}/{MAX_TOOL_ARGUMENT_RETRIES}. This counter resets only after this tool executes successfully; changing arguments alone does not reset it. Other tools have independent counters. Continued failures for this tool will stop the run.",
                    *invalid_count,
                )).into());
            }
        }
    }
    Ok((decisions, errors))
}

pub(super) fn action_fingerprint(decision: &AgentDecision) -> String {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" | "symbols" | "repo_map" => {
            format!("{}:{}", decision.action, input.path.trim())
        }
        "read_file" => {
            let start = input.start_line.unwrap_or(1);
            let end = input.end_line.unwrap_or_else(|| start.saturating_add(239));
            format!("read_file:{}:{}:{}", input.path.trim(), start, end)
        }
        "search_code" | "semantic_search" | "web_search" | "knowledge_search" | "memory_recall"
        | "find_definition" | "find_references" => {
            format!(
                "{}:{}:{}:{}",
                decision.action,
                input.path.trim(),
                input.query.trim(),
                input.symbol.trim()
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
        "view_image" => format!(
            "view_image:{}:{}",
            input.path.trim(),
            input.artifact_id.trim()
        ),
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

pub(super) const PARALLEL_READ_ONLY_LIMIT: usize = 6;

pub(super) fn is_parallelizable_read_only_tool(action: &str) -> bool {
    matches!(
        action,
        "read_file"
            | "list_files"
            | "search_code"
            | "symbols"
            | "find_definition"
            | "find_references"
            | "repo_map"
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
            | "search_docs"
            | "list_tests"
            | "read_diagnostics"
            | "view_image"
            | "video_filmstrip"
            | "video_waveform"
            | "mcp_list_tools"
    )
}

pub(super) fn decisions_are_parallel_read_only_batch(
    decisions: &[(String, AgentDecision)],
) -> bool {
    decisions.len() >= 2
        && decisions
            .iter()
            .all(|(_, d)| is_parallelizable_read_only_tool(&d.action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_fingerprint_differentiates_read_file_slices() {
        let d1: AgentDecision = parse_agent_json(r#"{"action":"read_file","thought":"","input":{"path":"README.md","startLine":1,"endLine":240}}"#).unwrap();
        let d2: AgentDecision = parse_agent_json(r#"{"action":"read_file","thought":"","input":{"path":"README.md","startLine":241,"endLine":480}}"#).unwrap();
        let d3: AgentDecision = parse_agent_json(
            r#"{"action":"read_file","thought":"","input":{"path":"README.md","start_line":241}}"#,
        )
        .unwrap();

        assert_eq!(action_fingerprint(&d1), "read_file:README.md:1:240");
        assert_eq!(action_fingerprint(&d2), "read_file:README.md:241:480");
        assert_eq!(action_fingerprint(&d3), "read_file:README.md:241:480");
        assert_ne!(action_fingerprint(&d1), action_fingerprint(&d2));
    }

    #[test]
    fn agent_input_deserializes_snake_case_and_camel_case() {
        let camel: AgentDecision = parse_agent_json(r#"{"action":"read_file","thought":"","input":{"path":"README.md","startLine":10,"endLine":50}}"#).unwrap();
        let snake: AgentDecision = parse_agent_json(r#"{"action":"read_file","thought":"","input":{"path":"README.md","start_line":10,"end_line":50}}"#).unwrap();

        assert_eq!(camel.input.start_line, Some(10));
        assert_eq!(camel.input.end_line, Some(50));
        assert_eq!(snake.input.start_line, Some(10));
        assert_eq!(snake.input.end_line, Some(50));
    }
}

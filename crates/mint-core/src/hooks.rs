use std::{
    io::Write,
    path::Path,
    process::Stdio,
    sync::mpsc,
    time::Duration,
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{ConfigError, MintConfig, load_config, save_config, shell::shell_command};

const DEFAULT_HOOK_TIMEOUT_SECS: u64 = 30;
const HOOK_BLOCK_EXIT_CODE: i32 = 2;
const MAX_HOOK_OUTPUT_BYTES: usize = 4_000;

#[derive(Debug, Error)]
pub enum HookError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("invalid hooks configuration: {0}")]
    InvalidConfig(#[from] serde_json::Error),
    #[error("event must be PreToolUse or PostToolUse, got '{0}'")]
    InvalidEvent(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "PascalCase")]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
}

impl HookEvent {
    pub fn parse(value: &str) -> Result<Self, HookError> {
        match value.trim().to_lowercase().as_str() {
            "pretooluse" | "pre" => Ok(Self::PreToolUse),
            "posttooluse" | "post" => Ok(Self::PostToolUse),
            other => Err(HookError::InvalidEvent(other.to_owned())),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
        }
    }
}

fn default_timeout_secs() -> u64 {
    DEFAULT_HOOK_TIMEOUT_SECS
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HookEntry {
    pub event: HookEvent,
    pub matcher: String,
    pub command: String,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

pub fn list_hooks(config: &MintConfig) -> Vec<HookEntry> {
    config
        .extra
        .get("hooks")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .unwrap_or_default()
        .unwrap_or_default()
}

pub fn add_hook(
    event: HookEvent,
    matcher: &str,
    command: &str,
    timeout_secs: Option<u64>,
) -> Result<(), HookError> {
    let mut config = load_config()?;
    let mut hooks = list_hooks(&config);
    hooks.push(HookEntry {
        event,
        matcher: matcher.to_owned(),
        command: command.to_owned(),
        timeout_secs: timeout_secs.unwrap_or(DEFAULT_HOOK_TIMEOUT_SECS),
    });
    save_hooks(&mut config, hooks)
}

pub fn remove_hook(index: usize) -> Result<bool, HookError> {
    let mut config = load_config()?;
    let mut hooks = list_hooks(&config);
    if index >= hooks.len() {
        return Ok(false);
    }
    hooks.remove(index);
    save_hooks(&mut config, hooks)?;
    Ok(true)
}

pub fn clear_hooks() -> Result<(), HookError> {
    let mut config = load_config()?;
    save_hooks(&mut config, Vec::new())
}

fn save_hooks(config: &mut MintConfig, hooks: Vec<HookEntry>) -> Result<(), HookError> {
    config
        .extra
        .insert("hooks".into(), serde_json::to_value(hooks)?);
    Ok(save_config(config)?)
}

fn matcher_matches(matcher: &str, action: &str) -> bool {
    let matcher = matcher.trim();
    if matcher == "*" || matcher.is_empty() {
        return true;
    }
    matcher
        .split(|c| c == ',' || c == '|')
        .map(str::trim)
        .any(|candidate| candidate == action)
}

struct HookRunOutput {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    timed_out: bool,
}

fn truncate(text: &str) -> String {
    if text.len() <= MAX_HOOK_OUTPUT_BYTES {
        text.to_owned()
    } else {
        format!("{}... [truncated]", &text[..MAX_HOOK_OUTPUT_BYTES])
    }
}

fn run_hook_command(command: &str, cwd: &Path, payload: &Value, timeout_secs: u64) -> HookRunOutput {
    let mut child = match shell_command(command)
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(error) => {
            return HookRunOutput {
                exit_code: None,
                stdout: String::new(),
                stderr: format!("unable to start hook command: {}", error),
                timed_out: false,
            };
        }
    };

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(payload.to_string().as_bytes());
        // stdin is dropped here, closing the pipe so the hook can read EOF.
    }

    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let output = child.wait_with_output();
        let _ = sender.send(output);
    });

    match receiver.recv_timeout(Duration::from_secs(timeout_secs)) {
        Ok(Ok(output)) => HookRunOutput {
            exit_code: output.status.code(),
            stdout: truncate(&String::from_utf8_lossy(&output.stdout)),
            stderr: truncate(&String::from_utf8_lossy(&output.stderr)),
            timed_out: false,
        },
        Ok(Err(error)) => HookRunOutput {
            exit_code: None,
            stdout: String::new(),
            stderr: format!("hook command failed: {}", error),
            timed_out: false,
        },
        Err(_) => HookRunOutput {
            exit_code: None,
            stdout: String::new(),
            stderr: format!("hook command timed out after {}s", timeout_secs),
            timed_out: true,
        },
    }
}

pub enum PreHookOutcome {
    Allowed,
    Blocked(String),
}

pub fn run_pre_tool_hooks(
    hooks: &[HookEntry],
    action: &str,
    input: &Value,
    cwd: &Path,
) -> PreHookOutcome {
    let payload = json!({
        "event": HookEvent::PreToolUse.as_str(),
        "action": action,
        "input": input,
        "cwd": cwd.to_string_lossy(),
    });

    for hook in hooks
        .iter()
        .filter(|hook| hook.event == HookEvent::PreToolUse && matcher_matches(&hook.matcher, action))
    {
        let output = run_hook_command(&hook.command, cwd, &payload, hook.timeout_secs);
        if output.timed_out {
            continue;
        }
        if output.exit_code == Some(HOOK_BLOCK_EXIT_CODE) {
            let reason = if output.stderr.trim().is_empty() {
                format!("hook '{}' blocked this action", hook.command)
            } else {
                output.stderr.trim().to_owned()
            };
            return PreHookOutcome::Blocked(reason);
        }
    }

    PreHookOutcome::Allowed
}

pub fn run_post_tool_hooks(
    hooks: &[HookEntry],
    action: &str,
    input: &Value,
    result: &str,
    success: bool,
    cwd: &Path,
) -> Vec<String> {
    let payload = json!({
        "event": HookEvent::PostToolUse.as_str(),
        "action": action,
        "input": input,
        "result": truncate(result),
        "success": success,
        "cwd": cwd.to_string_lossy(),
    });

    let mut messages = Vec::new();
    for hook in hooks
        .iter()
        .filter(|hook| hook.event == HookEvent::PostToolUse && matcher_matches(&hook.matcher, action))
    {
        let output = run_hook_command(&hook.command, cwd, &payload, hook.timeout_secs);
        if !output.stdout.trim().is_empty() {
            messages.push(output.stdout.trim().to_owned());
        }
    }
    messages
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matcher_wildcard_matches_anything() {
        assert!(matcher_matches("*", "write_file"));
        assert!(matcher_matches("", "write_file"));
    }

    #[test]
    fn matcher_matches_comma_and_pipe_separated_lists() {
        assert!(matcher_matches("write_file,apply_patch", "apply_patch"));
        assert!(matcher_matches("write_file|apply_patch", "write_file"));
        assert!(!matcher_matches("write_file,apply_patch", "run_shell"));
    }

    #[test]
    fn pre_tool_hook_blocks_on_exit_code_two() {
        let hooks = vec![HookEntry {
            event: HookEvent::PreToolUse,
            matcher: "write_file".into(),
            command: "echo denied >&2; exit 2".into(),
            timeout_secs: 5,
        }];
        let outcome = run_pre_tool_hooks(
            &hooks,
            "write_file",
            &json!({}),
            &std::env::temp_dir(),
        );
        match outcome {
            PreHookOutcome::Blocked(reason) => assert!(reason.contains("denied")),
            PreHookOutcome::Allowed => panic!("expected the hook to block this action"),
        }
    }

    #[test]
    fn pre_tool_hook_allows_when_not_matched() {
        let hooks = vec![HookEntry {
            event: HookEvent::PreToolUse,
            matcher: "run_shell".into(),
            command: "exit 2".into(),
            timeout_secs: 5,
        }];
        let outcome = run_pre_tool_hooks(
            &hooks,
            "write_file",
            &json!({}),
            &std::env::temp_dir(),
        );
        assert!(matches!(outcome, PreHookOutcome::Allowed));
    }

    #[test]
    fn post_tool_hook_surfaces_stdout() {
        let hooks = vec![HookEntry {
            event: HookEvent::PostToolUse,
            matcher: "*".into(),
            command: "echo formatted ok".into(),
            timeout_secs: 5,
        }];
        let messages = run_post_tool_hooks(
            &hooks,
            "write_file",
            &json!({}),
            "wrote file",
            true,
            &std::env::temp_dir(),
        );
        assert_eq!(messages, vec!["formatted ok".to_string()]);
    }
}

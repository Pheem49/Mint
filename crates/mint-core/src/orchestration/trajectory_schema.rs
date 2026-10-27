//! Standardized Trajectory Schema v1.0
//!
//! Provides a typed, versioned schema for agent execution trajectories compliant with
//! SWE-bench and OpenTelemetry / Agent Protocol standards. Facilitates deterministic
//! session replays, benchmark evaluations, and multi-platform telemetry export.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::AgentProgress;

/// Token usage metadata for an individual trajectory step.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct StepTokens {
    #[serde(default)]
    pub input_tokens: u64,
    #[serde(default)]
    pub generated_tokens: u64,
}

/// A discrete, typed execution step within an agent run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StandardTrajectoryStep {
    /// 1-based step counter
    pub step: usize,
    /// RFC3339 timestamp of step completion
    pub timestamp: String,
    /// Model reasoning or thought preceding action
    pub thought: String,
    /// Tool or action invoked (e.g. "apply_patch", "run_shell", "finish")
    pub action: String,
    /// Canonical JSON arguments supplied to the action
    pub input: Value,
    /// Observation returned by environment / harness
    pub observation: String,
    /// Step outcome status ("success", "failed", "intercepted", "skipped")
    pub status: String,
    /// Wall-clock execution duration in milliseconds
    pub duration_ms: u64,
    /// Step token metrics
    pub tokens: StepTokens,
}

/// Complete execution trajectory for an agent run, compliant with SWE-bench & Agent Protocol v1.0.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StandardTrajectoryRun {
    /// Schema version, e.g. "1.0"
    pub schema_version: String,
    /// Unique identifier for this run
    pub run_id: String,
    /// Chat session identifier
    pub chat_id: String,
    /// High-level user prompt or task specification
    pub task: String,
    /// Active model name
    pub model: String,
    /// Model provider name
    pub provider: String,
    /// RFC3339 timestamp when run started
    pub started_at: String,
    /// RFC3339 timestamp when run finished
    pub completed_at: String,
    /// Final status outcome ("SUCCESS", "VERIFIED", "PARTIAL", "FAILED", "ROLLED_BACK")
    pub outcome: String,
    /// Total number of steps executed
    pub total_steps: usize,
    /// Total tokens consumed across all steps
    pub total_tokens: u64,
    /// Wall-clock duration in seconds
    pub duration_secs: f64,
    /// List of file paths created during the run
    pub files_created: Vec<String>,
    /// List of file paths modified during the run
    pub files_modified: Vec<String>,
    /// Chronological list of steps executed
    pub trajectory: Vec<StandardTrajectoryStep>,
}

impl StandardTrajectoryRun {
    /// Initializes a new StandardTrajectoryRun with default 1.0 schema version.
    pub fn new(
        run_id: impl Into<String>,
        chat_id: impl Into<String>,
        task: impl Into<String>,
        model: impl Into<String>,
        provider: impl Into<String>,
    ) -> Self {
        let now = chrono::Utc::now().to_rfc3339();
        Self {
            schema_version: "1.0".to_string(),
            run_id: run_id.into(),
            chat_id: chat_id.into(),
            task: task.into(),
            model: model.into(),
            provider: provider.into(),
            started_at: now.clone(),
            completed_at: now,
            outcome: "PENDING".to_string(),
            total_steps: 0,
            total_tokens: 0,
            duration_secs: 0.0,
            files_created: Vec::new(),
            files_modified: Vec::new(),
            trajectory: Vec::new(),
        }
    }

    /// Records an executed trajectory step and updates total_steps.
    pub fn record_step(&mut self, step: StandardTrajectoryStep) {
        self.trajectory.push(step);
        self.total_steps = self.trajectory.len();
    }

    /// Finalizes the trajectory run metadata upon completion.
    pub fn finish(
        &mut self,
        outcome: impl Into<String>,
        total_tokens: u64,
        duration_secs: f64,
        files_created: Vec<String>,
        files_modified: Vec<String>,
    ) {
        self.outcome = outcome.into();
        self.total_tokens = total_tokens;
        self.duration_secs = duration_secs;
        self.files_created = files_created;
        self.files_modified = files_modified;
        self.completed_at = chrono::Utc::now().to_rfc3339();
        self.total_steps = self.trajectory.len();
    }

    /// Serializes the run into SWE-bench compliant pretty-printed JSON.
    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    /// Deserializes a run from JSON.
    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(json)
    }

    /// Reconstructs a `StandardTrajectoryRun` from an event stream (`AgentProgress`).
    /// Useful for historical runs or external consumers where only events were preserved.
    pub fn from_agent_activity(
        chat_id: &str,
        run_id: &str,
        task: &str,
        model: &str,
        provider: &str,
        events: &[AgentProgress],
    ) -> Self {
        // First check if a RunCompleted event already has an embedded trajectory
        for event in events {
            if let AgentProgress::RunCompleted { summary } = event {
                if let Some(existing) = &summary.trajectory {
                    return existing.clone();
                }
            }
        }

        let mut run = Self::new(run_id, chat_id, task, model, provider);
        let mut latest_thought = String::new();
        let mut step_counter = 1;

        for event in events {
            match event {
                AgentProgress::Thought { thought } => {
                    latest_thought = thought.clone();
                }
                AgentProgress::ExtendedThinking { thought, .. } => {
                    latest_thought = thought.clone();
                }
                AgentProgress::ToolEnd {
                    action,
                    input,
                    result,
                    elapsed_ms,
                    status,
                    ..
                } => {
                    let status_str = match status {
                        Some(crate::integrations::mcp_result::ToolStatus::Success) => "success",
                        Some(crate::integrations::mcp_result::ToolStatus::Failed) => "failed",
                        Some(crate::integrations::mcp_result::ToolStatus::TimedOut) => "timed_out",
                        Some(crate::integrations::mcp_result::ToolStatus::Cancelled) => "cancelled",
                        Some(crate::integrations::mcp_result::ToolStatus::Skipped) => "skipped",
                        _ => {
                            if result.starts_with("Error") || result.contains("failed") {
                                "failed"
                            } else {
                                "success"
                            }
                        }
                    };

                    run.record_step(StandardTrajectoryStep {
                        step: step_counter,
                        timestamp: chrono::Utc::now().to_rfc3339(),
                        thought: std::mem::take(&mut latest_thought),
                        action: action.clone(),
                        input: input.clone(),
                        observation: result.clone(),
                        status: status_str.to_string(),
                        duration_ms: elapsed_ms.unwrap_or(0),
                        tokens: StepTokens::default(),
                    });
                    step_counter += 1;
                }
                AgentProgress::RunCompleted { summary } => {
                    if !summary.run_id.is_empty() {
                        run.run_id = summary.run_id.clone();
                    }
                    if !summary.task.is_empty() {
                        run.task = summary.task.clone();
                    }
                    run.finish(
                        &summary.outcome,
                        summary.total_tokens as u64,
                        summary.duration_secs,
                        summary.files_created.clone(),
                        summary.files_changed.clone(),
                    );
                }
                _ => {}
            }
        }

        run
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn serializes_canonical_v1_schema() {
        let mut run = StandardTrajectoryRun::new(
            "run-101",
            "chat-abc",
            "Fix bug in parser",
            "claude-3-7-sonnet",
            "anthropic",
        );
        run.record_step(StandardTrajectoryStep {
            step: 1,
            timestamp: "2026-10-26T12:00:00Z".to_string(),
            thought: "I need to inspect the parser code".to_string(),
            action: "read_file".to_string(),
            input: json!({"path": "src/parser.rs"}),
            observation: "pub fn parse() {}".to_string(),
            status: "success".to_string(),
            duration_ms: 125,
            tokens: StepTokens {
                input_tokens: 1500,
                generated_tokens: 65,
            },
        });
        run.finish(
            "SUCCESS",
            1565,
            2.5,
            vec![],
            vec!["src/parser.rs".to_string()],
        );

        let json = run.to_json_pretty().expect("must serialize");
        assert!(json.contains("\"schemaVersion\": \"1.0\""));
        assert!(json.contains("\"runId\": \"run-101\""));
        assert!(json.contains("\"chatId\": \"chat-abc\""));
        assert!(json.contains("\"inputTokens\": 1500"));
        assert!(json.contains("\"durationMs\": 125"));
        assert!(json.contains("\"outcome\": \"SUCCESS\""));
    }

    #[test]
    fn roundtrip_json_parity() {
        let mut run =
            StandardTrajectoryRun::new("run-202", "chat-xyz", "Add feature", "gpt-4o", "openai");
        run.record_step(StandardTrajectoryStep {
            step: 1,
            timestamp: "2026-10-26T12:01:00Z".to_string(),
            thought: "Checking dir".to_string(),
            action: "list_files".to_string(),
            input: json!({"path": "."}),
            observation: "Cargo.toml\nsrc/".to_string(),
            status: "success".to_string(),
            duration_ms: 42,
            tokens: StepTokens::default(),
        });
        run.finish("VERIFIED", 300, 1.2, vec!["Cargo.toml".to_string()], vec![]);

        let json_str = run.to_json_pretty().expect("must serialize");
        let parsed = StandardTrajectoryRun::from_json(&json_str).expect("must deserialize");
        assert_eq!(run, parsed);
    }

    #[test]
    fn reconstructs_from_agent_activity() {
        let events = vec![
            AgentProgress::Thought {
                thought: "Locating function".to_string(),
            },
            AgentProgress::ToolEnd {
                call_id: Some("call-1".to_string()),
                action: "grep".to_string(),
                input: json!({"query": "fn test"}),
                result: "src/lib.rs:10: fn test()".to_string(),
                status: Some(crate::integrations::mcp_result::ToolStatus::Success),
                elapsed_ms: Some(85),
                subagent: None,
            },
            AgentProgress::RunCompleted {
                summary: crate::orchestration::RunTelemetrySummary {
                    run_id: "run-303".to_string(),
                    task: "Find function".to_string(),
                    outcome: "SUCCESS".to_string(),
                    total_tokens: 1200,
                    tool_calls_count: 1,
                    files_changed: vec![],
                    files_created: vec![],
                    duration_secs: 1.5,
                    tool_timeline: vec![],
                    retries_count: 0,
                    trajectory: None,
                },
            },
        ];

        let run = StandardTrajectoryRun::from_agent_activity(
            "chat-999",
            "run-303",
            "Find function",
            "gemini-2.5-pro",
            "google",
            &events,
        );

        assert_eq!(run.schema_version, "1.0");
        assert_eq!(run.run_id, "run-303");
        assert_eq!(run.chat_id, "chat-999");
        assert_eq!(run.outcome, "SUCCESS");
        assert_eq!(run.total_steps, 1);
        assert_eq!(run.trajectory[0].action, "grep");
        assert_eq!(run.trajectory[0].thought, "Locating function");
        assert_eq!(run.trajectory[0].duration_ms, 85);
        assert_eq!(run.trajectory[0].status, "success");
    }
}

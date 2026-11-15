use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to planning.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
fn persist_plan_to_workspace(root: &Path, plan: &ActivePlan) {
    let plans_dir = root.join(".agents").join("plans");
    if let Ok(()) = std::fs::create_dir_all(&plans_dir) {
        if let Ok(json) = serde_json::to_string_pretty(plan) {
            let _ = std::fs::write(plans_dir.join("active_plan.json"), json);
        }
    }
}

fn load_plan_from_workspace(root: &Path) -> Option<ActivePlan> {
    let plan_file = root.join(".agents").join("plans").join("active_plan.json");
    let content = std::fs::read_to_string(plan_file).ok()?;
    serde_json::from_str(&content).ok()
}

fn parse_plan_step_update(raw: &str) -> (&'static str, String) {
    let trimmed = raw.trim();
    let lower = trimmed.to_ascii_lowercase();

    const PREFIXES: &[(&str, &str)] = &[
        ("[x]", "completed"),
        ("[✓]", "completed"),
        ("done:", "completed"),
        ("completed:", "completed"),
        ("[>]", "in_progress"),
        ("in_progress:", "in_progress"),
        ("in-progress:", "in_progress"),
        ("current:", "in_progress"),
        ("[!]", "failed"),
        ("[✗]", "failed"),
        ("failed:", "failed"),
        ("blocked:", "failed"),
        ("[ ]", "pending"),
        ("[]", "pending"),
        ("pending:", "pending"),
        ("todo:", "pending"),
    ];

    for &(prefix, status) in PREFIXES {
        if lower.starts_with(prefix) {
            let rest = trimmed[prefix.len()..]
                .trim_start_matches(|c: char| c == '-' || c == ':' || c.is_whitespace())
                .trim();
            return (
                status,
                if rest.is_empty() {
                    trimmed.to_string()
                } else {
                    rest.to_string()
                },
            );
        }
    }

    // Check trailing parenthesized status markers like "Step 1 (completed)"
    const SUFFIXES: &[(&str, &str)] = &[
        ("(completed)", "completed"),
        ("(done)", "completed"),
        ("(in_progress)", "in_progress"),
        ("(in-progress)", "in_progress"),
        ("(current)", "in_progress"),
        ("(failed)", "failed"),
        ("(blocked)", "failed"),
        ("(pending)", "pending"),
    ];
    for &(suffix, status) in SUFFIXES {
        if lower.ends_with(suffix) {
            let rest = trimmed[..trimmed.len() - suffix.len()].trim();
            return (
                status,
                if rest.is_empty() {
                    trimmed.to_string()
                } else {
                    rest.to_string()
                },
            );
        }
    }

    ("pending", trimmed.to_string())
}

pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    _config: &MintConfig,
    _chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress_cb: &mut (dyn FnMut(AgentProgress) + Send),
) -> Result<String, OrchestrationError> {
    match action {
        "create_plan" => {
            let objective = if !input.summary.trim().is_empty() {
                input.summary.trim().to_string()
            } else {
                "Implementation Plan".to_string()
            };
            let tasks: Vec<PlanTaskItem> = input
                .steps
                .iter()
                .enumerate()
                .map(|(idx, step)| PlanTaskItem {
                    id: format!("step-{}", idx + 1),
                    title: step.clone(),
                    status: if idx == 0 {
                        "in_progress".to_string()
                    } else {
                        "pending".to_string()
                    },
                })
                .collect();

            let plan = ActivePlan {
                objective: objective.clone(),
                tasks: tasks.clone(),
            };
            persist_plan_to_workspace(root, &plan);
            progress_cb(AgentProgress::PlanUpdated { plan });

            let mut formatted = format!("Plan Created: {}\n", objective);
            for task in &tasks {
                let mark = if task.status == "in_progress" {
                    "[>]"
                } else if task.status == "completed" {
                    "[✓]"
                } else {
                    "[ ]"
                };
                formatted.push_str(&format!("{} {} ({})\n", mark, task.title, task.status));
            }
            Ok(formatted)
        }
        "update_plan" => {
            let tasks: Vec<PlanTaskItem> = input
                .steps
                .iter()
                .enumerate()
                .map(|(idx, step)| {
                    let (status, title) = parse_plan_step_update(step);
                    PlanTaskItem {
                        id: format!("step-{}", idx + 1),
                        title,
                        status: status.to_string(),
                    }
                })
                .collect();

            let objective = if !input.summary.trim().is_empty() {
                input.summary.trim().to_string()
            } else if let Some(existing) = load_plan_from_workspace(root)
                .map(|p| p.objective)
                .filter(|obj| !obj.trim().is_empty())
            {
                existing
            } else {
                "Implementation Plan".to_string()
            };

            let plan = ActivePlan {
                objective,
                tasks: tasks.clone(),
            };
            persist_plan_to_workspace(root, &plan);
            progress_cb(AgentProgress::PlanUpdated { plan });

            let mut formatted = String::from("Plan Updated:\n");
            for task in &tasks {
                let mark = match task.status.as_str() {
                    "completed" => "[✓]",
                    "in_progress" => "[>]",
                    "failed" => "[✗]",
                    _ => "[ ]",
                };
                formatted.push_str(&format!("{} {} ({})\n", mark, task.title, task.status));
            }
            Ok(formatted)
        }
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
            let options: Vec<AskUserOption> = input
                .options
                .iter()
                .filter_map(|o| {
                    let (label, description) = match o {
                        AskUserOptionInput::Plain(s) => (s.trim().to_owned(), None),
                        AskUserOptionInput::Detailed { label, description } => (
                            label.trim().to_owned(),
                            description
                                .as_deref()
                                .map(str::trim)
                                .filter(|d| !d.is_empty())
                                .map(str::to_owned),
                        ),
                    };
                    if label.is_empty() {
                        None
                    } else {
                        Some(AskUserOption { label, description })
                    }
                })
                .take(3)
                .collect();
            let header = {
                let h = input.header.trim();
                if h.is_empty() {
                    None
                } else {
                    Some(h.to_owned())
                }
            };
            // Canonical multi-select answer format, matched identically by the
            // CLI picker (mint-cli/src/agent/approval_prompts.rs) and the
            // desktop/web ApprovalCard: selected labels joined with ", ",
            // with any additional free text appended as " — <free text>".
            let approved = approve_cb(&AgentApproval::AskUser {
                question: question.to_owned(),
                options,
                header,
                multi_select: input.multi_select,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_and_update_plan_persists_to_workspace() {
        let temp = std::env::temp_dir().join(format!("mint-plan-test-{}", uuid::Uuid::new_v4()));
        let _ = std::fs::remove_dir_all(&temp);
        std::fs::create_dir_all(&temp).unwrap();

        let input = AgentInput {
            summary: "Test Feature".to_string(),
            steps: vec!["Step 1".to_string(), "Step 2".to_string()],
            ..AgentInput::default()
        };
        let mut progress_events = Vec::new();
        let config = MintConfig::default();

        let res = execute(
            "create_plan",
            &input,
            &temp,
            &config,
            "test_chat",
            &mut |_| Ok(ApprovalOutcome::Approved),
            &mut |p| progress_events.push(p),
        )
        .await
        .unwrap();

        assert!(res.contains("Plan Created: Test Feature"));
        assert_eq!(progress_events.len(), 1);

        // Verify file persisted on disk
        let plan_file = temp.join(".agents").join("plans").join("active_plan.json");
        assert!(plan_file.exists());
        let content = std::fs::read_to_string(&plan_file).unwrap();
        let plan: ActivePlan = serde_json::from_str(&content).unwrap();
        assert_eq!(plan.objective, "Test Feature");
        assert_eq!(plan.tasks.len(), 2);
        assert_eq!(plan.tasks[0].status, "in_progress");

        // Now test update_plan without summary (matching tool_catalog schema) and with
        // colon-prefixed statuses ("done: ...", "in_progress: ...", "pending: ...")
        let update_input = AgentInput {
            summary: String::new(),
            steps: vec![
                "done: Step 1".to_string(),
                "in_progress: Inspect current state".to_string(),
                "[ ] Ensure streaming is done".to_string(),
                "failed: Broken step".to_string(),
            ],
            ..AgentInput::default()
        };
        let update_res = execute(
            "update_plan",
            &update_input,
            &temp,
            &config,
            "test_chat",
            &mut |_| Ok(ApprovalOutcome::Approved),
            &mut |p| progress_events.push(p),
        )
        .await
        .unwrap();

        assert!(update_res.contains("[✓] Step 1"));
        let updated_content = std::fs::read_to_string(&plan_file).unwrap();
        let updated_plan: ActivePlan = serde_json::from_str(&updated_content).unwrap();
        assert_eq!(updated_plan.objective, "Test Feature");
        assert_eq!(updated_plan.tasks[0].status, "completed");
        assert_eq!(updated_plan.tasks[0].title, "Step 1");
        assert_eq!(updated_plan.tasks[1].status, "in_progress");
        assert_eq!(updated_plan.tasks[1].title, "Inspect current state");
        assert_eq!(updated_plan.tasks[2].status, "pending");
        assert_eq!(updated_plan.tasks[2].title, "Ensure streaming is done");
        assert_eq!(updated_plan.tasks[3].status, "failed");
        assert_eq!(updated_plan.tasks[3].title, "Broken step");

        let _ = std::fs::remove_dir_all(&temp);
    }
}

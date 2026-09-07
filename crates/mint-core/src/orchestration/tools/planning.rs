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
                    let lower = step.to_ascii_lowercase();
                    let status = if lower.starts_with("[x]")
                        || lower.starts_with("[✓]")
                        || lower.contains("completed")
                        || lower.contains("done")
                    {
                        "completed".to_string()
                    } else if lower.starts_with("[>]")
                        || lower.contains("in_progress")
                        || lower.contains("current")
                    {
                        "in_progress".to_string()
                    } else {
                        "pending".to_string()
                    };
                    let title = step
                        .trim_start_matches(|c| c == '[' || c == ']' || c == 'x' || c == 'X' || c == '✓' || c == ' ' || c == '>')
                        .trim()
                        .to_string();
                    PlanTaskItem {
                        id: format!("step-{}", idx + 1),
                        title: if title.is_empty() { step.clone() } else { title },
                        status,
                    }
                })
                .collect();

            let objective = if !input.summary.trim().is_empty() {
                input.summary.trim().to_string()
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
                let mark = if task.status == "completed" {
                    "[✓]"
                } else if task.status == "in_progress" {
                    "[>]"
                } else {
                    "[ ]"
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

        // Now test update_plan
        let update_input = AgentInput {
            summary: "Test Feature".to_string(),
            steps: vec!["[x] Step 1".to_string(), "[>] Step 2".to_string()],
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
        assert_eq!(updated_plan.tasks[0].status, "completed");
        assert_eq!(updated_plan.tasks[1].status, "in_progress");

        let _ = std::fs::remove_dir_all(&temp);
    }
}


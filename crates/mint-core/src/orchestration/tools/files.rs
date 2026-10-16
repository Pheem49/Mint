use std::path::Path;

use super::super::*;

fn is_skill_edit(root: &Path, path: &Path) -> Result<bool, OrchestrationError> {
    crate::skills::skill_edit_requires_approval(root, path)
        .map_err(|error| OrchestrationError::Agent(error.to_string()))
}

/// Handles the subset of `execute_tool` actions related to files.
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
        "list_files" => {
            let path = agent_read_path(root, &input.path, config)?;
            let entries = list_directory_entries(&path, input.limit.unwrap_or(100), config)?;
            Ok(serde_json::to_string_pretty(&entries)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "read_file" => {
            if input.start_line.is_none()
                && input.end_line.is_none()
                && let Some(skill) = crate::skills::skill_for_read_path(root, &input.path)
            {
                let memory = MemoryStore::open_default()?;
                return crate::skills::read_skill_file(&memory, &skill, chat_id, config)
                    .map_err(|error| OrchestrationError::Agent(error.to_string()));
            }
            // Explicit line-range reads are previews, never successful full skill loads.
            // Known global skills may live outside the workspace, but still require
            // the same path capability checks as any other file read.
            let known_skill = crate::skills::skill_for_read_path(root, &input.path);
            let path = if let Some(skill) = known_skill {
                PathBuf::from(skill.source_path)
            } else {
                workspace_path(root, required(&input.path, "path")?)?
            };
            let start = input.start_line.unwrap_or(1);
            let end = input.end_line.unwrap_or_else(|| start.saturating_add(239));
            Ok(read_code_file(&path, start, end, config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "note_write" => {
            let file_name = if !input.note_path.is_empty() {
                input.note_path.as_str()
            } else {
                required(&input.path, "path")?
            };
            if file_name.contains("..") || file_name.contains('/') {
                return Err(OrchestrationError::Agent(
                    "note_write path must be a simple filename".into(),
                ));
            }
            let notes_dir = dirs::config_dir()
                .ok_or_else(|| {
                    OrchestrationError::Agent("cannot determine config directory".into())
                })?
                .join("mint")
                .join("notes");
            let note_path = notes_dir.join(file_name);

            let approved = approve_cb(&AgentApproval::NoteWrite {
                path: file_name.to_owned(),
                content: input.file_content.clone(),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => {
                    std::fs::create_dir_all(&notes_dir).map_err(|e| {
                        OrchestrationError::Agent(format!("cannot create notes directory: {}", e))
                    })?;
                    std::fs::write(&note_path, &input.file_content).map_err(|e| {
                        OrchestrationError::Agent(format!("cannot write note: {}", e))
                    })?;
                    Ok(format!("Note saved to {}", note_path.display()))
                }
                ApprovalOutcome::Denied => Ok(format!("User denied note write: {}", file_name)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "apply_patch" => {
            let patch = input.patch.as_ref().ok_or_else(|| {
                OrchestrationError::Agent("apply_patch requires patch input".into())
            })?;
            if patch.hunks.is_empty() {
                return Err(OrchestrationError::Agent(
                    "apply_patch requires at least one hunk".into(),
                ));
            }
            let edit = build_code_patch(root, patch.path.clone(), &patch.hunks, config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let diff = proposal
                .edits
                .iter()
                .map(|e| e.diff.clone())
                .collect::<Vec<_>>()
                .join("\n");

            let approval = if is_skill_edit(root, &proposal.edits[0].path)?
                || is_skill_edit(root, &root.join(&patch.path))?
            {
                AgentApproval::SkillWrite {
                    path: patch.path.to_string_lossy().into_owned(),
                    content: edit.content.clone(),
                    diff,
                }
            } else {
                AgentApproval::ApplyPatch {
                    path: patch.path.to_string_lossy().into_owned(),
                    hunks: patch.hunks.clone(),
                    diff,
                }
            };
            let approved = approve_cb(&approval).map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => {
                    let applied = apply_code_edits(root, &[edit], &proposal.approval_token, config)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(serde_json::to_string_pretty(&applied)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                }
                ApprovalOutcome::Denied => {
                    Ok(format!("User denied file edit: {}", edit.path.display()))
                }
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "write_file" => {
            let path_str = required(&input.path, "path")?;
            validate_new_workspace_file(root, config, Path::new(path_str))?;
            let edit = CodeEdit {
                path: PathBuf::from(path_str),
                content: input.file_content.clone(),
            };
            let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let diff = proposal
                .edits
                .iter()
                .map(|e| e.diff.clone())
                .collect::<Vec<_>>()
                .join("\n");

            let approval = if is_skill_edit(root, &proposal.edits[0].path)?
                || is_skill_edit(root, &root.join(path_str))?
            {
                AgentApproval::SkillWrite {
                    path: path_str.to_owned(),
                    content: edit.content.clone(),
                    diff,
                }
            } else {
                AgentApproval::WriteFile {
                    path: path_str.to_owned(),
                    content: input.file_content.clone(),
                    diff,
                }
            };
            let approved = approve_cb(&approval).map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => {
                    let applied = apply_code_edits(root, &[edit], &proposal.approval_token, config)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(serde_json::to_string_pretty(&applied)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                }
                ApprovalOutcome::Denied => Ok(format!("User denied file edit: {}", path_str)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        _ => unreachable!(
            "execute_tool routed an unhandled action into tools::files::execute: {action}"
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn skill_writes_require_separate_approval_even_when_file_edits_are_allowed() {
        let root =
            std::env::temp_dir().join(format!("mint-skill-approval-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&root).unwrap();
        let config = MintConfig {
            allowed_write_paths: vec![root.clone()],
            allowed_read_paths: vec![root.clone()],
            ..MintConfig::default()
        };
        let input = AgentInput {
            path: ".agents/skills/example/SKILL.md".into(),
            file_content: "---\ndescription: example\n---\nOriginal instructions".into(),
            ..Default::default()
        };
        let mut requests = Vec::new();
        let mut approve = |approval: &AgentApproval| {
            let value = serde_json::to_value(approval).unwrap();
            requests.push(value.clone());
            Ok(if value.get("SkillWrite").is_some() {
                ApprovalOutcome::Denied
            } else {
                ApprovalOutcome::Approved
            })
        };
        execute(
            "write_file",
            &input,
            &root,
            &config,
            "skill-test",
            &mut approve,
        )
        .await
        .unwrap();
        assert!(
            !root.join(&input.path).exists(),
            "ordinary file approval must not authorize skill creation"
        );
        assert!(
            requests[0]["SkillWrite"]["diff"]
                .as_str()
                .unwrap()
                .contains("Original instructions")
        );

        let path = root.join(&input.path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &input.file_content).unwrap();
        let patch = AgentInput {
            patch: Some(AgentPatch {
                path: PathBuf::from(&input.path),
                hunks: vec![CodePatchHunk {
                    old_text: "Original instructions".into(),
                    new_text: "Changed instructions".into(),
                    replace_all: false,
                }],
            }),
            ..Default::default()
        };
        execute(
            "apply_patch",
            &patch,
            &root,
            &config,
            "skill-test",
            &mut |approval| {
                Ok(
                    if serde_json::to_value(approval)
                        .unwrap()
                        .get("SkillWrite")
                        .is_some()
                    {
                        ApprovalOutcome::Denied
                    } else {
                        ApprovalOutcome::Approved
                    },
                )
            },
        )
        .await
        .unwrap();
        assert_eq!(std::fs::read_to_string(&path).unwrap(), input.file_content);
        execute(
            "apply_patch",
            &patch,
            &root,
            &config,
            "skill-test",
            &mut |_| Ok(ApprovalOutcome::Approved),
        )
        .await
        .unwrap();
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("Changed instructions")
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

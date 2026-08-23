use std::path::Path;

use super::super::*;

/// Handles the subset of `execute_tool` actions related to files.
/// Only called for actions `execute_tool` has already routed here, so the
/// fallback arm is unreachable in practice.
pub(in crate::orchestration) async fn execute(
    action: &str,
    input: &AgentInput,
    root: &Path,
    config: &MintConfig,
    _chat_id: &str,
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
            let path = workspace_path(root, required(&input.path, "path")?)?;
            Ok(read_code_file(
                &path,
                input.start_line.unwrap_or(1),
                input.end_line.unwrap_or(240),
                config,
            )
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

            let approved = approve_cb(&AgentApproval::ApplyPatch {
                path: patch.path.to_string_lossy().into_owned(),
                hunks: patch.hunks.clone(),
                diff,
            })
            .map_err(OrchestrationError::Agent)?;

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

            let approved = approve_cb(&AgentApproval::WriteFile {
                path: path_str.to_owned(),
                content: input.file_content.clone(),
                diff,
            })
            .map_err(OrchestrationError::Agent)?;

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

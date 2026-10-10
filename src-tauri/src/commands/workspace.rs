use crate::native::actions::ActionResult;
use mint_core::{
    AppliedCodeEdit, BranchInfo, ChatRequest, CodeEdit, CodeEditProposal, LinkedFolder,
    LinkedFolderDraft, apply_code_edits, load_config, propose_code_edits,
};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[tauri::command]
pub(crate) async fn get_workspace_snapshot(
    operation: mint_core::workspace::WorkspaceOperation,
) -> Result<mint_core::workspace::WorkspaceSnapshot, String> {
    tokio::task::spawn_blocking(move || mint_core::workspace::snapshot(&operation))
        .await
        .map_err(|error| format!("workspace tree task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn get_git_branch_info(workspace_path: String) -> Result<BranchInfo, String> {
    tokio::task::spawn_blocking(move || {
        let root = workspace_root(Some(&workspace_path))?;
        mint_core::read_branch_info(&root)
    })
    .await
    .map_err(|error| format!("git branch task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn switch_git_branch(
    workspace_path: String,
    branch: String,
    confirmed_dirty_workspace: bool,
) -> Result<mint_core::BranchChangeOutcome, String> {
    tokio::task::spawn_blocking(move || {
        let root = workspace_root(Some(&workspace_path))?;
        mint_core::change_branch(
            &root,
            &mint_core::BranchChange::Switch { branch },
            confirmed_dirty_workspace,
        )
    })
    .await
    .map_err(|error| format!("git switch task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn create_git_branch(
    workspace_path: String,
    branch: String,
    confirmed_dirty_workspace: bool,
) -> Result<mint_core::BranchChangeOutcome, String> {
    tokio::task::spawn_blocking(move || {
        let root = workspace_root(Some(&workspace_path))?;
        mint_core::change_branch(
            &root,
            &mint_core::BranchChange::Create { branch },
            confirmed_dirty_workspace,
        )
    })
    .await
    .map_err(|error| format!("create branch task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn checkout_remote_git_branch(
    workspace_path: String,
    remote_branch: String,
    confirmed_dirty_workspace: bool,
) -> Result<mint_core::BranchChangeOutcome, String> {
    tokio::task::spawn_blocking(move || {
        let root = workspace_root(Some(&workspace_path))?;
        mint_core::change_branch(
            &root,
            &mint_core::BranchChange::Track { remote_branch },
            confirmed_dirty_workspace,
        )
    })
    .await
    .map_err(|error| format!("remote branch task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn get_git_graph(workspace_path: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || {
        let root = workspace_root(Some(&workspace_path))?;
        mint_core::read_graph(&root, 120)
    })
    .await
    .map_err(|error| format!("git graph task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn get_workspace_git_diff(
    workspace_path: String,
) -> Result<Vec<mint_core::WorkspaceFileChange>, String> {
    tokio::task::spawn_blocking(move || {
        let root = workspace_root(Some(&workspace_path))?;
        mint_core::read_workspace_git_diff(&root)
    })
    .await
    .map_err(|error| format!("git diff task failed: {error}"))?
}

#[tauri::command]
pub(crate) async fn create_workspace_file(
    operation: mint_core::workspace::WorkspaceOperation,
) -> Result<mint_core::workspace::WorkspaceSnapshot, String> {
    mint_core::workspace::create_file(&operation)
}

#[tauri::command]
pub(crate) async fn create_workspace_folder(
    operation: mint_core::workspace::WorkspaceOperation,
) -> Result<mint_core::workspace::WorkspaceSnapshot, String> {
    mint_core::workspace::create_folder(&operation)
}

#[tauri::command]
pub(crate) async fn delete_workspace_item(
    operation: mint_core::workspace::WorkspaceOperation,
) -> Result<mint_core::workspace::WorkspaceSnapshot, String> {
    mint_core::workspace::delete(&operation)
}

#[tauri::command]
pub(crate) async fn move_workspace_item(
    operation: mint_core::workspace::WorkspaceOperation,
    destination: String,
) -> Result<mint_core::workspace::WorkspaceSnapshot, String> {
    mint_core::workspace::move_item(&operation, &destination)
}

#[tauri::command]
pub(crate) async fn list_workspace_history(
    root: String,
) -> Result<Vec<mint_core::system::workspace_history::HistoryEntry>, String> {
    mint_core::system::workspace_history::list(std::path::Path::new(&root))
}

#[tauri::command]
pub(crate) async fn undo_workspace_action(
    root: String,
    revision: u64,
    expected_id: Option<String>,
) -> Result<mint_core::workspace::WorkspaceSnapshot, String> {
    mint_core::workspace::undo_last(
        std::path::Path::new(&root),
        revision,
        expected_id.as_deref(),
    )
}

#[tauri::command]
pub(crate) async fn select_workspace_directory() -> Result<Option<String>, String> {
    tokio::task::spawn_blocking(select_workspace_directory_blocking)
        .await
        .map_err(|error| format!("workspace picker task failed: {error}"))?
}

fn select_workspace_directory_blocking() -> Result<Option<String>, String> {
    for (program, args) in [
        (
            "zenity",
            vec!["--file-selection", "--directory", "--title=Select Project"],
        ),
        ("kdialog", vec!["--getexistingdirectory", "."]),
    ] {
        let Ok(output) = Command::new(program)
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .output()
        else {
            continue;
        };
        if !output.status.success() {
            return Ok(None);
        }
        let selected = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if selected.is_empty() {
            return Ok(None);
        }
        return Ok(Some(workspace_root(Some(&selected))?.display().to_string()));
    }
    Ok(None)
}

pub(crate) fn workspace_root(path: Option<&str>) -> Result<PathBuf, String> {
    let root = match path.map(str::trim).filter(|path| !path.is_empty()) {
        Some(path) => PathBuf::from(path),
        None => std::env::current_dir().map_err(|error| error.to_string())?,
    };
    let root = root.canonicalize().map_err(|error| error.to_string())?;
    let root = if root.ends_with("src-tauri") {
        root.parent().unwrap_or(&root).to_path_buf()
    } else {
        root
    };
    if !root.is_dir() {
        return Err(format!("workspace is not a directory: {}", root.display()));
    }
    Ok(root)
}

pub(crate) fn background_owner(token: Option<String>) -> Result<Option<String>, String> {
    match token {
        Some(token) => mint_core::session_user_id(&token)
            .map(Some)
            .ok_or_else(|| "401: Session expired".into()),
        None => Ok(None),
    }
}

pub(crate) fn bind_background_owner(
    request: &ChatRequest,
    token: Option<String>,
) -> Result<(), String> {
    let owner = background_owner(token)?;
    let chat = request
        .chat_id
        .as_deref()
        .unwrap_or(mint_core::DEFAULT_CONVERSATION_ID);
    if mint_core::bg_shell::bind_chat_owner(chat, owner) {
        Ok(())
    } else {
        Err("403: Conversation belongs to another user".into())
    }
}

#[tauri::command]
pub(crate) async fn start_html_preview(
    root: String,
    relative_path: String,
    owner_token: Option<String>,
) -> Result<serde_json::Value, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    let owner = background_owner(owner_token)?;
    let preview = tokio::task::spawn_blocking(move || {
        mint_core::system::html_preview::start(
            std::path::Path::new(&root),
            &relative_path,
            owner,
            &config,
        )
    })
    .await
    .map_err(|e| e.to_string())??;
    let url = preview["url"].as_str().ok_or("Preview URL unavailable")?;
    reqwest::Client::new()
        .head(url)
        .timeout(std::time::Duration::from_secs(2))
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    Ok(preview)
}

#[tauri::command]
pub(crate) fn list_background_jobs(
    workspace: Option<String>,
    owner_token: Option<String>,
) -> Result<Vec<serde_json::Value>, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    let workspace = workspace
        .map(|path| {
            mint_core::assert_path_capability(
                std::path::Path::new(&path),
                mint_core::Capability::Read,
                &config,
            )
        })
        .transpose()
        .map_err(|e| e.to_string())?;
    let owner = background_owner(owner_token)?;
    Ok(mint_core::bg_shell::list_json(&owner, workspace.as_deref())
        .into_iter()
        .filter(|j| {
            j["workspacePath"].as_str().is_some_and(|p| {
                mint_core::assert_path_capability(
                    std::path::Path::new(p),
                    mint_core::Capability::Read,
                    &config,
                )
                .is_ok()
            })
        })
        .collect())
}

#[tauri::command]
pub(crate) fn get_background_job(
    id: String,
    owner_token: Option<String>,
) -> Result<serde_json::Value, String> {
    let owner = background_owner(owner_token)?;
    if !mint_core::bg_shell::accessible(&id, &owner) {
        return Err("Background job not found".into());
    }
    let job = mint_core::bg_shell::job_json(&id, true).map_err(|e| e.to_string())?;
    let config = load_config().map_err(|e| e.to_string())?;
    let path = job["workspacePath"].as_str().ok_or("Missing workspace")?;
    mint_core::assert_path_capability(
        std::path::Path::new(path),
        mint_core::Capability::Read,
        &config,
    )
    .map_err(|e| e.to_string())?;
    Ok(job)
}

#[tauri::command]
pub(crate) fn stop_background_job(
    id: String,
    owner_token: Option<String>,
) -> Result<serde_json::Value, String> {
    let job = get_background_job(id.clone(), owner_token)?;
    let config = load_config().map_err(|e| e.to_string())?;
    let path = job["workspacePath"].as_str().ok_or("Missing workspace")?;
    let capability = if job["managedService"] == true {
        mint_core::Capability::Read
    } else {
        mint_core::Capability::Write
    };
    mint_core::assert_path_capability(std::path::Path::new(path), capability, &config)
        .map_err(|e| e.to_string())?;
    mint_core::bg_shell::kill_job(&id).map_err(|e| e.to_string())?;
    mint_core::bg_shell::job_json(&id, false).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_background_job(id: String, owner_token: Option<String>) -> Result<(), String> {
    let job = get_background_job(id.clone(), owner_token)?;
    let config = load_config().map_err(|e| e.to_string())?;
    let path = job["workspacePath"].as_str().ok_or("Missing workspace")?;
    mint_core::assert_path_capability(
        std::path::Path::new(path),
        mint_core::Capability::Read,
        &config,
    )
    .map_err(|e| e.to_string())?;
    mint_core::bg_shell::remove_job(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_linked_folders()
-> Result<std::collections::BTreeMap<String, LinkedFolder>, String> {
    mint_core::list_linked_folders().map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn add_linked_folder(draft: LinkedFolderDraft) -> Result<(), String> {
    mint_core::add_linked_folder(&draft.name, &draft.path, draft.description)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_linked_folder(name: String) -> Result<bool, String> {
    mint_core::remove_linked_folder(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn linked_folder_status(name: String) -> Result<mint_core::LinkedFolderStatus, String> {
    mint_core::linked_folder_status(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn refresh_linked_folder(name: String) -> Result<mint_core::LinkedFolderStatus, String> {
    mint_core::refresh_linked_folder(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_linked_folder_notes(
    name: String,
) -> Result<Vec<mint_core::LinkedFolderNote>, String> {
    mint_core::list_linked_folder_notes(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn read_linked_folder_note(name: String, id: String) -> Result<String, String> {
    mint_core::read_linked_folder_note(&name, &id).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn save_linked_folder_note(
    name: String,
    content: String,
) -> Result<mint_core::LinkedFolderNote, String> {
    mint_core::save_linked_folder_note(&name, &content).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn open_linked_folder_note(name: String, id: String) -> Result<(), String> {
    let note = mint_core::list_linked_folder_notes(&name)
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|note| note.id == id && note.status == "saved")
        .ok_or_else(|| "note not found".to_string())?;
    mint_core::read_linked_folder_note(&name, &id).map_err(|e| e.to_string())?;
    #[cfg(target_os = "linux")]
    let mut command = Command::new("xdg-open");
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("rundll32");
        command.arg("url.dll,FileProtocolHandler");
        command
    };
    command.arg(note.path).spawn().map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub(crate) fn open_folder(path: String) -> Result<ActionResult, String> {
    let target_path = PathBuf::from(path.trim());
    if target_path.as_os_str().is_empty() {
        return Err("folder path is required".into());
    }

    let folder = if target_path.is_dir() {
        target_path
    } else {
        target_path
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| "could not resolve containing folder".to_string())?
    };

    Command::new("xdg-open")
        .arg(&folder)
        .spawn()
        .map_err(|error| error.to_string())?;
    Ok(ActionResult {
        success: true,
        message: format!("opened {}", folder.display()),
    })
}

#[tauri::command]
pub(crate) fn propose_desktop_code_edits(
    root: String,
    edits: Vec<CodeEdit>,
) -> Result<CodeEditProposal, String> {
    propose_code_edits(
        std::path::Path::new(&root),
        &edits,
        &load_config().map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn apply_desktop_code_edits(
    root: String,
    edits: Vec<CodeEdit>,
    approval_token: String,
) -> Result<Vec<AppliedCodeEdit>, String> {
    apply_code_edits(
        std::path::Path::new(&root),
        &edits,
        &approval_token,
        &load_config().map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn list_git_checkpoints(
    chat_id: String,
) -> Result<Vec<mint_core::git::Checkpoint>, String> {
    Ok(mint_core::git::list_checkpoints(&chat_id))
}

#[tauri::command]
pub(crate) fn rollback_git_checkpoint(
    chat_id: String,
    step: usize,
    workspace_path: Option<String>,
) -> Result<String, String> {
    let root = workspace_root(workspace_path.as_deref())?;
    mint_core::git::rollback_to_step(&root, &chat_id, step)
}

#[tauri::command]
pub(crate) fn undo_git_checkpoint(workspace_path: Option<String>) -> Result<String, String> {
    let root = workspace_root(workspace_path.as_deref())?;
    mint_core::git::undo_rollback(&root)
}

#[tauri::command]
pub(crate) fn read_workspace_file(
    path: String,
    workspace_path: Option<String>,
) -> Result<String, String> {
    let ws_path = workspace_path.as_deref().map(std::path::Path::new);
    let resolved = mint_core::files::resolve_readable_path(&path, ws_path);
    std::fs::read_to_string(&resolved)
        .map_err(|e| format!("failed to read file '{}': {e}", resolved.display()))
}

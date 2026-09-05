use std::path::{Path, PathBuf};
use std::process::Command;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Checkpoint {
    pub id: String,
    pub chat_id: String,
    pub step: usize,
    pub commit_hash: String,
    pub timestamp: u64,
    pub description: String,
    pub action: String,
    pub target_path: Option<String>,
}

fn checkpoints_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("mint").join("checkpoints"))
}

fn checkpoint_file_for_chat(chat_id: &str) -> Option<PathBuf> {
    let safe_id: String = chat_id
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();
    checkpoints_dir().map(|dir| dir.join(format!("{safe_id}.json")))
}

pub fn is_git_repo(root: &Path) -> bool {
    let output = Command::new("git")
        .args(["rev-parse", "--is-inside-work-tree"])
        .current_dir(root)
        .output();
    matches!(output, Ok(out) if out.status.success() && out.stdout.starts_with(b"true"))
}

pub fn get_head_hash(root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git: {e}"))?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Err(String::from_utf8_lossy(&output.stderr).trim().to_string())
    }
}

pub fn list_checkpoints(chat_id: &str) -> Vec<Checkpoint> {
    let Some(path) = checkpoint_file_for_chat(chat_id) else {
        return Vec::new();
    };
    if !path.exists() {
        return Vec::new();
    }
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

pub fn record_checkpoint(checkpoint: &Checkpoint) {
    let Some(path) = checkpoint_file_for_chat(&checkpoint.chat_id) else {
        return;
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut all = list_checkpoints(&checkpoint.chat_id);
    // Deduplicate by step
    all.retain(|c| c.step != checkpoint.step);
    all.push(checkpoint.clone());
    all.sort_by_key(|c| c.step);
    if let Ok(json) = serde_json::to_string_pretty(&all) {
        let _ = std::fs::write(&path, json);
    }
}

pub fn create_checkpoint(
    root: &Path,
    chat_id: &str,
    step: usize,
    action: &str,
    target_path: Option<&str>,
    description: &str,
) -> Result<Option<Checkpoint>, String> {
    if !is_git_repo(root) {
        return Ok(None);
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let stash_msg = format!("mint:checkpoint:{chat_id}:step:{step}:{timestamp}");
    let stash_out = Command::new("git")
        .args(["stash", "create", &stash_msg])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to run git stash create: {e}"))?;

    let stash_hash = String::from_utf8_lossy(&stash_out.stdout).trim().to_string();
    let commit_hash = if !stash_hash.is_empty() {
        stash_hash
    } else {
        get_head_hash(root).unwrap_or_default()
    };

    if commit_hash.is_empty() {
        return Ok(None);
    }

    let checkpoint_id = format!("chk_{step}_{timestamp}");

    // Persist a ref in refs/mint/checkpoints/<chat_id>/<id> to prevent git gc
    let ref_name = format!("refs/mint/checkpoints/{chat_id}/{checkpoint_id}");
    let _ = Command::new("git")
        .args(["update-ref", &ref_name, &commit_hash])
        .current_dir(root)
        .output();

    let checkpoint = Checkpoint {
        id: checkpoint_id,
        chat_id: chat_id.to_string(),
        step,
        commit_hash,
        timestamp,
        description: description.to_string(),
        action: action.to_string(),
        target_path: target_path.map(|s| s.to_string()),
    };

    record_checkpoint(&checkpoint);
    Ok(Some(checkpoint))
}

pub fn rollback_checkpoint(root: &Path, checkpoint: &Checkpoint) -> Result<String, String> {
    if !is_git_repo(root) {
        return Err("Workspace is not a git repository".into());
    }

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    // Create a rescue checkpoint first so user work is never lost
    let rescue_msg = format!("mint:rescue-before-rewind-to-step-{}", checkpoint.step);
    if let Ok(rescue_out) = Command::new("git")
        .args(["stash", "create", &rescue_msg])
        .current_dir(root)
        .output()
    {
        let rescue_hash = String::from_utf8_lossy(&rescue_out.stdout).trim().to_string();
        if !rescue_hash.is_empty() {
            let _ = Command::new("git")
                .args(["update-ref", &format!("refs/mint/rescue/{timestamp}"), &rescue_hash])
                .current_dir(root)
                .output();
        }
    }

    // Restore working tree to the checkpoint commit
    let checkout_out = Command::new("git")
        .args(["checkout", &checkpoint.commit_hash, "--", "."])
        .current_dir(root)
        .output()
        .map_err(|e| format!("failed to checkout checkpoint: {e}"))?;

    if !checkout_out.status.success() {
        return Err(format!(
            "Failed to restore workspace: {}",
            String::from_utf8_lossy(&checkout_out.stderr).trim()
        ));
    }

    Ok(format!(
        "Successfully restored workspace to Step {} ({})",
        checkpoint.step,
        &checkpoint.commit_hash[..7.min(checkpoint.commit_hash.len())]
    ))
}

pub fn rollback_to_step(root: &Path, chat_id: &str, step: usize) -> Result<String, String> {
    let checkpoints = list_checkpoints(chat_id);
    let target = checkpoints
        .iter()
        .find(|c| c.step == step)
        .ok_or_else(|| format!("No checkpoint found for step {step} in chat {chat_id}"))?;
    rollback_checkpoint(root, target)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_checkpoint_serialization() {
        let cp = Checkpoint {
            id: "chk_1_12345".into(),
            chat_id: "test-chat".into(),
            step: 1,
            commit_hash: "abcdef0123456789".into(),
            timestamp: 12345,
            description: "Edit main.rs".into(),
            action: "apply_patch".into(),
            target_path: Some("src/main.rs".into()),
        };
        let json = serde_json::to_string(&cp).unwrap();
        let deserialized: Checkpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(cp, deserialized);
    }
}

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use chrono::Local;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::safety::{Capability, SafetyError, assert_path_capability};
use crate::{ChatRequest, ConfigError, MintConfig, OrchestrationError, load_config, save_config};

#[derive(Debug, Error)]
pub enum LinkedFolderError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("invalid linked-folder configuration: {0}")]
    InvalidConfig(#[from] serde_json::Error),
    #[error("path does not exist or is not a directory: {0}")]
    NotADirectory(PathBuf),
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("no linked folder named {0:?}")]
    MissingFolder(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedFolder {
    pub name: String,
    pub path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Request body shape for creating a linked folder from the GUI/API layer
/// (Tauri command / `POST /api/linked-folders`).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkedFolderDraft {
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub description: Option<String>,
}

pub fn configured_linked_folders(
    config: &MintConfig,
) -> Result<BTreeMap<String, LinkedFolder>, LinkedFolderError> {
    Ok(config
        .extra
        .get("linkedFolders")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default())
}

pub fn list_linked_folders() -> Result<BTreeMap<String, LinkedFolder>, LinkedFolderError> {
    configured_linked_folders(&load_config()?)
}

pub fn add_linked_folder(
    name: &str,
    path: &Path,
    description: Option<String>,
) -> Result<(), LinkedFolderError> {
    if !path.is_dir() {
        return Err(LinkedFolderError::NotADirectory(path.to_path_buf()));
    }
    let mut config = load_config()?;
    let resolved = assert_path_capability(path, Capability::Write, &config)?;
    let mut folders = configured_linked_folders(&config)?;
    folders.insert(
        name.into(),
        LinkedFolder {
            name: name.into(),
            path: resolved,
            description,
        },
    );
    save_linked_folders(&mut config, folders)
}

pub fn remove_linked_folder(name: &str) -> Result<bool, LinkedFolderError> {
    let mut config = load_config()?;
    let mut folders = configured_linked_folders(&config)?;
    let removed = folders.remove(name).is_some();
    save_linked_folders(&mut config, folders)?;
    Ok(removed)
}

fn save_linked_folders(
    config: &mut MintConfig,
    folders: BTreeMap<String, LinkedFolder>,
) -> Result<(), LinkedFolderError> {
    config
        .extra
        .insert("linkedFolders".into(), serde_json::to_value(folders)?);
    Ok(save_config(config)?)
}

/// Cheap pre-filter before the (costlier) note-drafting reflection call: only
/// folders whose name or description appears (case-insensitively) in either
/// side of the turn are worth asking the LLM about. Pure/sync so it's easy to
/// unit test without a network call.
fn matching_candidates<'a>(
    folders: &'a BTreeMap<String, LinkedFolder>,
    user_text: &str,
    ai_text: &str,
) -> Vec<&'a LinkedFolder> {
    let haystack = format!("{user_text} {ai_text}").to_lowercase();
    folders
        .values()
        .filter(|folder| {
            haystack.contains(&folder.name.to_lowercase())
                || folder
                    .description
                    .as_deref()
                    .is_some_and(|description| {
                        description
                            .split(|c: char| !c.is_alphanumeric())
                            .filter(|word| word.len() > 3)
                            .any(|word| {
                                // Prefix match (not a full-word match) so simple
                                // plural/singular differences ("recipe" in the
                                // chat vs. "recipes" in the description) still
                                // count — this is a cheap pre-filter, not the
                                // final judgment call, so false positives here
                                // are fine (the LLM call after this decides for
                                // real); false negatives just skip that call.
                                // Built char-by-char (not byte-sliced) so this
                                // never panics on multi-byte UTF-8 (e.g. Thai).
                                let prefix: String = word.to_lowercase().chars().take(5).collect();
                                haystack.contains(&prefix)
                            })
                    })
        })
        .collect()
}

/// Fire-and-forget: after a chat turn, ask the model (in a second, separate
/// call) whether it touched on a linked folder's topic closely enough to be
/// worth a note, and if so append one to `<folder>/mint-notes/<date>.md`.
/// Mirrors [`crate::orchestration::spawn_auto_skill_write`] — never blocks or
/// fails the turn that triggered it.
pub fn spawn_linked_folder_note(config: MintConfig, user_text: String, ai_text: String) {
    tokio::spawn(async move {
        if let Err(e) = write_note_if_relevant(&config, &user_text, &ai_text).await {
            eprintln!("Linked-folder note write failed: {:?}", e);
        }
    });
}

async fn write_note_if_relevant(
    config: &MintConfig,
    user_text: &str,
    ai_text: &str,
) -> Result<(), OrchestrationError> {
    let folders = configured_linked_folders(config).unwrap_or_default();
    if folders.is_empty() {
        return Ok(());
    }
    let candidates = matching_candidates(&folders, user_text, ai_text);
    if candidates.is_empty() {
        return Ok(());
    }

    let candidate_list = candidates
        .iter()
        .map(|folder| {
            format!(
                "- {}: {}",
                folder.name,
                folder.description.as_deref().unwrap_or("(no description)")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let system_instruction = format!(
        r#"You are a background agent that decides whether a conversation turn is
worth saving as a short note into one of the user's linked folders below. Only
save if the turn genuinely discusses that folder's topic (a recommendation, a
fact, a decision, something worth remembering) — not for small talk or
tangential mentions.

Linked folders:
{candidate_list}

You must return strictly valid JSON with no other text, markers, or markdown,
and do NOT wrap it in ```json fences. Two shapes are allowed:

Not worth saving:
{{"should_save": false}}

Worth saving:
{{"should_save": true, "folder": "<one of the folder names above, exactly>", "content": "<concise markdown note body, a few lines>"}}"#
    );

    let message = format!("User: {}\nAssistant: {}", user_text, ai_text);

    let request = ChatRequest {
        message,
        system_instruction,
        chat_id: None,
        image_data_uri: None,
        audio_data_uri: None,
        video_data_uri: None,
        document_attachment: None,
        workspace_path: None,
        agent_id: None,
        plan_mode: false,
        messages: None,
        tools: None,
    };

    let response = crate::chat::send_chat(config, &request).await?;
    let text_reply = response.text.trim();

    let clean_json = if text_reply.starts_with("```") {
        let lines: Vec<&str> = text_reply.lines().collect();
        let mut filtered = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if !trimmed.starts_with("```") {
                filtered.push(trimmed);
            }
        }
        filtered.join("\n")
    } else {
        text_reply.to_string()
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(&clean_json) else {
        return Ok(());
    };
    let Some(obj) = value.as_object() else {
        return Ok(());
    };
    if !obj
        .get("should_save")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(());
    }
    let (Some(folder_name), Some(content)) = (
        obj.get("folder").and_then(|v| v.as_str()),
        obj.get("content").and_then(|v| v.as_str()),
    ) else {
        return Ok(());
    };
    let Some(folder) = folders.get(folder_name) else {
        return Ok(());
    };

    let notes_dir = folder.path.join("mint-notes");
    let now = Local::now();
    let note_path = notes_dir.join(format!("{}.md", now.format("%Y-%m-%d")));

    // Defense in depth: re-check in case blocked_paths changed since linking.
    assert_path_capability(&note_path, Capability::Write, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;

    fs::create_dir_all(&notes_dir)
        .map_err(|e| OrchestrationError::Agent(format!("unable to create {notes_dir:?}: {e}")))?;

    let mut existing = fs::read_to_string(&note_path).unwrap_or_default();
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    existing.push_str(&format!("\n## {}\n\n{}\n", now.format("%H:%M"), content));

    fs::write(&note_path, existing)
        .map_err(|e| OrchestrationError::Agent(format!("unable to write {note_path:?}: {e}")))?;

    crate::push_linked_folder_notice(format!(
        "Saved note to {} ({})",
        folder.name,
        note_path.display()
    ));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(name: &str, description: Option<&str>) -> LinkedFolder {
        LinkedFolder {
            name: name.to_string(),
            path: PathBuf::from("/tmp"),
            description: description.map(str::to_string),
        }
    }

    #[test]
    fn matches_by_folder_name_mentioned_in_either_side_of_the_turn() {
        let mut folders = BTreeMap::new();
        folders.insert("Food".to_string(), folder("Food", None));
        folders.insert("YouTube".to_string(), folder("YouTube", None));

        let hits = matching_candidates(&folders, "I tried a great new restaurant", "That sounds like a Food topic!");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "Food");
    }

    #[test]
    fn matches_by_description_keyword() {
        let mut folders = BTreeMap::new();
        folders.insert(
            "Food".to_string(),
            folder("Food", Some("restaurant reviews and recipes")),
        );

        let hits = matching_candidates(&folders, "found an amazing recipe for pasta", "");
        assert_eq!(hits.len(), 1);
    }

    #[test]
    fn no_candidates_when_nothing_matches() {
        let mut folders = BTreeMap::new();
        folders.insert("Food".to_string(), folder("Food", Some("restaurants")));

        let hits = matching_candidates(&folders, "how do I write a for loop in rust", "here's how");
        assert!(hits.is_empty());
    }

    #[test]
    fn no_candidates_when_no_folders_configured() {
        let folders: BTreeMap<String, LinkedFolder> = BTreeMap::new();
        let hits = matching_candidates(&folders, "anything at all", "");
        assert!(hits.is_empty());
    }
}

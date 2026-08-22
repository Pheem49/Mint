use std::collections::{BTreeMap, BTreeSet};
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
                || folder.description.as_deref().is_some_and(|description| {
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

const MAX_CROSS_REFERENCE_CANDIDATES: usize = 15;

/// One existing note entry offered to the note-writing LLM call as a
/// cross-reference candidate. `id` is `"YYYY-MM-DD#HH:MM"` — deliberately the
/// same shape as an Obsidian block reference, since these files already live
/// in a real `## HH:MM`-headed daily-note format a user could open in
/// Obsidian directly; `[[id]]` links written by [`format_note_content`]
/// resolve there too, not just inside Mint.
struct NoteEntryRef {
    id: String,
    preview: String,
}

/// Splits one day's note file (`<date>.md`) back into its individual
/// `## HH:MM` entries — the inverse of how [`write_note_if_relevant`] builds
/// that file up one `write` at a time. Every entry we've ever written starts
/// with a literal `"\n## "`, so splitting on that delimiter and dropping the
/// first piece (whatever came before the first heading — empty for a file
/// this function created) recovers each one directly.
fn parse_note_entries(content: &str, date: &str) -> Vec<NoteEntryRef> {
    content
        .split("\n## ")
        .skip(1)
        .filter_map(|chunk| {
            let mut lines = chunk.lines();
            let time = lines.next()?.trim();
            if time.is_empty() {
                return None;
            }
            let preview: String = lines
                .map(str::trim)
                .filter(|line| !line.is_empty())
                .collect::<Vec<_>>()
                .join(" ")
                .chars()
                .take(80)
                .collect();
            Some(NoteEntryRef {
                id: format!("{date}#{time}"),
                preview,
            })
        })
        .collect()
}

/// The most recent entries across every `<date>.md` file in `notes_dir`,
/// newest day first and newest-within-a-day first, capped at `limit` so the
/// note-writing prompt built in [`write_note_if_relevant`] stays bounded
/// even for a folder with a long note history. Returns an empty list (never
/// an error) for a folder with no notes yet — that's the common case for a
/// newly linked folder, not a failure.
fn list_recent_note_entries(notes_dir: &Path, limit: usize) -> Vec<NoteEntryRef> {
    let Ok(read_dir) = fs::read_dir(notes_dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = read_dir
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("md"))
        .collect();
    // Filenames are `YYYY-MM-DD.md`, so lexicographic order is chronological
    // order — reverse it to get newest-first.
    files.sort();
    files.reverse();

    let mut entries = Vec::new();
    for path in files {
        if entries.len() >= limit {
            break;
        }
        let Some(date) = path.file_stem().and_then(|stem| stem.to_str()) else {
            continue;
        };
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };
        let mut day_entries = parse_note_entries(&content, date);
        day_entries.reverse();
        entries.extend(day_entries);
    }
    entries.truncate(limit);
    entries
}

/// Appends a `Related:` line of `[[id]]` wiki-links to `content` for every
/// entry in `related` that's actually present in `known_ids`, silently
/// dropping anything else. The model was only ever shown `known_ids` as
/// candidates, so anything outside that set is either a hallucinated id or a
/// stale one from a race with another note write — either way, writing it in
/// would produce a permanently broken link with no way to detect it later.
fn format_note_content(content: &str, related: &[String], known_ids: &BTreeSet<String>) -> String {
    let valid: Vec<&str> = related
        .iter()
        .filter(|id| known_ids.contains(id.as_str()))
        .map(String::as_str)
        .collect();
    if valid.is_empty() {
        return content.to_string();
    }
    let links = valid
        .iter()
        .map(|id| format!("[[{id}]]"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("{content}\n\nRelated: {links}")
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

    // One pass per candidate folder: list its existing entries, then derive
    // both the prompt text and the known-id set (kept by folder name so the
    // `related` ids the model comes back with can be validated against
    // whichever folder it actually chose — see `format_note_content`'s doc
    // comment for why that validation matters) from the same listing.
    let mut known_ids: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let candidate_list = candidates
        .iter()
        .map(|folder| {
            let entries = list_recent_note_entries(
                &folder.path.join("mint-notes"),
                MAX_CROSS_REFERENCE_CANDIDATES,
            );
            let existing_notes = if entries.is_empty() {
                "  Existing notes: (none yet)".to_string()
            } else {
                let lines = entries
                    .iter()
                    .map(|entry| format!("  - {}: {}", entry.id, entry.preview))
                    .collect::<Vec<_>>()
                    .join("\n");
                format!("  Existing notes (id format \"YYYY-MM-DD#HH:MM\"):\n{lines}")
            };
            known_ids.insert(
                folder.name.clone(),
                entries.into_iter().map(|entry| entry.id).collect(),
            );
            format!(
                "- {}: {}\n{existing_notes}",
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

If the new note is meaningfully related to one or more existing notes listed
above for the folder you're saving into (same topic, a follow-up, a
correction, something a reader would want cross-linked), include their exact
ids in "related". Only use ids that appear in the list above — never invent
one. Leave "related" empty or omit it if nothing existing is relevant.

You must return strictly valid JSON with no other text, markers, or markdown,
and do NOT wrap it in ```json fences. Two shapes are allowed:

Not worth saving:
{{"should_save": false}}

Worth saving:
{{"should_save": true, "folder": "<one of the folder names above, exactly>", "content": "<concise markdown note body, a few lines>", "related": ["<id>", ...]}}"#
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
        pinned_mcp_server: None,
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
    let related: Vec<String> = obj
        .get("related")
        .and_then(|v| v.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|v| v.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    let empty_known_ids = BTreeSet::new();
    let content = format_note_content(
        content,
        &related,
        known_ids.get(folder_name).unwrap_or(&empty_known_ids),
    );

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

        let hits = matching_candidates(
            &folders,
            "I tried a great new restaurant",
            "That sounds like a Food topic!",
        );
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

    #[test]
    fn parse_note_entries_splits_multiple_headings_in_one_day_file() {
        let content = "\n## 09:15\n\nGrandma's pad thai uses tamarind, not ketchup.\n\n## 14:30\n\nTried the new ramen place downtown.\n";
        let entries = parse_note_entries(content, "2026-08-15");
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].id, "2026-08-15#09:15");
        assert!(entries[0].preview.contains("tamarind"));
        assert_eq!(entries[1].id, "2026-08-15#14:30");
        assert!(entries[1].preview.contains("ramen"));
    }

    #[test]
    fn parse_note_entries_returns_empty_for_content_with_no_headings() {
        assert!(parse_note_entries("", "2026-08-15").is_empty());
        assert!(parse_note_entries("just some raw text, no heading", "2026-08-15").is_empty());
    }

    #[test]
    fn parse_note_entries_truncates_long_bodies_to_an_80_char_preview() {
        let long_body = "x".repeat(500);
        let content = format!("\n## 09:15\n\n{long_body}\n");
        let entries = parse_note_entries(&content, "2026-08-15");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].preview.chars().count(), 80);
    }

    #[test]
    fn list_recent_note_entries_orders_newest_first_and_respects_limit() {
        let dir = std::env::temp_dir().join("mint-linked-folders-test-recent-entries");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();

        fs::write(
            dir.join("2026-08-10.md"),
            "\n## 09:00\n\nold day, one entry\n",
        )
        .unwrap();
        fs::write(
            dir.join("2026-08-15.md"),
            "\n## 09:15\n\nnew day, first entry\n\n## 14:30\n\nnew day, second entry\n",
        )
        .unwrap();
        // Not a note file — must be ignored.
        fs::write(dir.join("notes.txt"), "not markdown").unwrap();

        let entries = list_recent_note_entries(&dir, 10);
        assert_eq!(
            entries.iter().map(|e| e.id.as_str()).collect::<Vec<_>>(),
            vec!["2026-08-15#14:30", "2026-08-15#09:15", "2026-08-10#09:00"],
            "expected newest-day-first, newest-within-day-first ordering"
        );

        let limited = list_recent_note_entries(&dir, 2);
        assert_eq!(limited.len(), 2);
        assert_eq!(limited[0].id, "2026-08-15#14:30");
        assert_eq!(limited[1].id, "2026-08-15#09:15");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn list_recent_note_entries_is_empty_for_a_folder_with_no_notes_yet() {
        let dir = std::env::temp_dir().join("mint-linked-folders-test-no-notes-yet");
        let _ = fs::remove_dir_all(&dir);
        assert!(list_recent_note_entries(&dir, 10).is_empty());
    }

    #[test]
    fn format_note_content_appends_links_only_for_known_ids() {
        let known: BTreeSet<String> = ["2026-08-10#09:00", "2026-08-12#12:00"]
            .into_iter()
            .map(String::from)
            .collect();
        let related = vec![
            "2026-08-10#09:00".to_string(),
            "2026-08-99#99:99".to_string(), // hallucinated id — must be dropped
        ];
        let result = format_note_content("Tried a new place.", &related, &known);
        assert_eq!(
            result,
            "Tried a new place.\n\nRelated: [[2026-08-10#09:00]]"
        );
    }

    #[test]
    fn format_note_content_leaves_content_untouched_when_nothing_is_related() {
        let known: BTreeSet<String> = BTreeSet::new();
        assert_eq!(
            format_note_content("Tried a new place.", &[], &known),
            "Tried a new place."
        );
        assert_eq!(
            format_note_content("Tried a new place.", &["unknown#id".to_string()], &known),
            "Tried a new place."
        );
    }
}

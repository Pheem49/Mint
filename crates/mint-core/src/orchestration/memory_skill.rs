use std::path::{Path, PathBuf};

use super::*;

/// Appends saved cross-session memory — the user's profile/preferences (Settings
/// → Memory) and this chat's recent interaction history — onto `system_prompt`.
/// Shared by the typed-chat agent loop and the Gemini Live bridge so a Live
/// session starts with the same "who is this user, what have we already
/// discussed" context instead of starting blank every call.
pub(crate) fn append_memory_context(system_prompt: &mut String, chat_id: &str) {
    let Ok(memory) = MemoryStore::open_default() else {
        return;
    };

    let mut profile_instructions = String::new();
    if let Ok(Some(name)) = memory.get_profile("name")
        && !name.trim().is_empty()
    {
        profile_instructions.push_str(&format!("User Name: {}\n", name.trim()));
    }
    if let Ok(Some(preferences)) = memory.get_profile("preferences")
        && !preferences.trim().is_empty()
    {
        profile_instructions.push_str(&format!(
            "User Preferences & Profile:\n{}\n",
            preferences.trim()
        ));
    }
    if !profile_instructions.is_empty() {
        *system_prompt = format!(
            "{}\n\nUser Profile Information:\n{}",
            system_prompt.trim(),
            profile_instructions.trim()
        );
    }

    if let Ok(mut interactions) = memory.recent_interactions_for_chat(chat_id, CONTEXT_LIMIT) {
        interactions.reverse();
        let transcript = interactions
            .into_iter()
            .map(|item| {
                format!(
                    "User: {}\nAssistant: {}",
                    truncate_for_context(&item.user_text, MAX_CONTEXT_MESSAGE_CHARS),
                    truncate_for_context(&item.ai_text, MAX_CONTEXT_MESSAGE_CHARS)
                )
            })
            .collect::<Vec<_>>()
            .join("\n\n");
        if !transcript.is_empty() {
            *system_prompt = format!(
                "{}\n\nRecent conversation context:\n{}",
                system_prompt.trim(),
                transcript
            );
        }
    }
}

pub fn spawn_auto_memory_update(config: MintConfig, user_text: String, ai_text: String) {
    tokio::spawn(async move {
        if let Err(e) = auto_extract_and_update_memory(&config, &user_text, &ai_text).await {
            eprintln!("Auto memory update failed: {:?}", e);
        }
    });
}

/// Fire-and-forget: after a task finishes and passes [`looks_skill_worthy`], ask the
/// model (in a second, separate call) whether the task was a genuinely reusable
/// problem worth turning into a skill, and if so write
/// `<root>/.agents/skills/<slug>/SKILL.md`. Mirrors [`spawn_auto_memory_update`] —
/// never blocks or fails the already-returned [`AgentResult`].
pub fn spawn_auto_skill_write(
    config: MintConfig,
    task: String,
    summary: String,
    root: PathBuf,
    existing_skills: String,
) {
    tokio::spawn(async move {
        if let Err(e) = auto_write_skill(&config, &task, &summary, &root, &existing_skills).await {
            eprintln!("Auto skill write failed: {:?}", e);
        }
    });
}

pub(super) async fn auto_write_skill(
    config: &MintConfig,
    task: &str,
    summary: &str,
    root: &Path,
    existing_skills: &str,
) -> Result<(), OrchestrationError> {
    let system_instruction = r#"You are a background agent that decides whether a just-completed
coding/agent task is worth turning into a reusable skill for future sessions.

A task is skill-worthy only if it was non-trivial (took real investigation or multiple
steps to solve) AND the solution generalizes beyond this one-off instance (a pattern,
workaround, command sequence, or gotcha that will plausibly recur). Do NOT save trivial
tasks, one-off questions, or anything already covered by an existing skill listed below
(reuse that skill's slug to update it instead of creating a near-duplicate).

You must return strictly valid JSON with no other text, markers, or markdown, and do NOT
wrap it in ```json fences. Two shapes are allowed:

Not worth saving:
{"should_save": false}

Worth saving:
{
  "should_save": true,
  "slug": "kebab-case-name",
  "description": "one-line summary of when this skill applies",
  "content": "full SKILL.md body as markdown, starting with YAML frontmatter:\n---\ndescription: one-line summary\n---\nthen step-by-step reusable instructions"
}"#
        .to_string();

    let message = format!(
        "Existing skills already known (avoid duplicating these; reuse a slug below to update it instead):\n{}\n\nTask:\n{}\n\nOutcome:\n{}",
        existing_skills, task, summary
    );

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

    let response = send_chat(config, &request).await?;
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
    let (Some(slug), Some(content)) = (
        obj.get("slug").and_then(|v| v.as_str()),
        obj.get("content").and_then(|v| v.as_str()),
    ) else {
        return Ok(());
    };

    let slug = slugify(slug);
    if slug.is_empty() {
        return Ok(());
    }

    let skill_dir = root.join(".agents").join("skills").join(&slug);
    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| OrchestrationError::Agent(format!("unable to create {skill_dir:?}: {e}")))?;
    std::fs::write(skill_dir.join("SKILL.md"), content)
        .map_err(|e| OrchestrationError::Agent(format!("unable to write SKILL.md: {e}")))?;

    Ok(())
}

/// Lowercases, replaces runs of non-alphanumeric characters with a single `-`, and
/// trims leading/trailing `-` — turns an arbitrary model-provided name into a safe
/// directory name under `.agents/skills/`.
pub(super) fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

pub async fn auto_extract_and_update_memory(
    config: &MintConfig,
    user_text: &str,
    ai_text: &str,
) -> Result<(), OrchestrationError> {
    let memory = MemoryStore::open_default()?;

    // Retrieve current profile values
    let current_name = memory
        .get_profile("name")
        .unwrap_or(None)
        .unwrap_or_default();
    let current_pref = memory
        .get_profile("preferences")
        .unwrap_or(None)
        .unwrap_or_default();

    // System instruction for memory extraction
    let system_instruction = r#"You are a background agent responsible for updating a user's profile memory.
Analyze the latest conversation turn below.
Determine if the user shared their name, nickname, or any preferences, hobbies, or instructions on how they want the assistant to behave (e.g. language, formatting preference, details).
Update the existing Profile Name and Profile Preferences accordingly.
Keep existing preferences, add new ones, and resolve conflicts. Do not add metadata (like "preferred name") unless it is a generic preference. Keep formatting simple (e.g. list style or bullet points).
You must return the updated profile strictly as a valid JSON object with keys:
- "name": (string) updated name or same if not changed.
- "preferences": (string) updated preferences list or same if not changed.

Format the response strictly as valid JSON, with no other text, markers, or markdown.
Do NOT wrap the JSON in ```json ... ``` code blocks. Just output the raw JSON object.

Example response:
{
  "name": "Pheem",
  "preferences": "Always explain code step-by-step. Prefers TypeScript. Default language is Thai."
}"#.to_string();

    let message = format!(
        "Current Name: {}\nCurrent Preferences:\n{}\n\nLatest Turn:\nUser: {}\nAssistant: {}",
        current_name, current_pref, user_text, ai_text
    );

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

    // Send the chat request to LLM
    let response = send_chat(config, &request).await?;
    let text_reply = response.text.trim();

    // Attempt to parse the JSON response
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

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&clean_json)
        && let Some(obj) = value.as_object()
    {
        if let Some(new_name) = obj.get("name").and_then(|v| v.as_str()) {
            let trimmed_name = new_name.trim();
            if !trimmed_name.is_empty() && trimmed_name != current_name {
                memory.set_profile("name", trimmed_name)?;
            }
        }
        if let Some(new_pref) = obj.get("preferences").and_then(|v| v.as_str()) {
            let trimmed_pref = new_pref.trim();
            if !trimmed_pref.is_empty() && trimmed_pref != current_pref {
                memory.set_profile("preferences", trimmed_pref)?;
            }
        }
    }

    Ok(())
}

use std::path::{Path, PathBuf};

use super::*;

/// Hard ceiling on the stored `preferences` profile value. This text is
/// injected into the system prompt on every single turn (see
/// `append_memory_context`), so letting it grow without bound quietly
/// inflates the token cost and latency of every future conversation.
/// Enforced as a backstop even though the extraction prompt below also
/// asks the model to keep itself under this budget.
const MAX_PREFERENCES_CHARS: usize = 2500;

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

If this task matches an existing workspace skill (full current content shown below under
"Existing workspace skill contents"), your "content" must be a genuine refinement/merge of
that current version — incorporate whatever is new and useful from this task, correct
anything this task's outcome shows was wrong, and keep everything still correct and worth
keeping. Do not silently discard existing content by writing a from-scratch replacement
that happens to reuse the same slug.

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
        "Existing skills already known (avoid duplicating these; reuse a slug below to update it instead):\n{}\n\nExisting workspace skill contents (reuse one of these exact slugs, shown as \"--- slug ---\", to refine that skill instead of creating a near-duplicate):\n{}\n\nTask:\n{}\n\nOutcome:\n{}",
        existing_skills,
        existing_workspace_skill_bodies(root),
        task,
        summary
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
        pinned_mcp_server: None,
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
    let skill_path = skill_dir.join("SKILL.md");

    // Computed here rather than trusted to the model's own arithmetic in its
    // JSON response — a running counter is exactly the kind of thing an LLM
    // has no reliable way to get right call after call, but reading the
    // previous file's own `revisions:` line (0 if this slug is brand new)
    // and adding one is trivial and always correct.
    let previous_revision = std::fs::read_to_string(&skill_path)
        .ok()
        .map(|previous| skill_revision(&previous))
        .unwrap_or(0);
    let content = set_skill_revision(content, previous_revision + 1);

    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| OrchestrationError::Agent(format!("unable to create {skill_dir:?}: {e}")))?;
    std::fs::write(&skill_path, &content)
        .map_err(|e| OrchestrationError::Agent(format!("unable to write SKILL.md: {e}")))?;

    Ok(())
}

/// Full current `SKILL.md` content of every existing workspace skill under
/// `<root>/.agents/skills/`, formatted for [`auto_write_skill`]'s reflection
/// prompt so the model can produce a genuine refinement of one instead of
/// blindly overwriting it from scratch. Deliberately separate from (and
/// richer than) `skills::learned_skills_context`'s general-purpose listing,
/// which intentionally keeps workspace skills to a Path+Status pointer to
/// save context budget on every ordinary agent turn — this one-off
/// reflection call can afford the real content, since `looks_skill_worthy`
/// already filtered for a task substantive enough to be worth the extra
/// tokens.
pub(super) fn existing_workspace_skill_bodies(root: &Path) -> String {
    let skills_dir = root.join(".agents").join("skills");
    let Ok(entries) = std::fs::read_dir(&skills_dir) else {
        return "(none yet)".to_string();
    };
    let mut bodies: Vec<(String, String)> = entries
        .flatten()
        .filter(|entry| entry.path().is_dir())
        .filter_map(|entry| {
            let slug = entry.file_name().to_string_lossy().into_owned();
            let content = std::fs::read_to_string(entry.path().join("SKILL.md")).ok()?;
            Some((slug, content))
        })
        .collect();
    if bodies.is_empty() {
        return "(none yet)".to_string();
    }
    bodies.sort_by(|a, b| a.0.cmp(&b.0));
    bodies
        .into_iter()
        .map(|(slug, content)| format!("--- {slug} ---\n{}", content.trim()))
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Reads the `revisions: N` line from `content`'s YAML frontmatter (the
/// `---`-delimited block [`set_skill_revision`] maintains), defaulting to
/// `0` if there's no frontmatter or no such line yet. Mirrors
/// `skills::parse_skill_description`'s frontmatter-scanning style.
pub(super) fn skill_revision(content: &str) -> u32 {
    let trimmed = content.trim_start();
    let Some(rest) = trimmed.strip_prefix("---") else {
        return 0;
    };
    let Some(end_idx) = rest.find("---") else {
        return 0;
    };
    rest[..end_idx]
        .lines()
        .find_map(|line| line.trim().strip_prefix("revisions:")?.trim().parse().ok())
        .unwrap_or(0)
}

/// Sets `content`'s frontmatter `revisions:` line to `revision`, replacing
/// an existing one or adding a frontmatter block if `content` doesn't have
/// one (defensive — every skill this module writes always starts with one
/// per its own system prompt above, but a hand-edited `SKILL.md` might
/// not).
pub(super) fn set_skill_revision(content: &str, revision: u32) -> String {
    let trimmed = content.trim_start();
    if let Some(rest) = trimmed.strip_prefix("---")
        && let Some(end_idx) = rest.find("---")
    {
        let frontmatter = &rest[..end_idx];
        let after_closing_marker = &rest[end_idx + 3..];
        let mut lines: Vec<&str> = frontmatter
            .lines()
            .filter(|line| !line.trim_start().starts_with("revisions:"))
            .collect();
        let revision_line = format!("revisions: {revision}");
        lines.push(&revision_line);
        return format!("---{}\n---{}", lines.join("\n"), after_closing_marker);
    }
    format!("---\nrevisions: {revision}\n---\n\n{trimmed}")
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
    let system_instruction = format!(
        r#"You are a background agent responsible for maintaining a user's profile memory.
Analyze the latest conversation turn below.
Determine if the user shared their name, nickname, or any preferences, hobbies, or instructions on how they want the assistant to behave (e.g. language, formatting preference, details).
Update the existing Profile Name and Profile Preferences accordingly.

The preferences list must stay a tight, deduplicated summary, not an ever-growing log:
- Do not just append the new fact to the end. Rewrite/reorganize the whole list each time.
- Merge similar or related points into a single bullet instead of listing near-duplicates separately.
- Drop preferences that are outdated, contradicted, or superseded by a newer statement in this turn.
- Keep the total under about {MAX_PREFERENCES_CHARS} characters. If you're close to the limit, tighten the phrasing of older or less important groups rather than dropping the newest information.

Do not add metadata (like "preferred name") unless it is a generic preference. Keep formatting simple (e.g. list style or bullet points).
You must return the updated profile strictly as a valid JSON object with keys:
- "name": (string) updated name or same if not changed.
- "preferences": (string) updated, consolidated preferences list or same if not changed.

Format the response strictly as valid JSON, with no other text, markers, or markdown.
Do NOT wrap the JSON in ```json ... ``` code blocks. Just output the raw JSON object.

Example response:
{{
  "name": "Pheem",
  "preferences": "Always explain code step-by-step. Prefers TypeScript. Default language is Thai."
}}"#
    );

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
        pinned_mcp_server: None,
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
                let capped_pref = if trimmed_pref.chars().count() > MAX_PREFERENCES_CHARS {
                    truncate_for_context(trimmed_pref, MAX_PREFERENCES_CHARS)
                } else {
                    trimmed_pref.to_string()
                };
                memory.set_profile("preferences", &capped_pref)?;
            }
        }
    }

    Ok(())
}

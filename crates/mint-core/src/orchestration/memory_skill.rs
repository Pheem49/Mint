use std::path::{Path, PathBuf};

use super::*;

/// Hard ceiling on the stored `preferences` profile value. This text is
/// injected into the system prompt on every single turn (see
/// `append_memory_context`), so letting it grow without bound quietly
/// inflates the token cost and latency of every future conversation.
/// Enforced as a backstop even though the extraction prompt below also
/// asks the model to keep itself under this budget.
const MAX_PREFERENCES_CHARS: usize = 2500;

/// Combined budget for the `Remembered facts:` block. Like `MAX_PREFERENCES_CHARS`
/// this rides on every turn, so overflow is dropped oldest-first rather than
/// allowed to crowd out the actual task.
const MAX_FACTS_CHARS: usize = 1500;

/// How many of the newest facts are always kept when the list overflows the
/// budget, before relevance ranking fills the rest — so a fact the user stated
/// moments ago can never be bumped out by an older, more on-topic one.
const FACT_RECENCY_KEEP: usize = 5;

/// Renders the live [`Fact`] rows visible to `agent_id` (shared facts, plus that
/// subagent's own — see [`MemoryStore::live_facts_for_agent`]) and relevant to
/// `workspace`, as a bulleted block truncated to [`MAX_FACTS_CHARS`]. Returns
/// `None` when there is nothing stored.
///
/// Below the budget the output is every fact, newest first (unchanged). When it
/// overflows and `semantic` is set with a non-empty `query`, the overflow slot
/// is filled by the facts most similar to `query` rather than simply the next
/// newest ones. Shared by [`append_memory_context`] and `enrich_request`.
pub(crate) fn render_memory_facts(
    memory: &MemoryStore,
    workspace: Option<&str>,
    agent_id: Option<&str>,
    query: Option<&str>,
    semantic: bool,
) -> Option<String> {
    let facts = memory.live_facts_for_agent(workspace, agent_id).ok()?;
    if facts.is_empty() {
        return None;
    }

    let render_line = |fact: &crate::Fact| {
        let tag = if fact.scope == "project" {
            " (this project)"
        } else {
            ""
        };
        format!("- {}{}\n", fact.body.trim(), tag)
    };

    // Fast path: everything fits, emit as-is.
    let total: usize = facts.iter().map(|f| render_line(f).chars().count()).sum();
    if total <= MAX_FACTS_CHARS {
        let out = facts.iter().map(render_line).collect::<String>();
        let out = out.trim_end();
        return (!out.is_empty()).then(|| out.to_string());
    }

    // Overflow: keep the newest few, then fill the remaining budget — by
    // relevance to `query` when asked, otherwise by recency (the old behavior).
    let keep = FACT_RECENCY_KEEP.min(facts.len());
    let mut chosen: Vec<&crate::Fact> = facts[..keep].iter().collect();
    let mut used: usize = chosen.iter().map(|f| render_line(f).chars().count()).sum();

    let query_tokens = query.map(str::trim).filter(|q| !q.is_empty());
    let mut rest: Vec<&crate::Fact> = facts[keep..].iter().collect();
    if semantic
        && let Some(q) = query_tokens
        && let Ok(scored) = memory.live_facts_with_embedding(workspace, agent_id)
    {
        let qv = crate::search::text_embedding::fact_embedding_backend().embed(q);
        let score_of = |id: i64| {
            scored
                .iter()
                .find(|(f, _)| f.id == id)
                .map(|(_, v)| crate::search::text_embedding::cosine_similarity(&qv, v))
                .unwrap_or(f32::MIN)
        };
        rest.sort_by(|a, b| {
            score_of(b.id)
                .partial_cmp(&score_of(a.id))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    for fact in rest {
        let line_len = render_line(fact).chars().count();
        if used + line_len > MAX_FACTS_CHARS {
            continue;
        }
        used += line_len;
        chosen.push(fact);
    }

    let out = chosen.iter().map(|f| render_line(f)).collect::<String>();
    let out = out.trim_end();
    (!out.is_empty()).then(|| out.to_string())
}

/// Combined budget for the recall block, matching [`MAX_FACTS_CHARS`]'s intent.
const MAX_RECALL_CHARS: usize = 1500;

/// Full-text-searches `chat_id`'s history for turns relevant to `query` and
/// formats the top few as a labelled block, oldest first. The `CONTEXT_LIMIT`
/// newest turns are skipped — they're already injected verbatim as "Recent
/// conversation context". Returns `None` when nothing relevant turns up (or the
/// query has no searchable token).
pub(crate) fn render_recalled_messages(chat_id: &str, query: &str) -> Option<String> {
    let memory = MemoryStore::open_default().ok()?;
    let hits = memory
        .recall_interactions(chat_id, query, CONTEXT_LIMIT, 5)
        .ok()?;
    if hits.is_empty() {
        return None;
    }
    let mut out = String::new();
    for item in hits.iter().rev() {
        let block = format!(
            "User: {}\nAssistant: {}\n\n",
            truncate_for_context(&item.user_text, MAX_CONTEXT_MESSAGE_CHARS),
            truncate_for_context(&item.ai_text, MAX_CONTEXT_MESSAGE_CHARS)
        );
        if out.chars().count() + block.chars().count() > MAX_RECALL_CHARS {
            break;
        }
        out.push_str(&block);
    }
    let out = out.trim_end();
    (!out.is_empty()).then(|| {
        format!("Possibly relevant earlier messages (from this conversation's history):\n{out}")
    })
}

/// Appends saved cross-session memory — the user's profile/preferences (Settings
/// → Memory) and this chat's recent interaction history — onto `system_prompt`.
/// Shared by the typed-chat agent loop and the Gemini Live bridge so a Live
/// session starts with the same "who is this user, what have we already
/// discussed" context instead of starting blank every call.
pub(crate) fn append_memory_context(
    system_prompt: &mut String,
    chat_id: &str,
    workspace: Option<&str>,
    query: Option<&str>,
    semantic_fact_recall: bool,
) {
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

    if let Some(facts) = render_memory_facts(
        &memory,
        workspace,
        crate::subagent_name(chat_id),
        query,
        semantic_fact_recall,
    ) {
        *system_prompt = format!("{}\n\nRemembered facts:\n{}", system_prompt.trim(), facts);
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

pub fn spawn_auto_memory_update(
    config: MintConfig,
    user_text: String,
    ai_text: String,
    workspace: Option<String>,
    chat_id: String,
) {
    tokio::spawn(async move {
        if let Err(e) = auto_extract_and_update_memory(&config, &user_text, &ai_text).await {
            eprintln!("Auto memory update failed: {:?}", e);
        }
        // Second, independent background pass: pull any durable fact/preference/
        // decision the user just stated into the structured `facts` table so it
        // rides every future turn via `render_memory_facts`. Gated by both a
        // config toggle and a cheap keyword pre-filter so it doesn't add an LLM
        // call to every single turn — only ones that look like they carry
        // something worth remembering. See `looks_fact_worthy`.
        if !config.auto_fact_extraction || !looks_fact_worthy(&user_text) {
            return;
        }
        if let Err(e) = auto_extract_facts(
            &config,
            &user_text,
            &ai_text,
            workspace.as_deref(),
            &chat_id,
        )
        .await
        {
            eprintln!("Auto fact extraction failed: {:?}", e);
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

/// Ceiling on how many fact ops a single turn's extraction is allowed to apply.
/// A background LLM pass occasionally over-reads a chatty turn as five separate
/// "facts"; capping keeps one turn from flooding the always-injected
/// `Remembered facts:` block.
const MAX_FACT_OPS_PER_TURN: usize = 3;

/// Once the live-fact count reaches this, the extraction prompt switches to
/// asking the model to prefer merging/superseding over adding — the read side
/// (`render_memory_facts`) already hard-truncates at `MAX_FACTS_CHARS`, so past
/// this point new adds mostly just push older facts out of the injected window.
const FACTS_CONSOLIDATE_THRESHOLD: usize = 40;

/// Cosine (over the on-device hash embedding) at or above which a fact the main
/// agent is about to add is treated as "the same thing" a quarantined subagent
/// fact already says — the subagent's fact is promoted to shared instead of a
/// near-duplicate being inserted. Hash embedding is bag-of-token, so this is a
/// deliberately high bar that really only fires on heavy lexical overlap.
const FACT_PROMOTE_SIMILARITY: f32 = 0.82;

/// Cheap, no-LLM pre-filter for [`auto_extract_facts`]: only turns whose user
/// message carries a durable-information signal (a stated preference, a naming,
/// a standing instruction, a project convention) are worth spending a second
/// background model call on. Mirrors [`super::looks_skill_worthy`]'s role as a
/// gate in front of a costlier reflection call. Matches English and Thai
/// markers, case-insensitively, and skips very short messages outright.
pub(super) fn looks_fact_worthy(user_text: &str) -> bool {
    let trimmed = user_text.trim();
    if trimmed.chars().count() < 15 {
        return false;
    }
    let lowered = trimmed.to_lowercase();
    const MARKERS: &[&str] = &[
        // English
        "prefer",
        "always",
        "never",
        "i use",
        "we use",
        "my ",
        "i'm ",
        "i am ",
        "call me",
        "remember",
        "from now on",
        "i like",
        "i hate",
        "i don't",
        "i do not",
        "the project uses",
        "this project uses",
        "make sure to",
        "don't ever",
        // Thai
        "ชอบ",
        "ไม่ชอบ",
        "ของฉัน",
        "เรียกฉันว่า",
        "จำไว้",
        "ต่อจากนี้",
        "ตั้งแต่นี้",
        "อย่า",
        "ทุกครั้ง",
        "เสมอ",
    ];
    MARKERS.iter().any(|marker| lowered.contains(marker))
}

/// Applies the `{"ops":[...]}` document produced by [`auto_extract_facts`] to
/// `store`. Pure and side-effect-contained (takes an explicit `store` so it is
/// unit-testable against a temp database) — never panics, silently drops any
/// malformed or disallowed op, and returns how many were actually applied.
///
/// `known_ids` is the set of fact ids that were shown to the model; a
/// `supersede` naming anything outside it is rejected as a hallucinated id.
///
/// `source_chat_id` decides ownership: a `"…::subagent::<name>"` id quarantines
/// every added fact to `<name>` ([`Fact::agent_id`]); an ordinary id adds shared
/// facts and, before each `add`, may instead *promote* a near-identical
/// quarantined fact (see [`FACT_PROMOTE_SIMILARITY`]).
pub(super) fn apply_fact_ops(
    store: &MemoryStore,
    ops_json: &str,
    workspace: Option<&str>,
    source_chat_id: &str,
    known_ids: &std::collections::HashSet<i64>,
) -> usize {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(ops_json) else {
        return 0;
    };
    let Some(ops) = value.get("ops").and_then(|v| v.as_array()) else {
        return 0;
    };

    let agent_id = crate::subagent_name(source_chat_id);

    let mut applied = 0usize;
    for op in ops.iter().take(MAX_FACT_OPS_PER_TURN) {
        let kind = op.get("op").and_then(|v| v.as_str()).unwrap_or_default();
        match kind {
            "add" => {
                let body = op
                    .get("body")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .trim();
                if body.is_empty() {
                    continue;
                }
                let scope = op.get("scope").and_then(|v| v.as_str()).unwrap_or("user");
                if !matches!(scope, "user" | "preference" | "project") {
                    continue;
                }
                // A `project` fact is meaningless without a workspace to pin it
                // to — skip rather than silently reclassify it as global.
                let project_path = if scope == "project" {
                    match workspace {
                        Some(path) if !path.trim().is_empty() => Some(path),
                        _ => continue,
                    }
                } else {
                    None
                };

                // Main agent restating something a subagent already found:
                // promote the subagent's fact (keeps its provenance) instead of
                // inserting a near-duplicate into the shared pool.
                if agent_id.is_none()
                    && let Some(promoted_id) = best_promotion_candidate(store, workspace, body)
                    && store.promote_fact(promoted_id).unwrap_or(false)
                {
                    applied += 1;
                    continue;
                }

                if let Ok(Some(_)) = store.add_fact_for_agent(
                    scope,
                    project_path,
                    body,
                    Some(source_chat_id),
                    None,
                    agent_id,
                ) {
                    applied += 1;
                }
            }
            "supersede" => {
                let Some(old_id) = op.get("id").and_then(|v| v.as_i64()) else {
                    continue;
                };
                if !known_ids.contains(&old_id) {
                    continue;
                }
                let replacement = op
                    .get("replacement")
                    .and_then(|v| v.as_str())
                    .map(str::trim)
                    .filter(|s| !s.is_empty());
                // Re-file the replacement under the same scope/workspace/owner as
                // the fact it supersedes, so an updated project (or subagent)
                // fact keeps that classification.
                let (old_scope, old_project, old_agent) = store
                    .live_facts_for_agent(workspace, agent_id)
                    .ok()
                    .and_then(|facts| facts.into_iter().find(|f| f.id == old_id))
                    .map(|f| (f.scope, f.project_path, f.agent_id))
                    .unwrap_or_else(|| ("user".to_string(), None, agent_id.map(str::to_owned)));
                let new_id = match replacement {
                    Some(text) => store
                        .add_fact_for_agent(
                            &old_scope,
                            old_project.as_deref(),
                            text,
                            Some(source_chat_id),
                            None,
                            old_agent.as_deref(),
                        )
                        .ok()
                        .flatten(),
                    None => None,
                };
                if store.supersede_fact(old_id, new_id).is_ok() {
                    applied += 1;
                }
            }
            _ => {}
        }
    }
    applied
}

/// Id of the live quarantined fact whose stored embedding is closest to `body`,
/// if that similarity clears [`FACT_PROMOTE_SIMILARITY`]. Used by
/// [`apply_fact_ops`] to fold a main-agent restatement into an existing
/// subagent fact.
fn best_promotion_candidate(
    store: &MemoryStore,
    workspace: Option<&str>,
    body: &str,
) -> Option<i64> {
    let candidates = store.quarantined_facts_with_embedding(workspace).ok()?;
    let bv = crate::search::text_embedding::fact_embedding_backend().embed(body);
    candidates
        .iter()
        .map(|(fact, vec)| {
            (
                fact.id,
                crate::search::text_embedding::cosine_similarity(&bv, vec),
            )
        })
        .filter(|(_, score)| *score >= FACT_PROMOTE_SIMILARITY)
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(id, _)| id)
}

/// Fire-and-forget: after a turn that [`looks_fact_worthy`], ask the model (in a
/// second, separate call) whether the user stated anything durable enough to
/// keep in long-term memory, and if so apply the resulting add/supersede ops to
/// the `facts` table. Mirrors [`auto_write_skill`] / [`spawn_auto_memory_update`]
/// — never blocks or fails the already-returned response.
pub(super) async fn auto_extract_facts(
    config: &MintConfig,
    user_text: &str,
    ai_text: &str,
    workspace: Option<&str>,
    chat_id: &str,
) -> Result<(), OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    // Scope the "already stored" list to what this agent can actually see, so a
    // subagent's extraction reasons over (and supersedes) its own quarantined
    // facts plus the shared pool — not another subagent's.
    let existing = memory
        .live_facts_for_agent(workspace, crate::subagent_name(chat_id))
        .unwrap_or_default();
    let known_ids: std::collections::HashSet<i64> = existing.iter().map(|f| f.id).collect();

    let existing_block = if existing.is_empty() {
        "(none yet)".to_string()
    } else {
        existing
            .iter()
            .map(|f| {
                let scope = if f.scope == "project" {
                    "project"
                } else {
                    "user"
                };
                format!("- [id {}] ({}) {}", f.id, scope, f.body.trim())
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    let consolidate_hint = if existing.len() >= FACTS_CONSOLIDATE_THRESHOLD {
        "\n\nThe stored list is already long: strongly prefer `supersede` (merging or updating an \
         existing fact) over adding new ones, and only add something genuinely not covered above."
    } else {
        ""
    };

    let workspace_line = match workspace {
        Some(path) if !path.trim().is_empty() => {
            format!("Current project workspace: {path}\n")
        }
        _ => "There is no active project workspace, so do NOT emit any \"project\"-scope ops.\n"
            .to_string(),
    };

    let system_instruction = format!(
        r#"You are a background agent that maintains a user's long-term memory of durable facts.
Look at the latest conversation turn and decide whether the USER stated anything worth
remembering permanently: a personal detail, a standing preference, a naming ("call me X"),
a lasting instruction on how the assistant should behave, or a stable convention of the
current project.

Do NOT save:
- Task-specific or one-off details (a filename in this task, a value being debugged).
- Anything already covered by the stored facts below — unless the turn changes or contradicts it,
  in which case emit a "supersede" op for that fact's exact id.
- Vague or speculative statements. Only save something you would be confident restating later.

Scope rules:
- "user": something about the person or how they always want the assistant to behave (global).
- "project": a stable fact about the current project/codebase. Only valid when a workspace is given.
{workspace_line}
Stored facts (use the exact id for "supersede"; never invent an id):
{existing_block}{consolidate_hint}

Return STRICTLY valid JSON, no prose, no markdown, no ```json fences. Shape:
{{"ops": [
  {{"op": "add", "scope": "user", "body": "one concise sentence"}},
  {{"op": "supersede", "id": 12, "replacement": "one concise sentence"}}
]}}
"replacement" may be omitted for a pure retraction. Emit {{"ops": []}} when nothing qualifies.
Emit at most {MAX_FACT_OPS_PER_TURN} ops."#
    );

    let message = format!("User: {user_text}\nAssistant: {ai_text}");

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

    apply_fact_ops(&memory, &clean_json, workspace, chat_id, &known_ids);
    Ok(())
}

#[cfg(test)]
mod fact_extraction_tests {
    use super::*;
    use std::collections::HashSet;

    fn temp_store(name: &str) -> MemoryStore {
        let path = std::env::temp_dir().join(format!(
            "mint-fact-ops-{name}-{}.sqlite",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&path);
        MemoryStore::open(path)
    }

    fn ids_of(store: &MemoryStore) -> HashSet<i64> {
        store
            .live_facts(None)
            .unwrap()
            .into_iter()
            .map(|f| f.id)
            .collect()
    }

    #[test]
    fn looks_fact_worthy_gates_on_signal_and_length() {
        assert!(looks_fact_worthy(
            "I prefer tabs over spaces in every language"
        ));
        assert!(looks_fact_worthy("From now on, answer me in Thai please"));
        assert!(looks_fact_worthy("เรียกฉันว่าฟีมนะ ต่อจากนี้ตอบสั้น ๆ พอ"));
        assert!(!looks_fact_worthy("fix this failing test for me"));
        assert!(!looks_fact_worthy("run the build"));
        assert!(!looks_fact_worthy("prefer")); // too short even with a marker
    }

    #[test]
    fn apply_adds_user_facts_and_dedups_exact_bodies() {
        let store = temp_store("add");
        let json = r#"{"ops":[{"op":"add","scope":"user","body":"prefers Helix editor"}]}"#;
        assert_eq!(
            apply_fact_ops(&store, json, None, "cli", &HashSet::new()),
            1
        );
        // Re-applying the identical body is a silent no-op (dedup unique index).
        assert_eq!(
            apply_fact_ops(&store, json, None, "cli", &HashSet::new()),
            0
        );
        let live = store.live_facts(None).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].body, "prefers Helix editor");
    }

    #[test]
    fn supersede_requires_a_known_id() {
        let store = temp_store("supersede");
        store
            .add_fact("user", None, "deploys on Fridays", None, None)
            .unwrap();
        let known = ids_of(&store);
        let old_id = *known.iter().next().unwrap();

        // Unknown id is rejected outright.
        let bogus = format!(
            r#"{{"ops":[{{"op":"supersede","id":{},"replacement":"x"}}]}}"#,
            old_id + 999
        );
        assert_eq!(apply_fact_ops(&store, &bogus, None, "cli", &known), 0);
        assert_eq!(store.live_facts(None).unwrap().len(), 1);

        // Known id supersedes and files the replacement.
        let good = format!(
            r#"{{"ops":[{{"op":"supersede","id":{old_id},"replacement":"never deploys on Fridays"}}]}}"#
        );
        assert_eq!(apply_fact_ops(&store, &good, None, "cli", &known), 1);
        let live = store.live_facts(None).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].body, "never deploys on Fridays");
    }

    #[test]
    fn caps_at_three_ops_per_turn() {
        let store = temp_store("cap");
        let json = r#"{"ops":[
            {"op":"add","scope":"user","body":"fact one"},
            {"op":"add","scope":"user","body":"fact two"},
            {"op":"add","scope":"user","body":"fact three"},
            {"op":"add","scope":"user","body":"fact four"},
            {"op":"add","scope":"user","body":"fact five"}
        ]}"#;
        assert_eq!(
            apply_fact_ops(&store, json, None, "cli", &HashSet::new()),
            3
        );
        assert_eq!(store.live_facts(None).unwrap().len(), 3);
    }

    #[test]
    fn project_op_without_workspace_is_skipped() {
        let store = temp_store("project");
        let json = r#"{"ops":[{"op":"add","scope":"project","body":"uses pnpm"}]}"#;
        assert_eq!(
            apply_fact_ops(&store, json, None, "cli", &HashSet::new()),
            0
        );
        assert!(store.live_facts(None).unwrap().is_empty());

        // With a workspace it lands and is scoped to that path.
        assert_eq!(
            apply_fact_ops(&store, json, Some("/tmp/repo-a"), "cli", &HashSet::new()),
            1
        );
        assert!(
            store
                .live_facts(Some("/tmp/repo-a"))
                .unwrap()
                .iter()
                .any(|f| f.body == "uses pnpm")
        );
        assert!(store.live_facts(Some("/tmp/repo-b")).unwrap().is_empty());
    }

    #[test]
    fn malformed_json_applies_nothing() {
        let store = temp_store("malformed");
        assert_eq!(
            apply_fact_ops(&store, "not json", None, "cli", &HashSet::new()),
            0
        );
        assert_eq!(
            apply_fact_ops(&store, "{}", None, "cli", &HashSet::new()),
            0
        );
    }

    // --- Part A: semantic fact recall --------------------------------------

    #[test]
    fn semantic_recall_keeps_the_on_topic_buried_fact() {
        let store = temp_store("recall");
        // Needle first, so it is the oldest (last by `updated_at DESC, id DESC`)
        // and well outside the recency-kept window.
        store
            .add_fact(
                "user",
                None,
                "obscure note about flyctl deploys zqxj",
                None,
                None,
            )
            .unwrap();
        // Then plenty of distinct filler to blow past MAX_FACTS_CHARS.
        let filler = "the quick brown fox jumps over the lazy dog thoroughly and repeatedly";
        for i in 0..40 {
            store
                .add_fact("user", None, &format!("{filler} number {i}"), None, None)
                .unwrap();
        }
        assert!(store.live_facts(None).unwrap().len() > FACT_RECENCY_KEEP + 5);

        let query = "how do I deploy with flyctl on this machine";

        let with = render_memory_facts(&store, None, None, Some(query), true).unwrap();
        assert!(
            with.contains("flyctl deploys zqxj"),
            "semantic recall should surface the buried on-topic fact:\n{with}"
        );

        let without = render_memory_facts(&store, None, None, Some(query), false).unwrap();
        assert!(
            !without.contains("flyctl deploys zqxj"),
            "newest-first fill should have dropped the oldest fact:\n{without}"
        );
    }

    // --- Part B: per-agent scoping + promotion ----------------------------

    #[test]
    fn subagent_add_is_quarantined_then_visible_only_to_that_agent() {
        let store = temp_store("quarantine");
        let json = r#"{"ops":[{"op":"add","scope":"user","body":"the researcher trusts arxiv over blogs"}]}"#;
        assert_eq!(
            apply_fact_ops(
                &store,
                json,
                None,
                "cli::subagent::researcher",
                &HashSet::new()
            ),
            1
        );
        assert!(
            store.live_facts_for_agent(None, None).unwrap().is_empty(),
            "a subagent fact must not enter the shared pool"
        );
        let own = store
            .live_facts_for_agent(None, Some("researcher"))
            .unwrap();
        assert_eq!(own.len(), 1);
        assert_eq!(own[0].agent_id.as_deref(), Some("researcher"));
        assert!(
            store
                .live_facts_for_agent(None, Some("other"))
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn main_agent_restatement_promotes_instead_of_duplicating() {
        let store = temp_store("promote-auto");
        // Quarantined subagent fact.
        apply_fact_ops(
            &store,
            r#"{"ops":[{"op":"add","scope":"user","body":"user deploys the app using flyctl and fly io"}]}"#,
            None,
            "cli::subagent::explorer",
            &HashSet::new(),
        );
        let quarantined_id = store.quarantined_facts(None).unwrap()[0].id;

        // Main agent says essentially the same thing (heavy token overlap so the
        // hash embedding clears FACT_PROMOTE_SIMILARITY).
        let n = apply_fact_ops(
            &store,
            r#"{"ops":[{"op":"add","scope":"user","body":"user deploys the app using flyctl and fly io CLI"}]}"#,
            None,
            "cli",
            &HashSet::new(),
        );
        assert_eq!(n, 1);
        assert!(store.quarantined_facts(None).unwrap().is_empty());
        let shared = store.live_facts_for_agent(None, None).unwrap();
        assert_eq!(shared.len(), 1, "promoted, not duplicated");
        assert_eq!(shared[0].id, quarantined_id, "same row, provenance kept");

        // A clearly unrelated main-agent add still inserts a fresh row.
        apply_fact_ops(
            &store,
            r#"{"ops":[{"op":"add","scope":"user","body":"user keyboard layout is Dvorak"}]}"#,
            None,
            "cli",
            &HashSet::new(),
        );
        assert_eq!(store.live_facts_for_agent(None, None).unwrap().len(), 2);
    }

    #[test]
    fn promote_fact_only_touches_quarantined_rows() {
        let store = temp_store("promote-manual");
        let shared_id = store
            .add_fact("user", None, "shared already", None, None)
            .unwrap()
            .unwrap();
        assert_eq!(store.promote_fact(shared_id).unwrap(), false);

        apply_fact_ops(
            &store,
            r#"{"ops":[{"op":"add","scope":"user","body":"quarantined thing"}]}"#,
            None,
            "cli::subagent::plan",
            &HashSet::new(),
        );
        let qid = store.quarantined_facts(None).unwrap()[0].id;
        assert_eq!(store.promote_fact(qid).unwrap(), true);
        assert!(store.quarantined_facts(None).unwrap().is_empty());
        assert_eq!(store.live_facts_for_agent(None, None).unwrap().len(), 2);
    }
}

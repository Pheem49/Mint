use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::search::text_embedding::{encode_embedding, fact_embedding_backend};

pub const CHAT_CLI_ID: &str = "cli";
pub const DEFAULT_CONVERSATION_ID: &str = "conversation-default";

/// Disabled: previously scoped the shared "cli" conversation by workspace,
/// but "workspace" in this app is a UI selection the user switches often
/// (the Workspace picker), not a stable per-process identity like a
/// terminal's cwd — scoping on it fragmented the one conversation meant to
/// stay shared across the CLI, desktop, and web into a different bucket
/// every time the selected workspace changed, making history disappear.
/// Always returns `chat_id` unchanged now; kept as a passthrough (rather
/// than removing it and its call sites) so this can be revisited later
/// with a stabler workspace identity.
pub fn scoped_chat_id(chat_id: &str, _workspace_path: Option<&str>) -> String {
    chat_id.to_owned()
}

/// True for the unscoped `"cli"` id or any workspace-scoped variant of it
/// (`"cli::<hash>"`), but NOT a subagent id nested under one
/// (`"cli::subagent::<name>"`, or `"cli::<hash>::subagent::<name>"`) —
/// those must keep behaving like a normal, deletable conversation exactly
/// as they do today, unaffected by workspace scoping.
fn is_cli_chat_id(chat_id: &str) -> bool {
    chat_id == CHAT_CLI_ID
        || (chat_id.starts_with(&format!("{CHAT_CLI_ID}::")) && !chat_id.contains("::subagent::"))
}

/// The subagent name embedded in a chat id of the form
/// `"<parent>::subagent::<name>"` (see `dispatch_one_subagent`), or `None` for
/// an ordinary conversation. Used to scope which agent a fact belongs to and
/// which facts an agent sees.
pub fn subagent_name(chat_id: &str) -> Option<&str> {
    chat_id
        .rsplit_once("::subagent::")
        .map(|(_, name)| name)
        .filter(|name| !name.is_empty())
}

#[derive(Debug, Error)]
pub enum MemoryError {
    #[error("unable to determine the user config directory")]
    ConfigDirectoryUnavailable,
    #[error("unable to create database directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

#[derive(Debug, Clone)]
pub struct MemoryStore {
    path: PathBuf,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct InteractionMemory {
    pub id: i64,
    pub chat_id: String,
    pub user_text: String,
    pub ai_text: String,
    pub provider: String,
    pub model: String,
    pub fallback_provider: Option<String>,
    pub created_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_activity: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ChatSession {
    pub id: String,
    pub title: String,
    pub kind: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LearnedSkill {
    pub id: i64,
    pub name: String,
    pub source_path: String,
    pub content: String,
    pub created_at: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, rename = "is_workspace")]
    pub is_workspace: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Fact {
    pub id: i64,
    /// `"user"` / `"preference"` are always injected; `"project"` only when the
    /// active workspace matches `project_path`.
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_path: Option<String>,
    pub body: String,
    /// The subagent that produced this fact, if any. `None` = shared / directly
    /// user-authored, injected into every agent's context. `Some(name)` =
    /// quarantined: only injected when that same subagent runs again, until it
    /// is promoted (see [`MemoryStore::promote_fact`]).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSession {
    pub workspace_path: String,
    pub summary: String,
    pub verification: String,
    pub updated_at: String,
}

impl MemoryStore {
    pub fn open_default() -> Result<Self, MemoryError> {
        Ok(Self::open(memory_path()?))
    }

    pub fn open(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn set_profile(&self, key: &str, value: &str) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO user_profile (key, value, updated_at)
             VALUES (?1, ?2, CURRENT_TIMESTAMP)
             ON CONFLICT(key) DO UPDATE SET
               value = excluded.value,
               updated_at = CURRENT_TIMESTAMP",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn get_profile(&self, key: &str) -> Result<Option<String>, MemoryError> {
        let connection = self.connection()?;
        Ok(connection
            .query_row(
                "SELECT value FROM user_profile WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn add_interaction(&self, user_text: &str, ai_text: &str) -> Result<i64, MemoryError> {
        self.add_interaction_with_metadata(user_text, ai_text, "", "")
    }

    pub fn add_interaction_with_metadata(
        &self,
        user_text: &str,
        ai_text: &str,
        provider: &str,
        model: &str,
    ) -> Result<i64, MemoryError> {
        self.add_interaction_for_chat(DEFAULT_CONVERSATION_ID, user_text, ai_text, provider, model)
    }

    pub fn add_interaction_for_chat(
        &self,
        chat_id: &str,
        user_text: &str,
        ai_text: &str,
        provider: &str,
        model: &str,
    ) -> Result<i64, MemoryError> {
        self.add_interaction_for_chat_with_fallback(
            chat_id, user_text, ai_text, provider, model, None,
        )
    }

    pub fn add_interaction_for_chat_with_fallback(
        &self,
        chat_id: &str,
        user_text: &str,
        ai_text: &str,
        provider: &str,
        model: &str,
        fallback_provider: Option<&str>,
    ) -> Result<i64, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        ensure_builtin_chat_sessions(&connection)?;
        ensure_chat_session_row(&connection, &chat_id)?;
        connection.execute(
            "INSERT INTO interaction_memories (chat_id, user_text, ai_text, provider, model, fallback_provider)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![chat_id, user_text, ai_text, provider, model, fallback_provider],
        )?;
        connection.execute(
            "UPDATE chat_sessions
             SET title = CASE
               WHEN title = 'New chat' AND ?2 != '' THEN substr(?2, 1, 80)
               ELSE title
             END,
             updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![chat_id, user_text.trim()],
        )?;
        Ok(connection.last_insert_rowid())
    }

    pub fn recent_interactions(&self, limit: usize) -> Result<Vec<InteractionMemory>, MemoryError> {
        self.recent_interactions_for_chat(DEFAULT_CONVERSATION_ID, limit)
    }

    pub fn recent_interactions_for_chat(
        &self,
        chat_id: &str,
        limit: usize,
    ) -> Result<Vec<InteractionMemory>, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        ensure_builtin_chat_sessions(&connection)?;
        ensure_chat_session_row(&connection, &chat_id)?;
        let mut statement = connection.prepare(
            "SELECT id, chat_id, user_text, ai_text, provider, model, fallback_provider, created_at, agent_activity_json
             FROM interaction_memories
             WHERE chat_id = ?1
             ORDER BY id DESC
             LIMIT ?2",
        )?;
        let rows = statement.query_map(params![chat_id, limit as i64], interaction_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Full-text search over `chat_id`'s past interactions for the turns most
    /// relevant to `query` (BM25 ranked), skipping the `exclude_recent` newest
    /// rows since those are already injected verbatim elsewhere. Returns
    /// `Ok(vec![])` without touching the database when `query` yields no usable
    /// search token.
    pub fn recall_interactions(
        &self,
        chat_id: &str,
        query: &str,
        exclude_recent: usize,
        limit: usize,
    ) -> Result<Vec<InteractionMemory>, MemoryError> {
        let Some(match_query) = fts_match_query(query) else {
            return Ok(Vec::new());
        };
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.chat_id, m.user_text, m.ai_text, m.provider, m.model,
                    m.fallback_provider, m.created_at, m.agent_activity_json
             FROM interaction_fts
             JOIN interaction_memories m ON m.id = interaction_fts.rowid
             WHERE interaction_fts MATCH ?1
               AND m.chat_id = ?2
               AND m.id NOT IN (
                 SELECT id FROM interaction_memories
                 WHERE chat_id = ?2 ORDER BY id DESC LIMIT ?3
               )
             ORDER BY bm25(interaction_fts)
             LIMIT ?4",
        )?;
        let rows = statement.query_map(
            params![match_query, chat_id, exclude_recent as i64, limit as i64],
            interaction_row,
        )?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_chat_sessions(&self) -> Result<Vec<ChatSession>, MemoryError> {
        let connection = self.connection()?;
        ensure_builtin_chat_sessions(&connection)?;
        let mut statement = connection.prepare(
            "SELECT id, title, kind, created_at, updated_at
             FROM chat_sessions
             ORDER BY CASE WHEN id = ?1 THEN 0 ELSE 1 END, updated_at DESC",
        )?;
        let rows = statement.query_map(params![CHAT_CLI_ID], |row| {
            Ok(ChatSession {
                id: row.get(0)?,
                title: row.get(1)?,
                kind: row.get(2)?,
                created_at: row.get(3)?,
                updated_at: row.get(4)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Registers a chat session with an explicit title/kind up front instead
    /// of the "New chat" placeholder — used to give a scheduled task's
    /// conversation its task name immediately (visible in the sidebar even
    /// before it has run once), rather than waiting for the generic
    /// first-message-based rename in `add_interaction_for_chat_with_fallback`
    /// to kick in. A no-op if the session already exists.
    pub fn ensure_named_chat_session(
        &self,
        chat_id: &str,
        title: &str,
        kind: &str,
    ) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
             VALUES (?1, ?2, ?3)",
            params![chat_id, title, kind],
        )?;
        Ok(())
    }

    pub fn set_interaction_agent_activity_json(
        &self,
        interaction_id: i64,
        activity_json: &str,
    ) -> Result<usize, MemoryError> {
        let connection = self.connection()?;
        Ok(connection.execute(
            "UPDATE interaction_memories
             SET agent_activity_json = ?2
             WHERE id = ?1",
            params![interaction_id, activity_json],
        )?)
    }

    pub fn clear_interactions(&self) -> Result<usize, MemoryError> {
        self.clear_interactions_for_chat(DEFAULT_CONVERSATION_ID)
    }

    pub fn clear_interactions_for_chat(&self, chat_id: &str) -> Result<usize, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        Ok(connection.execute(
            "DELETE FROM interaction_memories WHERE chat_id = ?1",
            params![chat_id],
        )?)
    }

    pub fn delete_chat_session(&self, chat_id: &str) -> Result<usize, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        if is_cli_chat_id(&chat_id) {
            return Ok(0);
        }
        let connection = self.connection()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM interaction_memories WHERE chat_id = ?1",
            params![chat_id],
        )?;
        // No `kind` filter here: the `is_cli_chat_id` guard above is what
        // protects the cli session(s) that must never be deleted this way.
        // Restricting to `kind = 'conversation'` used to also block deleting
        // any other kind (e.g. a stale row left behind by a since-removed
        // feature) even though nothing else needs that protection.
        let deleted =
            transaction.execute("DELETE FROM chat_sessions WHERE id = ?1", params![chat_id])?;
        transaction.commit()?;
        Ok(deleted)
    }

    /// Deletes `chat_id`'s conversation only if it has no interactions yet.
    ///
    /// Used when a scheduled task is removed: `CronStore::add` eagerly
    /// creates its conversation up front (titled, so it appears in the
    /// sidebar immediately instead of waiting for a first run) via
    /// `ensure_named_chat_session`, but if the task is deleted — or an "edit"
    /// is implemented as delete-then-recreate — before it ever actually
    /// fires, that placeholder conversation has zero interactions and would
    /// otherwise sit in the sidebar forever with nothing in it. A task that
    /// *did* run keeps its history: this only clears the empty case.
    pub fn delete_chat_session_if_empty(&self, chat_id: &str) -> Result<bool, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        if is_cli_chat_id(&chat_id) {
            return Ok(false);
        }
        let connection = self.connection()?;
        let transaction = connection.unchecked_transaction()?;
        let count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM interaction_memories WHERE chat_id = ?1",
            params![chat_id],
            |row| row.get(0),
        )?;
        if count > 0 {
            return Ok(false);
        }
        let deleted = transaction.execute(
            "DELETE FROM chat_sessions
             WHERE id = ?1 AND kind = 'conversation'",
            params![chat_id],
        )?;
        transaction.commit()?;
        Ok(deleted > 0)
    }

    pub fn rename_chat_session(
        &self,
        chat_id: &str,
        new_title: &str,
    ) -> Result<usize, MemoryError> {
        let chat_id = normalized_chat_id(chat_id);
        let connection = self.connection()?;
        let updated = connection.execute(
            "UPDATE chat_sessions
             SET title = ?2,
                 updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1",
            params![chat_id, new_title.trim()],
        )?;
        Ok(updated)
    }

    pub fn save_workspace_session(
        &self,
        workspace_path: &str,
        summary: &str,
        verification: &str,
    ) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO workspace_sessions (workspace_path, summary, verification, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(workspace_path) DO UPDATE SET
               summary = excluded.summary,
               verification = excluded.verification,
               updated_at = CURRENT_TIMESTAMP",
            params![workspace_path, summary, verification],
        )?;
        Ok(())
    }

    pub fn workspace_session(
        &self,
        workspace_path: &str,
    ) -> Result<Option<WorkspaceSession>, MemoryError> {
        let connection = self.connection()?;
        Ok(connection
            .query_row(
                "SELECT workspace_path, summary, verification, updated_at
                 FROM workspace_sessions WHERE workspace_path = ?1",
                params![workspace_path],
                |row| {
                    Ok(WorkspaceSession {
                        workspace_path: row.get(0)?,
                        summary: row.get(1)?,
                        verification: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?)
    }

    pub fn add_learned_skill(
        &self,
        name: &str,
        source_path: &str,
        content: &str,
    ) -> Result<LearnedSkill, MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "INSERT INTO learned_skills (name, source_path, content, updated_at)
             VALUES (?1, ?2, ?3, CURRENT_TIMESTAMP)
             ON CONFLICT(source_path) DO UPDATE SET
               name = excluded.name,
               content = excluded.content,
               updated_at = CURRENT_TIMESTAMP",
            params![name, source_path, content],
        )?;
        Ok(connection.query_row(
            "SELECT id, name, source_path, content, created_at
             FROM learned_skills WHERE source_path = ?1",
            params![source_path],
            learned_skill_row,
        )?)
    }

    pub fn learned_skills(&self, limit: usize) -> Result<Vec<LearnedSkill>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, name, source_path, content, created_at
             FROM learned_skills ORDER BY id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], learned_skill_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn delete_learned_skill(&self, identifier: &str) -> Result<usize, MemoryError> {
        let connection = self.connection()?;
        Ok(connection.execute(
            "DELETE FROM learned_skills
             WHERE CAST(id AS TEXT) = ?1 OR source_path = ?1 OR name = ?1",
            params![identifier],
        )?)
    }

    /// Inserts one durable fact. `scope` is `"user"` / `"preference"` (global,
    /// `project_path` must be `None`) or `"project"` (`project_path` set to the
    /// workspace it applies to). Returns `Some(id)` of the new row, or `None`
    /// when an identical live fact already exists (`idx_facts_dedup`).
    pub fn add_fact(
        &self,
        scope: &str,
        project_path: Option<&str>,
        body: &str,
        source_chat_id: Option<&str>,
        source_interaction_id: Option<i64>,
    ) -> Result<Option<i64>, MemoryError> {
        self.add_fact_for_agent(
            scope,
            project_path,
            body,
            source_chat_id,
            source_interaction_id,
            None,
        )
    }

    /// [`add_fact`] plus an `agent_id`: `Some(name)` quarantines the fact to that
    /// subagent (see [`Fact::agent_id`]); `None` is a shared fact. Also computes
    /// and stores the similarity embedding for the body.
    ///
    /// [`add_fact`]: Self::add_fact
    pub fn add_fact_for_agent(
        &self,
        scope: &str,
        project_path: Option<&str>,
        body: &str,
        source_chat_id: Option<&str>,
        source_interaction_id: Option<i64>,
        agent_id: Option<&str>,
    ) -> Result<Option<i64>, MemoryError> {
        let connection = self.connection()?;
        let embedding = encode_embedding(&fact_embedding_backend().embed(body));
        let changed = connection.execute(
            "INSERT INTO facts
               (scope, project_path, body, source_chat_id, source_interaction_id, agent_id, embedding)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT DO NOTHING",
            params![
                scope,
                project_path,
                body,
                source_chat_id,
                source_interaction_id,
                agent_id,
                embedding,
            ],
        )?;
        Ok((changed > 0).then(|| connection.last_insert_rowid()))
    }

    /// Live (non-superseded) facts relevant to `project_path`: all `user` /
    /// `preference` facts, plus `project` facts whose `project_path` matches.
    /// Shared facts only (`agent_id IS NULL`). Newest first.
    pub fn live_facts(&self, project_path: Option<&str>) -> Result<Vec<Fact>, MemoryError> {
        self.live_facts_for_agent(project_path, None)
    }

    /// [`live_facts`] scoped to a viewer: `agent_id = None` sees only shared
    /// facts; `agent_id = Some(name)` sees shared facts plus that subagent's own
    /// quarantined facts.
    ///
    /// [`live_facts`]: Self::live_facts
    pub fn live_facts_for_agent(
        &self,
        project_path: Option<&str>,
        agent_id: Option<&str>,
    ) -> Result<Vec<Fact>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope, project_path, body, created_at, updated_at, agent_id
             FROM facts
             WHERE superseded_by IS NULL
               AND (agent_id IS NULL OR agent_id = ?2)
               AND (scope IN ('user', 'preference')
                    OR (scope = 'project' AND project_path IS NOT NULL AND project_path = ?1))
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![project_path, agent_id], fact_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Live quarantined facts (`agent_id IS NOT NULL`) relevant to `project_path`
    /// — the candidates the parent agent's restatement can promote (see
    /// [`MemoryStore::promote_fact`]) and what `/memory facts` shows as
    /// `via <subagent>`.
    pub fn quarantined_facts(&self, project_path: Option<&str>) -> Result<Vec<Fact>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope, project_path, body, created_at, updated_at, agent_id
             FROM facts
             WHERE superseded_by IS NULL
               AND agent_id IS NOT NULL
               AND (scope IN ('user', 'preference')
                    OR (scope = 'project' AND project_path IS NOT NULL AND project_path = ?1))
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![project_path], fact_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Same rows as [`live_facts_for_agent`], each paired with its similarity
    /// vector — decoded from the stored blob, or recomputed from the body when
    /// the blob is missing (row predates the column) or the wrong width (written
    /// by a different embedding backend). One query, so ranking recall doesn't
    /// fan out into per-fact lookups.
    ///
    /// [`live_facts_for_agent`]: Self::live_facts_for_agent
    pub fn live_facts_with_embedding(
        &self,
        project_path: Option<&str>,
        agent_id: Option<&str>,
    ) -> Result<Vec<(Fact, Vec<f32>)>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope, project_path, body, created_at, updated_at, agent_id, embedding
             FROM facts
             WHERE superseded_by IS NULL
               AND (agent_id IS NULL OR agent_id = ?2)
               AND (scope IN ('user', 'preference')
                    OR (scope = 'project' AND project_path IS NOT NULL AND project_path = ?1))
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![project_path, agent_id], fact_with_embedding_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// [`quarantined_facts`] each paired with its similarity vector, resolved the
    /// same way as [`live_facts_with_embedding`].
    ///
    /// [`quarantined_facts`]: Self::quarantined_facts
    pub fn quarantined_facts_with_embedding(
        &self,
        project_path: Option<&str>,
    ) -> Result<Vec<(Fact, Vec<f32>)>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope, project_path, body, created_at, updated_at, agent_id, embedding
             FROM facts
             WHERE superseded_by IS NULL
               AND agent_id IS NOT NULL
               AND (scope IN ('user', 'preference')
                    OR (scope = 'project' AND project_path IS NOT NULL AND project_path = ?1))
             ORDER BY updated_at DESC, id DESC",
        )?;
        let rows = statement.query_map(params![project_path], fact_with_embedding_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Lifts a quarantined fact into the shared pool. Returns `true` if a row was
    /// actually promoted (it existed and had a non-NULL `agent_id`).
    pub fn promote_fact(&self, id: i64) -> Result<bool, MemoryError> {
        let connection = self.connection()?;
        let changed = connection.execute(
            "UPDATE facts SET agent_id = NULL, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND agent_id IS NOT NULL AND superseded_by IS NULL",
            params![id],
        )?;
        Ok(changed > 0)
    }

    /// Most recent facts regardless of scope or workspace, for `/memory facts`.
    pub fn list_facts(&self, limit: usize) -> Result<Vec<Fact>, MemoryError> {
        let connection = self.connection()?;
        let mut statement = connection.prepare(
            "SELECT id, scope, project_path, body, created_at, updated_at, agent_id
             FROM facts WHERE superseded_by IS NULL
             ORDER BY updated_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit as i64], fact_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    /// Marks `old_id` as no longer live. `new_id` records which fact replaced it;
    /// pass `None` for a plain retraction, stored as the sentinel `-1` so the row
    /// still counts as superseded (a real `NULL` would leave it live).
    pub fn supersede_fact(&self, old_id: i64, new_id: Option<i64>) -> Result<(), MemoryError> {
        let connection = self.connection()?;
        connection.execute(
            "UPDATE facts SET superseded_by = ?2, updated_at = CURRENT_TIMESTAMP
             WHERE id = ?1 AND superseded_by IS NULL",
            params![old_id, new_id.unwrap_or(-1)],
        )?;
        Ok(())
    }

    /// Hard-deletes facts matching `identifier` — an exact id, or a substring of
    /// the body. Returns the number of rows removed.
    pub fn forget_fact(&self, identifier: &str) -> Result<usize, MemoryError> {
        let connection = self.connection()?;
        Ok(connection.execute(
            "DELETE FROM facts
             WHERE CAST(id AS TEXT) = ?1 OR body LIKE '%' || ?1 || '%'",
            params![identifier],
        )?)
    }

    fn connection(&self) -> Result<Connection, MemoryError> {
        if let Some(directory) = self.path.parent() {
            std::fs::create_dir_all(directory).map_err(|source| MemoryError::CreateDirectory {
                path: directory.to_path_buf(),
                source,
            })?;
        }
        let connection = Connection::open(&self.path)?;
        // journal_mode=WAL (set in `initialize` below) means readers never
        // block writers, but two concurrent writers — always possible with
        // multiple Mint surfaces sharing this DB, more so now that
        // `live_sync` polls it continuously — still serialize against each
        // other. Without a busy_timeout the losing writer fails immediately
        // with SQLITE_BUSY instead of waiting briefly for the lock.
        connection.busy_timeout(std::time::Duration::from_millis(5000))?;

        static INITIALIZED_DATABASES: std::sync::LazyLock<
            std::sync::Mutex<std::collections::HashSet<PathBuf>>,
        > = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashSet::new()));

        let needs_init = {
            let mut set = INITIALIZED_DATABASES.lock().unwrap();
            set.insert(self.path.clone())
        };

        if needs_init {
            initialize(
                &connection,
                memory_path().is_ok_and(|default_path| default_path == self.path),
            )?;
            // This database holds OAuth tokens (see `oauth::save_oauth_tokens`,
            // stored in `user_profile`) alongside chat history — restrict it
            // to owner-only rather than leaving it at default (typically
            // world-readable) permissions. `needs_init` already gates this to
            // once per path per process, and any pre-existing database from
            // before this fix gets tightened the same way on its first open.
            restrict_to_owner(&self.path);
        }
        Ok(connection)
    }
}

#[cfg(unix)]
fn restrict_to_owner(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) {}

pub fn memory_path() -> Result<PathBuf, MemoryError> {
    dirs::config_dir()
        .map(|directory| directory.join("mint").join("mint-knowledge.sqlite"))
        .ok_or(MemoryError::ConfigDirectoryUnavailable)
}

fn migrate_json_history(connection: &Connection) -> Result<(), rusqlite::Error> {
    if cfg!(test) {
        return Ok(());
    }
    let config_dir = match dirs::config_dir() {
        Some(dir) => dir,
        None => return Ok(()),
    };
    let json_path = config_dir.join("mint").join("mint-chat-history.json");
    if !json_path.exists() {
        return Ok(());
    }

    let already_migrated: bool = connection
        .query_row(
            "SELECT 1 FROM user_profile WHERE key = 'json_history_migrated'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);

    if already_migrated {
        return Ok(());
    }

    let file_content = match std::fs::read_to_string(&json_path) {
        Ok(content) => content,
        Err(_) => return Ok(()),
    };

    let messages: Vec<serde_json::Value> = match serde_json::from_str(&file_content) {
        Ok(msgs) => msgs,
        Err(_) => return Ok(()),
    };

    let mut i = 0;
    while i < messages.len() {
        let msg = &messages[i];
        let role = msg.get("role").and_then(|r| r.as_str()).unwrap_or("");

        if role == "user" {
            let user_text = msg
                .get("parts")
                .and_then(|p| p.as_array())
                .and_then(|arr| arr.first())
                .and_then(|first| first.get("text"))
                .and_then(|t| t.as_str())
                .unwrap_or("");

            let mut ai_text = "";
            let ai_text_buf;

            if i + 1 < messages.len() {
                let next_msg = &messages[i + 1];
                let next_role = next_msg.get("role").and_then(|r| r.as_str()).unwrap_or("");
                if next_role == "model" {
                    let raw_ai_text = next_msg
                        .get("parts")
                        .and_then(|p| p.as_array())
                        .and_then(|arr| arr.first())
                        .and_then(|first| first.get("text"))
                        .and_then(|t| t.as_str())
                        .unwrap_or("");

                    if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(raw_ai_text) {
                        if let Some(resp) = parsed.get("response").and_then(|r| r.as_str()) {
                            ai_text_buf = resp.to_string();
                            ai_text = &ai_text_buf;
                        } else {
                            ai_text = raw_ai_text;
                        }
                    } else {
                        ai_text = raw_ai_text;
                    }
                    i += 2;
                } else {
                    i += 1;
                }
            } else {
                i += 1;
            }

            if !user_text.trim().is_empty() {
                let created_at = msg.get("timestamp").and_then(|t| t.as_str()).unwrap_or("");
                if !created_at.is_empty() {
                    let _ = connection.execute(
                        "INSERT INTO interaction_memories (user_text, ai_text, created_at)
                         VALUES (?1, ?2, ?3)",
                        params![user_text, ai_text, created_at],
                    );
                } else {
                    let _ = connection.execute(
                        "INSERT INTO interaction_memories (user_text, ai_text)
                         VALUES (?1, ?2)",
                        params![user_text, ai_text],
                    );
                }
            }
        } else {
            i += 1;
        }
    }

    let _ = connection.execute(
        "INSERT OR REPLACE INTO user_profile (key, value, updated_at)
         VALUES ('json_history_migrated', 'true', CURRENT_TIMESTAMP)",
        [],
    );

    Ok(())
}

fn initialize(
    connection: &Connection,
    migrate_legacy_history: bool,
) -> Result<(), rusqlite::Error> {
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         CREATE TABLE IF NOT EXISTS user_profile (
           key TEXT PRIMARY KEY,
           value TEXT,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS interaction_memories (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           chat_id TEXT NOT NULL DEFAULT 'conversation-default',
           user_text TEXT NOT NULL,
           ai_text TEXT NOT NULL,
           provider TEXT NOT NULL DEFAULT '',
           model TEXT NOT NULL DEFAULT '',
           fallback_provider TEXT DEFAULT NULL,
           keywords TEXT DEFAULT '',
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS chat_sessions (
           id TEXT PRIMARY KEY,
           title TEXT NOT NULL DEFAULT 'New chat',
           kind TEXT NOT NULL DEFAULT 'conversation',
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS learned_skills (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           name TEXT NOT NULL,
           source_path TEXT NOT NULL UNIQUE,
           content TEXT NOT NULL,
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS workspace_sessions (
           workspace_path TEXT PRIMARY KEY,
           summary TEXT NOT NULL,
           verification TEXT NOT NULL DEFAULT '',
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
         );
         CREATE TABLE IF NOT EXISTS facts (
           id INTEGER PRIMARY KEY AUTOINCREMENT,
           scope TEXT NOT NULL DEFAULT 'user',
           project_path TEXT DEFAULT NULL,
           body TEXT NOT NULL,
           source_chat_id TEXT DEFAULT NULL,
           source_interaction_id INTEGER DEFAULT NULL,
           created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
           updated_at DATETIME DEFAULT CURRENT_TIMESTAMP,
           superseded_by INTEGER DEFAULT NULL,
           agent_id TEXT DEFAULT NULL,
           embedding BLOB
         );
         CREATE INDEX IF NOT EXISTS idx_facts_live
           ON facts(scope, project_path) WHERE superseded_by IS NULL;
         CREATE TRIGGER IF NOT EXISTS trg_facts_touch
         AFTER UPDATE OF body, scope, project_path, superseded_by ON facts
         FOR EACH ROW WHEN NEW.updated_at = OLD.updated_at
         BEGIN
           UPDATE facts SET updated_at = CURRENT_TIMESTAMP WHERE id = NEW.id;
         END;
         CREATE VIRTUAL TABLE IF NOT EXISTS interaction_fts USING fts5(
           user_text,
           ai_text,
           content='interaction_memories',
           content_rowid='id',
           tokenize='trigram'
         );
         CREATE TRIGGER IF NOT EXISTS trg_interaction_fts_ai
         AFTER INSERT ON interaction_memories BEGIN
           INSERT INTO interaction_fts(rowid, user_text, ai_text)
           VALUES (new.id, new.user_text, new.ai_text);
         END;
         CREATE TRIGGER IF NOT EXISTS trg_interaction_fts_ad
         AFTER DELETE ON interaction_memories BEGIN
           INSERT INTO interaction_fts(interaction_fts, rowid, user_text, ai_text)
           VALUES ('delete', old.id, old.user_text, old.ai_text);
         END;
         CREATE TRIGGER IF NOT EXISTS trg_interaction_fts_au
         AFTER UPDATE ON interaction_memories BEGIN
           INSERT INTO interaction_fts(interaction_fts, rowid, user_text, ai_text)
           VALUES ('delete', old.id, old.user_text, old.ai_text);
           INSERT INTO interaction_fts(rowid, user_text, ai_text)
           VALUES (new.id, new.user_text, new.ai_text);
         END;",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "chat_id",
        "TEXT NOT NULL DEFAULT 'conversation-default'",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "provider",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "model",
        "TEXT NOT NULL DEFAULT ''",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "fallback_provider",
        "TEXT DEFAULT NULL",
    )?;
    ensure_column(
        connection,
        "interaction_memories",
        "agent_activity_json",
        "TEXT DEFAULT NULL",
    )?;
    ensure_column(
        connection,
        "chat_sessions",
        "kind",
        "TEXT NOT NULL DEFAULT 'conversation'",
    )?;
    // `agent_id` scopes a fact to the subagent that produced it (NULL = shared /
    // user-authored); `embedding` is the on-device similarity vector used for
    // relevance-ranked recall. Both are additive for databases created before
    // they existed.
    ensure_column(connection, "facts", "agent_id", "TEXT DEFAULT NULL")?;
    ensure_column(connection, "facts", "embedding", "BLOB")?;
    // The dedup uniqueness now keys on `agent_id` too, so a subagent's
    // quarantined fact and an identical shared one can coexist. Drop/recreate
    // rather than `IF NOT EXISTS` since the column set changed; it is a pure
    // derived index so rebuilding it is safe.
    connection.execute("DROP INDEX IF EXISTS idx_facts_dedup", [])?;
    connection.execute(
        "CREATE UNIQUE INDEX idx_facts_dedup
           ON facts(scope, ifnull(project_path, ''), ifnull(agent_id, ''), body)
           WHERE superseded_by IS NULL",
        [],
    )?;
    connection.execute(
        "UPDATE interaction_memories
         SET chat_id = ?1
         WHERE chat_id IS NULL OR trim(chat_id) = ''",
        params![DEFAULT_CONVERSATION_ID],
    )?;
    ensure_builtin_chat_sessions(connection)?;
    connection.execute(
        "UPDATE interaction_memories
         SET chat_id = ?1
         WHERE chat_id = ?2",
        params![CHAT_CLI_ID, DEFAULT_CONVERSATION_ID],
    )?;
    connection.execute(
        "CREATE INDEX IF NOT EXISTS idx_interaction_memories_chat_id_id
         ON interaction_memories(chat_id, id)",
        [],
    )?;
    // `keywords` was declared years ago but never read or written by any code
    // path — the FTS5 index below is what actually powers recall now. Drop it so
    // the column doesn't mislead. Needs SQLite >= 3.35 (bundled is far newer).
    drop_column_if_exists(connection, "interaction_memories", "keywords")?;
    if migrate_legacy_history {
        migrate_json_history(connection)?;
    }
    // One-time populate of `interaction_fts` for rows that predate it. The
    // triggers above keep it current from here on; this only needs to run once
    // per database, gated by a sentinel like `migrate_json_history` uses.
    let fts_backfilled: bool = connection
        .query_row(
            "SELECT 1 FROM user_profile WHERE key = 'interaction_fts_backfilled'",
            [],
            |_| Ok(true),
        )
        .unwrap_or(false);
    if !fts_backfilled {
        connection.execute(
            "INSERT INTO interaction_fts(interaction_fts) VALUES('rebuild')",
            [],
        )?;
        connection.execute(
            "INSERT OR REPLACE INTO user_profile (key, value, updated_at)
             VALUES ('interaction_fts_backfilled', 'true', CURRENT_TIMESTAMP)",
            [],
        )?;
    }
    Ok(())
}

fn ensure_column(
    connection: &Connection,
    table: &str,
    column: &str,
    definition: &str,
) -> Result<(), rusqlite::Error> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    for existing in columns {
        if existing? == column {
            return Ok(());
        }
    }
    connection.execute(
        &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
        [],
    )?;
    Ok(())
}

/// Inverse of [`ensure_column`]: drops `column` from `table` if it is still
/// there, and is a no-op once it's gone. `ALTER TABLE ... DROP COLUMN` needs
/// SQLite >= 3.35.
fn drop_column_if_exists(
    connection: &Connection,
    table: &str,
    column: &str,
) -> Result<(), rusqlite::Error> {
    let mut statement = connection.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
    let mut present = false;
    for existing in columns {
        if existing? == column {
            present = true;
            break;
        }
    }
    if present {
        connection.execute(&format!("ALTER TABLE {table} DROP COLUMN {column}"), [])?;
    }
    Ok(())
}

fn learned_skill_row(row: &rusqlite::Row<'_>) -> Result<LearnedSkill, rusqlite::Error> {
    let content: String = row.get(3)?;
    let description = crate::skills::parse_skill_description(&content);
    Ok(LearnedSkill {
        id: row.get(0)?,
        name: row.get(1)?,
        source_path: row.get(2)?,
        content,
        created_at: row.get(4)?,
        description,
        is_workspace: false,
    })
}

/// Turns free-form user text into a safe FTS5 `MATCH` string for the `trigram`
/// tokenizer: pulls out alphanumeric runs (Unicode-aware, so Thai counts) of at
/// least 3 chars — the trigram minimum — lowercases and dedupes them, caps at
/// 12, and quotes each so it is treated as a literal (a `"` can never appear in
/// a token, so this can't inject FTS operators). `None` if nothing usable is
/// left, so the caller can skip the query entirely.
fn fts_match_query(query: &str) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    let mut tokens: Vec<String> = Vec::new();
    for raw in query.split(|c: char| !c.is_alphanumeric()) {
        if raw.chars().count() < 3 {
            continue;
        }
        let token = raw.to_lowercase();
        if seen.insert(token.clone()) {
            tokens.push(token);
            if tokens.len() == 12 {
                break;
            }
        }
    }
    if tokens.is_empty() {
        return None;
    }
    Some(
        tokens
            .iter()
            .map(|t| format!("\"{t}\""))
            .collect::<Vec<_>>()
            .join(" OR "),
    )
}

fn fact_row(row: &rusqlite::Row<'_>) -> Result<Fact, rusqlite::Error> {
    Ok(Fact {
        id: row.get(0)?,
        scope: row.get(1)?,
        project_path: row.get(2)?,
        body: row.get(3)?,
        created_at: row.get(4)?,
        updated_at: row.get(5)?,
        agent_id: row.get(6)?,
    })
}

/// Row mapper for the `SELECT … , embedding` fact queries: the [`Fact`] plus a
/// usable similarity vector, recomputing from the body when the stored blob is
/// absent or the wrong width for the current backend.
fn fact_with_embedding_row(row: &rusqlite::Row<'_>) -> Result<(Fact, Vec<f32>), rusqlite::Error> {
    let fact = fact_row(row)?;
    let backend = fact_embedding_backend();
    let vector = row
        .get::<_, Option<Vec<u8>>>(7)?
        .and_then(|bytes| crate::search::text_embedding::decode_embedding(&bytes).ok())
        .filter(|v| v.len() == backend.dim())
        .unwrap_or_else(|| backend.embed(&fact.body));
    Ok((fact, vector))
}

fn interaction_row(row: &rusqlite::Row<'_>) -> Result<InteractionMemory, rusqlite::Error> {
    let agent_activity = row
        .get::<_, Option<String>>(8)?
        .and_then(|raw| serde_json::from_str(&raw).ok());
    Ok(InteractionMemory {
        id: row.get(0)?,
        chat_id: row.get(1)?,
        user_text: row.get(2)?,
        ai_text: row.get(3)?,
        provider: row.get(4)?,
        model: row.get(5)?,
        fallback_provider: row.get(6)?,
        created_at: row.get(7)?,
        agent_activity,
    })
}

fn normalized_chat_id(chat_id: &str) -> String {
    let trimmed = chat_id.trim();
    if trimmed.is_empty() {
        DEFAULT_CONVERSATION_ID.to_owned()
    } else {
        trimmed.to_owned()
    }
}

fn ensure_builtin_chat_sessions(connection: &Connection) -> Result<(), rusqlite::Error> {
    connection.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
         VALUES (?1, 'cli', 'cli')",
        params![CHAT_CLI_ID],
    )?;
    connection.execute(
        "UPDATE chat_sessions
         SET title = 'cli', kind = 'cli'
         WHERE id = ?1",
        params![CHAT_CLI_ID],
    )?;
    connection.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
         VALUES (?1, 'Conversation', 'conversation')",
        params![DEFAULT_CONVERSATION_ID],
    )?;
    Ok(())
}

fn ensure_chat_session_row(connection: &Connection, chat_id: &str) -> Result<(), rusqlite::Error> {
    let (title, kind) = if is_cli_chat_id(chat_id) {
        ("Chat CLI", "cli")
    } else {
        ("New chat", "conversation")
    };
    connection.execute(
        "INSERT OR IGNORE INTO chat_sessions (id, title, kind)
         VALUES (?1, ?2, ?3)",
        params![chat_id, title, kind],
    )?;
    Ok(())
}

#[cfg(test)]
mod scoped_chat_id_tests {
    use super::*;

    #[test]
    fn non_cli_ids_pass_through_unchanged() {
        assert_eq!(
            scoped_chat_id("conversation-default", Some("/tmp")),
            "conversation-default"
        );
        assert_eq!(
            scoped_chat_id("cli::subagent::search", Some("/tmp")),
            "cli::subagent::search"
        );
    }

    #[test]
    fn no_workspace_falls_back_to_the_plain_global_bucket() {
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, None), CHAT_CLI_ID);
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, Some("")), CHAT_CLI_ID);
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, Some("   ")), CHAT_CLI_ID);
    }

    #[test]
    fn a_workspace_no_longer_changes_the_shared_cli_id() {
        let cwd = std::env::current_dir().unwrap();
        let path = cwd.to_string_lossy().into_owned();
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, Some(&path)), CHAT_CLI_ID);
    }

    #[test]
    fn scoping_is_idempotent() {
        let cwd = std::env::current_dir().unwrap();
        let path = cwd.to_string_lossy().into_owned();
        let once = scoped_chat_id(CHAT_CLI_ID, Some(&path));
        let twice = scoped_chat_id(&once, Some(&path));
        assert_eq!(once, twice);
    }

    #[test]
    fn is_cli_chat_id_excludes_subagents_but_includes_scoped_variants() {
        assert!(is_cli_chat_id(CHAT_CLI_ID));
        assert!(is_cli_chat_id("cli::abc123456789"));
        assert!(!is_cli_chat_id("cli::subagent::search"));
        assert!(!is_cli_chat_id("cli::abc123456789::subagent::search"));
        assert!(!is_cli_chat_id("conversation-default"));
    }
}

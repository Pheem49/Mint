use std::path::{Path, PathBuf};

use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CHAT_CLI_ID: &str = "cli";
pub const DEFAULT_CONVERSATION_ID: &str = "conversation-default";

/// Scopes the shared "cli" conversation by workspace so unrelated projects
/// don't share context/history — every other `chat_id` (regular
/// conversations, subagent ids, ...) passes through unchanged. Idempotent:
/// calling this again on an already-scoped id (`"cli::<hash>"`) is a no-op,
/// since the guard below only fires on the literal `CHAT_CLI_ID`. With no
/// workspace known, falls back to the plain `"cli"` bucket — today's
/// behavior, so existing history never needs migrating.
pub fn scoped_chat_id(chat_id: &str, workspace_path: Option<&str>) -> String {
    if chat_id != CHAT_CLI_ID {
        return chat_id.to_owned();
    }
    let Some(path) = workspace_path.map(str::trim).filter(|p| !p.is_empty()) else {
        return CHAT_CLI_ID.to_owned();
    };
    let canonical = std::fs::canonicalize(path)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_owned());
    let digest = format!("{:x}", Sha256::digest(canonical.as_bytes()));
    format!("{CHAT_CLI_ID}::{}", &digest[..12])
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
         );",
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
    if migrate_legacy_history {
        migrate_json_history(connection)?;
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
        assert_eq!(scoped_chat_id("conversation-default", Some("/tmp")), "conversation-default");
        assert_eq!(scoped_chat_id("cli::subagent::search", Some("/tmp")), "cli::subagent::search");
    }

    #[test]
    fn no_workspace_falls_back_to_the_plain_global_bucket() {
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, None), CHAT_CLI_ID);
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, Some("")), CHAT_CLI_ID);
        assert_eq!(scoped_chat_id(CHAT_CLI_ID, Some("   ")), CHAT_CLI_ID);
    }

    #[test]
    fn a_workspace_produces_a_stable_scoped_id() {
        let cwd = std::env::current_dir().unwrap();
        let path = cwd.to_string_lossy().into_owned();
        let scoped = scoped_chat_id(CHAT_CLI_ID, Some(&path));
        assert!(scoped.starts_with("cli::"));
        assert_eq!(scoped, scoped_chat_id(CHAT_CLI_ID, Some(&path)));
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

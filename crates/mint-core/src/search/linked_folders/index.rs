//! Local, folder-scoped document index and durable note ledger.
use std::collections::{HashMap, HashSet};
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::time::UNIX_EPOCH;

use chrono::Local;
use ignore::WalkBuilder;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use uuid::Uuid;

use super::{LinkedFolder, LinkedFolderNote, LinkedFolderStatus};
use crate::search::text_embedding::{
    cosine_similarity, decode_embedding, embedding, encode_embedding,
};
use crate::{Capability, MintConfig, assert_path_capability, extract_document_text, memory_path};

const MAX_FILE_BYTES: u64 = 10 * 1024 * 1024;
const CHUNK_CHARS: usize = 1000;
const CHUNK_OVERLAP: usize = 200;

fn db() -> Result<Connection, String> {
    open_db().map_err(|error| error.to_string())
}

pub(super) type ClaimedJob = (String, String, String, i64);

#[derive(Debug, thiserror::Error)]
pub(super) enum ClaimError {
    #[error(transparent)]
    Database(#[from] rusqlite::Error),
    #[error("{0}")]
    Setup(String),
}

impl ClaimError {
    pub(super) fn is_busy(&self) -> bool {
        matches!(self, Self::Database(rusqlite::Error::SqliteFailure(error, _))
            if matches!(error.code, rusqlite::ErrorCode::DatabaseBusy | rusqlite::ErrorCode::DatabaseLocked))
    }
}

fn open_db() -> Result<Connection, ClaimError> {
    let path = memory_path().map_err(|e| ClaimError::Setup(e.to_string()))?;
    open_db_at(&path)
}

fn open_db_at(path: &Path) -> Result<Connection, ClaimError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| ClaimError::Setup(e.to_string()))?;
    }
    let conn = Connection::open(path)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS linked_folder_files (
           folder TEXT NOT NULL, root TEXT NOT NULL, path TEXT NOT NULL,
           mtime INTEGER NOT NULL, bytes INTEGER NOT NULL, PRIMARY KEY(folder, root, path));
         CREATE TABLE IF NOT EXISTS linked_folder_chunks (
           folder TEXT NOT NULL, root TEXT NOT NULL, path TEXT NOT NULL,
           ordinal INTEGER NOT NULL, text TEXT NOT NULL, embedding BLOB NOT NULL,
           PRIMARY KEY(folder, root, path, ordinal));
         CREATE INDEX IF NOT EXISTS idx_linked_chunks_folder ON linked_folder_chunks(folder, root);
         CREATE TABLE IF NOT EXISTS linked_folder_index_status (
           folder TEXT NOT NULL, root TEXT NOT NULL, indexed_at TEXT, error TEXT,
           PRIMARY KEY(folder, root));
         CREATE TABLE IF NOT EXISTS linked_folder_notes (
           id TEXT PRIMARY KEY, job_id TEXT, folder TEXT NOT NULL, root TEXT NOT NULL,
           path TEXT NOT NULL, content TEXT NOT NULL, created_at TEXT NOT NULL,
           status TEXT NOT NULL, error TEXT,
           UNIQUE(job_id, folder, root));
         CREATE INDEX IF NOT EXISTS idx_linked_notes_folder ON linked_folder_notes(folder, root, created_at);
         CREATE TABLE IF NOT EXISTS linked_folder_jobs (
           id TEXT PRIMARY KEY, user_text TEXT NOT NULL, ai_text TEXT NOT NULL,
           status TEXT NOT NULL, attempts INTEGER NOT NULL DEFAULT 0,
           error TEXT, created_at TEXT NOT NULL, claimed_at INTEGER);"
    )?;
    Ok(conn)
}

fn root(folder: &LinkedFolder) -> String {
    folder.path.to_string_lossy().into_owned()
}

fn supported(path: &Path) -> bool {
    path.extension().and_then(|e| e.to_str()).is_some_and(|e| {
        matches!(
            e.to_ascii_lowercase().as_str(),
            "txt"
                | "md"
                | "pdf"
                | "docx"
                | "xlsx"
                | "json"
                | "toml"
                | "yaml"
                | "yml"
                | "rs"
                | "ts"
                | "tsx"
                | "js"
                | "jsx"
                | "py"
                | "html"
                | "css"
        )
    })
}

pub(super) fn refresh(
    folder: &LinkedFolder,
    config: &MintConfig,
) -> Result<LinkedFolderStatus, String> {
    if fs::canonicalize(&folder.path).map_err(|e| e.to_string())? != folder.path {
        return Err("linked folder path has changed".into());
    }
    assert_path_capability(&folder.path, Capability::Read, config).map_err(|e| e.to_string())?;
    let mut conn = db()?;
    let root = root(folder);
    let mut seen = HashSet::new();
    let mut errors = Vec::new();
    let walker = WalkBuilder::new(&folder.path)
        .hidden(true)
        .follow_links(false)
        .build();
    for entry in walker {
        let entry = match entry {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        let path = entry.path();
        if !entry.file_type().is_some_and(|t| t.is_file())
            || !supported(path)
            || path
                .strip_prefix(&folder.path)
                .ok()
                .is_some_and(|relative| {
                    relative
                        .components()
                        .any(|part| part.as_os_str() == "mint-notes")
                })
        {
            continue;
        }
        let metadata = match entry.metadata() {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                continue;
            }
        };
        if metadata.len() > MAX_FILE_BYTES {
            continue;
        }
        let path_text = path.to_string_lossy().into_owned();
        seen.insert(path_text.clone());
        let mtime = metadata
            .modified()
            .ok()
            .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |v| v.as_nanos().min(i64::MAX as u128) as i64);
        let old: Option<(i64, i64)> = conn.query_row(
            "SELECT mtime, bytes FROM linked_folder_files WHERE folder=?1 AND root=?2 AND path=?3",
            params![folder.name, root, path_text], |r| Ok((r.get(0)?, r.get(1)?)))
            .optional().map_err(|e| e.to_string())?;
        if old == Some((mtime, metadata.len() as i64)) {
            continue;
        }
        let content = match extract_document_text(path, config) {
            Ok(v) => v,
            Err(e) => {
                errors.push(e.to_string());
                conn.execute(
                    "DELETE FROM linked_folder_chunks WHERE folder=?1 AND root=?2 AND path=?3",
                    params![folder.name, root, path_text],
                )
                .map_err(|e| e.to_string())?;
                conn.execute(
                    "DELETE FROM linked_folder_files WHERE folder=?1 AND root=?2 AND path=?3",
                    params![folder.name, root, path_text],
                )
                .map_err(|e| e.to_string())?;
                continue;
            }
        };
        let tx = conn.transaction().map_err(|e| e.to_string())?;
        tx.execute(
            "DELETE FROM linked_folder_chunks WHERE folder=?1 AND root=?2 AND path=?3",
            params![folder.name, root, path_text],
        )
        .map_err(|e| e.to_string())?;
        let chars: Vec<char> = content.chars().collect();
        let mut start = 0;
        let mut ordinal = 0;
        while start < chars.len() {
            let end = (start + CHUNK_CHARS).min(chars.len());
            let text: String = chars[start..end].iter().collect();
            tx.execute("INSERT INTO linked_folder_chunks(folder,root,path,ordinal,text,embedding) VALUES(?1,?2,?3,?4,?5,?6)",
                params![folder.name, root, path_text, ordinal, text, encode_embedding(&embedding(&text))]).map_err(|e| e.to_string())?;
            ordinal += 1;
            if end == chars.len() {
                break;
            }
            start = end.saturating_sub(CHUNK_OVERLAP);
        }
        tx.execute("INSERT INTO linked_folder_files(folder,root,path,mtime,bytes) VALUES(?1,?2,?3,?4,?5)
                    ON CONFLICT(folder,root,path) DO UPDATE SET mtime=excluded.mtime,bytes=excluded.bytes",
            params![folder.name, root, path_text, mtime, metadata.len() as i64]).map_err(|e| e.to_string())?;
        tx.commit().map_err(|e| e.to_string())?;
    }
    let mut stmt = conn
        .prepare("SELECT path FROM linked_folder_files WHERE folder=?1 AND root=?2")
        .map_err(|e| e.to_string())?;
    let old_paths = stmt
        .query_map(params![folder.name, root], |r| r.get::<_, String>(0))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    drop(stmt);
    for path in old_paths {
        if !seen.contains(&path) {
            conn.execute(
                "DELETE FROM linked_folder_chunks WHERE folder=?1 AND root=?2 AND path=?3",
                params![folder.name, root, path],
            )
            .map_err(|e| e.to_string())?;
            conn.execute(
                "DELETE FROM linked_folder_files WHERE folder=?1 AND root=?2 AND path=?3",
                params![folder.name, root, path],
            )
            .map_err(|e| e.to_string())?;
        }
    }
    let indexed_at = Local::now().to_rfc3339();
    let error = (!errors.is_empty()).then(|| {
        format!(
            "{} files could not be indexed: {}",
            errors.len(),
            errors.into_iter().take(2).collect::<Vec<_>>().join("; ")
        )
    });
    conn.execute("INSERT INTO linked_folder_index_status(folder,root,indexed_at,error) VALUES(?1,?2,?3,?4)
                  ON CONFLICT(folder,root) DO UPDATE SET indexed_at=excluded.indexed_at,error=excluded.error",
        params![folder.name, root, indexed_at, error]).map_err(|e| e.to_string())?;
    status(folder)
}

pub(super) fn status(folder: &LinkedFolder) -> Result<LinkedFolderStatus, String> {
    let conn = db()?;
    let root = root(folder);
    let indexed_files: i64 = conn
        .query_row(
            "SELECT count(*) FROM linked_folder_files WHERE folder=?1 AND root=?2",
            params![folder.name, root],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let state = conn
        .query_row(
            "SELECT indexed_at,error FROM linked_folder_index_status WHERE folder=?1 AND root=?2",
            params![folder.name, root],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(|e| e.to_string())?;
    let (indexed_at, index_error) = state.unwrap_or((None, None));
    let pending_jobs: i64 = conn
        .query_row(
            "SELECT count(*) FROM linked_folder_jobs WHERE status IN ('pending','running')",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let failed_jobs: i64 = conn
        .query_row(
            "SELECT count(*) FROM linked_folder_jobs WHERE status='failed'",
            [],
            |r| r.get(0),
        )
        .map_err(|e| e.to_string())?;
    let last_job_error: Option<String> = conn.query_row("SELECT error FROM linked_folder_jobs WHERE status='failed' ORDER BY created_at DESC LIMIT 1", [], |r| r.get(0)).optional().map_err(|e| e.to_string())?.flatten();
    Ok(LinkedFolderStatus {
        indexed_files: indexed_files as usize,
        indexed_at,
        index_error,
        pending_jobs: pending_jobs as usize,
        failed_jobs: failed_jobs as usize,
        last_job_error,
    })
}

pub(super) fn search(
    folder: &LinkedFolder,
    query: &str,
    limit: usize,
) -> Result<Vec<(String, String)>, String> {
    let conn = db()?;
    let root = root(folder);
    let vector = embedding(query);
    let query_grams = char_trigrams(query);
    let mut stmt = conn
        .prepare("SELECT path,text,embedding FROM linked_folder_chunks WHERE folder=?1 AND root=?2")
        .map_err(|e| e.to_string())?;
    let mut hits = stmt
        .query_map(params![folder.name, root], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, Vec<u8>>(2)?,
            ))
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?
        .into_iter()
        .map(|(path, text, raw)| {
            let score = decode_embedding(&raw)
                .map(|v| cosine_similarity(&vector, &v))
                .unwrap_or(0.0)
                .max(0.0)
                + 0.5 * trigram_overlap(&query_grams, &text);
            let stem = Path::new(&path)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            let name_score = if stem.chars().count() >= 3 && query.to_lowercase().contains(&stem) {
                0.25
            } else {
                0.0
            };
            (score + name_score, path, text)
        })
        .collect::<Vec<_>>();
    hits.retain(|(score, _, _)| *score >= 0.1);
    hits.sort_by(|a, b| b.0.total_cmp(&a.0));
    hits.truncate(limit);
    Ok(hits
        .into_iter()
        .map(|(_, path, text)| (path, text.chars().take(500).collect()))
        .collect())
}

fn char_trigrams(text: &str) -> HashSet<String> {
    let chars = text
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect::<Vec<_>>();
    chars
        .windows(3)
        .map(|window| window.iter().collect())
        .collect()
}

fn trigram_overlap(query_grams: &HashSet<String>, text: &str) -> f32 {
    if query_grams.is_empty() {
        return 0.0;
    }
    let text_grams = char_trigrams(text);
    if text_grams.is_empty() {
        return 0.0;
    }
    let matched = text_grams.intersection(query_grams).count();
    matched as f32 / query_grams.len().min(text_grams.len()) as f32
}

pub(super) fn remove_folder(name: &str) -> Result<(), String> {
    let conn = db()?;
    for table in [
        "linked_folder_chunks",
        "linked_folder_files",
        "linked_folder_index_status",
    ] {
        conn.execute(&format!("DELETE FROM {table} WHERE folder=?1"), [name])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub(super) fn save_note(
    folder: &LinkedFolder,
    content: &str,
    config: &MintConfig,
    job_id: Option<&str>,
) -> Result<LinkedFolderNote, String> {
    let conn = db()?;
    let root = root(folder);
    if let Some(job_id) = job_id
        && let Some(note) = conn.query_row("SELECT id,folder,path,content,created_at,status,error FROM linked_folder_notes WHERE job_id=?1 AND folder=?2 AND root=?3",
            params![job_id, folder.name, root], note_row).optional().map_err(|e| e.to_string())? {
        return write_pending(folder, note, config);
    }
    let now = Local::now();
    let id = Uuid::new_v4().to_string();
    let path = folder
        .path
        .join("mint-notes")
        .join(format!("{}.md", now.format("%Y-%m-%d")));
    assert_path_capability(&path, Capability::Write, config).map_err(|e| e.to_string())?;
    let note = LinkedFolderNote {
        id,
        folder: folder.name.clone(),
        path: path.to_string_lossy().into_owned(),
        content: content.to_owned(),
        created_at: now.to_rfc3339(),
        status: "pending".into(),
        error: None,
    };
    conn.execute("INSERT INTO linked_folder_notes(id,job_id,folder,root,path,content,created_at,status) VALUES(?1,?2,?3,?4,?5,?6,?7,'pending')",
        params![note.id, job_id, note.folder, root, note.path, note.content, note.created_at]).map_err(|e| e.to_string())?;
    write_pending(folder, note, config)
}

fn note_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<LinkedFolderNote> {
    Ok(LinkedFolderNote {
        id: r.get(0)?,
        folder: r.get(1)?,
        path: r.get(2)?,
        content: r.get(3)?,
        created_at: r.get(4)?,
        status: r.get(5)?,
        error: r.get(6)?,
    })
}

fn write_pending(
    folder: &LinkedFolder,
    mut note: LinkedFolderNote,
    config: &MintConfig,
) -> Result<LinkedFolderNote, String> {
    if note.status == "saved" {
        return Ok(note);
    }
    let path = Path::new(&note.path);
    let result = (|| -> Result<(), String> {
        if fs::canonicalize(&folder.path).map_err(|e| e.to_string())? != folder.path {
            return Err("linked folder path has changed".into());
        }
        if !path.starts_with(folder.path.join("mint-notes")) {
            return Err("note path is outside linked folder".into());
        }
        assert_path_capability(path, Capability::Write, config).map_err(|e| e.to_string())?;
        fs::create_dir_all(path.parent().ok_or("missing note directory")?)
            .map_err(|e| e.to_string())?;
        if fs::symlink_metadata(path.parent().ok_or("missing note directory")?)
            .map_err(|e| e.to_string())?
            .file_type()
            .is_symlink()
        {
            return Err("note directory must not be a symlink".into());
        }
        if fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_symlink()) {
            return Err("note file must not be a symlink".into());
        }
        append_note_entry(path, &note)
    })();
    let conn = db()?;
    note.status = if result.is_ok() {
        "saved".into()
    } else {
        "failed".into()
    };
    note.error = result.err();
    conn.execute(
        "UPDATE linked_folder_notes SET status=?2,error=?3 WHERE id=?1",
        params![note.id, note.status, note.error],
    )
    .map_err(|e| e.to_string())?;
    Ok(note)
}

fn append_note_entry(path: &Path, note: &LinkedFolderNote) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .read(true)
        .append(true)
        .open(path)
        .map_err(|e| e.to_string())?;
    file.lock().map_err(|e| e.to_string())?;
    let mut existing = String::new();
    file.read_to_string(&mut existing)
        .map_err(|e| e.to_string())?;
    let marker = format!("<!-- mint-note:{} -->", note.id);
    if !existing.contains(&marker) {
        let entry = format!(
            "\n## {} · {}\n\n{}\n{}\n",
            &note.created_at[11..19],
            note.id,
            note.content,
            marker
        );
        let original_len = file.metadata().map_err(|e| e.to_string())?.len();
        if let Err(error) = file.write_all(entry.as_bytes()) {
            let _ = file.set_len(original_len);
            return Err(error.to_string());
        }
        file.sync_all().map_err(|e| e.to_string())?;
    }
    file.unlock().map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn notes(
    folder: &LinkedFolder,
    config: &MintConfig,
) -> Result<Vec<LinkedFolderNote>, String> {
    assert_path_capability(&folder.path, Capability::Read, config).map_err(|e| e.to_string())?;
    let conn = db()?;
    let root = root(folder);
    let mut stmt = conn.prepare("SELECT id,folder,path,content,created_at,status,error FROM linked_folder_notes WHERE folder=?1 AND root=?2 ORDER BY created_at DESC LIMIT 100").map_err(|e| e.to_string())?;
    let mut notes = stmt
        .query_map(params![folder.name, root], note_row)
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    let mut file_cache: HashMap<String, Vec<(String, String)>> = HashMap::new();
    for note in &mut notes {
        if note.status == "saved" && !Path::new(&note.path).is_file() {
            note.status = "failed".into();
            note.error = Some("note file is missing".into());
        } else if note.status == "saved" && !safe_note_path(folder, Path::new(&note.path), config) {
            note.status = "failed".into();
            note.error = Some("note path is not readable".into());
        } else if note.status == "saved" {
            let sections = file_cache.entry(note.path.clone()).or_insert_with(|| {
                fs::read_to_string(&note.path)
                    .map(|body| super::note_sections(&body))
                    .unwrap_or_default()
            });
            let marker = format!("<!-- mint-note:{} -->", note.id);
            if let Some((_, entry)) = sections.iter().find(|(_, entry)| entry.contains(&marker)) {
                note.content = entry
                    .lines()
                    .filter(|line| !line.contains(&marker))
                    .collect::<Vec<_>>()
                    .join("\n")
                    .trim()
                    .to_owned();
            }
        }
    }
    let notes_dir = folder.path.join("mint-notes");
    if fs::symlink_metadata(&notes_dir).is_ok_and(|meta| !meta.file_type().is_symlink())
        && let Ok(files) = fs::read_dir(&notes_dir)
    {
        for file in files.flatten().take(365) {
            if !file.file_type().is_ok_and(|kind| kind.is_file()) {
                continue;
            }
            let path = file.path();
            if !safe_note_path(folder, &path, config) {
                continue;
            }
            let Some(date) = path.file_stem().and_then(|v| v.to_str()) else {
                continue;
            };
            if path.extension().and_then(|v| v.to_str()) != Some("md") || !valid_date(date) {
                continue;
            }
            let Ok(body) = fs::read_to_string(&path) else {
                continue;
            };
            for (ordinal, (heading, chunk)) in super::note_sections(&body).into_iter().enumerate() {
                if chunk.contains("<!-- mint-note:") {
                    continue;
                }
                let time = heading.split(" · ").next().unwrap_or("00:00");
                let time = if time.len() == 5 { time } else { "00:00" };
                notes.push(LinkedFolderNote {
                    id: format!("legacy:{date}:{ordinal}"),
                    folder: folder.name.clone(),
                    path: path.to_string_lossy().into_owned(),
                    content: chunk
                        .lines()
                        .collect::<Vec<_>>()
                        .join("\n")
                        .trim()
                        .chars()
                        .take(160)
                        .collect(),
                    created_at: format!("{date}T{time}:00"),
                    status: "saved".into(),
                    error: None,
                });
            }
        }
    }
    notes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    notes.truncate(100);
    Ok(notes)
}

fn valid_date(date: &str) -> bool {
    date.len() == 10
        && date.chars().enumerate().all(|(i, c)| {
            if i == 4 || i == 7 {
                c == '-'
            } else {
                c.is_ascii_digit()
            }
        })
}

fn safe_note_path(folder: &LinkedFolder, path: &Path, config: &MintConfig) -> bool {
    let Ok(root) = fs::canonicalize(&folder.path) else {
        return false;
    };
    if root != folder.path {
        return false;
    }
    let Ok(dir) = fs::canonicalize(folder.path.join("mint-notes")) else {
        return false;
    };
    let Ok(file) = fs::canonicalize(path) else {
        return false;
    };
    dir.starts_with(root)
        && file.starts_with(dir)
        && assert_path_capability(&file, Capability::Read, config).is_ok()
}

pub(super) fn read_note(
    folder: &LinkedFolder,
    id: &str,
    config: &MintConfig,
) -> Result<String, String> {
    let (path, legacy_ordinal): (String, Option<usize>) = if let Some(remainder) =
        id.strip_prefix("legacy:")
    {
        let (date, ordinal) = remainder.split_once(':').ok_or("invalid legacy note id")?;
        if !valid_date(date) {
            return Err("invalid legacy note date".into());
        }
        let ordinal: usize = ordinal.parse().map_err(|_| "invalid legacy note id")?;
        let path = folder.path.join("mint-notes").join(format!("{date}.md"));
        (path.to_string_lossy().into_owned(), Some(ordinal))
    } else {
        let conn = db()?;
        let root = root(folder);
        (conn.query_row("SELECT path FROM linked_folder_notes WHERE id=?1 AND folder=?2 AND root=?3 AND status='saved'",
            params![id,folder.name,root], |r| r.get(0)).map_err(|e| e.to_string())?, None)
    };
    let path = Path::new(&path);
    if !path.starts_with(folder.path.join("mint-notes")) {
        return Err("note path is outside linked folder".into());
    }
    let actual_root = fs::canonicalize(&folder.path).map_err(|e| e.to_string())?;
    if actual_root != folder.path {
        return Err("linked folder path has changed".into());
    }
    let actual_dir = fs::canonicalize(folder.path.join("mint-notes")).map_err(|e| e.to_string())?;
    let actual_path = fs::canonicalize(path).map_err(|e| e.to_string())?;
    if !actual_dir.starts_with(&actual_root) || !actual_path.starts_with(&actual_dir) {
        return Err("note path resolves outside linked folder".into());
    }
    assert_path_capability(&actual_path, Capability::Read, config).map_err(|e| e.to_string())?;
    let body = fs::read_to_string(actual_path).map_err(|e| e.to_string())?;
    if let Some(ordinal) = legacy_ordinal
        && super::note_sections(&body)
            .into_iter()
            .nth(ordinal)
            .is_none_or(|(_, entry)| entry.contains("<!-- mint-note:"))
    {
        return Err("legacy note not found".into());
    }
    Ok(body)
}

pub(super) fn queue_job(id: &str, user_text: &str, ai_text: &str) -> Result<(), String> {
    let conn = db()?;
    conn.execute("DELETE FROM linked_folder_jobs WHERE status IN ('saved','skipped') AND unixepoch(created_at) < unixepoch()-2592000", []).map_err(|e| e.to_string())?;
    conn.execute("INSERT OR IGNORE INTO linked_folder_jobs(id,user_text,ai_text,status,created_at) VALUES(?1,?2,?3,'pending',?4)",
        params![id,user_text,ai_text,Local::now().to_rfc3339()]).map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn claim_job() -> Result<Option<ClaimedJob>, ClaimError> {
    claim_job_on(&mut open_db()?)
}

fn claim_job_on(conn: &mut Connection) -> Result<Option<ClaimedJob>, ClaimError> {
    // Acquire the writer slot before reading: a deferred transaction can read
    // an outdated WAL snapshot and fail immediately when upgrading to a writer.
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let job = tx.query_row("SELECT id,user_text,ai_text,attempts FROM linked_folder_jobs WHERE (status='pending' AND (claimed_at IS NULL OR claimed_at <= unixepoch())) OR (status='running' AND claimed_at < unixepoch()-600) ORDER BY created_at LIMIT 1", [],
        |r| Ok((r.get::<_,String>(0)?,r.get::<_,String>(1)?,r.get::<_,String>(2)?,r.get::<_,i64>(3)?))).optional()?;
    if let Some((ref id, _, _, _)) = job {
        tx.execute("UPDATE linked_folder_jobs SET status='running',attempts=attempts+1,claimed_at=unixepoch() WHERE id=?1",[id])?;
    }
    tx.commit()?;
    Ok(job)
}

pub(super) fn finish_job(id: &str, status: &str, error: Option<&str>) -> Result<(), String> {
    db()?
        .execute(
            "UPDATE linked_folder_jobs SET status=?2,error=?3,user_text='',ai_text='' WHERE id=?1",
            params![id, status, error],
        )
        .map_err(|e| e.to_string())?;
    Ok(())
}

pub(super) fn retry_job(id: &str, error: &str, delay_seconds: i64) -> Result<(), String> {
    db()?.execute("UPDATE linked_folder_jobs SET status='pending',error=?2,claimed_at=unixepoch()+?3 WHERE id=?1",
        params![id,error,delay_seconds]).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim_test_db() -> (std::path::PathBuf, Connection) {
        let dir = std::env::temp_dir().join(format!("mint-linked-claim-{}", Uuid::new_v4()));
        let path = dir.join("jobs.sqlite");
        let conn = open_db_at(&path).unwrap();
        conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
        (path, conn)
    }

    fn insert_test_job(conn: &Connection, id: &str) {
        conn.execute("INSERT INTO linked_folder_jobs(id,user_text,ai_text,status,created_at) VALUES(?1,'user','ai','pending','2026-10-02')", [id]).unwrap();
    }

    #[test]
    fn concurrent_claims_take_each_job_once() {
        let (path, conn) = claim_test_db();
        for i in 0..8 {
            insert_test_job(&conn, &format!("job-{i}"));
        }
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let mut workers = Vec::new();
        for _ in 0..8 {
            let mut worker_conn = open_db_at(&path).unwrap();
            let barrier = barrier.clone();
            workers.push(std::thread::spawn(move || {
                barrier.wait();
                claim_job_on(&mut worker_conn).unwrap().unwrap()
            }));
        }
        let jobs: Vec<_> = workers
            .into_iter()
            .map(|worker| worker.join().unwrap())
            .collect();
        assert_eq!(
            jobs.iter().map(|job| &job.0).collect::<HashSet<_>>().len(),
            8
        );
        assert!(jobs.iter().all(|job| job.3 == 0));
        let running: i64 = conn
            .query_row(
                "SELECT count(*) FROM linked_folder_jobs WHERE status='running' AND attempts=1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(running, 8);
        drop(conn);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn claim_waits_for_writer_and_failed_lock_does_not_consume_job_attempt() {
        let (path, mut writer) = claim_test_db();
        insert_test_job(&writer, "job");
        let mut claimant = open_db_at(&path).unwrap();
        claimant.busy_timeout(std::time::Duration::ZERO).unwrap();
        let tx = writer
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let error = claim_job_on(&mut claimant).unwrap_err();
        assert!(error.is_busy());
        let attempts: i64 = tx
            .query_row(
                "SELECT attempts FROM linked_folder_jobs WHERE id='job'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attempts, 0);
        claimant
            .busy_timeout(std::time::Duration::from_secs(5))
            .unwrap();
        let (started_tx, started_rx) = std::sync::mpsc::channel();
        let (result_tx, result_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            result_tx.send(claim_job_on(&mut claimant)).unwrap();
        });
        started_rx.recv().unwrap();
        assert!(matches!(
            result_rx.recv_timeout(std::time::Duration::from_millis(50)),
            Err(std::sync::mpsc::RecvTimeoutError::Timeout)
        ));
        tx.commit().unwrap();
        let job = result_rx
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap()
            .unwrap()
            .unwrap();
        assert_eq!(job.0, "job");
        assert_eq!(job.3, 0);
        worker.join().unwrap();
        assert!(claim_job_on(&mut writer).unwrap().is_none());
        drop(writer);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[tokio::test]
    async fn claim_retry_recovers_after_real_database_lock_is_released() {
        let (path, mut writer) = claim_test_db();
        insert_test_job(&writer, "job");
        let mut claimant = open_db_at(&path).unwrap();
        claimant.busy_timeout(std::time::Duration::ZERO).unwrap();
        let mut lock = Some(
            writer
                .transaction_with_behavior(TransactionBehavior::Immediate)
                .unwrap(),
        );
        let mut calls = 0;
        let job = super::super::claim_job_with_retry(|| {
            calls += 1;
            if calls == 2 {
                lock.take().unwrap().commit().unwrap();
            }
            std::future::ready(claim_job_on(&mut claimant))
        })
        .await
        .unwrap()
        .unwrap();
        assert_eq!(calls, 2);
        assert_eq!(job.0, "job");
        assert_eq!(job.3, 0);
        let attempts: i64 = claimant
            .query_row(
                "SELECT attempts FROM linked_folder_jobs WHERE id='job'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(attempts, 1);
        drop(lock);
        drop(claimant);
        drop(writer);
        fs::remove_dir_all(path.parent().unwrap()).unwrap();
    }

    #[test]
    fn concurrent_appends_keep_each_note_once() {
        let dir = std::env::temp_dir().join(format!("mint-linked-write-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("2026-10-01.md");
        let notes = (0..12)
            .map(|i| LinkedFolderNote {
                id: Uuid::new_v4().to_string(),
                folder: "test".into(),
                path: path.to_string_lossy().into_owned(),
                content: format!("entry {i}"),
                created_at: "2026-10-01T10:15:00+07:00".into(),
                status: "pending".into(),
                error: None,
            })
            .collect::<Vec<_>>();
        std::thread::scope(|scope| {
            for note in &notes {
                let path = &path;
                scope.spawn(move || append_note_entry(path, note).unwrap());
            }
        });
        for note in &notes {
            append_note_entry(&path, note).unwrap();
        }
        let content = fs::read_to_string(&path).unwrap();
        for note in &notes {
            assert_eq!(
                content
                    .matches(&format!("<!-- mint-note:{} -->", note.id))
                    .count(),
                1
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn thai_topic_overlap_beats_unrelated_text() {
        let query = char_trigrams("ผัดไทยใส่มะขามอย่างไร");
        assert!(trigram_overlap(&query, "สูตรผัดไทยใส่มะขามเปียก") > 0.1);
        assert_eq!(trigram_overlap(&query, "ตารางการแข่งขันฟุตบอล"), 0.0);
    }
}

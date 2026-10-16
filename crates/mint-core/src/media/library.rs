//! One catalog for uploaded files and generated media, shared by every surface.
mod intake;
pub use intake::{AttachmentMetadata, AttachmentReceipt, prepare_chat};
mod migration;
mod removal;
mod thumbnails;
use crate::MemoryStore;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct LibraryStore {
    memory: MemoryStore,
    root: PathBuf,
    legacy: PathBuf,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryItem {
    pub id: String,
    pub name: String,
    pub filename: String,
    pub mime_type: String,
    pub kind: String,
    pub size: u64,
    pub created_at: String,
    pub origin: String,
    pub source: String,
    pub message: String,
    pub deleted_at: Option<String>,
    pub thumbnail_status: String,
    pub availability: String,
    pub references: Vec<LibraryReference>,
    #[serde(skip)]
    pub path: PathBuf,
    #[serde(skip)]
    pub thumbnail_path: Option<PathBuf>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryReference {
    pub chat_id: String,
    pub interaction_id: Option<i64>,
}
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryQuery {
    pub kind: Option<String>,
    pub search: Option<String>,
    pub chat_id: Option<String>,
    pub trashed: bool,
    pub cursor: Option<String>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryPage {
    pub items: Vec<LibraryItem>,
    pub next_cursor: Option<String>,
}
#[derive(Clone)]
pub struct LibraryUpload {
    pub name: String,
    pub data_uri: String,
}

#[derive(Debug, thiserror::Error)]
pub enum LibraryError {
    #[error("{0}")]
    Invalid(String),
    #[error(transparent)]
    Memory(#[from] crate::MemoryError),
    #[error(transparent)]
    Sql(#[from] rusqlite::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Config(#[from] crate::ConfigError),
}
type Result<T> = std::result::Result<T, LibraryError>;
fn invalid(message: impl Into<String>) -> LibraryError {
    LibraryError::Invalid(message.into())
}
use base64::{Engine as _, engine::general_purpose::STANDARD};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::{
    fs,
    sync::{Arc, Mutex, OnceLock, mpsc},
};

pub(crate) fn initialize(connection: &rusqlite::Connection) -> rusqlite::Result<()> {
    connection.execute_batch("CREATE TABLE IF NOT EXISTS library_items (
      id TEXT PRIMARY KEY, name TEXT NOT NULL, filename TEXT NOT NULL, mime_type TEXT NOT NULL,
      kind TEXT NOT NULL, size INTEGER NOT NULL, created_at TEXT NOT NULL, origin TEXT NOT NULL,
      source TEXT NOT NULL, message TEXT NOT NULL, path TEXT NOT NULL, thumbnail_path TEXT,
      thumbnail_status TEXT NOT NULL, deleted_at TEXT);
      CREATE INDEX IF NOT EXISTS library_order ON library_items(deleted_at, created_at DESC, id DESC);
      CREATE TABLE IF NOT EXISTS library_references (
      id INTEGER PRIMARY KEY AUTOINCREMENT, item_id TEXT NOT NULL, chat_id TEXT NOT NULL,
      request_id TEXT NOT NULL, slot INTEGER NOT NULL, interaction_id INTEGER,
      UNIQUE(chat_id,request_id,slot));
      CREATE INDEX IF NOT EXISTS library_refs_item ON library_references(item_id);
      CREATE INDEX IF NOT EXISTS library_refs_chat ON library_references(chat_id);
      CREATE TABLE IF NOT EXISTS library_migrations (name TEXT PRIMARY KEY);
      CREATE TRIGGER IF NOT EXISTS trg_library_refs_delete AFTER DELETE ON interaction_memories BEGIN
        DELETE FROM library_references WHERE interaction_id = OLD.id;
      END;")
}

#[derive(Default)]
struct StagedFiles(Vec<PathBuf>);
impl Drop for StagedFiles {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

impl LibraryStore {
    pub fn open(memory: MemoryStore, root: PathBuf, legacy: PathBuf) -> Result<Self> {
        let store = Self {
            memory,
            root,
            legacy,
        };
        store.migrate()?;
        static RESUMED: OnceLock<Mutex<std::collections::HashSet<PathBuf>>> = OnceLock::new();
        let first = RESUMED
            .get_or_init(Default::default)
            .lock()
            .unwrap()
            .insert(store.root.clone());
        if first {
            let connection = store.memory.connection()?;
            let mut statement = connection
                .prepare("SELECT id FROM library_items WHERE thumbnail_status='pending'")?;
            for id in statement.query_map([], |row| row.get::<_, String>(0))? {
                store.enqueue_thumbnail(id?);
            }
        }
        Ok(store)
    }
    pub fn open_default() -> Result<Self> {
        Self::for_memory(MemoryStore::open_default()?)
    }
    pub(crate) fn for_memory(memory: MemoryStore) -> Result<Self> {
        let base = if memory.path() == crate::memory_path()? {
            crate::config_path()?
        } else {
            memory.path().to_path_buf()
        };
        Self::open(
            memory,
            base.with_file_name("Library"),
            base.with_file_name("Pictures"),
        )
    }
    pub fn list(&self, query: &LibraryQuery) -> Result<LibraryPage> {
        if query
            .kind
            .as_deref()
            .is_some_and(|kind| !matches!(kind, "image" | "video" | "file"))
        {
            return Err(invalid("Invalid Library kind"));
        }
        let cursor: Option<(String, String)> = query
            .cursor
            .as_ref()
            .map(|cursor| {
                STANDARD
                    .decode(cursor)
                    .map_err(|_| invalid("Invalid Library cursor"))
                    .and_then(|bytes| serde_json::from_slice(&bytes).map_err(Into::into))
            })
            .transpose()?;
        let search = query
            .search
            .as_deref()
            .unwrap_or("")
            .trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_");
        let connection = self.memory.connection()?;
        let mut statement = connection.prepare("SELECT id FROM library_items i WHERE
          (?1 IS NULL OR kind=?1) AND name LIKE ?2 ESCAPE '\\' AND
          ((?3=1 AND deleted_at IS NOT NULL) OR (?3=0 AND deleted_at IS NULL)) AND
          (?4 IS NULL OR EXISTS(SELECT 1 FROM library_references r WHERE r.item_id=i.id AND r.chat_id=?4)) AND
          (?5 IS NULL OR (created_at,id) < (?5,?6)) ORDER BY created_at DESC,id DESC LIMIT 41")?;
        let ids = statement
            .query_map(
                params![
                    query.kind,
                    format!("%{search}%"),
                    query.trashed,
                    query.chat_id,
                    cursor.as_ref().map(|c| &c.0),
                    cursor.as_ref().map(|c| &c.1)
                ],
                |row| row.get::<_, String>(0),
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut items = ids
            .iter()
            .map(|id| self.get(id))
            .collect::<Result<Vec<_>>>()?;
        let more = items.len() > 40;
        items.truncate(40);
        let next_cursor = if more {
            items.last().map(|item| {
                STANDARD.encode(serde_json::to_vec(&(&item.created_at, &item.id)).unwrap())
            })
        } else {
            None
        };
        Ok(LibraryPage { items, next_cursor })
    }
    pub fn import(
        &self,
        files: &[LibraryUpload],
        request_id: &str,
        chat_id: Option<&str>,
        origin: &str,
        source: &str,
        message: &str,
    ) -> Result<Vec<LibraryItem>> {
        if request_id.is_empty() || request_id.len() > 256 || files.len() > 64 {
            return Err(invalid("Invalid attachment request"));
        }
        let decoded = files
            .iter()
            .map(decode_upload)
            .collect::<Result<Vec<_>>>()?;
        let total: usize = decoded.iter().map(|(_, _, bytes)| bytes.len()).sum();
        if total > 32 * 1024 * 1024 {
            return Err(invalid("Attachments exceed the 32 MiB request limit"));
        }
        if files.is_empty() {
            return Ok(vec![]);
        }
        // Flush originals before taking SQLite's writer lock. A slow disk must not
        // consume every concurrent import's database busy timeout.
        let mut staged = StagedFiles::default();
        let directory = self.root.join("files");
        fs::create_dir_all(&directory)?;
        let mut originals = Vec::with_capacity(files.len());
        for (file, (_, _, bytes)) in files.iter().zip(&decoded) {
            let name = file
                .name
                .replace('\\', "/")
                .rsplit('/')
                .next()
                .unwrap_or("")
                .trim()
                .to_owned();
            let extension = Path::new(&name)
                .extension()
                .and_then(|s| s.to_str())
                .filter(|s| s.len() <= 12 && s.chars().all(|c| c.is_ascii_alphanumeric()))
                .unwrap_or("bin")
                .to_ascii_lowercase();
            let id = uuid::Uuid::new_v4().to_string();
            let filename = format!("{id}.{extension}");
            let temporary = directory.join(format!("{id}.staging"));
            let path = directory.join(&filename);
            staged.0.push(temporary.clone());
            {
                use std::io::Write;
                let mut file = fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&temporary)?;
                file.write_all(bytes)?;
                file.sync_all()?;
            }
            fs::rename(&temporary, &path)?;
            staged.0.push(path.clone());
            originals.push((id, name, filename, path));
        }
        let chat_id = chat_id.unwrap_or("");
        let mut connection = self.memory.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let existing: usize = tx.query_row(
            "SELECT COUNT(*) FROM library_references WHERE chat_id=?1 AND request_id=?2",
            params![chat_id, request_id],
            |row| row.get(0),
        )?;
        if existing > 0 && existing != files.len() {
            return Err(invalid(
                "Submission ID was already used for different attachments",
            ));
        }
        let mut ids = vec![];
        let mut committed_paths = std::collections::HashSet::new();
        for (slot, ((id, name, filename, path), (mime, kind, bytes))) in
            originals.iter().zip(&decoded).enumerate()
        {
            if let Some(existing_id) = tx.query_row("SELECT item_id FROM library_references WHERE chat_id=?1 AND request_id=?2 AND slot=?3", params![chat_id,request_id,slot], |row|row.get::<_,String>(0)).optional()? {
                let item = self.get(&existing_id)?;
                if item.name != *name || item.mime_type != *mime || self.read(&existing_id, false)?.1 != *bytes {
                    return Err(invalid("Submission ID was already used for different attachments"));
                }
                ids.push(existing_id);
                continue;
            }
            let created = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            tx.execute("INSERT INTO library_items (id,name,filename,mime_type,kind,size,created_at,origin,source,message,path,thumbnail_status) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",params![id,name,filename,mime,kind,bytes.len(),created,origin,source,message.chars().take(240).collect::<String>(),path.to_string_lossy(),if kind=="file" {"unavailable"}else{"pending"}])?;
            tx.execute("INSERT INTO library_references(item_id,chat_id,request_id,slot) VALUES(?1,?2,?3,?4)",params![id,chat_id,request_id,slot])?;
            committed_paths.insert(path.clone());
            ids.push(id.clone());
        }
        tx.commit()?;
        // Retain committed originals; the guard removes temporary and duplicate
        // retry files, and cleans the entire batch on any earlier failure.
        staged.0.retain(|path| !committed_paths.contains(path));
        drop(staged);
        for id in &ids {
            if self.get(id)?.thumbnail_status == "pending" {
                self.enqueue_thumbnail(id.clone());
            }
        }
        ids.iter().map(|id| self.get(id)).collect()
    }
    pub fn get(&self, id: &str) -> Result<LibraryItem> {
        let connection = self.memory.connection()?;
        let mut item=connection.query_row("SELECT id,name,filename,mime_type,kind,size,created_at,origin,source,message,deleted_at,thumbnail_status,path,thumbnail_path FROM library_items WHERE id=?1",[id],|row|Ok(LibraryItem {
            id:row.get(0)?,name:row.get(1)?,filename:row.get(2)?,mime_type:row.get(3)?,kind:row.get(4)?,size:row.get(5)?,created_at:row.get(6)?,origin:row.get(7)?,source:row.get(8)?,message:row.get(9)?,deleted_at:row.get(10)?,thumbnail_status:row.get(11)?,path:PathBuf::from(row.get::<_,String>(12)?),thumbnail_path:row.get::<_,Option<String>>(13)?.map(PathBuf::from),availability:String::new(),references:vec![],
        })).optional()?.ok_or_else(||invalid("Library item not found"))?;
        item.availability = if self.managed_path(&item.path).is_ok() {
            "ready"
        } else {
            "missing"
        }
        .into();
        let mut statement=connection.prepare("SELECT DISTINCT chat_id,interaction_id FROM library_references WHERE item_id=?1 AND chat_id!='' ORDER BY id")?;
        item.references = statement
            .query_map([id], |row| {
                Ok(LibraryReference {
                    chat_id: row.get(0)?,
                    interaction_id: row.get(1)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(item)
    }
    fn managed_path(&self, path: &Path) -> Result<PathBuf> {
        let canonical = path.canonicalize()?;
        let allowed = [&self.root, &self.legacy].iter().any(|root| {
            root.canonicalize()
                .is_ok_and(|root| canonical.starts_with(root))
        });
        if !allowed || !canonical.is_file() {
            return Err(invalid("File is outside the Library"));
        }
        Ok(canonical)
    }
    pub fn set_trashed(&self, id: &str, trashed: bool) -> Result<()> {
        let connection = self.memory.connection()?;
        let deleted = trashed
            .then(|| chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true));
        if connection.execute(
            "UPDATE library_items SET deleted_at=?2 WHERE id=?1",
            params![id, deleted],
        )? == 0
        {
            return Err(invalid("Library item not found"));
        }
        Ok(())
    }
    pub fn read(&self, id: &str, thumbnail: bool) -> Result<(LibraryItem, Vec<u8>)> {
        let item = self.get(id)?;
        if item.deleted_at.is_some() {
            return Err(invalid(
                "This file is in Trash. Restore it from Library first.",
            ));
        }
        let path = if thumbnail {
            item.thumbnail_path
                .as_deref()
                .ok_or_else(|| invalid("Thumbnail unavailable"))?
        } else {
            &item.path
        };
        let bytes = fs::read(self.managed_path(path)?)?;
        Ok((item, bytes))
    }
    /// Associate generated originals with the running turn, never a newer queued prompt.
    pub fn link_generated(&self, ids: &[String], chat_id: &str) -> Result<()> {
        let mut connection = self.memory.connection()?;
        let tx = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let turn:Option<i64>=tx.query_row("SELECT id FROM interaction_memories WHERE chat_id=?1 AND status='running' ORDER BY id LIMIT 1",[chat_id],|row|row.get(0)).optional()?;
        for id in ids {
            let exists: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM library_items WHERE id=?1)",
                [id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(invalid("Generated Library item not found"));
            }
            tx.execute("INSERT OR IGNORE INTO library_references(item_id,chat_id,request_id,slot,interaction_id) VALUES(?1,?2,?3,0,?4)",params![id,chat_id,format!("generated:{id}"),turn])?;
        }
        tx.commit()?;
        Ok(())
    }
    pub fn bind_request(&self, chat_id: &str, request_id: &str, interaction_id: i64) -> Result<()> {
        let connection = self.memory.connection()?;
        let valid: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM interaction_memories WHERE id=?1 AND chat_id=?2)",
            params![interaction_id, chat_id],
            |row| row.get(0),
        )?;
        if !valid {
            return Err(invalid("Interaction does not belong to this conversation"));
        }
        connection.execute("UPDATE library_references SET interaction_id=?3 WHERE chat_id=?1 AND request_id=?2 AND interaction_id IS NULL",params![chat_id,request_id,interaction_id])?;
        Ok(())
    }
    pub(crate) fn session_attachments(
        &self,
        chat_id: &str,
    ) -> Result<Vec<crate::agent::memory::SessionAttachment>> {
        let connection = self.memory.connection()?;
        let mut statement = connection.prepare(
            "SELECT id,item_id,interaction_id FROM library_references WHERE chat_id=?1 ORDER BY id",
        )?;
        let rows = statement
            .query_map([chat_id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<i64>>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        rows.into_iter()
            .map(|(id, item_id, interaction_id)| {
                let item = self.get(&item_id)?;
                let thumbnail_data_uri =
                    if item.deleted_at.is_none() && item.thumbnail_status == "ready" {
                        self.read(&item_id, true).ok().map(|(_, bytes)| {
                            format!("data:image/png;base64,{}", STANDARD.encode(bytes))
                        })
                    } else {
                        None
                    };
                Ok(crate::agent::memory::SessionAttachment {
                    id: -id,
                    interaction_id,
                    library_item_id: Some(item_id),
                    availability: Some(item.availability),
                    trashed: item.deleted_at.is_some(),
                    attachment: crate::agent::memory::SourceAttachment {
                        name: item.name,
                        kind: item.kind,
                        mime_type: item.mime_type,
                        thumbnail_data_uri,
                    },
                })
            })
            .collect()
    }
    pub(crate) fn all_media(&self) -> Result<Vec<crate::PictureEntry>> {
        let mut result = vec![];
        for kind in ["image", "video"] {
            let mut query = LibraryQuery {
                kind: Some(kind.into()),
                ..Default::default()
            };
            loop {
                let page = self.list(&query)?;
                result.extend(page.items.into_iter().map(picture_entry));
                if page.next_cursor.is_none() {
                    break;
                }
                query.cursor = page.next_cursor;
            }
        }
        result.sort_by(|a, b| {
            b.created_at
                .cmp(&a.created_at)
                .then_with(|| b.id.cmp(&a.id))
        });
        Ok(result)
    }
}

fn decode_upload(file: &LibraryUpload) -> Result<(String, String, Vec<u8>)> {
    if file.name.trim().is_empty()
        || file.name.len() > 1024
        || file.data_uri.len() > 46 * 1024 * 1024
    {
        return Err(invalid("Invalid attachment"));
    }
    let (mime, encoded) = file
        .data_uri
        .strip_prefix("data:")
        .and_then(|data| data.split_once(";base64,"))
        .ok_or_else(|| invalid("Invalid attachment data URI"))?;
    let mime = mime.to_ascii_lowercase();
    let kind = if mime.starts_with("image/") {
        "image"
    } else if mime.starts_with("video/") {
        "video"
    } else if crate::knowledge::supported(Path::new(&file.name)) {
        "file"
    } else {
        return Err(invalid("Unsupported attachment type"));
    };
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| invalid("Invalid attachment encoding"))?;
    let limit = match kind {
        "file" => 10 * 1024 * 1024,
        "video" => 25 * 1024 * 1024,
        _ => 32 * 1024 * 1024,
    };
    if bytes.is_empty() || bytes.len() > limit {
        return Err(invalid(format!(
            "Attachment is empty or exceeds the {} MiB limit",
            limit / (1024 * 1024)
        )));
    }
    Ok((mime, kind.into(), bytes))
}

pub(crate) fn picture_entry(item: LibraryItem) -> crate::PictureEntry {
    crate::PictureEntry {
        id: item.id,
        filename: item.filename,
        path: item.path,
        mime_type: item.mime_type,
        created_at: item.created_at,
        source: item.source,
        message: item.message,
        thumbnail_path: item.thumbnail_path,
        url: None,
        thumbnail_url: None,
    }
}

#[cfg(test)]
mod tests;

//! Persistent, conflict-aware workspace undo and pre-edit backups.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

static HISTORY_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

#[derive(Debug)]
struct HistoryGuard {
    _thread: std::sync::MutexGuard<'static, ()>,
    _process: rusqlite::Connection,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub id: String,
    pub label: String,
    pub path: String,
    pub kind: String,
    pub destination: Option<String>,
    pub expected: Option<String>,
    pub backup: Option<String>,
    #[serde(default)]
    pub trash_id: Option<String>,
}

#[derive(Debug)]
pub struct PendingEdit {
    root: PathBuf,
    entry: HistoryEntry,
    _guard: HistoryGuard,
}

fn lock(root: &Path) -> Result<HistoryGuard, String> {
    let thread = HISTORY_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .map_err(|e| e.to_string())?;
    let dir = storage(root)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let process = rusqlite::Connection::open(dir.join("lock.sqlite")).map_err(|e| e.to_string())?;
    process
        .busy_timeout(std::time::Duration::from_secs(10))
        .map_err(|e| e.to_string())?;
    process
        .execute_batch("BEGIN IMMEDIATE")
        .map_err(|e| e.to_string())?;
    Ok(HistoryGuard {
        _thread: thread,
        _process: process,
    })
}

fn storage(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let key = format!("{:x}", Sha256::digest(root.to_string_lossy().as_bytes()));
    Ok(dirs::config_dir()
        .ok_or("No configuration directory")?
        .join("mint/workspace-history")
        .join(key))
}

fn records(root: &Path) -> Result<Vec<HistoryEntry>, String> {
    let path = storage(root)?.join("history.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    serde_json::from_slice(&fs::read(path).map_err(|e| e.to_string())?).map_err(|e| e.to_string())
}

fn save(root: &Path, entries: &[HistoryEntry]) -> Result<(), String> {
    let dir = storage(root)?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let temp = dir.join(format!("history-{}.tmp", uuid::Uuid::new_v4()));
    let mut file = fs::File::create(&temp).map_err(|e| e.to_string())?;
    file.write_all(&serde_json::to_vec_pretty(entries).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    fs::rename(temp, dir.join("history.json")).map_err(|e| e.to_string())
}

fn append(root: &Path, entry: HistoryEntry) -> Result<(), String> {
    let mut entries = records(root)?;
    entries.push(entry);
    save(root, &entries)
}

fn relative(root: &Path, path: &Path) -> Result<String, String> {
    path.strip_prefix(root)
        .map_err(|e| e.to_string())?
        .to_str()
        .map(str::to_owned)
        .ok_or("Path is not valid UTF-8".into())
}

fn fingerprint(path: &Path) -> Result<Option<String>, String> {
    if !path.exists() && !path.is_symlink() {
        return Ok(None);
    }
    let mut hasher = Sha256::new();
    hash_path(path, &mut hasher)?;
    Ok(Some(format!("{:x}", hasher.finalize())))
}

fn hash_path(path: &Path, hash: &mut Sha256) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| e.to_string())?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        hash.update(meta.permissions().mode().to_le_bytes());
    }
    if meta.file_type().is_symlink() {
        hash.update(b"link\0");
        hash.update(
            fs::read_link(path)
                .map_err(|e| e.to_string())?
                .to_string_lossy()
                .as_bytes(),
        );
    } else if meta.is_file() {
        hash.update(b"file\0");
        hash.update(meta.len().to_le_bytes());
        let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut buffer = [0u8; 65536];
        loop {
            let count = file.read(&mut buffer).map_err(|e| e.to_string())?;
            if count == 0 {
                break;
            }
            hash.update(&buffer[..count]);
        }
    } else if meta.is_dir() {
        hash.update(b"dir\0");
        let mut children = fs::read_dir(path)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|v| v.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        children.sort();
        hash.update((children.len() as u64).to_le_bytes());
        for child in children {
            let name = child.file_name().unwrap().to_string_lossy();
            hash.update((name.len() as u64).to_le_bytes());
            hash.update(name.as_bytes());
            hash_path(&child, hash)?;
        }
    } else {
        return Err("Unsupported workspace item".into());
    }
    Ok(())
}

fn copy_item(source: &Path, destination: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(source).map_err(|e| e.to_string())?;
    if meta.file_type().is_symlink() {
        #[cfg(unix)]
        std::os::unix::fs::symlink(
            fs::read_link(source).map_err(|e| e.to_string())?,
            destination,
        )
        .map_err(|e| e.to_string())?;
        #[cfg(not(unix))]
        return Err("Cannot back up a symbolic link on this platform".into());
    } else if meta.is_dir() {
        fs::create_dir(destination).map_err(|e| e.to_string())?;
        for child in fs::read_dir(source).map_err(|e| e.to_string())? {
            let child = child.map_err(|e| e.to_string())?;
            copy_item(&child.path(), &destination.join(child.file_name()))?;
        }
        fs::set_permissions(destination, meta.permissions()).map_err(|e| e.to_string())?;
    } else if meta.is_file() {
        let mut input = fs::File::open(source).map_err(|e| e.to_string())?;
        let mut output = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(|e| e.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|e| e.to_string())?;
        output.sync_all().map_err(|e| e.to_string())?;
        fs::set_permissions(destination, meta.permissions()).map_err(|e| e.to_string())?;
    } else {
        return Err("Cannot back up this item type".into());
    }
    Ok(())
}

fn backup(root: &Path, path: &Path, id: &str) -> Result<Option<String>, String> {
    let before = fingerprint(path)?;
    if before.is_none() {
        return Ok(None);
    }
    let dir = storage(root)?.join("backups");
    if dir.starts_with(path) {
        return Err("Cannot back up a folder containing Mint's recovery storage".into());
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    copy_item(path, &dir.join(id))?;
    if fingerprint(&dir.join(id))? != before || fingerprint(path)? != before {
        return Err("Item changed while its backup was being saved; retry the action".into());
    }
    Ok(Some(id.to_owned()))
}

pub fn begin_edit(root: &Path, path: &Path) -> Result<PendingEdit, String> {
    let guard = lock(root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let path_name = relative(&root, path)?;
    super::workspace::resolve(&root, &path_name)?;
    save(&root, &records(&root)?)?;
    let previous = backup(&root, path, &id)?;
    let label = if previous.is_some() {
        format!("Undo Edit: {path_name}")
    } else {
        format!("Undo Create: {path_name}")
    };
    let entry = HistoryEntry {
        id,
        label,
        path: path_name,
        kind: "edit".into(),
        destination: None,
        expected: None,
        backup: previous,
        trash_id: None,
    };
    // Keep the source path next to the pre-edit copy even if a later write or
    // history commit fails. Recovery copies can always be identified.
    let manifest = storage(&root)?
        .join("backups")
        .join(format!("{}.json", entry.id));
    fs::create_dir_all(manifest.parent().unwrap()).map_err(|e| e.to_string())?;
    fs::write(
        manifest,
        serde_json::to_vec_pretty(&entry).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(PendingEdit {
        root,
        _guard: guard,
        entry,
    })
}

impl PendingEdit {
    pub fn commit(mut self) -> Result<(), String> {
        self.entry.expected = fingerprint(&self.root.join(&self.entry.path))?;
        append(&self.root, self.entry)
    }
}

pub fn create(root: &Path, path: &Path, directory: bool) -> Result<(), String> {
    let _guard = lock(root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let name = relative(&root, path)?;
    if fingerprint(path)?.is_some() {
        return Err("Item already exists".into());
    }
    save(&root, &records(&root)?)?;
    if directory {
        fs::create_dir_all(path).map_err(|e| e.to_string())?;
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?;
    }
    append(
        &root,
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            label: format!("Undo Create: {name}"),
            path: name,
            kind: "create".into(),
            destination: None,
            expected: fingerprint(path)?,
            backup: None,
            trash_id: None,
        },
    )
}

pub fn record_trash(root: &Path, path: &Path) -> Result<(), String> {
    let _guard = lock(root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let id = uuid::Uuid::new_v4().to_string();
    let name = relative(&root, path)?;
    save(&root, &records(&root)?)?;
    let copy = backup(&root, path, &id)?;
    if copy.is_none() {
        return Err("Item no longer exists".into());
    }
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let previous_trash = trash::os_limited::list().unwrap_or_default();
    // A successful trash operation leaves no source path. The backup is kept even
    // if the system Trash is later emptied.
    trash::delete(path).map_err(|e| format!("Failed to move item to Trash: {e}"))?;
    #[cfg(any(target_os = "linux", target_os = "windows"))]
    let trash_id = trash::os_limited::list()
        .unwrap_or_default()
        .into_iter()
        .find(|item| {
            item.original_path() == path && !previous_trash.iter().any(|old| old.id == item.id)
        })
        .map(|item| item.id.to_string_lossy().to_string());
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    let trash_id = None;
    append(
        &root,
        HistoryEntry {
            id,
            label: format!("Undo Move to Trash: {name}"),
            path: name,
            kind: "trash".into(),
            destination: None,
            expected: None,
            backup: copy,
            trash_id,
        },
    )
}

pub fn move_item(root: &Path, source: &Path, destination: &Path) -> Result<(), String> {
    let _guard = lock(root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if fingerprint(source)?.is_none() || fingerprint(destination)?.is_some() {
        return Err("Source is missing or destination already exists".into());
    }
    if source.is_dir() && destination.starts_with(source) {
        return Err("Cannot move a folder into itself".into());
    }
    let from = relative(&root, source)?;
    let to = relative(&root, destination)?;
    let action = if source.parent() == destination.parent() {
        "Rename"
    } else {
        "Move"
    };
    if !destination.parent().is_some_and(Path::is_dir) {
        return Err("Destination folder does not exist".into());
    }
    let expected = fingerprint(source)?;
    save(&root, &records(&root)?)?;
    move_without_replacing(source, destination)?;
    append(
        &root,
        HistoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            label: format!("Undo {action}: {from} → {to}"),
            path: from,
            kind: "move".into(),
            destination: Some(to),
            expected,
            backup: None,
            trash_id: None,
        },
    )
}

pub fn list(root: &Path) -> Result<Vec<HistoryEntry>, String> {
    let _guard = lock(root)?;
    let mut entries = records(root)?;
    entries.reverse();
    Ok(entries)
}

pub fn undo(root: &Path, expected_id: Option<&str>) -> Result<Option<HistoryEntry>, String> {
    let _guard = lock(root)?;
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let mut entries = records(&root)?;
    let Some(entry) = entries.last().cloned() else {
        return Ok(None);
    };
    if expected_id.is_some_and(|id| id != entry.id) {
        return Err("Workspace history changed; refresh before undoing".into());
    }
    let source = super::workspace::resolve(&root, &entry.path)?;
    let current = match entry.kind.as_str() {
        "move" => {
            let dest = entry
                .destination
                .as_ref()
                .ok_or("Missing move destination")?;
            super::workspace::resolve(&root, dest)?
        }
        _ => source.clone(),
    };
    if fingerprint(&current)? != entry.expected {
        return Err(format!(
            "Cannot undo: {} changed since this action",
            current.display()
        ));
    }
    match entry.kind.as_str() {
        "move" => {
            if fingerprint(&source)?.is_some() {
                return Err("Cannot undo: original path is occupied".into());
            }
            move_without_replacing(&current, &source)?;
        }
        "trash" => {
            if fingerprint(&source)?.is_some() {
                return Err("Cannot restore: original path is occupied".into());
            }
            let id = entry.backup.as_ref().ok_or("No backup for trashed item")?;
            let backup_path = storage(&root)?.join("backups").join(id);
            if !backup_path.exists() {
                return Err("Undo backup is missing".into());
            }
            let mut restored = false;
            #[cfg(any(target_os = "linux", target_os = "windows"))]
            if let Some(trash_id) = &entry.trash_id {
                if let Some(item) = trash::os_limited::list()
                    .map_err(|e| e.to_string())?
                    .into_iter()
                    .find(|item| item.id.to_string_lossy() == trash_id.as_str())
                {
                    trash::os_limited::restore_all([item]).map_err(|e| e.to_string())?;
                    restored = true;
                }
            }
            if !restored {
                copy_item(&backup_path, &source)?;
            }
        }
        "create" => {
            // Undoing creation is itself recoverable from system Trash.
            trash::delete(&source).map_err(|e| e.to_string())?;
        }
        "edit" => {
            if let Some(id) = &entry.backup {
                let backup_path = storage(&root)?.join("backups").join(id);
                let temporary =
                    source.with_file_name(format!(".mint-undo-{}", uuid::Uuid::new_v4()));
                copy_item(&backup_path, &temporary)?;
                fs::rename(&temporary, &source).map_err(|e| e.to_string())?;
            } else if source.exists() {
                trash::delete(&source).map_err(|e| e.to_string())?;
            }
        }
        _ => return Err("Unknown history action".into()),
    }
    entries.pop();
    save(&root, &entries)?;
    Ok(Some(entry))
}

fn move_without_replacing(source: &Path, destination: &Path) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        use std::{ffi::CString, os::unix::ffi::OsStrExt};
        let source = CString::new(source.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        let destination =
            CString::new(destination.as_os_str().as_bytes()).map_err(|e| e.to_string())?;
        // Both C strings live across the call. NOREPLACE makes destination
        // collisions safe even if another process creates one after our check.
        let result = unsafe {
            libc::renameat2(
                libc::AT_FDCWD,
                source.as_ptr(),
                libc::AT_FDCWD,
                destination.as_ptr(),
                libc::RENAME_NOREPLACE,
            )
        };
        if result != 0 {
            return Err(std::io::Error::last_os_error().to_string());
        }
        Ok(())
    }
    #[cfg(not(target_os = "linux"))]
    {
        if destination.exists() || destination.is_symlink() {
            return Err("Destination already exists".into());
        }
        fs::rename(source, destination).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> PathBuf {
        let root = std::env::temp_dir().join(format!("mint-undo-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&root).unwrap();
        root.canonicalize().unwrap()
    }

    #[test]
    fn non_git_ai_edits_are_persisted_and_refuse_to_overwrite_later_work() {
        use crate::agent::code_tools::{CodeEdit, apply_code_edits, propose_code_edits};
        let root = root();
        let path = root.join("index.html");
        fs::write(&path, "original page").unwrap();
        let mut config = crate::MintConfig::default();
        config.allowed_read_paths = vec![root.clone()];
        config.allowed_write_paths = vec![root.clone()];
        config.blocked_paths.clear();
        let edit = CodeEdit {
            path: path.clone(),
            content: "AI page".into(),
        };
        let proposal = propose_code_edits(&root, std::slice::from_ref(&edit), &config).unwrap();
        apply_code_edits(&root, &[edit], &proposal.approval_token, &config).unwrap();
        assert!(!root.join(".git").exists());
        let entries = list(&root).unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].label, "Undo Edit: index.html");
        assert!(storage(&root).unwrap().join("history.json").exists());
        fs::write(&path, "new user work").unwrap();
        assert!(
            undo(&root, Some(&entries[0].id))
                .unwrap_err()
                .contains("changed")
        );
        assert_eq!(fs::read_to_string(&path).unwrap(), "new user work");
        assert_eq!(list(&root).unwrap().len(), 1);
        fs::write(&path, "AI page").unwrap();
        undo(&root, Some(&entries[0].id)).unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "original page");
        assert!(list(&root).unwrap().is_empty());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn move_and_rename_undo_protect_occupied_paths_and_stale_labels() {
        let root = root();
        let source = root.join("old.txt");
        let renamed = root.join("new.txt");
        fs::write(&source, "original").unwrap();
        move_item(&root, &source, &renamed).unwrap();
        let entry = list(&root).unwrap().remove(0);
        assert!(entry.label.starts_with("Undo Rename:"));
        assert!(undo(&root, Some("stale-entry")).is_err());
        fs::write(&source, "another file").unwrap();
        assert!(
            undo(&root, Some(&entry.id))
                .unwrap_err()
                .contains("occupied")
        );
        assert_eq!(fs::read_to_string(&source).unwrap(), "another file");
        assert_eq!(fs::read_to_string(&renamed).unwrap(), "original");
        fs::remove_file(&source).unwrap();
        undo(&root, Some(&entry.id)).unwrap();
        assert_eq!(fs::read_to_string(&source).unwrap(), "original");
        fs::create_dir(root.join("folder")).unwrap();
        move_item(&root, &source, &root.join("folder/old.txt")).unwrap();
        assert!(list(&root).unwrap()[0].label.starts_with("Undo Move:"));
        undo(&root, None).unwrap();
        assert_eq!(fs::read_to_string(&source).unwrap(), "original");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn undo_folder_creation_does_not_remove_new_contents() {
        let root = root();
        let folder = root.join("created");
        create(&root, &folder, true).unwrap();
        fs::write(folder.join("user-work.txt"), "keep").unwrap();
        assert!(undo(&root, None).unwrap_err().contains("changed"));
        assert_eq!(
            fs::read_to_string(folder.join("user-work.txt")).unwrap(),
            "keep"
        );
        assert!(create(&root, &folder, true).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn undo_trash_restores_nested_contents_and_protects_a_replacement() {
        let root = root();
        let folder = root.join("notes");
        fs::create_dir(&folder).unwrap();
        fs::write(folder.join("todo.txt"), "remember me").unwrap();
        record_trash(&root, &folder).unwrap();
        let entry = list(&root).unwrap().remove(0);
        assert!(!folder.exists());
        assert!(entry.trash_id.is_some());
        fs::write(&folder, "replacement").unwrap();
        assert!(undo(&root, None).is_err());
        assert_eq!(fs::read_to_string(&folder).unwrap(), "replacement");
        fs::remove_file(&folder).unwrap();
        undo(&root, Some(&entry.id)).unwrap();
        assert_eq!(
            fs::read_to_string(folder.join("todo.txt")).unwrap(),
            "remember me"
        );
        assert!(
            !trash::os_limited::list()
                .unwrap()
                .iter()
                .any(|item| Some(item.id.to_string_lossy().to_string()) == entry.trash_id)
        );
        fs::remove_dir_all(root).unwrap();
    }
}

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

const MAX_DEPTH: usize = 9;
const MAX_CHILDREN: usize = 400;
const COLLAPSED: &[&str] = &[
    ".git",
    "node_modules",
    "target",
    "dist",
    "out",
    "build",
    "coverage",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceTreeEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub children: Vec<WorkspaceTreeEntry>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceSnapshot {
    pub path: String,
    pub tree: WorkspaceTreeEntry,
    pub git: crate::git::BranchInfo,
    pub revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceOperation {
    pub root: PathBuf,
    pub relative_path: String,
    pub revision: u64,
}

pub fn snapshot(operation: &WorkspaceOperation) -> Result<WorkspaceSnapshot, String> {
    let root = canonical_root(&operation.root)?;
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| root.display().to_string());
    Ok(WorkspaceSnapshot {
        path: root.display().to_string(),
        tree: WorkspaceTreeEntry {
            name,
            path: ".".into(),
            kind: "directory".into(),
            children: children(&root, &root, 0)?,
        },
        git: crate::git::read_branch_info(&root)?,
        revision: operation.revision,
    })
}
pub fn resolve(root: &Path, relative: &str) -> Result<PathBuf, String> {
    let root = canonical_root(root)?;
    let relative = Path::new(relative);
    if relative.as_os_str().is_empty() || relative.is_absolute() {
        return Err("workspace path must be relative".into());
    }
    if relative
        .components()
        .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err("workspace path escapes root".into());
    }

    let candidate = root.join(relative);
    let mut existing = candidate.as_path();
    while !existing.exists() {
        existing = existing
            .parent()
            .ok_or_else(|| "workspace path escapes root".to_string())?;
    }
    let existing = existing.canonicalize().map_err(|e| e.to_string())?;
    if !existing.starts_with(&root) {
        return Err("workspace path escapes root".into());
    }
    Ok(candidate)
}
pub fn create_file(operation: &WorkspaceOperation) -> Result<WorkspaceSnapshot, String> {
    let path = resolve(&operation.root, &operation.relative_path)?;
    super::workspace_history::create(&operation.root, &path, false)?;
    snapshot(&WorkspaceOperation {
        revision: operation.revision + 1,
        ..operation.clone()
    })
}
pub fn create_folder(operation: &WorkspaceOperation) -> Result<WorkspaceSnapshot, String> {
    let path = resolve(&operation.root, &operation.relative_path)?;
    super::workspace_history::create(&operation.root, &path, true)?;
    snapshot(&WorkspaceOperation {
        revision: operation.revision + 1,
        ..operation.clone()
    })
}
pub fn delete(operation: &WorkspaceOperation) -> Result<WorkspaceSnapshot, String> {
    let path = resolve(&operation.root, &operation.relative_path)?;
    super::workspace_history::record_trash(&operation.root, &path)?;
    snapshot(&WorkspaceOperation {
        revision: operation.revision + 1,
        ..operation.clone()
    })
}
pub fn move_item(
    operation: &WorkspaceOperation,
    destination: &str,
) -> Result<WorkspaceSnapshot, String> {
    let source = resolve(&operation.root, &operation.relative_path)?;
    let target = resolve(&operation.root, destination)?;
    super::workspace_history::move_item(&operation.root, &source, &target)?;
    snapshot(&WorkspaceOperation {
        revision: operation.revision + 1,
        ..operation.clone()
    })
}

pub fn undo_last(
    root: &Path,
    revision: u64,
    expected_id: Option<&str>,
) -> Result<WorkspaceSnapshot, String> {
    super::workspace_history::undo(root, expected_id)?
        .ok_or("There is no workspace action to undo")?;
    snapshot(&WorkspaceOperation {
        root: root.to_path_buf(),
        relative_path: "".into(),
        revision: revision + 1,
    })
}
fn canonical_root(root: &Path) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    if root.is_dir() {
        Ok(root)
    } else {
        Err("workspace is not a directory".into())
    }
}
fn children(root: &Path, dir: &Path, depth: usize) -> Result<Vec<WorkspaceTreeEntry>, String> {
    if depth >= MAX_DEPTH {
        return Ok(vec![]);
    }
    let mut entries = fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter_map(|e| {
            let t = e.file_type().ok()?;
            if t.is_symlink() {
                return None;
            }
            Some((
                e.file_name().to_string_lossy().to_string(),
                e.path(),
                t.is_dir(),
            ))
        })
        .collect::<Vec<_>>();
    entries.sort_by(|a, b| b.2.cmp(&a.2).then_with(|| a.0.cmp(&b.0)));
    entries.truncate(MAX_CHILDREN);
    entries
        .into_iter()
        .map(|(name, path, is_dir)| {
            let children = if is_dir && !COLLAPSED.contains(&name.as_str()) {
                children(root, &path, depth + 1)?
            } else {
                vec![]
            };
            Ok(WorkspaceTreeEntry {
                name,
                path: path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string(),
                kind: if is_dir { "directory" } else { "file" }.into(),
                children,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn root() -> PathBuf {
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "mint-workspace-interface-{}-{suffix}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        root
    }

    #[test]
    fn interface_workspace_operations_return_snapshots_and_advance_revision() {
        let root = root();
        let create = WorkspaceOperation {
            root: root.clone(),
            relative_path: "notes/todo.md".into(),
            revision: 4,
        };
        let folder = WorkspaceOperation {
            relative_path: "notes".into(),
            ..create.clone()
        };
        let snapshot = create_folder(&folder).unwrap();
        assert_eq!(snapshot.revision, 5);
        let file = WorkspaceOperation {
            revision: snapshot.revision,
            ..create
        };
        let snapshot = create_file(&file).unwrap();
        assert_eq!(snapshot.revision, 6);
        assert!(root.join("notes/todo.md").is_file());
        assert_eq!(
            snapshot.path,
            root.canonicalize().unwrap().display().to_string()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn interface_workspace_rejects_parent_traversal() {
        let root = root();
        let operation = WorkspaceOperation {
            root: root.clone(),
            relative_path: "../escape.txt".into(),
            revision: 0,
        };
        assert_eq!(
            create_file(&operation).unwrap_err(),
            "workspace path escapes root"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(any(target_os = "linux", target_os = "windows"))]
    #[test]
    fn interface_workspace_trash_preserves_file_and_folder_contents_for_restore() {
        let root = root().canonicalize().unwrap();
        fs::write(root.join("note.txt"), "keep this file").unwrap();
        fs::create_dir(root.join("notes")).unwrap();
        fs::write(root.join("notes/nested.txt"), "keep nested contents").unwrap();

        for (revision, relative_path) in ["note.txt", "notes"].into_iter().enumerate() {
            let operation = WorkspaceOperation {
                root: root.clone(),
                relative_path: relative_path.into(),
                revision: revision as u64,
            };
            let snapshot = delete(&operation).unwrap();
            assert_eq!(snapshot.revision, revision as u64 + 1);
            assert!(!root.join(relative_path).exists());
            assert!(
                !snapshot
                    .tree
                    .children
                    .iter()
                    .any(|entry| entry.name == relative_path)
            );

            let item = trash::os_limited::list()
                .unwrap()
                .into_iter()
                .find(|item| item.original_path() == root.join(relative_path))
                .expect("workspace item must be recoverable from system Trash");
            trash::os_limited::restore_all([item]).unwrap();
        }
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).unwrap(),
            "keep this file"
        );
        assert_eq!(
            fs::read_to_string(root.join("notes/nested.txt")).unwrap(),
            "keep nested contents"
        );
        fs::remove_dir_all(root).unwrap();
    }
}

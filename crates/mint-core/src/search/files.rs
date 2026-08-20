use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::Serialize;
use thiserror::Error;

use crate::{Capability, MintConfig, SafetyError, assert_path_capability};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    "node_modules",
    ".cache",
    "dist",
    "build",
    "coverage",
    "target",
];

#[derive(Debug, Error)]
pub enum FileOperationError {
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("unable to create directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum PathKind {
    File,
    Directory,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PathMatch {
    pub path: PathBuf,
    pub kind: PathKind,
}

pub fn create_folder(target: &Path, config: &MintConfig) -> Result<PathBuf, FileOperationError> {
    let target = if target.is_absolute() || target.components().count() > 1 {
        target.to_path_buf()
    } else {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("Desktop")
            .join(target)
    };
    let target = assert_path_capability(&target, Capability::Write, config)?;
    fs::create_dir_all(&target).map_err(|source| FileOperationError::CreateDirectory {
        path: target.clone(),
        source,
    })?;
    Ok(target)
}

pub fn find_paths(
    query: &str,
    roots: &[PathBuf],
    limit: usize,
    config: &MintConfig,
) -> Vec<PathMatch> {
    let query = query.trim().to_lowercase();
    if query.is_empty() || limit == 0 {
        return Vec::new();
    }
    let mut exact = Vec::new();
    let mut partial = Vec::new();
    for root in roots {
        let Ok(root) = assert_path_capability(root, Capability::Read, config) else {
            continue;
        };
        visit(&root, &query, limit, config, &mut exact, &mut partial);
        if exact.len() >= limit || partial.len() >= limit {
            break;
        }
    }
    let mut matches = if exact.is_empty() { partial } else { exact };
    matches.sort_by(|left, right| left.path.cmp(&right.path));
    matches.truncate(limit);
    matches
}

fn visit(
    directory: &Path,
    query: &str,
    limit: usize,
    config: &MintConfig,
    exact: &mut Vec<PathMatch>,
    partial: &mut Vec<PathMatch>,
) {
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() && IGNORED_DIRECTORIES.contains(&name.as_str()) {
            continue;
        }
        if assert_path_capability(&path, Capability::Read, config).is_err() {
            continue;
        }
        let kind = if file_type.is_dir() {
            PathKind::Directory
        } else {
            PathKind::File
        };
        let lower_name = name.to_lowercase();
        if lower_name == query {
            exact.push(PathMatch {
                path: path.clone(),
                kind: kind.clone(),
            });
        } else if lower_name.contains(query) {
            partial.push(PathMatch {
                path: path.clone(),
                kind: kind.clone(),
            });
        }
        if exact.len() >= limit || partial.len() >= limit {
            return;
        }
        if file_type.is_dir() {
            visit(&path, query, limit, config, exact, partial);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_files_inside_allowed_root() {
        let root = std::env::temp_dir().join(format!("mint-find-{}", std::process::id()));
        fs::create_dir_all(root.join("nested")).unwrap();
        fs::write(root.join("nested/report.txt"), "ok").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            blocked_paths: vec![],
            ..MintConfig::default()
        };
        let matches = find_paths("report", std::slice::from_ref(&root), 5, &config);
        assert_eq!(matches.len(), 1);
        let _ = fs::remove_dir_all(root);
    }
}

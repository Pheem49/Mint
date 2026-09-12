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


/// Resolves a file path for reading/previewing across CLI, Desktop, and Web.
/// Handles:
/// 1. Tilde expansion (`~` or `~/...`) to user home directory.
/// 2. Config/notes paths (`.config/mint/notes/...`, `notes/...`, etc.) relative to home/config dir.
/// 3. Workspace-relative paths if workspace root is provided.
/// 4. Current working directory relative paths.
/// 5. Absolute paths.
/// Returns the first candidate that exists on disk, or the primary resolved path if none exists.
pub fn resolve_readable_path(target: &str, workspace: Option<&Path>) -> PathBuf {
    let trimmed = target.trim();
    if trimmed.is_empty() {
        return PathBuf::new();
    }

    let norm_slash = trimmed.replace('\\', "/");

    // 1. Tilde expansion (~/ or ~\)
    if let Some(home) = dirs::home_dir() {
        if trimmed == "~" {
            return home;
        }
        if let Some(rest) = norm_slash.strip_prefix("~/") {
            let candidate = home.join(rest);
            if candidate.exists() {
                return candidate;
            }
        }
    }

    let req_path = Path::new(trimmed);

    // If absolute and exists, return immediately
    if req_path.is_absolute() && req_path.exists() {
        return req_path.to_path_buf();
    }

    let mut candidates: Vec<PathBuf> = Vec::new();

    if req_path.is_absolute() {
        candidates.push(req_path.to_path_buf());
    }

    // Workspace-relative candidate
    if let Some(ws) = workspace {
        if !ws.as_os_str().is_empty() {
            if norm_slash != trimmed {
                candidates.push(ws.join(&norm_slash));
            }
            candidates.push(ws.join(trimmed));
        }
    }

    // Current working directory candidate
    if let Ok(cwd) = std::env::current_dir() {
        if norm_slash != trimmed {
            candidates.push(cwd.join(&norm_slash));
        }
        candidates.push(cwd.join(trimmed));
    }

    // Home directory candidates
    if let Some(home) = dirs::home_dir() {
        if let Some(rest) = norm_slash.strip_prefix("~/") {
            candidates.push(home.join(rest));
        }
        if norm_slash != trimmed {
            candidates.push(home.join(&norm_slash));
        }
        candidates.push(home.join(trimmed));
        if norm_slash.starts_with(".config/") {
            candidates.push(home.join(trimmed));
            if let Some(rest) = norm_slash.strip_prefix(".config/") {
                candidates.push(home.join(".config").join(rest));
            }
        }
        candidates.push(home.join(".config").join("mint").join("notes").join(trimmed));
        if norm_slash != trimmed {
            candidates.push(home.join(".config").join("mint").join("notes").join(&norm_slash));
        }
        if let Some(fname) = req_path.file_name() {
            candidates.push(home.join(".config").join("mint").join("notes").join(fname));
        }
    }

    // Config directory candidates (native OS: macOS ~/Library/Application Support, Windows %APPDATA%, Linux ~/.config)
    if let Some(cfg) = dirs::config_dir() {
        candidates.push(cfg.join(trimmed));
        if norm_slash != trimmed {
            candidates.push(cfg.join(&norm_slash));
        }
        if let Some(rest) = norm_slash.strip_prefix(".config/") {
            candidates.push(cfg.join(rest));
        }
        candidates.push(cfg.join("mint").join("notes").join(trimmed));
        if norm_slash != trimmed {
            candidates.push(cfg.join("mint").join("notes").join(&norm_slash));
        }
        if let Some(fname) = req_path.file_name() {
            candidates.push(cfg.join("mint").join("notes").join(fname));
        }
    }

    // Return the first candidate that exists as a file or directory
    for candidate in &candidates {
        if candidate.is_file() || candidate.exists() {
            return candidate.clone();
        }
    }

    // Fallback if none exists
    if let Some(ws) = workspace {
        if !ws.as_os_str().is_empty() {
            return ws.join(trimmed);
        }
    }

    candidates.into_iter().next().unwrap_or_else(|| req_path.to_path_buf())
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

    #[test]
    fn test_resolve_readable_path_workspace() {
        let temp_dir = std::env::temp_dir().join(format!("mint-resolve-test-{}", std::process::id()));
        let _ = fs::create_dir_all(temp_dir.join("sub"));
        let test_file = temp_dir.join("sub").join("sample.md");
        let _ = fs::write(&test_file, "# Test Preview");

        // Relative path with workspace provided (both slash and backslash)
        let resolved = resolve_readable_path("sub/sample.md", Some(&temp_dir));
        assert_eq!(resolved, test_file);

        let resolved_win = resolve_readable_path("sub\\sample.md", Some(&temp_dir));
        assert_eq!(resolved_win, test_file);

        // Non-existent path returns fallback joined with workspace
        let missing = resolve_readable_path("missing.md", Some(&temp_dir));
        assert_eq!(missing, temp_dir.join("missing.md"));

        let _ = fs::remove_dir_all(temp_dir);
    }
}


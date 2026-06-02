use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::{Capability, MintConfig, SafetyError, assert_path_capability};

const IGNORED_DIRECTORIES: &[&str] = &[
    ".git",
    ".cache",
    "build",
    "dist",
    "node_modules",
    "out",
    "target",
];

#[derive(Debug, Error)]
pub enum CodeInspectionError {
    #[error(transparent)]
    Safety(#[from] SafetyError),
    #[error("unable to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("path is not a file: {0}")]
    NotAFile(PathBuf),
    #[error("unable to write {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("edit request must contain at least one file")]
    EmptyEditRequest,
    #[error("approval token does not match the proposed edit")]
    InvalidApprovalToken,
    #[error("file changed after approval proposal: {0}")]
    StaleProposal(PathBuf),
    #[error("patch hunk {index} old text was not found in {path}")]
    PatchTextNotFound { path: PathBuf, index: usize },
    #[error("edit path escapes workspace root: {0}")]
    OutsideWorkspace(PathBuf),
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeFile {
    pub path: PathBuf,
    pub size: u64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeSearchHit {
    pub path: PathBuf,
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RepositorySummary {
    pub root: PathBuf,
    pub file_count: usize,
    pub total_bytes: u64,
    pub extensions: BTreeMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodePlan {
    pub task: String,
    pub root: PathBuf,
    pub inspect_files: Vec<PathBuf>,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeEdit {
    pub path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodePatchHunk {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeEditPreview {
    pub path: PathBuf,
    pub existed: bool,
    pub before_sha256: String,
    pub after_sha256: String,
    pub diff: String,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CodeEditProposal {
    pub approval_required: bool,
    pub approval_token: String,
    pub edits: Vec<CodeEditPreview>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AppliedCodeEdit {
    pub path: PathBuf,
    pub created: bool,
    pub bytes_written: usize,
}

pub fn list_code_files(
    root: &Path,
    limit: usize,
    config: &MintConfig,
) -> Result<Vec<CodeFile>, CodeInspectionError> {
    let root = assert_path_capability(root, Capability::Read, config)?;
    let mut files = Vec::new();
    collect_files(&root, &mut files, limit.max(1))?;
    Ok(files)
}

pub fn read_code_file(
    path: &Path,
    start_line: usize,
    end_line: usize,
    config: &MintConfig,
) -> Result<String, CodeInspectionError> {
    let path = assert_path_capability(path, Capability::Read, config)?;
    if !path.is_file() {
        return Err(CodeInspectionError::NotAFile(path));
    }
    let raw = fs::read_to_string(&path).map_err(|source| CodeInspectionError::Read {
        path: path.clone(),
        source,
    })?;
    let first = start_line.max(1);
    let last = end_line.max(first);
    Ok(raw
        .lines()
        .enumerate()
        .filter(|(index, _)| {
            let line = index + 1;
            line >= first && line <= last
        })
        .map(|(index, line)| format!("{:>6} | {line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n"))
}

pub fn search_code(
    root: &Path,
    query: &str,
    limit: usize,
    config: &MintConfig,
) -> Result<Vec<CodeSearchHit>, CodeInspectionError> {
    let files = list_code_files(root, usize::MAX, config)?;
    let mut hits = Vec::new();
    if query.trim().is_empty() {
        return Ok(hits);
    }
    let escaped = regex::escape(query);
    let re = match regex::RegexBuilder::new(&escaped)
        .case_insensitive(true)
        .build()
    {
        Ok(re) => re,
        Err(_) => return Ok(hits),
    };
    for file in files {
        let Ok(raw) = fs::read_to_string(&file.path) else {
            continue;
        };
        if !re.is_match(&raw) {
            continue;
        }
        for (index, line) in raw.lines().enumerate() {
            if re.is_match(line) {
                hits.push(CodeSearchHit {
                    path: file.path.clone(),
                    line: index + 1,
                    text: line.trim().to_owned(),
                });
                if hits.len() >= limit.max(1) {
                    return Ok(hits);
                }
            }
        }
    }
    Ok(hits)
}

pub fn repository_summary(
    root: &Path,
    config: &MintConfig,
) -> Result<RepositorySummary, CodeInspectionError> {
    let root = assert_path_capability(root, Capability::Read, config)?;
    let files = list_code_files(&root, usize::MAX, config)?;
    let mut extensions = BTreeMap::new();
    for file in &files {
        let extension = file
            .path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or("<none>")
            .to_lowercase();
        *extensions.entry(extension).or_insert(0) += 1;
    }
    Ok(RepositorySummary {
        root,
        file_count: files.len(),
        total_bytes: files.iter().map(|file| file.size).sum(),
        extensions,
    })
}

pub fn inspect_code_plan(
    task: impl Into<String>,
    root: &Path,
    inspect_files: Vec<PathBuf>,
    config: &MintConfig,
) -> Result<CodePlan, CodeInspectionError> {
    let root = assert_path_capability(root, Capability::Read, config)?;
    let inspect_files = inspect_files
        .into_iter()
        .map(|path| workspace_path(&root, &path, Capability::Read, config))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(CodePlan {
        task: task.into(),
        root,
        inspect_files,
        steps: vec![
            "Inspect repository summary and relevant files".into(),
            "Search for affected symbols and behavior contracts".into(),
            "Propose scoped edits and verification commands".into(),
            "Require explicit approval before shell execution or file writes".into(),
        ],
    })
}

pub fn build_code_patch(
    root: &Path,
    path: PathBuf,
    hunks: &[CodePatchHunk],
    config: &MintConfig,
) -> Result<CodeEdit, CodeInspectionError> {
    let root = assert_path_capability(root, Capability::Write, config)?;
    let path = workspace_path(&root, &path, Capability::Write, config)?;
    let mut content = read_existing_content(&path)?;
    for (index, hunk) in hunks.iter().enumerate() {
        if !content.contains(&hunk.old_text) {
            return Err(CodeInspectionError::PatchTextNotFound {
                path,
                index: index + 1,
            });
        }
        content = content.replacen(&hunk.old_text, &hunk.new_text, 1);
    }
    Ok(CodeEdit { path, content })
}

pub fn propose_code_edits(
    root: &Path,
    edits: &[CodeEdit],
    config: &MintConfig,
) -> Result<CodeEditProposal, CodeInspectionError> {
    if edits.is_empty() {
        return Err(CodeInspectionError::EmptyEditRequest);
    }
    let root = assert_path_capability(root, Capability::Write, config)?;
    let previews = prepare_edits(&root, edits, config)?;
    Ok(CodeEditProposal {
        approval_required: true,
        approval_token: approval_token(&root, &previews),
        edits: previews,
    })
}

pub fn apply_code_edits(
    root: &Path,
    edits: &[CodeEdit],
    approval_token_value: &str,
    config: &MintConfig,
) -> Result<Vec<AppliedCodeEdit>, CodeInspectionError> {
    let proposal = propose_code_edits(root, edits, config)?;
    if proposal.approval_token != approval_token_value {
        return Err(CodeInspectionError::InvalidApprovalToken);
    }
    let root = assert_path_capability(root, Capability::Write, config)?;
    let prepared = prepare_edits(&root, edits, config)?;
    for preview in &prepared {
        let current = read_optional_content(&preview.path)?;
        if sha256(&current) != preview.before_sha256 {
            return Err(CodeInspectionError::StaleProposal(preview.path.clone()));
        }
    }
    let mut applied = Vec::new();
    for (edit, preview) in edits.iter().zip(prepared) {
        if let Some(parent) = preview.path.parent() {
            fs::create_dir_all(parent).map_err(|source| CodeInspectionError::Write {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&preview.path, &edit.content).map_err(|source| CodeInspectionError::Write {
            path: preview.path.clone(),
            source,
        })?;
        applied.push(AppliedCodeEdit {
            path: preview.path,
            created: !preview.existed,
            bytes_written: edit.content.len(),
        });
    }
    Ok(applied)
}

fn prepare_edits(
    root: &Path,
    edits: &[CodeEdit],
    config: &MintConfig,
) -> Result<Vec<CodeEditPreview>, CodeInspectionError> {
    edits
        .iter()
        .map(|edit| {
            let path = workspace_path(root, &edit.path, Capability::Write, config)?;
            let existed = path.exists();
            let previous = read_optional_content(&path)?;
            Ok(CodeEditPreview {
                path: path.clone(),
                existed,
                before_sha256: sha256(&previous),
                after_sha256: sha256(&edit.content),
                diff: full_file_diff(&path, &previous, &edit.content),
            })
        })
        .collect()
}

fn workspace_path(
    root: &Path,
    path: &Path,
    capability: Capability,
    config: &MintConfig,
) -> Result<PathBuf, CodeInspectionError> {
    let path = assert_path_capability(&root.join(path), capability, config)?;
    if !path.starts_with(root) {
        return Err(CodeInspectionError::OutsideWorkspace(path));
    }
    Ok(path)
}

fn read_existing_content(path: &Path) -> Result<String, CodeInspectionError> {
    if !path.is_file() {
        return Err(CodeInspectionError::NotAFile(path.to_path_buf()));
    }
    fs::read_to_string(path).map_err(|source| CodeInspectionError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn read_optional_content(path: &Path) -> Result<String, CodeInspectionError> {
    if path.exists() {
        read_existing_content(path)
    } else {
        Ok(String::new())
    }
}

fn approval_token(root: &Path, edits: &[CodeEditPreview]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"mint-code-edit-approval-v1\0");
    hasher.update(root.to_string_lossy().as_bytes());
    for edit in edits {
        hasher.update(b"\0");
        hasher.update(edit.path.to_string_lossy().as_bytes());
        hasher.update(b"\0");
        hasher.update(edit.before_sha256.as_bytes());
        hasher.update(b"\0");
        hasher.update(edit.after_sha256.as_bytes());
    }
    format!("{:x}", hasher.finalize())
}

fn sha256(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

fn full_file_diff(path: &Path, previous: &str, next: &str) -> String {
    let label = path.display();
    let mut lines = vec![
        format!("--- a/{label}"),
        format!("+++ b/{label}"),
        format!(
            "@@ -1,{} +1,{} @@",
            previous.lines().count(),
            next.lines().count()
        ),
    ];
    lines.extend(previous.lines().map(|line| format!("-{line}")));
    lines.extend(next.lines().map(|line| format!("+{line}")));
    lines.join("\n")
}

fn collect_files(
    directory: &Path,
    files: &mut Vec<CodeFile>,
    limit: usize,
) -> Result<(), CodeInspectionError> {
    if files.len() >= limit || is_ignored_directory(directory) {
        return Ok(());
    }
    let entries = fs::read_dir(directory).map_err(|source| CodeInspectionError::Read {
        path: directory.to_path_buf(),
        source,
    })?;
    for entry in entries {
        if files.len() >= limit {
            break;
        }
        let entry = entry.map_err(|source| CodeInspectionError::Read {
            path: directory.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|source| CodeInspectionError::Read {
                path: path.clone(),
                source,
            })?;
        if file_type.is_dir() {
            collect_files(&path, files, limit)?;
        } else if file_type.is_file() {
            let size = entry
                .metadata()
                .map_err(|source| CodeInspectionError::Read {
                    path: path.clone(),
                    source,
                })?
                .len();
            files.push(CodeFile { path, size });
        }
    }
    Ok(())
}

fn is_ignored_directory(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| IGNORED_DIRECTORIES.contains(&name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config_for(root: &Path) -> MintConfig {
        MintConfig {
            allowed_read_paths: vec![root.to_path_buf()],
            allowed_write_paths: vec![root.to_path_buf()],
            blocked_paths: vec![],
            ..MintConfig::default()
        }
    }

    #[test]
    fn searches_text_files_and_skips_build_directories() {
        let root = std::env::temp_dir().join("mint-code-tools-search");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("target")).unwrap();
        fs::write(root.join("main.rs"), "fn mint_tool() {}\n").unwrap();
        fs::write(root.join("target/generated.rs"), "mint_tool\n").unwrap();
        let hits = search_code(&root, "mint_tool", 10, &config_for(&root)).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].line, 1);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn blocks_plan_files_outside_allowed_root() {
        let root = std::env::temp_dir().join("mint-code-tools-plan");
        fs::create_dir_all(&root).unwrap();
        let result = inspect_code_plan(
            "test",
            &root,
            vec![PathBuf::from("../../etc/passwd")],
            &config_for(&root),
        );
        assert!(matches!(result, Err(CodeInspectionError::Safety(_))));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn requires_matching_approval_token_before_writing() {
        let root = std::env::temp_dir().join("mint-code-tools-approval");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let edit = CodeEdit {
            path: PathBuf::from("note.txt"),
            content: "approved\n".into(),
        };
        let config = config_for(&root);
        assert!(matches!(
            apply_code_edits(&root, std::slice::from_ref(&edit), "wrong", &config),
            Err(CodeInspectionError::InvalidApprovalToken)
        ));
        assert!(!root.join("note.txt").exists());
        let proposal = propose_code_edits(&root, std::slice::from_ref(&edit), &config).unwrap();
        apply_code_edits(&root, &[edit], &proposal.approval_token, &config).unwrap();
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).unwrap(),
            "approved\n"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_approved_edit_after_source_changes() {
        let root = std::env::temp_dir().join("mint-code-tools-stale");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "before\n").unwrap();
        let edit = CodeEdit {
            path: PathBuf::from("note.txt"),
            content: "after\n".into(),
        };
        let config = config_for(&root);
        let proposal = propose_code_edits(&root, std::slice::from_ref(&edit), &config).unwrap();
        fs::write(root.join("note.txt"), "changed elsewhere\n").unwrap();
        assert!(matches!(
            apply_code_edits(&root, &[edit], &proposal.approval_token, &config),
            Err(CodeInspectionError::InvalidApprovalToken)
                | Err(CodeInspectionError::StaleProposal(_))
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn patch_replaces_exact_text_once() {
        let root = std::env::temp_dir().join("mint-code-tools-patch");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "one one\n").unwrap();
        let edit = build_code_patch(
            &root,
            PathBuf::from("note.txt"),
            &[CodePatchHunk {
                old_text: "one".into(),
                new_text: "two".into(),
            }],
            &config_for(&root),
        )
        .unwrap();
        assert_eq!(edit.content, "two one\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn blocks_edit_paths_outside_workspace_even_when_policy_allows_them() {
        let root = std::env::temp_dir().join("mint-code-tools-workspace");
        fs::create_dir_all(&root).unwrap();
        let config = MintConfig {
            allowed_write_paths: vec![std::env::temp_dir()],
            blocked_paths: vec![],
            ..MintConfig::default()
        };
        let result = propose_code_edits(
            &root,
            &[CodeEdit {
                path: PathBuf::from("../outside.txt"),
                content: "blocked".into(),
            }],
            &config,
        );
        assert!(matches!(
            result,
            Err(CodeInspectionError::OutsideWorkspace(_))
        ));
        let _ = fs::remove_dir_all(root);
    }
}

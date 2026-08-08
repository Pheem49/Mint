use std::{
    collections::BTreeMap,
    fs,
    io::Write as _,
    path::{Path, PathBuf},
};

use grep_regex::RegexMatcherBuilder;
use grep_searcher::{SearcherBuilder, Sink, SinkMatch};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use similar::{ChangeTag, TextDiff};
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
    #[error(
        "patch hunk {index} old text matches {occurrences} locations in {path}; \
         supply more surrounding context to make it unique, or set replaceAll to replace all of them"
    )]
    AmbiguousPatchText {
        path: PathBuf,
        index: usize,
        occurrences: usize,
    },
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
    /// Replace every occurrence of `old_text` instead of requiring it to be
    /// unique. Defaults to `false`, matching the safer default of rejecting
    /// ambiguous matches rather than silently editing the wrong one.
    #[serde(default)]
    pub replace_all: bool,
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
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(false)
        .git_ignore(true)
        .ignore(true)
        .build();

    for result in walker {
        if files.len() >= limit.max(1) {
            break;
        }
        let entry = match result {
            Ok(e) => e,
            Err(_) => continue,
        };
        let path = entry.path();
        if path.is_file() {
            if contains_ignored_directory(path, &root) {
                continue;
            }
            if let Ok(metadata) = path.metadata() {
                files.push(CodeFile {
                    path: path.to_path_buf(),
                    size: metadata.len(),
                });
            }
        }
    }
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
    let total_lines = raw.lines().count();
    let first = start_line.max(1);
    let last = end_line.max(first);
    let body = raw
        .lines()
        .enumerate()
        .filter(|(index, _)| {
            let line = index + 1;
            line >= first && line <= last
        })
        .map(|(index, line)| format!("{:>6} | {line}", index + 1))
        .collect::<Vec<_>>()
        .join("\n");

    // Without this, a truncated read looks identical to "this is the whole
    // file" — the caller (model or human) has no signal that more content
    // exists, or what range to ask for next.
    if last < total_lines {
        let next_end = (last + (last - first + 1)).min(total_lines);
        Ok(format!(
            "{body}\n\n[Showing lines {first}-{last} of {total_lines} total lines — the rest of \
             the file was NOT included. To continue reading, call read_file again with \
             startLine={}, endLine={next_end}.]",
            last + 1,
        ))
    } else {
        Ok(body)
    }
}

struct SearchHitSink<'a> {
    path: &'a Path,
    hits: &'a mut Vec<CodeSearchHit>,
    limit: usize,
}

impl<'a> Sink for SearchHitSink<'a> {
    type Error = std::io::Error;

    fn matched(
        &mut self,
        _searcher: &grep_searcher::Searcher,
        mat: &SinkMatch<'_>,
    ) -> Result<bool, Self::Error> {
        let line = mat.line_number().unwrap_or(0) as usize;
        let text = String::from_utf8_lossy(mat.bytes()).trim().to_owned();
        self.hits.push(CodeSearchHit {
            path: self.path.to_path_buf(),
            line,
            text,
        });
        Ok(self.hits.len() < self.limit)
    }
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
    let matcher = match RegexMatcherBuilder::new()
        .case_insensitive(true)
        .build(&escaped)
    {
        Ok(m) => m,
        Err(_) => return Ok(hits),
    };
    let mut searcher = SearcherBuilder::new().build();
    for file in files {
        let mut sink = SearchHitSink {
            path: &file.path,
            hits: &mut hits,
            limit,
        };
        let _ = searcher.search_path(&matcher, &file.path, &mut sink);
        if hits.len() >= limit.max(1) {
            break;
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
        let occurrences = content.matches(hunk.old_text.as_str()).count();
        if occurrences == 0 {
            return Err(CodeInspectionError::PatchTextNotFound {
                path,
                index: index + 1,
            });
        }
        if occurrences > 1 && !hunk.replace_all {
            return Err(CodeInspectionError::AmbiguousPatchText {
                path,
                index: index + 1,
                occurrences,
            });
        }
        content = if hunk.replace_all {
            content.replace(&hunk.old_text, &hunk.new_text)
        } else {
            content.replacen(&hunk.old_text, &hunk.new_text, 1)
        };
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
        write_atomic(&preview.path, &edit.content)?;
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

/// Writes `content` to `path` atomically: writes to a temp file in the same
/// directory (guaranteeing the same filesystem, required for an atomic rename),
/// `fsync`s it for durability, then renames it onto `path`. A crash or power
/// loss mid-write leaves either the old file or the new file intact, never a
/// truncated/corrupted one. Best-effort cleans up the temp file on failure.
fn write_atomic(path: &Path, content: &str) -> Result<(), CodeInspectionError> {
    let write_error = |source: std::io::Error| CodeInspectionError::Write {
        path: path.to_path_buf(),
        source,
    };
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let file_name = path
        .file_name()
        .ok_or_else(|| {
            write_error(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "path has no file name",
            ))
        })?
        .to_string_lossy();
    let tmp_path = parent.join(format!(".{file_name}.mint-tmp-{}", std::process::id()));

    let result = (|| -> std::io::Result<()> {
        let mut file = fs::File::create(&tmp_path)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()
    })();

    match result {
        Ok(()) => fs::rename(&tmp_path, path).map_err(write_error),
        Err(source) => {
            let _ = fs::remove_file(&tmp_path);
            Err(write_error(source))
        }
    }
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
    let diff = TextDiff::from_lines(previous, next);
    let mut lines = vec![format!("--- a/{label}"), format!("+++ b/{label}")];

    for group in diff.grouped_ops(3) {
        let mut first = true;
        for op in group {
            if first {
                let old_start = op.old_range().start + 1;
                let old_len = op.old_range().len();
                let new_start = op.new_range().start + 1;
                let new_len = op.new_range().len();
                lines.push(format!(
                    "@@ -{old_start},{old_len} +{new_start},{new_len} @@"
                ));
                first = false;
            }
            for change in diff.iter_changes(&op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                let content = change.value().trim_end_matches(&['\r', '\n'][..]);
                lines.push(format!("{sign}{content}"));
            }
        }
    }

    lines.join("\n")
}

fn contains_ignored_directory(path: &Path, root: &Path) -> bool {
    let relative = path.strip_prefix(root).unwrap_or(path);
    for component in relative.components() {
        if let std::path::Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                if IGNORED_DIRECTORIES.contains(&name_str) {
                    return true;
                }
            }
        }
    }
    false
}

pub fn parse_github_url(url: &str) -> Option<(String, String)> {
    let cleaned = url
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .trim_start_matches("github.com/");

    let parts: Vec<&str> = cleaned.split('/').collect();
    if parts.len() >= 2 {
        let owner = parts[0].to_string();
        let mut repo = parts[1].to_string();
        if repo.ends_with(".git") {
            repo = repo[..repo.len() - 4].to_string();
        }
        Some((owner, repo))
    } else {
        None
    }
}

pub async fn fetch_github_repo_summary(owner: &str, repo: &str) -> Result<String, String> {
    let client = crate::HTTP_CLIENT.clone();

    // 1. Fetch Repository Info
    let repo_url = format!("https://api.github.com/repos/{}/{}", owner, repo);
    let repo_resp = client
        .get(&repo_url)
        .header("User-Agent", "mint-core")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if !repo_resp.status().is_success() {
        return Err(format!(
            "Failed to fetch repository metadata: {}",
            repo_resp.status()
        ));
    }
    let repo_info: serde_json::Value = repo_resp.json().await.map_err(|e| e.to_string())?;

    let description = repo_info["description"]
        .as_str()
        .unwrap_or("No description provided.");
    let language = repo_info["language"].as_str().unwrap_or("Unknown");
    let stars = repo_info["stargazers_count"].as_u64().unwrap_or(0);
    let forks = repo_info["forks_count"].as_u64().unwrap_or(0);

    let mut topics_list = Vec::new();
    if let Some(topics) = repo_info["topics"].as_array() {
        for t in topics {
            if let Some(t_str) = t.as_str() {
                topics_list.push(t_str.to_string());
            }
        }
    }
    let topics_str = if topics_list.is_empty() {
        "None".to_string()
    } else {
        topics_list.join(", ")
    };

    // 2. Fetch Directory contents (top level)
    let contents_url = format!("https://api.github.com/repos/{}/{}/contents", owner, repo);
    let contents_resp = client
        .get(&contents_url)
        .header("User-Agent", "mint-core")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let mut file_tree = String::from("Unavailable");
    if contents_resp.status().is_success()
        && let Ok(contents_info) = contents_resp.json::<serde_json::Value>().await
        && let Some(arr) = contents_info.as_array()
    {
        let mut files = Vec::new();
        for item in arr {
            let name = item["name"].as_str().unwrap_or("");
            let r#type = item["type"].as_str().unwrap_or("");
            files.push(format!("- {} ({})", name, r#type));
        }
        file_tree = files.join("\n");
    }

    // 3. Fetch README.md
    let readme_url = format!("https://api.github.com/repos/{}/{}/readme", owner, repo);
    let readme_resp = client
        .get(&readme_url)
        .header("User-Agent", "mint-core")
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let mut readme_text = String::from("No README available.");
    if readme_resp.status().is_success()
        && let Ok(readme_info) = readme_resp.json::<serde_json::Value>().await
        && let Some(content_b64) = readme_info["content"].as_str()
    {
        let cleaned_b64 = content_b64.replace(['\n', '\r'], "");
        use base64::{Engine as _, engine::general_purpose::STANDARD};
        if let Ok(decoded_bytes) = STANDARD.decode(cleaned_b64) {
            readme_text = String::from_utf8_lossy(&decoded_bytes).to_string();
        }
    }

    let summary = format!(
        "Repository: {}/{}\nDescription: {}\nPrimary Language: {}\nStars: {}\nForks: {}\nTopics: {}\n\nTop-level File Directory:\n{}\n\nREADME.md:\n{}",
        owner, repo, description, language, stars, forks, topics_str, file_tree, readme_text
    );

    Ok(summary)
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
    fn read_code_file_has_no_truncation_note_when_whole_file_fits() {
        let root = std::env::temp_dir().join("mint-code-tools-read-full");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("small.txt"), "one\ntwo\nthree\n").unwrap();
        let content = read_code_file(&root.join("small.txt"), 1, 240, &config_for(&root)).unwrap();
        assert!(content.contains("one"));
        assert!(content.contains("three"));
        assert!(!content.contains("Showing lines"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn read_code_file_notes_truncation_and_the_exact_next_range() {
        let root = std::env::temp_dir().join("mint-code-tools-read-truncated");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let lines: Vec<String> = (1..=500).map(|n| format!("line{n}")).collect();
        fs::write(root.join("big.txt"), lines.join("\n") + "\n").unwrap();

        let content = read_code_file(&root.join("big.txt"), 1, 240, &config_for(&root)).unwrap();
        assert!(content.contains("line240"));
        assert!(
            !content.contains("line241"),
            "must not leak past the requested range"
        );
        assert!(content.contains("Showing lines 1-240 of 500 total lines"));
        assert!(content.contains("startLine=241, endLine=480"));

        // Following that exact guidance should reach the end without another
        // truncation note.
        let rest = read_code_file(&root.join("big.txt"), 241, 480, &config_for(&root)).unwrap();
        assert!(rest.contains("line480"));
        assert!(rest.contains("Showing lines 241-480 of 500 total lines"));
        let final_chunk =
            read_code_file(&root.join("big.txt"), 481, 500, &config_for(&root)).unwrap();
        assert!(final_chunk.contains("line500"));
        assert!(!final_chunk.contains("Showing lines"));

        let _ = fs::remove_dir_all(root);
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
    fn lists_explicitly_requested_build_directory() {
        let root = std::env::temp_dir().join("mint-code-tools-explicit-out");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("out")).unwrap();
        fs::write(root.join("out/index.html"), "<div>built</div>\n").unwrap();
        fs::write(root.join("main.rs"), "fn main() {}\n").unwrap();

        let config = config_for(&root);
        let repo_files = list_code_files(&root, 10, &config).unwrap();
        assert_eq!(repo_files.len(), 1);
        assert_eq!(repo_files[0].path, root.join("main.rs"));

        let out_files = list_code_files(&root.join("out"), 10, &config).unwrap();
        assert_eq!(out_files.len(), 1);
        assert_eq!(out_files[0].path, root.join("out/index.html"));

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
    fn patch_replaces_unique_text_once() {
        let root = std::env::temp_dir().join("mint-code-tools-patch-unique");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "one two\n").unwrap();
        let edit = build_code_patch(
            &root,
            PathBuf::from("note.txt"),
            &[CodePatchHunk {
                old_text: "one".into(),
                new_text: "three".into(),
                replace_all: false,
            }],
            &config_for(&root),
        )
        .unwrap();
        assert_eq!(edit.content, "three two\n");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn patch_rejects_ambiguous_old_text_by_default() {
        let root = std::env::temp_dir().join("mint-code-tools-patch-ambiguous");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "one one\n").unwrap();
        let error = build_code_patch(
            &root,
            PathBuf::from("note.txt"),
            &[CodePatchHunk {
                old_text: "one".into(),
                new_text: "two".into(),
                replace_all: false,
            }],
            &config_for(&root),
        )
        .unwrap_err();
        assert!(matches!(
            error,
            CodeInspectionError::AmbiguousPatchText { occurrences: 2, .. }
        ));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn patch_replace_all_replaces_every_occurrence() {
        let root = std::env::temp_dir().join("mint-code-tools-patch-replace-all");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("note.txt"), "one one\n").unwrap();
        let edit = build_code_patch(
            &root,
            PathBuf::from("note.txt"),
            &[CodePatchHunk {
                old_text: "one".into(),
                new_text: "two".into(),
                replace_all: true,
            }],
            &config_for(&root),
        )
        .unwrap();
        assert_eq!(edit.content, "two two\n");
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

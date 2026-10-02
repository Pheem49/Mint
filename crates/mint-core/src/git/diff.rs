use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DiffHunk {
    pub old_text: String,
    pub new_text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceFileChange {
    pub path: String,
    pub created: bool,
    pub additions: usize,
    pub deletions: usize,
    pub hunks: Vec<DiffHunk>,
}

/// Read all working tree and staged Git changes in the workspace.
pub fn read_workspace_git_diff(root: &Path) -> Result<Vec<WorkspaceFileChange>, String> {
    if !crate::git::is_git_repo(root) {
        return Ok(Vec::new());
    }

    // Try diff against HEAD. If HEAD doesn't exist yet (fresh repo), diff unstaged and staged.
    let diff_output = Command::new("git")
        .args(["diff", "-U3", "HEAD"])
        .current_dir(root)
        .output();

    let diff_text = match diff_output {
        Ok(out) if out.status.success() => String::from_utf8_lossy(&out.stdout).to_string(),
        _ => {
            // Fallback for empty repo without HEAD commit
            let mut combined = String::new();
            if let Ok(out) = Command::new("git")
                .args(["diff", "-U3"])
                .current_dir(root)
                .output()
            {
                combined.push_str(&String::from_utf8_lossy(&out.stdout));
            }
            if let Ok(out) = Command::new("git")
                .args(["diff", "--cached", "-U3"])
                .current_dir(root)
                .output()
            {
                combined.push_str(&String::from_utf8_lossy(&out.stdout));
            }
            combined
        }
    };

    let mut changes = parse_unified_diff(&diff_text);
    let tracked_paths: std::collections::HashSet<String> =
        changes.iter().map(|c| c.path.clone()).collect();

    // Also include untracked new files from `git status --porcelain`
    if let Ok(status_out) = Command::new("git")
        .args(["status", "--porcelain", "-uall"])
        .current_dir(root)
        .output()
    {
        let status_text = String::from_utf8_lossy(&status_out.stdout);
        for line in status_text.lines() {
            if line.starts_with("?? ") {
                let rel_path = line[3..].trim().trim_matches('"');
                if tracked_paths.contains(rel_path) {
                    continue;
                }
                let full_path = root.join(rel_path);
                if full_path.is_file() {
                    // Check file size, skip if > 250KB to keep UI snappy
                    if let Ok(meta) = fs::metadata(&full_path) {
                        if meta.len() > 256 * 1024 {
                            changes.push(WorkspaceFileChange {
                                path: rel_path.to_string(),
                                created: true,
                                additions: 0,
                                deletions: 0,
                                hunks: vec![DiffHunk {
                                    old_text: String::new(),
                                    new_text: "(Large untracked file omitted from preview)"
                                        .to_string(),
                                }],
                            });
                            continue;
                        }
                    }

                    if let Ok(content) = fs::read_to_string(&full_path) {
                        let lines_count = content.lines().count();
                        changes.push(WorkspaceFileChange {
                            path: rel_path.to_string(),
                            created: true,
                            additions: lines_count,
                            deletions: 0,
                            hunks: vec![DiffHunk {
                                old_text: String::new(),
                                new_text: content,
                            }],
                        });
                    }
                }
            }
        }
    }

    Ok(changes)
}

fn parse_unified_diff(diff: &str) -> Vec<WorkspaceFileChange> {
    let mut files = Vec::new();
    let mut current_file: Option<WorkspaceFileChange> = None;
    let mut current_old_lines = Vec::new();
    let mut current_new_lines = Vec::new();
    let mut in_hunk = false;

    let flush_hunk =
        |f: &mut WorkspaceFileChange, old_lines: &mut Vec<String>, new_lines: &mut Vec<String>| {
            if !old_lines.is_empty() || !new_lines.is_empty() {
                f.hunks.push(DiffHunk {
                    old_text: old_lines.join("\n"),
                    new_text: new_lines.join("\n"),
                });
                old_lines.clear();
                new_lines.clear();
            }
        };

    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            if let Some(mut f) = current_file.take() {
                flush_hunk(&mut f, &mut current_old_lines, &mut current_new_lines);
                files.push(f);
            }
            in_hunk = false;

            // Extract file path from "diff --git a/path b/path"
            let path = line
                .split(" b/")
                .nth(1)
                .unwrap_or_else(|| line.split_whitespace().last().unwrap_or(""))
                .to_string();

            current_file = Some(WorkspaceFileChange {
                path,
                created: false,
                additions: 0,
                deletions: 0,
                hunks: Vec::new(),
            });
            continue;
        }

        let Some(ref mut f) = current_file else {
            continue;
        };

        if line.starts_with("new file mode") {
            f.created = true;
            continue;
        }

        if line.starts_with("--- ") || line.starts_with("+++ ") {
            if line.starts_with("--- /dev/null") {
                f.created = true;
            }
            continue;
        }

        if line.starts_with("@@ ") {
            if in_hunk {
                flush_hunk(f, &mut current_old_lines, &mut current_new_lines);
            }
            in_hunk = true;
            continue;
        }

        if in_hunk {
            if let Some(rest) = line.strip_prefix('+') {
                f.additions += 1;
                current_new_lines.push(rest.to_string());
            } else if let Some(rest) = line.strip_prefix('-') {
                f.deletions += 1;
                current_old_lines.push(rest.to_string());
            } else if let Some(rest) = line.strip_prefix(' ') {
                current_old_lines.push(rest.to_string());
                current_new_lines.push(rest.to_string());
            } else if line == "\\ No newline at end of file" {
                // Ignore newline warning marker
            }
        }
    }

    if let Some(mut f) = current_file.take() {
        flush_hunk(&mut f, &mut current_old_lines, &mut current_new_lines);
        files.push(f);
    }

    files
}

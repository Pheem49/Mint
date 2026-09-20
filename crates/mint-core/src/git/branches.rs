use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

/// The branch state visible to every Mint surface for one workspace.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BranchInfo {
    pub is_repository: bool,
    pub current_branch: Option<String>,
    pub detached_head: Option<String>,
    pub branches: Vec<String>,
    pub remote_branches: Vec<String>,
    pub is_dirty: bool,
}

pub fn read_branch_info(root: &Path) -> Result<BranchInfo, String> {
    let repository_check = git(root, ["rev-parse", "--is-inside-work-tree"])?;
    if !repository_check.status.success()
        || String::from_utf8_lossy(&repository_check.stdout).trim() != "true"
    {
        return Ok(BranchInfo {
            is_repository: false,
            current_branch: None,
            detached_head: None,
            branches: Vec::new(),
            remote_branches: Vec::new(),
            is_dirty: false,
        });
    }

    let mut branches = read_ref_names(root, "refs/heads", "local branches")?;
    let remote_branches = read_ref_names(root, "refs/remotes", "remote branches")?
        .into_iter()
        .filter(|branch| !branch.ends_with("/HEAD"))
        .collect();

    let current_output = git(root, ["symbolic-ref", "--quiet", "--short", "HEAD"])?;
    let current_branch = current_output
        .status
        .success()
        .then(|| {
            String::from_utf8_lossy(&current_output.stdout)
                .trim()
                .to_string()
        })
        .filter(|branch| !branch.is_empty());
    if let Some(current) = current_branch.as_ref()
        && !branches.iter().any(|branch| branch == current)
    {
        branches.insert(0, current.clone());
    }

    let detached_head = if current_branch.is_none() {
        let output = git(root, ["rev-parse", "--short", "HEAD"])?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|head| !head.is_empty())
    } else {
        None
    };

    let status_output = git(root, ["status", "--porcelain", "--untracked-files=normal"])?;
    ensure_success(&status_output, "read git status")?;

    Ok(BranchInfo {
        is_repository: true,
        current_branch,
        detached_head,
        branches,
        remote_branches,
        is_dirty: !status_output.stdout.is_empty(),
    })
}

pub fn switch_branch(root: &Path, branch: &str, allow_dirty: bool) -> Result<BranchInfo, String> {
    let before = read_branch_info(root)?;
    ensure_change_allowed(&before, allow_dirty)?;
    if !before.branches.iter().any(|candidate| candidate == branch) {
        return Err(format!("Local branch not found: {branch}"));
    }
    if before.current_branch.as_deref() == Some(branch) {
        return Ok(before);
    }
    run_switch(root, ["switch", "--", branch])?;
    read_branch_info(root)
}

pub fn create_branch(root: &Path, branch: &str, allow_dirty: bool) -> Result<BranchInfo, String> {
    let branch = branch.trim();
    if branch.is_empty() {
        return Err("Enter a branch name.".to_string());
    }
    let before = read_branch_info(root)?;
    ensure_change_allowed(&before, allow_dirty)?;
    validate_new_branch_name(root, branch)?;
    if before.branches.iter().any(|candidate| candidate == branch) {
        return Err(format!("Local branch already exists: {branch}"));
    }
    run_switch(root, ["switch", "-c", branch])?;
    read_branch_info(root)
}

pub fn checkout_remote_branch(
    root: &Path,
    remote_branch: &str,
    allow_dirty: bool,
) -> Result<BranchInfo, String> {
    let before = read_branch_info(root)?;
    ensure_change_allowed(&before, allow_dirty)?;
    if !before
        .remote_branches
        .iter()
        .any(|candidate| candidate == remote_branch)
    {
        return Err(format!("Remote branch not found: {remote_branch}"));
    }
    run_switch(root, ["switch", "--track", remote_branch])?;
    read_branch_info(root)
}

pub fn read_graph(root: &Path, limit: usize) -> Result<Vec<String>, String> {
    let info = read_branch_info(root)?;
    if !info.is_repository {
        return Err("The selected workspace is not a Git repository.".to_string());
    }
    let limit = limit.clamp(1, 500).to_string();
    let output = git(
        root,
        [
            "log",
            "--graph",
            "--all",
            "--decorate=short",
            "--date=short",
            "--pretty=format:%h%x09%ad%x09%an%x09%d%x09%s",
            "-n",
            &limit,
        ],
    )?;
    if output.status.success() {
        return Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_string)
            .collect());
    }

    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    if detail.contains("does not have any commits yet") {
        Ok(Vec::new())
    } else if detail.is_empty() {
        Err("Git history is unavailable.".to_string())
    } else {
        Err(detail)
    }
}

fn read_ref_names(root: &Path, reference: &str, label: &str) -> Result<Vec<String>, String> {
    let output = git(
        root,
        ["for-each-ref", "--format=%(refname:short)", reference],
    )?;
    ensure_success(&output, &format!("list {label}"))?;
    let mut names = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    names.sort_by_key(|branch| branch.to_lowercase());
    Ok(names)
}

fn ensure_change_allowed(info: &BranchInfo, allow_dirty: bool) -> Result<(), String> {
    if !info.is_repository {
        return Err("The selected workspace is not a Git repository.".to_string());
    }
    if info.is_dirty && !allow_dirty {
        return Err(
            "The workspace has uncommitted changes. Confirm before changing branches.".to_string(),
        );
    }
    Ok(())
}

fn validate_new_branch_name(root: &Path, branch: &str) -> Result<(), String> {
    let output = git(root, ["check-ref-format", "--branch", branch])?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!("Invalid branch name: {branch}"))
    }
}

fn run_switch<const N: usize>(root: &Path, args: [&str; N]) -> Result<(), String> {
    let output = git(root, args)?;
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            "Git could not change branches.".to_string()
        } else {
            detail
        })
    }
}

fn ensure_success(output: &std::process::Output, action: &str) -> Result<(), String> {
    if output.status.success() {
        Ok(())
    } else {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if detail.is_empty() {
            format!("Git could not {action}.")
        } else {
            detail
        })
    }
}

fn git<const N: usize>(root: &Path, args: [&str; N]) -> Result<std::process::Output, String> {
    Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|error| format!("failed to run git: {error}"))
}

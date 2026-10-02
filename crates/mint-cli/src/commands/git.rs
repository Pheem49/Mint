use std::path::Path;

use anyhow::{Result, bail};
use clap::Subcommand;
use crossterm::tty::IsTty;
use mint_core::{
    BranchChange, BranchChangeOutcome, BranchInfo, change_branch, read_branch_info, read_graph,
};

use crate::{DIM, MINT, RESET, confirm};

#[derive(Debug, Subcommand)]
pub enum GitCommand {
    /// Show the active branch, working tree state, and known refs.
    Status {
        /// Print the branch state as JSON.
        #[arg(long)]
        json: bool,
    },
    /// Print local and locally known remote branches.
    List,
    /// Switch to an existing local branch.
    Switch {
        branch: String,
        /// Allow the change when the workspace has uncommitted changes.
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Create a branch from the current HEAD and switch to it.
    Create {
        branch: String,
        /// Allow the change when the workspace has uncommitted changes.
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Create and switch to a local tracking branch from a known remote ref.
    Track {
        remote_branch: String,
        /// Allow the change when the workspace has uncommitted changes.
        #[arg(long)]
        allow_dirty: bool,
    },
    /// Print decorated repository history as a graph.
    Graph {
        /// Number of commits to show, from 1 to 500.
        #[arg(long, default_value_t = 120)]
        limit: usize,
    },
}

pub fn handle_git(command: GitCommand) -> Result<()> {
    let root = std::env::current_dir()?;
    match command {
        GitCommand::Status { json } => print_status(&root, json),
        GitCommand::List => print_list(&root),
        GitCommand::Switch {
            branch,
            allow_dirty,
        } => {
            let info = apply_branch_change(
                &root,
                BranchChange::Switch {
                    branch: branch.clone(),
                },
                allow_dirty,
                &format!("Switch to {branch}?"),
            )?;
            print_changed(&info)
        }
        GitCommand::Create {
            branch,
            allow_dirty,
        } => {
            let info = apply_branch_change(
                &root,
                BranchChange::Create {
                    branch: branch.clone(),
                },
                allow_dirty,
                &format!("Create and switch to {branch}?"),
            )?;
            print_changed(&info)
        }
        GitCommand::Track {
            remote_branch,
            allow_dirty,
        } => {
            let info = apply_branch_change(
                &root,
                BranchChange::Track {
                    remote_branch: remote_branch.clone(),
                },
                allow_dirty,
                &format!("Create a local tracking branch from {remote_branch}?"),
            )?;
            print_changed(&info)
        }
        GitCommand::Graph { limit } => {
            let graph = read_graph(&root, limit).map_err(anyhow::Error::msg)?;
            if graph.is_empty() {
                println!("{DIM}No commits yet.{RESET}");
            } else {
                println!("{}", graph.join("\n"));
            }
            Ok(())
        }
    }
}

fn print_status(root: &Path, json: bool) -> Result<()> {
    let info = read_branch_info(root).map_err(anyhow::Error::msg)?;
    if json {
        println!("{}", serde_json::to_string_pretty(&info)?);
        return Ok(());
    }
    print_info(&info);
    Ok(())
}

fn print_list(root: &Path) -> Result<()> {
    let info = read_branch_info(root).map_err(anyhow::Error::msg)?;
    ensure_repository(&info)?;
    println!("Local branches");
    for branch in &info.branches {
        let marker = if info.current_branch.as_deref() == Some(branch) {
            format!("{MINT}*{RESET}")
        } else {
            " ".to_string()
        };
        println!("  {marker} {branch}");
    }
    if !info.remote_branches.is_empty() {
        println!("\nRemote branches {DIM}(locally known){RESET}");
        for branch in &info.remote_branches {
            println!("    {branch}");
        }
    }
    Ok(())
}

fn print_changed(info: &BranchInfo) -> Result<()> {
    ensure_repository(info)?;
    let branch = info.current_branch.as_deref().unwrap_or("detached HEAD");
    println!("{MINT}Switched to {branch}{RESET}");
    Ok(())
}

fn print_info(info: &BranchInfo) {
    if !info.is_repository {
        println!("Not a Git repository.");
        return;
    }
    let branch = info
        .current_branch
        .as_deref()
        .or(info.detached_head.as_deref())
        .unwrap_or("No branch");
    let state = if info.is_dirty { "dirty" } else { "clean" };
    println!("Branch: {branch}");
    println!("Working tree: {state}");
    println!("Local branches: {}", info.branches.len());
    println!(
        "Remote branches: {} {DIM}(locally known){RESET}",
        info.remote_branches.len()
    );
}

fn apply_branch_change(
    root: &Path,
    change: BranchChange,
    confirmed_dirty_workspace: bool,
    action: &str,
) -> Result<BranchInfo> {
    match change_branch(root, &change, confirmed_dirty_workspace).map_err(anyhow::Error::msg)? {
        BranchChangeOutcome::Changed { info } => Ok(info),
        BranchChangeOutcome::ConfirmationRequired { .. } => {
            confirm_and_change(root, change, action)
        }
    }
}

fn confirm_and_change(root: &Path, change: BranchChange, action: &str) -> Result<BranchInfo> {
    if !std::io::stdin().is_tty() || !std::io::stdout().is_tty() {
        bail!("Workspace has uncommitted changes. Re-run with --allow-dirty to confirm.");
    }
    if confirm(&format!("Workspace has uncommitted changes. {action}"))? {
        match change_branch(root, &change, true).map_err(anyhow::Error::msg)? {
            BranchChangeOutcome::Changed { info } => Ok(info),
            BranchChangeOutcome::ConfirmationRequired { .. } => {
                bail!("Workspace still requires confirmation.")
            }
        }
    } else {
        bail!("Branch change cancelled.")
    }
}

fn ensure_repository(info: &BranchInfo) -> Result<()> {
    if info.is_repository {
        Ok(())
    } else {
        bail!("The selected workspace is not a Git repository.")
    }
}

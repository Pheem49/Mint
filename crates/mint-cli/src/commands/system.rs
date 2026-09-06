use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use mint_core::{
    Capability, assert_path_capability, classify_shell_command, create_folder, find_paths,
    load_config, run_shell_command, save_config,
};

use crate::{actions, confirm};

#[derive(Debug, Subcommand)]
pub enum SafetyCommand {
    /// Classify a shell command before execution.
    Shell {
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Check whether a path is readable or writable.
    Path {
        path: PathBuf,
        #[arg(long)]
        write: bool,
    },
    /// Manage persistent "always allow"/"always deny" agent approval rules.
    Permissions {
        #[command(subcommand)]
        command: PermissionsCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum PermissionsCommand {
    /// List saved permission rules.
    List,
    /// Remove a saved permission rule by its index (see `list`).
    Remove { index: usize },
}

#[derive(Debug, Subcommand)]
pub enum FilesCommand {
    Find {
        query: String,
        #[arg(long, default_value_t = 20)]
        limit: usize,
        #[arg(long)]
        root: Vec<PathBuf>,
    },
    CreateFolder {
        path: PathBuf,
    },
}

pub fn handle_run(approve: bool, cwd: PathBuf, command: Vec<String>) -> Result<()> {
    let cmd_str = command.join(" ");
    let approved = approve || confirm(&format!("Execute: {cmd_str}"))?;
    let output = run_shell_command(&cmd_str, &cwd, approved, &load_config()?, None)?;
    actions::print_shell_output(&output);
    if !output.success {
        anyhow::bail!(
            "shell command exited with status {}",
            output
                .status
                .map_or_else(|| "unknown".into(), |status| status.to_string())
        );
    }
    Ok(())
}

pub fn handle_open(target: String) -> Result<()> {
    actions::open_system_handler(&target)
}

pub fn handle_open_app(name: String) -> Result<()> {
    actions::launch_desktop_app(&name)
}

pub fn handle_preview(path: PathBuf) -> Result<()> {
    actions::preview_file(&path)
}

pub fn handle_read_file(path: PathBuf) -> Result<()> {
    actions::read_file_content(&path)
}

pub fn handle_read_folder(path: PathBuf) -> Result<()> {
    actions::read_folder_content(&path)
}

pub fn handle_safety(command: SafetyCommand) -> Result<()> {
    match command {
        SafetyCommand::Shell { command } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&classify_shell_command(&command.join(" ")))?
            );
        }
        SafetyCommand::Path { path, write } => {
            let capability = if write {
                Capability::Write
            } else {
                Capability::Read
            };
            println!(
                "{}",
                assert_path_capability(&path, capability, &load_config()?)?.display()
            );
        }
        SafetyCommand::Permissions { command } => match command {
            PermissionsCommand::List => {
                let config = load_config()?;
                if config.permission_rules.is_empty() {
                    println!("No saved permission rules.");
                } else {
                    for (index, rule) in config.permission_rules.iter().enumerate() {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "index": index,
                                "rule": rule,
                            }))?
                        );
                    }
                }
            }
            PermissionsCommand::Remove { index } => {
                let mut config = load_config()?;
                if index >= config.permission_rules.len() {
                    anyhow::bail!(
                        "no permission rule at index {index} (have {})",
                        config.permission_rules.len()
                    );
                }
                let removed = config.permission_rules.remove(index);
                save_config(&config)?;
                println!("Removed: {}", serde_json::to_string_pretty(&removed)?);
            }
        },
    }
    Ok(())
}

pub fn handle_files(command: FilesCommand) -> Result<()> {
    let config = load_config()?;
    match command {
        FilesCommand::Find {
            query,
            limit,
            mut root,
        } => {
            if root.is_empty() {
                root.push(std::env::current_dir()?);
                if let Some(home) = dirs::home_dir() {
                    root.push(home);
                }
            }
            println!(
                "{}",
                serde_json::to_string_pretty(&find_paths(&query, &root, limit, &config))?
            );
        }
        FilesCommand::CreateFolder { path } => {
            println!("{}", create_folder(&path, &config)?.display())
        }
    }
    Ok(())
}

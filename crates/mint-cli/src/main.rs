use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use mint_core::{
    Capability, ChatRequest, MemoryStore, TaskStore, assert_path_capability,
    classify_shell_command, config_path, create_folder, execute_native_plugin, find_paths,
    load_config, native_plugins, orchestrate_chat, set_config_value,
};

#[derive(Debug, Parser)]
#[command(name = "mint", version, about = "Mint native CLI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Display the current native runtime status.
    Status,
    /// Inspect the local Mint configuration.
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
    /// List AI providers that are configured locally.
    Providers,
    /// Send one message through the configured Rust AI provider.
    Chat {
        message: String,
        #[arg(long, default_value = "")]
        system: String,
    },
    /// Inspect or update local long-term memory.
    Memory {
        #[command(subcommand)]
        command: MemoryCommand,
    },
    /// Manage durable native tasks.
    Task {
        #[command(subcommand)]
        command: TaskCommand,
    },
    /// Search and create local folders through the native safety policy.
    Files {
        #[command(subcommand)]
        command: FilesCommand,
    },
    /// Run built-in native plugins.
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },
    /// Inspect native safety policy decisions.
    Safety {
        #[command(subcommand)]
        command: SafetyCommand,
    },
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Print the config file path.
    Path,
    /// Print the config as JSON.
    Show,
    /// Set one JSON-compatible config value.
    Set { key: String, value: String },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    Add { description: String },
    List,
    Show { id: String },
    ClearCompleted,
}

#[derive(Debug, Subcommand)]
enum FilesCommand {
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

#[derive(Debug, Subcommand)]
enum PluginCommand {
    List,
    Run { name: String, instruction: String },
}

#[derive(Debug, Subcommand)]
enum SafetyCommand {
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
}

#[derive(Debug, Subcommand)]
enum MemoryCommand {
    /// Read one profile value.
    Get { key: String },
    /// Store one profile value.
    Set { key: String, value: String },
    /// Show recent chat interactions.
    Recent {
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    match Cli::parse().command.unwrap_or(Command::Status) {
        Command::Status => {
            let config = load_config()?;
            println!("Mint native CLI");
            println!("provider: {}", config.ai_provider);
            println!("model: {}", active_model(&config.ai_provider, &config));
            println!("config: {}", config_path()?.display());
        }
        Command::Config { command } => match command {
            ConfigCommand::Path => println!("{}", config_path()?.display()),
            ConfigCommand::Show => println!("{}", serde_json::to_string_pretty(&load_config()?)?),
            ConfigCommand::Set { key, value } => {
                let value = serde_json::from_str(&value)
                    .unwrap_or_else(|_| serde_json::Value::String(value));
                println!(
                    "{}",
                    serde_json::to_string_pretty(&set_config_value(&key, value)?)?
                );
            }
        },
        Command::Providers => {
            for provider in load_config()?.available_providers() {
                println!("{provider}");
            }
        }
        Command::Chat { message, system } => {
            let response = orchestrate_chat(
                &load_config()?,
                &ChatRequest {
                    message: message.clone(),
                    system_instruction: system,
                },
            )
            .await?;
            println!("{}", response.text);
        }
        Command::Memory { command } => {
            let memory = MemoryStore::open_default()?;
            match command {
                MemoryCommand::Get { key } => {
                    println!("{}", memory.get_profile(&key)?.unwrap_or_default());
                }
                MemoryCommand::Set { key, value } => {
                    memory.set_profile(&key, &value)?;
                    println!("stored");
                }
                MemoryCommand::Recent { limit } => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&memory.recent_interactions(limit)?)?
                    );
                }
            }
        }
        Command::Safety { command } => match command {
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
        },
        Command::Task { command } => {
            let tasks = TaskStore::open_default()?;
            match command {
                TaskCommand::Add { description } => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&tasks.add(description)?)?
                    );
                }
                TaskCommand::List => println!("{}", serde_json::to_string_pretty(&tasks.list()?)?),
                TaskCommand::Show { id } => {
                    println!("{}", serde_json::to_string_pretty(&tasks.get(&id)?)?)
                }
                TaskCommand::ClearCompleted => println!("{}", tasks.clear_completed()?),
            }
        }
        Command::Files { command } => {
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
        }
        Command::Plugin { command } => match command {
            PluginCommand::List => println!("{}", serde_json::to_string_pretty(&native_plugins())?),
            PluginCommand::Run { name, instruction } => {
                println!("{}", execute_native_plugin(&name, &instruction)?)
            }
        },
    }
    Ok(())
}

fn active_model<'a>(provider: &str, config: &'a mint_core::MintConfig) -> &'a str {
    match provider {
        "anthropic" => &config.anthropic_model,
        "openai" => &config.openai_model,
        "huggingface" => &config.hf_model,
        "local_openai" => &config.local_model_name,
        "ollama" => &config.ollama_model,
        _ => &config.gemini_model,
    }
}

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

use mint_core::{
    Capability, ChatRequest, MemoryStore, assert_path_capability, classify_shell_command,
    config_path, load_config, send_chat,
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
        },
        Command::Providers => {
            for provider in load_config()?.available_providers() {
                println!("{provider}");
            }
        }
        Command::Chat { message, system } => {
            let response = send_chat(
                &load_config()?,
                &ChatRequest {
                    message: message.clone(),
                    system_instruction: system,
                },
            )
            .await?;
            MemoryStore::open_default()?.add_interaction(&message, &response.text)?;
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

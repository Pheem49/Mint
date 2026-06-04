use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use mint_core::{
    Capability, ChatRequest, CodeEdit, CodePatchHunk, KnowledgeStore, MemoryStore, MintConfig,
    TaskStore, apply_code_edits, assert_path_capability, build_code_patch, build_symbol_index,
    classify_shell_command, config_path, create_folder, execute_native_plugin, find_paths,
    index_semantic_code, initialize_config, inspect_code_plan, list_code_files, load_config,
    native_plugins, orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback,
    propose_code_edits, read_code_file, repository_summary, run_shell_command, search_code,
    search_semantic_code, set_config_value,
};

mod agent;
mod gmail;
mod image;
mod mcp;
mod onboard;
mod setup;
mod skills;
mod updater;

async fn run_code_agent_with_saved_image(
    task: &str,
    current_dir: &Path,
    config: &MintConfig,
    image_data_uri: Option<String>,
    options: agent::AgentOptions,
) -> Result<()> {
    let sent_image = image_data_uri.clone();
    agent::run_code_agent_with_options(task, current_dir, config, image_data_uri, options).await?;
    image::save_sent_image_after_send(sent_image.as_deref(), task);
    Ok(())
}

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
        #[arg(long)]
        image: Option<PathBuf>,
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
    /// Index and search native local text knowledge.
    Knowledge {
        #[command(subcommand)]
        command: KnowledgeCommand,
    },
    /// Inspect a code workspace through the native read-only code-agent tools.
    Code {
        #[command(subcommand)]
        command: CodeCommand,
    },
    /// Inspect native safety policy decisions.
    Safety {
        #[command(subcommand)]
        command: SafetyCommand,
    },
    /// Run one queued or supplied task through the native CLI agent.
    Agent { task: Option<String> },
    /// Launch the web UI and local API server.
    Web,
    /// Start only the local API server.
    Api {
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
    /// Manage configured MCP stdio servers.
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
    /// Configure Gmail OAuth.
    Gmail {
        #[command(subcommand)]
        command: GmailCommand,
    },
    /// Check or install the latest npm-distributed CLI.
    Update {
        #[arg(long)]
        check: bool,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        approve: bool,
    },
    /// Import, list, or delete persistent learned skill files.
    Learn {
        path: Option<PathBuf>,
        #[arg(long)]
        list: bool,
        #[arg(long)]
        delete: Option<String>,
    },
    /// Build a local source symbol index.
    Symbols {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Build or search semantic source embeddings.
    SemanticCode {
        #[command(subcommand)]
        command: SemanticCodeCommand,
    },
    /// Run a local shell command after explicit approval.
    Run {
        #[arg(long)]
        approve: bool,
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },
    /// Open a URL, file, or folder using the system default handler.
    Open { target: String },
    /// Launch a desktop program.
    OpenApp { name: String },
    /// Read the contents of a text file.
    ReadFile { path: PathBuf },
    /// List the contents of a directory.
    ReadFolder {
        #[arg(default_value = ".")]
        path: PathBuf,
    },
    /// Configure Mint for first use.
    Onboard,
    /// Interactively manage enabled agent tools.
    Setup,
}

#[derive(Debug, Subcommand)]
enum ConfigCommand {
    /// Create the native config file and fill missing runtime defaults.
    Init,
    /// Print the config file path.
    Path,
    /// Print the config as JSON.
    Show,
    /// Set one JSON-compatible config value.
    Set { key: String, value: String },
    /// Show configured native providers and integrations.
    Doctor,
}

#[derive(Debug, Subcommand)]
enum McpCommand {
    Add {
        name: String,
        command: String,
        #[arg(long, num_args = 0.., allow_hyphen_values = true)]
        args: Vec<String>,
        #[arg(long, num_args = 0..)]
        env: Vec<String>,
    },
    List,
    Remove {
        name: String,
    },
    Clear,
    Call {
        server: String,
        tool: String,
        #[arg(long, default_value = "{}")]
        arguments: String,
    },
}

#[derive(Debug, Subcommand)]
enum GmailCommand {
    Auth {
        #[arg(long)]
        no_open: bool,
        #[arg(long, default_value_t = 0)]
        port: u16,
    },
}

#[derive(Debug, Subcommand)]
enum SemanticCodeCommand {
    Index {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    Search {
        query: String,
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
enum TaskCommand {
    Add {
        description: String,
    },
    List,
    Show {
        id: String,
    },
    Pending,
    Resume,
    Update {
        id: String,
        status: String,
        #[arg(long)]
        result: Option<String>,
    },
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
enum KnowledgeCommand {
    Add {
        path: PathBuf,
    },
    List,
    Search {
        query: String,
        #[arg(long, default_value_t = 5)]
        limit: usize,
    },
}

#[derive(Debug, Subcommand)]
enum CodeCommand {
    /// Run the autonomous inspect, act, and verify code-agent loop.
    Agent {
        task: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Summarize source files while skipping build and dependency directories.
    Summary {
        #[arg(default_value = ".")]
        root: PathBuf,
    },
    /// List source files while skipping build and dependency directories.
    List {
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 100)]
        limit: usize,
    },
    /// Read a numbered source range.
    Read {
        path: PathBuf,
        #[arg(long, default_value_t = 1)]
        start: usize,
        #[arg(long, default_value_t = 200)]
        end: usize,
    },
    /// Search source text without invoking a shell command.
    Search {
        query: String,
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
    /// Print a bounded inspection-first plan. This never edits files or runs shell commands.
    Plan {
        task: String,
        #[arg(default_value = ".")]
        root: PathBuf,
        #[arg(long)]
        file: Vec<PathBuf>,
    },
    /// Preview a full file write and print its content-bound approval token.
    ProposeWrite {
        path: PathBuf,
        #[arg(long, conflicts_with = "from_file")]
        content: Option<String>,
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Apply exactly the full file write that was previously approved.
    ApplyWrite {
        path: PathBuf,
        #[arg(long, conflicts_with = "from_file")]
        content: Option<String>,
        #[arg(long)]
        from_file: Option<PathBuf>,
        #[arg(long)]
        approval_token: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Preview an exact text replacement and print its content-bound approval token.
    ProposePatch {
        path: PathBuf,
        old_text: String,
        new_text: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Apply exactly the text replacement that was previously approved.
    ApplyPatch {
        path: PathBuf,
        old_text: String,
        new_text: String,
        #[arg(long)]
        approval_token: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Preview multiple full file writes. Use TARGET=SOURCE for each edit.
    ProposeEdits {
        #[arg(long, required = true)]
        edit: Vec<String>,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
    /// Apply exactly the multi-file write proposal that was previously approved.
    ApplyEdits {
        #[arg(long, required = true)]
        edit: Vec<String>,
        #[arg(long)]
        approval_token: String,
        #[arg(long, default_value = ".")]
        root: PathBuf,
    },
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
    match Cli::parse().command {
        None => {
            run_interactive_chat().await?;
        }
        Some(cmd) => match cmd {
            Command::Status => {
                let config = load_config()?;
                println!("Mint native CLI");
                println!("provider: {}", config.ai_provider);
                println!("model: {}", active_model(&config.ai_provider, &config));
                println!("config: {}", config_path()?.display());
            }
            Command::Config { command } => match command {
                ConfigCommand::Init => {
                    initialize_config()?;
                    println!("{}", config_path()?.display());
                }
                ConfigCommand::Path => println!("{}", config_path()?.display()),
                ConfigCommand::Show => {
                    println!("{}", serde_json::to_string_pretty(&load_config()?)?)
                }
                ConfigCommand::Set { key, value } => {
                    let value = serde_json::from_str(&value)
                        .unwrap_or_else(|_| serde_json::Value::String(value));
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&set_config_value(&key, value)?)?
                    );
                }
                ConfigCommand::Doctor => {
                    let config = load_config()?;
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&serde_json::json!({
                            "configPath": config_path()?,
                            "activeProvider": config.ai_provider,
                            "availableProviders": config.available_providers(),
                            "headlessTaskQueue": config.extra["enableHeadlessTaskQueue"],
                            "updater": {
                                "enabled": config.extra["enableAutoUpdate"],
                                "endpointConfigured": configured(&config, &["updaterEndpoint"]),
                                "publicKeyConfigured": configured(&config, &["updaterPublicKey"]),
                                "automaticInstall": false,
                            },
                            "channels": {
                                "telegram": configured(&config, &["telegramBotToken"]),
                                "discord": configured(&config, &["discordBotToken"]),
                                "slack": configured(&config, &["slackBotToken", "slackAppToken"]),
                                "line": configured(&config, &["lineChannelAccessToken", "lineChannelSecret"]),
                                "whatsappCloud": configured(&config, &["whatsappCloudAccessToken", "whatsappPhoneNumberId", "whatsappVerifyToken"]),
                            },
                            "plugins": {
                                "gmail": configured(&config, &["gmailClientId", "gmailClientSecret", "gmailRefreshToken"]),
                                "googleCalendar": configured(&config, &["googleCalendarClientId", "googleCalendarClientSecret", "googleCalendarRefreshToken"]),
                                "notion": configured(&config, &["notionApiKey"]),
                            }
                        }))?
                    );
                }
            },
            Command::Providers => {
                for provider in load_config()?.available_providers() {
                    println!("{provider}");
                }
            }
            Command::Agent { task } => {
                run_cli_agent_task(task).await?;
            }
            Command::Web => {
                launch_mint_target("web".into()).await?;
            }
            Command::Api { port } => {
                mint_core::start_api_server(port).await?;
            }
            Command::Mcp { command } => match command {
                McpCommand::Add {
                    name,
                    command,
                    args,
                    env,
                } => {
                    mcp::add(&name, &command, args, env)?;
                    println!("Added MCP server: {name}");
                }
                McpCommand::List => println!("{}", serde_json::to_string_pretty(&mcp::list()?)?),
                McpCommand::Remove { name } => {
                    println!(
                        "{}",
                        if mcp::remove(&name)? {
                            "removed"
                        } else {
                            "not found"
                        }
                    )
                }
                McpCommand::Clear => {
                    mcp::clear()?;
                    println!("cleared");
                }
                McpCommand::Call {
                    server,
                    tool,
                    arguments,
                } => println!(
                    "{}",
                    serde_json::to_string_pretty(&mcp::call(
                        &server,
                        &tool,
                        serde_json::from_str(&arguments)?
                    )?)?
                ),
            },
            Command::Gmail { command } => match command {
                GmailCommand::Auth { no_open, port } => gmail::auth(no_open, port).await?,
            },
            Command::Update {
                check,
                dry_run,
                approve,
            } => updater::run(check, dry_run, approve)?,
            Command::Learn { path, list, delete } => {
                let memory = MemoryStore::open_default()?;
                if list {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&memory.learned_skills(100)?)?
                    );
                } else if let Some(identifier) = delete {
                    println!("{}", memory.delete_learned_skill(&identifier)?);
                } else if let Some(path) = path {
                    println!("{}", serde_json::to_string_pretty(&skills::learn(&path)?)?);
                } else {
                    anyhow::bail!("use mint learn <path>, --list, or --delete <id|path|name>");
                }
            }
            Command::Symbols { root, limit } => println!(
                "{}",
                serde_json::to_string_pretty(&build_symbol_index(&root, limit, &load_config()?)?)?
            ),
            Command::SemanticCode { command } => match command {
                SemanticCodeCommand::Index { root } => println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &index_semantic_code(&root, &load_config()?).await?
                    )?
                ),
                SemanticCodeCommand::Search { query, root, limit } => println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &search_semantic_code(&root, &query, limit, &load_config()?).await?
                    )?
                ),
            },
            Command::Chat {
                message,
                system,
                image,
            } => {
                let image_data_uri = image
                    .as_deref()
                    .map(image::load_image_as_data_uri)
                    .transpose()?;
                let sent_image = image_data_uri.clone();
                if system.trim().is_empty() {
                    run_code_agent_with_saved_image(
                        &message,
                        &std::env::current_dir()?,
                        &load_config()?,
                        image_data_uri,
                        agent::AgentOptions::default(),
                    )
                    .await?;
                } else {
                    let (response, _) = orchestrate_chat_with_fallback(
                        &load_config()?,
                        &ChatRequest {
                            message: message.clone(),
                            system_instruction: system,
                            image_data_uri,
                            audio_data_uri: None,
                        },
                    )
                    .await?;
                    image::save_sent_image_after_send(sent_image.as_deref(), &message);
                    println!("{}", response.text);
                }
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
            Command::Run {
                approve,
                cwd,
                command,
            } => {
                let output = run_shell_command(&command.join(" "), &cwd, approve, &load_config()?)?;
                print_shell_output(&output);
                if !output.success {
                    anyhow::bail!(
                        "shell command exited with status {}",
                        output
                            .status
                            .map_or_else(|| "unknown".into(), |status| status.to_string())
                    );
                }
            }
            Command::Task { command } => {
                let tasks = TaskStore::open_default()?;
                match command {
                    TaskCommand::Add { description } => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&tasks.add(description)?)?
                        );
                    }
                    TaskCommand::List => {
                        println!("{}", serde_json::to_string_pretty(&tasks.list()?)?)
                    }
                    TaskCommand::Show { id } => {
                        println!("{}", serde_json::to_string_pretty(&tasks.get(&id)?)?)
                    }
                    TaskCommand::Pending => {
                        println!("{}", serde_json::to_string_pretty(&tasks.pending()?)?)
                    }
                    TaskCommand::Resume => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&tasks.resume_running()?)?
                        )
                    }
                    TaskCommand::Update { id, status, result } => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&tasks.update_status(
                                &id,
                                &status,
                                result.map(serde_json::Value::String)
                            )?)?
                        )
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
                            serde_json::to_string_pretty(&find_paths(
                                &query, &root, limit, &config
                            ))?
                        );
                    }
                    FilesCommand::CreateFolder { path } => {
                        println!("{}", create_folder(&path, &config)?.display())
                    }
                }
            }
            Command::Plugin { command } => match command {
                PluginCommand::List => {
                    println!("{}", serde_json::to_string_pretty(&native_plugins())?)
                }
                PluginCommand::Run { name, instruction } => {
                    println!(
                        "{}",
                        execute_native_plugin(&load_config()?, &name, &instruction).await?
                    )
                }
            },
            Command::Knowledge { command } => {
                let store = KnowledgeStore::open_default()?;
                match command {
                    KnowledgeCommand::Add { path } => {
                        println!("{}", store.index_file(&path, &load_config()?)?)
                    }
                    KnowledgeCommand::List => {
                        println!("{}", serde_json::to_string_pretty(&store.list_sources()?)?)
                    }
                    KnowledgeCommand::Search { query, limit } => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&store.search(&query, limit)?)?
                        )
                    }
                }
            }
            Command::Code { command } => {
                let config = load_config()?;
                match command {
                    CodeCommand::Agent { task, root } => {
                        agent::run_code_agent(&task, &root, &config).await?;
                    }
                    CodeCommand::Summary { root } => println!(
                        "{}",
                        serde_json::to_string_pretty(&repository_summary(&root, &config)?)?
                    ),
                    CodeCommand::List { root, limit } => println!(
                        "{}",
                        serde_json::to_string_pretty(&list_code_files(&root, limit, &config)?)?
                    ),
                    CodeCommand::Read { path, start, end } => {
                        println!("{}", read_code_file(&path, start, end, &config)?)
                    }
                    CodeCommand::Search { query, root, limit } => println!(
                        "{}",
                        serde_json::to_string_pretty(&search_code(&root, &query, limit, &config)?)?
                    ),
                    CodeCommand::Plan { task, root, file } => println!(
                        "{}",
                        serde_json::to_string_pretty(&inspect_code_plan(
                            task, &root, file, &config
                        )?)?
                    ),
                    CodeCommand::ProposeWrite {
                        path,
                        content,
                        from_file,
                        root,
                    } => println!(
                        "{}",
                        serde_json::to_string_pretty(&propose_code_edits(
                            &root,
                            &[CodeEdit {
                                path,
                                content: edit_content(content, from_file, &config)?,
                            }],
                            &config,
                        )?)?
                    ),
                    CodeCommand::ApplyWrite {
                        path,
                        content,
                        from_file,
                        approval_token,
                        root,
                    } => println!(
                        "{}",
                        serde_json::to_string_pretty(&apply_code_edits(
                            &root,
                            &[CodeEdit {
                                path,
                                content: edit_content(content, from_file, &config)?,
                            }],
                            &approval_token,
                            &config,
                        )?)?
                    ),
                    CodeCommand::ProposePatch {
                        path,
                        old_text,
                        new_text,
                        root,
                    } => println!(
                        "{}",
                        serde_json::to_string_pretty(&propose_code_edits(
                            &root,
                            &[build_code_patch(
                                &root,
                                path,
                                &[CodePatchHunk { old_text, new_text }],
                                &config,
                            )?],
                            &config,
                        )?)?
                    ),
                    CodeCommand::ApplyPatch {
                        path,
                        old_text,
                        new_text,
                        approval_token,
                        root,
                    } => println!(
                        "{}",
                        serde_json::to_string_pretty(&apply_code_edits(
                            &root,
                            &[build_code_patch(
                                &root,
                                path,
                                &[CodePatchHunk { old_text, new_text }],
                                &config,
                            )?],
                            &approval_token,
                            &config,
                        )?)?
                    ),
                    CodeCommand::ProposeEdits { edit, root } => println!(
                        "{}",
                        serde_json::to_string_pretty(&propose_code_edits(
                            &root,
                            &file_edits(&edit, &config)?,
                            &config,
                        )?)?
                    ),
                    CodeCommand::ApplyEdits {
                        edit,
                        approval_token,
                        root,
                    } => println!(
                        "{}",
                        serde_json::to_string_pretty(&apply_code_edits(
                            &root,
                            &file_edits(&edit, &config)?,
                            &approval_token,
                            &config,
                        )?)?
                    ),
                }
            }
            Command::Open { target } => {
                open_system_handler(&target)?;
            }
            Command::OpenApp { name } => {
                launch_desktop_app(&name)?;
            }
            Command::ReadFile { path } => {
                read_file_content(&path)?;
            }
            Command::ReadFolder { path } => {
                read_folder_content(&path)?;
            }
            Command::Onboard => {
                onboard::run().await?;
            }
            Command::Setup => {
                if let Some(target) = setup::run().await? {
                    launch_mint_target(target).await?;
                }
            }
        },
    }
    Ok(())
}

async fn launch_mint_target(target: String) -> Result<()> {
    match target.as_str() {
        "cli" => {
            println!("\x1b[32mStarting CLI Interactive Chat Assistant...\x1b[0m\n");
            run_interactive_chat().await?;
        }
        "desktop" => {
            println!("\x1b[32mLaunching Desktop App (npm run tauri:dev)... \x1b[0m\n");
            let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .ok_or_else(|| anyhow::anyhow!("Failed to find project root directory"))?;
            std::process::Command::new("npm")
                .current_dir(project_root)
                .args(&["run", "tauri:dev"])
                .status()
                .map_err(|e| anyhow::anyhow!("Failed to run desktop app: {e}"))?;
        }
        "web" => {
            println!("\x1b[32mLaunching Web App (vite) in background... (Vite Dev UI at http://localhost:9000)\x1b[0m");
            let project_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .and_then(|p| p.parent())
                .ok_or_else(|| anyhow::anyhow!("Failed to find project root directory"))?;
            std::process::Command::new("npm")
                .current_dir(project_root)
                .args(&["run", "dev:web"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to launch web app: {e}"))?;

            println!("\x1b[32mStarting local API server in foreground on port 3000...\x1b[0m\n");
            println!("\x1b[36m👉 Please open \x1b[1;36mhttp://localhost:9000\x1b[0m\x1b[36m in your web browser to access the Mint Web UI!\x1b[0m\n");
            mint_core::start_api_server(3000).await?;
        }
        _ => {}
    }

    Ok(())
}

struct ActionStreamFilter {
    buffer: String,
    in_action: bool,
    action_text: String,
    actions: Vec<String>,
}

impl ActionStreamFilter {
    fn new() -> Self {
        Self {
            buffer: String::new(),
            in_action: false,
            action_text: String::new(),
            actions: Vec::new(),
        }
    }

    fn process_chunk(&mut self, chunk: &str, mut print_fn: impl FnMut(&str)) {
        for c in chunk.chars() {
            if self.in_action {
                if c == ']' {
                    self.in_action = false;
                    self.actions.push(self.action_text.clone());
                    self.action_text.clear();
                } else {
                    self.action_text.push(c);
                }
            } else {
                self.buffer.push(c);
                let action_prefix = "[ACTION:";
                if action_prefix.starts_with(&self.buffer) {
                    if self.buffer == action_prefix {
                        self.in_action = true;
                        self.buffer.clear();
                    }
                } else {
                    if let Some(pos) = self.buffer.find('[') {
                        print_fn(&self.buffer[..pos]);
                        let new_buf = self.buffer[pos..].to_string();
                        self.buffer = new_buf;
                        if !action_prefix.starts_with(&self.buffer) {
                            print_fn(&self.buffer);
                            self.buffer.clear();
                        }
                    } else {
                        print_fn(&self.buffer);
                        self.buffer.clear();
                    }
                }
            }
        }
    }

    fn finalize(&mut self, mut print_fn: impl FnMut(&str)) -> Vec<String> {
        if !self.buffer.is_empty() {
            print_fn(&self.buffer);
            self.buffer.clear();
        }
        if self.in_action {
            print_fn(&format!("[ACTION:{}", self.action_text));
            self.action_text.clear();
            self.in_action = false;
        }
        std::mem::take(&mut self.actions)
    }
}

async fn run_cli_agent_task(task: Option<String>) -> Result<()> {
    let store = TaskStore::open_default()?;
    let task = match task {
        Some(description) => store.add(description)?,
        None => store
            .pending()?
            .ok_or_else(|| anyhow::anyhow!("no pending task is available"))?,
    };
    store.update_status(&task.id, "running", None)?;
    println!("Running task {}: {}", task.id, task.description);
    match agent::run_code_agent(
        &task.description,
        &std::env::current_dir()?,
        &load_config()?,
    )
    .await
    {
        Ok(result) => {
            store.update_status(
                &task.id,
                "completed",
                Some(serde_json::json!({
                    "summary": result.summary,
                    "verification": result.verification,
                })),
            )?;
            println!("Task completed: {}", task.id);
            Ok(())
        }
        Err(error) => {
            store.fail_with_retry(&task.id, &error.to_string())?;
            Err(error)
        }
    }
}

fn execute_action(action: &str, config: &MintConfig) -> Result<()> {
    let trimmed = action.trim();
    if let Some((cmd, args)) = trimmed.split_once(' ') {
        let cmd = cmd.trim();
        let args = args.trim();
        match cmd {
            "open" => {
                println!("\x1b[33m⚡ System Action: Opening {}...\x1b[0m\n", args);
                open_system_handler(args)?;
            }
            "open-app" => {
                println!(
                    "\x1b[33m⚡ System Action: Launching App {}...\x1b[0m\n",
                    args
                );
                launch_desktop_app(args)?;
            }
            "read-file" => {
                println!(
                    "\x1b[33m⚡ System Action: Reading File {}...\x1b[0m\n",
                    args
                );
                let path = PathBuf::from(args);
                read_file_content(&path)?;
            }
            "read-folder" => {
                println!(
                    "\x1b[33m⚡ System Action: Reading Folder {}...\x1b[0m\n",
                    args
                );
                let path = PathBuf::from(args);
                read_folder_content(&path)?;
            }
            "run-shell" => {
                println!("\x1b[33mSystem Action: Run local shell command\x1b[0m");
                println!("  {args}");
                if confirm_shell_execution()? {
                    let output = run_shell_command(args, &std::env::current_dir()?, true, config)?;
                    print_shell_output(&output);
                } else {
                    println!("Shell command cancelled.\n");
                }
            }
            _ => {
                println!(
                    "\x1b[31m⚠️ Unknown System Action: {} with args {}\x1b[0m\n",
                    cmd, args
                );
            }
        }
    } else {
        match trimmed {
            "read-folder" => {
                println!("\x1b[33m⚡ System Action: Reading Folder . ...\x1b[0m\n");
                read_folder_content(&PathBuf::from("."))?;
            }
            _ => {
                println!("\x1b[31m⚠️ Invalid Action format: {}\x1b[0m\n", trimmed);
            }
        }
    }
    Ok(())
}

/// Per-session mutable state for the interactive chat loop.
struct InteractiveSession {
    config: MintConfig,
    current_dir: PathBuf,
    fast_mode: bool,
    pending_image: Option<String>, // base64 data URI
}

struct InteractiveInput {
    text: String,
    pasted_image: Option<String>,
}

/// What the slash-command router wants the loop to do next.
enum SlashResult {
    /// Command handled — continue loop without sending to agent.
    Handled,
    /// Pass this (possibly modified) query to the agent.
    ForwardToAgent(String),
    /// Break out of the loop.
    Exit,
}

/// Route `/…` commands. Returns `None` if the input is not a slash command.
async fn handle_slash_command(
    session: &mut InteractiveSession,
    query: &str,
) -> Option<SlashResult> {
    let trimmed = query.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    // Split into command word and optional rest
    let (cmd, rest) = trimmed
        .split_once(char::is_whitespace)
        .map(|(c, r)| (c, r.trim()))
        .unwrap_or((trimmed, ""));

    match cmd {
        "/help" => {
            println!("\n\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
            println!("\x1b[32m  Mint Interactive Commands\x1b[0m");
            println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
            let commands = [
                ("/help", "Show this help"),
                ("/fast [on|off]", "Toggle fast mode (hide thinking traces)"),
                ("/models [name]", "List providers or switch provider"),
                ("/clear", "Clear conversation history"),
                ("/cd <path>", "Change workspace directory"),
                ("/image <path> [prompt]", "Attach image from file"),
                ("/paste [prompt]", "Attach image from clipboard"),
                ("Ctrl+V", "Paste clipboard image as [Image #1]"),
                ("/learn <path>", "Import a persistent .md or .txt skill"),
                ("/memory list", "Show recent interactions"),
                ("/memory clear", "Clear all interactions"),
                ("/memory get <key>", "Read a profile value"),
                ("/memory set <key> <val>", "Store a profile value"),
                ("/stats", "Show session statistics"),
                ("/exit | /quit", "Exit Mint"),
                ("/code <task>", "Run in code-agent mode"),
            ];
            for (cmd_name, desc) in &commands {
                println!("  \x1b[33m{:<30}\x1b[0m {}", cmd_name, desc);
            }
            println!();
            Some(SlashResult::Handled)
        }

        "/fast" => {
            session.fast_mode = match rest {
                "off" => false,
                "on" => true,
                "" => !session.fast_mode,
                _ => {
                    println!("\x1b[33m/fast usage: /fast [on|off]\x1b[0m");
                    return Some(SlashResult::Handled);
                }
            };
            if session.fast_mode {
                println!("\x1b[90m[Fast] mode ON — thinking traces hidden\x1b[0m\n");
            } else {
                println!("\x1b[90m[Fast] mode OFF\x1b[0m\n");
            }
            Some(SlashResult::Handled)
        }

        "/models" => {
            if rest.is_empty() {
                println!("\n\x1b[36mConfigured providers:\x1b[0m");
                for p in session.config.available_providers() {
                    let active = if p == session.config.ai_provider.as_str() {
                        " \x1b[32m← active\x1b[0m"
                    } else {
                        ""
                    };
                    println!("  {p}{active}");
                }
                println!();
            } else {
                session.config.ai_provider = rest.to_owned();
                println!(
                    "\x1b[90mSwitched to provider: {}\x1b[0m\n",
                    session.config.ai_provider
                );
            }
            Some(SlashResult::Handled)
        }

        "/clear" | "/reset" => {
            println!("Clear conversation history? [y/N] ");
            if let Ok(true) = confirm("Clear conversation history? [y/N] ") {
                if let Ok(memory) = MemoryStore::open_default() {
                    match memory.clear_interactions() {
                        Ok(count) => println!("\x1b[90mCleared {count} interactions.\x1b[0m"),
                        Err(error) => println!("\x1b[31mMemory error: {error}\x1b[0m"),
                    }
                }
                println!("\x1b[90mConversation context cleared.\x1b[0m\n");
            }
            Some(SlashResult::Handled)
        }

        "/cd" => {
            if rest.is_empty() {
                println!("\x1b[33m/cd requires a path\x1b[0m\n");
            } else {
                let new_dir = PathBuf::from(rest);
                if new_dir.is_dir() {
                    session.current_dir = new_dir.canonicalize().unwrap_or(new_dir);
                    println!(
                        "\x1b[90mWorkspace: {}\x1b[0m\n",
                        format_path_with_tilde(&session.current_dir)
                    );
                } else {
                    println!("\x1b[31mDirectory not found: {rest}\x1b[0m\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/image" => {
            let (img_path, prompt) = rest
                .split_once(char::is_whitespace)
                .map(|(p, r)| (p, r.trim()))
                .unwrap_or((rest, ""));

            if img_path.is_empty() {
                println!("\x1b[33m/image usage: /image <path> [prompt]\x1b[0m\n");
                return Some(SlashResult::Handled);
            }
            match image::load_image_as_data_uri(std::path::Path::new(img_path)) {
                Ok(uri) => {
                    session.pending_image = Some(uri);
                    if prompt.is_empty() {
                        println!(
                            "\x1b[90mImage attached — type your prompt and press Enter\x1b[0m\n"
                        );
                        Some(SlashResult::Handled)
                    } else {
                        Some(SlashResult::ForwardToAgent(prompt.to_owned()))
                    }
                }
                Err(e) => {
                    println!("\x1b[31mFailed to load image: {e}\x1b[0m\n");
                    Some(SlashResult::Handled)
                }
            }
        }

        "/paste" => match image::read_clipboard_image() {
            Ok(Some(uri)) => {
                session.pending_image = Some(uri);
                if rest.is_empty() {
                    println!(
                        "\x1b[90mClipboard image attached — type your prompt and press Enter\x1b[0m\n"
                    );
                    Some(SlashResult::Handled)
                } else {
                    Some(SlashResult::ForwardToAgent(rest.to_owned()))
                }
            }
            Ok(None) => {
                println!("\x1b[33mNo image found in clipboard.\x1b[0m\n");
                Some(SlashResult::Handled)
            }
            Err(e) => {
                println!("\x1b[31mClipboard error: {e}\x1b[0m\n");
                Some(SlashResult::Handled)
            }
        },

        "/learn" => {
            if rest.is_empty() {
                println!("\x1b[33m/learn usage: /learn <path>\x1b[0m\n");
            } else {
                let path = PathBuf::from(rest);
                let path = if path.is_absolute() {
                    path
                } else {
                    session.current_dir.join(path)
                };
                match skills::learn(&path) {
                    Ok(skill) => println!(
                        "\x1b[90mLearned skill: {} ({})\x1b[0m\n",
                        skill.name, skill.source_path
                    ),
                    Err(error) => println!("\x1b[31mLearn error: {error}\x1b[0m\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/memory" => {
            let memory = match MemoryStore::open_default() {
                Ok(m) => m,
                Err(e) => {
                    println!("\x1b[31mMemory error: {e}\x1b[0m\n");
                    return Some(SlashResult::Handled);
                }
            };
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match memory.recent_interactions(10) {
                    Ok(items) => {
                        if items.is_empty() {
                            println!("\x1b[90mNo interactions yet.\x1b[0m\n");
                        } else {
                            println!("\n\x1b[36mRecent interactions:\x1b[0m");
                            for item in items.iter().rev() {
                                println!(
                                    "  \x1b[90m[{}]\x1b[0m \x1b[36mYou:\x1b[0m {}",
                                    &item.created_at[..16.min(item.created_at.len())],
                                    if item.user_text.len() > 80 {
                                        format!("{}…", &item.user_text[..80])
                                    } else {
                                        item.user_text.clone()
                                    }
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("\x1b[31mError: {e}\x1b[0m\n"),
                },
                "get" => {
                    if args.is_empty() {
                        println!("\x1b[33m/memory get <key>\x1b[0m\n");
                    } else {
                        match memory.get_profile(args) {
                            Ok(Some(val)) => println!("{val}\n"),
                            Ok(None) => println!("\x1b[90m(not set)\x1b[0m\n"),
                            Err(e) => println!("\x1b[31mError: {e}\x1b[0m\n"),
                        }
                    }
                }
                "set" => {
                    let (key, val) = args
                        .split_once(char::is_whitespace)
                        .map(|(k, v)| (k, v.trim()))
                        .unwrap_or((args, ""));
                    if key.is_empty() {
                        println!("\x1b[33m/memory set <key> <value>\x1b[0m\n");
                    } else {
                        match memory.set_profile(key, val) {
                            Ok(()) => println!("\x1b[90mStored {key}.\x1b[0m\n"),
                            Err(e) => println!("\x1b[31mError: {e}\x1b[0m\n"),
                        }
                    }
                }
                "skills" => match memory.learned_skills(20) {
                    Ok(skills) => {
                        if skills.is_empty() {
                            println!("\x1b[90mNo learned skills.\x1b[0m\n");
                        } else {
                            println!("\n\x1b[36mLearned skills:\x1b[0m");
                            for s in &skills {
                                println!("  [{}] {} — {}", s.id, s.name, s.source_path);
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("\x1b[31mError: {e}\x1b[0m\n"),
                },
                "clear" => match memory.clear_interactions() {
                    Ok(count) => println!("\x1b[90mCleared {count} interactions.\x1b[0m\n"),
                    Err(e) => println!("\x1b[31mError: {e}\x1b[0m\n"),
                },
                _ => println!(
                    "\x1b[33m/memory usage: list | clear | get <key> | set <key> <val> | skills\x1b[0m\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/stats" => {
            let provider = &session.config.ai_provider;
            let model = active_model(provider, &session.config);
            let interactions = MemoryStore::open_default()
                .and_then(|m| m.recent_interactions(1000))
                .map(|v| v.len())
                .unwrap_or(0);
            println!("\n\x1b[36m─ Session Stats ─────────────────────────\x1b[0m");
            println!("  Provider : \x1b[32m{provider}\x1b[0m");
            println!("  Model    : {model}");
            println!(
                "  Workspace: {}",
                format_path_with_tilde(&session.current_dir)
            );
            println!(
                "  Fast mode: {}",
                if session.fast_mode { "on" } else { "off" }
            );
            println!("  Memory   : {interactions} interactions");
            if session.pending_image.is_some() {
                println!("  Image    : \x1b[33mattached\x1b[0m");
            }
            println!();
            Some(SlashResult::Handled)
        }

        "/exit" | "/quit" => Some(SlashResult::Exit),

        "/code" => {
            if rest.is_empty() {
                println!("\x1b[33m/code requires a task description\x1b[0m\n");
                Some(SlashResult::Handled)
            } else {
                Some(SlashResult::ForwardToAgent(format!("[code] {rest}")))
            }
        }

        _ => {
            // Unknown slash command — treat as normal message to the agent
            None
        }
    }
}

async fn run_interactive_chat() -> Result<()> {
    let config = load_config()?;

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let provider = &config.ai_provider.clone();
    let model = active_model(provider, &config).to_owned();

    // Print startup banner
    let now = chrono::Local::now();
    let year = now.format("%Y").to_string().parse::<i32>().unwrap_or(2026) + 543;
    let date_time = format!(
        "{}/{:02}/{:02} {:02}:{:02}",
        now.format("%d"),
        now.format("%m"),
        year,
        now.format("%H"),
        now.format("%M")
    );
    let version = env!("CARGO_PKG_VERSION");
    let line1_text = format!("[Mint] v{} | Active AI: {}", version, provider);
    let line2_text = format!("{} • {}", date_time, model);

    let len1 = line1_text.chars().count();
    let len2 = line2_text.chars().count();
    let content_width = std::cmp::max(len1, len2);
    let border_len = content_width + 2;

    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_width = term_width as usize;
    let ascii_width = 34;
    let spacing = 3;
    let box_width = border_len + 2;

    if term_width >= ascii_width + spacing + box_width {
        println!(
            "\x1b[32m __  __ _       _    ___ _    ___ \x1b[0m   \x1b[37m╭{}╮\x1b[0m",
            "─".repeat(border_len)
        );
        println!(
            "\x1b[32m|  \\/  (_)_ __ | |_ / __| |  |_ _|\x1b[0m   \x1b[37m│\x1b[0m \x1b[32m[Mint]\x1b[0m v{} | Active AI: {}{} \x1b[37m│\x1b[0m",
            version,
            provider,
            " ".repeat(content_width - len1)
        );
        println!(
            "\x1b[32m| |\\/| | | '_ \\|  _| (__| |__ | | \x1b[0m   \x1b[37m│\x1b[0m {}{} \x1b[37m│\x1b[0m",
            line2_text,
            " ".repeat(content_width - len2)
        );
        println!(
            "\x1b[32m|_|  |_|_|_| |_|\\__|\\___|\\___|___|\x1b[0m   \x1b[37m╰{}╯\x1b[0m",
            "─".repeat(border_len)
        );
    } else {
        println!("\x1b[37m╭{}╮\x1b[0m", "─".repeat(border_len));
        println!(
            "\x1b[37m│\x1b[0m \x1b[32m[Mint]\x1b[0m v{} | Active AI: {}{} \x1b[37m│\x1b[0m",
            version,
            provider,
            " ".repeat(content_width - len1)
        );
        println!(
            "\x1b[37m│\x1b[0m {}{} \x1b[37m│\x1b[0m",
            line2_text,
            " ".repeat(content_width - len2)
        );
        println!("\x1b[37m╰{}╯\x1b[0m", "─".repeat(border_len));
        println!("\x1b[32m __  __ _       _    ___ _    ___ \x1b[0m");
        println!("\x1b[32m|  \\/  (_)_ __ | |_ / __| |  |_ _|\x1b[0m");
        println!("\x1b[32m| |\\/| | | '_ \\|  _| (__| |__ | | \x1b[0m");
        println!("\x1b[32m|_|  |_|_|_| |_|\\__|\\___|\\___|___|\\x1b[0m");
    }
    println!("Type naturally or /help for commands. Ctrl+V pastes images. Ctrl+D exits.\n");

    let mut session = InteractiveSession {
        config,
        current_dir: current_dir.clone(),
        fast_mode: false,
        pending_image: None,
    };

    loop {
        let path_str = format_path_with_tilde(&session.current_dir);
        let model_str = active_model(&session.config.ai_provider, &session.config).to_owned();

        if let Some(input) =
            read_line_interactive(&session.config.ai_provider, &model_str, &path_str)?
        {
            if input.pasted_image.is_some() {
                session.pending_image = input.pasted_image;
            }
            let query_str = input.text.trim().to_owned();
            if query_str.is_empty() {
                continue;
            }

            // Run slash-command router
            match handle_slash_command(&mut session, &query_str).await {
                Some(SlashResult::Handled) => continue,
                Some(SlashResult::Exit) => {
                    println!("Goodbye!");
                    break;
                }
                Some(SlashResult::ForwardToAgent(task)) => {
                    // Force code agent for /code forwarded tasks
                    println!();
                    if let Err(error) = run_code_agent_with_saved_image(
                        &task,
                        &session.current_dir,
                        &session.config,
                        session.pending_image.take(),
                        agent::AgentOptions {
                            fast_mode: session.fast_mode,
                        },
                    )
                    .await
                    {
                        println!("\x1b[31mError:\x1b[0m {error}\n");
                    }
                    continue;
                }
                None => {} // Not a slash command, fall through
            }

            // Check if it's a /code or regular agent request
            if let Some(task) = query_str.strip_prefix("/code ") {
                println!();
                if let Err(error) = run_code_agent_with_saved_image(
                    task.trim(),
                    &session.current_dir,
                    &session.config,
                    session.pending_image.take(),
                    agent::AgentOptions {
                        fast_mode: session.fast_mode,
                    },
                )
                .await
                {
                    println!("\x1b[31mError:\x1b[0m {error}\n");
                }
                continue;
            }

            // Regular agent loop (handles both chat and coding)
            if !query_str.starts_with("/chat ") {
                if let Err(error) = run_code_agent_with_saved_image(
                    query_str.trim_start_matches("/chat "),
                    &session.current_dir,
                    &session.config,
                    session.pending_image.take(),
                    agent::AgentOptions {
                        fast_mode: session.fast_mode,
                    },
                )
                .await
                {
                    println!("\x1b[31mError:\x1b[0m {error}\n");
                }
                continue;
            }

            // /chat explicit: use streaming chat with fallback
            let message = query_str
                .strip_prefix("/chat ")
                .unwrap_or(&query_str)
                .trim()
                .to_owned();

            println!();
            print!("\x1b[32mMint:\x1b[0m \x1b[90mThinking...\x1b[0m");
            let _ = io::stdout().flush();

            let mut system_instruction = format!(
                "You are Mint, a cute and helpful AI assistant. You speak in a polite, friendly, and sweet Thai tone (using \"คุณ\", \"ค่ะ\", \"นะคะ\"). \
                You are running inside the Mint CLI interactive chat. \
                You have access to native system actions to help the user! If the user asks you to open a website, launch an app, read a file, list a folder, run code, run tests, or execute a local shell command, you can execute these actions by writing a special block at the very end of your response: \
                `[ACTION: <command> <arguments>]` \
                The available actions are: \
                - `[ACTION: open <url_or_path>]` to open a URL or a folder path. \
                - `[ACTION: open-app <app_name>]` to launch a desktop application. \
                - `[ACTION: read-file <file_path>]` to read the contents of a file. \
                - `[ACTION: read-folder <path>]` to list files/folders in a directory. \
                - `[ACTION: run-shell <command>]` to run a non-destructive local shell command after approval. \
                Write the action block on a single line at the very end of your response."
            );
            if let Ok(memory) = MemoryStore::open_default() {
                if let Ok(Some(name)) = memory.get_profile("name") {
                    system_instruction.push_str(&format!(
                        "\nThe user's name is {}. Refer to them by their name when appropriate.",
                        name
                    ));
                }
            }

            let image_uri = session.pending_image.take();
            let sent_image = image_uri.clone();
            let mut first_chunk = true;
            let mut filter = ActionStreamFilter::new();

            let stream_result = orchestrate_chat_stream_with_fallback(
                &session.config,
                &ChatRequest {
                    message: message.clone(),
                    system_instruction,
                    image_data_uri: image_uri,
                    audio_data_uri: None,
                },
                |chunk| {
                    if first_chunk {
                        first_chunk = false;
                        print!("\r\x1b[2K\x1b[32mMint:\x1b[0m ");
                    }
                    filter.process_chunk(&chunk, |text| {
                        print!("{}", text);
                    });
                    let _ = io::stdout().flush();
                },
            )
            .await;

            let actions = filter.finalize(|text| {
                print!("{}", text);
            });
            let _ = io::stdout().flush();

            match stream_result {
                Ok((response, fallback)) => {
                    image::save_sent_image_after_send(sent_image.as_deref(), &message);
                    if first_chunk {
                        print!("\r\x1b[2K");
                        let _ = io::stdout().flush();
                    } else {
                        println!("\n");
                        let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
                        let width = tw as usize;
                        // Show provider badge (with fallback indicator if applicable)
                        let badge = if let Some(fb_provider) = &fallback {
                            format!(
                                "\x1b[90m{} • {} → fallback: {} • {}\x1b[0m",
                                session.config.ai_provider,
                                active_model(&session.config.ai_provider, &session.config),
                                fb_provider,
                                response.model
                            )
                        } else {
                            format!("\x1b[90m{} • {}\x1b[0m", response.provider, response.model)
                        };
                        println!("{badge}");
                        println!("\x1b[90m{}\x1b[0m\n", "─".repeat(width));
                    }
                    for action in actions {
                        if let Err(e) = execute_action(&action, &session.config) {
                            println!("\x1b[31mError executing action:\x1b[0m {}\n", e);
                        }
                    }
                }
                Err(e) => {
                    if first_chunk {
                        print!("\r\x1b[2K");
                    }
                    println!("\x1b[31mError:\x1b[0m {}\n", e);
                }
            }
        } else {
            println!("Goodbye!");
            break;
        }
    }

    Ok(())
}

const AUTOCOMPLETE_COMMANDS: &[(&str, &str)] = &[
    ("/help", "Show help menu"),
    ("/fast", "Toggle fast mode (hide thinking traces)"),
    ("/models", "List AI providers or switch active provider"),
    ("/clear", "Clear conversation history"),
    ("/cd", "Change active workspace directory"),
    ("/image", "Attach image from disk"),
    ("/paste", "Attach image from clipboard"),
    ("/learn", "Import persistent skill/instruction"),
    ("/memory", "Manage long-term memory store"),
    ("/stats", "Show session statistics"),
    ("/exit", "Exit Mint CLI"),
    ("/quit", "Exit Mint CLI"),
    ("/code", "Run in code-agent mode"),
];

fn draw_input_box(
    input: &str,
    placeholder: &str,
    model: &str,
    path_str: &str,
    tab_base_input: Option<&str>,
    tab_index: Option<usize>,
) -> usize {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = term_width as usize;
    let prefix = "› ";
    let input_width = width.saturating_sub(2);
    let content_max_len = input_width.saturating_sub(prefix.chars().count());

    let display_str = if input.is_empty() {
        format!("\x1b[90m{}\x1b[39m", placeholder)
    } else {
        input.to_string()
    };

    let visible_len = if input.is_empty() {
        placeholder.chars().count()
    } else {
        string_visual_width(input)
    };

    let pad_len = content_max_len.saturating_sub(visible_len);
    let padding = " ".repeat(pad_len);
    let blank_line = " ".repeat(input_width);

    println!(" \x1b[48;2;65;69;77m{}\x1b[0m", blank_line);
    println!(
        " \x1b[48;2;65;69;77m{}{}{}\x1b[0m",
        prefix, display_str, padding
    );
    println!(" \x1b[48;2;65;69;77m{}\x1b[0m", blank_line);

    let agent_str = format!(" [Agent] \x1b[33m{}\x1b[0m", model);
    let path_label = format!("path: {}", path_str);
    let agent_visible_len = " [Agent] ".len() + model.chars().count();
    let path_visible_len = path_label.chars().count();

    let status_pad_len = (width - 1).saturating_sub(agent_visible_len + path_visible_len);
    let status_padding = " ".repeat(status_pad_len);

    print!("{}{}{}", agent_str, status_padding, path_label);

    // Compute and draw suggestions
    let search_query = tab_base_input.unwrap_or(input);
    let mut match_count = 0;
    if search_query.starts_with('/') {
        let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(search_query))
            .collect();

        if !matches.is_empty() {
            match_count = matches.len();
            println!();
            println!(" \x1b[36mSuggestions:\x1b[0m");
            let highlight_idx = tab_index.map(|idx| idx % matches.len());
            for (i, (cmd, desc)) in matches.iter().enumerate() {
                if Some(i) == highlight_idx {
                    println!(
                        "  \x1b[32m▶\x1b[0m \x1b[1;33m{:<12}\x1b[0m \x1b[90m-\x1b[0m \x1b[1;37m{}\x1b[0m",
                        cmd, desc
                    );
                } else {
                    println!("    \x1b[33m{:<12}\x1b[0m \x1b[90m-\x1b[0m {}", cmd, desc);
                }
            }
        }
    }

    let _ = io::stdout().flush();
    match_count
}

fn input_cursor_column(input_chars: &[char], cursor_pos: usize) -> usize {
    let visual_cursor_pos: usize = input_chars[..cursor_pos]
        .iter()
        .copied()
        .map(char_visual_width)
        .sum();
    4 + visual_cursor_pos
}

fn position_input_cursor(input_chars: &[char], cursor_pos: usize, match_count: usize) {
    let up_lines = if match_count > 0 {
        4 + match_count
    } else {
        2
    };
    print!(
        "\x1b[{}A\x1b[{}G",
        up_lines,
        input_cursor_column(input_chars, cursor_pos)
    );
}

fn clear_input_box() {
    print!("\r\x1b[1A\x1b[J");
}

fn redraw_input_box(
    input_chars: &[char],
    cursor_pos: usize,
    placeholder: &str,
    model: &str,
    path_str: &str,
    tab_base_input: Option<&str>,
    tab_index: Option<usize>,
) {
    clear_input_box();
    let input: String = input_chars.iter().collect();
    let match_count = draw_input_box(
        &input,
        placeholder,
        model,
        path_str,
        tab_base_input,
        tab_index,
    );
    position_input_cursor(input_chars, cursor_pos, match_count);
    let _ = io::stdout().flush();
}

fn read_line_interactive(
    _provider: &str,
    model: &str,
    path_str: &str,
) -> io::Result<Option<InteractiveInput>> {
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    let mut input_chars: Vec<char> = Vec::new();
    let mut cursor_pos = 0;
    let placeholder = "Ask anything...";
    let mut ctrl_d_pressed = false;
    let mut pasted_image = None;

    // Track tab autocomplete state
    let mut tab_base_input: Option<String> = None;
    let mut tab_index: Option<usize> = None;

    let match_count = draw_input_box("", placeholder, model, path_str, None, None);
    position_input_cursor(&input_chars, cursor_pos, match_count);
    let _ = io::stdout().flush();

    enable_raw_mode()?;

    let result = loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            if let Event::Key(key_event) = event::read()? {
                if key_event.kind == event::KeyEventKind::Press {
                    let is_ctrl_d = matches!(key_event.code, KeyCode::Char('d'))
                        && key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL);
                    if ctrl_d_pressed && !is_ctrl_d {
                        ctrl_d_pressed = false;
                        disable_raw_mode()?;
                        print!("\r\x1b[3B\r\x1b[2K\x1b[3A");
                        print!("\x1b[{}G", input_cursor_column(&input_chars, cursor_pos));
                        let _ = io::stdout().flush();
                        enable_raw_mode()?;
                    }

                    // Reset tab autocomplete state if any key other than Tab, Up, or Down is pressed
                    if key_event.code != KeyCode::Tab
                        && key_event.code != KeyCode::Up
                        && key_event.code != KeyCode::Down
                    {
                        tab_base_input = None;
                        tab_index = None;
                    }

                    match key_event.code {
                        KeyCode::Char('d')
                            if key_event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            if ctrl_d_pressed {
                                disable_raw_mode()?;
                                clear_input_box();
                                let _ = io::stdout().flush();
                                break None;
                            } else {
                                ctrl_d_pressed = true;
                                disable_raw_mode()?;
                                print!(
                                    "\r\x1b[3B\r\x1b[2K\x1b[33mPress Ctrl+D again to exit\x1b[0m\x1b[3A"
                                );
                                print!("\x1b[{}G", input_cursor_column(&input_chars, cursor_pos));
                                let _ = io::stdout().flush();
                                enable_raw_mode()?;
                            }
                        }
                        KeyCode::Char('v')
                            if key_event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            if let Ok(Some(uri)) = image::read_clipboard_image() {
                                pasted_image = Some(uri);
                                insert_image_placeholder(&mut input_chars, &mut cursor_pos);

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    None,
                                    None,
                                );
                                enable_raw_mode()?;
                            }
                        }
                        KeyCode::Char(c) => {
                            let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
                            let max_width = (term_width as usize).saturating_sub(4);
                            let current_visual_width: usize =
                                input_chars.iter().copied().map(char_visual_width).sum();

                            if current_visual_width < max_width {
                                input_chars.insert(cursor_pos, c);
                                cursor_pos += 1;

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    None,
                                    None,
                                );
                                enable_raw_mode()?;
                            }
                        }
                        KeyCode::Backspace => {
                            if cursor_pos > 0 {
                                cursor_pos -= 1;
                                input_chars.remove(cursor_pos);

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    None,
                                    None,
                                );
                                enable_raw_mode()?;
                            }
                        }
                        KeyCode::Tab => {
                            let base = match &tab_base_input {
                                Some(b) => b.clone(),
                                None => {
                                    let current_str: String = input_chars.iter().collect();
                                    tab_base_input = Some(current_str.clone());
                                    current_str
                                }
                            };

                            if base.starts_with('/') {
                                let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
                                    .iter()
                                    .filter(|(cmd, _)| cmd.starts_with(&base))
                                    .collect();

                                if !matches.is_empty() {
                                    let idx = tab_index.unwrap_or(0) % matches.len();
                                    let completed = format!("{} ", matches[idx].0);
                                    input_chars = completed.chars().collect();
                                    cursor_pos = input_chars.len();
                                    
                                    // Highlight currently completed item in suggestions
                                    let current_highlight = Some(idx);
                                    tab_index = Some(idx + 1);

                                    disable_raw_mode()?;
                                    redraw_input_box(
                                        &input_chars,
                                        cursor_pos,
                                        placeholder,
                                        model,
                                        path_str,
                                        Some(&base),
                                        current_highlight,
                                    );
                                    enable_raw_mode()?;
                                }
                            }
                        }
                        KeyCode::Down => {
                            let base = match &tab_base_input {
                                Some(b) => b.clone(),
                                None => {
                                    let current_str: String = input_chars.iter().collect();
                                    tab_base_input = Some(current_str.clone());
                                    current_str
                                }
                            };

                            if base.starts_with('/') {
                                let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
                                    .iter()
                                    .filter(|(cmd, _)| cmd.starts_with(&base))
                                    .collect();

                                if !matches.is_empty() {
                                    let new_idx = match tab_index {
                                        Some(idx) => (idx + 1) % matches.len(),
                                        None => 0,
                                    };
                                    tab_index = Some(new_idx);
                                    let completed = format!("{} ", matches[new_idx].0);
                                    input_chars = completed.chars().collect();
                                    cursor_pos = input_chars.len();

                                    disable_raw_mode()?;
                                    redraw_input_box(
                                        &input_chars,
                                        cursor_pos,
                                        placeholder,
                                        model,
                                        path_str,
                                        Some(&base),
                                        Some(new_idx),
                                    );
                                    enable_raw_mode()?;
                                }
                            }
                        }
                        KeyCode::Up => {
                            let base = match &tab_base_input {
                                Some(b) => b.clone(),
                                None => {
                                    let current_str: String = input_chars.iter().collect();
                                    tab_base_input = Some(current_str.clone());
                                    current_str
                                }
                            };

                            if base.starts_with('/') {
                                let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
                                    .iter()
                                    .filter(|(cmd, _)| cmd.starts_with(&base))
                                    .collect();

                                if !matches.is_empty() {
                                    let new_idx = match tab_index {
                                        Some(idx) => {
                                            if idx == 0 {
                                                matches.len() - 1
                                            } else {
                                                idx - 1
                                            }
                                        }
                                        None => matches.len() - 1,
                                    };
                                    tab_index = Some(new_idx);
                                    let completed = format!("{} ", matches[new_idx].0);
                                    input_chars = completed.chars().collect();
                                    cursor_pos = input_chars.len();

                                    disable_raw_mode()?;
                                    redraw_input_box(
                                        &input_chars,
                                        cursor_pos,
                                        placeholder,
                                        model,
                                        path_str,
                                        Some(&base),
                                        Some(new_idx),
                                    );
                                    enable_raw_mode()?;
                                }
                            }
                        }
                        KeyCode::Left => {
                            while cursor_pos > 0 {
                                cursor_pos -= 1;
                                if cursor_pos == 0 || !is_thai_zero_width(input_chars[cursor_pos]) {
                                    break;
                                }
                            }
                            disable_raw_mode()?;
                            let visual_cursor_pos: usize = input_chars[..cursor_pos]
                                .iter()
                                .copied()
                                .map(char_visual_width)
                                .sum();
                            print!("\x1b[{}G", 4 + visual_cursor_pos);
                            let _ = io::stdout().flush();
                            enable_raw_mode()?;
                        }
                        KeyCode::Right => {
                            while cursor_pos < input_chars.len() {
                                cursor_pos += 1;
                                if cursor_pos == input_chars.len()
                                    || !is_thai_zero_width(input_chars[cursor_pos])
                                {
                                    break;
                                }
                            }
                            disable_raw_mode()?;
                            let visual_cursor_pos: usize = input_chars[..cursor_pos]
                                .iter()
                                .copied()
                                .map(char_visual_width)
                                .sum();
                            print!("\x1b[{}G", 4 + visual_cursor_pos);
                            let _ = io::stdout().flush();
                            enable_raw_mode()?;
                        }
                        KeyCode::Enter => {
                            disable_raw_mode()?;
                            clear_input_box();
                            let input_str: String = input_chars.iter().collect();
                            println!("\x1b[36mYou ›\x1b[0m {}", input_str);
                            let _ = io::stdout().flush();
                            break Some(InteractiveInput {
                                text: input_str,
                                pasted_image,
                            });
                        }
                        KeyCode::Esc => {
                            disable_raw_mode()?;
                            clear_input_box();
                            let _ = io::stdout().flush();
                            break None;
                        }
                        _ => {}
                    }
                }
            }
        }
    };

    Ok(result)
}

fn insert_image_placeholder(input_chars: &mut Vec<char>, cursor_pos: &mut usize) {
    const PLACEHOLDER: &str = "[Image #1]";
    let input: String = input_chars.iter().collect();
    if input.contains(PLACEHOLDER) {
        return;
    }
    let placeholder_chars = PLACEHOLDER.chars().collect::<Vec<_>>();
    input_chars.splice(*cursor_pos..*cursor_pos, placeholder_chars.iter().copied());
    *cursor_pos += placeholder_chars.len();
}

fn format_path_with_tilde(path: &std::path::Path) -> String {
    let path_str = path.to_string_lossy().to_string();
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        if path_str.starts_with(&home_str) {
            return path_str.replacen(&home_str, "~", 1);
        }
    }
    path_str
}

fn is_thai_zero_width(c: char) -> bool {
    let cp = c as u32;
    cp == 0x0E31 || (0x0E34..=0x0E3A).contains(&cp) || (0x0E47..=0x0E4E).contains(&cp)
}

fn char_visual_width(c: char) -> usize {
    if is_thai_zero_width(c) { 0 } else { 1 }
}

fn string_visual_width(s: &str) -> usize {
    s.chars().map(char_visual_width).sum()
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

fn configured(config: &mint_core::MintConfig, keys: &[&str]) -> bool {
    keys.iter().all(|key| {
        config
            .extra
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

fn edit_content(
    content: Option<String>,
    from_file: Option<PathBuf>,
    config: &MintConfig,
) -> Result<String> {
    match from_file {
        Some(path) => {
            let path = assert_path_capability(&path, Capability::Read, config)?;
            Ok(fs::read_to_string(path)?)
        }
        None => Ok(content.unwrap_or_default()),
    }
}

fn file_edits(values: &[String], config: &MintConfig) -> Result<Vec<CodeEdit>> {
    values
        .iter()
        .map(|value| {
            let (target, source) = value
                .split_once('=')
                .ok_or_else(|| anyhow::anyhow!("edit must use TARGET=SOURCE format"))?;
            Ok(CodeEdit {
                path: PathBuf::from(target),
                content: edit_content(None, Some(PathBuf::from(source)), config)?,
            })
        })
        .collect()
}

fn open_system_handler(target: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/c", "start", "", target])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(target).spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        std::process::Command::new("xdg-open").arg(target).spawn()?;
    }
    Ok(())
}

fn launch_desktop_app(name: &str) -> Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(&["/c", "start", "", name])
            .spawn()?;
    }
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .args(&["-a", name])
            .spawn()?;
    }
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    {
        if std::process::Command::new(name)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
        let lower = name.to_lowercase();
        if std::process::Command::new("gtk-launch")
            .arg(&lower)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .is_ok()
        {
            return Ok(());
        }
        std::process::Command::new("xdg-open").arg(name).spawn()?;
    }
    Ok(())
}

fn read_file_content(path: &std::path::Path) -> Result<()> {
    let content = fs::read_to_string(path)?;
    println!("{}", content);
    Ok(())
}

fn read_folder_content(path: &std::path::Path) -> Result<()> {
    let entries = fs::read_dir(path)?;
    for entry in entries {
        let entry = entry?;
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();
        let metadata = entry.metadata()?;
        if metadata.is_dir() {
            println!("\x1b[34m{}/\x1b[0m", file_name_str);
        } else {
            println!("{}", file_name_str);
        }
    }
    Ok(())
}

pub static SESSION_APPROVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub fn confirm(prompt: &str) -> Result<bool> {
    let clean_prompt = prompt
        .replace(" [y/N] ", "")
        .replace(" [y/N]", "")
        .trim()
        .to_string();

    if SESSION_APPROVED.load(std::sync::atomic::Ordering::Relaxed) {
        println!("{} \x1b[32mApprove (session-wide)\x1b[0m", clean_prompt);
        return Ok(true);
    }

    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() || enable_raw_mode().is_err() {
        print!("{} [y/N] ", clean_prompt);
        let _ = io::stdout().flush();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        return Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ));
    }

    let _ = disable_raw_mode();
    println!("{}", clean_prompt);

    let options = ["Approve", "Approve this session", "No"];
    let mut selected = 0;

    let print_choices = |selected: usize| -> Result<()> {
        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                println!("  \x1b[36m❯ {}. {}\x1b[0m", i + 1, opt);
            } else {
                println!("    {}. {}", i + 1, opt);
            }
        }
        io::stdout().flush()?;
        Ok(())
    };

    print_choices(selected)?;

    let _ = enable_raw_mode();

    let choice = loop {
        match event::poll(std::time::Duration::from_millis(100)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key_event)) => {
                    if key_event.kind == event::KeyEventKind::Press {
                        let is_ctrl_c = matches!(key_event.code, KeyCode::Char('c'))
                            && key_event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL);
                        if is_ctrl_c {
                            break 2;
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if selected > 0 {
                                    selected -= 1;
                                } else {
                                    selected = options.len() - 1;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Down => {
                                if selected < options.len() - 1 {
                                    selected += 1;
                                } else {
                                    selected = 0;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Tab => {
                                if selected < options.len() - 1 {
                                    selected += 1;
                                } else {
                                    selected = 0;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Char('1') | KeyCode::Char('a') | KeyCode::Char('y') => {
                                break 0;
                            }
                            KeyCode::Char('2') | KeyCode::Char('s') => {
                                break 1;
                            }
                            KeyCode::Char('3') | KeyCode::Char('n') | KeyCode::Char('c') => {
                                break 2;
                            }
                            KeyCode::Enter => {
                                break selected;
                            }
                            KeyCode::Esc => {
                                break 2;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    break 2;
                }
            },
            Ok(false) => {}
            Err(_) => {
                break 2;
            }
        }
    };

    let _ = disable_raw_mode();
    print!("\x1b[{}A\x1b[J", options.len() + 1);

    let result_str = match choice {
        0 => "\x1b[32mApprove\x1b[0m",
        1 => "\x1b[32mApprove this session\x1b[0m",
        _ => "\x1b[31mNo\x1b[0m",
    };
    println!("{} {}", clean_prompt, result_str);
    let _ = io::stdout().flush();

    match choice {
        0 => Ok(true),
        1 => {
            SESSION_APPROVED.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn confirm_shell_execution() -> Result<bool> {
    confirm("Approve local shell execution? [y/N]")
}

fn print_shell_output(output: &mint_core::ShellOutput) {
    if !output.stdout.is_empty() {
        print!("{}", output.stdout);
    }
    if !output.stderr.is_empty() {
        eprint!("{}", output.stderr);
    }
    if !output.stdout.ends_with('\n') && !output.stderr.ends_with('\n') {
        println!();
    }
    println!(
        "\x1b[90m[exit: {} | sandboxed: {}]\x1b[0m\n",
        output
            .status
            .map_or_else(|| "unknown".into(), |status| status.to_string()),
        output.sandboxed
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_one_image_placeholder_at_cursor() {
        let mut chars = "ask ".chars().collect::<Vec<_>>();
        let mut cursor = chars.len();

        insert_image_placeholder(&mut chars, &mut cursor);
        insert_image_placeholder(&mut chars, &mut cursor);

        assert_eq!(chars.iter().collect::<String>(), "ask [Image #1]");
        assert_eq!(cursor, "ask [Image #1]".chars().count());
    }
}

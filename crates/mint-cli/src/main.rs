use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
};

use mint_core::{
    CHAT_CLI_ID, Capability, ChatRequest, CodeEdit, CodePatchHunk, ImageGenRequest, KnowledgeStore,
    MemoryStore, MintConfig, TaskStore, apply_code_edits, assert_path_capability, build_code_patch,
    build_symbol_index, classify_shell_command, config_path, create_folder, execute_native_plugin,
    fetch_github_repo_summary, find_paths, generate_images, index_semantic_code, initialize_config,
    inspect_code_plan, list_code_files, load_config, native_plugins,
    orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback, parse_github_url,
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

const RESET: &str = "\x1b[0m";
const MINT: &str = "\x1b[32m";
const BLUE: &str = "\x1b[38;2;78;201;216m";
const DIM: &str = "\x1b[90m";
const ERROR: &str = "\x1b[31m";
const WARN: &str = "\x1b[33m";
const COMPOSER_BG: &str = "\x1b[48;2;35;39;45m";

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
    /// Start the browser automation environment and enable browser actions.
    Auto,
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
    /// Generate an image from a text prompt using NanoBanana (Gemini image model).
    Imagine {
        /// Text description of the image to generate.
        prompt: String,
        /// Aspect ratio: 1:1, 16:9, 9:16, or 4:3 [default: 1:1]
        #[arg(long, default_value = "1:1")]
        aspect: String,
        /// Number of images to generate (1–4) [default: 1]
        #[arg(long, default_value_t = 1)]
        count: u8,
        /// Negative prompt — elements to avoid in the image
        #[arg(long)]
        negative: Option<String>,
        /// Save the first generated image to this path
        #[arg(long)]
        output: Option<PathBuf>,
    },
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
    Allow {
        server: String,
        tool: String,
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
    /// Fetch GitHub repository metadata and README, then get an overview of the repo.
    GithubOverview {
        /// The GitHub repository URL or name (e.g. "https://github.com/owner/repo" or "owner/repo").
        repo: String,
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

fn print_mcp_servers(servers: &std::collections::BTreeMap<String, mint_core::mcp::McpServer>) {
    if servers.is_empty() {
        println!("{DIM}(No MCP servers configured.){RESET}\n");
        return;
    }
    for (name, srv) in servers {
        let args_str = srv.args.join(" ");
        println!(
            "  {BLUE}●{RESET} {name} {DIM}({} {}){RESET}",
            srv.command, args_str
        );
    }
    println!();
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
                    let value =
                        serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
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
            Command::Auto => {
                println!("🚀 Starting Mint Browser Automation Environment...");
                let config = mint_core::load_config()?;
                mint_core::spawn_automation_browser(&config)
                    .await
                    .map_err(anyhow::Error::msg)?;

                // Enable the browser tools if they are disabled
                let mut config_mut = config.clone();
                let mut changed = false;
                for tool in &[
                    "browser_open",
                    "browser_click",
                    "browser_type",
                    "browser_read",
                ] {
                    if config_mut.disabled_tools.contains(&tool.to_string()) {
                        config_mut.disabled_tools.retain(|x| x != *tool);
                        changed = true;
                    }
                }
                if changed {
                    mint_core::save_config(&config_mut)?;
                    println!(
                        "✅ Enabled browser automation tools in config: browser_open, browser_click, browser_type, browser_read"
                    );
                }

                println!(
                    "🌐 Isolated browser running with remote debugging on http://127.0.0.1:9222"
                );
                println!(
                    "💬 Keep this terminal open while you want Mint to automate browser tasks."
                );
                println!("Press Ctrl+C to terminate the automation browser session.");
                println!("----------------------------------------------------------------------");

                // Start tailing the log file in a background task
                let log_dir = dirs::config_dir()
                    .unwrap_or_else(std::env::temp_dir)
                    .join("mint");
                let log_file = log_dir.join("browser-automation.log");

                // Clear existing log file on startup
                let _ = std::fs::remove_file(&log_file);

                let log_file_clone = log_file.clone();
                tokio::spawn(async move {
                    use std::io::{BufRead, BufReader, Seek, SeekFrom};
                    let mut file_pos = 0;
                    loop {
                        if log_file_clone.exists()
                            && let Ok(mut file) = std::fs::File::open(&log_file_clone)
                            && file.seek(SeekFrom::Start(file_pos)).is_ok()
                        {
                            let reader = BufReader::new(file);
                            for line_str in reader.lines().flatten() {
                                if line_str.contains("[NAVIGATE]")
                                    || line_str.contains("[NAVIGATE_SUCCESS]")
                                {
                                    println!("🌐 {line_str}");
                                } else if line_str.contains("[CLICK]")
                                    || line_str.contains("[CLICK_SUCCESS]")
                                {
                                    println!("🖱️ {line_str}");
                                } else if line_str.contains("[TYPE]")
                                    || line_str.contains("[TYPE_SUCCESS]")
                                {
                                    println!("⌨️ {line_str}");
                                } else if line_str.contains("[READ]")
                                    || line_str.contains("[READ_SUCCESS]")
                                {
                                    println!("📖 {line_str}");
                                } else if line_str.contains("_ERROR]") {
                                    println!("❌ {line_str}");
                                } else {
                                    println!("📝 {line_str}");
                                }
                            }
                            if let Ok(pos) = log_file_clone.metadata().map(|m| m.len()) {
                                file_pos = pos;
                            }
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                    }
                });

                tokio::signal::ctrl_c().await.ok();
                println!("\n👋 Terminating Mint Browser Automation Environment...");
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
                McpCommand::List => {
                    println!("\n{BLUE}MCP servers:{RESET}");
                    print_mcp_servers(&mcp::list()?);
                }
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
                McpCommand::Allow { server, tool } => {
                    if mcp::allow(&server, &tool)? {
                        println!("allowed {server}/{tool}");
                    } else {
                        println!("already allowed {server}/{tool}");
                    }
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
                    let mut skills = memory.learned_skills(100)?;

                    if let Some(home) = dirs::home_dir() {
                        let global_skills_path =
                            home.join(".config").join("mint").join("mint-skills");
                        mint_core::skills::load_skills_from_dir(&global_skills_path, &mut skills);
                    }
                    if let Ok(root) = std::env::current_dir()
                        && let Ok(root) = root.canonicalize()
                    {
                        let workspace_skills_path1 = root.join(".agents").join("skills");
                        mint_core::skills::load_skills_from_dir(
                            &workspace_skills_path1,
                            &mut skills,
                        );

                        let workspace_skills_path2 = root.join("skills");
                        mint_core::skills::load_skills_from_dir(
                            &workspace_skills_path2,
                            &mut skills,
                        );
                    }

                    let mut unique_skills = std::collections::BTreeMap::new();
                    for skill in skills {
                        let loc = if skill.source_path.contains("/.config/mint/mint-skills") {
                            "Global"
                        } else if skill.source_path.contains("/skills")
                            || skill.source_path.contains("/.agents/skills")
                        {
                            "Workspace"
                        } else {
                            "Taught"
                        };
                        unique_skills.insert(skill.name.clone(), (skill, loc));
                    }

                    if unique_skills.is_empty() {
                        println!("No learned skills found.");
                    } else {
                        println!("Learned AI Skills:");
                        for (name, (skill, loc)) in &unique_skills {
                            if *loc == "Taught" {
                                println!("  ● [{}] {}", loc, name);
                            } else {
                                println!("  ● [{}] {} (Source: {})", loc, name, skill.source_path);
                            }
                        }
                    }
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
                            chat_id: Some(CHAT_CLI_ID.to_owned()),
                            image_data_uri,
                            audio_data_uri: None,
                            document_attachment: None,
                            workspace_path: None,
                            agent_id: None,
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
                            serde_json::to_string_pretty(
                                &memory.recent_interactions_for_chat(CHAT_CLI_ID, limit)?
                            )?
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
                    CodeCommand::GithubOverview { repo } => {
                        run_github_overview(&repo, &config).await?;
                    }
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
            Command::Imagine {
                prompt,
                aspect,
                count,
                negative,
                output,
            } => {
                let config = load_config()?;
                let count = count.clamp(1, 4);
                eprint!("{DIM}✦ Generating {count} image(s)...{RESET}");
                let _ = std::io::stderr().flush();
                let request = ImageGenRequest {
                    prompt: prompt.clone(),
                    negative_prompt: negative,
                    aspect_ratio: Some(aspect),
                    num_images: Some(count),
                    model: None,
                    provider: None,
                };
                match generate_images(&config, &request).await {
                    Ok(result) => {
                        eprintln!(
                            "\r{MINT}✦ Generated {} image(s)         {RESET}",
                            result.images.len()
                        );
                        let data_uris: Vec<String> = result
                            .images
                            .iter()
                            .map(|img| img.data_uri.clone())
                            .collect();
                        match mint_core::save_chat_images(
                            data_uris,
                            Some(result.provider.clone()),
                            Some(prompt.clone()),
                        ) {
                            Ok(saved) => {
                                for entry in &saved {
                                    println!("{MINT}✓{RESET} Saved: {}", entry.path.display());
                                }
                                // If --output specified, copy first image there
                                if let (Some(out_path), Some(first)) = (&output, saved.first()) {
                                    match std::fs::copy(&first.path, out_path) {
                                        Ok(_) => println!(
                                            "{MINT}✓{RESET} Copied to: {}",
                                            out_path.display()
                                        ),
                                        Err(e) => eprintln!(
                                            "{WARN}Warning: could not copy to output path: {e}{RESET}"
                                        ),
                                    }
                                }
                                if let Some(desc) = &result.description
                                    && !desc.is_empty()
                                {
                                    println!("\n{DIM}{desc}{RESET}");
                                }
                            }
                            Err(e) => eprintln!("{ERROR}Failed to save images: {e}{RESET}"),
                        }
                    }
                    Err(e) => {
                        eprintln!("{ERROR}✗ Image generation failed: {e}{RESET}");
                        anyhow::bail!("image generation failed: {e}");
                    }
                }
            }
        },
    }
    Ok(())
}

async fn launch_mint_target(target: String) -> Result<()> {
    match target.as_str() {
        "cli" => {
            println!("{MINT}Starting CLI Interactive Chat Assistant...{RESET}\n");
            run_interactive_chat().await?;
        }
        "app_link" => {
            const APP_URL: &str = "https://mint.aemeth.xyz";
            println!("{MINT}Opening Mint App Link...{RESET}");
            println!("{BLUE}Open app:{RESET} {APP_URL}\n");
            open_system_handler(APP_URL)?;
        }
        "web" => {
            println!(
                "{MINT}Launching Web App (vite) in background...{RESET} {DIM}(Vite Dev UI at http://localhost:9000){RESET}"
            );
            let project_root = {
                let mut found = None;
                if let Ok(exe_path) = std::env::current_exe() {
                    let mut path = exe_path.parent();
                    while let Some(p) = path {
                        if p.join("package.json").exists() {
                            found = Some(p.to_path_buf());
                            break;
                        }
                        path = p.parent();
                    }
                }
                if found.is_none()
                    && let Ok(cwd) = std::env::current_dir()
                {
                    let mut path = Some(cwd.as_path());
                    while let Some(p) = path {
                        if p.join("package.json").exists() {
                            found = Some(p.to_path_buf());
                            break;
                        }
                        path = p.parent();
                    }
                }
                found.ok_or_else(|| {
                    anyhow::anyhow!("Failed to find project root directory containing package.json")
                })?
            };
            std::process::Command::new("npm")
                .current_dir(&project_root)
                .args(["run", "dev:web"])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to launch web app: {e}"))?;

            println!("{MINT}Starting local API server in foreground on port 3000...{RESET}\n");
            let local_ip_msg = if let Some(ip) = mint_core::api_server::get_local_ip() {
                format!("http://localhost:9000 (or http://{}:9000 from mobile)", ip)
            } else {
                "http://localhost:9000".to_string()
            };
            println!(
                "{BLUE}Please open {RESET}{}{BLUE} in your web browser to access the Mint Web UI.{RESET}\n",
                local_ip_msg
            );
            mint_core::start_api_server(3000).await?;
        }
        _ => {}
    }

    Ok(())
}

fn is_table_line(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with('|') && trimmed.ends_with('|') && trimmed.len() > 1
}

fn process_inline_bold(s: &str) -> String {
    let mut result = String::new();
    let parts = s.split("**");
    let mut is_bold = false;
    for part in parts {
        if is_bold {
            result.push_str("\x1b[1m");
            result.push_str(part);
            result.push_str("\x1b[0m");
        } else {
            result.push_str(part);
        }
        is_bold = !is_bold;
    }
    result
}

fn format_line(line: &str) -> String {
    let mut formatted = line.to_string();
    let trimmed = line.trim_start();
    if (trimmed.starts_with("- ") || trimmed.starts_with("* ") || trimmed.starts_with("+ "))
        && trimmed.len() >= 2
    {
        let leading_len = line.len() - trimmed.len();
        let leading_spaces = &line[..leading_len];
        formatted = format!("{}•{}", leading_spaces, &trimmed[1..]);
    }
    process_inline_bold(&formatted)
}

fn unicode_width(s: &str) -> usize {
    let mut count = 0;
    for c in s.chars() {
        let val = c as u32;
        if (0x0E31..=0x0E31).contains(&val)
            || (0x0E34..=0x0E3A).contains(&val)
            || (0x0E47..=0x0E4E).contains(&val)
        {
            continue;
        }
        if is_cjk(c) {
            count += 2;
        } else {
            count += 1;
        }
    }
    count
}

fn is_cjk(c: char) -> bool {
    let val = c as u32;
    (0x4E00..=0x9FFF).contains(&val)
        || (0x3400..=0x4DBF).contains(&val)
        || (0x20000..=0x2A6DF).contains(&val)
        || (0xAC00..=0xD7AF).contains(&val)
        || (0x3040..=0x309F).contains(&val)
        || (0x30A0..=0x30FF).contains(&val)
}

fn render_markdown_table(table_lines: &[String]) -> String {
    let mut rows: Vec<Vec<String>> = Vec::new();
    for line in table_lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let content = &trimmed[1..trimmed.len() - 1];
        let content_escaped = content.replace("\\|", "\u{0000}");
        let cols: Vec<String> = content_escaped
            .split('|')
            .map(|s| s.replace("\u{0000}", "|").trim().to_string())
            .collect();
        rows.push(cols);
    }

    if rows.is_empty() {
        return String::new();
    }

    let mut has_separator = false;
    let mut separator_idx = None;
    for (i, row) in rows.iter().enumerate() {
        if row.iter().all(|col| {
            col.chars()
                .all(|c| c == '-' || c == ':' || c == ' ' || c == '\t')
        }) && !row.is_empty()
        {
            has_separator = true;
            separator_idx = Some(i);
            break;
        }
    }

    let mut data_rows: Vec<Vec<String>> = Vec::new();
    let mut header_row: Option<Vec<String>> = None;

    if has_separator {
        let sep_idx = separator_idx.unwrap();
        if sep_idx > 0 {
            header_row = Some(rows[sep_idx - 1].clone());
            for (i, row) in rows.iter().enumerate() {
                if i != sep_idx && i != sep_idx - 1 {
                    data_rows.push(row.clone());
                }
            }
        } else {
            for (i, row) in rows.iter().enumerate() {
                if i != sep_idx {
                    data_rows.push(row.clone());
                }
            }
        }
    } else {
        header_row = Some(rows[0].clone());
        data_rows = rows[1..].to_vec();
    }

    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let mut col_widths = vec![0; num_cols];

    if let Some(ref header) = header_row {
        for (i, col) in header.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(unicode_width(col));
            }
        }
    }
    for row in &data_rows {
        for (i, col) in row.iter().enumerate() {
            if i < num_cols {
                col_widths[i] = col_widths[i].max(unicode_width(col));
            }
        }
    }

    let mut rendered = String::new();

    let draw_border =
        |left: &str, mid: &str, right: &str, fill: &str, widths: &[usize]| -> String {
            let mut s = left.to_string();
            for (i, &w) in widths.iter().enumerate() {
                s.push_str(&fill.repeat(w + 2));
                if i < widths.len() - 1 {
                    s.push_str(mid);
                }
            }
            s.push_str(right);
            s.push('\n');
            s
        };

    rendered.push_str(&draw_border("┌", "┬", "┐", "─", &col_widths));

    if let Some(ref header) = header_row {
        rendered.push('│');
        for (i, col) in header.iter().enumerate() {
            let width = unicode_width(col);
            let padding = col_widths[i] - width;
            rendered.push_str(&format!(
                " \x1b[1;36m{}\x1b[0m{}",
                col,
                " ".repeat(padding + 1)
            ));
            rendered.push('│');
        }
        rendered.push('\n');
        rendered.push_str(&draw_border("├", "┼", "┤", "─", &col_widths));
    }

    for (r_idx, row) in data_rows.iter().enumerate() {
        rendered.push('│');
        for i in 0..num_cols {
            let col_val = row.get(i).cloned().unwrap_or_default();
            let width = unicode_width(&col_val);
            let padding = col_widths[i] - width;
            rendered.push_str(&format!(" {}{}", col_val, " ".repeat(padding + 1)));
            rendered.push('│');
        }
        rendered.push('\n');

        if r_idx < data_rows.len() - 1 {
            rendered.push_str(&draw_border("├", "┼", "┤", "─", &col_widths));
        }
    }

    rendered.push_str(draw_border("└", "┴", "┘", "─", &col_widths).trim_end());
    rendered
}

struct CliStreamFormatter {
    line_buffer: String,
    table_buffer: Vec<String>,
    in_code_block: bool,
}

impl CliStreamFormatter {
    fn new() -> Self {
        Self {
            line_buffer: String::new(),
            table_buffer: Vec::new(),
            in_code_block: false,
        }
    }

    fn process_char(&mut self, c: char, mut print_fn: impl FnMut(char)) {
        self.line_buffer.push(c);
        if c == '\n' {
            let mut line = self.line_buffer.clone();
            self.line_buffer.clear();
            if line.ends_with('\n') {
                line.pop();
            }

            let trimmed = line.trim();
            if trimmed.starts_with("```") {
                self.in_code_block = !self.in_code_block;
            }

            if self.in_code_block {
                if !self.table_buffer.is_empty() {
                    self.flush_table(&mut print_fn);
                }
                for ch in line.chars() {
                    print_fn(ch);
                }
                print_fn('\n');
                for ch in "  ".chars() {
                    print_fn(ch);
                }
            } else {
                if is_table_line(&line) {
                    self.table_buffer.push(line);
                } else {
                    if !self.table_buffer.is_empty() {
                        self.flush_table(&mut print_fn);
                    }
                    let formatted = format_line(&line);
                    for ch in formatted.chars() {
                        print_fn(ch);
                    }
                    print_fn('\n');
                    for ch in "  ".chars() {
                        print_fn(ch);
                    }
                }
            }
        }
    }

    fn finalize(&mut self, mut print_fn: impl FnMut(char)) {
        if !self.line_buffer.is_empty() {
            let mut line = self.line_buffer.clone();
            self.line_buffer.clear();
            if line.ends_with('\n') {
                line.pop();
            }
            if self.in_code_block {
                if !self.table_buffer.is_empty() {
                    self.flush_table(&mut print_fn);
                }
                for ch in line.chars() {
                    print_fn(ch);
                }
            } else if is_table_line(&line) {
                self.table_buffer.push(line);
            } else {
                if !self.table_buffer.is_empty() {
                    self.flush_table(&mut print_fn);
                }
                let formatted = format_line(&line);
                for ch in formatted.chars() {
                    print_fn(ch);
                }
            }
        }
        if !self.table_buffer.is_empty() {
            self.flush_table(&mut print_fn);
        }
    }

    fn flush_table<F: FnMut(char)>(&mut self, mut print_fn: F) {
        let rendered = render_markdown_table(&self.table_buffer);
        for ch in rendered.chars() {
            print_fn(ch);
        }
        print_fn('\n');
        self.table_buffer.clear();
    }
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
                println!("{WARN}System action:{RESET} opening {args}...\n");
                open_system_handler(args)?;
            }
            "open-app" => {
                println!("{WARN}System action:{RESET} launching app {args}...\n");
                launch_desktop_app(args)?;
            }
            "read-file" => {
                println!("{WARN}System action:{RESET} reading file {args}...\n");
                let path = PathBuf::from(args);
                read_file_content(&path)?;
            }
            "read-folder" => {
                println!("{WARN}System action:{RESET} reading folder {args}...\n");
                let path = PathBuf::from(args);
                read_folder_content(&path)?;
            }
            "run-shell" => {
                println!("{WARN}System action:{RESET} run local shell command");
                println!("  {args}");
                if confirm_shell_execution()? {
                    let output = run_shell_command(args, &std::env::current_dir()?, true, config)?;
                    print_shell_output(&output);
                } else {
                    println!("Shell command cancelled.\n");
                }
            }
            _ => {
                println!("{ERROR}Unknown system action:{RESET} {cmd} with args {args}\n");
            }
        }
    } else {
        match trimmed {
            "read-folder" => {
                println!("{WARN}System action:{RESET} reading folder . ...\n");
                read_folder_content(&PathBuf::from("."))?;
            }
            _ => {
                println!("{ERROR}Invalid action format:{RESET} {trimmed}\n");
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
            println!("\n{BLUE}────────────────────────────────────────────{RESET}");
            println!("{MINT}  Mint Interactive Commands{RESET}");
            println!("{BLUE}────────────────────────────────────────────{RESET}");
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
                (
                    "/plugins <name> [prompt]",
                    "Generate a skill.md file for a plugin/skill",
                ),
                ("/memory list", "Show recent interactions"),
                ("/memory clear", "Clear all interactions"),
                ("/memory get <key>", "Read a profile value"),
                ("/memory set <key> <val>", "Store a profile value"),
                (
                    "/image-provider [name]",
                    "List image gen providers or switch default provider",
                ),
                ("/mcp list", "List configured MCP servers"),
                ("/mcp allow <server> <tool>", "Allow an MCP tool"),
                ("/stats", "Show session statistics"),
                ("/exit", "Exit Mint"),
                ("/code <task>", "Run in code-agent mode"),
                (
                    "/multi-agent [on|off]",
                    "Show or toggle Multi-Agent Collaboration system",
                ),
            ];
            for (cmd_name, desc) in &commands {
                println!("  {MINT}{:<30}{RESET} {DIM}{}{RESET}", cmd_name, desc);
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
                    println!("{WARN}/fast usage: /fast [on|off]{RESET}");
                    return Some(SlashResult::Handled);
                }
            };
            if session.fast_mode {
                println!("{DIM}[Fast] mode ON — thinking traces hidden{RESET}\n");
            } else {
                println!("{DIM}[Fast] mode OFF{RESET}\n");
            }
            Some(SlashResult::Handled)
        }

        "/models" => {
            if rest.is_empty() {
                println!("\n{BLUE}Configured providers:{RESET}");
                for p in session.config.available_providers() {
                    let active = if p == session.config.ai_provider.as_str() {
                        format!(" {MINT}← active{RESET}")
                    } else {
                        String::new()
                    };
                    println!("  {p}{active}");
                }
                println!();
            } else {
                session.config.ai_provider = rest.to_owned();
                match mint_core::save_config(&session.config) {
                    Ok(()) => {
                        let active_model =
                            active_model(&session.config.ai_provider, &session.config);
                        let display_name = format!(
                            "Changed model to {} • {}",
                            session.config.ai_provider, active_model
                        );

                        // Save system event interaction in memory
                        if let Ok(memory) = MemoryStore::open_default() {
                            let _ = memory.add_interaction_for_chat_with_fallback(
                                CHAT_CLI_ID,
                                &display_name,
                                "",
                                "system",
                                "provider_change",
                                None,
                            );
                        }

                        println!(
                            "\n{DIM}───{RESET} {MINT}{}{RESET} {DIM}───{RESET}\n",
                            display_name
                        );
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/multi-agent" => {
            if rest == "on" || rest == "off" {
                session.config.enable_agent_collaboration = rest == "on";
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Multi-Agent collaboration set to: {}{RESET}\n",
                        if session.config.enable_agent_collaboration {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /multi-agent [on|off]{RESET}\n");
            } else {
                println!("\n{BLUE}Multi-Agent System Settings:{RESET}");
                let collab_status = if session.config.enable_agent_collaboration {
                    format!("{MINT}Enabled (on){RESET}")
                } else {
                    format!("{DIM}Disabled (off){RESET}")
                };
                println!("  Global Collaboration: {collab_status}");
                println!("\n{BLUE}Configured Agents:{RESET}");
                if session.config.agents.is_empty() {
                    println!("  No agents configured.");
                } else {
                    for agent in &session.config.agents {
                        let status_label = if agent.enabled {
                            format!("{MINT}[Enabled]{RESET}")
                        } else {
                            format!("{DIM}[Disabled]{RESET}")
                        };
                        println!(
                            "  {:<15} {}  Provider: {:<12} Model: {}",
                            agent.name, status_label, agent.provider, agent.model
                        );
                        let truncated_instruction = if agent.system_instruction.len() > 60 {
                            format!("{}...", &agent.system_instruction[..60].replace('\n', " "))
                        } else {
                            agent.system_instruction.replace('\n', " ")
                        };
                        println!("    {DIM}Instruction: {truncated_instruction}{RESET}");
                    }
                }
                println!();
            }
            Some(SlashResult::Handled)
        }

        "/image-provider" => {
            let mut available = Vec::new();
            if !session.config.api_key.trim().is_empty() {
                available.push("nanobanana");
            }
            if !session.config.openai_api_key.trim().is_empty() {
                available.push("dalle");
            }
            if !session.config.stability_api_key.trim().is_empty() {
                available.push("stability");
            }
            if !session.config.ideogram_api_key.trim().is_empty() {
                available.push("ideogram");
            }
            if !session.config.replicate_api_key.trim().is_empty() {
                available.push("replicate");
            }
            if available.is_empty() {
                available.push("nanobanana");
            }

            if rest.is_empty() {
                println!("\n{BLUE}Configured image generation providers:{RESET}");
                for p in &available {
                    let active = if p == &session.config.image_gen_provider.as_str() {
                        format!(" {MINT}← active{RESET}")
                    } else {
                        String::new()
                    };
                    println!("  {p}{active}");
                }
                println!();
            } else {
                if available.contains(&rest) {
                    session.config.image_gen_provider = rest.to_owned();
                    match mint_core::save_config(&session.config) {
                        Ok(()) => println!(
                            "{DIM}Switched default image provider to: {}{RESET}\n",
                            session.config.image_gen_provider
                        ),
                        Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                    }
                } else {
                    println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/clear" | "/reset" => {
            println!("Clear conversation history? [y/N] ");
            if let Ok(true) = confirm("Clear conversation history? [y/N] ") {
                if let Ok(memory) = MemoryStore::open_default() {
                    match memory.clear_interactions() {
                        Ok(count) => println!("{DIM}Cleared {count} interactions.{RESET}"),
                        Err(error) => println!("{ERROR}Memory error:{RESET} {error}"),
                    }
                }
                println!("{DIM}Conversation context cleared.{RESET}\n");
            }
            Some(SlashResult::Handled)
        }

        "/cd" => {
            if rest.is_empty() {
                println!("{WARN}/cd requires a path{RESET}\n");
            } else {
                let new_dir = PathBuf::from(rest);
                if new_dir.is_dir() {
                    session.current_dir = new_dir.canonicalize().unwrap_or(new_dir);
                    println!(
                        "{DIM}Workspace: {}{RESET}\n",
                        format_path_with_tilde(&session.current_dir)
                    );
                } else {
                    println!("{ERROR}Directory not found:{RESET} {rest}\n");
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
                println!("{WARN}/image usage: /image <path> [prompt]{RESET}\n");
                return Some(SlashResult::Handled);
            }
            match image::load_image_as_data_uri(std::path::Path::new(img_path)) {
                Ok(uri) => {
                    if let Some(ref mut current) = session.pending_image {
                        current.push(' ');
                        current.push_str(&uri);
                    } else {
                        session.pending_image = Some(uri);
                    }
                    if prompt.is_empty() {
                        println!("{DIM}Image attached — type your prompt and press Enter{RESET}\n");
                        Some(SlashResult::Handled)
                    } else {
                        Some(SlashResult::ForwardToAgent(prompt.to_owned()))
                    }
                }
                Err(e) => {
                    println!("{ERROR}Failed to load image:{RESET} {e}\n");
                    Some(SlashResult::Handled)
                }
            }
        }

        "/paste" => match image::read_clipboard_image() {
            Ok(Some(uri)) => {
                if let Some(ref mut current) = session.pending_image {
                    current.push(' ');
                    current.push_str(&uri);
                } else {
                    session.pending_image = Some(uri);
                }
                if rest.is_empty() {
                    println!(
                        "{DIM}Clipboard image attached — type your prompt and press Enter{RESET}\n"
                    );
                    Some(SlashResult::Handled)
                } else {
                    Some(SlashResult::ForwardToAgent(rest.to_owned()))
                }
            }
            Ok(None) => {
                println!("{WARN}No image found in clipboard.{RESET}\n");
                Some(SlashResult::Handled)
            }
            Err(e) => {
                println!("{ERROR}Clipboard error:{RESET} {e}\n");
                Some(SlashResult::Handled)
            }
        },

        "/learn" => {
            if rest.is_empty() {
                let mut skills = match MemoryStore::open_default() {
                    Ok(m) => match m.learned_skills(100) {
                        Ok(s) => s,
                        Err(e) => {
                            println!("{ERROR}Error loading custom skills: {e}{RESET}\n");
                            return Some(SlashResult::Handled);
                        }
                    },
                    Err(e) => {
                        println!("{ERROR}Memory error: {e}{RESET}\n");
                        return Some(SlashResult::Handled);
                    }
                };

                if let Some(home) = dirs::home_dir() {
                    let global_skills_path = home.join(".config").join("mint").join("mint-skills");
                    mint_core::skills::load_skills_from_dir(&global_skills_path, &mut skills);
                }
                let workspace_skills_path1 = session.current_dir.join(".agents").join("skills");
                mint_core::skills::load_skills_from_dir(&workspace_skills_path1, &mut skills);
                let workspace_skills_path2 = session.current_dir.join("skills");
                mint_core::skills::load_skills_from_dir(&workspace_skills_path2, &mut skills);

                let mut unique_skills = std::collections::BTreeMap::new();
                for skill in skills {
                    let loc = if skill.source_path.contains("/.config/mint/mint-skills") {
                        "Global"
                    } else if skill.source_path.contains("/skills")
                        || skill.source_path.contains("/.agents/skills")
                    {
                        "Workspace"
                    } else {
                        "Taught"
                    };
                    unique_skills.insert(skill.name.clone(), (skill, loc));
                }

                if unique_skills.is_empty() {
                    println!("No learned skills found. Use '/learn <path>' to add one.\n");
                } else {
                    println!("Learned AI Skills:");
                    for (name, (skill, loc)) in &unique_skills {
                        if *loc == "Taught" {
                            println!("  ● [{}] {}", loc, name);
                        } else {
                            println!("  ● [{}] {} (Source: {})", loc, name, skill.source_path);
                        }
                    }
                    println!();
                }
            } else {
                let path = PathBuf::from(rest);
                let path = if path.is_absolute() {
                    path
                } else {
                    session.current_dir.join(path)
                };
                match skills::learn(&path) {
                    Ok(skill) => println!(
                        "{DIM}Learned skill: {} ({}){RESET}\n",
                        skill.name, skill.source_path
                    ),
                    Err(error) => println!("{ERROR}Learn error:{RESET} {error}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/memory" => {
            let memory = match MemoryStore::open_default() {
                Ok(m) => m,
                Err(e) => {
                    println!("{ERROR}Memory error:{RESET} {e}\n");
                    return Some(SlashResult::Handled);
                }
            };
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match memory.recent_interactions_for_chat(CHAT_CLI_ID, 10) {
                    Ok(items) => {
                        if items.is_empty() {
                            println!("{DIM}No interactions yet.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Recent interactions:{RESET}");
                            for item in items.iter().rev() {
                                println!(
                                    "  {DIM}[{}]{RESET} {BLUE}You:{RESET} {}",
                                    &item.created_at[..16.min(item.created_at.len())],
                                    if item.user_text.chars().count() > 80 {
                                        let short: String =
                                            item.user_text.chars().take(80).collect();
                                        format!("{}…", short)
                                    } else {
                                        item.user_text.clone()
                                    }
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "get" => {
                    if args.is_empty() {
                        println!("{WARN}/memory get <key>{RESET}\n");
                    } else {
                        match memory.get_profile(args) {
                            Ok(Some(val)) => println!("{val}\n"),
                            Ok(None) => println!("{DIM}(not set){RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "set" => {
                    let (key, val) = args
                        .split_once(char::is_whitespace)
                        .map(|(k, v)| (k, v.trim()))
                        .unwrap_or((args, ""));
                    if key.is_empty() {
                        println!("{WARN}/memory set <key> <value>{RESET}\n");
                    } else {
                        match memory.set_profile(key, val) {
                            Ok(()) => println!("{DIM}Stored {key}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "skills" => match memory.learned_skills(20) {
                    Ok(skills) => {
                        if skills.is_empty() {
                            println!("{DIM}No learned skills.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Learned skills:{RESET}");
                            for s in &skills {
                                println!("  [{}] {} — {}", s.id, s.name, s.source_path);
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "clear" => match memory.clear_interactions_for_chat(CHAT_CLI_ID) {
                    Ok(count) => println!("{DIM}Cleared {count} interactions.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                _ => println!(
                    "{WARN}/memory usage: list | clear | get <key> | set <key> <val> | skills{RESET}\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/mcp" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match mcp::list() {
                    Ok(servers) => {
                        println!("\n{BLUE}MCP servers:{RESET}");
                        print_mcp_servers(&servers);
                    }
                    Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                },
                "allow" => {
                    let mut parts = args.split_whitespace();
                    let server = parts.next();
                    let tool = parts.next();
                    if let (Some(server), Some(tool)) = (server, tool) {
                        match mcp::allow(server, tool) {
                            Ok(true) => println!("{DIM}Allowed MCP tool: {server}/{tool}{RESET}\n"),
                            Ok(false) => {
                                println!("{DIM}MCP tool already allowed: {server}/{tool}{RESET}\n")
                            }
                            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                        }
                    } else {
                        println!("{WARN}/mcp allow usage: <server> <tool>{RESET}\n");
                    }
                }
                _ => println!("{WARN}/mcp usage: list | allow <server> <tool>{RESET}\n"),
            }
            Some(SlashResult::Handled)
        }

        "/stats" => {
            let provider = &session.config.ai_provider;
            let model = active_model(provider, &session.config);
            let interactions = MemoryStore::open_default()
                .and_then(|m| m.recent_interactions_for_chat(CHAT_CLI_ID, 1000))
                .map(|v| v.len())
                .unwrap_or(0);
            println!("\n{BLUE}─ Session Stats ─────────────────────────{RESET}");
            println!("  Provider : {MINT}{provider}{RESET}");
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
            if let Some(ref img_data) = session.pending_image {
                let count = img_data.split_whitespace().count();
                if count > 1 {
                    println!("  Images   : {WARN}{} images attached{RESET}", count);
                } else {
                    println!("  Image    : {WARN}attached{RESET}");
                }
            }
            println!();
            Some(SlashResult::Handled)
        }

        "/exit" | "/quit" => Some(SlashResult::Exit),

        "/plugins" | "/plugin" => {
            let (plugin_name, prompt) = rest
                .split_once(char::is_whitespace)
                .map(|(p, r)| (p, r.trim()))
                .unwrap_or((rest, ""));

            if plugin_name.is_empty() {
                println!("\n{BLUE}────────────────────────────────────────────{RESET}");
                println!("{MINT}  Available Plugins & Skills{RESET}");
                println!("{BLUE}────────────────────────────────────────────{RESET}");

                println!("{MINT}Native Plugins:{RESET}");
                for p in native_plugins() {
                    println!("  - {BLUE}{:<20}{RESET} {}", p.name, p.description);
                }

                println!("\n{MINT}Custom Skills/Plugins (Active):{RESET}");
                let mut active_skills = std::collections::HashSet::new();
                if let Ok(memory) = MemoryStore::open_default() {
                    match memory.learned_skills(100) {
                        Ok(skills) => {
                            if skills.is_empty() {
                                println!(
                                    "  {DIM}(No custom skills/plugins learned yet. Use '/learn <path>' to add one.){RESET}"
                                );
                            } else {
                                for s in &skills {
                                    println!("  - {BLUE}{:<20}{RESET} {}", s.name, s.source_path);

                                    // Try to store canonicalized active path
                                    if let Ok(canon) = Path::new(&s.source_path).canonicalize() {
                                        active_skills.insert(canon);
                                    } else {
                                        active_skills.insert(PathBuf::from(&s.source_path));
                                    }
                                }
                            }
                        }
                        Err(e) => println!("  {ERROR}Error loading custom skills: {e}{RESET}"),
                    }
                }

                // Scan local workspace skills directory
                let local_skills_dir = session.current_dir.join("skills");
                if local_skills_dir.is_dir()
                    && let Ok(entries) = fs::read_dir(&local_skills_dir)
                {
                    let mut local_files = Vec::new();
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.is_file()
                            && let Some(ext) = path.extension().and_then(|e| e.to_str())
                            && matches!(ext.to_ascii_lowercase().as_str(), "md" | "txt")
                            && let Some(name) = path.file_name().and_then(|n| n.to_str())
                        {
                            if let Ok(canon_path) = path.canonicalize() {
                                local_files.push((name.to_owned(), canon_path));
                            } else {
                                local_files.push((name.to_owned(), path));
                            }
                        }
                    }

                    let inactive_files: Vec<_> = local_files
                        .into_iter()
                        .filter(|(_, full_path)| !active_skills.contains(full_path))
                        .map(|(name, _)| name)
                        .collect();

                    if !inactive_files.is_empty() {
                        println!(
                            "\n{MINT}Local Skill Files (Found but not learned/active yet):{RESET}"
                        );
                        for file in inactive_files {
                            println!(
                                "  - {WARN}{:<20}{RESET} skills/{} {DIM}(Type '/learn skills/{}' to activate){RESET}",
                                file, file, file
                            );
                        }
                    }
                }

                println!();
                Some(SlashResult::Handled)
            } else {
                let agent_prompt = if prompt.is_empty() {
                    format!(
                        "Create a skill markdown file for the plugin '{}' at the path 'skills/{}.md' in the workspace. Write the appropriate instructions inside.",
                        plugin_name, plugin_name
                    )
                } else {
                    format!(
                        "Create a skill markdown file for the plugin '{}' at the path 'skills/{}.md' in the workspace based on these instructions: {}",
                        plugin_name, plugin_name, prompt
                    )
                };
                Some(SlashResult::ForwardToAgent(agent_prompt))
            }
        }

        "/code" => {
            if rest.is_empty() {
                println!("{WARN}/code requires a task description{RESET}\n");
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
    let clean_provider_name = format_provider_display_name(provider, &config);
    let line1_text = format!("[Mint] v{} | Active AI: {}", version, clean_provider_name);
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
            "{MINT} __  __ _       _    ___ _    ___ {RESET}   {DIM}╭{}╮{RESET}",
            "─".repeat(border_len)
        );
        println!(
            "{MINT}|  \\/  (_)_ __ | |_ / __| |  |_ _|{RESET}   {DIM}│{RESET} {MINT}[Mint]{RESET} v{} | Active AI: {}{} {DIM}│{RESET}",
            version,
            clean_provider_name,
            " ".repeat(content_width - len1)
        );
        println!(
            "{MINT}| |\\/| | | '_ \\|  _| (__| |__ | | {RESET}   {DIM}│{RESET} {DIM}{}{}{RESET} {DIM}│{RESET}",
            line2_text,
            " ".repeat(content_width - len2)
        );
        println!(
            "{MINT}|_|  |_|_|_| |_|\\__|\\___|\\___|___|{RESET}   {DIM}╰{}╯{RESET}",
            "─".repeat(border_len)
        );
    } else {
        println!("{DIM}╭{}╮{RESET}", "─".repeat(border_len));
        println!(
            "{DIM}│{RESET} {MINT}[Mint]{RESET} v{} | Active AI: {}{} {DIM}│{RESET}",
            version,
            clean_provider_name,
            " ".repeat(content_width - len1)
        );
        println!(
            "{DIM}│{RESET} {DIM}{}{}{RESET} {DIM}│{RESET}",
            line2_text,
            " ".repeat(content_width - len2)
        );
        println!("{DIM}╰{}╯{RESET}", "─".repeat(border_len));
        println!("{MINT} __  __ _       _    ___ _    ___ {RESET}");
        println!("{MINT}|  \\/  (_)_ __ | |_ / __| |  |_ _|{RESET}");
        println!("{MINT}| |\\/| | | '_ \\|  _| (__| |__ | | {RESET}");
        println!("{MINT}|_|  |_|_|_| |_|\\__|\\___|\\___|___|{RESET}");
    }
    println!("Type naturally or /help for commands. Ctrl+V pastes images. Ctrl+D exits.\n");

    let mut session = InteractiveSession {
        config,
        current_dir: current_dir.clone(),
        fast_mode: false,
        pending_image: None,
    };

    let mut printed_update = false;
    if let Some((current, latest)) = updater::get_cached_update_notice() {
        updater::print_update_notice(&current, &latest);
        printed_update = true;
    }

    let mut update_handle = if updater::should_check_for_update() {
        Some(tokio::task::spawn_blocking(
            updater::check_for_update_quietly,
        ))
    } else {
        None
    };

    loop {
        if let Some(handle) = update_handle.take() {
            if handle.is_finished() {
                if let Ok(Some((current, latest))) = handle.await
                    && !printed_update
                {
                    updater::print_update_notice(&current, &latest);
                    printed_update = true;
                }
            } else {
                update_handle = Some(handle);
            }
        }

        let path_str = format_path_with_tilde(&session.current_dir);
        let model_str = active_model(&session.config.ai_provider, &session.config).to_owned();

        if let Some(input) =
            read_line_interactive(&session.config.ai_provider, &model_str, &path_str)?
        {
            if let Some(uri) = input.pasted_image {
                if let Some(ref mut current) = session.pending_image {
                    current.push(' ');
                    current.push_str(&uri);
                } else {
                    session.pending_image = Some(uri);
                }
            }
            let query_str = input.text.trim().to_owned();
            if query_str.is_empty() {
                continue;
            }

            // Run slash-command router
            match handle_slash_command(&mut session, &query_str).await {
                Some(SlashResult::Handled) => continue,
                Some(SlashResult::Exit) => {
                    print_exit_message(&session);
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
                        println!("{ERROR}Error:{RESET} {error}\n");
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
                    println!("{ERROR}Error:{RESET} {error}\n");
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
                    println!("{ERROR}Error:{RESET} {error}\n");
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
            print!("  {MINT}Mint:{RESET} {DIM}Thinking...{RESET}");
            let _ = io::stdout().flush();

            let mut system_instruction = "You are Mint, a cute and helpful AI assistant. You speak in a polite, friendly, and sweet Thai tone (using \"คุณ\", \"ค่ะ\", \"นะคะ\"). \
                You are running inside the Mint CLI interactive chat. \
                You have access to native system actions to help the user! If the user asks you to open a website, launch an app, read a file, list a folder, run code, run tests, or execute a local shell command, you can execute these actions by writing a special block at the very end of your response: \
                `[ACTION: <command> <arguments>]` \
                The available actions are: \
                - `[ACTION: open <url_or_path>]` to open a URL or a folder path. \
                - `[ACTION: open-app <app_name>]` to launch a desktop application. \
                - `[ACTION: read-file <file_path>]` to read the contents of a file. \
                - `[ACTION: read-folder <path>]` to list files/folders in a directory. \
                - `[ACTION: run-shell <command>]` to run a non-destructive local shell command after approval. \
                You may discuss mature personal topics at a non-explicit level, including adult relationships, sex education, intimacy, emotions, and feelings. Keep the tone respectful and supportive, avoid graphic sexual detail, and do not engage with sexual content involving minors, coercion, exploitation, or sexual violence. \
                Write the action block on a single line at the very end of your response.".to_string();
            if let Ok(memory) = MemoryStore::open_default()
                && let Ok(Some(name)) = memory.get_profile("name")
            {
                system_instruction.push_str(&format!(
                    "\nThe user's name is {}. Refer to them by their name when appropriate.",
                    name
                ));
            }

            let image_uri = session.pending_image.take();
            let sent_image = image_uri.clone();
            let mut first_chunk = true;
            let mut filter = ActionStreamFilter::new();
            let mut formatter = CliStreamFormatter::new();

            let stream_result = orchestrate_chat_stream_with_fallback(
                &session.config,
                &ChatRequest {
                    message: message.clone(),
                    system_instruction,
                    chat_id: Some(CHAT_CLI_ID.to_owned()),
                    image_data_uri: image_uri,
                    audio_data_uri: None,
                    document_attachment: None,
                    workspace_path: None,
                    agent_id: None,
                },
                |chunk| {
                    if first_chunk {
                        first_chunk = false;
                        print!("\r\x1b[2K  {MINT}Mint:{RESET} ");
                    }
                    filter.process_chunk(&chunk, |text| {
                        for c in text.chars() {
                            formatter.process_char(c, |formatted_c| {
                                print!("{}", formatted_c);
                            });
                        }
                    });
                    let _ = io::stdout().flush();
                },
            )
            .await;

            let actions = filter.finalize(|text| {
                for c in text.chars() {
                    formatter.process_char(c, |formatted_c| {
                        print!("{}", formatted_c);
                    });
                }
            });
            formatter.finalize(|formatted_c| {
                print!("{}", formatted_c);
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
                                "{DIM}{} • {} → fallback: {} • {}{RESET}",
                                session.config.ai_provider,
                                active_model(&session.config.ai_provider, &session.config),
                                fb_provider,
                                response.model
                            )
                        } else {
                            format!("{DIM}{} • {}{RESET}", response.provider, response.model)
                        };
                        println!("{badge}");
                        println!("{DIM}{}{RESET}\n", "─".repeat(width));
                    }
                    for action in actions {
                        if let Err(e) = execute_action(&action, &session.config) {
                            println!("{ERROR}Error executing action:{RESET} {e}\n");
                        }
                    }
                }
                Err(e) => {
                    if first_chunk {
                        print!("\r\x1b[2K");
                    }
                    println!("{ERROR}Error:{RESET} {e}\n");
                }
            }
        } else {
            print_exit_message(&session);
            break;
        }
    }

    Ok(())
}

fn print_exit_message(session: &InteractiveSession) {
    println!("\n{MINT}──────────────── Mint session closed ────────────────{RESET}");
    let clean_provider = format_provider_display_name(&session.config.ai_provider, &session.config);
    println!(
        "{DIM}Provider:{RESET} {} {DIM}• Model:{RESET} {}",
        clean_provider,
        active_model(&session.config.ai_provider, &session.config)
    );
    println!(
        "{DIM}Workspace:{RESET} {}",
        format_path_with_tilde(&session.current_dir)
    );
    println!("{DIM}Saved config stays available for the next Mint run.{RESET}");
    println!("{MINT}See you next time.{RESET}\n");
}

fn format_provider_display_name(provider: &str, config: &mint_core::MintConfig) -> String {
    if provider.starts_with("custom:") {
        let id = provider.strip_prefix("custom:").unwrap_or(provider);
        let cp = config.custom_providers.iter().find(|p| p.id == id);
        match cp {
            Some(p) if !p.display_name.is_empty() => format!("{} (Custom)", p.display_name),
            Some(p) => format!("{} (Custom)", p.id),
            None => format!("{} (Custom)", id),
        }
    } else {
        match provider {
            "gemini" => "Gemini".to_string(),
            "openai" => "OpenAI".to_string(),
            "openrouter" => "OpenRouter".to_string(),
            "deepseek" => "DeepSeek".to_string(),
            "anthropic" => "Claude".to_string(),
            "huggingface" => "HF".to_string(),
            "local_openai" => "Local".to_string(),
            "ollama" => "Ollama".to_string(),
            _ => {
                let mut chars = provider.chars();
                match chars.next() {
                    None => String::new(),
                    Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                }
            }
        }
    }
}

const AUTOCOMPLETE_COMMANDS: &[(&str, &str)] = &[
    ("/cd", "Change active workspace directory"),
    ("/clear", "Clear conversation history"),
    ("/code", "Run in code-agent mode"),
    ("/exit", "Exit Mint CLI"),
    ("/fast", "Toggle fast mode (hide thinking traces)"),
    ("/help", "Show help menu"),
    ("/image", "Attach image from disk"),
    (
        "/image-provider",
        "List image gen providers or switch default provider",
    ),
    ("/learn", "Import persistent skill/instruction"),
    ("/mcp", "List configured MCP servers"),
    ("/memory", "Manage long-term memory store"),
    ("/models", "List AI providers or switch active provider"),
    (
        "/multi-agent",
        "Show or toggle Multi-Agent Collaboration system",
    ),
    ("/paste", "Attach image from clipboard"),
    ("/plugins", "List or generate plugins/skills"),
    ("/stats", "Show session statistics"),
];

fn draw_input_box(
    input_chars: &[char],
    cursor_pos: usize,
    placeholder: &str,
    model: &str,
    path_str: &str,
    tab_base_input: Option<&str>,
    tab_index: Option<usize>,
) -> (usize, usize) {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = term_width as usize;
    let prefix = "› ";
    let input_width = width.saturating_sub(2);
    let content_max_len = input_width.saturating_sub(prefix.chars().count());

    let mut start_idx = 0;
    if input_chars.len() > content_max_len && cursor_pos >= content_max_len {
        start_idx = (cursor_pos + 5).saturating_sub(content_max_len);
        if start_idx + content_max_len > input_chars.len() {
            start_idx = input_chars.len() - content_max_len;
        }
    }

    let slice_chars = if input_chars.len() > content_max_len {
        &input_chars[start_idx..std::cmp::min(start_idx + content_max_len, input_chars.len())]
    } else {
        input_chars
    };
    let displayed_input: String = slice_chars.iter().collect();

    let display_str = if displayed_input.is_empty() {
        format!("{DIM}{}\x1b[39m", placeholder)
    } else {
        format_placeholders(&displayed_input)
    };

    let visible_len = if displayed_input.is_empty() {
        placeholder.chars().count()
    } else {
        string_visual_width(&displayed_input)
    };

    let pad_len = content_max_len.saturating_sub(visible_len);
    let padding = " ".repeat(pad_len);
    let blank_line = " ".repeat(input_width);

    println!(" {COMPOSER_BG}{blank_line}{RESET}");
    println!(
        " {COMPOSER_BG}{MINT}{}{RESET}{COMPOSER_BG}{}{}{RESET}",
        prefix, display_str, padding
    );
    println!(" {COMPOSER_BG}{blank_line}{RESET}");

    let agent_str = format!(" {DIM}[Agent]{RESET} {MINT}{}{RESET}", model);
    let path_label = format!("path: {}", path_str);
    let agent_visible_len = " [Agent] ".len() + model.chars().count();
    let path_visible_len = path_label.chars().count();

    let status_pad_len = (width - 1).saturating_sub(agent_visible_len + path_visible_len);
    let status_padding = " ".repeat(status_pad_len);

    print!(
        "{}{}{}{}{}",
        agent_str, status_padding, DIM, path_label, RESET
    );

    // Compute and draw suggestions
    let raw_input: String = input_chars.iter().collect();
    let search_query = tab_base_input.unwrap_or(&raw_input);
    let mut match_count = 0;
    if search_query.starts_with('/') {
        let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(search_query))
            .collect();

        if !matches.is_empty() {
            let total_pages = matches.len().div_ceil(5);
            let highlight_idx = tab_index.map(|idx| idx % matches.len());
            let selected_idx = highlight_idx.unwrap_or(0);
            let current_page = selected_idx / 5;
            let start_idx = current_page * 5;
            let end_idx = std::cmp::min(start_idx + 5, matches.len());
            match_count = end_idx - start_idx;

            println!();
            println!(
                " {BLUE}Suggestions ({}/{}){RESET}",
                current_page + 1,
                total_pages
            );
            for i in start_idx..end_idx {
                let (cmd, desc) = matches[i];
                if Some(i) == highlight_idx {
                    println!("  {BLUE}▶ {:<16}{RESET} {DIM}- {}{RESET}", cmd, desc);
                } else {
                    println!("    {DIM}{:<16} - {}{RESET}", cmd, desc);
                }
            }
        }
    }

    let _ = io::stdout().flush();
    (match_count, start_idx)
}

fn input_cursor_column(input_chars: &[char], cursor_pos: usize) -> usize {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = term_width as usize;
    let prefix = "› ";
    let input_width = width.saturating_sub(2);
    let content_max_len = input_width.saturating_sub(prefix.chars().count());

    let mut start_idx = 0;
    if input_chars.len() > content_max_len && cursor_pos >= content_max_len {
        start_idx = (cursor_pos + 5).saturating_sub(content_max_len);
        if start_idx + content_max_len > input_chars.len() {
            start_idx = input_chars.len() - content_max_len;
        }
    }

    let cursor_pos_clamped = std::cmp::min(cursor_pos, input_chars.len());
    let start_idx_clamped = std::cmp::min(start_idx, cursor_pos_clamped);
    let visual_cursor_pos: usize = input_chars[start_idx_clamped..cursor_pos_clamped]
        .iter()
        .copied()
        .map(char_visual_width)
        .sum();
    4 + visual_cursor_pos
}

fn position_input_cursor(input_chars: &[char], cursor_pos: usize, match_count: usize) {
    let up_lines = if match_count > 0 { 4 + match_count } else { 2 };
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
    let (match_count, _) = draw_input_box(
        input_chars,
        cursor_pos,
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
    let mut pasted_image: Option<String> = None;
    let mut paste_contents: Vec<(String, String)> = Vec::new();
    let mut last_paste_time: Option<std::time::Instant> = None;

    // Track tab autocomplete state
    let mut tab_base_input: Option<String> = None;
    let mut tab_index: Option<usize> = None;

    let (match_count, _) = draw_input_box(
        &input_chars,
        cursor_pos,
        placeholder,
        model,
        path_str,
        None,
        None,
    );
    position_input_cursor(&input_chars, cursor_pos, match_count);
    let _ = io::stdout().flush();

    enable_raw_mode()?;
    let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableBracketedPaste);

    let result = loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            if let Event::Paste(text) = &ev {
                let clean_text = text.trim_end_matches(&['\r', '\n'][..]).to_string();
                let lines_count = clean_text.lines().count();
                if lines_count > 1 || clean_text.chars().count() > 100 {
                    let paste_id = paste_contents.len() + 1;
                    let placeholder_str = if lines_count > 1 {
                        format!("[Pasted text #{} +{} lines]", paste_id, lines_count - 1)
                    } else {
                        format!("[Pasted text #{}]", paste_id)
                    };
                    paste_contents.push((placeholder_str.clone(), clean_text));
                    for c in placeholder_str.chars() {
                        input_chars.insert(cursor_pos, c);
                        cursor_pos += 1;
                    }
                } else {
                    for c in clean_text.chars() {
                        input_chars.insert(cursor_pos, c);
                        cursor_pos += 1;
                    }
                }
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
                last_paste_time = Some(std::time::Instant::now());
                continue;
            }
            if let Event::Key(key_event) = ev
                && key_event.kind == event::KeyEventKind::Press
            {
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
                                "\r\x1b[3B\r\x1b[2K{WARN}Press Ctrl+D again to exit{RESET}\x1b[3A"
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
                            if let Some(ref mut current) = pasted_image {
                                current.push(' ');
                                current.push_str(&uri);
                            } else {
                                pasted_image = Some(uri);
                            }
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
                    KeyCode::Char(c) if input_chars.len() < 10000 => {
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
                    KeyCode::Backspace if cursor_pos > 0 => {
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
                    KeyCode::Enter => {
                        if let Some(time) = last_paste_time
                            && time.elapsed() < std::time::Duration::from_millis(100)
                        {
                            continue;
                        }
                        disable_raw_mode()?;
                        clear_input_box();
                        let input_str: String = input_chars.iter().collect();

                        let mut expanded_str = input_str.clone();
                        for (placeholder_str, content) in &paste_contents {
                            expanded_str = expanded_str.replace(placeholder_str, content);
                        }

                        println!("  {BLUE}You ›{RESET} {}", expanded_str);
                        let _ = io::stdout().flush();

                        break Some(InteractiveInput {
                            text: expanded_str,
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
    };

    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableBracketedPaste);
    Ok(result)
}

fn insert_image_placeholder(input_chars: &mut Vec<char>, cursor_pos: &mut usize) {
    let input: String = input_chars.iter().collect();
    let mut idx = 1;
    while input.contains(&format!("[Image #{}]", idx)) {
        idx += 1;
    }
    let placeholder = format!("[Image #{}]", idx);
    let placeholder_chars = placeholder.chars().collect::<Vec<_>>();
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

fn format_placeholders(s: &str) -> String {
    let mut result = String::new();
    let chars = s.chars().collect::<Vec<_>>();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' && i + 12 < chars.len() {
            let slice: String = chars[i..i + 13].iter().collect();
            if slice == "[Pasted text #"
                && let Some(end_offset) = chars[i..].iter().position(|&c| c == ']')
            {
                let end_idx = i + end_offset;
                let inside: String = chars[i + 1..end_idx].iter().collect();
                result.push('[');
                result.push_str(BLUE);
                result.push_str(&inside);
                result.push_str("\x1b[39m");
                result.push(']');
                i = end_idx + 1;
                continue;
            }
        }
        result.push(chars[i]);
        i += 1;
    }
    result
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
        "openrouter" => &config.openrouter_model,
        "deepseek" => &config.deepseek_model,
        "huggingface" => &config.hf_model,
        "local_openai" => &config.local_model_name,
        "ollama" => &config.ollama_model,
        p if p.starts_with("custom:") => {
            if let Some(id) = p.strip_prefix("custom:")
                && let Some(selections) = config
                    .extra
                    .get("customModelSelections")
                    .and_then(|v| v.as_object())
                && let Some(m) = selections.get(id).and_then(|v| v.as_str())
            {
                return m;
            }
            config
                .resolve_custom_provider(p)
                .and_then(|cp| cp.models.first())
                .map(|m| m.model_id.as_str())
                .unwrap_or("")
        }
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
            println!("{MINT}{}/{}", file_name_str, RESET);
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
        println!("  {} {MINT}Approve (session-wide){RESET}", clean_prompt);
        return Ok(true);
    }

    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() || enable_raw_mode().is_err() {
        print!("  {} [y/N] ", clean_prompt);
        let _ = io::stdout().flush();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        return Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ));
    }

    let _ = disable_raw_mode();
    println!("  {}", clean_prompt);

    let options = ["Approve", "Approve this session", "No"];
    let mut selected = 0;

    let print_choices = |selected: usize| -> Result<()> {
        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                println!("  {BLUE}❯ {}. {}{RESET}", i + 1, opt);
            } else {
                println!("    {DIM}{}. {}{RESET}", i + 1, opt);
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
        0 => format!("{MINT}Approve{RESET}"),
        1 => format!("{MINT}Approve this session{RESET}"),
        _ => format!("{ERROR}No{RESET}"),
    };
    println!("  {} {}", clean_prompt, result_str);
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
        "{DIM}[exit: {} | sandboxed: {}]{RESET}\n",
        output
            .status
            .map_or_else(|| "unknown".into(), |status| status.to_string()),
        output.sandboxed
    );
}

async fn run_github_overview(repo: &str, config: &MintConfig) -> Result<()> {
    let Some((owner, repo_name)) = parse_github_url(repo) else {
        anyhow::bail!(
            "Invalid GitHub repository URL/format. Please use 'owner/repo' or a full GitHub URL."
        );
    };

    println!(
        "Fetching information for {}/{} from GitHub...",
        owner, repo_name
    );
    let summary = match fetch_github_repo_summary(&owner, &repo_name).await {
        Ok(s) => s,
        Err(e) => {
            anyhow::bail!(
                "Failed to fetch repository summary: {}. Check that the repository is public and spelled correctly.",
                e
            );
        }
    };

    println!("Analyzing repository with AI model...");
    let prompt = format!(
        "Here is the metadata, top-level directory structure, and README.md content for the GitHub repository {}/{}:\n\n{}\n\nBased on this information, please provide a high-level overview of what this repository is about, what tech stack it uses, its overall architecture, and how it is organized.",
        owner, repo_name, summary
    );

    let (response, _) = orchestrate_chat_with_fallback(
        config,
        &ChatRequest {
            message: prompt,
            system_instruction: "You are a professional software architect providing a high-level overview of a code repository based on its metadata and README.".to_string(),
            chat_id: Some("github_review".to_string()),
            image_data_uri: None,
            audio_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
        },
    )
    .await?;

    println!(
        "\n--- AI Repository Overview for {}/{} ---",
        owner, repo_name
    );
    println!("{}", response.text);
    println!("--------------------------------------------------");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_one_image_placeholder_at_cursor() {
        let mut chars = "ask ".chars().collect::<Vec<_>>();
        let mut cursor = chars.len();

        insert_image_placeholder(&mut chars, &mut cursor);

        assert_eq!(chars.iter().collect::<String>(), "ask [Image #1]");
        assert_eq!(cursor, "ask [Image #1]".chars().count());
    }
}

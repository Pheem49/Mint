use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use mint_core::{
    CHAT_CLI_ID, Capability, ChatRequest, CodeEdit, CodePatchHunk, ImageGenRequest, KnowledgeStore,
    MemoryStore, MintConfig, TaskStore, apply_code_edits, assert_path_capability, build_code_patch,
    build_symbol_index, classify_shell_command, config_path, create_folder, execute_native_plugin,
    fetch_github_repo_summary, find_paths, generate_images, index_semantic_code, initialize_config,
    inspect_code_plan, list_code_files, load_config, native_plugins,
    orchestrate_chat_with_fallback, parse_github_url,
    propose_code_edits, read_code_file, repository_summary, run_shell_command, search_code,
    search_semantic_code, set_config_value,
};

mod actions;
mod agent;
mod gmail;
mod image;
mod interactive;
mod markdown;
mod mcp;
mod onboard;
mod setup;
mod skills;
mod updater;

pub use interactive::{confirm, active_model, print_welcome_banner, run_interactive_chat, SESSION_APPROVED};

pub const RESET: &str = "\x1b[0m";
pub const MINT: &str = "\x1b[32m";
pub const BLUE: &str = "\x1b[38;2;78;201;216m";
pub const DIM: &str = "\x1b[90m";
pub const ERROR: &str = "\x1b[31m";
pub const WARN: &str = "\x1b[33m";
pub const COMPOSER_BG: &str = "\x1b[48;2;35;39;45m";

pub(crate) async fn run_code_agent_with_saved_image(
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

pub(crate) fn print_mcp_servers(servers: &std::collections::BTreeMap<String, mint_core::mcp::McpServer>) {
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
                let config = load_config()?;
                print_welcome_banner(&config);
                println!("\n🚀 Starting Mint Browser Automation Environment...");
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
                let config = load_config()?;
                print_welcome_banner(&config);

                println!("\n{MINT}✔ Mint API Server is running!{RESET}\n");
                println!("    {BLUE}API Server URL:{RESET} {MINT}http://localhost:{}{RESET}\n", port);

                println!("Messaging Bridges Status:");
                
                let bridges = [
                    ("enableTelegramBridge", "Telegram Bot Bridge"),
                    ("enableDiscordBridge", "Discord Bot Bridge"),
                    ("enableSlackBridge", "Slack Bot Bridge"),
                    ("enableLineBridge", "LINE Bot Bridge"),
                    ("enableWhatsappBridge", "WhatsApp Cloud Bridge"),
                ];

                for &(key, name) in &bridges {
                    let enabled = config.extra.get(key).and_then(|v| v.as_bool()).unwrap_or(false);
                    if enabled {
                        println!("  {MINT}● {name:<23} [Active]{RESET}");
                    } else {
                        println!("  {DIM}○ {name:<23} [Inactive]{RESET}");
                    }
                }

                println!("\n{DIM}Press Ctrl+C to stop{RESET}\n");
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
                actions::print_shell_output(&output);
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
                actions::open_system_handler(&target)?;
            }
            Command::OpenApp { name } => {
                actions::launch_desktop_app(&name)?;
            }
            Command::ReadFile { path } => {
                actions::read_file_content(&path)?;
            }
            Command::ReadFolder { path } => {
                actions::read_folder_content(&path)?;
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
            actions::open_system_handler(APP_URL)?;
        }
        "web" => {
            let config = load_config()?;
            print_welcome_banner(&config);
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

            println!("\n{MINT}✔ Mint Web is running!{RESET}\n");
            println!("    {BLUE}Web UI:{RESET}     {MINT}http://localhost:9000{RESET}");
            if let Some(ip) = mint_core::api_server::get_local_ip() {
                println!("    {BLUE}Mobile:{RESET}     {MINT}http://{}:9000{RESET}", ip);
            }
            println!("    {BLUE}API Server:{RESET} {MINT}http://localhost:3000{RESET}\n");

            println!("Point your browser to:");
            println!("{MINT}http://localhost:9000{RESET}\n");

            println!("{DIM}Press Ctrl+C to stop{RESET}\n");
            mint_core::start_api_server(3000).await?;
        }
        _ => {}
    }

    Ok(())
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



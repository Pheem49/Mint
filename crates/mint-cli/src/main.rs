use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{
    fs,
    path::{Path, PathBuf},
};

use mint_core::{
    CHAT_CLI_ID, Capability, ChatRequest, CodeEdit, CodePatchHunk, CronStore, ImageGenRequest,
    KnowledgeStore, MemoryStore, MintConfig, TaskStore, add_linked_folder, apply_code_edits,
    assert_path_capability, build_code_patch,
    build_symbol_index, classify_shell_command, config_path, create_folder,
    fetch_github_repo_summary, find_paths, generate_images, index_semantic_code, initialize_config,
    inspect_code_plan, list_code_files, list_linked_folders, list_subagents, load_config,
    orchestrate_chat_with_fallback, parse_github_url, propose_code_edits, read_code_file,
    remove_linked_folder, repository_summary, run_shell_command, sandbox_availability, save_config,
    search_code, search_semantic_code, set_config_value,
};

mod actions;
mod agent;
mod background;
mod cron_wizard;
mod gmail;
mod hooks;
mod image;
mod interactive;
mod markdown;
mod mcp;
mod onboard;
mod plugins_cli;
mod setup;
mod skills;
mod updater;

pub use interactive::{
    SESSION_APPROVED, active_model, confirm, confirm_security, print_welcome_banner,
    run_interactive_chat,
};

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
    video_data_uri: Option<String>,
    options: agent::AgentOptions,
) -> Result<()> {
    let sent_image = image_data_uri.clone();
    let sent_video = video_data_uri.clone();
    agent::run_code_agent_with_options(
        task,
        current_dir,
        config,
        image_data_uri,
        video_data_uri,
        options,
    )
    .await?;
    // Save any attached images and videos that were sent with the task
    image::save_sent_image_after_send(sent_image.as_deref(), task);
    image::save_sent_image_after_send(sent_video.as_deref(), task);
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
    /// Manage scheduled/recurring agent tasks (cron jobs).
    Cron {
        #[command(subcommand)]
        command: CronCommand,
    },
    /// Search and create local folders through the native safety policy.
    Files {
        #[command(subcommand)]
        command: FilesCommand,
    },
    /// Manage built-in plugins & integrations (Gmail, Calendar, Notion, Spotify, Vercel, GitHub).
    #[command(alias = "plugin")]
    Plugins {
        #[command(subcommand)]
        command: Option<plugins_cli::PluginsSubcommand>,
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
    Web {
        /// Force development mode with Hot Module Replacement (HMR)
        #[arg(long, default_value_t = false)]
        dev: bool,
    },
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
    /// Manage linked folders that chat can auto-write notes into.
    Link {
        #[command(subcommand)]
        command: LinkCommand,
    },
    /// Manage PreToolUse/PostToolUse hooks.
    Hooks {
        #[command(subcommand)]
        command: HooksCommand,
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
    /// Generate a video from a text prompt using Google Veo.
    Veo {
        /// Text description of the video to generate.
        prompt: String,
        /// Aspect ratio: 16:9, 9:16, or 1:1 [default: 16:9]
        #[arg(long, default_value = "16:9")]
        aspect: String,
        /// Duration of the video in seconds (5 or 8) [default: 5]
        #[arg(long, default_value_t = 5)]
        duration: u32,
        /// Negative prompt — elements to avoid in the video
        #[arg(long)]
        negative: Option<String>,
        /// Save the generated video to this path
        #[arg(long)]
        output: Option<PathBuf>,
    },
    /// AI Video Editor — inspect, edit, and process local video files with FFmpeg.
    Video {
        #[command(subcommand)]
        command: VideoCommand,
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
    /// Re-run a server's OAuth flow in the foreground (e.g. after an
    /// expired/invalid refresh token) by invoking `<command> <args...> auth`.
    /// Only works for servers whose underlying package supports that
    /// convention (e.g. @pouyanafisi/gmail-mcp).
    Reauth {
        server: String,
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
enum LinkCommand {
    Add {
        name: String,
        path: PathBuf,
        #[arg(long)]
        description: Option<String>,
    },
    List,
    Remove {
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum HooksCommand {
    /// Add a hook. event: PreToolUse or PostToolUse. matcher: "*" or a comma/pipe-separated
    /// list of action names (e.g. "write_file,apply_patch"). command: a shell command that
    /// receives a JSON payload on stdin and, for PreToolUse, exits with code 2 to block.
    Add {
        event: String,
        matcher: String,
        command: String,
        #[arg(long)]
        timeout: Option<u64>,
    },
    List,
    Remove {
        index: usize,
    },
    Clear,
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
enum CronCommand {
    /// Create a new scheduled job. `schedule` accepts 5-field unix cron
    /// ("min hour dom month dow", e.g. "0 8 * * *" for every day at 08:00),
    /// or 6/7-field forms with seconds/year. Omit `--name`/`--schedule`/
    /// `--task` entirely to walk through an interactive wizard instead.
    Add {
        #[arg(long)]
        name: Option<String>,
        #[arg(long)]
        schedule: Option<String>,
        #[arg(long)]
        task: Option<String>,
        #[arg(long)]
        workspace: Option<PathBuf>,
        /// IANA timezone name (e.g. "Asia/Bangkok") to interpret `schedule`
        /// in. When set, `schedule` must give a specific minute/hour (and,
        /// for weekly/monthly/one-time schedules, a specific weekday/day/
        /// date) — no `*`, ranges, or steps in those fields — since it gets
        /// converted to the UTC expression the scheduler actually stores and
        /// evaluates. Omit to keep writing `schedule` in UTC directly, as
        /// before.
        #[arg(long)]
        timezone: Option<String>,
    },
    List,
    Show {
        id: String,
    },
    Remove {
        id: String,
    },
    Enable {
        id: String,
    },
    Disable {
        id: String,
    },
    /// Run a job immediately, without waiting for its schedule.
    RunNow {
        id: String,
    },
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
        /// Investigate read-only and require plan approval before editing files or running commands.
        #[arg(long)]
        plan: bool,
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
    /// List subagent definitions available to `dispatch_subagent` (global
    /// `~/.config/mint/mint-agents/` plus workspace `.agents/subagents/`).
    Subagents {
        #[arg(default_value = ".")]
        root: PathBuf,
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
    /// Manage persistent "always allow"/"always deny" agent approval rules.
    Permissions {
        #[command(subcommand)]
        command: PermissionsCommand,
    },
}

#[derive(Debug, Subcommand)]
enum PermissionsCommand {
    /// List saved permission rules.
    List,
    /// Remove a saved permission rule by its index (see `list`).
    Remove { index: usize },
}

#[derive(Debug, Subcommand)]
enum VideoCommand {
    /// Inspect a video file and print its metadata (duration, fps, resolution, etc.).
    Load {
        /// Path to the video file.
        path: PathBuf,
    },
    /// Trim a video between start and end seconds.
    Trim {
        /// Path to the input video.
        input: PathBuf,
        /// Trim start in seconds.
        #[arg(long)]
        start: f64,
        /// Trim end in seconds.
        #[arg(long)]
        end: f64,
        /// Output file path.
        #[arg(long)]
        output: PathBuf,
    },
    /// Resize a video. Use -1 for width or height to preserve aspect ratio.
    Resize {
        input: PathBuf,
        #[arg(long, default_value_t = -1)]
        width: i32,
        #[arg(long, default_value_t = -1)]
        height: i32,
        #[arg(long)]
        output: PathBuf,
    },
    /// Merge multiple video files into one.
    Merge {
        /// Input video files (space-separated).
        #[arg(required = true)]
        inputs: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Extract the audio track from a video as a WAV file.
    ExtractAudio {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    /// Remove silent sections from a video.
    RemoveSilence {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// Silence threshold in dB (default: -30)
        #[arg(long, default_value_t = -30.0)]
        threshold: f64,
        /// Minimum silence duration in seconds to remove (default: 0.5)
        #[arg(long, default_value_t = 0.5)]
        min_duration: f64,
    },
    /// Re-encode a video to a target resolution, fps, and codec.
    Export {
        input: PathBuf,
        #[arg(long)]
        output: PathBuf,
        /// e.g. "1920x1080", "1280x720"
        #[arg(long)]
        resolution: Option<String>,
        #[arg(long)]
        fps: Option<u32>,
        /// Video codec: libx264 (default), libx265, libvpx-vp9
        #[arg(long)]
        codec: Option<String>,
        /// CRF quality 0-51 (lower = better, default 23)
        #[arg(long)]
        crf: Option<u32>,
    },
    /// Transcribe speech in audio/video to an SRT subtitle file.
    Transcribe {
        input: PathBuf,
        #[arg(long)]
        language: Option<String>,
        /// Output .srt file path.
        #[arg(long)]
        output: PathBuf,
    },
    /// Burn subtitles into a video with style preset (default, tiktok, minimal).
    Subtitle {
        input: PathBuf,
        /// Path to .srt file or raw SRT string content.
        #[arg(long)]
        srt: String,
        /// Style preset: default, tiktok, minimal
        #[arg(long, default_value = "default")]
        style: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Translate an SRT subtitle file to another language.
    TranslateSubtitle {
        srt: PathBuf,
        #[arg(long)]
        lang: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Automatically make viral vertical Shorts clips from a video.
    MakeShorts {
        input: PathBuf,
        /// Destination folder for generated shorts clips.
        #[arg(long)]
        output_dir: Option<PathBuf>,
        /// Number of clips to generate (default: 3).
        #[arg(long, default_value_t = 3)]
        clips: u32,
        /// Target duration in seconds for each clip (default: 60).
        #[arg(long, default_value_t = 60.0)]
        duration: f64,
        /// Disable burning vertical TikTok-style subtitles.
        #[arg(long, default_value_t = false)]
        no_subtitles: bool,
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

pub(crate) fn print_mcp_servers(
    servers: &std::collections::BTreeMap<String, mint_core::mcp::McpServer>,
) {
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

/// Installs a global panic hook, as early as possible in `main()` so it
/// covers the whole process lifetime. Two things previously had no safety
/// net at all: (1) a panic while raw mode is enabled (key reads, inline
/// `ratatui` draws in the interactive REPL) left the user's shell stuck in
/// raw mode with no explanation, since raw mode is only ever disabled on the
/// normal, non-panicking exit path; (2) there was no record of a crash
/// anywhere — a user hitting a panic had no way to report *what* happened
/// beyond whatever scrolled off their terminal.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        log_panic_to_file(&info.to_string());
        default_hook(info);
    }));
}

/// Appends one panic entry to a local, machine-only log file — deliberately
/// not sent anywhere off the user's disk, matching Mint's local-first
/// positioning. Best-effort throughout: a failure to log a panic must never
/// itself panic or block the real panic handling that follows it.
fn log_panic_to_file(message: &str) {
    let Some(config_dir) = dirs::config_dir() else {
        return;
    };
    let log_path = config_dir.join("mint").join("error.log");
    let Some(parent) = log_path.parent() else {
        return;
    };
    if std::fs::create_dir_all(parent).is_err() {
        return;
    }
    let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S");
    let entry = format!("[{timestamp}] {message}\n");
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open(&log_path) {
        use std::io::Write as _;
        let _ = file.write_all(entry.as_bytes());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    install_panic_hook();
    match Cli::parse().command {
        None => {
            mint_core::channels::start_channels();
            mint_core::start_cron_scheduler();
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
                            "sandbox": {
                                "mode": config.sandbox_mode,
                                "command": config.sandbox_command,
                                "availability": sandbox_availability(&config),
                            },
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
                    "browser_mouse_move",
                    "browser_mouse_click",
                    "browser_key_press",
                    "browser_screenshot",
                ] {
                    if config_mut.disabled_tools.contains(&tool.to_string()) {
                        config_mut.disabled_tools.retain(|x| x != *tool);
                        changed = true;
                    }
                }
                if changed {
                    mint_core::save_config(&config_mut)?;
                    println!(
                        "✅ Enabled browser automation tools in config: browser_open, browser_click, browser_type, browser_read, browser_mouse_move, browser_mouse_click, browser_key_press, browser_screenshot"
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
                                } else if line_str.contains("[MOUSE_MOVE]")
                                    || line_str.contains("[MOUSE_MOVE_SUCCESS]")
                                {
                                    println!("🖱️  {line_str}");
                                } else if line_str.contains("[MOUSE_CLICK]")
                                    || line_str.contains("[MOUSE_CLICK_SUCCESS]")
                                {
                                    println!("🖱️ ● {line_str}");
                                } else if line_str.contains("[KEY_PRESS]")
                                    || line_str.contains("[KEY_PRESS_SUCCESS]")
                                {
                                    println!("⌨️  {line_str}");
                                } else if line_str.contains("[SCREENSHOT]")
                                    || line_str.contains("[SCREENSHOT_SUCCESS]")
                                {
                                    println!("📸 {line_str}");
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
            Command::Web { dev } => {
                launch_mint_target("web".into(), dev).await?;
            }
            Command::Api { port } => {
                let config = load_config()?;
                print_welcome_banner(&config);

                println!("\n{MINT}✔ Mint API Server is running!{RESET}\n");
                println!(
                    "    {BLUE}API Server URL:{RESET} {MINT}http://localhost:{}{RESET}\n",
                    port
                );

                println!("Messaging Bridges Status:");

                let bridges = [
                    ("enableTelegramBridge", "Telegram Bot Bridge"),
                    ("enableDiscordBridge", "Discord Bot Bridge"),
                    ("enableSlackBridge", "Slack Bot Bridge"),
                    ("enableLineBridge", "LINE Bot Bridge"),
                    ("enableWhatsappBridge", "WhatsApp Cloud Bridge"),
                ];

                for &(key, name) in &bridges {
                    let enabled = config
                        .extra
                        .get(key)
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if enabled {
                        println!("  {MINT}● {name:<23} [Active]{RESET}");
                    } else {
                        println!("  {DIM}○ {name:<23} [Inactive]{RESET}");
                    }
                }

                println!("\n{DIM}(API Requests & Error logs will be displayed below){RESET}\n");
                println!("{DIM}Press Ctrl+C to stop{RESET}\n");
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
                McpCommand::Reauth { server } => {
                    println!("Re-authenticating MCP server '{server}'...");
                    if mcp::reauth(&server)? {
                        println!("Re-authentication succeeded for '{server}'.");
                    } else {
                        println!("Re-authentication failed for '{server}' (see output above).");
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
                } => {
                    use indicatif::{ProgressBar, ProgressStyle};
                    use std::time::Duration;
                    let spinner = ProgressBar::new_spinner();
                    spinner.set_style(
                        ProgressStyle::default_spinner()
                            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                            .template("{spinner:.green} {msg}")
                            .unwrap(),
                    );
                    spinner.set_message(format!(
                        "Executing MCP tool '{}' on server '{}'...",
                        tool, server
                    ));
                    spinner.enable_steady_tick(Duration::from_millis(80));

                    let res = mcp::call(&server, &tool, serde_json::from_str(&arguments)?);
                    spinner.finish_and_clear();
                    println!("{}", serde_json::to_string_pretty(&res?)?);
                }
            },
            Command::Link { command } => match command {
                LinkCommand::Add {
                    name,
                    path,
                    description,
                } => {
                    add_linked_folder(&name, &path, description)?;
                    println!("Linked folder: {name}");
                }
                LinkCommand::List => {
                    let folders = list_linked_folders()?;
                    println!("{}", serde_json::to_string_pretty(&folders)?);
                }
                LinkCommand::Remove { name } => {
                    println!(
                        "{}",
                        if remove_linked_folder(&name)? {
                            "removed"
                        } else {
                            "not found"
                        }
                    )
                }
            },
            Command::Hooks { command } => match command {
                HooksCommand::Add {
                    event,
                    matcher,
                    command,
                    timeout,
                } => {
                    let event = mint_core::HookEvent::parse(&event)?;
                    hooks::add(event, &matcher, &command, timeout)?;
                    println!("Added {} hook for '{}'", event.as_str(), matcher);
                }
                HooksCommand::List => {
                    let entries = hooks::list()?;
                    if entries.is_empty() {
                        println!("\n{BLUE}No hooks configured.{RESET}");
                    } else {
                        println!("\n{BLUE}Hooks:{RESET}");
                        for (index, hook) in entries.iter().enumerate() {
                            println!(
                                "  [{}] {} matcher='{}' timeout={}s command={}",
                                index,
                                hook.event.as_str(),
                                hook.matcher,
                                hook.timeout_secs,
                                hook.command
                            );
                        }
                    }
                }
                HooksCommand::Remove { index } => {
                    println!(
                        "{}",
                        if hooks::remove(index)? {
                            "removed"
                        } else {
                            "not found"
                        }
                    )
                }
                HooksCommand::Clear => {
                    hooks::clear()?;
                    println!("cleared");
                }
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
            Command::Symbols { root, limit } => {
                use indicatif::{ProgressBar, ProgressStyle};
                use std::time::Duration;
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                        .template("{spinner:.green} {msg}")
                        .unwrap(),
                );
                spinner.set_message("Building symbol index...");
                spinner.enable_steady_tick(Duration::from_millis(80));
                let index = build_symbol_index(&root, limit, &load_config()?);
                spinner.finish_and_clear();
                println!("{}", serde_json::to_string_pretty(&index?)?);
            }
            Command::SemanticCode { command } => match command {
                SemanticCodeCommand::Index { root } => {
                    use indicatif::{ProgressBar, ProgressStyle};
                    use std::time::Duration;
                    let spinner = ProgressBar::new_spinner();
                    spinner.set_style(
                        ProgressStyle::default_spinner()
                            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                            .template("{spinner:.green} {msg}")
                            .unwrap(),
                    );
                    spinner.set_message("Indexing semantic code workspace...");
                    spinner.enable_steady_tick(Duration::from_millis(80));
                    let res = index_semantic_code(&root, &load_config()?).await;
                    spinner.finish_and_clear();
                    println!("{}", serde_json::to_string_pretty(&res?)?);
                }
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
                        None,
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
                            video_data_uri: None,
                            document_attachment: None,
                            workspace_path: None,
                            agent_id: None,
                            plan_mode: false,
                            messages: None,
                            tools: None,
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
            Command::Cron { command } => {
                let cron_jobs = CronStore::open_default()?;
                match command {
                    CronCommand::Add {
                        name,
                        schedule,
                        task,
                        workspace,
                        timezone,
                    } => {
                        let default_workspace = std::env::current_dir()?;
                        if name.is_none() && schedule.is_none() && task.is_none() {
                            let job = cron_wizard::run_add_wizard(&cron_jobs, &default_workspace)?;
                            println!("\nCreated cron job {} — next run: {}", job.id, job.next_run);
                        } else {
                            let name = name.ok_or_else(|| anyhow::anyhow!("--name is required"))?;
                            let schedule =
                                schedule.ok_or_else(|| anyhow::anyhow!("--schedule is required"))?;
                            let task = task.ok_or_else(|| anyhow::anyhow!("--task is required"))?;
                            let workspace = workspace.unwrap_or(default_workspace);
                            let schedule = match timezone {
                                Some(tz) => mint_core::localize_schedule(
                                    &schedule,
                                    &tz,
                                    chrono::Utc::now(),
                                )
                                .map_err(|e| anyhow::anyhow!(e))?,
                                None => schedule,
                            };
                            println!(
                                "{}",
                                serde_json::to_string_pretty(&cron_jobs.add(
                                    name, schedule, task, workspace
                                )?)?
                            );
                        }
                    }
                    CronCommand::List => {
                        println!("{}", serde_json::to_string_pretty(&cron_jobs.list()?)?)
                    }
                    CronCommand::Show { id } => {
                        println!("{}", serde_json::to_string_pretty(&cron_jobs.get(&id)?)?)
                    }
                    CronCommand::Remove { id } => {
                        println!("{}", serde_json::to_string_pretty(&cron_jobs.remove(&id)?)?)
                    }
                    CronCommand::Enable { id } => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&cron_jobs.set_enabled(&id, true)?)?
                        )
                    }
                    CronCommand::Disable { id } => {
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&cron_jobs.set_enabled(&id, false)?)?
                        )
                    }
                    CronCommand::RunNow { id } => {
                        let job = cron_jobs
                            .get(&id)?
                            .ok_or_else(|| anyhow::anyhow!("no cron job with id {id}"))?;
                        println!("Running cron job {}: {}", job.id, job.name);
                        match agent::run_code_agent(&job.task, &job.workspace, &load_config()?)
                            .await
                        {
                            Ok(result) => {
                                cron_jobs.record_run(
                                    &job.id,
                                    "success",
                                    Some(result.summary.clone()),
                                )?;
                                println!("Job completed: {}", result.summary);
                            }
                            Err(error) => {
                                cron_jobs.record_run(&job.id, "failed", Some(error.to_string()))?;
                                return Err(error);
                            }
                        }
                    }
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
            Command::Plugins { command } => {
                plugins_cli::run_plugins_command(command).await?;
            }
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
                    CodeCommand::Agent { task, root, plan } => {
                        agent::run_code_agent_with_options(
                            &task,
                            &root,
                            &config,
                            None,
                            None,
                            agent::AgentOptions {
                                fast_mode: false,
                                plan_mode: plan,
                            },
                        )
                        .await?;
                    }
                    CodeCommand::Summary { root } => println!(
                        "{}",
                        serde_json::to_string_pretty(&repository_summary(&root, &config)?)?
                    ),
                    CodeCommand::List { root, limit } => println!(
                        "{}",
                        serde_json::to_string_pretty(&list_code_files(&root, limit, &config)?)?
                    ),
                    CodeCommand::Subagents { root } => {
                        let subagents = list_subagents(Some(&root));
                        if subagents.is_empty() {
                            println!(
                                "No subagent definitions found under {}/.agents/subagents/ or ~/.config/mint/mint-agents/.",
                                root.display()
                            );
                        } else {
                            println!("{}", serde_json::to_string_pretty(&subagents)?);
                        }
                    }
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
                                &[CodePatchHunk {
                                    old_text,
                                    new_text,
                                    replace_all: false
                                }],
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
                                &[CodePatchHunk {
                                    old_text,
                                    new_text,
                                    replace_all: false
                                }],
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
                    launch_mint_target(target, false).await?;
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
                use indicatif::{ProgressBar, ProgressStyle};
                use std::time::Duration;
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                        .template("{spinner:.green} {msg}")
                        .unwrap(),
                );
                spinner.set_message(format!("Generating {} image(s)...", count));
                spinner.enable_steady_tick(Duration::from_millis(80));

                let request = ImageGenRequest {
                    prompt: prompt.clone(),
                    negative_prompt: negative,
                    aspect_ratio: Some(aspect),
                    num_images: Some(count),
                    model: None,
                    provider: None,
                    image_data_uri: None,
                    mask_data_uri: None,
                    mode: None,
                };
                match generate_images(&config, &request).await {
                    Ok(result) => {
                        spinner.finish_and_clear();
                        eprintln!("{MINT}✦ Generated {} image(s){RESET}", result.images.len());
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
                        spinner.finish_and_clear();
                        eprintln!("{ERROR}✗ Image generation failed: {e}{RESET}");
                        anyhow::bail!("image generation failed: {e}");
                    }
                }
            }
            Command::Veo {
                prompt,
                aspect,
                duration,
                negative,
                output,
            } => {
                let config = load_config()?;
                use indicatif::{ProgressBar, ProgressStyle};
                use std::time::Duration as StdDuration;
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                        .template("{spinner:.magenta} {msg}")
                        .unwrap(),
                );
                spinner.set_message(format!(
                    "Generating {}s (aspect: {}) video with Veo for: \"{}\"...",
                    duration, aspect, prompt
                ));
                spinner.enable_steady_tick(StdDuration::from_millis(80));

                let gen_request = mint_core::VideoGenRequest {
                    prompt: prompt.clone(),
                    negative_prompt: negative,
                    aspect_ratio: aspect.clone(),
                    duration,
                    model: None,
                    provider: "veo".to_string(),
                };

                match mint_core::generate_video(&config, &gen_request).await {
                    Ok(result) => {
                        spinner.finish_and_clear();
                        if let Some(video) = result.videos.first() {
                            println!("{MINT}✓ Video generated successfully!{RESET}");
                            println!("{MINT}Saved to: {}{RESET}", video.path.display());
                            if let Some(out_path) = output {
                                if let Err(e) = std::fs::copy(&video.path, &out_path) {
                                    eprintln!(
                                        "{ERROR}✗ Failed to copy generated video to output path: {e}{RESET}"
                                    );
                                } else {
                                    println!(
                                        "{MINT}✓ Copied to output destination: {}{RESET}",
                                        out_path.display()
                                    );
                                }
                            }
                        } else {
                            eprintln!("{ERROR}✗ Video generation returned no videos.{RESET}");
                        }
                    }
                    Err(e) => {
                        spinner.finish_and_clear();
                        eprintln!("{ERROR}✗ Video generation failed: {e}{RESET}");
                        anyhow::bail!("video generation failed: {e}");
                    }
                }
            }
            Command::Video { command } => {
                use mint_core::{
                    ExportRequest, ExtractAudioRequest, MergeRequest, RemoveSilenceRequest,
                    ResizeRequest, TrimRequest, video_export, video_extract_audio, video_load,
                    video_merge, video_remove_silence, video_resize, video_trim,
                };
                match command {
                    VideoCommand::Load { path } => {
                        let path_str = path.to_string_lossy().to_string();
                        match video_load(&path_str) {
                            Ok(info) => {
                                println!("{MINT}✓ Video loaded:{RESET}");
                                println!("  Path:        {}", info.path);
                                println!("  Duration:    {:.2}s", info.duration);
                                println!("  Resolution:  {}x{}", info.width, info.height);
                                println!("  FPS:         {:.2}", info.fps);
                                println!("  Audio:       {} stream(s)", info.audio_streams);
                                println!("  Format:      {}", info.format);
                                println!("  Size:        {} bytes", info.size_bytes);
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Failed to load video: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::Trim {
                        input,
                        start,
                        end,
                        output,
                    } => {
                        println!("{BLUE}→ Trimming {:.2}s–{:.2}s...{RESET}", start, end);
                        let req = TrimRequest {
                            input: input.to_string_lossy().to_string(),
                            output: output.to_string_lossy().to_string(),
                            start,
                            end,
                        };
                        match video_trim(&req) {
                            Ok(r) => println!("{MINT}✓ Trimmed → {}{RESET}", r.output_path),
                            Err(e) => {
                                eprintln!("{ERROR}✗ Trim failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::Resize {
                        input,
                        width,
                        height,
                        output,
                    } => {
                        println!("{BLUE}→ Resizing to {}x{}...{RESET}", width, height);
                        let req = ResizeRequest {
                            input: input.to_string_lossy().to_string(),
                            output: output.to_string_lossy().to_string(),
                            width,
                            height,
                        };
                        match video_resize(&req) {
                            Ok(r) => println!("{MINT}✓ Resized → {}{RESET}", r.output_path),
                            Err(e) => {
                                eprintln!("{ERROR}✗ Resize failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::Merge { inputs, output } => {
                        println!("{BLUE}→ Merging {} clips...{RESET}", inputs.len());
                        let req = MergeRequest {
                            inputs: inputs
                                .iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect(),
                            output: output.to_string_lossy().to_string(),
                        };
                        match video_merge(&req) {
                            Ok(r) => println!("{MINT}✓ Merged → {}{RESET}", r.output_path),
                            Err(e) => {
                                eprintln!("{ERROR}✗ Merge failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::ExtractAudio { input, output } => {
                        println!("{BLUE}→ Extracting audio...{RESET}");
                        let req = ExtractAudioRequest {
                            input: input.to_string_lossy().to_string(),
                            output: output.to_string_lossy().to_string(),
                        };
                        match video_extract_audio(&req) {
                            Ok(r) => println!("{MINT}✓ Audio extracted → {}{RESET}", r.output_path),
                            Err(e) => {
                                eprintln!("{ERROR}✗ Extract failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::RemoveSilence {
                        input,
                        output,
                        threshold,
                        min_duration,
                    } => {
                        println!(
                            "{BLUE}→ Removing silence (threshold: {}dB, min: {}s)...{RESET}",
                            threshold, min_duration
                        );
                        let req = RemoveSilenceRequest {
                            input: input.to_string_lossy().to_string(),
                            output: output.to_string_lossy().to_string(),
                            threshold_db: threshold,
                            min_silence_secs: min_duration,
                        };
                        match video_remove_silence(&req) {
                            Ok(r) => {
                                let dur =
                                    r.duration.map(|d| format!("{d:.2}s")).unwrap_or_default();
                                println!(
                                    "{MINT}✓ Silence removed → {} ({dur}){RESET}",
                                    r.output_path
                                );
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Remove silence failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::Export {
                        input,
                        output,
                        resolution,
                        fps,
                        codec,
                        crf,
                    } => {
                        println!("{BLUE}→ Exporting video...{RESET}");
                        let req = ExportRequest {
                            input: input.to_string_lossy().to_string(),
                            output: output.to_string_lossy().to_string(),
                            resolution,
                            fps,
                            codec,
                            crf,
                        };
                        match video_export(&req) {
                            Ok(r) => {
                                let dur =
                                    r.duration.map(|d| format!("{d:.2}s")).unwrap_or_default();
                                let size = r
                                    .size_bytes
                                    .map(|s| format!("{} bytes", s))
                                    .unwrap_or_default();
                                println!(
                                    "{MINT}✓ Exported → {} ({dur}, {size}){RESET}",
                                    r.output_path
                                );
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Export failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::Transcribe {
                        input,
                        language,
                        output,
                    } => {
                        println!("{BLUE}→ Transcribing audio/speech...{RESET}");
                        let req = mint_core::TranscribeRequest {
                            input: input.to_string_lossy().to_string(),
                            language,
                            prompt: None,
                        };
                        let config = load_config()?;
                        match mint_core::transcribe(&config, &req).await {
                            Ok(res) => {
                                let srt = mint_core::generate_srt(&res.segments);
                                std::fs::write(&output, srt)?;
                                println!(
                                    "{MINT}✓ Transcribed {} segments → {}{RESET}",
                                    res.segments.len(),
                                    output.display()
                                );
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Transcription failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::Subtitle {
                        input,
                        srt,
                        style,
                        output,
                    } => {
                        println!("{BLUE}→ Burning subtitles ({style} preset)...{RESET}");
                        let srt_content = if std::path::Path::new(&srt).exists() {
                            std::fs::read_to_string(&srt)?
                        } else {
                            srt.clone()
                        };
                        let req = mint_core::BurnSubtitleRequest {
                            input_video: input.to_string_lossy().to_string(),
                            srt_input: srt_content,
                            output_video: output.to_string_lossy().to_string(),
                            style: None,
                            preset: Some(style),
                        };
                        match mint_core::burn_subtitles(&req) {
                            Ok(r) => {
                                println!("{MINT}✓ Subtitles burned → {}{RESET}", r.output_path)
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Subtitle burn failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::TranslateSubtitle { srt, lang, output } => {
                        println!("{BLUE}→ Translating SRT to {lang}...{RESET}");
                        let srt_content = std::fs::read_to_string(&srt)?;
                        let req = mint_core::TranslateSubtitleRequest {
                            srt_content,
                            target_language: lang,
                        };
                        let config = load_config()?;
                        match mint_core::translate_subtitles(&config, &req).await {
                            Ok(translated_srt) => {
                                std::fs::write(&output, translated_srt)?;
                                println!(
                                    "{MINT}✓ Subtitles translated → {}{RESET}",
                                    output.display()
                                );
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Subtitle translation failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                    VideoCommand::MakeShorts {
                        input,
                        output_dir,
                        clips,
                        duration,
                        no_subtitles,
                    } => {
                        println!(
                            "{BLUE}🚀 Generating up to {clips} vertical Shorts clips from video...{RESET}"
                        );
                        let req = mint_core::MakeShortsRequest {
                            input: input.to_string_lossy().to_string(),
                            output_dir: output_dir.map(|p| p.to_string_lossy().to_string()),
                            max_clips: clips,
                            target_duration: duration,
                            burn_subtitles: !no_subtitles,
                            width: 1080,
                            height: 1920,
                        };
                        let config = load_config()?;
                        match mint_core::make_shorts(&config, &req).await {
                            Ok(res) => {
                                println!(
                                    "{MINT}✓ Generated {} vertical Shorts!{RESET}",
                                    res.clips.len()
                                );
                                for clip in &res.clips {
                                    println!(
                                        "  [{}] {} ({:.1}s) → {}",
                                        clip.id, clip.title, clip.duration, clip.path
                                    );
                                }
                            }
                            Err(e) => {
                                eprintln!("{ERROR}✗ Make Shorts failed: {e}{RESET}");
                                anyhow::bail!("{e}");
                            }
                        }
                    }
                }
            }
        },
    }
    Ok(())
}

async fn launch_mint_target(target: String, dev: bool) -> Result<()> {
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
            let dist_web = project_root
                .join("out")
                .join("renderer")
                .join("index-web.html");
            let web_cmd = if dev {
                "dev:web"
            } else if dist_web.exists() {
                "preview:web"
            } else {
                "dev:web"
            };
            std::process::Command::new("npm")
                .current_dir(&project_root)
                .args(["run", web_cmd])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
                .map_err(|e| anyhow::anyhow!("Failed to launch web app: {e}"))?;

            println!("\n{MINT}✔ Mint Web is running!{RESET}\n");
            println!("    {BLUE}Web UI:{RESET}     {MINT}http://localhost:9000{RESET}");
            if let Some(ip) = mint_core::api_server::get_local_ip() {
                println!(
                    "    {BLUE}Mobile:{RESET}     {MINT}http://{}:9000{RESET}",
                    ip
                );
            }
            println!("    {BLUE}API Server:{RESET} {MINT}http://localhost:3000{RESET}\n");

            println!("Point your browser to:");
            println!("{MINT}http://localhost:9000{RESET}\n");
            println!("{DIM}(API Requests & Error logs will be displayed below){RESET}\n");

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

    use indicatif::{ProgressBar, ProgressStyle};
    use std::time::Duration;

    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::default_spinner()
            .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
            .template("{spinner:.green} {msg}")
            .unwrap(),
    );
    spinner.set_message(format!(
        "Fetching information for {}/{} from GitHub...",
        owner, repo_name
    ));
    spinner.enable_steady_tick(Duration::from_millis(80));

    let summary = match fetch_github_repo_summary(&owner, &repo_name).await {
        Ok(s) => s,
        Err(e) => {
            spinner.finish_and_clear();
            anyhow::bail!(
                "Failed to fetch repository summary: {}. Check that the repository is public and spelled correctly.",
                e
            );
        }
    };

    spinner.set_message("Analyzing repository with AI model...");
    let prompt = format!(
        "Here is the metadata, top-level directory structure, and README.md content for the GitHub repository {}/{}:\n\n{}\n\nBased on this information, please provide a high-level overview of what this repository is about, what tech stack it uses, its overall architecture, and how it is organized.",
        owner, repo_name, summary
    );

    let (response, _) = match orchestrate_chat_with_fallback(
        config,
        &ChatRequest {
            message: prompt,
            system_instruction: "You are a professional software architect providing a high-level overview of a code repository based on its metadata and README.".to_string(),
            chat_id: Some("github_review".to_string()),
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        },
    )
    .await {
        Ok(res) => {
            spinner.finish_and_clear();
            res
        }
        Err(e) => {
            spinner.finish_and_clear();
            return Err(e.into());
        }
    };

    println!(
        "\n--- AI Repository Overview for {}/{} ---",
        owner, repo_name
    );
    println!("{}", response.text);
    println!("--------------------------------------------------");
    Ok(())
}

use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use mint_core::MintConfig;

pub mod agent;
pub mod config;
pub mod eval;
pub mod integrations;
pub mod knowledge;
pub mod system;
pub mod tasks;

pub use agent::*;
pub use config::*;
pub use eval::*;
pub use integrations::*;
pub use knowledge::*;
pub use system::*;
pub use tasks::*;

#[derive(Debug, Subcommand)]
pub enum Command {
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
    /// Manage built-in plugins & integrations (Gmail, Calendar, Notion, Spotify, GitHub, Discord).
    #[command(alias = "plugin")]
    Plugins {
        #[command(subcommand)]
        command: Option<crate::plugins_cli::PluginsSubcommand>,
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
    /// Run an engineering benchmark evaluation suite against the agent harness.
    Eval {
        /// Path to the benchmark suite JSON file.
        #[arg(long, default_value = "benchmarks/mint_eval.json")]
        suite: PathBuf,
        /// Maximum number of tasks to execute.
        #[arg(long, default_value_t = 1)]
        limit: usize,
    },
    /// Rewind workspace to a previous git checkpoint snapshot.
    Rewind {
        /// Step number to restore (omitting lists available checkpoints).
        step: Option<usize>,
        /// Chat ID (defaults to "cli").
        #[arg(long, default_value = "cli")]
        chat_id: String,
    },
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
    /// Run Mint headless: chat bridges (Telegram, Discord, Slack, LINE,
    /// WhatsApp) and the cron scheduler, with no interactive TUI.
    Gateway {
        #[command(subcommand)]
        command: GatewayCommand,
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
    /// Live preview an artifact file (HTML, SVG, Markdown) in browser or terminal.
    Preview { path: PathBuf },
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
    /// Connect agent activity to Project Avatar.
    Avatar {
        #[command(subcommand)]
        command: AvatarCommand,
    },
    /// Install, or list, local AI skills.
    Skills {
        #[command(subcommand)]
        command: SkillsCommand,
    },
}

pub async fn dispatch(cmd: Command, config: &mut MintConfig, cli: &crate::Cli) -> Result<()> {
    match cmd {
        Command::Status => config::handle_status(config),
        Command::Config { command } => config::handle_config(command),
        Command::Providers => config::handle_providers(),
        Command::Update {
            check,
            dry_run,
            approve,
        } => config::handle_update(check, dry_run, approve),
        Command::Onboard => config::handle_onboard().await,
        Command::Setup => config::handle_setup().await,

        Command::Run {
            approve,
            cwd,
            command,
        } => system::handle_run(approve, cwd, command),
        Command::Open { target } => system::handle_open(target),
        Command::OpenApp { name } => system::handle_open_app(name),
        Command::Preview { path } => system::handle_preview(path),
        Command::ReadFile { path } => system::handle_read_file(path),
        Command::ReadFolder { path } => system::handle_read_folder(path),
        Command::Safety { command } => system::handle_safety(command),
        Command::Files { command } => system::handle_files(command),

        Command::Agent { task } => agent::handle_agent(task).await,
        Command::Eval { suite, limit } => eval::handle_eval(suite, limit, config).await,
        Command::Rewind { step, chat_id } => agent::handle_rewind(step, chat_id),
        Command::Auto => agent::handle_auto().await,
        Command::Web { dev } => agent::handle_web(dev).await,
        Command::Api { port } => agent::handle_api(port).await,
        Command::Gateway { command } => agent::handle_gateway(command).await,
        Command::Chat {
            message,
            system,
            image,
        } => agent::handle_chat(message, system, image, cli, config).await,
        Command::Imagine {
            prompt,
            aspect,
            count,
            negative,
            output,
        } => agent::handle_imagine(prompt, aspect, count, negative, output).await,
        Command::Veo {
            prompt,
            aspect,
            duration,
            negative,
            output,
        } => agent::handle_veo(prompt, aspect, duration, negative, output).await,
        Command::Video { command } => agent::handle_video(command).await,
        Command::Avatar { command } => agent::handle_avatar(command).await,

        Command::Mcp { command } => integrations::handle_mcp(command).await,
        Command::Link { command } => integrations::handle_link(command),
        Command::Hooks { command } => integrations::handle_hooks(command),
        Command::Gmail { command } => integrations::handle_gmail(command).await,
        Command::Plugins { command } => integrations::handle_plugins(command).await,

        Command::Learn { path, list, delete } => knowledge::handle_learn(path, list, delete),
        Command::Symbols { root, limit } => knowledge::handle_symbols(root, limit),
        Command::SemanticCode { command } => knowledge::handle_semantic_code(command).await,
        Command::Knowledge { command } => knowledge::handle_knowledge(command),
        Command::Code { command } => knowledge::handle_code(command, cli, config).await,
        Command::Skills { command } => knowledge::handle_skills(command),

        Command::Memory { command } => tasks::handle_memory(command),
        Command::Task { command } => tasks::handle_task(command),
        Command::Cron { command } => tasks::handle_cron(command, config).await,
    }
}

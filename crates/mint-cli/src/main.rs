use anyhow::Result;
use clap::Parser;
use std::path::{Path, PathBuf};

use mint_core::{MintConfig, load_config};

mod actions;
mod agent;
mod background;
pub mod commands;
mod cron_wizard;
mod gateway;
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
mod subagent_wizard;
mod updater;

pub use commands::Command;
pub use interactive::{
    SESSION_APPROVED, active_model, confirm, confirm_security, print_welcome_banner,
    run_interactive_chat, run_interactive_chat_with_options,
};

pub const RESET: &str = "\x1b[0m";
pub const MINT: &str = "\x1b[32m";
pub const BLUE: &str = "\x1b[38;2;78;201;216m";
pub const DIM: &str = "\x1b[90m";
pub const ERROR: &str = "\x1b[31m";
pub const WARN: &str = "\x1b[33m";

pub(crate) async fn run_code_agent_with_saved_image(
    task: &str,
    current_dir: &Path,
    config: &MintConfig,
    image_data_uri: Option<String>,
    video_data_uri: Option<String>,
    options: agent::AgentOptions,
) -> Result<(Vec<String>, Option<String>)> {
    let sent_image = image_data_uri.clone();
    let sent_video = video_data_uri.clone();
    // Follow-up messages the user typed into the queueing box while this
    // turn was still running (see `agent::run_code_agent_with_options`)
    // land here; the caller is responsible for dispatching them. On error
    // the queue is dropped along with the interrupted turn.
    let queue = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    // The box's leftover, not-yet-submitted text (typed but no Enter yet) at
    // the moment the turn ended — same idea as `queue` above, but for the one
    // partial entry that was never confirmed, so the caller can hand it back
    // as the next prompt's starting text instead of dropping it silently.
    let draft = std::sync::Arc::new(std::sync::Mutex::new(None));
    let result = agent::run_code_agent_with_options(
        task,
        current_dir,
        config,
        image_data_uri,
        video_data_uri,
        options,
        std::sync::Arc::clone(&queue),
        std::sync::Arc::clone(&draft),
    )
    .await;
    result?;
    // This turn just committed a row to the workspace-scoped "cli" chat_id
    // (see `scoped_chat_id`) — raise live_sync's watermark past it so the
    // next poll tick doesn't mistake the user's own just-sent message for
    // one that arrived from another surface (web/desktop).
    if let Ok(memory) = mint_core::MemoryStore::open_default()
        && let Ok(rows) = memory.recent_interactions_for_chat(
            &mint_core::scoped_chat_id(
                mint_core::CHAT_CLI_ID,
                Some(&current_dir.to_string_lossy()),
            ),
            1,
        )
        && let Some(row) = rows.first()
    {
        mint_core::live_sync::note_own_interaction(row.id);
    }
    // Save any attached images and videos that were sent with the task
    image::save_sent_image_after_send(sent_image.as_deref(), task);
    image::save_sent_image_after_send(sent_video.as_deref(), task);
    let queued = queue
        .lock()
        .map(|mut q| std::mem::take(&mut *q))
        .unwrap_or_default();
    let draft = draft.lock().map(|mut d| d.take()).unwrap_or_default();
    Ok((queued, draft))
}

pub(crate) fn apply_temporary_model_override(config: &mut MintConfig, model_str: &str) {
    let trimmed = model_str.trim();
    if trimmed.is_empty() {
        return;
    }
    if let Some((provider, model)) = trimmed.split_once(':') {
        let p = provider.trim();
        let m = model.trim();
        config.ai_provider = p.to_string();
        apply_model_to_provider(config, p, m);
    } else if trimmed.starts_with("claude") {
        config.ai_provider = "anthropic".to_string();
        config.anthropic_model = trimmed.to_string();
    } else if trimmed.starts_with("gpt-") || trimmed.starts_with("o1") || trimmed.starts_with("o3") {
        config.ai_provider = "openai".to_string();
        config.openai_model = trimmed.to_string();
    } else if trimmed.starts_with("gemini") {
        config.ai_provider = "gemini".to_string();
        config.gemini_model = trimmed.to_string();
    } else if trimmed.starts_with("deepseek") {
        config.ai_provider = "deepseek".to_string();
        config.deepseek_model = trimmed.to_string();
    } else {
        let p = config.ai_provider.clone();
        apply_model_to_provider(config, &p, trimmed);
    }
}

fn apply_model_to_provider(config: &mut MintConfig, provider: &str, model: &str) {
    match provider {
        "anthropic" => config.anthropic_model = model.to_string(),
        "openai" => config.openai_model = model.to_string(),
        "openrouter" => config.openrouter_model = model.to_string(),
        "deepseek" => config.deepseek_model = model.to_string(),
        "huggingface" => config.hf_model = model.to_string(),
        "local_openai" => config.local_model_name = model.to_string(),
        "ollama" => config.ollama_model = model.to_string(),
        "gemini" => config.gemini_model = model.to_string(),
        _ => {}
    }
}

pub(crate) async fn run_oneshot_agent_task(
    task: &str,
    current_dir: &Path,
    config: &MintConfig,
    fast_mode: bool,
    plan_mode: bool,
    image_path: Option<&Path>,
) -> Result<()> {
    let image_data_uri = image_path
        .map(image::load_image_as_data_uri)
        .transpose()?;

    let pinned_mcp_server = mint_core::list_mcp_servers().ok().and_then(|servers| {
        task.split_whitespace()
            .find_map(|word| word.strip_prefix('@'))
            .filter(|name| servers.contains_key(*name))
            .map(str::to_owned)
    });

    let options = agent::AgentOptions {
        fast_mode,
        plan_mode,
        queueing: false,
        pinned_mcp_server,
    };

    let queue = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let draft = std::sync::Arc::new(std::sync::Mutex::new(None));

    let res = agent::run_code_agent_with_options(
        task,
        current_dir,
        config,
        image_data_uri.clone(),
        None,
        options,
        queue,
        draft,
    )
    .await;

    if let Err(err) = res {
        let err_msg = err.to_string();
        if err_msg.to_lowercase().contains("interrupted") {
            eprintln!("\n{DIM}[Cancelled by user]{RESET}");
            std::process::exit(130);
        }
        return Err(err);
    }

    if let Ok(memory) = mint_core::MemoryStore::open_default()
        && let Ok(rows) = memory.recent_interactions_for_chat(
            &mint_core::scoped_chat_id(
                mint_core::CHAT_CLI_ID,
                Some(&current_dir.to_string_lossy()),
            ),
            1,
        )
        && let Some(row) = rows.first()
    {
        mint_core::live_sync::note_own_interaction(row.id);
    }

    image::save_sent_image_after_send(image_data_uri.as_deref(), task);
    Ok(())
}

pub(crate) fn print_mcp_servers(
    servers: &std::collections::BTreeMap<String, mint_core::mcp::McpServer>,
) {
    if servers.is_empty() {
        println!("{DIM}(No MCP servers configured.){RESET}\n");
        return;
    }
    for (name, srv) in servers {
        let desc = if let Some(url) = srv.remote_url() {
            format!("(url: {url})")
        } else {
            let args_str = srv.args.join(" ");
            format!("({} {args_str})", srv.command)
        };
        let (dot, suffix) = if srv.disabled {
            (DIM, format!(" {DIM}[disabled]{RESET}"))
        } else {
            (BLUE, String::new())
        };
        println!(
            "  {dot}●{RESET} {name} {DIM}{desc}{RESET}{suffix}",
        );
    }
    println!();
}

#[derive(Debug, Parser)]
#[command(name = "mint", version, about = "Mint native CLI — AI agent for your terminal and workspace")]
pub struct Cli {
    /// Workspace directory to run in (mirrors `git -C`)
    #[arg(short = 'C', long, global = true)]
    pub cwd: Option<PathBuf>,

    /// Override active AI model for this execution/session
    #[arg(short = 'm', long, global = true)]
    pub model: Option<String>,

    /// Fast mode (hide thinking traces)
    #[arg(long, global = true)]
    pub fast: bool,

    /// Plan mode (investigate read-only and require plan approval)
    #[arg(long, global = true)]
    pub plan: bool,

    /// Attach an image file to the prompt
    #[arg(long, global = true)]
    pub image: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,

    /// Optional task or prompt to run directly as a one-shot turn.
    /// If omitted and no subcommand is passed, opens interactive chat.
    #[arg(trailing_var_arg = true)]
    pub prompt: Vec<String>,
}

fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = crossterm::terminal::disable_raw_mode();
        log_panic_to_file(&info.to_string());
        default_hook(info);
    }));
}

/// Appends one panic entry to a local, machine-only log file.
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
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)
    {
        use std::io::Write as _;
        let _ = file.write_all(entry.as_bytes());
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    install_panic_hook();
    let mut cli = Cli::parse();

    if let Some(ref dir) = cli.cwd {
        std::env::set_current_dir(dir)?;
    }

    let mut config = load_config()?;
    if let Some(ref m) = cli.model {
        apply_temporary_model_override(&mut config, m);
    }

    match cli.command.take() {
        None => {
            if !cli.prompt.is_empty() {
                let task = cli.prompt.join(" ");
                let current_dir = std::env::current_dir()?;
                run_oneshot_agent_task(
                    &task,
                    &current_dir,
                    &config,
                    cli.fast,
                    cli.plan,
                    cli.image.as_deref(),
                )
                .await?;
            } else {
                mint_core::channels::start_channels();
                mint_core::start_cron_scheduler();
                run_interactive_chat_with_options(cli.model, cli.fast, cli.plan).await?;
            }
        }
        Some(cmd) => {
            commands::dispatch(cmd, &mut config, &cli).await?;
        }
    }
    // Every subcommand that could have spawned a persistent stdio MCP child
    // (interactive chat, `agent`/`code`, `chat`, `web`/`api`, `gateway` after
    // its shutdown signal) funnels back here on a clean exit. `SESSIONS` is a
    // `static`, so `McpSession::Drop` won't run at process teardown — do it
    // explicitly so we don't leave orphaned MCP server processes behind.
    mint_core::close_all_mcp_sessions();
    Ok(())
}

#[cfg(test)]
mod cli_tests {
    use super::*;

    #[test]
    fn parse_subcommand_status() {
        let cli = Cli::try_parse_from(["mint", "status"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Status)));
        assert!(cli.prompt.is_empty());
    }

    #[test]
    fn parse_oneshot_quoted_prompt() {
        let cli = Cli::try_parse_from(["mint", "explain this function"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.prompt, vec!["explain this function"]);
    }

    #[test]
    fn parse_oneshot_unquoted_prompt_words() {
        let cli = Cli::try_parse_from(["mint", "fix", "all", "tests"]).unwrap();
        assert!(cli.command.is_none());
        assert_eq!(cli.prompt, vec!["fix", "all", "tests"]);
    }

    #[test]
    fn parse_flags_with_prompt() {
        let cli = Cli::try_parse_from([
            "mint",
            "-C",
            "/tmp",
            "-m",
            "gemini-2.5-flash",
            "--fast",
            "--plan",
            "write tests",
        ])
        .unwrap();
        assert_eq!(cli.cwd, Some(PathBuf::from("/tmp")));
        assert_eq!(cli.model.as_deref(), Some("gemini-2.5-flash"));
        assert!(cli.fast);
        assert!(cli.plan);
        assert_eq!(cli.prompt, vec!["write tests"]);
    }

    #[test]
    fn model_override_auto_detects_provider() {
        let mut config = MintConfig::default();
        config.ai_provider = "openai".to_string();

        apply_temporary_model_override(&mut config, "claude-3-7-sonnet");
        assert_eq!(config.ai_provider, "anthropic");
        assert_eq!(config.anthropic_model, "claude-3-7-sonnet");

        apply_temporary_model_override(&mut config, "gemini-2.5-flash");
        assert_eq!(config.ai_provider, "gemini");
        assert_eq!(config.gemini_model, "gemini-2.5-flash");

        apply_temporary_model_override(&mut config, "openrouter:meta-llama/llama-3");
        assert_eq!(config.ai_provider, "openrouter");
        assert_eq!(config.openrouter_model, "meta-llama/llama-3");
    }

    #[test]
    fn parse_subcommand_config_show() {
        let cli = Cli::try_parse_from(["mint", "config", "show"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Config {
                command: commands::ConfigCommand::Show
            })
        ));
    }

    #[test]
    fn parse_subcommand_mcp_list() {
        let cli = Cli::try_parse_from(["mint", "mcp", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Command::Mcp {
                command: commands::McpCommand::List
            })
        ));
    }

    #[test]
    fn parse_subcommand_memory_get() {
        let cli = Cli::try_parse_from(["mint", "memory", "get", "user_name"]).unwrap();
        if let Some(Command::Memory {
            command: commands::MemoryCommand::Get { key },
        }) = cli.command
        {
            assert_eq!(key, "user_name");
        } else {
            panic!("Expected Command::Memory::Get");
        }
    }

    #[test]
    fn parse_subcommand_run() {
        let cli = Cli::try_parse_from(["mint", "run", "--approve", "cargo", "test"]).unwrap();
        if let Some(Command::Run {
            approve,
            command,
            ..
        }) = cli.command
        {
            assert!(approve);
            assert_eq!(command, vec!["cargo", "test"]);
        } else {
            panic!("Expected Command::Run");
        }
    }

    #[test]
    fn parse_subcommand_code_list() {
        let cli = Cli::try_parse_from(["mint", "code", "list", "--limit", "50", "src"]).unwrap();
        if let Some(Command::Code {
            command: commands::CodeCommand::List { root, limit },
        }) = cli.command
        {
            assert_eq!(root, PathBuf::from("src"));
            assert_eq!(limit, 50);
        } else {
            panic!("Expected Command::Code::List");
        }
    }
}

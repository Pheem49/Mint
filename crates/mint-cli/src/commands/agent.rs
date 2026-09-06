use std::path::PathBuf;

use anyhow::Result;
use clap::Subcommand;
use mint_core::{
    CHAT_CLI_ID, ChatRequest, ImageGenRequest, MintConfig, TaskStore, generate_images, load_config,
    orchestrate_chat_with_fallback, save_config,
};

use crate::{
    BLUE, DIM, ERROR, MINT, RESET, WARN, actions, agent, gateway, image, print_welcome_banner,
    run_interactive_chat, run_oneshot_agent_task,
};

#[derive(Debug, Subcommand)]
pub enum GatewayCommand {
    /// Start the gateway: chat bridges + cron scheduler, blocking until
    /// terminated (Ctrl+C or, under systemd, SIGTERM).
    Start {
        /// Also serve the local REST API + WebUI on this port (reachable
        /// remotely over an SSH tunnel or Tailscale). Omit to run bridges
        /// and cron only.
        #[arg(long)]
        api_port: Option<u16>,
    },
    /// Register `mint gateway start` as a systemd unit so it survives
    /// reboots and keeps running without a login session (Linux only).
    Install {
        /// Passed through to the installed unit's `mint gateway start` command.
        #[arg(long)]
        api_port: Option<u16>,
        /// Write a system-wide unit under /etc/systemd/system (needs root)
        /// instead of a per-user one under ~/.config/systemd/user.
        #[arg(long)]
        system: bool,
        /// Enable and start the service immediately after installing it.
        #[arg(long)]
        now: bool,
        /// Hard memory cap for the service (systemd size syntax, e.g. "512M",
        /// "1G", "75%"). Omit to leave it unbounded — Mint's video/image
        /// tooling can legitimately need real memory for a single task, and a
        /// guessed default risks an OOM-kill mid-task for no real benefit.
        #[arg(long)]
        memory_max: Option<String>,
    },
}

#[derive(Debug, Subcommand)]
pub enum VideoCommand {
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
pub enum AvatarCommand {
    /// Print your avatar share link, generating a channel token on first use.
    Link,
    /// Show relay channel status: selected model, viewer count, last event.
    Status,
    /// Point at a self-hosted relay/app instead of the public default.
    SetRelay {
        relay_url: String,
        #[arg(long)]
        app_url: Option<String>,
    },
    /// Clear the saved token, disabling the bridge.
    Disable,
}

pub(crate) async fn launch_mint_target(target: String, dev: bool) -> Result<()> {
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

pub(crate) async fn run_cli_agent_task(task: Option<String>) -> Result<()> {
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

async fn wait_for_shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{SignalKind, signal};
        let Ok(mut sigterm) = signal(SignalKind::terminate()) else {
            let _ = tokio::signal::ctrl_c().await;
            return;
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

pub async fn handle_agent(task: Option<String>) -> Result<()> {
    run_cli_agent_task(task).await
}

pub fn handle_rewind(step: Option<usize>, chat_id: String) -> Result<()> {
    let cwd = std::env::current_dir()?;
    if let Some(step_num) = step {
        let msg = mint_core::git::rollback_to_step(&cwd, &chat_id, step_num)
            .map_err(|e| anyhow::anyhow!(e))?;
        println!("{msg}");
    } else {
        let checkpoints = mint_core::git::list_checkpoints(&chat_id);
        if checkpoints.is_empty() {
            println!("No checkpoints recorded for chat '{chat_id}'");
        } else {
            println!("Checkpoints for chat '{chat_id}':");
            for cp in checkpoints.iter().rev() {
                let hash = if cp.commit_hash.len() >= 7 {
                    &cp.commit_hash[..7]
                } else {
                    &cp.commit_hash
                };
                let target = cp.target_path.as_deref().unwrap_or("-");
                println!(
                    "  Step {:<3} [{}] {:<18} ({})",
                    cp.step, hash, cp.action, target
                );
            }
            println!("\nRun 'mint rewind <step> --chat-id {chat_id}' to restore.");
        }
    }
    Ok(())
}

pub async fn handle_auto() -> Result<()> {
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
                for line_str in reader.lines().map_while(Result::ok) {
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
    Ok(())
}

pub async fn handle_web(dev: bool) -> Result<()> {
    launch_mint_target("web".into(), dev).await
}

pub async fn handle_api(port: u16) -> Result<()> {
    let config = load_config()?;
    print_welcome_banner(&config);

    println!("\n{MINT}✔ Mint API Server is running!{RESET}\n");
    println!(
        "    {BLUE}API Server URL:{RESET} {MINT}http://localhost:{port}{RESET}\n"
    );

    println!("Messaging Bridges Status:");

    let bridges = [
        ("enableTelegramBridge", "Telegram Bot Bridge"),
        ("enableDiscordBridge", "Discord Bot Bridge"),
        ("enableSlackBridge", "Slack Bot Bridge"),
        ("enableLineBridge", "LINE Bot Bridge"),
        ("enableWhatsappBridge", "WhatsApp Cloud Bridge"),
        ("enableSignalBridge", "Signal Bridge"),
        ("enableEmailBridge", "Email Bridge"),
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
    Ok(())
}

pub async fn handle_gateway(command: GatewayCommand) -> Result<()> {
    match command {
        GatewayCommand::Start { api_port } => {
            let config = load_config()?;
            print_welcome_banner(&config);

            println!("\n{MINT}✔ Mint Gateway is running (headless){RESET}\n");
            println!("Messaging Bridges Status:");

            let bridges = [
                ("enableTelegramBridge", "Telegram Bot Bridge"),
                ("enableDiscordBridge", "Discord Bot Bridge"),
                ("enableSlackBridge", "Slack Bot Bridge"),
                ("enableLineBridge", "LINE Bot Bridge"),
                ("enableWhatsappBridge", "WhatsApp Cloud Bridge"),
                ("enableSignalBridge", "Signal Bridge"),
                ("enableEmailBridge", "Email Bridge"),
            ];
            let mut any_bridge_enabled = false;
            for &(key, name) in &bridges {
                let enabled = config
                    .extra
                    .get(key)
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);
                any_bridge_enabled |= enabled;
                if enabled {
                    println!("  {MINT}● {name:<23} [Active]{RESET}");
                } else {
                    println!("  {DIM}○ {name:<23} [Inactive]{RESET}");
                }
            }
            if !any_bridge_enabled {
                println!(
                    "\n{WARN}No chat bridge is enabled — the gateway will only run \
                     cron jobs. Enable one first, e.g. \
                     `mint config set enableTelegramBridge true`.{RESET}"
                );
            }

            if let Some(port) = api_port {
                println!(
                    "\n    {BLUE}API Server URL:{RESET} {MINT}http://localhost:{port}{RESET}"
                );
                tokio::spawn(async move {
                    if let Err(error) = mint_core::start_api_server(port).await {
                        eprintln!("{ERROR}API server exited: {error}{RESET}");
                    }
                });
            } else {
                mint_core::channels::start_channels();
                mint_core::start_cron_scheduler();
            }

            println!("\n{DIM}(Bridge & cron activity will be logged below){RESET}");
            println!("{DIM}Press Ctrl+C to stop{RESET}\n");
            wait_for_shutdown_signal().await;
            println!("\n👋 Shutting down Mint Gateway...");
        }
        GatewayCommand::Install {
            api_port,
            system,
            now,
            memory_max,
        } => {
            gateway::install(api_port, system, now, memory_max)?;
        }
    }
    Ok(())
}

pub async fn handle_chat(
    message: String,
    system: String,
    image: Option<PathBuf>,
    cli: &crate::Cli,
    config: &MintConfig,
) -> Result<()> {
    let image_path = image.as_deref().or(cli.image.as_deref());
    if system.trim().is_empty() {
        let current_dir = std::env::current_dir()?;
        run_oneshot_agent_task(
            &message,
            &current_dir,
            config,
            cli.fast,
            cli.plan,
            image_path,
        )
        .await?;
    } else {
        let image_data_uri = image_path
            .map(image::load_image_as_data_uri)
            .transpose()?;
        let (response, _) = orchestrate_chat_with_fallback(
            config,
            &ChatRequest {
                message: message.clone(),
                system_instruction: system,
                chat_id: Some(CHAT_CLI_ID.to_owned()),
                image_data_uri: image_data_uri.clone(),
                audio_data_uri: None,
                video_data_uri: None,
                document_attachment: None,
                workspace_path: Some(
                    std::env::current_dir()?.to_string_lossy().into_owned(),
                ),
                agent_id: None,
                plan_mode: false,
                pinned_mcp_server: None,
                messages: None,
                tools: None,
            },
        )
        .await?;
        image::save_sent_image_after_send(image_data_uri.as_deref(), &message);
        println!("{}", response.text);
    }
    Ok(())
}

pub async fn handle_imagine(
    prompt: String,
    aspect: String,
    count: u8,
    negative: Option<String>,
    output: Option<PathBuf>,
) -> Result<()> {
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
    Ok(())
}

pub async fn handle_veo(
    prompt: String,
    aspect: String,
    duration: u32,
    negative: Option<String>,
    output: Option<PathBuf>,
) -> Result<()> {
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
    Ok(())
}

pub async fn handle_video(command: VideoCommand) -> Result<()> {
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
    Ok(())
}

pub async fn handle_avatar(command: AvatarCommand) -> Result<()> {
    use mint_core::avatar_bridge::{AvatarBridgeConfig, fetch_channel_state};
    match command {
        AvatarCommand::Link => {
            let mut config = load_config()?;
            if config.avatar_token.is_empty() {
                config.avatar_token = AvatarBridgeConfig::generate_token();
                save_config(&config)?;
            }
            let cfg = AvatarBridgeConfig::from_mint_config(&config);
            println!(
                "{MINT}Avatar share link:{RESET}\n{}",
                cfg.share_link().expect("token was just ensured non-empty")
            );
        }
        AvatarCommand::Status => {
            let config = load_config()?;
            let cfg = AvatarBridgeConfig::from_mint_config(&config);
            if !cfg.enabled {
                println!("No avatar token set yet. Run `mint avatar link` first.");
            } else {
                match fetch_channel_state(&cfg).await {
                    Ok(state) => {
                        let now_ms = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(0);
                        println!("{MINT}Avatar channel status:{RESET}");
                        println!(
                            "  model:       {}",
                            state.model.unwrap_or_else(|| "not selected".into())
                        );
                        println!("  viewers:     {}", state.connected_clients);
                        println!(
                            "  last event:  {}",
                            state
                                .last_agent_event_at
                                .map(|t| format!("{}s ago", (now_ms - t).max(0) / 1000))
                                .unwrap_or_else(|| "never".into())
                        );
                    }
                    Err(e) => {
                        eprintln!("{ERROR}✗ {e}{RESET}");
                        anyhow::bail!("{e}");
                    }
                }
            }
        }
        AvatarCommand::SetRelay { relay_url, app_url } => {
            let mut config = load_config()?;
            config.avatar_relay_url = relay_url.trim_end_matches('/').to_string();
            if let Some(app) = app_url {
                config.avatar_app_url = app.trim_end_matches('/').to_string();
            }
            save_config(&config)?;
            println!(
                "Relay set to {} (app: {})",
                config.avatar_relay_url, config.avatar_app_url
            );
        }
        AvatarCommand::Disable => {
            let mut config = load_config()?;
            config.avatar_token = String::new();
            save_config(&config)?;
            println!("Avatar bridge disabled.");
        }
    }
    Ok(())
}

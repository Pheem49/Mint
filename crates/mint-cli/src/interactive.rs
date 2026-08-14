use crate::background::{BackgroundJobs, JobStatus};
use crate::onboard;
use crate::{BLUE, COMPOSER_BG, DIM, ERROR, MINT, RESET, WARN};
use crate::{agent, image};
use anyhow::Result;
use mint_core::{CHAT_CLI_ID, MemoryStore, MintConfig};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

pub static SESSION_APPROVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Separate from `SESSION_APPROVED`: gates only the agent's high-risk
/// "Security Authorization" prompts, so approving "Entire Session" for a
/// routine shell/skill confirmation can never silently wave through an
/// unrelated, higher-stakes agent action.
pub static SECURITY_SESSION_APPROVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub struct InteractiveSession {
    pub config: MintConfig,
    pub current_dir: PathBuf,
    pub fast_mode: bool,
    pub plan_mode: bool,
    pub pending_image: Option<String>, // base64 data URI
    pub history: Vec<String>,          // previously submitted input lines, oldest first
    pub jobs: BackgroundJobs,          // /bg jobs running (or finished) outside the prompt loop
}

pub struct InteractiveInput {
    pub text: String,
    pub pasted_image: Option<String>,
}

/// What the slash-command router wants the loop to do next.
pub enum SlashResult {
    /// Command handled — continue loop without sending to agent.
    Handled,
    /// Pass this (possibly modified) query to the agent.
    ForwardToAgent(String),
    /// Break out of the loop.
    Exit,
}

fn parse_path_and_prompt(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if rest.is_empty() {
        return (String::new(), String::new());
    }

    let mut chars = rest.chars();
    if let Some(first) = chars.next() {
        if first == '"' || first == '\'' {
            let quote = first;
            if let Some(end_idx) = rest[1..].find(quote) {
                let path = rest[1..1 + end_idx].to_string();
                let prompt = rest[1 + end_idx + 1..].trim().to_string();
                return (path, prompt);
            }
        }
    }

    if let Some((path, prompt)) = rest.split_once(char::is_whitespace) {
        (path.to_string(), prompt.trim().to_string())
    } else {
        (rest.to_string(), String::new())
    }
}

/// Route `/…` commands. Returns `None` if the input is not a slash command.
pub async fn handle_slash_command(
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
                (
                    "/plan [on|off]",
                    "Toggle plan mode (read-only until you approve a plan)",
                ),
                ("/models [name]", "List providers or switch provider"),
                ("/clear", "Clear conversation history"),
                ("/cd <path>", "Change workspace directory"),
                ("/image <path> [prompt]", "Attach image from file"),
                ("/paste [prompt]", "Attach image from clipboard"),
                ("Ctrl+V", "Paste clipboard image as [Image #1]"),
                ("↑ / ↓", "Recall previously submitted input"),
                ("/learn <path>", "Import a persistent .md or .txt skill"),
                (
                    "/skill add <path>",
                    "Copy/install skill file or folder to global config",
                ),
                (
                    "/plugins <name> [prompt]",
                    "Generate a skill.md file for a plugin/skill",
                ),
                ("/memory list", "Show recent interactions"),
                ("/memory clear", "Clear all interactions"),
                ("/memory get <key>", "Read a profile value"),
                ("/memory set <key> <val>", "Store a profile value"),
                (
                    "/autoskill [on|off]",
                    "Toggle auto-writing a SKILL.md after hard tasks",
                ),
                (
                    "/cron add <name> | <sched> | <task>",
                    "Create/list/remove/enable/disable scheduled agent tasks",
                ),
                (
                    "/link add <name> | <path> | <desc>",
                    "Link a folder chat can auto-write notes into",
                ),
                (
                    "/image-provider [name]",
                    "List image gen providers or switch default provider",
                ),
                ("/generate-image <prompt>", "Generate image using AI model"),
                ("/veo <prompt>", "Generate video using Google Veo"),
                (
                    "/video-provider [name]",
                    "List video gen providers or switch default provider",
                ),
                (
                    "/search-provider [name]",
                    "List web search providers or switch default provider",
                ),
                ("/bg <query>", "Run a query in the background, non-blocking"),
                (
                    "/jobs [show|cancel <id>]",
                    "List, inspect, or cancel background jobs",
                ),
                ("/mcp list", "List configured MCP servers"),
                ("/mcp allow <server> <tool>", "Allow an MCP tool"),
                (
                    "/mcp reauth <server>",
                    "Re-run a server's OAuth login (e.g. expired token)",
                ),
                ("/release-notes", "Show release notes for the current version"),
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
            let choice = if rest.is_empty() {
                let options = vec![
                    "on (hide thinking traces)".to_string(),
                    "off (show thinking traces)".to_string(),
                ];
                let current = if session.fast_mode {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Select Fast Mode", &options, current) {
                    Ok(Some(sel)) => {
                        if sel.starts_with("on") {
                            Some(true)
                        } else {
                            Some(false)
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        println!("{ERROR}Error selecting fast mode:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                match rest {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => {
                        println!("{WARN}/fast usage: /fast [on|off]{RESET}\n");
                        None
                    }
                }
            };

            if let Some(mode) = choice {
                session.fast_mode = mode;
                if session.fast_mode {
                    println!("{DIM}[Fast] mode ON — thinking traces hidden{RESET}\n");
                } else {
                    println!("{DIM}[Fast] mode OFF{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/plan" => {
            let choice = if rest.is_empty() {
                let options = vec![
                    "on (read-only until plan is approved)".to_string(),
                    "off (edits/shell run immediately)".to_string(),
                ];
                let current = if session.plan_mode {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Select Plan Mode", &options, current) {
                    Ok(Some(sel)) => {
                        if sel.starts_with("on") {
                            Some(true)
                        } else {
                            Some(false)
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        println!("{ERROR}Error selecting plan mode:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                match rest {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => {
                        println!("{WARN}/plan usage: /plan [on|off]{RESET}\n");
                        None
                    }
                }
            };

            if let Some(mode) = choice {
                session.plan_mode = mode;
                if session.plan_mode {
                    println!(
                        "{DIM}[Plan] mode ON — agent will investigate read-only and present a plan via exit_plan_mode before editing files or running commands{RESET}\n"
                    );
                } else {
                    println!("{DIM}[Plan] mode OFF{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/models" => {
            let selected_provider = if rest.is_empty() {
                let providers = session.config.available_providers();
                match prompt_interactive_select(
                    "Select AI provider",
                    &providers,
                    &session.config.ai_provider,
                ) {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        println!("Cancelled provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                Some(rest.to_owned())
            };

            if let Some(provider) = selected_provider {
                // Switch provider first so `active_model()` reflects that
                // provider's own default/last-used model, then offer a
                // second picker (when a model list is known for it) so the
                // user isn't stuck with the default after choosing a provider.
                match session.config.set_active_model(&provider, None) {
                    Ok(mut display_name) => {
                        let mut model_options =
                            model_options_for_provider(&session.config, &provider);
                        let current_model = session.config.active_model().to_string();
                        if !current_model.is_empty()
                            && !model_options.iter().any(|m| m == &current_model)
                        {
                            model_options.insert(0, current_model.clone());
                        }

                        if !model_options.is_empty() {
                            match prompt_interactive_select(
                                &format!("Select {provider} model"),
                                &model_options,
                                &current_model,
                            ) {
                                Ok(Some(model)) if model != current_model => {
                                    match session.config.set_active_model(&provider, Some(&model)) {
                                        Ok(name) => display_name = name,
                                        Err(error) => {
                                            println!("{ERROR}Config error:{RESET} {error}")
                                        }
                                    }
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    println!("{ERROR}Error selecting model:{RESET} {e}\n")
                                }
                            }
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
                        let truncated_instruction = if agent.system_instruction.chars().count() > 60
                        {
                            let head: String = agent.system_instruction.chars().take(60).collect();
                            format!("{}...", head.replace('\n', " "))
                        } else {
                            agent.system_instruction.replace('\n', " ")
                        };
                        println!("    {DIM}Instruction: {truncated_instruction}{RESET}");
                    }
                }
                println!();

                let options = vec![
                    "Keep current status".to_string(),
                    "on (enable collaboration)".to_string(),
                    "off (disable collaboration)".to_string(),
                ];
                let current = &options[0];
                match prompt_interactive_select("Toggle Collaboration Status?", &options, current) {
                    Ok(Some(sel)) => {
                        if sel.starts_with("on") {
                            session.config.enable_agent_collaboration = true;
                            match mint_core::save_config(&session.config) {
                                Ok(()) => println!(
                                    "{DIM}Multi-Agent collaboration set to: Enabled{RESET}\n"
                                ),
                                Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                            }
                        } else if sel.starts_with("off") {
                            session.config.enable_agent_collaboration = false;
                            match mint_core::save_config(&session.config) {
                                Ok(()) => println!(
                                    "{DIM}Multi-Agent collaboration set to: Disabled{RESET}\n"
                                ),
                                Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(SlashResult::Handled)
        }

        "/autoskill" => {
            if rest == "on" || rest == "off" {
                session.config.auto_skill_writing = rest == "on";
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Auto skill writing set to: {}{RESET}\n",
                        if session.config.auto_skill_writing {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /autoskill [on|off]{RESET}\n");
            } else {
                println!(
                    "{DIM}When enabled, the agent may write a new .agents/skills/<name>/SKILL.md after finishing a non-trivial, reusable task.{RESET}"
                );
                let options = vec!["on (enable)".to_string(), "off (disable)".to_string()];
                let current = if session.config.auto_skill_writing {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Auto Skill Writing", &options, current) {
                    Ok(Some(sel)) => {
                        session.config.auto_skill_writing = sel.starts_with("on");
                        match mint_core::save_config(&session.config) {
                            Ok(()) => println!(
                                "{DIM}Auto skill writing set to: {}{RESET}\n",
                                if session.config.auto_skill_writing {
                                    "Enabled"
                                } else {
                                    "Disabled"
                                }
                            ),
                            Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                        }
                    }
                    Ok(None) => println!("{DIM}Cancelled.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error selecting option:{RESET} {e}\n"),
                }
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

            let selected_provider = if rest.is_empty() {
                let options: Vec<String> = available.iter().map(|s| s.to_string()).collect();
                match prompt_interactive_select(
                    "Select Image Generation provider",
                    &options,
                    &session.config.image_gen_provider,
                ) {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        println!("Cancelled image provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                if available.contains(&rest) {
                    Some(rest.to_owned())
                } else {
                    println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                    None
                }
            };

            if let Some(provider) = selected_provider {
                session.config.image_gen_provider = provider;
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Switched default image provider to: {}{RESET}\n",
                        session.config.image_gen_provider
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/video-provider" => {
            let mut available = Vec::new();
            if !session.config.api_key.trim().is_empty() || std::env::var("GEMINI_API_KEY").is_ok()
            {
                available.push("veo");
            }
            if available.is_empty() {
                available.push("veo");
            }

            let current_provider = session
                .config
                .extra
                .get("videoGenProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("veo")
                .to_string();

            let selected_provider = if rest.is_empty() {
                let options = vec!["Google Veo (Gemini Videos)".to_string()];
                let current_display = if current_provider == "veo" {
                    "Google Veo (Gemini Videos)"
                } else {
                    &current_provider
                };
                match prompt_interactive_select(
                    "Select Video Generation provider",
                    &options,
                    current_display,
                ) {
                    Ok(Some(p)) => {
                        if p == "Google Veo (Gemini Videos)" {
                            Some("veo".to_string())
                        } else {
                            Some(p)
                        }
                    }
                    Ok(None) => {
                        println!("Cancelled video provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                if rest == "veo" || rest == "Google Veo (Gemini Videos)" {
                    Some("veo".to_string())
                } else {
                    println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                    None
                }
            };

            if let Some(provider) = selected_provider {
                session.config.extra.insert(
                    "videoGenProvider".to_string(),
                    serde_json::Value::String(provider.clone()),
                );
                match mint_core::save_config(&session.config) {
                    Ok(()) => {
                        let display_name = if provider == "veo" {
                            "Google Veo (Gemini Videos)"
                        } else {
                            &provider
                        };
                        println!(
                            "{DIM}Switched default video provider to: {}{RESET}\n",
                            display_name
                        );
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/search-provider" | "/searchProvider" => {
            let mut available: Vec<(&str, &str)> = Vec::new();
            let google_configured = session
                .config
                .extra
                .get("googleSearchApiKey")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty())
                && session
                    .config
                    .extra
                    .get("googleSearchCx")
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| !v.trim().is_empty());
            if google_configured {
                available.push(("google", "Google Search API"));
            }
            if session
                .config
                .extra
                .get("braveSearchApiKey")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty())
            {
                available.push(("brave", "Brave Search API"));
            }
            if session
                .config
                .extra
                .get("searxngBaseUrl")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty())
            {
                available.push(("searxng", "SearXNG (self-hosted)"));
            }

            if available.is_empty() {
                println!(
                    "{ERROR}No search provider is configured yet.{RESET} Run onboarding or set a Google/Brave key or SearXNG URL in Settings first.\n"
                );
                return Some(SlashResult::Handled);
            }

            let current_provider = session
                .config
                .extra
                .get("searchProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let selected_provider = if rest.is_empty() {
                let options: Vec<String> = available
                    .iter()
                    .map(|(_, label)| label.to_string())
                    .collect();
                let current_display = available
                    .iter()
                    .find(|(key, _)| *key == current_provider)
                    .map(|(_, label)| *label)
                    .unwrap_or(available[0].1);
                match prompt_interactive_select(
                    "Select Web Search provider",
                    &options,
                    current_display,
                ) {
                    Ok(Some(label)) => available
                        .iter()
                        .find(|(_, l)| *l == label)
                        .map(|(key, _)| key.to_string()),
                    Ok(None) => {
                        println!("Cancelled search provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else if let Some((key, _)) = available
                .iter()
                .find(|(key, label)| *key == rest || *label == rest)
            {
                Some(key.to_string())
            } else {
                println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                None
            };

            if let Some(provider) = selected_provider {
                session.config.extra.insert(
                    "searchProvider".to_string(),
                    serde_json::Value::String(provider.clone()),
                );
                match mint_core::save_config(&session.config) {
                    Ok(()) => {
                        let display_name = available
                            .iter()
                            .find(|(key, _)| *key == provider)
                            .map(|(_, label)| *label)
                            .unwrap_or(&provider);
                        println!(
                            "{DIM}Switched default web search provider to: {}{RESET}\n",
                            display_name
                        );
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/bg" => {
            if rest.is_empty() {
                println!("{ERROR}Usage:{RESET} /bg <query>\n");
                return Some(SlashResult::Handled);
            }
            let id = session
                .jobs
                .spawn(&session.config, &session.current_dir, rest);
            println!(
                "{DIM}Started background job #{id}. Keep chatting — check on it with {RESET}/jobs{DIM}, or view its result with {RESET}/jobs show {id}{DIM}.{RESET}"
            );
            println!(
                "{DIM}Note: /bg runs non-interactively, so file writes/shell/plugin/MCP actions needing approval are declined automatically.{RESET}\n"
            );
            Some(SlashResult::Handled)
        }

        "/jobs" => {
            if rest.is_empty() {
                let list = session.jobs.list();
                if list.is_empty() {
                    println!("{DIM}No background jobs yet. Start one with /bg <query>.{RESET}\n");
                } else {
                    println!("\n{BLUE}Background jobs{RESET}");
                    for job in &list {
                        let status_str = match job.status {
                            JobStatus::Running => format!("{MINT}running{RESET}"),
                            JobStatus::Done => "done".to_string(),
                            JobStatus::Failed => format!("{ERROR}failed{RESET}"),
                            JobStatus::Cancelled => format!("{DIM}cancelled{RESET}"),
                        };
                        let preview: String = job.query.chars().take(50).collect();
                        let suffix = if job.query.chars().count() > 50 {
                            "…"
                        } else {
                            ""
                        };
                        println!(
                            "  #{:<3} [{status_str}] {DIM}{:>4}s{RESET}  {preview}{suffix}",
                            job.id,
                            job.elapsed_secs(),
                        );
                    }
                    println!();
                }
                return Some(SlashResult::Handled);
            }

            let (sub, arg) = rest
                .split_once(char::is_whitespace)
                .map(|(s, a)| (s, a.trim()))
                .unwrap_or((rest, ""));

            match sub {
                "show" => match arg.parse::<u32>().ok().and_then(|id| session.jobs.show(id)) {
                    Some(job) => {
                        println!("\n{BLUE}Job #{} — {}{RESET}", job.id, job.status);
                        println!("{DIM}Query:{RESET} {}", job.query);
                        match job.result {
                            Some(result) => println!("\n{}\n", result),
                            None => println!("{DIM}(still running — no result yet){RESET}\n"),
                        }
                    }
                    None => println!("{ERROR}No such job.{RESET}\n"),
                },
                "cancel" => match arg.parse::<u32>().ok() {
                    Some(id) if session.jobs.cancel(id) => {
                        println!("{DIM}Cancelling job #{id}...{RESET}\n");
                    }
                    _ => println!("{ERROR}No such running job.{RESET}\n"),
                },
                _ => println!("{ERROR}Usage:{RESET} /jobs [show <id>|cancel <id>]\n"),
            }
            Some(SlashResult::Handled)
        }

        "/shells" => {
            if rest.is_empty() {
                let jobs = mint_core::bg_shell::list_jobs();
                if jobs.is_empty() {
                    println!(
                        "{DIM}No background shell jobs. The agent starts one when it calls run_shell with background: true.{RESET}\n"
                    );
                } else {
                    println!("\n{BLUE}Background shell jobs{RESET}");
                    for job in &jobs {
                        let status_str = match &job.status {
                            mint_core::bg_shell::JobStatus::Running => {
                                format!("{MINT}running{RESET}")
                            }
                            mint_core::bg_shell::JobStatus::Exited(0) => "exited(0)".to_string(),
                            mint_core::bg_shell::JobStatus::Exited(code) => {
                                format!("{ERROR}exited({code}){RESET}")
                            }
                            mint_core::bg_shell::JobStatus::Killed => {
                                format!("{DIM}killed{RESET}")
                            }
                            mint_core::bg_shell::JobStatus::Failed(err) => {
                                format!("{ERROR}failed: {err}{RESET}")
                            }
                        };
                        let preview: String = job.command.chars().take(50).collect();
                        let suffix = if job.command.chars().count() > 50 {
                            "…"
                        } else {
                            ""
                        };
                        println!(
                            "  {:<6} [{status_str}] {DIM}{:>4}s{RESET}  {preview}{suffix}",
                            job.id, job.elapsed_secs,
                        );
                    }
                    println!();
                }
                return Some(SlashResult::Handled);
            }

            let (sub, arg) = rest
                .split_once(char::is_whitespace)
                .map(|(s, a)| (s, a.trim()))
                .unwrap_or((rest, ""));

            match sub {
                "show" if !arg.is_empty() => match mint_core::bg_shell::snapshot(arg) {
                    Ok(job) => {
                        println!("\n{BLUE}Job {} — {}{RESET}", job.id, job.command);
                        if let Some(pid) = job.pid {
                            println!("{DIM}pid: {pid}  cwd: {}{RESET}", job.cwd.display());
                        }
                        println!("{DIM}stdout:{RESET}\n{}", job.stdout);
                        println!("{DIM}stderr:{RESET}\n{}\n", job.stderr);
                    }
                    Err(_) => println!("{ERROR}No such job.{RESET}\n"),
                },
                "kill" if !arg.is_empty() => match mint_core::bg_shell::kill_job(arg) {
                    Ok(msg) => println!("{DIM}{msg}{RESET}\n"),
                    Err(_) => println!("{ERROR}No such job.{RESET}\n"),
                },
                _ => println!("{ERROR}Usage:{RESET} /shells [show <id>|kill <id>]\n"),
            }
            Some(SlashResult::Handled)
        }

        "/clear" | "/reset" => {
            let options = vec![
                "No (keep history)".to_string(),
                "Yes (clear history)".to_string(),
            ];
            let choice = match prompt_interactive_select(
                "Clear conversation history?",
                &options,
                &options[0],
            ) {
                Ok(Some(sel)) => sel == options[1],
                _ => false,
            };

            if choice {
                if let Ok(memory) = MemoryStore::open_default() {
                    match memory.clear_interactions() {
                        Ok(count) => println!("{DIM}Cleared {count} interactions.{RESET}"),
                        Err(error) => println!("{ERROR}Memory error:{RESET} {error}"),
                    }
                }
                println!("{DIM}Conversation context cleared.{RESET}\n");
            } else {
                println!("{DIM}Cancelled.{RESET}\n");
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

        "/veo" => {
            if rest.is_empty() {
                println!(
                    "{WARN}Usage: /veo <prompt> [--aspect <ratio>] [--duration <secs>]{RESET}\n"
                );
            } else {
                let mut prompt = rest.to_string();
                let mut aspect = "16:9".to_string();
                let mut duration = 5;

                // Simple flag parsing
                if let Some(pos) = prompt.find("--aspect") {
                    let rest_str = prompt[pos..].to_string();
                    let mut parts = rest_str.split_whitespace();
                    parts.next(); // Skip --aspect
                    if let Some(val) = parts.next() {
                        aspect = val.to_string();
                    }
                    prompt = prompt[..pos].trim().to_string();
                }
                if let Some(pos) = prompt.find("--duration") {
                    let rest_str = prompt[pos..].to_string();
                    let mut parts = rest_str.split_whitespace();
                    parts.next(); // Skip --duration
                    if let Some(val) = parts.next() {
                        if let Ok(parsed) = val.parse::<u32>() {
                            duration = parsed;
                        }
                    }
                    prompt = prompt[..pos].trim().to_string();
                }

                use indicatif::{ProgressBar, ProgressStyle};
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                        .template("{spinner:.magenta} {msg}")
                        .unwrap(),
                );
                let gen_request = mint_core::VideoGenRequest {
                    prompt: prompt.clone(),
                    negative_prompt: None,
                    aspect_ratio: aspect.clone(),
                    duration,
                    model: None,
                    provider: "veo".to_string(),
                };

                match mint_core::generate_video(&session.config, &gen_request).await {
                    Ok(result) => {
                        spinner.finish_and_clear();
                        if let Some(video) = result.videos.first() {
                            println!("{MINT}✓ Video generated successfully!{RESET}");
                            println!("{MINT}Saved to: {}{RESET}\n", video.path.display());
                        } else {
                            println!("{ERROR}✗ Video generation returned no videos.{RESET}\n");
                        }
                    }
                    Err(e) => {
                        spinner.finish_and_clear();
                        println!("{ERROR}✗ Video generation failed: {e}{RESET}\n");
                    }
                }
            }
            Some(SlashResult::Handled)
        }

        "/image" => {
            let (img_path, prompt) = parse_path_and_prompt(rest);

            if img_path.is_empty() {
                println!("{WARN}/image usage: /image <path> [prompt]{RESET}\n");
                return Some(SlashResult::Handled);
            }
            match image::load_image_as_data_uri(std::path::Path::new(&img_path)) {
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

        "/edit-image" => {
            let (img_path, instruction) = parse_path_and_prompt(rest);

            if img_path.is_empty() || instruction.is_empty() {
                println!("{WARN}Usage: /edit-image <image_path> <editing instruction>{RESET}\n");
                return Some(SlashResult::Handled);
            }
            match image::load_image_as_data_uri(std::path::Path::new(&img_path)) {
                Ok(uri) => {
                    println!(
                        "{DIM}Editing image with prompt: \"{}\"...{RESET}",
                        instruction
                    );
                    let req = mint_core::ImageGenRequest {
                        prompt: instruction.to_owned(),
                        negative_prompt: None,
                        aspect_ratio: None,
                        num_images: Some(1),
                        model: None,
                        provider: None,
                        image_data_uri: Some(uri),
                        mask_data_uri: None,
                        mode: Some("edit".to_owned()),
                    };
                    match mint_core::generate_images(&session.config, &req).await {
                        Ok(resp) => {
                            if let Some(first) = resp.images.first() {
                                println!(
                                    "{DIM}Image edited successfully using {} ({}){RESET}\n",
                                    resp.provider, resp.model
                                );
                                println!(
                                    "{DIM}Image data URI prefix: {}{RESET}\n",
                                    &first.data_uri[..first.data_uri.len().min(60)]
                                );
                            }
                        }
                        Err(e) => {
                            println!("{ERROR}Image editing failed: {e}{RESET}\n");
                        }
                    }
                }
                Err(e) => {
                    println!("{ERROR}Failed to load image: {e}{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/generate-image" | "/gen-image" => {
            let prompt = rest.trim();
            if prompt.is_empty() {
                println!("{WARN}Usage: /generate-image <prompt>{RESET}\n");
                Some(SlashResult::Handled)
            } else {
                Some(SlashResult::ForwardToAgent(format!(
                    "Generate an image of {}",
                    prompt
                )))
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
                match crate::skills::learn(&path) {
                    Ok(skill) => println!(
                        "{DIM}Learned skill: {} ({}){RESET}\n",
                        skill.name, skill.source_path
                    ),
                    Err(error) => println!("{ERROR}Learn error:{RESET} {error}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/skill" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));

            match subcmd {
                "add" | "install" => {
                    if args.is_empty() {
                        println!(
                            "{WARN}/skill add <path> requires a path to a skill file or folder{RESET}\n"
                        );
                    } else {
                        let source_path = PathBuf::from(args);
                        let source_path = if source_path.is_absolute() {
                            source_path
                        } else {
                            session.current_dir.join(source_path)
                        };

                        if !source_path.exists() {
                            println!("{ERROR}Source skill path not found: {}{RESET}\n", args);
                        } else if let Some(home) = dirs::home_dir() {
                            let global_skills_path =
                                home.join(".config").join("mint").join("mint-skills");
                            if !global_skills_path.exists() {
                                let _ = std::fs::create_dir_all(&global_skills_path);
                            }

                            let name = source_path
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("skill");

                            let dest_path = global_skills_path.join(name);
                            let copy_res = if source_path.is_dir() {
                                copy_dir_all(&source_path, &dest_path)
                            } else {
                                std::fs::copy(&source_path, &dest_path)
                                    .map(|_| ())
                                    .map_err(|e| e)
                            };

                            match copy_res {
                                Ok(()) => println!(
                                    "{DIM}Skill successfully copied to Global config: {}{RESET}\n",
                                    dest_path.display()
                                ),
                                Err(e) => println!(
                                    "{ERROR}Failed to copy skill to Global: {}{RESET}\n",
                                    e
                                ),
                            }
                        } else {
                            println!(
                                "{ERROR}Unable to resolve home directory for Global config.{RESET}\n"
                            );
                        }
                    }
                }
                _ => {
                    println!("{WARN}Usage: /skill add <path>  (or /skill install <path>){RESET}\n");
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

        "/cron" => {
            let cron_jobs = match mint_core::CronStore::open_default() {
                Ok(store) => store,
                Err(e) => {
                    println!("{ERROR}Cron error:{RESET} {e}\n");
                    return Some(SlashResult::Handled);
                }
            };
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match cron_jobs.list() {
                    Ok(jobs) => {
                        if jobs.is_empty() {
                            println!("{DIM}No cron jobs.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Cron jobs:{RESET}");
                            for job in &jobs {
                                let status = if job.enabled {
                                    format!("{MINT}on{RESET}")
                                } else {
                                    format!("{DIM}off{RESET}")
                                };
                                println!(
                                    "  [{}] {} {DIM}({}){RESET} next: {} {DIM}last: {}{RESET}",
                                    job.id,
                                    job.name,
                                    status,
                                    format_local_time(&job.next_run),
                                    job.last_status.as_deref().unwrap_or("never run")
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "add" => {
                    let fields: Vec<&str> = args.splitn(3, '|').map(str::trim).collect();
                    match fields.as_slice() {
                        [name, schedule, task] if !name.is_empty() && !task.is_empty() => {
                            match cron_jobs.add(
                                *name,
                                *schedule,
                                *task,
                                session.current_dir.clone(),
                            ) {
                                Ok(job) => println!(
                                    "{DIM}Created cron job {} — next run: {}{RESET}\n",
                                    job.id,
                                    format_local_time(&job.next_run)
                                ),
                                Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                            }
                        }
                        _ => println!(
                            "{WARN}/cron add <name> | <schedule> | <task>{RESET}\n{DIM}e.g. /cron add stock report | 0 8 * * * | fetch today's stock prices and summarize{RESET}\n"
                        ),
                    }
                }
                "remove" => {
                    if args.is_empty() {
                        println!("{WARN}/cron remove <id>{RESET}\n");
                    } else {
                        match cron_jobs.remove(args) {
                            Ok(true) => println!("{DIM}Removed {args}.{RESET}\n"),
                            Ok(false) => println!("{WARN}No cron job with id {args}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "enable" | "disable" => {
                    if args.is_empty() {
                        println!("{WARN}/cron {subcmd} <id>{RESET}\n");
                    } else {
                        match cron_jobs.set_enabled(args, subcmd == "enable") {
                            Ok(Some(_)) => println!("{DIM}{subcmd}d {args}.{RESET}\n"),
                            Ok(None) => println!("{WARN}No cron job with id {args}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                _ => println!(
                    "{WARN}/cron usage: list | add <name> | <schedule> | <task> | remove <id> | enable <id> | disable <id>{RESET}\n{DIM}For run-now, use `mint cron run-now <id>` in a terminal.{RESET}\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/link" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match mint_core::list_linked_folders() {
                    Ok(folders) => {
                        if folders.is_empty() {
                            println!("{DIM}No linked folders.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Linked folders:{RESET}");
                            for folder in folders.values() {
                                println!(
                                    "  {} → {} {DIM}{}{RESET}",
                                    folder.name,
                                    folder.path.display(),
                                    folder.description.as_deref().unwrap_or("")
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "add" => {
                    let fields: Vec<&str> = args.splitn(3, '|').map(str::trim).collect();
                    match fields.as_slice() {
                        [name, path] | [name, path, ""] if !name.is_empty() && !path.is_empty() => {
                            match mint_core::add_linked_folder(name, Path::new(path), None) {
                                Ok(()) => println!("{DIM}Linked folder: {name}{RESET}\n"),
                                Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                            }
                        }
                        [name, path, description] if !name.is_empty() && !path.is_empty() => {
                            match mint_core::add_linked_folder(
                                name,
                                Path::new(path),
                                Some(description.to_string()),
                            ) {
                                Ok(()) => println!("{DIM}Linked folder: {name}{RESET}\n"),
                                Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                            }
                        }
                        _ => println!(
                            "{WARN}/link add <name> | <path> | <description>{RESET}\n{DIM}e.g. /link add Food | ~/notes/food | restaurant reviews and recipes{RESET}\n"
                        ),
                    }
                }
                "remove" => {
                    if args.is_empty() {
                        println!("{WARN}/link remove <name>{RESET}\n");
                    } else {
                        match mint_core::remove_linked_folder(args) {
                            Ok(true) => println!("{DIM}Removed {args}.{RESET}\n"),
                            Ok(false) => println!("{WARN}No linked folder named {args}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                _ => println!(
                    "{WARN}/link usage: list | add <name> | <path> | <description> | remove <name>{RESET}\n"
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
                "list" | "" => match crate::mcp::list() {
                    Ok(servers) => {
                        if servers.is_empty() {
                            println!("{DIM}(No MCP servers configured.){RESET}\n");
                        } else {
                            let allowed_mcp = session
                                .config
                                .extra
                                .get("allowedMcpTools")
                                .and_then(|v| v.as_object());

                            let mut choices = vec!["Cancel / Keep current settings".to_string()];
                            let mut server_names = vec![];

                            let max_name_len = servers.keys().map(|k| k.len()).max().unwrap_or(10);

                            for (name, srv) in &servers {
                                let args_str = srv.args.join(" ");
                                let status_label = if let Some(allowed) =
                                    allowed_mcp.and_then(|m| m.get(name))
                                {
                                    if let Some(arr) = allowed.as_array() {
                                        let tools: Vec<&str> =
                                            arr.iter().filter_map(|v| v.as_str()).collect();
                                        if tools.contains(&"*") {
                                            format!("{MINT}[Allowed: *]{RESET}")
                                        } else if tools.is_empty() {
                                            format!("{DIM}[No tools allowed]{RESET}")
                                        } else {
                                            format!("{MINT}[Allowed: {}]{RESET}", tools.join(", "))
                                        }
                                    } else {
                                        format!("{DIM}[No tools allowed]{RESET}")
                                    }
                                } else {
                                    format!("{DIM}[No tools allowed]{RESET}")
                                };

                                let padded_name =
                                    format!("{:<width$}", name, width = max_name_len + 2);
                                choices.push(format!(
                                    "{}{} ({} {})",
                                    padded_name, status_label, srv.command, args_str
                                ));
                                server_names.push(name.clone());
                            }

                            match prompt_interactive_select(
                                "Select MCP Server",
                                &choices,
                                &choices[0],
                            ) {
                                Ok(Some(selected_choice)) => {
                                    if selected_choice != choices[0] {
                                        if let Some(pos) =
                                            choices.iter().position(|c| c == &selected_choice)
                                        {
                                            let server_name = &server_names[pos - 1];

                                            println!(
                                                "{DIM}Checking connection to '{server_name}'...{RESET}"
                                            );
                                            let is_connected = mint_core::list_server_tools(
                                                &session.config,
                                                server_name,
                                            )
                                            .is_ok();
                                            if is_connected {
                                                println!("{MINT}● Connected{RESET}\n");
                                            } else {
                                                println!(
                                                    "{ERROR}● Not connected{RESET} {DIM}(token may be expired, or the server is unreachable){RESET}\n"
                                                );
                                            }

                                            let auth_options = vec![
                                                "Keep current settings".to_string(),
                                                "Allow all tools (*)".to_string(),
                                                "Re-authenticate (re-run OAuth login)".to_string(),
                                            ];
                                            let default_choice = if is_connected {
                                                &auth_options[0]
                                            } else {
                                                &auth_options[2]
                                            };

                                            let title =
                                                format!("Authorize MCP Server '{}'?", server_name);
                                            match prompt_interactive_select(
                                                &title,
                                                &auth_options,
                                                default_choice,
                                            ) {
                                                Ok(Some(auth_choice)) => {
                                                    if auth_choice == auth_options[1] {
                                                        match crate::mcp::allow(server_name, "*") {
                                                            Ok(true) => {
                                                                println!(
                                                                    "{DIM}Allowed MCP tool: {server_name}/*{RESET}"
                                                                );
                                                                if let Ok(updated_config) =
                                                                    mint_core::load_config()
                                                                {
                                                                    session.config = updated_config;
                                                                }
                                                                println!(
                                                                    "{MINT}Successfully authorized all tools for: {server_name}{RESET}\n"
                                                                );
                                                            }
                                                            Ok(false) => {
                                                                println!(
                                                                    "{DIM}MCP tools already allowed for {server_name}{RESET}\n"
                                                                );
                                                            }
                                                            Err(e) => println!(
                                                                "{ERROR}MCP error:{RESET} {e}\n"
                                                            ),
                                                        }
                                                    } else if auth_choice == auth_options[2] {
                                                        println!(
                                                            "{DIM}Re-authenticating MCP server '{server_name}'... (a browser tab may open){RESET}\n"
                                                        );
                                                        match crate::mcp::reauth(server_name) {
                                                            Ok(true) => println!(
                                                                "{MINT}Re-authentication succeeded for '{server_name}'.{RESET}\n"
                                                            ),
                                                            Ok(false) => println!(
                                                                "{ERROR}Re-authentication failed for '{server_name}' (see output above).{RESET}\n"
                                                            ),
                                                            Err(e) => println!(
                                                                "{ERROR}MCP error:{RESET} {e}\n"
                                                            ),
                                                        }
                                                    }
                                                }
                                                _ => {}
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                },
                "allow" => {
                    let mut parts = args.split_whitespace();
                    let server = parts.next();
                    let tool = parts.next();
                    if let (Some(server), Some(tool)) = (server, tool) {
                        match crate::mcp::allow(server, tool) {
                            Ok(true) => {
                                println!("{DIM}Allowed MCP tool: {server}/{tool}{RESET}\n");
                                if let Ok(updated_config) = mint_core::load_config() {
                                    session.config = updated_config;
                                }
                            }
                            Ok(false) => {
                                println!("{DIM}MCP tool already allowed: {server}/{tool}{RESET}\n")
                            }
                            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                        }
                    } else {
                        println!("{WARN}/mcp allow usage: <server> <tool>{RESET}\n");
                    }
                }
                "reauth" => {
                    if args.is_empty() {
                        println!("{WARN}/mcp reauth usage: <server>{RESET}\n");
                    } else {
                        println!(
                            "{DIM}Re-authenticating MCP server '{args}'... (a browser tab may open){RESET}\n"
                        );
                        match crate::mcp::reauth(args) {
                            Ok(true) => println!(
                                "{MINT}Re-authentication succeeded for '{args}'.{RESET}\n"
                            ),
                            Ok(false) => println!(
                                "{ERROR}Re-authentication failed for '{args}' (see output above).{RESET}\n"
                            ),
                            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                        }
                    }
                }
                _ => println!(
                    "{WARN}/mcp usage: list | allow <server> <tool> | reauth <server> | remove <name> | clear{RESET}\n"
                ),
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

        "/release-notes" => {
            const RELEASE_NOTES: &str = include_str!("../../../Release_Note.md");
            println!("\n{BLUE}────────────────────────────────────────────{RESET}");
            println!("{}", RELEASE_NOTES.trim());
            println!("{BLUE}────────────────────────────────────────────{RESET}\n");
            Some(SlashResult::Handled)
        }

        "/exit" | "/quit" => Some(SlashResult::Exit),

        "/plugins" | "/plugin" => {
            let (plugin_name, prompt) = rest
                .split_once(char::is_whitespace)
                .map(|(p, r)| (p, r.trim()))
                .unwrap_or((rest, ""));

            if plugin_name.is_empty() {
                // Collect all plugins with their OAuth status
                use crossterm::{
                    event::{self, Event, KeyCode},
                    terminal::{disable_raw_mode, enable_raw_mode},
                };
                use mint_core::oauth::list_oauth_statuses;

                let oauth_statuses = list_oauth_statuses();

                // Build native plugin list with statuses
                struct PluginEntry {
                    name: String,
                    desc: String,
                    is_oauth: bool,
                    oauth_provider: String,
                    connected: bool,
                    account: Option<String>,
                }

                let mut entries: Vec<PluginEntry> = mint_core::native_plugins()
                    .iter()
                    .map(|p| {
                        // Try to find matching OAuth status
                        let (connected, account) = if !p.oauth_provider.is_empty() {
                            if let Some(st) = oauth_statuses
                                .iter()
                                .find(|s| s.provider == p.oauth_provider)
                            {
                                (st.connected, st.account_email.clone())
                            } else {
                                (false, None)
                            }
                        } else {
                            (false, None)
                        };
                        PluginEntry {
                            name: p.name.to_string(),
                            desc: p.description.to_string(),
                            is_oauth: !p.oauth_provider.is_empty(),
                            oauth_provider: p.oauth_provider.to_string(),
                            connected,
                            account,
                        }
                    })
                    .collect();

                // Add custom skills as non-OAuth entries
                let mut custom_entries: Vec<PluginEntry> = Vec::new();
                if let Ok(memory) = MemoryStore::open_default() {
                    if let Ok(skills) = memory.learned_skills(100) {
                        for s in &skills {
                            custom_entries.push(PluginEntry {
                                name: s.name.clone(),
                                desc: format!("Custom skill • {}", s.source_path),
                                is_oauth: false,
                                oauth_provider: String::new(),
                                connected: false,
                                account: None,
                            });
                        }
                    }
                }

                // Flatten into display structs
                #[allow(dead_code)]
                struct PE {
                    name: String,
                    desc: String,
                    is_oauth: bool,
                    oauth_provider: String,
                    connected: bool,
                    account: Option<String>,
                    is_custom: bool,
                }
                let display: Vec<PE> = entries
                    .drain(..)
                    .map(|e| PE {
                        is_custom: false,
                        name: e.name,
                        desc: e.desc,
                        is_oauth: e.is_oauth,
                        oauth_provider: e.oauth_provider,
                        connected: e.connected,
                        account: e.account,
                    })
                    .chain(custom_entries.drain(..).map(|e| PE {
                        is_custom: true,
                        name: e.name,
                        desc: e.desc,
                        is_oauth: e.is_oauth,
                        oauth_provider: e.oauth_provider,
                        connected: e.connected,
                        account: e.account,
                    }))
                    .collect();

                // Print header + list (separator printed inside draw_list so line count stays accurate)

                // Simple interactive list
                let mut cursor = 0usize;
                let total = display.len();
                if total == 0 {
                    println!("{MINT}  Available Plugins & Skills{RESET}");
                    println!("{BLUE}────────────────────────────────────────────{RESET}");
                    println!("  {DIM}(No plugins found.){RESET}\n");
                    return Some(SlashResult::Handled);
                }

                // Truncate helper to ensure line items don't wrap and break terminal line counts
                fn truncate_str(s: &str, max_len: usize) -> String {
                    if s.chars().count() > max_len {
                        let end: String = s.chars().take(max_len.saturating_sub(3)).collect();
                        format!("{end}...")
                    } else {
                        s.to_string()
                    }
                }

                // Count exact terminal lines printed by draw_list
                fn count_menu_lines(display: &[PE]) -> usize {
                    display.len() // one line per item only
                }

                // Draw list — same clean style as /models (❯ selected, dimmed others, no section headers)
                let draw_list = |cur: usize| {
                    for (i, e) in display.iter().enumerate() {
                        let plain_status = if e.connected {
                            format!(
                                "● {}",
                                truncate_str(e.account.as_deref().unwrap_or("yes"), 18)
                            )
                        } else if e.is_oauth {
                            "○ Not Connected".to_string()
                        } else {
                            "■ Active".to_string()
                        };
                        let short_desc = truncate_str(&e.desc, 38);
                        if i == cur {
                            let status_colored = if e.connected {
                                format!("\x1b[32m{:<22}\x1b[0m", plain_status)
                            } else if e.is_oauth {
                                format!("{DIM}{:<22}{RESET}", plain_status)
                            } else {
                                format!("\x1b[36m{:<22}\x1b[0m", plain_status)
                            };
                            println!(
                                "  {BLUE}❯ {:<18}{RESET}{}  {DIM}{short_desc}{RESET}",
                                e.name, status_colored
                            );
                        } else {
                            println!(
                                "    {DIM}{:<18}  {:<22}  {short_desc}{RESET}",
                                e.name, plain_status
                            );
                        }
                    }
                };

                let total_lines = count_menu_lines(&display);
                // Title pinned above list — printed once, not cleared on redraw (matches /models style)
                println!(
                    "  {BLUE}Plugins & Integrations (↑/↓ navigate, Enter: manage, q: exit):{RESET}"
                );
                draw_list(cursor);
                let _ = enable_raw_mode();

                let selected_idx = loop {
                    match event::poll(std::time::Duration::from_millis(100)) {
                        Ok(true) => {
                            if let Ok(Event::Key(key_event)) = event::read() {
                                if key_event.kind == event::KeyEventKind::Press {
                                    let is_ctrl_c = matches!(key_event.code, KeyCode::Char('c'))
                                        && key_event
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::CONTROL);
                                    if is_ctrl_c
                                        || matches!(
                                            key_event.code,
                                            KeyCode::Char('q') | KeyCode::Esc
                                        )
                                    {
                                        let _ = disable_raw_mode();
                                        break None;
                                    }
                                    match key_event.code {
                                        KeyCode::Up => {
                                            if cursor > 0 {
                                                cursor -= 1;
                                            } else {
                                                cursor = total - 1;
                                            }
                                            let _ = disable_raw_mode();
                                            print!("\x1b[{}A\x1b[0J", total_lines);
                                            let _ = std::io::Write::flush(&mut std::io::stdout());
                                            draw_list(cursor);
                                            let _ = enable_raw_mode();
                                        }
                                        KeyCode::Down => {
                                            if cursor < total - 1 {
                                                cursor += 1;
                                            } else {
                                                cursor = 0;
                                            }
                                            let _ = disable_raw_mode();
                                            print!("\x1b[{}A\x1b[0J", total_lines);
                                            let _ = std::io::Write::flush(&mut std::io::stdout());
                                            draw_list(cursor);
                                            let _ = enable_raw_mode();
                                        }
                                        KeyCode::Enter => {
                                            let _ = disable_raw_mode();
                                            break Some(cursor);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Ok(false) => {}
                        Err(_) => {
                            let _ = disable_raw_mode();
                            break None;
                        }
                    }
                };

                if let Some(idx) = selected_idx {
                    let selected = &display[idx];
                    println!();
                    if selected.is_oauth {
                        if selected.connected {
                            println!(
                                "\x1b[32m🟢 {} is already connected!\x1b[0m  Account: {}",
                                selected.name,
                                selected.account.as_deref().unwrap_or("yes")
                            );
                            println!(
                                "{DIM}  To disconnect, run: mint plugins logout {}{RESET}",
                                selected.oauth_provider
                            );
                        } else {
                            println!(
                                "\x1b[1;36m🔑 Sign In to {} via OAuth...\x1b[0m",
                                selected.name
                            );
                            println!("{DIM}  Opening browser for authorization...{RESET}\n");
                            crate::plugins_cli::login_plugin_oauth_public(&selected.oauth_provider)
                                .await;
                        }
                    } else if selected.is_custom {
                        println!("{BLUE}📄 Custom Skill: {}\x1b[0m", selected.name);
                        println!("{DIM}  Source: {}{RESET}", selected.desc);
                    } else {
                        println!(
                            "{BLUE}⚙️ {} is a native plugin (always available).\x1b[0m",
                            selected.name
                        );
                        println!("{DIM}  {}{RESET}", selected.desc);
                    }
                    println!();
                }

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

fn apply_welcome_gradient(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let count = chars.len();
    if count == 0 {
        return String::new();
    }

    // Gradient stops: Mint (105, 230, 166) -> Sky Blue (72, 202, 228) -> Deep Blue (0, 119, 182)
    let stops = [
        (105.0, 230.0, 166.0), // Mint Green
        (72.0, 202.0, 228.0),  // Sky Blue
        (0.0, 119.0, 182.0),   // Deep Blue
    ];

    let mut result = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            result.push(c);
            continue;
        }
        let t = if count > 1 {
            i as f32 / (count - 1) as f32
        } else {
            0.0
        };

        let (r, g, b) = if t <= 0.5 {
            let local_t = t * 2.0;
            let r = stops[0].0 + (stops[1].0 - stops[0].0) * local_t;
            let g = stops[0].1 + (stops[1].1 - stops[0].1) * local_t;
            let b = stops[0].2 + (stops[1].2 - stops[0].2) * local_t;
            (r, g, b)
        } else {
            let local_t = (t - 0.5) * 2.0;
            let r = stops[1].0 + (stops[2].0 - stops[1].0) * local_t;
            let g = stops[1].1 + (stops[2].1 - stops[1].1) * local_t;
            let b = stops[1].2 + (stops[2].2 - stops[1].2) * local_t;
            (r, g, b)
        };

        result.push_str(&format!(
            "\x1b[38;2;{};{};{}m{}\x1b[0m",
            r.round() as u8,
            g.round() as u8,
            b.round() as u8,
            c
        ));
    }
    result
}

pub fn print_welcome_banner(config: &MintConfig) {
    let provider = &config.ai_provider;
    let model = active_model(provider, config);

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
    let clean_provider_name = format_provider_display_name(provider, config);
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
            "{}   {DIM}╭{}╮{RESET}",
            apply_welcome_gradient(" __  __ _       _    ___ _    ___ "),
            "─".repeat(border_len)
        );
        println!(
            "{}   {DIM}│{RESET} {MINT}[Mint]{RESET} v{} | Active AI: {}{} {DIM}│{RESET}",
            apply_welcome_gradient("|  \\/  (_)_ __ | |_ / __| |  |_ _|"),
            version,
            clean_provider_name,
            " ".repeat(content_width - len1)
        );
        println!(
            "{}   {DIM}│{RESET} {DIM}{}{}{RESET} {DIM}│{RESET}",
            apply_welcome_gradient("| |\\/| | | '_ \\|  _| (__| |__ | | "),
            line2_text,
            " ".repeat(content_width - len2)
        );
        println!(
            "{}   {DIM}╰{}╯{RESET}",
            apply_welcome_gradient("|_|  |_|_|_| |_|\\__|\\___|\\___|___|"),
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
        println!(
            "{}",
            apply_welcome_gradient(" __  __ _       _    ___ _    ___ ")
        );
        println!(
            "{}",
            apply_welcome_gradient("|  \\/  (_)_ __ | |_ / __| |  |_ _|")
        );
        println!(
            "{}",
            apply_welcome_gradient("| |\\/| | | '_ \\|  _| (__| |__ | | ")
        );
        println!(
            "{}",
            apply_welcome_gradient("|_|  |_|_|_| |_|\\__|\\___|\\___|___|")
        );
    }
}

pub async fn run_interactive_chat() -> Result<()> {
    let config = mint_core::load_config()?;

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    print_welcome_banner(&config);
    println!("Type naturally or /help for commands. Ctrl+V pastes images. Ctrl+D exits.\n");

    let mut session = InteractiveSession {
        config,
        current_dir: current_dir.clone(),
        fast_mode: false,
        plan_mode: false,
        pending_image: None,
        history: Vec::new(),
        jobs: BackgroundJobs::new(),
    };

    let mut printed_update = false;
    if let Some((current, latest)) = crate::updater::get_cached_update_notice() {
        crate::updater::print_update_notice(&current, &latest);
        printed_update = true;
    }

    let mut update_handle = if crate::updater::should_check_for_update() {
        Some(tokio::task::spawn_blocking(
            crate::updater::check_for_update_quietly,
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
                    crate::updater::print_update_notice(&current, &latest);
                    printed_update = true;
                }
            } else {
                update_handle = Some(handle);
            }
        }

        let path_str = format_path_with_tilde(&session.current_dir);
        let model_str = active_model(&session.config.ai_provider, &session.config).to_owned();

        if let Some(input) = read_line_interactive(
            &session.config.ai_provider,
            &model_str,
            &path_str,
            &session.current_dir,
            &session.history,
            &session.jobs,
        )? {
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

            if session.history.last().map(|s| s.as_str()) != Some(query_str.as_str()) {
                session.history.push(query_str.clone());
            }

            if query_str.starts_with('$') {
                let (skill_word, task_part) = query_str
                    .split_once(char::is_whitespace)
                    .map(|(s, t)| (s, t.trim()))
                    .unwrap_or((&query_str, ""));

                let skill_name = skill_word.trim_start_matches('$').to_lowercase();
                let skills = load_all_available_skills(&session.current_dir);
                let skill_opt = skills.iter().find(|s| s.name.to_lowercase() == skill_name);

                if let Some(skill) = skill_opt {
                    println!("\n{BLUE}Skill: {}{RESET}", skill.name);
                    if let Some(ref desc) = skill.description {
                        println!("{DIM}{}{RESET}", desc);
                    }
                    println!("{DIM}────────────────────────────────────────────{RESET}");
                    println!("{}", skill.content);
                    println!("{DIM}────────────────────────────────────────────{RESET}\n");

                    if confirm("ต้องการ activate skill นี้ไหม? [y/N] ")? {
                        let final_task = if task_part.is_empty() {
                            print!("พิมพ์ Task ที่ต้องการให้ทำงานด้วย Skill นี้: ");
                            let _ = io::stdout().flush();
                            let mut input = String::new();
                            io::stdin().read_line(&mut input)?;
                            let input = input.trim().to_owned();
                            if input.is_empty() {
                                println!("{WARN}Cancelled: Task cannot be empty.{RESET}\n");
                                continue;
                            }
                            input
                        } else {
                            task_part.to_owned()
                        };

                        let task_with_skill = format!(
                            "=== ACTIVATED SKILL: {} ===\n\
                             {}\n\
                             ===========================\n\n\
                             Task: {}",
                            skill.name, skill.content, final_task
                        );

                        println!();
                        println!("{MINT}●{RESET} \x1b[1mSkill({}){RESET}", skill.name);
                        println!("  {DIM}└ Successfully loaded skill{RESET}");
                        println!();
                        if let Err(error) = crate::run_code_agent_with_saved_image(
                            &task_with_skill,
                            &session.current_dir,
                            &session.config,
                            session.pending_image.take(),
                            None,
                            agent::AgentOptions {
                                fast_mode: session.fast_mode,
                                plan_mode: session.plan_mode,
                            },
                        )
                        .await
                        {
                            println!("{ERROR}Error:{RESET} {error}\n");
                        }
                    } else {
                        println!("{DIM}Cancelled.{RESET}\n");
                    }
                } else {
                    println!("{ERROR}Skill '{}' not found.{RESET}\n", skill_name);
                }
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
                    if let Err(error) = crate::run_code_agent_with_saved_image(
                        &task,
                        &session.current_dir,
                        &session.config,
                        session.pending_image.take(),
                        None,
                        agent::AgentOptions {
                            fast_mode: session.fast_mode,
                            plan_mode: session.plan_mode,
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

            // Regular agent loop (handles both chat and coding).
            // Note: /code is fully handled by handle_slash_command above
            // (its "/code" arm always returns Some(...)), so no separate
            // "/code " fallback is needed here.
            if let Err(error) = crate::run_code_agent_with_saved_image(
                &query_str,
                &session.current_dir,
                &session.config,
                session.pending_image.take(),
                None,
                agent::AgentOptions {
                    fast_mode: session.fast_mode,
                    plan_mode: session.plan_mode,
                },
            )
            .await
            {
                println!("{ERROR}Error:{RESET} {error}\n");
            }
        } else {
            print_exit_message(&session);
            break;
        }
    }

    Ok(())
}

pub fn print_exit_message(session: &InteractiveSession) {
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

pub fn format_provider_display_name(provider: &str, config: &mint_core::MintConfig) -> String {
    if let Some(custom) = config.resolve_custom_provider(provider) {
        custom.display_name.clone()
    } else {
        match provider {
            "gemini" => "Google Gemini".to_owned(),
            "anthropic" => "Anthropic Claude".to_owned(),
            "openai" => "OpenAI".to_owned(),
            "openrouter" => "OpenRouter".to_owned(),
            "deepseek" => "DeepSeek".to_owned(),
            "huggingface" => "Hugging Face".to_owned(),
            "local_openai" => "Local OpenAI".to_owned(),
            "ollama" => "Ollama".to_owned(),
            p => p.to_owned(),
        }
    }
}

pub fn copy_dir_all(src: impl AsRef<Path>, dst: impl AsRef<Path>) -> std::io::Result<()> {
    fs::create_dir_all(&dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        if ty.is_dir() {
            copy_dir_all(entry.path(), dst.as_ref().join(entry.file_name()))?;
        } else {
            fs::copy(entry.path(), dst.as_ref().join(entry.file_name()))?;
        }
    }
    Ok(())
}

pub fn load_all_available_skills(current_dir: &Path) -> Vec<mint_core::LearnedSkill> {
    let mut skills = match MemoryStore::open_default() {
        Ok(m) => m.learned_skills(100).unwrap_or_default(),
        Err(_) => Vec::new(),
    };

    if let Some(home) = dirs::home_dir() {
        let global_agents_path = home.join(".gemini").join("config").join("AGENTS.md");
        mint_core::skills::load_agent_rules_file(&global_agents_path, &mut skills);

        let global_skills_path = home.join(".config").join("mint").join("mint-skills");
        mint_core::skills::load_skills_from_dir(&global_skills_path, &mut skills);
    }
    let workspace_agents_path1 = current_dir.join(".agents").join("AGENTS.md");
    mint_core::skills::load_agent_rules_file(&workspace_agents_path1, &mut skills);
    let workspace_agents_path2 = current_dir.join("AGENTS.md");
    mint_core::skills::load_agent_rules_file(&workspace_agents_path2, &mut skills);

    let workspace_skills_path1 = current_dir.join(".agents").join("skills");
    mint_core::skills::load_skills_from_dir(&workspace_skills_path1, &mut skills);
    let workspace_skills_path2 = current_dir.join("skills");
    mint_core::skills::load_skills_from_dir(&workspace_skills_path2, &mut skills);

    let mut unique_skills = std::collections::BTreeMap::new();
    for skill in skills {
        unique_skills.insert(skill.name.clone(), skill);
    }
    unique_skills.into_values().collect()
}

const AUTOCOMPLETE_COMMANDS: &[(&str, &str)] = &[
    (
        "/autoskill",
        "Toggle auto-writing a SKILL.md after hard tasks",
    ),
    ("/bg", "Run a query in the background, non-blocking"),
    ("/cd", "Change active workspace directory"),
    (
        "/cron",
        "Create/list/remove/enable/disable scheduled agent tasks",
    ),
    ("/link", "Link a folder chat can auto-write notes into"),
    ("/clear", "Clear conversation history"),
    ("/code", "Run in code-agent mode"),
    ("/edit-image", "Edit attached image with prompt instruction"),
    ("/exit", "Exit Mint CLI"),
    ("/fast", "Toggle fast mode (hide thinking traces)"),
    (
        "/plan",
        "Toggle plan mode (read-only until you approve a plan)",
    ),
    ("/gen-image", "Generate image using AI model"),
    ("/generate-image", "Generate image using AI model"),
    ("/help", "Show help menu"),
    ("/image", "Attach image from disk"),
    (
        "/image-provider",
        "List image gen providers or switch default provider",
    ),
    ("/jobs", "List, inspect, or cancel background jobs"),
    ("/learn", "Import persistent skill/instruction"),
    ("/mcp", "List configured MCP servers"),
    ("/memory", "Manage long-term memory store"),
    ("/models", "List AI providers or switch active provider"),
    ("/multi-agent", "Toggle Multi-Agent Collaboration system"),
    ("/paste", "Attach image from clipboard"),
    ("/plugins", "List or generate plugins/skills"),
    (
        "/release-notes",
        "Show release notes for the current version",
    ),
    (
        "/search-provider",
        "List web search providers or switch default provider",
    ),
    (
        "/shells",
        "List, inspect, or kill background shell jobs run_shell started",
    ),
    ("/skill add", "Add or install global skill file or folder"),
    ("/stats", "Show session statistics"),
    ("/veo", "Generate video using Google Veo"),
    (
        "/video-provider",
        "List video gen providers or switch default provider",
    ),
];

/// Width, in raw characters, available for input text within the box —
/// shared by every function that needs to reason about row layout, so the
/// terminal-width query and margin/prefix math stay in one place.
fn input_content_width() -> usize {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = term_width as usize;
    let prefix_len = "› ".chars().count();
    width.saturating_sub(2).saturating_sub(prefix_len).max(1)
}

/// Splits `input_chars` into visual rows — breaking both at `row_width`
/// characters (character-count, not visual-width, matching this module's
/// existing horizontal-scroll convention) and at any explicit `\n` the user
/// inserted (Alt+Enter) — and reports which row/column the cursor falls in.
/// A `\n` itself is consumed by the break and never appears in a row's text.
fn wrap_input_into_rows(
    input_chars: &[char],
    row_width: usize,
    cursor_pos: usize,
) -> (Vec<String>, usize, usize) {
    let row_width = row_width.max(1);
    if input_chars.is_empty() {
        return (vec![String::new()], 0, 0);
    }

    let mut rows: Vec<String> = Vec::new();
    let mut row_starts: Vec<usize> = Vec::new();
    let mut row_start = 0usize;
    let mut row_len = 0usize;

    for (i, &c) in input_chars.iter().enumerate() {
        if c == '\n' {
            rows.push(input_chars[row_start..i].iter().collect());
            row_starts.push(row_start);
            row_start = i + 1;
            row_len = 0;
            continue;
        }
        row_len += 1;
        if row_len == row_width {
            rows.push(input_chars[row_start..=i].iter().collect());
            row_starts.push(row_start);
            row_start = i + 1;
            row_len = 0;
        }
    }
    // Only a trailing `\n` should conjure up a fresh empty row after it —
    // reaching exactly `row_width` with no more input left should NOT (matches
    // this box's prior single-row behavior: the cursor just sits at the edge
    // of the full row until another character is actually typed).
    if row_start < input_chars.len() || input_chars[input_chars.len() - 1] == '\n' {
        rows.push(input_chars[row_start..].iter().collect());
        row_starts.push(row_start);
    }

    let cursor_pos = cursor_pos.min(input_chars.len());
    let mut cursor_row = rows.len() - 1;
    for (idx, &start) in row_starts.iter().enumerate() {
        let end = start + rows[idx].chars().count();
        if cursor_pos <= end {
            cursor_row = idx;
            break;
        }
    }
    let cursor_col = cursor_pos - row_starts[cursor_row];

    (rows, cursor_row, cursor_col)
}

/// Visual (1-indexed) terminal column for the cursor within its current row —
/// column 4 is the first content character (columns 1-3 are the leading
/// margin space plus the 2-visual-width `"› "`/`"  "` prefix).
fn cursor_visual_column(input_chars: &[char], cursor_pos: usize, content_width: usize) -> usize {
    let (_, _, col) = wrap_input_into_rows(input_chars, content_width, cursor_pos);
    let cursor_pos = cursor_pos.min(input_chars.len());
    let row_start = cursor_pos - col;
    let visual: usize = input_chars[row_start..cursor_pos]
        .iter()
        .copied()
        .map(char_visual_width)
        .sum();
    4 + visual
}

pub fn draw_input_box(
    input_chars: &[char],
    cursor_pos: usize,
    placeholder: &str,
    model: &str,
    path_str: &str,
    tab_base_input: Option<&str>,
    tab_index: Option<usize>,
    current_dir: &Path,
) -> usize {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = term_width as usize;
    let prefix = "› ";
    let cont_prefix = "  ";
    let input_width = width.saturating_sub(2);
    let content_max_len = input_content_width();
    let blank_line = " ".repeat(input_width);

    println!(" {COMPOSER_BG}{blank_line}{RESET}");

    if input_chars.is_empty() {
        let pad_len = content_max_len.saturating_sub(placeholder.chars().count());
        let padding = " ".repeat(pad_len);
        println!(
            " {COMPOSER_BG}{MINT}{prefix}{RESET}{COMPOSER_BG}{DIM}{}\x1b[39m{}{RESET}",
            placeholder, padding
        );
    } else {
        let (rows, _, _) = wrap_input_into_rows(input_chars, content_max_len, cursor_pos);
        for (i, row) in rows.iter().enumerate() {
            let row_prefix = if i == 0 { prefix } else { cont_prefix };
            let display_row = format_placeholders(row);
            let visible_len = string_visual_width(row);
            let pad_len = content_max_len.saturating_sub(visible_len);
            let padding = " ".repeat(pad_len);
            println!(
                " {COMPOSER_BG}{MINT}{}{RESET}{COMPOSER_BG}{}{}{RESET}",
                row_prefix, display_row, padding
            );
        }
    }

    println!(" {COMPOSER_BG}{blank_line}{RESET}");

    let agent_str = format!(" {DIM}[Agent]{RESET} {MINT}{}{RESET}", model);
    // Background shell jobs (run_shell(background: true)) still running —
    // shown so they're never invisible between the start and finish notice.
    let bg_running = mint_core::bg_shell::running_count();
    let jobs_prefix = if bg_running > 0 {
        format!(
            "{bg_running} bg shell{} · ",
            if bg_running == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };
    // A `\0`-prefixed path_str is a status override (e.g. history browsing),
    // rendered as-is instead of the usual "path: ..." label.
    let path_rest = match path_str.strip_prefix('\0') {
        Some(status) => status.to_string(),
        None => format!("path: {}", path_str),
    };
    let agent_visible_len = " [Agent] ".len() + model.chars().count();
    let path_visible_len = jobs_prefix.chars().count() + path_rest.chars().count();

    let status_pad_len = (width - 1).saturating_sub(agent_visible_len + path_visible_len);
    let status_padding = " ".repeat(status_pad_len);

    // The job-count prefix gets its own BLUE so it stands out from the DIM
    // path text next to it, instead of blending into the status line.
    let colored_jobs_prefix = if bg_running > 0 {
        format!("{BLUE}{jobs_prefix}{RESET}")
    } else {
        String::new()
    };

    print!(
        "{}{}{}{}{}{}",
        agent_str, status_padding, colored_jobs_prefix, DIM, path_rest, RESET
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
            let s_start_idx = current_page * 5;
            let s_end_idx = std::cmp::min(s_start_idx + 5, matches.len());
            match_count = s_end_idx - s_start_idx;

            println!();
            println!(
                " {BLUE}Suggestions ({}/{}){RESET}",
                current_page + 1,
                total_pages
            );
            for i in s_start_idx..s_end_idx {
                let (cmd, desc) = matches[i];
                if Some(i) == highlight_idx {
                    println!("  {BLUE}▶ {:<16}{RESET} {DIM}- {}{RESET}", cmd, desc);
                } else {
                    println!("    {DIM}{:<16} - {}{RESET}", cmd, desc);
                }
            }
        }
    } else if search_query.starts_with('$') {
        // Parse skill name (excluding arguments)
        let skill_query = search_query
            .split_whitespace()
            .next()
            .unwrap_or(search_query);
        let prefix = &skill_query[1..].to_lowercase();
        let skills = load_all_available_skills(current_dir);
        let matches: Vec<_> = skills
            .iter()
            .filter(|skill| skill.name.to_lowercase().starts_with(prefix))
            .collect();

        if !matches.is_empty() {
            let total_pages = matches.len().div_ceil(5);
            let highlight_idx = tab_index.map(|idx| idx % matches.len());
            let selected_idx = highlight_idx.unwrap_or(0);
            let current_page = selected_idx / 5;
            let s_start_idx = current_page * 5;
            let s_end_idx = std::cmp::min(s_start_idx + 5, matches.len());
            match_count = s_end_idx - s_start_idx;

            println!();
            println!(
                " {BLUE}Suggestions ({}/{}){RESET}",
                current_page + 1,
                total_pages
            );
            for i in s_start_idx..s_end_idx {
                let skill = matches[i];
                let desc = skill
                    .description
                    .as_deref()
                    .unwrap_or("No description provided");
                let max_desc_len = width.saturating_sub(35);
                let truncated_desc = if desc.chars().count() > max_desc_len {
                    let mut s: String = desc.chars().take(max_desc_len.saturating_sub(3)).collect();
                    s.push_str("...");
                    s
                } else {
                    desc.to_string()
                };

                if Some(i) == highlight_idx {
                    println!(
                        "  {BLUE}▶ ${:<20}{RESET} {MINT}[Skill]{RESET} {DIM}{}{RESET}",
                        skill.name, truncated_desc
                    );
                } else {
                    println!(
                        "    {DIM}${:<20} [Skill] {}{RESET}",
                        skill.name, truncated_desc
                    );
                }
            }
        }
    }

    match_count
}

/// Moves the terminal cursor onto the correct wrapped input row/column and
/// returns that row, 1-indexed from the box's top blank line — callers stash
/// this so a later `clear_input_box` knows how far up to travel regardless of
/// how many rows the box currently spans.
pub fn position_input_cursor(input_chars: &[char], cursor_pos: usize, match_count: usize) -> usize {
    let content_width = input_content_width();
    let (rows, cursor_row_idx, _) = wrap_input_into_rows(input_chars, content_width, cursor_pos);
    let row_count = rows.len();
    let row = cursor_row_idx + 1;

    let base = if match_count > 0 { 2 + match_count } else { 0 };
    let up_lines = base + 2 + (row_count - row);
    print!(
        "\x1b[{}A\x1b[{}G",
        up_lines,
        cursor_visual_column(input_chars, cursor_pos, content_width)
    );
    row
}

/// `cursor_row` is the value last returned by `position_input_cursor` — how
/// many lines up from the terminal's current cursor position the top of the
/// box sits. `\x1b[J` (erase to end of screen) then wipes the whole box
/// regardless of whether it's growing or shrinking on this redraw.
pub fn clear_input_box(cursor_row: usize) {
    print!("\r\x1b[{}A\x1b[J", cursor_row.max(1));
}

pub fn redraw_input_box(
    input_chars: &[char],
    cursor_pos: usize,
    placeholder: &str,
    model: &str,
    path_str: &str,
    tab_base_input: Option<&str>,
    tab_index: Option<usize>,
    current_dir: &Path,
    cursor_row: &mut usize,
) {
    clear_input_box(*cursor_row);
    let match_count = draw_input_box(
        input_chars,
        cursor_pos,
        placeholder,
        model,
        path_str,
        tab_base_input,
        tab_index,
        current_dir,
    );
    *cursor_row = position_input_cursor(input_chars, cursor_pos, match_count);
    let _ = io::stdout().flush();
}

pub fn read_line_interactive(
    _provider: &str,
    model: &str,
    path_str: &str,
    current_dir: &Path,
    history: &[String],
    jobs: &BackgroundJobs,
) -> Result<Option<InteractiveInput>> {
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            return Ok(None);
        }
        return Ok(Some(InteractiveInput {
            text: trimmed,
            pasted_image: None,
        }));
    }

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

    // Track input-history browsing state (Up/Down over previously submitted lines).
    // `history_index` counts back from the most recent entry: Some(0) is the
    // newest, Some(len-1) the oldest. `tab_base_input` (above) doubles as the
    // saved in-progress draft to restore when navigating back past the newest entry.
    let mut history_index: Option<usize> = None;

    let match_count = draw_input_box(
        &input_chars,
        cursor_pos,
        placeholder,
        model,
        path_str,
        None,
        None,
        current_dir,
    );
    let mut cursor_row = position_input_cursor(&input_chars, cursor_pos, match_count);
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
                    current_dir,
                    &mut cursor_row,
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

                if !is_ctrl_d {
                    ctrl_d_pressed = false;
                }

                // Reset tab autocomplete / history-browsing state if any key other
                // than Tab, Up, or Down is pressed
                if key_event.code != KeyCode::Tab
                    && key_event.code != KeyCode::Up
                    && key_event.code != KeyCode::Down
                {
                    tab_base_input = None;
                    tab_index = None;
                    history_index = None;
                }

                match key_event.code {
                    KeyCode::Char('c')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        input_chars.clear();
                        cursor_pos = 0;
                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Char('d')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if ctrl_d_pressed {
                            disable_raw_mode()?;
                            clear_input_box(cursor_row);
                            let _ = io::stdout().flush();
                            break Some(InteractiveInput {
                                text: "/exit".to_string(),
                                pasted_image: None,
                            });
                        } else {
                            ctrl_d_pressed = true;
                            disable_raw_mode()?;
                            let content_width = input_content_width();
                            let (rows, cursor_row_idx, _) =
                                wrap_input_into_rows(&input_chars, content_width, cursor_pos);
                            let down_lines = rows.len() - cursor_row_idx + 2;
                            print!(
                                "\r\x1b[{down_lines}B\r\x1b[2K{WARN}Press Ctrl+D again to exit{RESET}\x1b[{down_lines}A"
                            );
                            print!(
                                "\x1b[{}G",
                                cursor_visual_column(&input_chars, cursor_pos, content_width)
                            );
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
                                current_dir,
                                &mut cursor_row,
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
                            current_dir,
                            &mut cursor_row,
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
                            current_dir,
                            &mut cursor_row,
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
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if base.starts_with('$') {
                            let skill_query = base.split_whitespace().next().unwrap_or(&base);
                            let prefix = &skill_query[1..].to_lowercase();
                            let skills = load_all_available_skills(current_dir);
                            let matches: Vec<_> = skills
                                .iter()
                                .filter(|s| s.name.to_lowercase().starts_with(prefix))
                                .collect();

                            if !matches.is_empty() {
                                let idx = tab_index.unwrap_or(0) % matches.len();
                                let completed = format!("${} ", matches[idx].name);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

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
                                    current_dir,
                                    &mut cursor_row,
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
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if base.starts_with('$') {
                            let skill_query = base.split_whitespace().next().unwrap_or(&base);
                            let prefix = &skill_query[1..].to_lowercase();
                            let skills = load_all_available_skills(current_dir);
                            let matches: Vec<_> = skills
                                .iter()
                                .filter(|s| s.name.to_lowercase().starts_with(prefix))
                                .collect();

                            if !matches.is_empty() {
                                let new_idx = match tab_index {
                                    Some(idx) => (idx + 1) % matches.len(),
                                    None => 0,
                                };
                                tab_index = Some(new_idx);
                                let completed = format!("${} ", matches[new_idx].name);
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
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if let Some(idx) = history_index {
                            // Down moves toward more recent entries, then back to the draft.
                            let status_path;
                            if idx == 0 {
                                history_index = None;
                                input_chars = base.chars().collect();
                                status_path = path_str.to_string();
                            } else {
                                let new_idx = idx - 1;
                                history_index = Some(new_idx);
                                input_chars =
                                    history[history.len() - 1 - new_idx].chars().collect();
                                status_path =
                                    format!("\0History {}/{}", new_idx + 1, history.len());
                            }
                            cursor_pos = input_chars.len();

                            disable_raw_mode()?;
                            redraw_input_box(
                                &input_chars,
                                cursor_pos,
                                placeholder,
                                model,
                                &status_path,
                                None,
                                None,
                                current_dir,
                                &mut cursor_row,
                            );
                            enable_raw_mode()?;
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
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if base.starts_with('$') {
                            let skill_query = base.split_whitespace().next().unwrap_or(&base);
                            let prefix = &skill_query[1..].to_lowercase();
                            let skills = load_all_available_skills(current_dir);
                            let matches: Vec<_> = skills
                                .iter()
                                .filter(|s| s.name.to_lowercase().starts_with(prefix))
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
                                let completed = format!("${} ", matches[new_idx].name);
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
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if !history.is_empty() {
                            // Up moves toward older entries, saving the in-progress
                            // draft (`base`) the first time history browsing starts.
                            let new_idx = match history_index {
                                Some(idx) if idx + 1 < history.len() => idx + 1,
                                Some(idx) => idx,
                                None => 0,
                            };
                            history_index = Some(new_idx);
                            input_chars = history[history.len() - 1 - new_idx].chars().collect();
                            cursor_pos = input_chars.len();
                            let status_path =
                                format!("\0History {}/{}", new_idx + 1, history.len());

                            disable_raw_mode()?;
                            redraw_input_box(
                                &input_chars,
                                cursor_pos,
                                placeholder,
                                model,
                                &status_path,
                                None,
                                None,
                                current_dir,
                                &mut cursor_row,
                            );
                            enable_raw_mode()?;
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
                            current_dir,
                            &mut cursor_row,
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
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Enter
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::ALT) =>
                    {
                        // Alt+Enter inserts a literal newline without submitting —
                        // Enter alone still submits. Backspace/wrap/echo all treat
                        // '\n' as just another character already, so no other
                        // handler needs to change to support this.
                        input_chars.insert(cursor_pos, '\n');
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
                            current_dir,
                            &mut cursor_row,
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
                        clear_input_box(cursor_row);
                        let input_str: String = input_chars.iter().collect();

                        let mut expanded_str = input_str.clone();
                        for (placeholder_str, content) in &paste_contents {
                            expanded_str = expanded_str.replace(placeholder_str, content);
                        }

                        let lines: Vec<&str> = expanded_str.lines().collect();
                        if lines.len() <= 1 {
                            println!("  {BLUE}You ›{RESET} {}", expanded_str);
                        } else {
                            for (idx, line) in lines.iter().enumerate() {
                                if idx == 0 {
                                    println!("  {BLUE}You ›{RESET} {}", line);
                                } else {
                                    println!("        {}", line);
                                }
                            }
                        }
                        let _ = io::stdout().flush();

                        break Some(InteractiveInput {
                            text: expanded_str,
                            pasted_image,
                        });
                    }
                    KeyCode::Esc => {
                        disable_raw_mode()?;
                        clear_input_box(cursor_row);
                        let _ = io::stdout().flush();
                        break None;
                    }
                    _ => {}
                }
            }
        } else {
            // Idle tick (no key event within the poll window) — a good place
            // to surface any /bg job, a background shell job (started via
            // run_shell(background: true)), or a linked-folder note write,
            // that finished while the user was typing elsewhere, without
            // disturbing whatever they're mid-edit on.
            let mut notices = jobs.take_notices();
            notices.extend(mint_core::bg_shell::take_finished_notices());
            notices.extend(mint_core::take_linked_folder_notices());
            if !notices.is_empty() {
                disable_raw_mode()?;
                clear_input_box(cursor_row);
                for notice in &notices {
                    println!(" {DIM}{}{RESET}", notice);
                }
                let match_count = draw_input_box(
                    &input_chars,
                    cursor_pos,
                    placeholder,
                    model,
                    path_str,
                    tab_base_input.as_deref(),
                    tab_index,
                    current_dir,
                );
                cursor_row = position_input_cursor(&input_chars, cursor_pos, match_count);
                let _ = io::stdout().flush();
                enable_raw_mode()?;
            }
        }
    };

    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableBracketedPaste);
    Ok(result)
}

pub fn insert_image_placeholder(input_chars: &mut Vec<char>, cursor_pos: &mut usize) {
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

/// Renders a stored UTC RFC3339 timestamp (e.g. a cron job's `next_run`) in
/// the local system timezone with an explicit offset, so `/cron list` shown
/// in a terminal reads the same "when" a human means as the web/desktop
/// app's already-localized display, instead of a raw UTC instant that looks
/// like a different time even though it isn't. Falls back to the raw string
/// if it doesn't parse as RFC3339 (defensive only — cron timestamps are
/// always written by `CronStore` in that format).
fn format_local_time(rfc3339: &str) -> String {
    match chrono::DateTime::parse_from_rfc3339(rfc3339) {
        Ok(utc_time) => utc_time
            .with_timezone(&chrono::Local)
            .format("%Y-%m-%d %H:%M %:z")
            .to_string(),
        Err(_) => rfc3339.to_string(),
    }
}

pub fn format_path_with_tilde(path: &Path) -> String {
    let path_str = path.to_string_lossy().to_string();
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy().to_string();
        if path_str.starts_with(&home_str) {
            return path_str.replacen(&home_str, "~", 1);
        }
    }
    path_str
}

pub fn format_placeholders(s: &str) -> String {
    let mut result = String::new();
    let chars = s.chars().collect::<Vec<_>>();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '[' && i + 13 < chars.len() {
            let slice: String = chars[i..i + 14].iter().collect();
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

pub fn is_thai_zero_width(c: char) -> bool {
    let cp = c as u32;
    cp == 0x0E31 || (0x0E34..=0x0E3A).contains(&cp) || (0x0E47..=0x0E4E).contains(&cp)
}

pub fn char_visual_width(c: char) -> usize {
    if is_thai_zero_width(c) { 0 } else { 1 }
}

pub fn string_visual_width(s: &str) -> usize {
    s.chars().map(char_visual_width).sum()
}

pub fn active_model<'a>(provider: &str, config: &'a mint_core::MintConfig) -> &'a str {
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

pub fn confirm(prompt: &str) -> Result<bool> {
    confirm_scoped(prompt, &SESSION_APPROVED)
}

/// Same picker as `confirm`, but scoped to the agent's "Security
/// Authorization" prompts via `SECURITY_SESSION_APPROVED` instead of the
/// general-purpose `SESSION_APPROVED` flag — see that flag's doc comment.
pub fn confirm_security(prompt: &str) -> Result<bool> {
    confirm_scoped(prompt, &SECURITY_SESSION_APPROVED)
}

fn confirm_scoped(
    prompt: &str,
    session_flag: &'static std::sync::atomic::AtomicBool,
) -> Result<bool> {
    let clean_prompt = prompt
        .replace(" [y/N] ", "")
        .replace(" [y/N]", "")
        .trim()
        .to_string();

    if session_flag.load(std::sync::atomic::Ordering::Relaxed) {
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
    println!("  \x1b[1;97m{}\x1b[0m", clean_prompt);

    let options_with_desc = [
        ("Approve (Once)", "Allow single execution"),
        (
            "Approve (Entire Session)",
            "Auto-approve throughout session",
        ),
        ("Deny", "Cancel action"),
    ];
    let mut selected = 0;

    let print_choices = |selected: usize| -> Result<()> {
        for (i, (opt, desc)) in options_with_desc.iter().enumerate() {
            if i == selected {
                println!(
                    "  {BLUE}❯ {}. {:<24}{RESET} {DIM}- {}{RESET}",
                    i + 1,
                    opt,
                    desc
                );
            } else {
                println!("    {DIM}{}. {:<24} - {}{RESET}", i + 1, opt, desc);
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
                                    selected = options_with_desc.len() - 1;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options_with_desc.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Down => {
                                if selected < options_with_desc.len() - 1 {
                                    selected += 1;
                                } else {
                                    selected = 0;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options_with_desc.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Tab => {
                                if selected < options_with_desc.len() - 1 {
                                    selected += 1;
                                } else {
                                    selected = 0;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options_with_desc.len());
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
    print!("\x1b[{}A\x1b[J", options_with_desc.len() + 1);

    let result_str = match choice {
        0 => format!("{MINT}Approve (Once){RESET}"),
        1 => format!("{MINT}Approve (Entire Session){RESET}"),
        _ => format!("{ERROR}Deny{RESET}"),
    };
    println!("  {} {}", clean_prompt, result_str);
    let _ = io::stdout().flush();

    match choice {
        0 => Ok(true),
        1 => {
            session_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(true)
        }
        _ => Ok(false),
    }
}

/// Known model choices for a given provider, used by `/models` to offer a
/// second picker after the provider is chosen. Empty means no known list
/// (e.g. `local_openai`), so `/models` skips straight to that provider's
/// current/default model instead of showing an empty picker.
fn model_options_for_provider(config: &mint_core::MintConfig, provider: &str) -> Vec<String> {
    match provider {
        "gemini" => onboard::GEMINI_MODEL_PRESETS,
        "anthropic" => onboard::ANTHROPIC_MODEL_PRESETS,
        "openai" => onboard::OPENAI_MODEL_PRESETS,
        "openrouter" => onboard::OPENROUTER_MODEL_PRESETS,
        "deepseek" => onboard::DEEPSEEK_MODEL_PRESETS,
        "huggingface" => onboard::HUGGINGFACE_MODEL_PRESETS,
        "ollama" => return onboard::installed_ollama_models(),
        p if p.starts_with("custom:") => {
            return config
                .resolve_custom_provider(p)
                .map(|cp| cp.models.iter().map(|m| m.model_id.clone()).collect())
                .unwrap_or_default();
        }
        _ => &[],
    }
    .iter()
    .map(|s| s.to_string())
    .collect()
}

pub fn prompt_interactive_select(
    title: &str,
    options: &[String],
    current_selection: &str,
) -> Result<Option<String>> {
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() || enable_raw_mode().is_err() {
        return Ok(None);
    }

    let _ = disable_raw_mode();
    println!(
        "  {BLUE}{} (Use ↑/↓ to navigate, Enter to select, Esc to cancel):{RESET}",
        title
    );

    let mut selected = options
        .iter()
        .position(|p| p == current_selection)
        .unwrap_or(0);

    let print_choices = |selected: usize| -> Result<()> {
        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                println!("  {BLUE}❯ {}{RESET}", opt);
            } else {
                println!("    {DIM}{}{RESET}", opt);
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
                            break None;
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
                            KeyCode::Down | KeyCode::Tab => {
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
                            KeyCode::Enter => {
                                break Some(selected);
                            }
                            KeyCode::Esc => {
                                break None;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    break None;
                }
            },
            Ok(false) => {}
            Err(_) => {
                break None;
            }
        }
    };

    let _ = disable_raw_mode();
    print!("\x1b[{}A\x1b[J", options.len() + 1);
    let _ = io::stdout().flush();

    match choice {
        Some(idx) => Ok(Some(options[idx].clone())),
        None => Ok(None),
    }
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

    #[test]
    fn empty_input_wraps_to_a_single_empty_row() {
        let (rows, row, col) = wrap_input_into_rows(&[], 10, 0);
        assert_eq!(rows, vec![String::new()]);
        assert_eq!((row, col), (0, 0));
    }

    #[test]
    fn short_input_stays_on_one_row() {
        let chars: Vec<char> = "hello".chars().collect();
        let (rows, row, col) = wrap_input_into_rows(&chars, 10, 3);
        assert_eq!(rows, vec!["hello".to_string()]);
        assert_eq!((row, col), (0, 3));
    }

    #[test]
    fn input_longer_than_row_width_splits_into_multiple_rows() {
        let chars: Vec<char> = "0123456789ABCDE".chars().collect(); // 15 chars
        let (rows, _, _) = wrap_input_into_rows(&chars, 10, 0);
        assert_eq!(rows, vec!["0123456789".to_string(), "ABCDE".to_string()]);
    }

    #[test]
    fn cursor_in_second_row_reports_correct_row_and_column() {
        let chars: Vec<char> = "0123456789ABCDE".chars().collect();
        let (_, row, col) = wrap_input_into_rows(&chars, 10, 12);
        assert_eq!((row, col), (1, 2));
    }

    #[test]
    fn cursor_at_exact_row_boundary_clamps_to_the_last_existing_row() {
        // 10 chars with row_width 10 makes exactly one full row; a cursor at
        // the end (pos 10) must not index a nonexistent second row.
        let chars: Vec<char> = "0123456789".chars().collect();
        let (rows, row, col) = wrap_input_into_rows(&chars, 10, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!((row, col), (0, 10));
    }

    #[test]
    fn up_lines_for_a_single_row_matches_the_original_hardcoded_constants() {
        // Before wrapping existed this was hard-coded to 2 (no suggestions)
        // or 4+match_count (with suggestions); the generalized formula in
        // `position_input_cursor` must reduce to exactly those values when
        // there's only one input row.
        let chars: Vec<char> = "hi".chars().collect();
        let content_width = 50;
        let (rows, row_idx, _) = wrap_input_into_rows(&chars, content_width, 2);
        let row = row_idx + 1;
        let row_count = rows.len();

        let up_lines_no_suggestions = 0 + 2 + (row_count - row);
        assert_eq!(up_lines_no_suggestions, 2);

        let match_count = 3;
        let base = 2 + match_count;
        let up_lines_with_suggestions = base + 2 + (row_count - row);
        assert_eq!(up_lines_with_suggestions, 4 + match_count);
    }

    #[test]
    fn explicit_newline_forces_a_row_break_even_under_the_width_limit() {
        let chars: Vec<char> = "ab\ncd".chars().collect();
        let (rows, _, _) = wrap_input_into_rows(&chars, 10, 0);
        assert_eq!(rows, vec!["ab".to_string(), "cd".to_string()]);
    }

    #[test]
    fn cursor_right_before_a_newline_stays_on_the_preceding_row() {
        let chars: Vec<char> = "ab\ncd".chars().collect(); // indices: a=0 b=1 \n=2 c=3 d=4
        let (_, row, col) = wrap_input_into_rows(&chars, 10, 2);
        assert_eq!((row, col), (0, 2));
    }

    #[test]
    fn cursor_right_after_a_newline_starts_the_next_row() {
        let chars: Vec<char> = "ab\ncd".chars().collect();
        let (_, row, col) = wrap_input_into_rows(&chars, 10, 3);
        assert_eq!((row, col), (1, 0));
    }

    #[test]
    fn a_long_logical_line_still_wraps_by_width_between_newlines() {
        let chars: Vec<char> = "0123456789ABCDE\nxyz".chars().collect();
        let (rows, _, _) = wrap_input_into_rows(&chars, 10, 0);
        assert_eq!(
            rows,
            vec![
                "0123456789".to_string(),
                "ABCDE".to_string(),
                "xyz".to_string()
            ]
        );
    }

    #[test]
    fn trailing_newline_produces_an_extra_empty_row() {
        let chars: Vec<char> = "hi\n".chars().collect();
        let (rows, row, col) = wrap_input_into_rows(&chars, 10, 3);
        assert_eq!(rows, vec!["hi".to_string(), String::new()]);
        assert_eq!((row, col), (1, 0));
    }

    #[test]
    fn cursor_visual_column_accounts_for_row_start_after_a_newline() {
        let chars: Vec<char> = "hello\nworld".chars().collect();
        let content_width = 20;
        // cursor after "wor" on the second row (index 5 for '\n' + 1 + 3 = 9)
        let col = cursor_visual_column(&chars, 9, content_width);
        assert_eq!(col, 4 + 3);
    }
}

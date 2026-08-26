use crate::background::{BackgroundJobs, JobStatus};
use crate::onboard;
use crate::{BLUE, DIM, ERROR, MINT, RESET, WARN};
use crate::{agent, image};
use anyhow::Result;
use mint_core::{CHAT_CLI_ID, MemoryStore, MintConfig};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

mod commands;
mod confirm;
mod format;
mod input_box;
mod picker;
mod slash_commands;

pub use commands::*;
pub use confirm::*;
pub use format::*;
pub use input_box::*;
pub use picker::*;
pub use slash_commands::*;

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

    let (term_width, _) = crate::markdown::terminal_size_or_default();
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

    // Notifies this prompt loop when web/desktop writes a message into the
    // same, workspace-scoped "cli" conversation while this terminal is open
    // — see live_sync's module docs for why this is DB polling rather than
    // a server push.
    mint_core::live_sync::start_live_sync_poller(mint_core::scoped_chat_id(
        mint_core::CHAT_CLI_ID,
        Some(&current_dir.to_string_lossy()),
    ));

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

    // Follow-up messages typed while an agent turn is still running (into the
    // queueing box `agent::run_code_agent_with_options` keeps on screen) come
    // back out via `run_code_agent_with_saved_image`'s return value. They're
    // queued here and drained before prompting for new input, so a message
    // typed mid-turn is dispatched automatically instead of being lost.
    let mut pending_inputs: std::collections::VecDeque<String> = std::collections::VecDeque::new();
    // The box's in-progress, not-yet-submitted text at the moment the previous
    // turn ended (see `run_code_agent_with_saved_image`'s `draft_out`) —
    // reappears as the next `read_line_interactive` call's starting text
    // instead of being discarded. `None` once consumed until a new turn
    // leaves something unsent behind again.
    let mut pending_draft: Option<String> = None;

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

        let query_str = if let Some(queued) = pending_inputs.pop_front() {
            let (term_width, _) = crate::markdown::terminal_size_or_default();
            let echo_divider = format!(
                "{DIM}{}{RESET}",
                "─".repeat((term_width as usize).saturating_sub(2))
            );
            println!("{echo_divider}");
            println!("  {BLUE}You ›{RESET} {}", queued);
            println!("{echo_divider}");
            queued
        } else if let Some(input) = read_line_interactive(
            &session.config.ai_provider,
            &model_str,
            &path_str,
            &session.current_dir,
            &session.history,
            &session.jobs,
            session.plan_mode,
            pending_draft.take().unwrap_or_default().as_str(),
        )? {
            if let Some(uri) = input.pasted_image {
                if let Some(ref mut current) = session.pending_image {
                    current.push(' ');
                    current.push_str(&uri);
                } else {
                    session.pending_image = Some(uri);
                }
            }
            let text = input.text.trim().to_owned();
            if text.is_empty() {
                continue;
            }
            text
        } else {
            print_exit_message(&session);
            break;
        };

        if session.history.last().map(|s| s.as_str()) != Some(query_str.as_str()) {
            session.history.push(query_str.clone());
        }

        // An `@servername` mention anywhere in the query (picked from the
        // composer's `@` suggestions, or hand-typed) restricts this turn's
        // mcp_tool/mcp_list_tools calls to that one configured server —
        // mirrors the GUI composer's `@` mention picker.
        let pinned_mcp_server = mint_core::list_mcp_servers().ok().and_then(|servers| {
            query_str
                .split_whitespace()
                .find_map(|word| word.strip_prefix('@'))
                .filter(|name| servers.contains_key(*name))
                .map(str::to_owned)
        });

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
                    match crate::run_code_agent_with_saved_image(
                        &task_with_skill,
                        &session.current_dir,
                        &session.config,
                        session.pending_image.take(),
                        None,
                        agent::AgentOptions {
                            fast_mode: session.fast_mode,
                            plan_mode: session.plan_mode,
                            queueing: true,
                            pinned_mcp_server: pinned_mcp_server.clone(),
                        },
                    )
                    .await
                    {
                        Ok((queued, draft)) => {
                            pending_inputs.extend(queued);
                            pending_draft = draft;
                        }
                        Err(error) => println!("{ERROR}Error:{RESET} {error}\n"),
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
                match crate::run_code_agent_with_saved_image(
                    &task,
                    &session.current_dir,
                    &session.config,
                    session.pending_image.take(),
                    None,
                    agent::AgentOptions {
                        fast_mode: session.fast_mode,
                        plan_mode: session.plan_mode,
                        queueing: true,
                        pinned_mcp_server: pinned_mcp_server.clone(),
                    },
                )
                .await
                {
                    Ok((queued, draft)) => {
                        pending_inputs.extend(queued);
                        pending_draft = draft;
                    }
                    Err(error) => println!("{ERROR}Error:{RESET} {error}\n"),
                }
                continue;
            }
            None => {} // Not a slash command, fall through
        }

        // Regular agent loop (handles both chat and coding).
        // Note: /code is fully handled by handle_slash_command above
        // (its "/code" arm always returns Some(...)), so no separate
        // "/code " fallback is needed here.
        match crate::run_code_agent_with_saved_image(
            &query_str,
            &session.current_dir,
            &session.config,
            session.pending_image.take(),
            None,
            agent::AgentOptions {
                fast_mode: session.fast_mode,
                plan_mode: session.plan_mode,
                queueing: true,
                pinned_mcp_server: pinned_mcp_server.clone(),
            },
        )
        .await
        {
            Ok((queued, draft)) => {
                pending_inputs.extend(queued);
                pending_draft = draft;
            }
            Err(error) => println!("{ERROR}Error:{RESET} {error}\n"),
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

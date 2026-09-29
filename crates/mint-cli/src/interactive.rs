use crate::background::{BackgroundJobs, JobStatus};
use crate::{BLUE, BOLD, DIM, ERROR, MINT, RESET, WARN};
use crate::{agent, image};
use anyhow::Result;
use mint_core::{CHAT_CLI_ID, MemoryStore, MintConfig};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

mod chat_tui;
pub mod command_ui;
mod commands;
mod confirm;
mod format;
mod input_box;
mod picker;
mod resume_picker;
mod slash_commands;
mod thought_viewer;

pub(crate) use chat_tui::TuiHandle;
pub(crate) use chat_tui::restore_terminal;
pub use command_ui::*;
pub use commands::*;
pub use confirm::*;
pub use format::*;
pub use input_box::*;
pub use picker::*;
pub use resume_picker::*;
pub use slash_commands::*;
pub use thought_viewer::*;

pub fn truncate_utf8(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let take_count = max_chars.saturating_sub(3).max(1);
    let prefix: String = text.chars().take(take_count).collect();
    format!("{prefix}...")
}

pub static SESSION_APPROVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Separate from `SESSION_APPROVED`: gates only the agent's high-risk
/// "Security Authorization" prompts, so approving "Entire Session" for a
/// routine shell/skill confirmation can never silently wave through an
/// unrelated, higher-stakes agent action.
pub static SECURITY_SESSION_APPROVED: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

pub struct InteractiveSession {
    pub chat_id: String,
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
    pub switch_mode: bool,
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

type FullScreenUi = (
    chat_tui::ChatTui,
    Arc<Mutex<chat_tui::ChatViewState>>,
    TuiHandle,
);

async fn run_interactive_agent_turn(
    task: String,
    session: &mut InteractiveSession,
    pinned_mcp_server: Option<String>,
    tui: &mut Option<FullScreenUi>,
) -> Result<(Vec<String>, Option<String>)> {
    let mut options = agent::AgentOptions {
        fast_mode: session.fast_mode,
        plan_mode: session.plan_mode,
        queueing: true,
        pinned_mcp_server,
        chat_id: Some(session.chat_id.clone()),
        tui: None,
    };
    let image = session.pending_image.take();
    if let Some((terminal, _, handle)) = tui.as_mut() {
        terminal.resume()?;
        options.tui = Some(handle.clone());
        let current_dir = session.current_dir.clone();
        let config = session.config.clone();
        let task = tokio::spawn(async move {
            crate::run_code_agent_with_saved_image(
                &task,
                &current_dir,
                &config,
                image,
                None,
                options,
            )
            .await
        });
        terminal.drive_agent(handle, task).await?
    } else {
        crate::run_code_agent_with_saved_image(
            &task,
            &session.current_dir,
            &session.config,
            image,
            None,
            options,
        )
        .await
    }
}

fn report_interactive_turn_error(tui: &Option<FullScreenUi>, error: &anyhow::Error) {
    if let Some((_, _, handle)) = tui {
        handle.push_notice(format!("Turn failed: {error}"));
    } else {
        print_turn_error(error);
    }
}

fn apply_welcome_gradient(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut result = String::new();
    for &c in &chars {
        if c == ' ' {
            result.push(c);
            continue;
        }
        result.push_str(&format!(
            "{}{}{}",
            crate::terminal_theme::ANSI_ACCENT,
            c,
            crate::terminal_theme::ANSI_RESET
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

/// Prompts kept for the interactive box's Up/Down recall, persisted across
/// sessions (the in-memory `InteractiveSession::history` used to start empty
/// every launch). Oldest first, capped at the last `PROMPT_HISTORY_LIMIT`,
/// stored as a JSON array next to the rest of Mint's config.
const PROMPT_HISTORY_LIMIT: usize = 100;

fn prompt_history_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("mint").join("prompt-history.json"))
}

fn trim_to_limit(list: &mut Vec<String>) {
    if list.len() > PROMPT_HISTORY_LIMIT {
        let overflow = list.len() - PROMPT_HISTORY_LIMIT;
        list.drain(0..overflow);
    }
}

fn load_prompt_history() -> Vec<String> {
    let Some(path) = prompt_history_path() else {
        return Vec::new();
    };
    let Ok(raw) = fs::read_to_string(&path) else {
        return Vec::new();
    };
    let mut list: Vec<String> = serde_json::from_str(&raw).unwrap_or_default();
    trim_to_limit(&mut list);
    list
}

/// Append `line` to `history` (skipping an immediate duplicate and the
/// pure control commands `/exit` / `/quit`), cap it, and best-effort write
/// the whole list back to disk.
fn record_prompt(history: &mut Vec<String>, line: &str) {
    if matches!(line.trim(), "/exit" | "/quit") {
        return;
    }
    if history.last().map(String::as_str) == Some(line) {
        return;
    }
    history.push(line.to_owned());
    trim_to_limit(history);

    if let Some(path) = prompt_history_path()
        && let Ok(json) = serde_json::to_string(history)
    {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let _ = fs::write(&path, json);
    }
}

fn print_turn_error(error: &anyhow::Error) {
    let err_str = error.to_string();
    if err_str.to_lowercase().contains("interrupted") {
        println!("{DIM}[Cancelled by user]{RESET}\n");
    } else {
        println!("{ERROR}Error:{RESET} {error}\n");
    }
}

pub async fn run_interactive_chat() -> Result<()> {
    run_interactive_chat_with_options(None, false, false).await
}

pub async fn run_interactive_chat_with_options(
    model_override: Option<String>,
    fast_mode: bool,
    plan_mode: bool,
) -> Result<()> {
    run_interactive_chat_with_session(model_override, fast_mode, plan_mode, None, true).await
}

pub async fn run_interactive_chat_with_session(
    model_override: Option<String>,
    fast_mode: bool,
    plan_mode: bool,
    resume_id: Option<String>,
    use_tui: bool,
) -> Result<()> {
    let mut config = mint_core::load_config()?;
    if let Some(ref m) = model_override {
        crate::apply_temporary_model_override(&mut config, m);
    }

    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    let (chat_id, prompt_resume_at_start) = if let Some(rid) = resume_id {
        if rid == "__prompt__" {
            (mint_core::generate_cli_session_id(), true)
        } else {
            let trimmed = rid.trim().to_string();
            let candidate = if !trimmed.starts_with("cli::") && trimmed != mint_core::CHAT_CLI_ID {
                format!("cli::{trimmed}")
            } else {
                trimmed.clone()
            };
            if let Ok(memory) = mint_core::MemoryStore::open_default()
                && let Ok(sessions) = memory.list_chat_sessions()
                && sessions.iter().any(|s| s.id == candidate)
            {
                (candidate, false)
            } else {
                (trimmed, false)
            }
        }
    } else {
        (mint_core::generate_cli_session_id(), false)
    };

    // Notifies this prompt loop when web/desktop writes a message into the
    // same conversation while this terminal is open
    mint_core::live_sync::start_live_sync_poller(mint_core::scoped_chat_id(
        &chat_id,
        Some(&current_dir.to_string_lossy()),
    ));

    let mut session = InteractiveSession {
        chat_id,
        config: config.clone(),
        current_dir: current_dir.clone(),
        fast_mode,
        plan_mode,
        pending_image: None,
        history: load_prompt_history(),
        jobs: BackgroundJobs::new(),
    };

    let mut tui = if use_tui {
        match chat_tui::ChatTui::enter(&session.current_dir) {
            Ok(terminal) => {
                let state = chat_tui::ChatViewState::from_session(&session);
                let state = Arc::new(Mutex::new(state));
                let handle = chat_tui::ChatTui::handle(Arc::clone(&state));
                Some((terminal, state, handle))
            }
            Err(error) => {
                println!(
                    "{WARN}Could not start full-screen TUI: {error}. Using classic mode.{RESET}\n"
                );
                None
            }
        }
    } else {
        None
    };

    if tui.is_none() {
        print_welcome_banner(&session.config);

        if let Ok(memory) = mint_core::MemoryStore::open_default() {
            let sessions = memory.list_chat_sessions().unwrap_or_default();
            if let Some(target) = sessions.iter().find(|s| s.id == session.chat_id) {
                println!(
                    "{MINT}●{RESET} Resumed session: {BOLD}{}{RESET} {DIM}({}){RESET}",
                    target.title, target.id
                );
                if let Ok(recent) = memory.get_session_preview(&session.chat_id, 2)
                    && let Some(last) = recent.first()
                {
                    let snippet = truncate_utf8(&last.user_text, 60);
                    println!("  {DIM}Last turn: {snippet}{RESET}");
                }
                println!();
            }
        }

        println!("Type naturally or /help for commands. F6: TUI. Ctrl+D exits.\n");
    }

    if prompt_resume_at_start {
        let mut ui: Box<dyn CommandUi> = match tui.as_mut() {
            Some((terminal, state, handle)) => {
                Box::new(FullScreenCommandUi::new(terminal, state, handle))
            }
            None => Box::new(ClassicCommandUi::new()),
        };
        match prompt_resume_session_picker_with_ui(
            ui.as_mut(),
            &session.current_dir,
            &session.chat_id,
        ) {
            Ok(Some(target_id)) => {
                session.chat_id = target_id.clone();
                ui.reload_transcript(&session.chat_id, &session.current_dir);
                ui.push_notice(format!("Switched to session: {target_id}"));
            }
            Ok(None) => {
                ui.push_notice("Resume cancelled. Continuing with new session.");
            }
            Err(err) => {
                ui.push_notice(format!("Failed to load sessions: {err}"));
            }
        }
    }

    let mut printed_update = false;
    if let Some((current, latest)) = crate::updater::get_cached_update_notice() {
        if let Some((_, _, handle)) = tui.as_mut() {
            handle.push_notice(crate::updater::format_tui_update_notice(&current, &latest));
        } else {
            crate::updater::print_update_notice(&current, &latest);
        }
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
                    if let Some((_, _, handle)) = tui.as_mut() {
                        handle.push_notice(crate::updater::format_tui_update_notice(
                            &current, &latest,
                        ));
                    } else {
                        crate::updater::print_update_notice(&current, &latest);
                    }
                    printed_update = true;
                }
            } else {
                update_handle = Some(handle);
            }
        }
        if let Some((terminal, state, _)) = tui.as_mut() {
            if let Ok(mut state) = state.lock() {
                state.sync_session(&session);
            }
            terminal.resume()?;
        }

        let path_str = format_workspace_with_branch(&session.current_dir);
        let model_str = active_model(&session.config.ai_provider, &session.config).to_owned();

        let query_str = if let Some(queued) = pending_inputs.pop_front() {
            if tui.is_none() {
                let (term_width, _) = crate::markdown::terminal_size_or_default();
                let echo_divider = format!(
                    "{DIM}{}{RESET}",
                    "─".repeat((term_width as usize).saturating_sub(2))
                );
                println!("{echo_divider}");
                println!("  {BLUE}You ›{RESET} {}", queued);
                println!("{echo_divider}");
            }
            queued
        } else if let Some(input) = if let Some((terminal, state, _)) = tui.as_mut() {
            terminal.read_input(state)?
        } else {
            read_line_interactive(
                &session.config.ai_provider,
                &model_str,
                &path_str,
                &session.current_dir,
                &session.history,
                &session.jobs,
                session.plan_mode,
                pending_draft.take().unwrap_or_default().as_str(),
            )?
        } {
            if let Some(uri) = input.pasted_image {
                if let Some(ref mut current) = session.pending_image {
                    current.push(' ');
                    current.push_str(&uri);
                } else {
                    session.pending_image = Some(uri);
                }
            }
            if input.switch_mode {
                let target = if tui.is_some() { "Classic CLI" } else { "TUI" };
                let choices = [
                    ChoiceItem::new("Stay in current mode", "stay"),
                    ChoiceItem::new(format!("Switch to {target}"), "switch"),
                ];
                let mut ui: Box<dyn CommandUi> = match tui.as_mut() {
                    Some((terminal, state, handle)) => {
                        Box::new(FullScreenCommandUi::new(terminal, state, handle))
                    }
                    None => Box::new(ClassicCommandUi::new()),
                };
                let confirmed = ui.prompt_choice(
                    "Switch CLI interface?",
                    &format!("Change to {target}?"),
                    &choices,
                )? == Some(1);
                drop(ui);
                if !confirmed {
                    if tui.is_none() {
                        pending_draft = Some(input.text);
                    }
                    continue;
                }
                pending_draft = Some(input.text);
                if let Some((mut terminal, _, _)) = tui.take() {
                    terminal.suspend();
                    println!("{MINT}Switched to Classic CLI. Press F6 for TUI.{RESET}");
                } else {
                    match chat_tui::ChatTui::enter(&session.current_dir) {
                        Ok(terminal) => {
                            let mut state = chat_tui::ChatViewState::from_session(&session);
                            state.set_draft(pending_draft.take().unwrap_or_default());
                            let state = Arc::new(Mutex::new(state));
                            let handle = chat_tui::ChatTui::handle(Arc::clone(&state));
                            tui = Some((terminal, state, handle));
                        }
                        Err(error) => println!("{WARN}Could not start TUI: {error}.{RESET}"),
                    }
                }
                continue;
            }
            let text = input.text.trim().to_owned();
            if text.is_empty() {
                continue;
            }
            text
        } else {
            if let Some((terminal, _, _)) = tui.as_mut() {
                terminal.suspend();
            }
            print_exit_message(&session);
            break;
        };

        if let Some((_, state, _)) = tui.as_mut()
            && let Ok(mut state) = state.lock()
        {
            state.push_user(query_str.clone());
        }

        record_prompt(&mut session.history, &query_str);

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
            let skill_opt = skills
                .iter()
                .find(|s| s.name.to_lowercase() == skill_name)
                .cloned();

            if let Some(skill) = skill_opt {
                let mut ui: Box<dyn CommandUi> = match tui.as_mut() {
                    Some((terminal, state, handle)) => {
                        Box::new(FullScreenCommandUi::new(terminal, state, handle))
                    }
                    None => Box::new(ClassicCommandUi::new()),
                };

                let description = skill.description.as_deref().unwrap_or("");
                ui.push_command_output(format!(
                    "Skill: {}\n{}\n\n{}",
                    skill.name, description, skill.content
                ));
                let approved = ui
                    .prompt_confirm(&format!("Activate skill '{}'?", skill.name), true)
                    .unwrap_or(false);

                if approved {
                    let final_task = if task_part.is_empty() {
                        let input = ui
                            .prompt_text(
                                &format!("Task for {}", skill.name),
                                "Describe what this skill should do",
                                None,
                            )
                            .ok()
                            .flatten()
                            .unwrap_or_default();
                        let input = input.trim().to_owned();
                        if input.is_empty() {
                            ui.push_notice("Cancelled: task cannot be empty");
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

                    ui.push_notice(format!("Skill({}) loaded", skill.name));
                    drop(ui);

                    match run_interactive_agent_turn(
                        task_with_skill,
                        &mut session,
                        pinned_mcp_server.clone(),
                        &mut tui,
                    )
                    .await
                    {
                        Ok((queued, draft)) => {
                            pending_inputs.extend(queued);
                            pending_draft = draft;
                        }
                        Err(error) => report_interactive_turn_error(&tui, &error),
                    }
                } else {
                    ui.push_notice("Skill activation cancelled");
                }
            } else {
                let mut ui: Box<dyn CommandUi> = match tui.as_mut() {
                    Some((terminal, state, handle)) => {
                        Box::new(FullScreenCommandUi::new(terminal, state, handle))
                    }
                    None => Box::new(ClassicCommandUi::new()),
                };
                ui.push_notice(format!("Skill '{skill_name}' not found"));
            }
            continue;
        }

        // Run slash-command router via unified CommandUi abstraction.
        let mut ui: Box<dyn CommandUi> = match tui.as_mut() {
            Some((terminal, state, handle)) => {
                Box::new(FullScreenCommandUi::new(terminal, state, handle))
            }
            None => Box::new(ClassicCommandUi::new()),
        };
        let slash_result = handle_slash_command(&mut session, ui.as_mut(), &query_str).await;
        drop(ui);

        match slash_result {
            Some(SlashResult::Handled) => continue,
            Some(SlashResult::Exit) => {
                print_exit_message(&session);
                break;
            }
            Some(SlashResult::ForwardToAgent(task)) => {
                println!();
                match run_interactive_agent_turn(
                    task,
                    &mut session,
                    pinned_mcp_server.clone(),
                    &mut tui,
                )
                .await
                {
                    Ok((queued, draft)) => {
                        pending_inputs.extend(queued);
                        pending_draft = draft;
                    }
                    Err(error) => report_interactive_turn_error(&tui, &error),
                }
                continue;
            }
            None => {} // Not a slash command, fall through
        }

        // Regular agent loop (handles both chat and coding).
        // Note: /code is fully handled by handle_slash_command above
        // (its "/code" arm always returns Some(...)), so no separate
        // "/code " fallback is needed here.
        match run_interactive_agent_turn(
            query_str,
            &mut session,
            pinned_mcp_server.clone(),
            &mut tui,
        )
        .await
        {
            Ok((queued, draft)) => {
                pending_inputs.extend(queued);
                pending_draft = draft;
            }
            Err(error) => report_interactive_turn_error(&tui, &error),
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
        format_workspace_with_branch(&session.current_dir)
    );
    let has_history = if let Ok(memory) = MemoryStore::open_default() {
        memory
            .recent_interactions_for_chat(&session.chat_id, 1)
            .map(|rows| !rows.is_empty())
            .unwrap_or(false)
    } else {
        false
    };

    if has_history {
        println!("\n{DIM}Resume this session with:{RESET}");
        println!("{BOLD}mint --resume {}{RESET}\n", session.chat_id);
    } else {
        println!();
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_utf8_multibyte_safety() {
        let thai_input = "'/home/pheem49/vscode/Project/Mint-CLI/Release_Note.md'สรุปไฟล์นี้ให้หน่อย";
        for len in 1..=thai_input.chars().count() + 5 {
            let truncated = truncate_utf8(thai_input, len);
            assert!(!truncated.is_empty());
        }
    }
}

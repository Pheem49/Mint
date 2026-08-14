use std::io::{self, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use ansi_to_tui::IntoText;
use mint_core::{
    AgentApproval, AgentProgress, AgentResult, ApprovalOutcome, CHAT_CLI_ID, MemoryStore,
    MintConfig, OrchestrationError, PermissionDecision, PermissionRule, orchestrate_agent_loop,
    permission_decision_for,
};
use syntect::easy::HighlightLines;
use syntect::highlighting::ThemeSet;
use syntect::parsing::SyntaxSet;
use syntect::util::as_24_bit_terminal_escaped;

use crate::markdown;

const RESET: &str = "\x1b[0m";
const MINT: &str = "\x1b[32m";
const GREEN: &str = "\x1b[32m";
const RED: &str = "\x1b[31m";
const BLUE: &str = "\x1b[38;2;78;201;216m";
const CYAN: &str = "\x1b[38;2;56;189;248m";
const DIM: &str = "\x1b[90m";
const BRIGHT: &str = "\x1b[1;97m";
const BG_ADD: &str = "\x1b[48;2;20;53;32m\x1b[38;2;166;226;46m";
const BG_DEL: &str = "\x1b[48;2;61;23;23m\x1b[38;2;255;121;121m";

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentOptions {
    pub fast_mode: bool,
    pub plan_mode: bool,
}

pub async fn run_code_agent(task: &str, root: &Path, config: &MintConfig) -> Result<AgentResult> {
    run_code_agent_with_image(task, root, config, None, None).await
}

pub async fn run_code_agent_with_image(
    task: &str,
    root: &Path,
    config: &MintConfig,
    image_data_uri: Option<String>,
    video_data_uri: Option<String>,
) -> Result<AgentResult> {
    run_code_agent_with_options(
        task,
        root,
        config,
        image_data_uri,
        video_data_uri,
        AgentOptions::default(),
    )
    .await
}

pub async fn run_code_agent_with_options(
    task: &str,
    root: &Path,
    config: &MintConfig,
    image_data_uri: Option<String>,
    video_data_uri: Option<String>,
    options: AgentOptions,
) -> Result<AgentResult> {
    let started_at = Instant::now();
    let thinking_verb = random_thinking_verb();
    let approval_active = Arc::new(AtomicBool::new(false));
    let agent_done = Arc::new(AtomicBool::new(false));
    let live_status = Arc::new(Mutex::new(LiveStatus::default()));
    let approve_approval_active = Arc::clone(&approval_active);
    let approve_live_status = Arc::clone(&live_status);
    // Seeded from disk, then grown in-memory as the user picks "Always allow"
    // during this run, so a rule added mid-run is honored immediately without
    // waiting for a fresh process start.
    let mut permission_rules = config.permission_rules.clone();

    let approve_cb = |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
        approve_approval_active.store(true, Ordering::Relaxed);
        if let Ok(mut status) = approve_live_status.lock() {
            clear_live_status(&mut status);
        }

        struct ApprovalGuard(Arc<AtomicBool>);
        impl Drop for ApprovalGuard {
            fn drop(&mut self) {
                self.0.store(false, Ordering::Relaxed);
            }
        }
        let _guard = ApprovalGuard(Arc::clone(&approve_approval_active));

        match approval {
            AgentApproval::WriteFile { path, diff, .. } => confirm_with_persistence(
                "write_file",
                path,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    let (additions, deletions) = diff_stats(diff);
                    print_diff_header("Create", path, additions, deletions);
                    print_colored_diff(diff);
                },
            ),
            AgentApproval::ApplyPatch { path, diff, .. } => confirm_with_persistence(
                "apply_patch",
                path,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    let (additions, deletions) = diff_stats(diff);
                    print_diff_header("Update", path, additions, deletions);
                    print_colored_diff(diff);
                },
            ),
            AgentApproval::RunShell {
                command,
                mode,
                background,
            } => confirm_with_persistence(
                "run_shell",
                command,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    print_approval_card(
                        if *background {
                            "Local Shell Command (background)"
                        } else {
                            "Local Shell Command"
                        },
                        &[("Command", command), ("Mode", mode)],
                    );
                },
            ),
            AgentApproval::NoteWrite { path, .. } => confirm_with_persistence(
                "note_write",
                path,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    print_approval_card("Note Creation", &[("Path", path)]);
                },
            ),
            AgentApproval::RunPlugin { name, instruction } => {
                // Scope the persisted rule to the exact instruction the user
                // reviewed (matching the write_file/run_shell pattern, which
                // persist on the literal path/command) so "Always allow"
                // can't be reused to auto-approve a differently-worded,
                // possibly far more dangerous instruction to the same plugin.
                let subject = format!("{name}: {instruction}");
                confirm_with_persistence(
                    "run_plugin",
                    &subject,
                    root,
                    &mut permission_rules,
                    &approve_approval_active,
                    || {
                        print_approval_card(
                            "Plugin Execution",
                            &[("Plugin", name), ("Detail", instruction)],
                        );
                    },
                )
            }
            AgentApproval::McpTool {
                server,
                tool,
                arguments,
            } => {
                // Include the arguments in the persisted subject for the same
                // reason as run_plugin above — a saved rule must not cover a
                // future call to the same tool with different arguments.
                let subject = format!("{server}:{tool}:{arguments}");
                confirm_with_persistence(
                    "mcp_tool",
                    &subject,
                    root,
                    &mut permission_rules,
                    &approve_approval_active,
                    || {
                        let mut fields = vec![("Server", server.as_str()), ("Tool", tool.as_str())];
                        let formatted_args = arguments.to_string();
                        if !formatted_args.is_empty()
                            && formatted_args != "{}"
                            && formatted_args != "null"
                        {
                            fields.push(("Arguments", &formatted_args));
                        }
                        print_approval_card("MCP Tool Call", &fields);
                    },
                )
            }
            AgentApproval::UserApproval { title, prompt } => {
                print_approval_card(
                    "Security Authorization",
                    &[("Title", title), ("Detail", prompt)],
                );
                if confirm_pausing_interrupt("Approve this request?", &approve_approval_active) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::ExitPlanMode { plan } => {
                print_approval_card("Review Plan", &[("Plan", plan)]);
                print!(
                    "  Approve this plan and allow edits? [y/N] (or type feedback to keep planning): "
                );
                let _ = std::io::Write::flush(&mut std::io::stdout());
                let mut answer = String::new();
                match std::io::stdin().read_line(&mut answer) {
                    Ok(_) => {
                        let trimmed = answer.trim();
                        if trimmed.eq_ignore_ascii_case("y") || trimmed.eq_ignore_ascii_case("yes")
                        {
                            Ok(ApprovalOutcome::Approved)
                        } else if trimmed.is_empty()
                            || trimmed.eq_ignore_ascii_case("n")
                            || trimmed.eq_ignore_ascii_case("no")
                        {
                            Ok(ApprovalOutcome::Denied)
                        } else {
                            Ok(ApprovalOutcome::Intercepted(trimmed.to_owned()))
                        }
                    }
                    Err(error) => Err(error.to_string()),
                }
            }
            AgentApproval::AskUser { question, options } => {
                if options.is_empty() {
                    print_approval_card("Agent Question", &[("Question", question)]);
                    print!("  Answer (leave empty to decline): ");
                    let _ = io::stdout().flush();
                    let mut answer = String::new();
                    match std::io::stdin().read_line(&mut answer) {
                        Ok(_) => {
                            let trimmed = answer.trim();
                            if trimmed.is_empty() {
                                Ok(ApprovalOutcome::Denied)
                            } else {
                                Ok(ApprovalOutcome::Intercepted(trimmed.to_owned()))
                            }
                        }
                        Err(error) => Err(error.to_string()),
                    }
                } else {
                    print_approval_card("Agent Question", &[("Question", question)]);
                    run_option_picker(options)
                }
            }
        }
    };
    let timer_live_status = Arc::clone(&live_status);
    let timer_agent_done = Arc::clone(&agent_done);
    let timer_approval_active = Arc::clone(&approval_active);
    let timer_started_at = started_at;
    if !options.fast_mode {
        tokio::spawn(async move {
            loop {
                if timer_agent_done.load(Ordering::Relaxed) {
                    break;
                }
                if !timer_approval_active.load(Ordering::Relaxed)
                    && let Ok(mut status) = timer_live_status.lock()
                {
                    status.thinking = Some(format!(
                        "{thinking_verb} ({} • Esc to interrupt)",
                        format_elapsed(timer_started_at.elapsed())
                    ));
                    render_live_status(&mut status);
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        });
    }
    let progress_live_status = Arc::clone(&live_status);
    let progress_approval_active = Arc::clone(&approval_active);
    let progress_cb = |progress: AgentProgress| match progress {
        AgentProgress::Thinking {
            elapsed_secs,
            agent_name,
            model_name,
        } => {
            if !options.fast_mode
                && !progress_approval_active.load(Ordering::Relaxed)
                && let Ok(mut status) = progress_live_status.lock()
            {
                let label = if let (Some(a), Some(m)) = (agent_name, model_name) {
                    format!(
                        "{} ({}) is {} ({} • Esc to interrupt)",
                        a,
                        m,
                        thinking_verb.to_lowercase(),
                        format_elapsed(Duration::from_secs(elapsed_secs))
                    )
                } else {
                    format!(
                        "{thinking_verb} ({} • Esc to interrupt)",
                        format_elapsed(Duration::from_secs(elapsed_secs))
                    )
                };
                status.thinking = Some(label);
                render_live_status(&mut status);
            }
        }
        AgentProgress::Thought { thought } => {
            if !options.fast_mode
                && !progress_approval_active.load(Ordering::Relaxed)
                && let Ok(mut status) = progress_live_status.lock()
            {
                commit_activity_snapshot(&mut status);
                print_timeline_note(&mut status, &thought);
                status.thinking = None;
                render_live_status(&mut status);
            }
        }
        AgentProgress::ToolStart { action, input } => {
            if !options.fast_mode && !progress_approval_active.load(Ordering::Relaxed) {
                if (action == "create_plan" || action == "update_plan")
                    && let Some(steps) = extract_plan_steps(&input)
                {
                    if let Ok(mut status) = progress_live_status.lock() {
                        status.thinking = None;
                        status.plan_steps = steps;
                        render_live_status(&mut status);
                    }
                    return;
                }
                if action == "read_file"
                    && let Some(path) = input.get("path").and_then(|v| v.as_str())
                    && skill_name_for_read_path(path).is_some()
                {
                    // The agent chose to read a skill file on its own initiative
                    // (as opposed to the human typing `$skillname`). Skip the
                    // generic explored-files grouping — ToolEnd below renders a
                    // dedicated Skill(...) card for this instead.
                    return;
                }
                if let Some(label) = explored_action_label(&action, &input) {
                    if let Ok(mut status) = progress_live_status.lock() {
                        status.thinking = None;
                        status.explored.push(label);
                        render_live_status(&mut status);
                    }
                    return;
                }

                let (is_activity, label) = match action.as_str() {
                    "web_search" => {
                        let query = input.get("query").and_then(|v| v.as_str()).unwrap_or("");
                        (
                            true,
                            format!("[web_search] Searching the web for \"{}\"...", query),
                        )
                    }
                    "run_shell" => {
                        let command = input.get("command").and_then(|v| v.as_str()).unwrap_or("");
                        let background = input
                            .get("background")
                            .and_then(|v| v.as_bool())
                            .unwrap_or(false);
                        (
                            false,
                            if background {
                                format!("[run_shell] Starting background command: `{}`...", command)
                            } else {
                                format!("[run_shell] Running command: `{}`...", command)
                            },
                        )
                    }
                    "shell_output" => {
                        let job_id = input.get("job_id").and_then(|v| v.as_str()).unwrap_or("");
                        (
                            false,
                            format!("[shell_output] Checking background job {}...", job_id),
                        )
                    }
                    "kill_shell" => {
                        let job_id = input.get("job_id").and_then(|v| v.as_str()).unwrap_or("");
                        (
                            false,
                            format!("[kill_shell] Stopping background job {}...", job_id),
                        )
                    }
                    "git_status" | "git_diff" | "git_log" | "git_branch" => {
                        (false, format!("[{}] Reading repository state...", action))
                    }
                    "create_plan" | "update_plan" => {
                        (false, format!("[{}] Updating task plan...", action))
                    }
                    "request_user_approval" => (
                        false,
                        "[request_user_approval] Waiting for approval...".into(),
                    ),
                    "ask_user" => (false, "[ask_user] Waiting for user answer...".into()),
                    "detect_project" => {
                        (false, "[detect_project] Detecting project type...".into())
                    }
                    "list_tests" => (false, "[list_tests] Listing tests...".into()),
                    "read_diagnostics" => {
                        (false, "[read_diagnostics] Reading diagnostics...".into())
                    }
                    "view_image" => {
                        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        (false, format!("[view_image] Reading image: {}...", path))
                    }
                    "write_file" => {
                        let path = input.get("path").and_then(|v| v.as_str()).unwrap_or("");
                        (false, format!("[write_file] Writing file: {}...", path))
                    }
                    "apply_patch" => {
                        let path = input
                            .get("patch")
                            .and_then(|p| p.get("path"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        (false, format!("[apply_patch] Patching file: {}...", path))
                    }
                    "run_plugin" => {
                        let name = input.get("name").and_then(|v| v.as_str()).unwrap_or("");
                        (false, format!("[run_plugin] Running plugin: {}...", name))
                    }
                    "mcp_tool" => {
                        let tool_name = input.get("tool").and_then(|v| v.as_str()).unwrap_or("");
                        (
                            false,
                            format!("[mcp_tool] Running MCP tool: {}...", tool_name),
                        )
                    }
                    _ => (false, format!("[{}] Using tool...", action)),
                };

                if let Ok(mut status) = progress_live_status.lock() {
                    status.thinking = None;
                    if is_activity {
                        status.activities.push(label);
                    } else {
                        status.tasks.push(label.into());
                    }
                    render_live_status(&mut status);
                }
            }
        }
        AgentProgress::ToolEnd {
            action,
            input,
            result,
        } => {
            if !options.fast_mode && !progress_approval_active.load(Ordering::Relaxed) {
                if action == "create_plan" || action == "update_plan" {
                    if let Some(steps) = extract_plan_steps(&input)
                        && let Ok(mut status) = progress_live_status.lock()
                    {
                        status.thinking = None;
                        status.plan_steps = steps;
                        render_live_status(&mut status);
                    }
                } else if command_was_run(&result)
                    && let Some(commands) = ran_command_labels(&action, &input)
                    && let Ok(mut status) = progress_live_status.lock()
                {
                    status.thinking = None;
                    let preview = command_output_preview(&result);
                    let last_index = commands.len().saturating_sub(1);
                    for (index, cmd) in commands.into_iter().enumerate() {
                        status.tasks.push(TaskEntry {
                            label: format!("Finished command: `{}`", cmd),
                            output: if index == last_index {
                                preview.clone()
                            } else {
                                Vec::new()
                            },
                        });
                    }
                    render_live_status(&mut status);
                } else if action == "read_file"
                    && let Some(path) = input.get("path").and_then(|v| v.as_str())
                    && let Some(skill_name) = skill_name_for_read_path(path)
                    && let Ok(mut status) = progress_live_status.lock()
                {
                    status.thinking = None;
                    status.tasks.push(skill_card(&skill_name));
                    render_live_status(&mut status);
                } else if action == "memory_recall" {
                    let skill_names = skill_names_from_memory_recall(&result);
                    if !skill_names.is_empty()
                        && let Ok(mut status) = progress_live_status.lock()
                    {
                        status.thinking = None;
                        for name in skill_names {
                            status.tasks.push(skill_card(&name));
                        }
                        render_live_status(&mut status);
                    }
                }
            }
            // Parse web search sources from the result and store them for display
            if action == "web_search"
                && !result.starts_with("Web search error:")
                && result != "No web search results found."
                && let Ok(mut status) = progress_live_status.lock()
            {
                let sources = parse_web_search_sources(&result);
                status.web_sources.extend(sources);
            }
        }
    };

    let chunk_live_status = Arc::clone(&live_status);
    let on_chunk = |summary: String| {
        if !options.fast_mode
            && let Ok(mut status) = chunk_live_status.lock()
        {
            status.thinking = None;
            commit_activity_snapshot(&mut status);
            clear_live_status(&mut status);
        }
        let formatted_summary = format_markdown_bold(&sanitize_latex(&summary));
        print!("\n  {MINT}Mint:{RESET} ");
        render_live_summary(&formatted_summary);

        // Print web search sources if any were collected (grouped by domain)
        if let Ok(mut status) = chunk_live_status.lock()
            && !status.web_sources.is_empty()
        {
            println!();
            println!("  {DIM}Sources:{RESET}");

            let mut domain_groups: Vec<(String, Vec<(String, String)>)> = Vec::new();
            for (title, url) in status.web_sources.drain(..) {
                let domain = extract_domain(&url);
                if let Some(group) = domain_groups.iter_mut().find(|(d, _)| d == &domain) {
                    group.1.push((title, url));
                } else {
                    domain_groups.push((domain, vec![(title, url)]));
                }
            }

            for (i, (domain, items)) in domain_groups.iter().enumerate() {
                let (first_title, first_url) = &items[0];
                let extra_count = items.len() - 1;
                if extra_count > 0 {
                    println!(
                        "  {DIM}{}.{RESET} {BLUE}{}{RESET} {DIM}({}){RESET} {CYAN}[+{} extra]{RESET}",
                        i + 1,
                        first_title,
                        domain,
                        extra_count
                    );
                } else {
                    println!(
                        "  {DIM}{}.{RESET} {BLUE}{}{RESET} {DIM}({}){RESET}",
                        i + 1,
                        first_title,
                        domain
                    );
                }
                println!("     {DIM}{}{RESET}", first_url);
            }
        }

        println!();
    };

    let user_name = MemoryStore::open_default()
        .ok()
        .and_then(|memory| memory.get_profile("name").ok().flatten());

    let agent_loop = orchestrate_agent_loop(
        config,
        task,
        root,
        image_data_uri,
        None,
        video_data_uri,
        Some(CHAT_CLI_ID),
        None,
        user_name.as_deref(),
        options.fast_mode,
        options.plan_mode,
        approve_cb,
        progress_cb,
        on_chunk,
    );
    let res = if options.fast_mode {
        agent_loop.await
    } else {
        tokio::select! {
            res = agent_loop => res,
            _ = wait_for_escape_interrupt(Arc::clone(&approval_active)) => {
                Err(OrchestrationError::Agent("interrupted by Esc".into()))
            }
        }
    };
    agent_done.store(true, Ordering::Relaxed);
    if !options.fast_mode
        && let Ok(mut status) = live_status.lock()
    {
        status.thinking = None;
        if res.is_err() {
            commit_activity_snapshot(&mut status);
        }
        clear_live_status(&mut status);
    }
    let res = res.map_err(|e| anyhow!("{}", e))?;

    if should_show_verification(&res.verification) {
        println!("  Verification: {}", res.verification);
    }
    let badge = if let Some(fb_provider) = &res.fallback {
        format!(
            "{DIM}{} • {} → fallback: {} • {}{RESET}",
            config.ai_provider,
            crate::active_model(&config.ai_provider, config),
            fb_provider,
            res.model
        )
    } else {
        format!("{DIM}{} • {}{RESET}", res.provider, res.model)
    };
    println!("  {badge}");
    println!(
        "  {DIM}─ Worked for {}{RESET}",
        format_elapsed(started_at.elapsed())
    );

    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    println!("  {DIM}{}{RESET}", "─".repeat(width.saturating_sub(2)));

    Ok(res)
}

fn clear_working_status() {
    print!("\r\x1b[2K");
    let _ = io::stdout().flush();
}

fn extract_domain(url: &str) -> String {
    let clean = url.trim();
    let without_scheme = clean
        .strip_prefix("https://")
        .or_else(|| clean.strip_prefix("http://"))
        .unwrap_or(clean);
    let hostname = without_scheme.split('/').next().unwrap_or(without_scheme);
    hostname
        .strip_prefix("www.")
        .unwrap_or(hostname)
        .to_lowercase()
}

/// Parse (title, url) pairs from the formatted `web_search` ToolEnd result text.
/// The format produced by orchestration.rs is:
///   1. Title\n   URL: https://...\n   Snippet\n
fn parse_web_search_sources(result: &str) -> Vec<(String, String)> {
    let mut sources = Vec::new();
    let mut current_title: Option<String> = None;
    for line in result.lines() {
        let trimmed = line.trim();
        // Match numbered title lines: "1. Title text"
        if let Some(rest) = trimmed.split_once(". ")
            && rest.0.parse::<usize>().is_ok()
        {
            current_title = Some(rest.1.trim().to_owned());
            continue;
        }
        // Match URL lines: "URL: https://..."
        if let Some(url) = trimmed.strip_prefix("URL: ")
            && let Some(title) = current_title.take()
        {
            let url = url.trim().to_owned();
            if !url.is_empty() {
                sources.push((title, url));
            }
        }
    }
    sources
}

/// Playful gerunds shown in place of "Thinking" while the model is working,
/// picked once per turn so the label doesn't change mid-flight.
const THINKING_VERBS: &[&str] = &[
    "Thinking",
    "Pondering",
    "Percolating",
    "Ruminating",
    "Noodling",
    "Marinating",
    "Simmering",
    "Mulling",
    "Cogitating",
    "Deliberating",
    "Puzzling",
    "Burrowing",
    "Excavating",
    "Foraging",
    "Spelunking",
    "Divining",
    "Conjuring",
    "Brewing",
    "Synthesizing",
    "Weaving",
    "Wrangling",
    "Herding",
    "Contemplating",
    "Musing",
    "Untangling",
    "Unpacking",
];

fn random_thinking_verb() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    THINKING_VERBS[nanos as usize % THINKING_VERBS.len()]
}

fn format_elapsed(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

fn render_live_summary(summary: &str) {
    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;

    let mut is_first = true;
    let mut table_buffer: Vec<String> = Vec::new();

    for line in summary.split('\n') {
        if markdown::is_table_line(line) {
            table_buffer.push(line.to_string());
            continue;
        }

        if !table_buffer.is_empty() {
            print_table_block(&table_buffer, &mut is_first);
            table_buffer.clear();
        }

        let indent = if is_first { "" } else { "  " };
        let options = textwrap::Options::new(width)
            .initial_indent(indent)
            .subsequent_indent("  ")
            .break_words(true);
        let wrapped = textwrap::fill(line, &options);

        if is_first {
            print!("{wrapped}");
            is_first = false;
        } else {
            print!("\n{wrapped}");
        }
    }

    if !table_buffer.is_empty() {
        print_table_block(&table_buffer, &mut is_first);
    }

    let _ = io::stdout().flush();
}

fn print_table_block(table_lines: &[String], is_first: &mut bool) {
    let rendered = markdown::render_markdown_table(table_lines);
    for line in rendered.split('\n') {
        if *is_first {
            print!("{line}");
            *is_first = false;
        } else {
            print!("\n  {line}");
        }
    }
}

fn diff_stats(diff: &str) -> (usize, usize) {
    let mut additions = 0;
    let mut deletions = 0;
    for line in diff.lines() {
        if line.starts_with("+++ ") || line.starts_with("--- ") {
            continue;
        }
        if line.starts_with('+') {
            additions += 1;
        } else if line.starts_with('-') {
            deletions += 1;
        }
    }
    (additions, deletions)
}

fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let trimmed = line.trim();
    if !trimmed.starts_with("@@") {
        return None;
    }
    let parts: Vec<&str> = trimmed.split("@@").collect();
    if parts.len() < 3 {
        return None;
    }
    let header_body = parts[1].trim();
    let mut old_start = 1;
    let mut new_start = 1;

    for token in header_body.split_whitespace() {
        if let Some(num_str) = token.strip_prefix('-') {
            let start_str = num_str.split(',').next().unwrap_or(num_str);
            old_start = start_str.parse().unwrap_or(1);
        } else if let Some(num_str) = token.strip_prefix('+') {
            let start_str = num_str.split(',').next().unwrap_or(num_str);
            new_start = start_str.parse().unwrap_or(1);
        }
    }
    Some((old_start, new_start))
}

/// Compact single-line approval header, e.g. `Update(src/foo.rs) — +3 -1 lines`.
fn print_diff_header(action: &str, path: &str, additions: usize, deletions: usize) {
    println!();
    println!(
        "  {BRIGHT}{action}{RESET}({BLUE}{path}{RESET}) {DIM}—{RESET} {GREEN}+{additions}{RESET} {RED}-{deletions}{RESET} {DIM}lines{RESET}"
    );
}

/// Prints one diff row as a full-width color band (like a diff viewer's gutter
/// highlight), padding `content` with spaces out to the terminal width so the
/// background color fills the row instead of just wrapping the text.
fn print_diff_band(bg: &str, line_num: usize, content: &str, term_width: usize) {
    let line_num_str = format!("{:>5}", line_num);
    let prefix_visible_len = 2 + line_num_str.chars().count() + 1;
    // Use the app's East-Asian/Thai-combining-mark-aware width, not a raw
    // char count — a naive count over/under-shoots the true terminal column
    // width for wide glyphs and zero-width Thai marks, leaving the band short
    // of (or past) the terminal edge.
    let available_width = term_width.saturating_sub(prefix_visible_len).max(1);
    let blank_line_num = " ".repeat(line_num_str.chars().count());

    // A code/diff line longer than the terminal is hard-wrapped (never
    // word-wrapped, which would collapse indentation) into multiple physical
    // lines so the colored band never overflows past the terminal edge — the
    // same class of bug the markdown table renderer had.
    for (i, wrapped_line) in markdown::hard_wrap(content, available_width)
        .into_iter()
        .enumerate()
    {
        let line_label = if i == 0 {
            &line_num_str
        } else {
            &blank_line_num
        };
        let content_len = crate::interactive::string_visual_width(&wrapped_line);
        let pad_len = available_width.saturating_sub(content_len);
        println!(
            "  {DIM}{line_label}{RESET} {bg}{wrapped_line}{}{RESET}",
            " ".repeat(pad_len)
        );
    }
}

fn print_colored_diff(diff: &str) {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_width = term_width as usize;
    let mut current_old_line = 1;
    let mut current_new_line = 1;

    for line in diff.lines() {
        if line.starts_with("@@") {
            // Still track line numbers from the hunk header, just don't print
            // the raw unified-diff header/marker lines — they're noise here.
            if let Some((old_s, new_s)) = parse_hunk_header(line) {
                current_old_line = old_s;
                current_new_line = new_s;
            }
        } else if line.starts_with("--- ") || line.starts_with("+++ ") {
            // skip: raw diff file-header lines aren't useful in an approval prompt
        } else if let Some(content) = line.strip_prefix('-') {
            print_diff_band(BG_DEL, current_old_line, content, term_width);
            current_old_line += 1;
        } else if let Some(content) = line.strip_prefix('+') {
            print_diff_band(BG_ADD, current_new_line, content, term_width);
            current_new_line += 1;
        } else {
            let line_num_str = format!("{:>5}", current_new_line);
            current_old_line += 1;
            current_new_line += 1;
            println!("  {DIM}{line_num_str}{RESET} {line}");
        }
    }
}

fn should_show_verification(verification: &str) -> bool {
    let normalized = verification.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if normalized.starts_with("information retrieved from web search")
        || normalized.starts_with("successfully ran background command")
        || normalized.starts_with("opened ")
        || normalized.contains("background command to open")
        || normalized.contains("web search results")
    {
        return false;
    }
    !matches!(
        normalized.as_str(),
        "not run"
            | "not run."
            | "no checks run"
            | "no checks run."
            | "no technical task requested"
            | "no technical task requested."
            | "no technical task requested, just a greeting."
            | "not required"
            | "not required."
            | "none"
            | "n/a"
    )
}

#[derive(Debug, Default)]
struct LiveStatus {
    thinking: Option<String>,
    explored: Vec<ExploredAction>,
    activities: Vec<String>,
    tasks: Vec<TaskEntry>,
    plan_steps: Vec<String>,
    committed_explored: usize,
    committed_activities: usize,
    committed_tasks: usize,
    spinner_tick: usize,
    /// Sources collected from web_search ToolEnd results (title, url)
    web_sources: Vec<(String, String)>,
    inline_tui: InlineTui,
}

/// A `ratatui` inline-viewport terminal for the live status region — the
/// only part of the interactive chat migrated to `ratatui` so far (see the
/// TUI migration plan). Lazily constructed on first use and torn down by
/// [`clear_live_status`] before any other code prints raw lines, since a
/// live `Terminal` desyncs (and corrupts the next redraw) if the screen
/// changes underneath it without going through the terminal's own API.
///
/// Deliberately constructed **once** per live stretch (not resized/rebuilt
/// as content grows) and held for the rest of the turn, including across
/// [`commit_activity_snapshot`]/[`print_timeline_note`]'s `insert_before`
/// calls. An earlier version reconstructed the whole `Terminal` (a fresh
/// cursor-position query + fresh `viewport_area`/`last_known_area`) every
/// time live content grew even slightly, and combined with `insert_before`
/// that measurably corrupted already-printed conversation history — the
/// repeated resets desynced `insert_before`'s internal scroll bookkeeping
/// from where content actually was on screen. A single long-lived instance
/// per turn is the pattern ratatui's own inline example uses (many
/// `insert_before` calls against one `Terminal`, never rebuilt mid-run),
/// so this follows that instead of inventing a resize dance the library
/// wasn't built around. The tradeoff: the live viewport's height is fixed
/// generously up front rather than tracking content exactly, so unusually
/// long in-flight content (e.g. a very long plan) can get visually clipped
/// in the *live* view — but nothing is lost, since it still prints in full
/// once committed (`insert_before`'s own per-call height isn't capped by
/// this).
#[derive(Debug, Default)]
struct InlineTui {
    terminal: Option<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>>,
}

/// Set for the duration of a `ratatui`/`crossterm` cursor-position query
/// (see [`with_raw_mode_for_cursor_query`]) so [`wait_for_escape_interrupt`]
/// — which polls the *same* stdin for an Esc key every 80ms for the entire
/// agent turn — knows to back off instead of racing to read the terminal's
/// `\x1b[row;colR` reply first. Without this, `event::read()` over there
/// can consume the reply before the query's own reader sees it (it doesn't
/// look like a recognized key event so it's silently swallowed), leaving
/// the query to time out or read whatever garbled remainder is left —
/// which is what the literal `^[[49;9R` text some users saw came from.
static CURSOR_QUERY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Runs `f` with raw mode temporarily forced on, restoring whatever state
/// was in effect before. Any `ratatui`/`crossterm` call that queries the
/// terminal's cursor position sends `\x1b[6n` and reads the reply off
/// stdin — a reply that only reaches that read if raw mode is on; in cooked
/// mode the terminal instead local-echoes it as literal `^[[row;colR` text
/// into whatever's currently on screen. The rest of the interactive loop
/// only enables raw mode while reading a key, not during agent-turn output
/// (where the inline-viewport code runs), so every such call has to
/// bracket it like this itself. Also claims [`CURSOR_QUERY_ACTIVE`] for the
/// duration, so the Esc-watcher doesn't steal the reply off stdin.
pub(crate) fn with_raw_mode_for_cursor_query<T>(f: impl FnOnce() -> T) -> T {
    CURSOR_QUERY_ACTIVE.store(true, Ordering::Relaxed);
    let was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        let _ = crossterm::terminal::enable_raw_mode();
    }
    let result = f();
    if !was_raw {
        let _ = crossterm::terminal::disable_raw_mode();
    }
    CURSOR_QUERY_ACTIVE.store(false, Ordering::Relaxed);
    result
}

impl InlineTui {
    /// Returns the live terminal, constructing it once (sized generously
    /// from the current window height, not from content) if this is the
    /// first call since the last [`teardown`](Self::teardown). Never
    /// reconstructs for an already-live terminal — see the struct docs for
    /// why that matters for `insert_before`'s correctness.
    fn ensure(
        &mut self,
    ) -> io::Result<&mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>>
    {
        if self.terminal.is_none() {
            let (_, term_rows) = crossterm::terminal::size().unwrap_or((80, 24));
            // Generous enough for realistic in-flight content between two
            // commits, capped so it can't dominate a short terminal.
            let height = term_rows.saturating_sub(6).clamp(3, 20);
            let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
            let terminal = with_raw_mode_for_cursor_query(move || {
                ratatui::Terminal::with_options(
                    backend,
                    ratatui::TerminalOptions {
                        viewport: ratatui::Viewport::Inline(height),
                    },
                )
            })?;
            self.terminal = Some(terminal);
        }
        Ok(self.terminal.as_mut().expect("just set above"))
    }

    fn teardown(&mut self) {
        self.drop_and_erase();
    }

    /// Erases the current terminal's drawn rows *before* dropping it — a
    /// bare `self.terminal = None` leaves last frame's content sitting on
    /// screen, so a freshly-constructed `Terminal`'s cursor-position query
    /// lands below it instead of on top of it, stacking a new copy under
    /// the old one every time. `Terminal::clear()` moves the cursor back to
    /// the top of its own viewport and clears from there, so the *next*
    /// construction's cursor query sees a clean slate in the right place.
    fn drop_and_erase(&mut self) {
        if let Some(mut terminal) = self.terminal.take() {
            let _ = terminal.clear();
        }
    }
}

#[derive(Debug, Clone, Default)]
struct TaskEntry {
    label: String,
    /// Truncated preview of the command's raw output, shown indented under the label.
    output: Vec<String>,
}

impl From<String> for TaskEntry {
    fn from(label: String) -> Self {
        TaskEntry {
            label,
            output: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
struct ExploredAction {
    kind: &'static str,
    target: String,
}

fn explored_action_label(action: &str, input: &serde_json::Value) -> Option<ExploredAction> {
    match action {
        "list_files" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|path| ExploredAction {
                kind: "[list_files] List",
                target: display_tool_target(path),
            }),
        "read_file" => {
            let path = input.get("path").and_then(|v| v.as_str())?;
            let start = input.get("startLine").and_then(|v| v.as_u64());
            let end = input.get("endLine").and_then(|v| v.as_u64());
            let file_name = display_tool_target(path);
            let target = match (start, end) {
                (Some(s), Some(e)) => format!("{} #L{}-{}", file_name, s, e),
                (Some(s), None) => format!("{} #L{}", file_name, s),
                _ => file_name,
            };
            Some(ExploredAction {
                kind: "[read_file] Read",
                target,
            })
        }
        "search_code" => {
            let query = input.get("query").and_then(|v| v.as_str())?;
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let target = if path.trim().is_empty() || path == "." {
                query.to_owned()
            } else {
                format!("{} in {}", query, display_tool_target(path))
            };
            Some(ExploredAction {
                kind: "[search_code] Search",
                target,
            })
        }
        "symbols" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|path| ExploredAction {
                kind: "[symbols] Index symbols",
                target: display_tool_target(path),
            }),
        _ => None,
    }
}

fn display_tool_target(path: &str) -> String {
    if path.trim().is_empty() {
        ".".into()
    } else {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path)
            .into()
    }
}

fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }
    result
}

fn is_thai_combining(c: char) -> bool {
    matches!(c,
        '\u{0e31}' | '\u{0e34}'..='\u{0e37}' | '\u{0e38}'..='\u{0e39}' |
        '\u{0e47}'..='\u{0e4e}'
    )
}

fn apply_wave_effect(text: &str, tick: usize) -> String {
    let (label, metadata) = if let Some(idx) = text.rfind(" • Esc to interrupt)") {
        if let Some(open_paren_idx) = text[..idx].rfind('(') {
            (&text[..open_paren_idx], &text[open_paren_idx..])
        } else {
            (text, "")
        }
    } else {
        (text, "")
    };

    let trimmed_label = label.trim_end();
    let spaces_count = label.len() - trimmed_label.len();

    let chars: Vec<char> = trimmed_label.chars().collect();
    let n = chars.len();
    if n == 0 {
        return text.to_string();
    }

    let mut animated_label = String::new();

    // Smooth wave movement using a sine wave function.
    // phase shifts with tick (time), index i acts as spatial shift.
    // 0.3 speed controls animation rate, 0.4 controls width of the wave crest.
    let speed = 0.3;
    let phase = tick as f32 * speed;

    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            animated_label.push(c);
            continue;
        }

        let x = (i as f32 * 0.4) - phase;
        let t = (x.sin() + 1.0) / 2.0; // Oscillates in [0.0, 1.0]

        // Stop colors: Dim Gray (70, 70, 70) -> Mint Green (105, 230, 166) -> Cyan (78, 201, 216)
        let (r, g, b) = if t < 0.3 {
            let local_t = t / 0.3;
            let r = 70.0 + (105.0 - 70.0) * local_t;
            let g = 70.0 + (230.0 - 70.0) * local_t;
            let b = 70.0 + (166.0 - 70.0) * local_t;
            (r, g, b)
        } else if t < 0.7 {
            let local_t = (t - 0.3) / 0.4;
            let r = 105.0 + (78.0 - 105.0) * local_t;
            let g = 230.0 + (201.0 - 230.0) * local_t;
            let b = 166.0 + (216.0 - 166.0) * local_t;
            (r, g, b)
        } else {
            let local_t = (t - 0.7) / 0.3;
            let r = 78.0 + (70.0 - 78.0) * local_t;
            let g = 201.0 + (70.0 - 201.0) * local_t;
            let b = 216.0 + (70.0 - 216.0) * local_t;
            (r, g, b)
        };

        animated_label.push_str(&format!(
            "\x1b[1m\x1b[38;2;{};{};{}m{}\x1b[0m",
            r.round() as u8,
            g.round() as u8,
            b.round() as u8,
            c
        ));
    }

    animated_label.push_str(&" ".repeat(spaces_count));
    if !metadata.is_empty() {
        animated_label.push_str(&format!("{BRIGHT}{metadata}{RESET}"));
    }
    animated_label
}

fn render_live_status(status: &mut LiveStatus) {
    let mut lines = Vec::new();
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let is_thinking = status.thinking.is_some();
    let tick = status.spinner_tick;

    lines.extend(plan_lines(&status.plan_steps, is_thinking, tick));
    lines.extend(activity_block_lines(
        &status.tasks[tasks_start..],
        &status.activities[activities_start..],
        &status.explored[explored_start..],
        is_thinking,
        tick,
    ));
    if let Some(thinking) = &status.thinking {
        let frames = &[
            "🌑\u{FE0E}",
            "🌒\u{FE0E}",
            "🌓\u{FE0E}",
            "🌔\u{FE0E}",
            "🌕\u{FE0E}",
            "🌖\u{FE0E}",
            "🌗\u{FE0E}",
            "🌘\u{FE0E}",
        ];
        let frame = frames[status.spinner_tick % frames.len()];

        let dots_frames = &["", ".", "..", "..."];
        let dots = dots_frames[(status.spinner_tick / 2) % dots_frames.len()];

        status.spinner_tick += 1;

        // The verb is whatever precedes the trailing "(elapsed • Esc to interrupt)"
        // segment — find that split point rather than matching a literal word, since
        // the verb is now randomized per turn (see THINKING_VERBS).
        let custom_thinking = if let Some(idx) = thinking.rfind(" (") {
            format!("{}{:<3}{}", &thinking[..idx], dots, &thinking[idx..])
        } else {
            thinking.clone()
        };

        let waved_thinking = apply_wave_effect(&custom_thinking, status.spinner_tick);

        lines.push(format!("  {MINT}{frame}{RESET} {waved_thinking}"));
    }
    if lines.is_empty() {
        status.inline_tui.teardown();
        return;
    }

    let Ok(text) = lines.join("\n").into_text() else {
        return;
    };
    let Ok(terminal) = status.inline_tui.ensure() else {
        return;
    };
    // `draw()` itself only queries the cursor position on the rare path
    // where it detects the terminal window was actually resized since the
    // last frame — bracket it too, defensively, for that case.
    let _ = with_raw_mode_for_cursor_query(|| {
        terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(
                ratatui::widgets::Paragraph::new(text).wrap(ratatui::widgets::Wrap { trim: false }),
                area,
            );
        })
    });
}

fn commit_activity_snapshot(status: &mut LiveStatus) {
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let mut lines = activity_block_lines(
        &status.tasks[tasks_start..],
        &status.activities[activities_start..],
        &status.explored[explored_start..],
        false,
        0,
    );
    if lines.is_empty() {
        return;
    }

    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    lines.push(String::new());
    lines.push(format!("{DIM}{}{RESET}", "─".repeat(width)));
    lines.push(String::new());
    insert_permanent_lines(status, &lines);

    status.committed_explored = status.explored.len();
    status.committed_activities = status.activities.len();
    status.committed_tasks = status.tasks.len();
}

fn print_timeline_note(status: &mut LiveStatus, thought: &str) {
    let thought = thought.trim();
    if thought.is_empty() {
        return;
    }
    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    let options = textwrap::Options::new(width)
        .initial_indent("  • ")
        .subsequent_indent("    ")
        .break_words(true);
    let wrapped = textwrap::fill(thought, &options);
    insert_permanent_lines(status, &[format!("{DIM}{wrapped}{RESET}")]);
}

/// Inserts already ANSI-formatted `lines` as permanent content above the
/// live inline region — used by [`commit_activity_snapshot`] and
/// [`print_timeline_note`] to "freeze" in-flight status into real
/// scrollback, via `Terminal::insert_before`.
///
/// A first attempt at this kept `insert_before`'s target `Terminal` alive
/// correctly, but `InlineTui::ensure` at the time *also* reconstructed that
/// same `Terminal` from scratch every time live-status content grew even
/// slightly — a fresh cursor-position query and fresh `viewport_area`/
/// `last_known_area` on every reconstruction, discarding what
/// `insert_before` had been tracking. Interleaved with enough
/// reconstructions, that measurably corrupted already-printed conversation
/// history, not just live-status content. `ensure` no longer reconstructs
/// once a terminal is live (see its docs), so this is safe to use again:
/// exactly one `Terminal` instance persists for the whole live stretch, and
/// only `draw`/`insert_before` — never a full rebuild — touch it in between.
/// Falls back to plain `println!` if no inline terminal is currently live
/// (e.g. committing before anything has rendered yet this turn).
fn insert_permanent_lines(status: &mut LiveStatus, lines: &[String]) {
    let Some(terminal) = status.inline_tui.terminal.as_mut() else {
        for line in lines {
            println!("{line}");
        }
        let _ = io::stdout().flush();
        return;
    };

    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    let mut height: u16 = 0;
    for line in lines {
        let stripped = strip_ansi_escapes(line);
        let line_len = stripped.chars().filter(|&c| !is_thai_combining(c)).count();
        let physical_lines = if width > 0 {
            line_len.div_ceil(width)
        } else {
            1
        }
        .max(1);
        height = height.saturating_add(physical_lines as u16);
    }

    let Ok(text) = lines.join("\n").into_text() else {
        return;
    };
    let _ = terminal.insert_before(height, |buf| {
        use ratatui::widgets::Widget as _;
        ratatui::widgets::Paragraph::new(text).render(buf.area, buf);
    });
}

/// Tears down the shared inline `ratatui` terminal (if one is currently
/// live), so whatever prints next — an approval card, a committed activity
/// snapshot, the final AI answer — starts from a clean, un-managed cursor
/// position instead of colliding with content the inline viewport still
/// thinks it owns. Dropping the `Terminal` doesn't itself touch the screen
/// (its `Drop` impl only restores cursor visibility); the next
/// `render_live_status` call reconstructs it fresh, re-querying the
/// terminal for where the cursor actually is now.
fn clear_live_status(status: &mut LiveStatus) {
    if status.inline_tui.terminal.is_none() {
        clear_working_status();
        return;
    }
    status.inline_tui.teardown();
}

fn bullet_char(is_thinking: bool, tick: usize) -> &'static str {
    if is_thinking {
        if (tick / 4).is_multiple_of(2) {
            "●"
        } else {
            "○"
        }
    } else {
        "●"
    }
}

fn get_bullet(name: &str, is_thinking: bool, tick: usize) -> String {
    let char_str = bullet_char(is_thinking, tick);
    match name {
        "plan" => format!("{BLUE}{char_str}{RESET} plan"),
        _ => char_str.to_string(),
    }
}

/// One-line rollup of everything counted in `activity_summary_line`, e.g.
/// "Searching for 8 patterns, reading 5 files, listing 1 directory, running 2
/// shell commands…" — shown as the header for the combined tasks/activities/
/// explored block instead of a bare word, so the user gets a sense of scope
/// at a glance instead of only a growing, undifferentiated list.
fn activity_summary_line(
    tasks: &[TaskEntry],
    activities: &[String],
    explored: &[ExploredAction],
) -> Option<String> {
    let mut pattern_count = 0usize;
    let mut file_count = 0usize;
    let mut dir_count = 0usize;
    let mut symbol_count = 0usize;
    for action in explored {
        match action.kind {
            "[search_code] Search" => pattern_count += 1,
            "[read_file] Read" => file_count += 1,
            "[list_files] List" => dir_count += 1,
            "[symbols] Index symbols" => symbol_count += 1,
            _ => {}
        }
    }
    let web_count = activities.len();
    let shell_count = tasks
        .iter()
        .filter(|t| t.label.starts_with("[run_shell]"))
        .count();

    let mut parts: Vec<String> = Vec::new();
    if pattern_count > 0 {
        parts.push(format!(
            "searching for {pattern_count} pattern{}",
            if pattern_count == 1 { "" } else { "s" }
        ));
    }
    if file_count > 0 {
        parts.push(format!(
            "reading {file_count} file{}",
            if file_count == 1 { "" } else { "s" }
        ));
    }
    if dir_count > 0 {
        parts.push(format!(
            "listing {dir_count} director{}",
            if dir_count == 1 { "y" } else { "ies" }
        ));
    }
    if symbol_count > 0 {
        parts.push(format!(
            "indexing {symbol_count} symbol file{}",
            if symbol_count == 1 { "" } else { "s" }
        ));
    }
    if web_count > 0 {
        parts.push(format!(
            "searching the web {web_count} time{}",
            if web_count == 1 { "" } else { "s" }
        ));
    }
    if shell_count > 0 {
        parts.push(format!(
            "running {shell_count} shell command{}",
            if shell_count == 1 { "" } else { "s" }
        ));
    }

    if parts.is_empty() {
        return None;
    }
    let sentence = parts.join(", ");
    let mut chars = sentence.chars();
    let capitalized = match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => sentence,
    };
    Some(format!("{capitalized}…"))
}

/// Combined header + body for the tasks/activities/explored groups: a single
/// summary line (via `activity_summary_line`) instead of three separate,
/// unlabeled bullet groups.
fn activity_block_lines(
    tasks: &[TaskEntry],
    activities: &[String],
    explored: &[ExploredAction],
    is_thinking: bool,
    tick: usize,
) -> Vec<String> {
    if tasks.is_empty() && activities.is_empty() && explored.is_empty() {
        return Vec::new();
    }
    let char_str = bullet_char(is_thinking, tick);
    let header_text =
        activity_summary_line(tasks, activities, explored).unwrap_or_else(|| "activity".into());
    let mut lines = vec![format!("  {BLUE}{char_str}{RESET} {header_text}")];
    lines.extend(tasks_lines(tasks));
    lines.extend(activities_lines(activities));
    lines.extend(explored_lines(explored));
    lines
}

fn explored_lines(actions: &[ExploredAction]) -> Vec<String> {
    if actions.is_empty() {
        return Vec::new();
    }
    let grouped = grouped_explored_actions(actions);
    let mut lines: Vec<String> = grouped
        .iter()
        .take(24)
        .enumerate()
        .map(|(index, action)| {
            let prefix = if index == 0 { "    └" } else { "     " };
            format!("{DIM}{prefix} {action}{RESET}")
        })
        .collect();
    if grouped.len() > 24 {
        lines.push(format!("{DIM}     ... {} more{RESET}", grouped.len() - 24));
    }
    lines
}

fn ran_command_labels(action: &str, input: &serde_json::Value) -> Option<Vec<String>> {
    match action {
        "run_shell" => input
            .get("command")
            .and_then(|v| v.as_str())
            .filter(|command| !command.trim().is_empty())
            .map(|command| vec![command.trim().to_owned()]),
        "shell_output" | "kill_shell" => input
            .get("job_id")
            .and_then(|v| v.as_str())
            .filter(|job_id| !job_id.trim().is_empty())
            .map(|job_id| vec![job_id.trim().to_owned()]),
        "verify" => input
            .get("commands")
            .and_then(|v| v.as_array())
            .map(|commands| {
                commands
                    .iter()
                    .filter_map(|command| command.as_str())
                    .map(str::trim)
                    .filter(|command| !command.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            }),
        _ => None,
    }
}

/// If `path` looks like a skill file — `<skills-dir>/<name>/SKILL.md` (or
/// SKILL.txt / lowercase variants), or a flat `<skills-dir>/<name>.md` in the
/// global skills folder — returns the skill's name. Mirrors the file shapes
/// `mint_core::skills::load_skills_from_dir` recognizes. Used to tell a
/// skill file read apart from an ordinary `read_file` call so it renders as
/// a `Skill(name)` card instead of a generic "explored files" line.
fn skill_name_for_read_path(path: &str) -> Option<String> {
    let path = Path::new(path);
    let file_name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    let parent_name = parent.file_name()?.to_str()?;

    if matches!(file_name, "SKILL.md" | "SKILL.txt" | "skill.md" | "skill.txt") {
        let grandparent_name = parent
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        if matches!(grandparent_name, Some("skills") | Some("mint-skills")) {
            return Some(parent_name.to_string());
        }
    }

    if matches!(parent_name, "skills" | "mint-skills") {
        let ext = path.extension().and_then(|e| e.to_str())?;
        if matches!(ext.to_ascii_lowercase().as_str(), "md" | "txt") {
            return path.file_stem().and_then(|n| n.to_str()).map(str::to_owned);
        }
    }

    None
}

/// Skill names the `memory_recall` tool matched, parsed out of its
/// `"[Skill: {name}]\n{content}"` result blocks (see the `"memory_recall"`
/// arm in `orchestration.rs`) — lets ToolEnd render a `Skill(name)` card
/// when the agent found and is about to use a learned skill this way.
fn skill_names_from_memory_recall(result: &str) -> Vec<String> {
    result
        .lines()
        .filter_map(|line| {
            line.strip_prefix("[Skill: ")
                .and_then(|rest| rest.strip_suffix(']'))
                .map(str::to_owned)
        })
        .collect()
}

fn skill_card(skill_name: &str) -> TaskEntry {
    TaskEntry {
        label: format!("Skill({skill_name})"),
        output: vec!["Successfully loaded skill".to_string()],
    }
}

fn command_was_run(result: &str) -> bool {
    result.lines().any(|line| line.starts_with("exit: "))
}

/// Truncated preview of a command's raw stdout/stderr, shown indented under its
/// "Finished command" label. Drops internal bookkeeping lines (`mode:`, `sandboxed:`)
/// and caps the output so a noisy command can't flood the terminal.
fn command_output_preview(result: &str) -> Vec<String> {
    const MAX_LINES: usize = 7;
    // A single very long line (a full `pgrep -af` invocation, a minified
    // JSON blob, ...) still wraps across many terminal rows even though it
    // only counts as one line against MAX_LINES above — cut it down too.
    const MAX_LINE_WIDTH: usize = 120;

    let filtered: Vec<&str> = result
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("mode: ") || trimmed.starts_with("sandboxed: "))
        })
        .collect();

    let start = filtered.iter().position(|l| !l.trim().is_empty());
    let Some(start) = start else {
        return Vec::new();
    };
    let end = filtered
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(start, |i| i + 1);
    let trimmed = &filtered[start..end];

    let mut preview: Vec<String> = trimmed
        .iter()
        .take(MAX_LINES)
        .map(|line| truncate_line(line, MAX_LINE_WIDTH))
        .collect();
    if trimmed.len() > MAX_LINES {
        preview.push(format!("... {} more lines", trimmed.len() - MAX_LINES));
    }
    preview
}

fn truncate_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    let head: String = line.chars().take(max_chars).collect();
    format!("{head}…")
}

fn activities_lines(activities: &[String]) -> Vec<String> {
    if activities.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = activities
        .iter()
        .take(24)
        .enumerate()
        .map(|(index, act)| {
            let prefix = if index == 0 { "    └" } else { "     " };
            format!("{DIM}{prefix} {act}{RESET}")
        })
        .collect();
    if activities.len() > 24 {
        lines.push(format!(
            "{DIM}     ... {} more{RESET}",
            activities.len() - 24
        ));
    }
    lines
}

fn extract_plan_steps(input: &serde_json::Value) -> Option<Vec<String>> {
    let steps_val = input.get("steps")?;
    let arr = steps_val.as_array()?;
    let mut steps = Vec::new();
    for v in arr {
        if let Some(s) = v.as_str() {
            steps.push(s.to_string());
        }
    }
    Some(steps)
}

fn plan_lines(steps: &[String], is_thinking: bool, tick: usize) -> Vec<String> {
    if steps.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("  {}", get_bullet("plan", is_thinking, tick))];
    for (index, step) in steps.iter().enumerate() {
        let prefix = if index == steps.len() - 1 {
            "    └"
        } else {
            "    ├"
        };
        let (checked, text) = if step.to_lowercase().starts_with("done:") {
            (format!("{MINT}[x]{RESET}"), step["done:".len()..].trim())
        } else if step.to_lowercase().starts_with("done: ") {
            (format!("{MINT}[x]{RESET}"), step["done: ".len()..].trim())
        } else if step.to_lowercase().starts_with("in_progress:") {
            (
                format!("{BLUE}[~]{RESET}"),
                step["in_progress:".len()..].trim(),
            )
        } else if step.to_lowercase().starts_with("in_progress: ") {
            (
                format!("{BLUE}[~]{RESET}"),
                step["in_progress: ".len()..].trim(),
            )
        } else if step.to_lowercase().starts_with("todo:") {
            (format!("{DIM}[ ]{RESET}"), step["todo:".len()..].trim())
        } else if step.to_lowercase().starts_with("todo: ") {
            (format!("{DIM}[ ]{RESET}"), step["todo: ".len()..].trim())
        } else {
            (format!("{DIM}[ ]{RESET}"), step.trim())
        };
        lines.push(format!("{DIM}{} {}{RESET} {}", prefix, checked, text));
    }
    lines
}

fn tasks_lines(tasks: &[TaskEntry]) -> Vec<String> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for (index, task) in tasks.iter().take(24).enumerate() {
        let prefix = if index == 0 { "    └" } else { "     " };
        lines.push(format!("{DIM}{prefix} {}{RESET}", task.label));
        for out_line in &task.output {
            lines.push(format!("{DIM}       │ {}{RESET}", out_line));
        }
    }
    if tasks.len() > 24 {
        lines.push(format!("{DIM}     ... {} more{RESET}", tasks.len() - 24));
    }
    lines
}

fn grouped_explored_actions(actions: &[ExploredAction]) -> Vec<String> {
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for action in actions {
        if let Some((_, targets)) = groups.iter_mut().find(|(kind, _)| *kind == action.kind) {
            if !targets.iter().any(|target| *target == action.target) {
                targets.push(action.target.as_str());
            }
        } else {
            groups.push((action.kind, vec![action.target.as_str()]));
        }
    }
    groups
        .into_iter()
        .map(|(kind, targets)| format!("{} {}", kind, targets.join(", ")))
        .collect()
}

async fn wait_for_escape_interrupt(approval_active: Arc<AtomicBool>) {
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    loop {
        if approval_active.load(Ordering::Relaxed) || CURSOR_QUERY_ACTIVE.load(Ordering::Relaxed)
        {
            tokio::time::sleep(Duration::from_millis(100)).await;
            continue;
        }

        let _ = enable_raw_mode();
        let escaped = matches!(event::poll(Duration::from_millis(0)), Ok(true))
            && matches!(
                event::read(),
                Ok(Event::Key(key_event))
                    if key_event.kind == event::KeyEventKind::Press
                        && key_event.code == KeyCode::Esc
            );
        let _ = disable_raw_mode();

        if escaped {
            break;
        }
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
}

/// Interactive picker for `ask_user` options: ↑/↓ + Enter to choose, digits for a quick
/// jump, any other character drops into free-text mode, Esc declines. Falls back to a
/// plain numbered prompt when raw mode isn't available (e.g. piped/non-interactive stdin).
fn run_option_picker(options: &[String]) -> Result<ApprovalOutcome, String> {
    use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    if enable_raw_mode().is_err() {
        return ask_numbered_fallback(options);
    }

    println!(
        "  {DIM}\u{2191}/\u{2193} + Enter to choose  \u{00b7}  type to answer freely  \u{00b7}  Esc to decline{RESET}"
    );
    let mut selected = 0usize;
    render_options(options, selected, true);

    let result = loop {
        let event = match event::read() {
            Ok(ev) => ev,
            Err(e) => break Err(e.to_string()),
        };
        let Event::Key(key) = event else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Up => {
                selected = selected.saturating_sub(1);
                render_options(options, selected, false);
            }
            KeyCode::Down => {
                selected = (selected + 1).min(options.len().saturating_sub(1));
                render_options(options, selected, false);
            }
            KeyCode::Enter => break Ok(ApprovalOutcome::Intercepted(options[selected].clone())),
            KeyCode::Esc => break Ok(ApprovalOutcome::Denied),
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let _ = disable_raw_mode();
                println!();
                std::process::exit(130);
            }
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if let Some(idx) = (c as usize)
                    .checked_sub('1' as usize)
                    .filter(|idx| *idx < options.len())
                {
                    break Ok(ApprovalOutcome::Intercepted(options[idx].clone()));
                }
            }
            KeyCode::Char(c) => {
                let _ = disable_raw_mode();
                print!("\n  Answer: {c}");
                let _ = io::stdout().flush();
                let mut rest = String::new();
                let _ = std::io::stdin().read_line(&mut rest);
                let full = format!("{c}{}", rest.trim_end_matches(['\n', '\r']));
                let trimmed = full.trim();
                return Ok(if trimmed.is_empty() {
                    ApprovalOutcome::Denied
                } else {
                    ApprovalOutcome::Intercepted(trimmed.to_owned())
                });
            }
            _ => {}
        }
    };

    let _ = disable_raw_mode();
    println!();
    result
}

fn render_options(options: &[String], selected: usize, first: bool) {
    use crossterm::terminal::{Clear, ClearType};
    use crossterm::{cursor, execute};

    let mut out = io::stdout();
    if !first {
        let _ = execute!(out, cursor::MoveUp(options.len() as u16));
    }
    for (i, opt) in options.iter().enumerate() {
        let _ = execute!(out, cursor::MoveToColumn(0), Clear(ClearType::CurrentLine));
        if i == selected {
            println!("  {CYAN}\u{276f}{RESET} {CYAN}{}) {}{RESET}", i + 1, opt);
        } else {
            println!("    {DIM}{}) {}{RESET}", i + 1, opt);
        }
    }
    let _ = out.flush();
}

fn ask_numbered_fallback(options: &[String]) -> Result<ApprovalOutcome, String> {
    for (i, opt) in options.iter().enumerate() {
        println!("    {}) {}", i + 1, opt);
    }
    print!(
        "  Answer (type a number 1-{}, your own text, or leave empty to decline): ",
        options.len()
    );
    let _ = io::stdout().flush();
    let mut answer = String::new();
    match std::io::stdin().read_line(&mut answer) {
        Ok(_) => {
            let trimmed = answer.trim();
            if trimmed.is_empty() {
                Ok(ApprovalOutcome::Denied)
            } else if let Some(choice) = trimmed
                .parse::<usize>()
                .ok()
                .and_then(|n| n.checked_sub(1))
                .and_then(|idx| options.get(idx))
            {
                Ok(ApprovalOutcome::Intercepted(choice.clone()))
            } else {
                Ok(ApprovalOutcome::Intercepted(trimmed.to_owned()))
            }
        }
        Err(error) => Err(error.to_string()),
    }
}

fn print_approval_card(title: &str, fields: &[(&str, &str)]) {
    let top_bar = format!("  {DIM}{}┬{}{RESET}", "─".repeat(11), "─".repeat(56));
    let bot_bar = format!("  {DIM}{}┴{}{RESET}", "─".repeat(11), "─".repeat(56));

    println!();
    println!(
        "  {BRIGHT}APPROVAL REQUIRED{RESET} {DIM}•{RESET} {BLUE}{}{RESET}",
        title
    );
    println!("{}", top_bar);
    for (label, val) in fields {
        let val_lines: Vec<&str> = val.lines().collect();
        if val_lines.is_empty() {
            println!("  {BRIGHT}{:<10}{RESET} {DIM}│{RESET}", label);
        } else {
            for (idx, line) in val_lines.iter().enumerate() {
                if idx == 0 {
                    println!("  {BRIGHT}{:<10}{RESET} {DIM}│{RESET} {}", label, line);
                } else {
                    println!("             {DIM}│{RESET} {}", line);
                }
            }
        }
    }
    println!("{}", bot_bar);
}

/// Approval prompt for the persistable `AgentApproval` variants (`run_shell`,
/// `write_file`, `apply_patch`, `note_write`, `run_plugin`, `mcp_tool`).
///
/// Checks `permission_rules` (in-memory rules from this run, seeded from
/// disk) first — if `tool`/`subject` already has a saved decision, returns
/// immediately without prompting or calling `render_card`. Otherwise calls
/// `render_card` (each approval type prints its own card format — a diff for
/// file edits, a plain field list for everything else) and shows a 3-option
/// prompt (Yes / Yes-and-don't-ask-again-this-session / No); the middle
/// choice appends the new rule to `permission_rules` for the rest of this
/// run only — deliberately *not* persisted via `save_config`, so it doesn't
/// outlive the process. Rules already on disk from before this change (or
/// added via `mint safety permissions`) are still honored by the lookup
/// above; this prompt just no longer creates new durable ones.
fn confirm_with_persistence(
    tool: &str,
    subject: &str,
    root: &Path,
    permission_rules: &mut Vec<PermissionRule>,
    approval_active: &AtomicBool,
    render_card: impl FnOnce(),
) -> Result<ApprovalOutcome, String> {
    if let Some(decision) = permission_decision_for(permission_rules, tool, subject, root) {
        return Ok(match decision {
            PermissionDecision::Allow => {
                println!(
                    "  {DIM}Auto-approved by saved permission rule ({tool}: {subject}){RESET}"
                );
                ApprovalOutcome::Approved
            }
            PermissionDecision::Deny => {
                println!("  {DIM}Auto-denied by saved permission rule ({tool}: {subject}){RESET}");
                ApprovalOutcome::Denied
            }
        });
    }

    render_card();
    approval_active.store(true, Ordering::Relaxed);
    let choice = prompt_persistent_approval(subject);
    approval_active.store(false, Ordering::Relaxed);

    match choice {
        0 => Ok(ApprovalOutcome::Approved),
        1 => {
            let rule = PermissionRule {
                tool: tool.to_string(),
                pattern: subject.to_string(),
                decision: PermissionDecision::Allow,
                project_root: None,
            };
            permission_rules.push(rule);
            Ok(ApprovalOutcome::Approved)
        }
        _ => Ok(ApprovalOutcome::Denied),
    }
}

/// Renders the 3-option approval choice and returns its index (0 = Yes,
/// 1 = Yes and don't ask again this session, 2 = No). Falls back to a plain
/// stdin line read (matching `crate::confirm`'s non-TTY behavior, e.g.
/// piped/CI invocations) when not running in a real terminal, rather than
/// silently defaulting to deny like the underlying `prompt_interactive_select`
/// does on its own.
fn prompt_persistent_approval(subject: &str) -> usize {
    use crossterm::tty::IsTty;

    // Kept short so the option still fits on one line in the picker.
    let truncated: String = if subject.chars().count() > 60 {
        subject.chars().take(60).collect::<String>() + "…"
    } else {
        subject.to_string()
    };

    let options = [
        "Yes".to_string(),
        format!("Yes, and don't ask again for: {truncated}"),
        "No".to_string(),
    ];

    if !io::stdout().is_tty() {
        print!("  Approve? [y]es / [d]on't ask again this session / [N]o: ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_err() {
            return 2;
        }
        return match answer.trim().to_ascii_lowercase().as_str() {
            "y" | "yes" => 0,
            "d" | "dont" | "don't" => 1,
            _ => 2,
        };
    }

    match crate::interactive::prompt_interactive_select(
        "Approve this action?",
        &options,
        &options[0],
    ) {
        Ok(Some(choice)) => options.iter().position(|o| *o == choice).unwrap_or(2),
        _ => 2,
    }
}

fn confirm_pausing_interrupt(prompt: &str, approval_active: &AtomicBool) -> bool {
    approval_active.store(true, Ordering::Relaxed);
    let approved = crate::confirm_security(prompt).unwrap_or(false);
    approval_active.store(false, Ordering::Relaxed);
    approved
}

/// Dim-cyan used for the code block gutter/border — same hue as `CYAN` but
/// at reduced intensity so it doesn't compete with actual code content.
const CODE_BORDER: &str = "\x1b[2m\x1b[38;2;56;189;248m";

/// Kept deliberately short (not spanning the full terminal width): the
/// already-ANSI-colored line gets re-wrapped by `textwrap` in
/// `render_live_summary`, which counts escape-code bytes toward width, so a
/// long border risks being cut mid-line. A short border avoids that.
fn code_block_border_dashes(term_width: usize) -> usize {
    term_width.saturating_sub(10).clamp(16, 40)
}

fn code_block_top_border(lang: &str, term_width: usize) -> String {
    let dashes = code_block_border_dashes(term_width);
    let label = if lang.is_empty() {
        String::new()
    } else {
        format!("─ {lang} ")
    };
    format!("{CODE_BORDER}┌{label}{}{RESET}", "─".repeat(dashes))
}

fn code_block_bottom_border(term_width: usize) -> String {
    let dashes = code_block_border_dashes(term_width);
    format!("{CODE_BORDER}└{}{RESET}", "─".repeat(dashes))
}

/// Loading these involves parsing bundled `.sublime-syntax`/theme data, so it's done once
/// per process (on first code block rendered) rather than on every `format_markdown_bold`
/// call — a chatty agent turn can print many code blocks in one response.
static SYNTAX_SET: std::sync::LazyLock<SyntaxSet> =
    std::sync::LazyLock::new(SyntaxSet::load_defaults_newlines);
static THEME_SET: std::sync::LazyLock<ThemeSet> = std::sync::LazyLock::new(ThemeSet::load_defaults);

/// Starts a highlighter for `lang` (a fenced code block's language hint, e.g. `rust`,
/// `ts`, `py` — `find_syntax_by_token` already knows the common short aliases). Returns
/// `None` for an empty/unrecognized hint, in which case the caller falls back to
/// unhighlighted code — better than guessing wrong and coloring things incorrectly.
fn start_code_highlighter(lang: &str) -> Option<HighlightLines<'static>> {
    let lang = lang.trim();
    if lang.is_empty() {
        return None;
    }
    let syntax = SYNTAX_SET.find_syntax_by_token(lang)?;
    let theme = &THEME_SET.themes["base16-ocean.dark"];
    Some(HighlightLines::new(syntax, theme))
}

/// Highlights one code-block line and returns it ANSI-colored, ready to print. Falls back
/// to the plain line on any error rather than dropping content — a rendering glitch should
/// never be the reason a line of the agent's actual answer goes missing.
fn highlight_code_line(highlighter: &mut HighlightLines, line: &str) -> String {
    // syntect's line-oriented highlighter tracks state (e.g. "inside a string") across
    // calls and expects each line to end in `\n` for that state tracking to be accurate,
    // even though the trailing newline itself isn't meaningful here.
    let with_newline = format!("{line}\n");
    match highlighter.highlight_line(&with_newline, &SYNTAX_SET) {
        Ok(ranges) => as_24_bit_terminal_escaped(&ranges, false)
            .trim_end_matches('\n')
            .to_string(),
        Err(_) => line.to_string(),
    }
}

fn format_markdown_bold(text: &str) -> String {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let term_width = term_width as usize;

    let mut formatted_lines = Vec::new();
    let mut in_code_block = false;
    let mut highlighter: Option<HighlightLines> = None;

    for line in text.lines() {
        let mut formatted_line = line.to_string();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            let was_in_code_block = in_code_block;
            in_code_block = !in_code_block;
            if was_in_code_block {
                highlighter = None;
                formatted_lines.push(code_block_bottom_border(term_width));
            } else {
                let lang = trimmed.trim_start_matches('`').trim();
                formatted_lines.push(code_block_top_border(lang, term_width));
                highlighter = start_code_highlighter(lang);
            }
            continue;
        }

        if !in_code_block {
            let mut leading_spaces = 0;
            let mut is_list_item = false;
            let mut marker_char = '-';

            for (idx, c) in line.char_indices() {
                if c.is_whitespace() {
                    leading_spaces += c.len_utf8();
                } else {
                    if c == '-' || c == '*' || c == '+' {
                        let after = &line[idx + c.len_utf8()..];
                        if after.chars().next().is_some_and(|c| c.is_whitespace()) {
                            is_list_item = true;
                            marker_char = c;
                        }
                    }
                    break;
                }
            }

            if is_list_item {
                let marker_len = marker_char.len_utf8();
                let mut new_line = String::new();
                new_line.push_str(&line[..leading_spaces]);
                new_line.push('•');
                new_line.push_str(&line[leading_spaces + marker_len..]);
                formatted_line = process_inline_bold(&new_line);
            } else {
                // '#' and ' ' are both single-byte ASCII, so these are
                // always valid char-boundary slice points.
                let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
                let is_heading = (1..=6).contains(&hash_count)
                    && trimmed.as_bytes().get(hash_count) == Some(&b' ');
                if is_heading {
                    let leading_len = line.len() - trimmed.len();
                    let leading_spaces_str = &line[..leading_len];
                    let heading_text = trimmed[hash_count + 1..].trim_end();
                    let (style_start, style_end) = match hash_count {
                        1 => ("\x1b[1m\x1b[4m\x1b[38;2;56;189;248m", RESET),
                        2 => ("\x1b[1m\x1b[38;2;56;189;248m", RESET),
                        _ => (BRIGHT, RESET),
                    };
                    formatted_line = format!(
                        "{leading_spaces_str}{style_start}{}{style_end}",
                        process_inline_bold(heading_text)
                    );
                } else {
                    formatted_line = process_inline_bold(&formatted_line);
                }
            }
        } else {
            // Do not reformat markdown inside code blocks — only add a left
            // gutter bar so the block reads as visually distinct from
            // prose, matching the border drawn around it. The content itself
            // is syntax-highlighted when the fence's language hint matched a
            // known syntax; otherwise it's printed as-is, same as before.
            let content = match highlighter.as_mut() {
                Some(h) => highlight_code_line(h, line),
                None => line.to_string(),
            };
            formatted_line = format!("{CODE_BORDER}│{RESET} {content}{RESET}");
        }

        formatted_lines.push(formatted_line);
    }

    let mut result = formatted_lines.join("\n");
    if text.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn process_inline_bold(text: &str) -> String {
    let count = text.matches("**").count();
    let pair_limit = (count / 2) * 2;
    let mut result = String::with_capacity(text.len());
    let parts = text.split("**");
    let mut is_bold = false;
    let mut processed_markers = 0;
    for part in parts {
        if is_bold && processed_markers < pair_limit {
            result.push_str(BLUE);
            result.push_str(part);
            result.push_str(RESET);
        } else {
            result.push_str(part);
        }
        processed_markers += 1;
        is_bold = !is_bold;
    }
    result
}

/// Replace common LaTeX math symbols with Unicode equivalents.
/// Fixes garbled output like "ightarrow$" from models that emit LaTeX notation.
fn sanitize_latex(text: &str) -> String {
    let mut s = text.to_owned();
    for (pat, uni) in [
        // arrows
        ("$\\rightarrow$", "→"),
        ("\\rightarrow", "→"),
        ("ightarrow", "→"),
        ("$\\leftarrow$", "←"),
        ("\\leftarrow", "←"),
        ("eftarrow", "←"),
        ("$\\Rightarrow$", "⇒"),
        ("\\Rightarrow", "⇒"),
        ("$\\Leftarrow$", "⇐"),
        ("\\Leftarrow$", "⇐"),
        ("$\\leftrightarrow$", "↔"),
        ("\\leftrightarrow", "↔"),
        // comparison
        ("$\\leq$", "≤"),
        ("\\leq", "≤"),
        ("$\\geq$", "≥"),
        ("\\geq", "≥"),
        ("$\\neq$", "≠"),
        ("\\neq", "≠"),
        ("$\\approx$", "≈"),
        ("\\approx", "≈"),
        // math
        ("$\\times$", "×"),
        ("\\times", "×"),
        ("$\\div$", "÷"),
        ("\\div", "÷"),
        ("$\\pm$", "±"),
        ("\\pm", "±"),
        ("$\\infty$", "∞"),
        ("\\infty", "∞"),
        ("$\\cdot$", "·"),
        ("\\cdot", "·"),
        // sets
        ("$\\in$", "∈"),
        ("$\\subset$", "⊂"),
        ("$\\cup$", "∪"),
        ("$\\cap$", "∩"),
    ] {
        s = s.replace(pat, uni);
    }
    s
}

#[cfg(test)]
mod format_markdown_bold_tests {
    use super::*;

    /// Strips ANSI escape sequences so a test can check for literal text without caring
    /// whether syntax highlighting split it into several differently-colored spans.
    fn strip_ansi(s: &str) -> String {
        let mut out = String::with_capacity(s.len());
        let mut chars = s.chars().peekable();
        while let Some(c) = chars.next() {
            if c == '\x1b' && chars.peek() == Some(&'[') {
                chars.next();
                for c2 in chars.by_ref() {
                    if c2.is_ascii_alphabetic() {
                        break;
                    }
                }
                continue;
            }
            out.push(c);
        }
        out
    }

    #[test]
    fn h1_header_marker_is_stripped_and_styled() {
        let out = format_markdown_bold("# Title");
        assert!(!out.contains('#'), "marker should be stripped: {out}");
        assert!(out.contains("Title"));
        assert!(out.contains("\x1b[4m"), "H1 should be underlined: {out}");
    }

    #[test]
    fn h1_and_h2_are_visually_distinct() {
        let h1 = format_markdown_bold("# One");
        let h2 = format_markdown_bold("## Two");
        assert_ne!(h1.replace("One", ""), h2.replace("Two", ""));
    }

    #[test]
    fn code_block_gets_borders_and_language_label() {
        let out = format_markdown_bold("```python\nprint(1)\n```");
        assert!(out.contains('┌'), "missing top border: {out}");
        assert!(out.contains('└'), "missing bottom border: {out}");
        assert!(out.contains("python"), "missing language label: {out}");
        assert!(
            strip_ansi(&out).contains("print(1)"),
            "code content dropped: {out}"
        );
        assert!(out.contains('│'), "missing gutter bar: {out}");
    }

    #[test]
    fn code_comment_starting_with_hash_is_not_treated_as_a_header() {
        let out = format_markdown_bold("```\n# not a header\n```");
        assert!(
            strip_ansi(&out).contains("# not a header"),
            "code content mangled: {out}"
        );
    }

    #[test]
    fn known_language_gets_syntax_highlighted() {
        let out = format_markdown_bold("```python\nprint(1)\n```");
        // A recognized language should split the line into multiple differently-colored
        // ANSI spans rather than the single gutter-only color the fallback path uses.
        let color_codes = out.matches("\x1b[38;2;").count();
        assert!(
            color_codes > 2,
            "expected multiple highlight colors for a known language, got {color_codes}: {out}"
        );
    }

    #[test]
    fn unrecognized_language_falls_back_to_plain_code() {
        let out = format_markdown_bold("```not-a-real-language\nsome text\n```");
        assert!(
            strip_ansi(&out).contains("some text"),
            "code content dropped: {out}"
        );
    }

    #[test]
    fn fenceless_code_block_falls_back_to_plain_code() {
        let out = format_markdown_bold("```\nsome text\n```");
        assert!(
            strip_ansi(&out).contains("some text"),
            "code content dropped: {out}"
        );
    }
}

#[cfg(test)]
mod command_output_preview_tests {
    use super::*;

    #[test]
    fn caps_at_seven_lines_and_notes_the_rest() {
        let stdout: String = (1..=25).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
        let result = format!("exit: 0\nmode: readOnly\nsandboxed: false\nstdout:\n{stdout}\nstderr:\n");

        let preview = command_output_preview(&result);

        assert_eq!(preview.len(), 8, "7 lines + 1 truncation-note line: {preview:?}");
        assert_eq!(preview[0], "exit: 0");
        assert_eq!(preview[6], "line 5");
        assert_eq!(preview[7], "... 21 more lines");
    }

    #[test]
    fn a_single_very_long_line_is_cut_down_too() {
        let long_line = "x".repeat(400);
        let result = format!("exit: 0\nstdout:\n{long_line}\nstderr:\n");

        let preview = command_output_preview(&result);

        let content_line = &preview[2];
        assert!(
            content_line.chars().count() <= 121,
            "expected the long line truncated to ~120 chars, got {} chars",
            content_line.chars().count()
        );
        assert!(content_line.ends_with('…'));
    }

    #[test]
    fn short_output_is_left_untouched() {
        let result = "exit: 0\nstdout:\nhello\nstderr:\n";
        assert_eq!(
            command_output_preview(result),
            vec!["exit: 0", "stdout:", "hello", "stderr:"]
        );
    }
}

#[cfg(test)]
mod skill_card_tests {
    use super::*;

    #[test]
    fn recognizes_workspace_skill_paths() {
        assert_eq!(
            skill_name_for_read_path("/repo/.agents/skills/daily-report/SKILL.md"),
            Some("daily-report".to_string())
        );
        assert_eq!(
            skill_name_for_read_path("/repo/skills/refactor-helper/skill.md"),
            Some("refactor-helper".to_string())
        );
    }

    #[test]
    fn recognizes_global_flat_skill_files() {
        assert_eq!(
            skill_name_for_read_path("/home/user/.config/mint/mint-skills/notes.md"),
            Some("notes".to_string())
        );
    }

    #[test]
    fn recognizes_global_skill_subdirectories_too() {
        assert_eq!(
            skill_name_for_read_path("/home/user/.config/mint/mint-skills/daily-report/SKILL.md"),
            Some("daily-report".to_string())
        );
    }

    #[test]
    fn an_ordinary_file_read_is_not_mistaken_for_a_skill() {
        assert_eq!(
            skill_name_for_read_path("/repo/src/main.rs"),
            None,
            "a plain source file must never render as a Skill(...) card"
        );
        assert_eq!(skill_name_for_read_path("/repo/README.md"), None);
    }

    #[test]
    fn parses_skill_names_out_of_a_memory_recall_result() {
        let result = "[Skill: daily-report]\nSome content here\n\n[Skill: refactor-helper]\nMore content";
        assert_eq!(
            skill_names_from_memory_recall(result),
            vec!["daily-report", "refactor-helper"]
        );
    }

    #[test]
    fn memory_recall_result_with_no_skill_hits_yields_nothing() {
        let result = "[2026-08-01] You: hi\nMint: hello";
        assert!(skill_names_from_memory_recall(result).is_empty());
    }
}

use std::io::{self, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use mint_core::{
    AgentApproval, AgentProgress, AgentResult, ApprovalOutcome, CHAT_CLI_ID, MintConfig,
    OrchestrationError, PermissionDecision, PermissionRule, orchestrate_agent_loop,
    permission_decision_for, save_config,
};

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
                config,
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
                config,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    let (additions, deletions) = diff_stats(diff);
                    print_diff_header("Update", path, additions, deletions);
                    print_colored_diff(diff);
                },
            ),
            AgentApproval::RunShell { command, mode } => confirm_with_persistence(
                "run_shell",
                command,
                config,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    print_approval_card(
                        "Local Shell Command",
                        &[("Command", command), ("Mode", mode)],
                    );
                },
            ),
            AgentApproval::NoteWrite { path, .. } => confirm_with_persistence(
                "note_write",
                path,
                config,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    print_approval_card("Note Creation", &[("Path", path)]);
                },
            ),
            AgentApproval::RunPlugin { name, instruction } => confirm_with_persistence(
                "run_plugin",
                name,
                config,
                root,
                &mut permission_rules,
                &approve_approval_active,
                || {
                    print_approval_card(
                        "Plugin Execution",
                        &[("Plugin", name), ("Detail", instruction)],
                    );
                },
            ),
            AgentApproval::McpTool {
                server,
                tool,
                arguments,
            } => {
                let subject = format!("{server}:{tool}");
                confirm_with_persistence(
                    "mcp_tool",
                    &subject,
                    config,
                    root,
                    &mut permission_rules,
                    &approve_approval_active,
                    || {
                        let mut fields =
                            vec![("Server", server.as_str()), ("Tool", tool.as_str())];
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
                        "Thinking ({} • Esc to interrupt)",
                        format_elapsed(timer_started_at.elapsed())
                    ));
                    render_live_status(&mut status);
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        });
    }
    let progress_live_status = Arc::clone(&live_status);
    let progress_cb = |progress: AgentProgress| match progress {
        AgentProgress::Thinking {
            elapsed_secs,
            agent_name,
            model_name,
        } => {
            if !options.fast_mode
                && let Ok(mut status) = progress_live_status.lock()
            {
                let label = if let (Some(a), Some(m)) = (agent_name, model_name) {
                    format!(
                        "{} ({}) is thinking ({} • Esc to interrupt)",
                        a,
                        m,
                        format_elapsed(Duration::from_secs(elapsed_secs))
                    )
                } else {
                    format!(
                        "Thinking ({} • Esc to interrupt)",
                        format_elapsed(Duration::from_secs(elapsed_secs))
                    )
                };
                status.thinking = Some(label);
                render_live_status(&mut status);
            }
        }
        AgentProgress::Thought { thought } => {
            if !options.fast_mode
                && let Ok(mut status) = progress_live_status.lock()
            {
                commit_activity_snapshot(&mut status);
                print_timeline_note(&thought);
                status.thinking = None;
                render_live_status(&mut status);
            }
        }
        AgentProgress::ToolStart { action, input } => {
            if !options.fast_mode {
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
                        (
                            false,
                            format!("[run_shell] Running command: `{}`...", command),
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
            if !options.fast_mode {
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
                    println!("  {DIM}{}.{RESET} {BLUE}{}{RESET} {DIM}({}){RESET} {CYAN}[+{} extra]{RESET}", i + 1, first_title, domain, extra_count);
                } else {
                    println!("  {DIM}{}.{RESET} {BLUE}{}{RESET} {DIM}({}){RESET}", i + 1, first_title, domain);
                }
                println!("     {DIM}{}{RESET}", first_url);
            }
        }

        println!();
    };

    let agent_loop = orchestrate_agent_loop(
        config,
        task,
        root,
        image_data_uri,
        video_data_uri,
        Some(CHAT_CLI_ID),
        None,
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
    hostname.strip_prefix("www.").unwrap_or(hostname).to_lowercase()
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
    let content_len = crate::interactive::string_visual_width(content);
    let pad_len = term_width.saturating_sub(prefix_visible_len + content_len);
    println!(
        "  {DIM}{line_num_str}{RESET} {bg}{content}{}{RESET}",
        " ".repeat(pad_len)
    );
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
    rendered_lines: usize,
    spinner_tick: usize,
    /// Sources collected from web_search ToolEnd results (title, url)
    web_sources: Vec<(String, String)>,
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
    clear_live_status(status);
    let mut lines = Vec::new();
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let is_thinking = status.thinking.is_some();
    let tick = status.spinner_tick;

    lines.extend(plan_lines(&status.plan_steps, is_thinking, tick));
    lines.extend(tasks_lines(&status.tasks[tasks_start..], is_thinking, tick));
    lines.extend(activities_lines(
        &status.activities[activities_start..],
        is_thinking,
        tick,
    ));
    lines.extend(explored_lines(
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

        let custom_thinking = if thinking.contains("is thinking") {
            thinking.replace("is thinking", &format!("is thinking{:<3}", dots))
        } else {
            thinking.replace("Thinking", &format!("Thinking{:<3}", dots))
        };

        let waved_thinking = apply_wave_effect(&custom_thinking, status.spinner_tick);

        lines.push(format!("  {MINT}{frame}{RESET} {waved_thinking}"));
    }
    if lines.is_empty() {
        return;
    }

    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;

    let mut physical_lines_count = 0;
    for line in &lines {
        let stripped = strip_ansi_escapes(line);
        let line_len = stripped.chars().filter(|&c| !is_thai_combining(c)).count();
        let physical_lines = if width > 0 {
            line_len.div_ceil(width)
        } else {
            1
        }
        .max(1);
        physical_lines_count += physical_lines;
        println!("{line}");
    }
    status.rendered_lines = physical_lines_count;
    let _ = io::stdout().flush();
}

fn commit_activity_snapshot(status: &mut LiveStatus) {
    clear_live_status(status);
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let mut lines = explored_lines(&status.explored[explored_start..], false, 0);
    lines.extend(activities_lines(
        &status.activities[activities_start..],
        false,
        0,
    ));
    lines.extend(tasks_lines(&status.tasks[tasks_start..], false, 0));
    if lines.is_empty() {
        return;
    }
    for line in &lines {
        println!("{line}");
    }
    print_timeline_separator();
    status.committed_explored = status.explored.len();
    status.committed_activities = status.activities.len();
    status.committed_tasks = status.tasks.len();
    let _ = io::stdout().flush();
}

fn print_timeline_note(thought: &str) {
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
    println!("{DIM}{wrapped}{RESET}");
}

fn print_timeline_separator() {
    let (term_width, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = term_width as usize;
    println!("\n{DIM}{}{RESET}\n", "─".repeat(width));
}

fn clear_live_status(status: &mut LiveStatus) {
    if status.rendered_lines == 0 {
        clear_working_status();
        return;
    }
    for _ in 0..status.rendered_lines {
        print!("\x1b[1A\r\x1b[2K");
    }
    status.rendered_lines = 0;
    let _ = io::stdout().flush();
}

fn get_bullet(name: &str, is_thinking: bool, tick: usize) -> String {
    let char_str = if is_thinking {
        if (tick / 4).is_multiple_of(2) {
            "●"
        } else {
            "○"
        }
    } else {
        "●"
    };

    match name {
        "plan" => format!("{BLUE}{char_str}{RESET} plan"),
        "tasks" => format!("{MINT}{char_str}{RESET} tasks"),
        "activity" => format!("{BLUE}{char_str}{RESET} activity"),
        "explored" => format!("{DIM}{char_str}{RESET} explored"),
        _ => char_str.to_string(),
    }
}

fn explored_lines(actions: &[ExploredAction], is_thinking: bool, tick: usize) -> Vec<String> {
    if actions.is_empty() {
        return Vec::new();
    }
    let grouped = grouped_explored_actions(actions);
    let mut lines = vec![format!("  {}", get_bullet("explored", is_thinking, tick))];
    lines.extend(grouped.iter().take(24).enumerate().map(|(index, action)| {
        let prefix = if index == 0 { "    └" } else { "     " };
        format!("{DIM}{prefix} {action}{RESET}")
    }));
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

fn command_was_run(result: &str) -> bool {
    result.lines().any(|line| line.starts_with("exit: "))
}

/// Truncated preview of a command's raw stdout/stderr, shown indented under its
/// "Finished command" label. Drops internal bookkeeping lines (`mode:`, `sandboxed:`)
/// and caps the output so a noisy command can't flood the terminal.
fn command_output_preview(result: &str) -> Vec<String> {
    const MAX_LINES: usize = 8;

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
        .map(|s| s.to_string())
        .collect();
    if trimmed.len() > MAX_LINES {
        preview.push(format!("... {} more lines", trimmed.len() - MAX_LINES));
    }
    preview
}

fn activities_lines(activities: &[String], is_thinking: bool, tick: usize) -> Vec<String> {
    if activities.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("  {}", get_bullet("activity", is_thinking, tick))];
    lines.extend(activities.iter().take(24).enumerate().map(|(index, act)| {
        let prefix = if index == 0 { "    └" } else { "     " };
        format!("{DIM}{prefix} {act}{RESET}")
    }));
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

fn tasks_lines(tasks: &[TaskEntry], is_thinking: bool, tick: usize) -> Vec<String> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("  {}", get_bullet("tasks", is_thinking, tick))];
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
        if approval_active.load(Ordering::Relaxed) {
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
/// file edits, a plain field list for everything else) and shows a 4-option
/// prompt; an "Always" choice appends the new rule to `permission_rules` and
/// persists it via `save_config` so it survives past this process, in
/// addition to taking effect for the rest of this run.
#[allow(clippy::too_many_arguments)]
fn confirm_with_persistence(
    tool: &str,
    subject: &str,
    config: &MintConfig,
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
    let choice = prompt_persistent_approval();
    approval_active.store(false, Ordering::Relaxed);

    match choice {
        0 => Ok(ApprovalOutcome::Approved),
        1 | 2 => {
            let rule = PermissionRule {
                tool: tool.to_string(),
                pattern: subject.to_string(),
                decision: PermissionDecision::Allow,
                project_root: if choice == 1 {
                    Some(root.to_path_buf())
                } else {
                    None
                },
            };
            permission_rules.push(rule.clone());
            let mut updated = config.clone();
            updated.permission_rules = permission_rules.clone();
            if let Err(error) = save_config(&updated) {
                eprintln!("  {DIM}Warning: could not save permission rule: {error}{RESET}");
            }
            Ok(ApprovalOutcome::Approved)
        }
        _ => Ok(ApprovalOutcome::Denied),
    }
}

/// Renders the 4-option approval choice and returns its index (0 = Once,
/// 1 = Always this project, 2 = Always everywhere, 3 = Deny). Falls back to a
/// plain stdin line read (matching `crate::confirm`'s non-TTY behavior, e.g.
/// piped/CI invocations) when not running in a real terminal, rather than
/// silently defaulting to deny like the underlying `prompt_interactive_select`
/// does on its own.
fn prompt_persistent_approval() -> usize {
    use crossterm::tty::IsTty;

    let options = [
        "Approve (Once)".to_string(),
        "Approve (Always \u{2014} this project)".to_string(),
        "Approve (Always \u{2014} everywhere)".to_string(),
        "Deny".to_string(),
    ];

    if !io::stdout().is_tty() {
        print!("  Approve? [o]nce / [p]roject-always / [g]lobal-always / [N]deny: ");
        let _ = io::stdout().flush();
        let mut answer = String::new();
        if io::stdin().read_line(&mut answer).is_err() {
            return 3;
        }
        return match answer.trim().to_ascii_lowercase().as_str() {
            "o" | "once" | "y" | "yes" => 0,
            "p" | "project" => 1,
            "g" | "global" | "always" => 2,
            _ => 3,
        };
    }

    match crate::interactive::prompt_interactive_select(
        "Approve this action?",
        &options,
        &options[0],
    ) {
        Ok(Some(choice)) => options.iter().position(|o| *o == choice).unwrap_or(3),
        _ => 3,
    }
}

fn confirm_pausing_interrupt(prompt: &str, approval_active: &AtomicBool) -> bool {
    approval_active.store(true, Ordering::Relaxed);
    let approved = crate::confirm(prompt).unwrap_or(false);
    approval_active.store(false, Ordering::Relaxed);
    approved
}

fn format_markdown_bold(text: &str) -> String {
    let mut formatted_lines = Vec::new();
    let mut in_code_block = false;

    for line in text.lines() {
        let mut formatted_line = line.to_string();
        let trimmed = line.trim_start();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
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
                let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
                if hash_count > 0 && hash_count <= 6 {
                    let next_char = trimmed.chars().nth(hash_count);
                    let is_heading = match next_char {
                        Some(' ') => true,
                        Some(c) => !c.is_alphanumeric(),
                        None => true,
                    };
                    if is_heading {
                        // Make the entire heading line bold and bright white
                        formatted_line = format!("{}{}{}", BRIGHT, line, RESET);
                    } else {
                        formatted_line = process_inline_bold(&formatted_line);
                    }
                } else {
                    formatted_line = process_inline_bold(&formatted_line);
                }
            }
        } else {
            // Do not format markdown inside code blocks
            formatted_line = line.to_string();
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

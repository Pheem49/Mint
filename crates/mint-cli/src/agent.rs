use std::io::{self, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::{Duration, Instant};

use ansi_to_tui::IntoText;
use anyhow::{Result, anyhow};
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

mod approval_prompts;
mod diff_render;
mod live_status;
mod markdown_render;
use approval_prompts::*;
use diff_render::*;
pub(crate) use live_status::*;
use markdown_render::*;

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
    /// Keeps a lightweight, typeable input box pinned under the live status
    /// region for the duration of this turn. Enter queues the typed text
    /// (into the `Arc<Mutex<Vec<String>>>` passed to
    /// `run_code_agent_with_options`) rather than sending it immediately —
    /// the caller drains that queue once this turn returns and dispatches
    /// each entry as its own follow-up turn. Only meaningful on an
    /// interactive TTY; one-shot/non-interactive callers should leave this
    /// `false`.
    pub queueing: bool,
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
        Arc::new(Mutex::new(Vec::new())),
        Arc::new(Mutex::new(None)),
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
    queued_out: Arc<Mutex<Vec<String>>>,
    draft_out: Arc<Mutex<Option<String>>>,
) -> Result<AgentResult> {
    let started_at = Instant::now();
    let thinking_verb = random_thinking_verb();
    let approval_active = Arc::new(AtomicBool::new(false));
    let agent_done = Arc::new(AtomicBool::new(false));
    // True between a tool starting and finishing — tells the periodic timer
    // below not to overwrite the live status with "Thinking (Xs)…" text
    // while a tool (e.g. a shell command) is actually the thing in flight,
    // without stopping it from still re-rendering (so the bullets keep
    // pulsing) while that's happening.
    let tool_running = Arc::new(AtomicBool::new(false));
    let live_status = Arc::new(Mutex::new(LiveStatus::default()));
    {
        use crossterm::tty::IsTty;
        if options.queueing
            && !options.fast_mode
            && io::stdout().is_tty()
            && let Ok(mut status) = live_status.lock()
        {
            status.queue_enabled = true;
            status.accepting_input = true;
            status.model_label = crate::active_model(&config.ai_provider, config).to_string();
            status.path_label = crate::interactive::format_path_with_tilde(root);
            status.plan_mode = options.plan_mode;
        }
    }
    let approve_approval_active = Arc::clone(&approval_active);
    let approve_live_status = Arc::clone(&live_status);
    // Seeded from disk, then grown in-memory as the user picks "Always allow"
    // during this run, so a rule added mid-run is honored immediately without
    // waiting for a fresh process start.
    let mut permission_rules = config.permission_rules.clone();

    let approve_cb = |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
        approve_approval_active.store(true, Ordering::Relaxed);
        // Synchronous, not polled: `wait_for_escape_interrupt` holds raw
        // mode continuously while the queueing box is live (see its docs),
        // and only reacts to `approval_active` on its next ~15ms tick. Every
        // print below needs cooked mode's `\n` → `\r\n` translation *before*
        // it happens, not up to a tick later, so this closure — which knows
        // exactly when it's about to print — drops raw mode itself instead
        // of waiting to be noticed.
        let _ = crossterm::terminal::disable_raw_mode();
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
            AgentApproval::EnterPlanMode { reason } => {
                print_approval_card("Enter Plan Mode?", &[("Reason", reason)]);
                plan_mode_option_picker("Yes, switch to plan mode", "No, proceed directly")
            }
            AgentApproval::ExitPlanMode { plan } => {
                print_approval_card("Review Plan", &[("Plan", plan)]);
                plan_mode_option_picker("Yes, approve and start implementing", "No, keep planning")
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
    let timer_tool_running = Arc::clone(&tool_running);
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
                    if !timer_tool_running.load(Ordering::Relaxed) {
                        status.thinking = Some(format!(
                            "{thinking_verb} ({} • Esc to interrupt)",
                            format_elapsed(timer_started_at.elapsed())
                        ));
                    }
                    render_live_status(&mut status);
                }
                tokio::time::sleep(Duration::from_millis(150)).await;
            }
        });
    }
    let progress_live_status = Arc::clone(&live_status);
    let progress_approval_active = Arc::clone(&approval_active);
    let progress_tool_running = Arc::clone(&tool_running);
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
            progress_tool_running.store(true, Ordering::Relaxed);
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
            progress_tool_running.store(false, Ordering::Relaxed);
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
            // Stop accepting keystrokes for the queueing box before it's torn
            // down below — otherwise a keypress landing between this
            // `clear_live_status` and the final answer's plain `println!`
            // would resurrect the box (via `render_live_status`'s lazy
            // `InlineTui::ensure`) at whatever the cursor's current position
            // happens to be, underneath the answer that just printed.
            status.accepting_input = false;
            commit_activity_snapshot(&mut status);
            clear_live_status(&mut status);
        }
        // Same reasoning as `approve_cb`: drop raw mode synchronously,
        // right here, rather than leaving `wait_for_escape_interrupt` to
        // notice `accepting_input` went false on its next tick — the prints
        // below need cooked mode's `\n` → `\r\n` translation immediately.
        let _ = crossterm::terminal::disable_raw_mode();
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
            _ = wait_for_escape_interrupt(Arc::clone(&approval_active), Arc::clone(&live_status)) => {
                Err(OrchestrationError::Agent("interrupted by Esc".into()))
            }
        }
    };
    agent_done.store(true, Ordering::Relaxed);
    if !options.fast_mode
        && let Ok(mut status) = live_status.lock()
    {
        status.thinking = None;
        status.accepting_input = false;
        if let Ok(mut out) = queued_out.lock() {
            *out = status.queued.clone();
        }
        if let Ok(mut out) = draft_out.lock() {
            *out = if status.draft.is_empty() {
                None
            } else {
                Some(status.draft.iter().collect())
            };
        }
        if res.is_err() {
            commit_activity_snapshot(&mut status);
        }
        clear_live_status(&mut status);
    }
    let res = res.map_err(|e| anyhow!("{}", e))?;

    if should_show_verification(&res.verification) {
        println!("  Verification: {}", res.verification);
    }
    let badge_plain = if let Some(fb_provider) = &res.fallback {
        format!(
            "{} • {} → fallback: {} • {}",
            config.ai_provider,
            crate::active_model(&config.ai_provider, config),
            fb_provider,
            res.model
        )
    } else {
        format!("{} • {}", res.provider, res.model)
    };

    // "─ Worked for {elapsed} • {provider} • {model}" as one *labeled*
    // divider — filled out with more "─" to the same width the box's own
    // two divider lines use — rather than the provider/model badge and the
    // elapsed-time label as two separate short lines followed by a third,
    // unlabeled full-width divider directly under them. The three used to
    // look like unrelated elements stacked on top of each other; folding
    // both labels into one rule reads as a single line doing all three
    // jobs, matching the box below it.
    let (tw, _) = markdown::terminal_size_or_default();
    let width = tw as usize;
    let label = format!(
        "─ Worked for {} • {badge_plain}",
        format_elapsed(started_at.elapsed())
    );
    let fill_len = width
        .saturating_sub(2)
        .saturating_sub(label.chars().count() + 1);
    println!("  {DIM}{label} {}{RESET}", "─".repeat(fill_len));

    Ok(res)
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
        let stdout: String = (1..=25)
            .map(|i| format!("line {i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let result =
            format!("exit: 0\nmode: readOnly\nsandboxed: false\nstdout:\n{stdout}\nstderr:\n");

        let preview = command_output_preview(&result);

        assert_eq!(
            preview.len(),
            8,
            "7 lines + 1 truncation-note line: {preview:?}"
        );
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
        let result =
            "[Skill: daily-report]\nSome content here\n\n[Skill: refactor-helper]\nMore content";
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

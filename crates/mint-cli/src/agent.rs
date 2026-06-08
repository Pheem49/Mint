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
    OrchestrationError, orchestrate_agent_loop,
};

const RESET: &str = "\x1b[0m";
const MINT: &str = "\x1b[32m";
const BLUE: &str = "\x1b[38;2;78;201;216m";
const DIM: &str = "\x1b[90m";
const BRIGHT: &str = "\x1b[1;97m";

#[derive(Debug, Clone, Copy, Default)]
pub struct AgentOptions {
    pub fast_mode: bool,
}

pub async fn run_code_agent(task: &str, root: &Path, config: &MintConfig) -> Result<AgentResult> {
    run_code_agent_with_image(task, root, config, None).await
}

pub async fn run_code_agent_with_image(
    task: &str,
    root: &Path,
    config: &MintConfig,
    image_data_uri: Option<String>,
) -> Result<AgentResult> {
    run_code_agent_with_options(task, root, config, image_data_uri, AgentOptions::default()).await
}

pub async fn run_code_agent_with_options(
    task: &str,
    root: &Path,
    config: &MintConfig,
    image_data_uri: Option<String>,
    options: AgentOptions,
) -> Result<AgentResult> {
    let started_at = Instant::now();
    let approval_active = Arc::new(AtomicBool::new(false));
    let agent_done = Arc::new(AtomicBool::new(false));
    let live_status = Arc::new(Mutex::new(LiveStatus::default()));
    let approve_approval_active = Arc::clone(&approval_active);
    let approve_live_status = Arc::clone(&live_status);

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
            AgentApproval::WriteFile { diff, .. } => {
                println!("  Proposed edit");
                print_colored_diff(diff);
                if confirm_pausing_interrupt("Approve file edit? [y/N]", &approve_approval_active) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::ApplyPatch { diff, .. } => {
                println!("  Proposed edit");
                print_colored_diff(diff);
                if confirm_pausing_interrupt("Approve file edit? [y/N]", &approve_approval_active) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::RunShell { command, mode } => {
                println!("  {BLUE}• Proposed command{RESET}");
                println!("    mode: {}", mode);
                println!("    {}", command);
                if confirm_pausing_interrupt(
                    "Approve local shell execution? [y/N]",
                    &approve_approval_active,
                ) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::NoteWrite { path, .. } => {
                println!("  {BLUE}• Proposed note write{RESET}");
                println!("    {}", path);
                if confirm_pausing_interrupt(
                    "Approve writing this note? [y/N]",
                    &approve_approval_active,
                ) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::RunPlugin { name, instruction } => {
                println!("    Run plugin {}: {}", name, instruction);
                if confirm_pausing_interrupt(
                    &format!("Approve running plugin '{}'? [y/N]", name),
                    &approve_approval_active,
                ) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::McpTool { server, tool, .. } => {
                println!("  Called MCP tool");
                println!("    {} {}", server, tool);
                if confirm_pausing_interrupt(
                    "Approve MCP tool call? [y/N]",
                    &approve_approval_active,
                ) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::UserApproval { title, prompt } => {
                println!("  {BLUE}• Approval requested:{RESET} {}", title);
                println!("    {}", prompt);
                if confirm_pausing_interrupt(
                    "Approve this request? [y/N]",
                    &approve_approval_active,
                ) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::AskUser { question } => {
                println!("  {BLUE}• Question from agent{RESET}");
                println!("    {}", question);
                print!("Answer (leave empty to decline): ");
                let _ = std::io::Write::flush(&mut std::io::stdout());
                let mut answer = String::new();
                match std::io::stdin().read_line(&mut answer) {
                    Ok(_) if !answer.trim().is_empty() => {
                        Ok(ApprovalOutcome::Intercepted(answer.trim().to_owned()))
                    }
                    Ok(_) => Ok(ApprovalOutcome::Denied),
                    Err(error) => Err(error.to_string()),
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
                tokio::time::sleep(Duration::from_secs(1)).await;
            }
        });
    }
    let progress_live_status = Arc::clone(&live_status);
    let progress_cb = |progress: AgentProgress| match progress {
        AgentProgress::Thinking { elapsed_secs } => {
            if !options.fast_mode {
                if let Ok(mut status) = progress_live_status.lock() {
                    status.thinking = Some(format!(
                        "Thinking ({} • Esc to interrupt)",
                        format_elapsed(Duration::from_secs(elapsed_secs))
                    ));
                    render_live_status(&mut status);
                }
            }
        }
        AgentProgress::Thought { thought } => {
            if !options.fast_mode {
                if let Ok(mut status) = progress_live_status.lock() {
                    commit_activity_snapshot(&mut status);
                    print_timeline_note(&thought);
                    status.thinking = None;
                    render_live_status(&mut status);
                }
            }
        }
        AgentProgress::ToolStart { action, input } => {
            if !options.fast_mode {
                if action == "create_plan" || action == "update_plan" {
                    if let Some(steps) = extract_plan_steps(&input) {
                        if let Ok(mut status) = progress_live_status.lock() {
                            status.thinking = None;
                            status.plan_steps = steps;
                            render_live_status(&mut status);
                        }
                        return;
                    }
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
                        status.tasks.push(label);
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
                    if let Some(steps) = extract_plan_steps(&input) {
                        if let Ok(mut status) = progress_live_status.lock() {
                            status.thinking = None;
                            status.plan_steps = steps;
                            render_live_status(&mut status);
                        }
                    }
                } else if command_was_run(&result)
                    && let Some(commands) = ran_command_labels(&action, &input)
                    && let Ok(mut status) = progress_live_status.lock()
                {
                    status.thinking = None;
                    for cmd in commands {
                        status.tasks.push(format!("Finished command: `{}`", cmd));
                    }
                    render_live_status(&mut status);
                }
            }
        }
    };

    let chunk_live_status = Arc::clone(&live_status);
    let on_chunk = |summary: String| {
        if !options.fast_mode {
            if let Ok(mut status) = chunk_live_status.lock() {
                status.thinking = None;
                commit_activity_snapshot(&mut status);
                clear_live_status(&mut status);
            }
        }
        let formatted_summary = format_markdown_bold(&summary);
        print!("\n{MINT}Mint:{RESET} ");
        render_live_summary(&formatted_summary);
        println!();
    };

    let agent_loop = orchestrate_agent_loop(
        config,
        task,
        root,
        image_data_uri,
        Some(CHAT_CLI_ID),
        options.fast_mode,
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
    if !options.fast_mode {
        if let Ok(mut status) = live_status.lock() {
            status.thinking = None;
            if res.is_err() {
                commit_activity_snapshot(&mut status);
            }
            clear_live_status(&mut status);
        }
    }
    let res = res.map_err(|e| anyhow!("{}", e))?;

    if should_show_verification(&res.verification) {
        println!("Verification: {}", res.verification);
    }
    println!(
        "{DIM}─ Worked for {}{RESET}",
        format_elapsed(started_at.elapsed())
    );

    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    println!("{DIM}{}{RESET}", "─".repeat(width));

    Ok(res)
}

fn clear_working_status() {
    print!("\r\x1b[2K");
    let _ = io::stdout().flush();
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
    let mut chunk = String::new();
    for character in summary.chars() {
        chunk.push(character);
        if chunk.chars().count() >= 96 {
            print!("{chunk}");
            let _ = io::stdout().flush();
            chunk.clear();
        }
    }
    if !chunk.is_empty() {
        print!("{chunk}");
        let _ = io::stdout().flush();
    }
}

fn print_colored_diff(diff: &str) {
    for line in diff.lines() {
        if line.starts_with("@@") {
            println!("{BLUE}{line}{RESET}");
        } else if line.starts_with("--- ") || line.starts_with("+++ ") {
            println!("{DIM}{line}{RESET}");
        } else if line.starts_with('-') {
            println!("\x1b[31m{line}\x1b[0m");
        } else if line.starts_with('+') {
            println!("\x1b[32m{line}\x1b[0m");
        } else {
            println!("{line}");
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
    tasks: Vec<String>,
    plan_steps: Vec<String>,
    committed_explored: usize,
    committed_activities: usize,
    committed_tasks: usize,
    rendered_lines: usize,
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
        "read_file" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|path| ExploredAction {
                kind: "[read_file] Read",
                target: display_tool_target(path),
            }),
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

fn render_live_status(status: &mut LiveStatus) {
    clear_live_status(status);
    let mut lines = Vec::new();
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    lines.extend(plan_lines(&status.plan_steps));
    lines.extend(tasks_lines(&status.tasks[tasks_start..]));
    lines.extend(activities_lines(&status.activities[activities_start..]));
    lines.extend(explored_lines(&status.explored[explored_start..]));
    if let Some(thinking) = &status.thinking {
        lines.push(format!("{MINT}●{RESET} {BRIGHT}{thinking}{RESET}"));
    }
    if lines.is_empty() {
        return;
    }
    for line in &lines {
        println!("{line}");
    }
    status.rendered_lines = lines.len();
    let _ = io::stdout().flush();
}

fn commit_activity_snapshot(status: &mut LiveStatus) {
    clear_live_status(status);
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let mut lines = explored_lines(&status.explored[explored_start..]);
    lines.extend(activities_lines(&status.activities[activities_start..]));
    lines.extend(tasks_lines(&status.tasks[tasks_start..]));
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
    println!("{DIM}• {thought}{RESET}");
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

fn explored_lines(actions: &[ExploredAction]) -> Vec<String> {
    if actions.is_empty() {
        return Vec::new();
    }
    let grouped = grouped_explored_actions(actions);
    let mut lines = vec![format!("{BLUE}○ explored{RESET}")];
    lines.extend(grouped.iter().take(24).enumerate().map(|(index, action)| {
        let prefix = if index == 0 { "  └" } else { "   " };
        format!("{DIM}{prefix} {action}{RESET}")
    }));
    if grouped.len() > 24 {
        lines.push(format!("{DIM}   ... {} more{RESET}", grouped.len() - 24));
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

fn activities_lines(activities: &[String]) -> Vec<String> {
    if activities.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("{BLUE}◇ activity{RESET}")];
    lines.extend(activities.iter().take(24).enumerate().map(|(index, act)| {
        let prefix = if index == 0 { "  └" } else { "   " };
        format!("{DIM}{prefix} {act}{RESET}")
    }));
    if activities.len() > 24 {
        lines.push(format!("{DIM}   ... {} more{RESET}", activities.len() - 24));
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

fn plan_lines(steps: &[String]) -> Vec<String> {
    if steps.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("{BLUE}● plan{RESET}")];
    for (index, step) in steps.iter().enumerate() {
        let prefix = if index == steps.len() - 1 { "  └" } else { "  ├" };
        let (checked, text) = if step.to_lowercase().starts_with("done:") {
            (format!("{MINT}[x]{RESET}"), step["done:".len()..].trim())
        } else if step.to_lowercase().starts_with("done: ") {
            (format!("{MINT}[x]{RESET}"), step["done: ".len()..].trim())
        } else if step.to_lowercase().starts_with("in_progress:") {
            (format!("{BLUE}[~]{RESET}"), step["in_progress:".len()..].trim())
        } else if step.to_lowercase().starts_with("in_progress: ") {
            (format!("{BLUE}[~]{RESET}"), step["in_progress: ".len()..].trim())
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

fn tasks_lines(tasks: &[String]) -> Vec<String> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("{BLUE}● tasks{RESET}")];
    lines.extend(tasks.iter().take(24).enumerate().map(|(index, task)| {
        let prefix = if index == 0 { "  └" } else { "   " };
        format!("{DIM}{prefix} {task}{RESET}")
    }));
    if tasks.len() > 24 {
        lines.push(format!("{DIM}   ... {} more{RESET}", tasks.len() - 24));
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

fn confirm_pausing_interrupt(prompt: &str, approval_active: &AtomicBool) -> bool {
    approval_active.store(true, Ordering::Relaxed);
    let approved = crate::confirm(prompt).unwrap_or(false);
    approval_active.store(false, Ordering::Relaxed);
    approved
}

fn format_markdown_bold(text: &str) -> String {
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

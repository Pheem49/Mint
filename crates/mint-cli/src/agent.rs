use std::io::{self, Write};
use std::path::Path;
use std::time::{Duration, Instant};

use anyhow::{Result, anyhow};
use mint_core::{
    AgentApproval, AgentProgress, AgentResult, ApprovalOutcome, MintConfig,
    orchestrate_agent_loop,
};

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

    let approve_cb = |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
        match approval {
            AgentApproval::WriteFile { diff, .. } => {
                println!("  Proposed edit");
                println!("{}", diff);
                if confirm("Approve file edit? [y/N]") {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::ApplyPatch { diff, .. } => {
                println!("  Proposed edit");
                println!("{}", diff);
                if confirm("Approve file edit? [y/N]") {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::RunShell { command } => {
                println!("  \x1b[96m• Proposed command\x1b[0m");
                println!("    {}", command);
                if confirm("Approve local shell execution? [y/N]") {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::NoteWrite { path, .. } => {
                println!("  \x1b[96m• Proposed note write\x1b[0m");
                println!("    {}", path);
                if confirm("Approve writing this note? [y/N]") {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::RunPlugin { name, instruction } => {
                println!("    Run plugin {}: {}", name, instruction);
                if confirm(&format!("Approve running plugin '{}'? [y/N]", name)) {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
            AgentApproval::McpTool { server, tool, .. } => {
                println!("  Called MCP tool");
                println!("    {} {}", server, tool);
                if confirm("Approve MCP tool call? [y/N]") {
                    Ok(ApprovalOutcome::Approved)
                } else {
                    Ok(ApprovalOutcome::Denied)
                }
            }
        }
    };

    let progress_cb = |progress: AgentProgress| {
        match progress {
            AgentProgress::Thinking { elapsed_secs } => {
                if !options.fast_mode {
                    print!(
                        "\r\x1b[2K\x1b[90m• Thinking ({} • Ctrl+C to interrupt)\x1b[0m",
                        format_elapsed(Duration::from_secs(elapsed_secs))
                    );
                    let _ = io::stdout().flush();
                }
            }
            AgentProgress::Thought { thought } => {
                if !options.fast_mode {
                    clear_working_status();
                    println!("\n\x1b[90m• Thinking: {}\x1b[0m", thought);
                }
            }
            AgentProgress::ToolStart { action, input } => {
                if !options.fast_mode {
                    clear_working_status();
                    let detail = match action.as_str() {
                        "web_search" => input.get("query").and_then(|v| v.as_str()).map(|s| format!("Searching the web for \"{}\"...", s)),
                        "run_shell" => input.get("command").and_then(|v| v.as_str()).map(|s| format!("Running command: `{}`...", s)),
                        "read_file" => input.get("path").and_then(|v| v.as_str()).map(|s| format!("Reading file: {}...", s)),
                        "write_file" => input.get("path").and_then(|v| v.as_str()).map(|s| format!("Writing file: {}...", s)),
                        "apply_patch" => input.get("patch").and_then(|p| p.get("path")).and_then(|v| v.as_str()).map(|s| format!("Patching file: {}...", s)),
                        "search_code" => input.get("query").and_then(|v| v.as_str()).map(|s| format!("Searching code: \"{}\"...", s)),
                        "list_files" => input.get("path").and_then(|v| v.as_str()).map(|s| format!("Listing files in: {}...", s)),
                        "run_plugin" => input.get("name").and_then(|v| v.as_str()).map(|s| format!("Running plugin: {}...", s)),
                        "mcp_tool" => input.get("tool").and_then(|v| v.as_str()).map(|s| format!("Running MCP tool: {}...", s)),
                        _ => Some(format!("Using tool: {}...", action)),
                    };
                    if let Some(msg) = detail {
                        println!("\x1b[96m• {}\x1b[0m", msg);
                    }
                }
            }
            AgentProgress::ToolEnd { .. } => {
                // Tool completed
            }
        }
    };

    let on_chunk = |summary: String| {
        clear_working_status();
        let formatted_summary = format_markdown_bold(&summary);
        print!("\n\x1b[32mMint:\x1b[0m ");
        render_live_summary(&formatted_summary);
        println!();
    };

    let res = orchestrate_agent_loop(
        config,
        task,
        root,
        image_data_uri,
        options.fast_mode,
        approve_cb,
        progress_cb,
        on_chunk,
    )
    .await
    .map_err(|e| anyhow!("{}", e))?;

    if !res.verification.is_empty() {
        println!("Verification: {}", res.verification);
    }
    println!(
        "\x1b[90m─ Worked for {}\x1b[0m",
        format_elapsed(started_at.elapsed())
    );

    let (tw, _) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    println!("\x1b[90m{}\x1b[0m", "─".repeat(width));

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

fn confirm(prompt: &str) -> bool {
    crate::confirm(prompt).unwrap_or(false)
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
            result.push_str("\x1b[96m");
            result.push_str(part);
            result.push_str("\x1b[0m");
        } else {
            result.push_str(part);
        }
        processed_markers += 1;
        is_bold = !is_bold;
    }
    result
}

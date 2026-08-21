//! Interactive `/subagent add` wizard — walks through creating a
//! `.agents/subagents/*.md` (or global `~/.config/mint/mint-agents/*.md`)
//! definition without hand-writing the frontmatter. Reuses the onboarding
//! flow's own prompt helpers (`prompt_choice`, `prompt_input`), same as
//! `cron_wizard.rs`.

use anyhow::Result;
use mint_core::{SubagentDefinition, SubagentDraft, save_subagent};
use std::path::Path;

use crate::onboard::{prompt_choice, prompt_input};

fn prompt_required(label: &str) -> Result<String> {
    loop {
        let answer = prompt_input(label, None)?;
        if !answer.trim().is_empty() {
            return Ok(answer.trim().to_string());
        }
        println!("  \x1b[31mrequired\x1b[0m");
    }
}

/// Runs the interactive wizard and creates the subagent definition file.
/// `workspace_root` being `None` (no workspace open) skips offering the
/// "Workspace" scope option, mirroring the Settings UI's own
/// disable-when-no-workspace behavior.
pub fn run_add_wizard(workspace_root: Option<&Path>) -> Result<SubagentDefinition> {
    println!("\x1b[1mNew subagent\x1b[0m");
    let name = prompt_required("Name")?;
    let description = prompt_required(
        "Description (the agent reads this to decide when to dispatch to it)",
    )?;
    let system_prompt = prompt_required("System prompt (what should this subagent do?)")?;

    let tools_raw = prompt_input(
        "Allowed tools, comma-separated (blank = full toolset)",
        None,
    )?;
    let tools = if tools_raw.trim().is_empty() {
        None
    } else {
        Some(
            tools_raw
                .split(',')
                .map(|t| t.trim().to_string())
                .filter(|t| !t.is_empty())
                .collect(),
        )
    };

    let model = prompt_input("Model override (blank = inherit)", None)?;
    let provider = prompt_input("Provider override (blank = inherit)", None)?;

    let scope = if workspace_root.is_some() {
        let scope_options = [
            "Workspace (.agents/subagents/, this project only)".to_string(),
            "Global (~/.config/mint/mint-agents/, all projects)".to_string(),
        ];
        let choice = prompt_choice("Scope", &scope_options, 0)?;
        if choice.starts_with("Workspace") {
            "workspace"
        } else {
            "global"
        }
    } else {
        "global"
    };

    let draft = SubagentDraft {
        name,
        description,
        tools,
        model: Some(model).filter(|m| !m.trim().is_empty()),
        provider: Some(provider).filter(|p| !p.trim().is_empty()),
        system_prompt,
        scope: scope.to_string(),
        previous_source_path: None,
    };
    save_subagent(&draft, workspace_root).map_err(|e| anyhow::anyhow!(e))
}

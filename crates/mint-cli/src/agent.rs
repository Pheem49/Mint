use std::{
    io::{self, Write},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use mint_core::{
    ChatRequest, CodeEdit, CodePatchHunk, KnowledgeStore, MintConfig, apply_code_edits,
    build_code_patch, build_symbol_index, index_semantic_code, list_code_files, propose_code_edits,
    read_code_file, run_shell_command, search_code, search_semantic_code, send_chat,
};
use serde::Deserialize;
use serde_json::Value;

const MAX_STEPS: usize = 16;
const MAX_OBSERVATION_BYTES: usize = 16_000;
const SYSTEM_PROMPT: &str = r#"You are Mint Unified CLI Agent, a pragmatic autonomous assistant working in a local workspace.
Follow an inspect -> act -> verify loop. Return exactly one JSON object per response, with no markdown:
{"thought":"short user-visible progress note","action":"list_files|read_file|search_code|symbols|semantic_index|semantic_search|knowledge_search|mcp_tool|run_shell|verify|apply_patch|write_file|finish","input":{...}}

Input formats:
- list_files: {"path":".","limit":100}
- read_file: {"path":"relative/path","startLine":1,"endLine":240}
- search_code: {"query":"text","path":".","limit":20}
- symbols: {"path":".","limit":100}
- semantic_index: {"path":"."}
- semantic_search: {"query":"behavior description","path":".","limit":5}
- knowledge_search: {"query":"local knowledge query","limit":5}
- mcp_tool: {"server":"configured-server","tool":"tool-name","arguments":{}}
- run_shell: {"command":"non-destructive command"}
- verify: {"commands":["cargo test","npm test"]}
- apply_patch: {"patch":{"path":"relative/path","hunks":[{"oldText":"exact text","newText":"replacement"}]}}
- write_file: {"path":"relative/path","content":"full file content"}
- finish: {"summary":"concise final answer","verification":"checks run or not run"}

Rules:
0. For casual conversation or questions that need no local tool, use finish immediately.
1. Inspect the workspace before editing.
2. Use search_code before reading many files when searching for a symbol or behavior.
3. Prefer apply_patch over write_file for existing files.
4. Shell commands and file edits require user approval. Mint handles approval after you request the tool.
5. Never request destructive commands such as rm -rf, git reset --hard, git checkout --, or git clean -f.
6. Verify code changes when possible.
7. Keep thought short and concrete. Use Thai for the final summary when the task is written in Thai."#;

#[derive(Debug, Deserialize)]
struct AgentDecision {
    #[serde(default)]
    thought: String,
    action: String,
    #[serde(default)]
    input: AgentInput,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentInput {
    #[serde(default)]
    path: String,
    #[serde(default)]
    query: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    content: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    verification: String,
    #[serde(default)]
    start_line: Option<usize>,
    #[serde(default)]
    end_line: Option<usize>,
    #[serde(default)]
    limit: Option<usize>,
    #[serde(default)]
    patch: Option<AgentPatch>,
    #[serde(default)]
    server: String,
    #[serde(default)]
    tool: String,
    #[serde(default)]
    arguments: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgentPatch {
    path: PathBuf,
    #[serde(default)]
    hunks: Vec<CodePatchHunk>,
}

#[derive(Debug, Clone)]
pub struct AgentResult {
    pub summary: String,
    pub verification: String,
}

pub async fn run_code_agent(task: &str, root: &Path, config: &MintConfig) -> Result<AgentResult> {
    let root = root
        .canonicalize()
        .with_context(|| format!("unable to resolve workspace root {}", root.display()))?;
    let skills = crate::skills::context()?;
    let mut observation = initial_observation(task, &root, &skills);

    for step in 1..=MAX_STEPS {
        let response = send_chat(
            config,
            &ChatRequest {
                message: observation,
                system_instruction: SYSTEM_PROMPT.into(),
                image_data_uri: None,
                audio_data_uri: None,
            },
        )
        .await?;
        let decision = parse_decision(&response.text)?;
        if !decision.thought.trim().is_empty() {
            println!("\n{}", decision.thought.trim());
        }

        if decision.action == "finish" {
            let summary = fallback(&decision.input.summary, "Task complete.").to_owned();
            let verification = decision.input.verification.trim().to_owned();
            println!("\n{summary}");
            if !decision.input.verification.trim().is_empty() {
                println!("Verification: {verification}");
            }
            mint_core::MemoryStore::open_default()?.add_interaction(task, &summary)?;
            return Ok(AgentResult {
                summary,
                verification,
            });
        }

        let result = match execute_tool(&root, config, &decision).await {
            Ok(result) => result,
            Err(error) => {
                println!("  Tool error: {error}");
                format!("Error: {error}")
            }
        };
        observation = format!(
            "Task: {task}\nWorkspace: {}\nStep {step} completed.\nPrevious action: {}\nObservation:\n{}",
            root.display(),
            decision.action,
            truncate(&result)
        );
    }

    bail!("code agent reached the limit of {MAX_STEPS} steps")
}

fn initial_observation(task: &str, root: &Path, skills: &str) -> String {
    format!(
        "Task: {task}\nWorkspace: {}\nLearned skills:\n{}\nChoose the first action. Finish immediately for casual conversation.",
        root.display(),
        if skills.trim().is_empty() {
            "(none)"
        } else {
            skills
        }
    )
}

async fn execute_tool(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
) -> Result<String> {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" => {
            let path = workspace_path(root, &input.path)?;
            println!("  Explored");
            println!("    List {}", relative_label(root, &path));
            let files = list_code_files(&path, input.limit.unwrap_or(100), config)?;
            Ok(serde_json::to_string_pretty(&files)?)
        }
        "read_file" => {
            let path = workspace_path(root, required(&input.path, "path")?)?;
            println!("  Explored");
            println!("    Read {}", relative_label(root, &path));
            Ok(read_code_file(
                &path,
                input.start_line.unwrap_or(1),
                input.end_line.unwrap_or(240),
                config,
            )?)
        }
        "search_code" => {
            let path = workspace_path(root, &input.path)?;
            println!("  Explored");
            println!(
                "    Search {} in {}",
                required(&input.query, "query")?,
                relative_label(root, &path)
            );
            Ok(serde_json::to_string_pretty(&search_code(
                &path,
                &input.query,
                input.limit.unwrap_or(20),
                config,
            )?)?)
        }
        "symbols" => {
            let path = workspace_path(root, &input.path)?;
            println!("  Explored");
            println!("    Symbols {}", relative_label(root, &path));
            Ok(serde_json::to_string_pretty(&build_symbol_index(
                &path,
                input.limit.unwrap_or(100),
                config,
            )?)?)
        }
        "semantic_index" => {
            let path = workspace_path(root, &input.path)?;
            println!("  Explored");
            println!("    Semantic index {}", relative_label(root, &path));
            Ok(serde_json::to_string_pretty(
                &index_semantic_code(&path, config).await?,
            )?)
        }
        "semantic_search" => {
            let path = workspace_path(root, &input.path)?;
            println!("  Explored");
            println!("    Semantic search {}", required(&input.query, "query")?);
            Ok(serde_json::to_string_pretty(
                &search_semantic_code(
                    &path,
                    required(&input.query, "query")?,
                    input.limit.unwrap_or(5),
                    config,
                )
                .await?,
            )?)
        }
        "knowledge_search" => {
            println!("  Explored");
            println!("    Knowledge search {}", required(&input.query, "query")?);
            Ok(serde_json::to_string_pretty(
                &KnowledgeStore::open_default()?
                    .search(required(&input.query, "query")?, input.limit.unwrap_or(5))?,
            )?)
        }
        "mcp_tool" => {
            println!("  Called MCP tool");
            println!("    {} {}", input.server, input.tool);
            if !confirm("Approve MCP tool call? [y/N] ")? {
                return Ok(format!(
                    "User denied MCP tool call: {} {}",
                    input.server, input.tool
                ));
            }
            Ok(serde_json::to_string_pretty(&crate::mcp::call(
                required(&input.server, "server")?,
                required(&input.tool, "tool")?,
                input.arguments.clone(),
            )?)?)
        }
        "run_shell" => run_shell(root, config, required(&input.command, "command")?),
        "verify" => {
            if input.commands.is_empty() {
                bail!("verify requires at least one command");
            }
            let mut output = Vec::new();
            for command in &input.commands {
                output.push(run_shell(root, config, command)?);
            }
            Ok(output.join("\n\n"))
        }
        "apply_patch" => {
            let patch = input
                .patch
                .as_ref()
                .ok_or_else(|| anyhow!("apply_patch requires patch input"))?;
            if patch.hunks.is_empty() {
                bail!("apply_patch requires at least one hunk");
            }
            let edit = build_code_patch(root, patch.path.clone(), &patch.hunks, config)?;
            propose_and_apply(root, config, edit)
        }
        "write_file" => propose_and_apply(
            root,
            config,
            CodeEdit {
                path: PathBuf::from(required(&input.path, "path")?),
                content: input.content.clone(),
            },
        ),
        other => bail!("unsupported code-agent action '{other}'"),
    }
}

fn run_shell(root: &Path, config: &MintConfig, command: &str) -> Result<String> {
    println!("  Proposed command");
    println!("    {command}");
    if !confirm("Approve local shell execution? [y/N] ")? {
        return Ok(format!("User denied shell command: {command}"));
    }
    println!("  Ran `{command}`");
    let output = run_shell_command(command, root, true, config)?;
    Ok(format!(
        "exit: {}\nsandboxed: {}\nstdout:\n{}\nstderr:\n{}",
        output
            .status
            .map_or_else(|| "unknown".into(), |status| status.to_string()),
        output.sandboxed,
        output.stdout,
        output.stderr
    ))
}

fn propose_and_apply(root: &Path, config: &MintConfig, edit: CodeEdit) -> Result<String> {
    let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)?;
    println!("  Proposed edit");
    for preview in &proposal.edits {
        println!("{}", preview.diff);
    }
    if !confirm("Approve file edit? [y/N] ")? {
        return Ok(format!("User denied file edit: {}", edit.path.display()));
    }
    let applied = apply_code_edits(root, &[edit], &proposal.approval_token, config)?;
    println!("  Applied edit");
    Ok(serde_json::to_string_pretty(&applied)?)
}

fn parse_decision(raw: &str) -> Result<AgentDecision> {
    if let Ok(decision) = serde_json::from_str(raw) {
        return Ok(decision);
    }
    let start = raw
        .find('{')
        .ok_or_else(|| anyhow!("missing JSON object"))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| anyhow!("missing JSON object"))?;
    serde_json::from_str(&raw[start..=end])
        .context("provider did not return a valid code-agent action")
}

fn workspace_path(root: &Path, value: &str) -> Result<PathBuf> {
    let path = root.join(if value.trim().is_empty() { "." } else { value });
    let path = path
        .canonicalize()
        .with_context(|| format!("unable to resolve workspace path {}", path.display()))?;
    if !path.starts_with(root) {
        bail!("path is outside workspace: {}", path.display());
    }
    Ok(path)
}

fn relative_label(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .filter(|path| !path.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .display()
        .to_string()
}

fn required<'a>(value: &'a str, name: &str) -> Result<&'a str> {
    if value.trim().is_empty() {
        bail!("{name} is required");
    }
    Ok(value)
}

fn truncate(value: &str) -> String {
    if value.len() <= MAX_OBSERVATION_BYTES {
        value.into()
    } else {
        let mut end = MAX_OBSERVATION_BYTES;
        while !value.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}\n...<truncated>", &value[..end])
    }
}

fn fallback<'a>(value: &'a str, default: &'a str) -> &'a str {
    if value.trim().is_empty() {
        default
    } else {
        value.trim()
    }
}

fn confirm(prompt: &str) -> Result<bool> {
    print!("{prompt}");
    io::stdout().flush()?;
    let mut answer = String::new();
    io::stdin().read_line(&mut answer)?;
    Ok(matches!(
        answer.trim().to_ascii_lowercase().as_str(),
        "y" | "yes"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_wrapped_in_provider_text() {
        let action = parse_decision(
            "```json\n{\"thought\":\"inspect\",\"action\":\"list_files\",\"input\":{\"path\":\".\"}}\n```",
        )
        .unwrap();
        assert_eq!(action.action, "list_files");
        assert_eq!(action.input.path, ".");
    }

    #[test]
    fn blocks_paths_outside_workspace() {
        let root = std::env::temp_dir().join("mint-cli-agent-workspace");
        std::fs::create_dir_all(&root).unwrap();
        assert!(workspace_path(&root, "..").is_err());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn truncates_unicode_observations_on_a_character_boundary() {
        let value = "ก".repeat(MAX_OBSERVATION_BYTES);
        let truncated = truncate(&value);
        assert!(truncated.ends_with("...<truncated>"));
    }
}

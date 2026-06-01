use std::{
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

use anyhow::{Context, Result, anyhow, bail};
use mint_core::web_search as ws;
use mint_core::{
    ChatRequest, CodeEdit, CodePatchHunk, KnowledgeStore, MintConfig, apply_code_edits,
    build_code_patch, build_symbol_index, execute_native_plugin, index_semantic_code,
    list_code_files, propose_code_edits, read_code_file, run_shell_command, search_code,
    search_semantic_code, send_chat_with_fallback,
};
use serde::Deserialize;
use serde_json::Value;

const MAX_STEPS: usize = 16;
const MAX_OBSERVATION_BYTES: usize = 16_000;
const SYSTEM_PROMPT: &str = r#"You are Mint Unified CLI Agent, a pragmatic autonomous assistant working in a local workspace.
You are also Mint: a cute, warm, and helpful Thai assistant. Speak politely, naturally, and sweetly in Thai when the user writes in Thai. Refer to yourself as "มิ้น" and use polite particles such as "ค่ะ" and "นะคะ" where appropriate. Keep the personality subtle during technical work: be friendly without adding fluff or reducing precision.
Follow an inspect -> act -> verify loop. Return exactly one JSON object per response, with no markdown:
{"thought":"short user-visible progress note","action":"list_files|read_file|search_code|symbols|semantic_index|semantic_search|knowledge_search|web_search|memory_recall|note_write|run_plugin|mcp_tool|run_shell|verify|apply_patch|write_file|finish","input":{...}}

Input formats:
- list_files: {"path":".","limit":100}
- read_file: {"path":"relative/path","startLine":1,"endLine":240}
- search_code: {"query":"text","path":".","limit":20}
- symbols: {"path":".","limit":100}
- semantic_index: {"path":"."}
- semantic_search: {"query":"behavior description","path":".","limit":5}
- knowledge_search: {"query":"local knowledge query","limit":5}
- web_search: {"query":"search terms","limit":5}
- memory_recall: {"query":"what did user say about X"}
- note_write: {"path":"filename.md","content":"note content"}
- run_plugin: {"name":"gmail|google_calendar|notion|docker|spotify|obsidian|system_metrics","instruction":"instruction string"}
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
7. Use web_search when the user asks to look something up online or needs current information.
8. Use memory_recall to search past interactions before asking the user to repeat context.
9. Use note_write to save information to ~/.config/mint/notes/ when asked to remember something.
10. Use run_plugin to interact with Google Workspace (Gmail, Calendar), Notion, Docker, Obsidian, Spotify, or System Metrics.
11. Keep thought short and concrete. Use Thai for the final summary when the task is written in Thai.
12. Commands that open URLs, files, folders, or launch apps (e.g. xdg-open, open) run in the background. Once they succeed (exit: 0), you are done. Use the 'finish' action immediately."#;

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
    // note_write destination (relative to ~/.config/mint/notes/)
    #[serde(default)]
    note_path: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    instruction: String,
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
    let root = root
        .canonicalize()
        .with_context(|| format!("unable to resolve workspace root {}", root.display()))?;
    let skills = crate::skills::context()?;
    let mut observation = initial_observation(task, &root, &skills);
    let sent_image = image_data_uri.clone();
    let mut pending_image = image_data_uri;
    let mut image_saved = false;

    let mut system_prompt = SYSTEM_PROMPT.to_string();
    if let Ok(memory) = mint_core::MemoryStore::open_default() {
        if let Ok(mut interactions) = memory.recent_interactions(6) {
            interactions.reverse();
            let transcript = interactions
                .into_iter()
                .map(|item| format!("User: {}\nAssistant: {}", item.user_text, item.ai_text))
                .collect::<Vec<_>>()
                .join("\n\n");
            if !transcript.is_empty() {
                system_prompt = format!(
                    "{}\n\nRecent conversation context:\n{}",
                    system_prompt.trim(),
                    transcript
                );
            }
        }
    }

    for step in 1..=MAX_STEPS {
        let response = send_chat_with_status(
            config,
            &ChatRequest {
                message: observation.clone(),
                system_instruction: system_prompt.clone(),
                image_data_uri: pending_image.take(),
                audio_data_uri: None,
            },
            started_at,
            options,
        )
        .await?;
        if !image_saved {
            crate::image::save_sent_image_after_send(sent_image.as_deref(), task);
            image_saved = true;
        }
        let decision = match parse_decision_or_finish(&response.text) {
            Ok(decision) => decision,
            Err(error) => {
                let repaired = send_chat_with_status(
                    config,
                    &ChatRequest {
                        message: format!(
                            "Your previous response was not valid Mint agent JSON.\n\
                             Return exactly one corrected JSON object with an action and input. \
                             Do not use markdown.\n\nPrevious response:\n{}",
                            truncate(&response.text)
                        ),
                        system_instruction: system_prompt.clone(),
                        image_data_uri: None,
                        audio_data_uri: None,
                    },
                    started_at,
                    options,
                )
                .await?;
                parse_decision_or_finish(&repaired.text)
                    .with_context(|| format!("unable to repair invalid agent response: {error}"))?
            }
        };
        if !options.fast_mode && decision.action != "finish" && !decision.thought.trim().is_empty()
        {
            println!("\n\x1b[96m• Thinking:\x1b[0m {}", decision.thought.trim());
        }
        if decision.action == "finish" {
            let mut summary = decision.input.summary.trim().to_owned();
            let is_thai_task = task.chars().any(|c| ('\u{0e00}'..='\u{0e7f}').contains(&c));
            if let Some(err_line) = observation
                .lines()
                .find(|l| l.contains("Web search error:"))
            {
                let clean_err = err_line
                    .replace("Web search error: ", "")
                    .replace("Web search is currently unavailable.", "")
                    .trim()
                    .to_string();
                if summary.is_empty() {
                    if is_thai_task {
                        summary = format!(
                            "การค้นหาข้อมูลจากเว็บล้มเหลวเนื่องจากข้อผิดพลาด: {}\nมิ้นท์ขออภัยด้วยนะคะที่ไม่สามารถค้นหาข้อมูลเรียลไทม์ให้ได้ในขณะนี้ค่ะ",
                            clean_err
                        );
                    } else {
                        summary = format!(
                            "Web search failed due to error: {}\nI apologize, but I cannot retrieve real-time information at the moment.",
                            clean_err
                        );
                    }
                } else {
                    let err_lower = clean_err.to_lowercase();
                    let summary_lower = summary.to_lowercase();
                    let already_mentions_error = if is_thai_task {
                        summary_lower.contains("ล้มเหลว")
                            || summary_lower.contains("ข้อผิดพลาด")
                            || summary_lower.contains(&err_lower)
                    } else {
                        summary_lower.contains("fail")
                            || summary_lower.contains("error")
                            || summary_lower.contains(&err_lower)
                    };
                    if !already_mentions_error {
                        if is_thai_task {
                            summary.push_str(&format!(
                                "\n\n(การค้นหาเว็บล้มเหลวเนื่องจากข้อผิดพลาด: {})",
                                clean_err
                            ));
                        } else {
                            summary.push_str(&format!(
                                "\n\n(Web search failed due to error: {})",
                                clean_err
                            ));
                        }
                    }
                }
            } else {
                if summary.is_empty() {
                    observation = format!(
                        "Task: {task}\nWorkspace: {}\nStep {step} completed.\nPrevious action: {}\n\
                         Observation:\nError: Your finish action summary was empty. \
                         You MUST provide a final answer, explanation, or response to the user's query \
                         in the 'summary' field of the 'finish' action input. Do not leave it empty.",
                        root.display(),
                        decision.action
                    );
                    continue;
                }
                let mut provider_used = None;
                for line in observation.lines() {
                    if line.contains("Web search succeeded using Google Search") {
                        provider_used = Some("Google");
                    } else if line.contains("Web search succeeded using Brave Search") {
                        provider_used = Some("Brave");
                    }
                }
                if let Some(prov) = provider_used {
                    let summary_lower = summary.to_lowercase();
                    if !summary_lower.contains("google") && !summary_lower.contains("brave") {
                        if is_thai_task {
                            summary.push_str(&format!(
                                "\n\n(มิ้นท์หาข้อมูลนี้มาจาก {} Search นะคะ 💖)",
                                prov
                            ));
                        } else {
                            summary.push_str(&format!(
                                "\n\n(Information retrieved via {} Search 💖)",
                                prov
                            ));
                        }
                    }
                }
            }
            let verification = meaningful_verification(&decision.input.verification).to_owned();
            let formatted_summary = format_markdown_bold(&summary);
            print!("\n\x1b[32mMint:\x1b[0m ");
            render_live_summary(&formatted_summary);
            println!();
            if !verification.is_empty() {
                println!("Verification: {verification}");
            }
            println!(
                "\x1b[90m─ Worked for {}\x1b[0m",
                format_elapsed(started_at.elapsed())
            );
            let memory = mint_core::MemoryStore::open_default()?;
            memory.add_interaction(task, &summary)?;
            memory.save_workspace_session(&root.to_string_lossy(), &summary, &verification)?;
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

async fn send_chat_with_status(
    config: &MintConfig,
    request: &ChatRequest,
    started_at: Instant,
    options: AgentOptions,
) -> Result<mint_core::ChatResponse> {
    let response = send_chat_with_fallback(config, request);
    tokio::pin!(response);
    let mut ticker = tokio::time::interval(Duration::from_secs(1));

    loop {
        tokio::select! {
            response = &mut response => {
                if !options.fast_mode {
                    clear_working_status();
                }
                return Ok(response?.0);
            }
            _ = ticker.tick(), if !options.fast_mode => {
                print!(
                    "\r\x1b[2K\x1b[90m• Thinking ({} • Ctrl+C to interrupt)\x1b[0m",
                    format_elapsed(started_at.elapsed())
                );
                let _ = io::stdout().flush();
            }
        }
    }
}

fn clear_working_status() {
    print!("\r\x1b[2K");
    let _ = io::stdout().flush();
}

fn print_explored() {
    println!("  \x1b[96m• Explored\x1b[0m");
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

fn initial_observation(task: &str, root: &Path, skills: &str) -> String {
    let mut observation = format!(
        "Task: {task}\nWorkspace: {}\nLearned skills:\n{}\n",
        root.display(),
        if skills.trim().is_empty() {
            "(none)"
        } else {
            skills
        }
    );
    if let Ok(memory) = mint_core::MemoryStore::open_default() {
        if let Ok(Some(name)) = memory.get_profile("name") {
            observation.push_str(&format!("User Name: {name}\n"));
        }
        if let Ok(Some(session)) = memory.workspace_session(&root.to_string_lossy()) {
            observation.push_str(&format!(
                "Previous workspace session ({}):\nSummary: {}\nVerification: {}\n",
                session.updated_at,
                session.summary,
                if session.verification.trim().is_empty() {
                    "(none)"
                } else {
                    &session.verification
                }
            ));
        }
    }
    observation.push_str(&workspace_context(root));
    observation.push_str("Choose the first action. Finish immediately for casual conversation.");
    observation
}

fn workspace_context(root: &Path) -> String {
    let mut context = String::from("Automatic workspace context:\n");
    context.push_str(&format!(
        "Git status:\n{}\n",
        command_output(root, "git", &["status", "--short"])
    ));
    context.push_str(&format!(
        "Diff summary:\n{}\n",
        command_output(root, "git", &["diff", "--stat"])
    ));
    context.push_str(&format!("Package scripts:\n{}\n", package_scripts(root)));
    context
}

fn command_output(root: &Path, program: &str, args: &[&str]) -> String {
    match Command::new(program).args(args).current_dir(root).output() {
        Ok(output) if output.status.success() => {
            let value = String::from_utf8_lossy(&output.stdout);
            if value.trim().is_empty() {
                "(none)".into()
            } else {
                truncate(&value).trim().into()
            }
        }
        _ => "(unavailable)".into(),
    }
}

fn package_scripts(root: &Path) -> String {
    let Ok(raw) = std::fs::read_to_string(root.join("package.json")) else {
        return "(none)".into();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return "(invalid package.json)".into();
    };
    let Some(scripts) = value.get("scripts").and_then(Value::as_object) else {
        return "(none)".into();
    };
    scripts
        .iter()
        .map(|(name, command)| format!("{name}: {}", command.as_str().unwrap_or_default()))
        .collect::<Vec<_>>()
        .join("\n")
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

async fn execute_tool(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
) -> Result<String> {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" => {
            let path = workspace_path(root, &input.path)?;
            print_explored();
            println!("    List {}", relative_label(root, &path));
            let files = list_code_files(&path, input.limit.unwrap_or(100), config)?;
            Ok(serde_json::to_string_pretty(&files)?)
        }
        "read_file" => {
            let path = workspace_path(root, required(&input.path, "path")?)?;
            print_explored();
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
            print_explored();
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
            print_explored();
            println!("    Symbols {}", relative_label(root, &path));
            Ok(serde_json::to_string_pretty(&build_symbol_index(
                &path,
                input.limit.unwrap_or(100),
                config,
            )?)?)
        }
        "semantic_index" => {
            let path = workspace_path(root, &input.path)?;
            print_explored();
            println!("    Semantic index {}", relative_label(root, &path));
            Ok(serde_json::to_string_pretty(
                &index_semantic_code(&path, config).await?,
            )?)
        }
        "semantic_search" => {
            let path = workspace_path(root, &input.path)?;
            print_explored();
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
            print_explored();
            println!("    Knowledge search {}", required(&input.query, "query")?);
            Ok(serde_json::to_string_pretty(
                &KnowledgeStore::open_default()?
                    .search(required(&input.query, "query")?, input.limit.unwrap_or(5))?,
            )?)
        }
        "web_search" => {
            let query = required(&input.query, "query")?;
            print_explored();
            println!("    Web search: {query}");
            let limit = input.limit.unwrap_or(5);
            match ws::search(query, limit, config).await {
                Ok((hits, provider)) => {
                    if hits.is_empty() {
                        Ok("No web search results found.".to_owned())
                    } else {
                        let formatted: String = hits
                            .iter()
                            .enumerate()
                            .map(|(i, h)| {
                                format!(
                                    "{}. {}\n   URL: {}\n   {}\n",
                                    i + 1,
                                    h.title,
                                    h.url,
                                    h.snippet
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        Ok(format!(
                            "{formatted}\n\nNote: Web search succeeded using {provider} Search. In your finish summary, you MUST state that you found this information using the {provider} Search API (e.g. \"มิ้นท์ค้นหาข้อมูลนี้มาจาก {provider} Search นะคะ\")."
                        ))
                    }
                }
                Err(e) => Ok(format!(
                    "Web search error: {e}. Web search is currently unavailable. \
                         Do not try to search again. You MUST now proceed by calling the 'finish' action. \
                         In your finish summary, explain to the user in Thai that the web search failed (mentioning the search error: {e}), \
                         and then answer their query using your own pre-existing knowledge/database."
                )),
            }
        }
        "memory_recall" => {
            let query = required(&input.query, "query")?;
            print_explored();
            println!("    Memory recall: {query}");
            let query_lower = query.to_ascii_lowercase();
            let mut results = Vec::new();

            if let Ok(memory) = mint_core::MemoryStore::open_default() {
                // Search recent interactions
                if let Ok(interactions) = memory.recent_interactions(50) {
                    for item in interactions.iter().rev() {
                        if item.user_text.to_ascii_lowercase().contains(&query_lower)
                            || item.ai_text.to_ascii_lowercase().contains(&query_lower)
                        {
                            results.push(format!(
                                "[{}] You: {}\nMint: {}",
                                &item.created_at[..16.min(item.created_at.len())],
                                if item.user_text.len() > 200 {
                                    format!("{}…", &item.user_text[..200])
                                } else {
                                    item.user_text.clone()
                                },
                                if item.ai_text.len() > 200 {
                                    format!("{}…", &item.ai_text[..200])
                                } else {
                                    item.ai_text.clone()
                                },
                            ));
                            if results.len() >= 5 {
                                break;
                            }
                        }
                    }
                }

                // Search learned skills
                if let Ok(skills) = memory.learned_skills(20) {
                    for skill in &skills {
                        if skill.content.to_ascii_lowercase().contains(&query_lower)
                            || skill.name.to_ascii_lowercase().contains(&query_lower)
                        {
                            results.push(format!(
                                "[Skill: {}]\n{}",
                                skill.name,
                                if skill.content.len() > 300 {
                                    format!("{}…", &skill.content[..300])
                                } else {
                                    skill.content.clone()
                                }
                            ));
                        }
                    }
                }
            }

            if results.is_empty() {
                Ok(format!("No memory found matching: {query}"))
            } else {
                Ok(results.join("\n\n"))
            }
        }
        "note_write" => {
            let file_name = if !input.note_path.is_empty() {
                input.note_path.as_str()
            } else {
                required(&input.path, "path")?
            };
            // Sanitize: only allow simple filenames, no path traversal
            if file_name.contains("..") || file_name.contains('/') {
                bail!("note_write path must be a simple filename (no directories or ..)");
            }
            let notes_dir = dirs::config_dir()
                .ok_or_else(|| anyhow!("cannot determine config directory"))?
                .join("mint")
                .join("notes");
            let note_path = notes_dir.join(file_name);
            println!("  \x1b[96m• Proposed note write\x1b[0m");
            println!("    {}", note_path.display());
            if !confirm("Approve writing this note? [y/N] ")? {
                return Ok(format!("User denied note write: {file_name}"));
            }
            std::fs::create_dir_all(&notes_dir).with_context(|| {
                format!("cannot create notes directory: {}", notes_dir.display())
            })?;
            std::fs::write(&note_path, &input.content)
                .with_context(|| format!("cannot write note: {}", note_path.display()))?;
            println!("  Note saved: {}", note_path.display());
            Ok(format!("Note saved to {}", note_path.display()))
        }

        "run_plugin" => {
            let name = required(&input.name, "name")?;
            let instruction = required(&input.instruction, "instruction")?;
            print_explored();
            println!("    Run plugin {name}: {instruction}");
            if !confirm(&format!("Approve running plugin '{name}'? [y/N] "))? {
                return Ok(format!("User denied plugin execution: {name}"));
            }
            Ok(execute_native_plugin(config, name, instruction).await?)
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
    println!("  \x1b[96m• Proposed command\x1b[0m");
    println!("    {command}");
    if !confirm("Approve local shell execution? [y/N] ")? {
        return Ok(format!("User denied shell command: {command}"));
    }
    let output = run_shell_command(command, root, true, config)?;
    if output.success {
        println!("  \x1b[32m• Ran\x1b[0m `{command}`");
    } else {
        println!("  \x1b[31m• Failed\x1b[0m `{command}`");
    }
    let status_str = output
        .status
        .map_or_else(|| "unknown".into(), |status| status.to_string());

    let mut hint = "";
    let cmd_lower = command.to_lowercase();
    if output.success {
        if cmd_lower.contains("open")
            || cmd_lower.contains("launch")
            || cmd_lower.contains("chrome")
            || cmd_lower.contains("firefox")
        {
            hint = "\nNote: Opening URLs, files, folders, or launching applications are background processes. Even if there are warnings or stdout/stderr outputs, since the command exited successfully with status 0, the operation has succeeded and you should now use the 'finish' action to inform the user.";
        }
    }

    Ok(format!(
        "exit: {}\nsandboxed: {}\nstdout:\n{}\nstderr:\n{}{}",
        status_str, output.sandboxed, output.stdout, output.stderr, hint
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
    if let Ok(decision) = mint_core::parse_agent_json(raw) {
        return Ok(decision);
    }
    parse_shorthand_finish(raw).context("provider did not return a valid code-agent action")
}

fn parse_shorthand_finish(raw: &str) -> Result<AgentDecision> {
    let value: Value = mint_core::parse_agent_json(raw)?;
    let finish = value
        .get("finish")
        .ok_or_else(|| anyhow!("missing action"))?;
    Ok(AgentDecision {
        thought: value
            .get("thought")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        action: "finish".into(),
        input: serde_json::from_value(finish.clone())?,
    })
}

fn parse_decision_or_finish(raw: &str) -> Result<AgentDecision> {
    match parse_decision(raw) {
        Ok(decision) => Ok(decision),
        Err(_) if !raw.trim().is_empty() && !raw.contains("\"action\"") => Ok(AgentDecision {
            thought: String::new(),
            action: "finish".into(),
            input: AgentInput {
                summary: raw.trim().into(),
                ..AgentInput::default()
            },
        }),
        Err(error) => Err(error),
    }
}

fn meaningful_verification(value: &str) -> &str {
    let value = value.trim();
    if matches!(
        value.to_ascii_lowercase().as_str(),
        "" | "not run"
            | "not run."
            | "no checks run"
            | "no checks run."
            | "not_required"
            | "not required"
            | "none"
            | "n/a"
    ) {
        ""
    } else {
        value
    }
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

fn confirm(prompt: &str) -> Result<bool> {
    crate::confirm(prompt)
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
    fn formats_markdown_bold_to_cyan_ansi_codes() {
        assert_eq!(
            format_markdown_bold("Hello **world** check"),
            "Hello \x1b[96mworld\x1b[0m check"
        );
        assert_eq!(
            format_markdown_bold("No bold text here"),
            "No bold text here"
        );
        assert_eq!(
            format_markdown_bold("Unmatched **bold text"),
            "Unmatched bold text"
        );
    }

    #[test]
    fn treats_plain_provider_text_as_a_final_answer() {
        let action = parse_decision_or_finish("สวัสดีค่ะ มิ้นอยู่นี่นะคะ").unwrap();
        assert_eq!(action.action, "finish");
        assert_eq!(action.input.summary, "สวัสดีค่ะ มิ้นอยู่นี่นะคะ");
    }

    #[test]
    fn rejects_malformed_json_instead_of_treating_it_as_a_final_answer() {
        assert!(parse_decision_or_finish("{\"action\":\"finish\"").is_err());
    }

    #[test]
    fn treats_casual_text_with_braces_as_a_final_answer() {
        let action = parse_decision_or_finish("ใช้รูปแบบ {ชื่อ} ได้ครับ").unwrap();
        assert_eq!(action.action, "finish");
        assert_eq!(action.input.summary, "ใช้รูปแบบ {ชื่อ} ได้ครับ");
    }

    #[test]
    fn parses_shorthand_finish_action() {
        let action = parse_decision_or_finish(
            r#"{"thought":"greet","finish":{"summary":"hello","verification":"not_required"}}"#,
        )
        .unwrap();
        assert_eq!(action.action, "finish");
        assert_eq!(action.input.summary, "hello");
        assert_eq!(meaningful_verification(&action.input.verification), "");
    }

    #[test]
    fn hides_empty_verification_placeholders() {
        assert_eq!(meaningful_verification("not run"), "");
        assert_eq!(meaningful_verification("no checks run"), "");
        assert_eq!(meaningful_verification(" cargo test "), "cargo test");
    }

    #[test]
    fn formats_elapsed_agent_time() {
        assert_eq!(format_elapsed(Duration::from_secs(20)), "20s");
        assert_eq!(format_elapsed(Duration::from_secs(64)), "1m 04s");
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

    #[test]
    fn reads_package_scripts_for_workspace_context() {
        let root = std::env::temp_dir().join(format!(
            "mint-cli-agent-package-scripts-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join("package.json"),
            r#"{"scripts":{"test":"cargo test","build":"npm run build"}}"#,
        )
        .unwrap();
        let scripts = package_scripts(&root);
        assert!(scripts.contains("test: cargo test"));
        assert!(scripts.contains("build: npm run build"));
        let _ = std::fs::remove_dir_all(root);
    }
}

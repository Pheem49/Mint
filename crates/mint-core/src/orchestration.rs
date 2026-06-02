use std::path::{Path, PathBuf};
use std::time::Instant;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::chat::{send_chat_with_fallback, stream_chat_with_fallback};
use crate::{
    ChatError, ChatRequest, ChatResponse, MemoryError, MemoryStore, MintConfig, send_chat,
    stream_chat,
};
use crate::code_tools::{
    CodeEdit, CodePatchHunk, apply_code_edits, build_code_patch, list_code_files,
    propose_code_edits, read_code_file, search_code,
};
use crate::symbols::build_symbol_index;
use crate::semantic::{index_semantic_code, search_semantic_code};
use crate::plugins::execute_native_plugin;
use crate::shell::run_shell_command;
use crate::knowledge::KnowledgeStore;

const CONTEXT_LIMIT: usize = 6;

#[derive(Debug, Error)]
pub enum OrchestrationError {
    #[error(transparent)]
    Chat(#[from] ChatError),
    #[error(transparent)]
    Memory(#[from] MemoryError),
    #[error("agent error: {0}")]
    Agent(String),
}

pub async fn orchestrate_chat(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let response = send_chat(config, &enriched).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok(response)
}

pub async fn orchestrate_chat_stream<F>(
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: F,
) -> Result<ChatResponse, OrchestrationError>
where
    F: FnMut(String),
{
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let response = stream_chat(config, &enriched, on_chunk).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok(response)
}

pub async fn orchestrate_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), OrchestrationError> {
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let (response, fallback) = send_chat_with_fallback(config, &enriched).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok((response, fallback))
}

pub async fn orchestrate_chat_stream_with_fallback<F>(
    config: &MintConfig,
    request: &ChatRequest,
    on_chunk: F,
) -> Result<(ChatResponse, Option<String>), OrchestrationError>
where
    F: FnMut(String),
{
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(&memory, request)?;
    let (response, fallback) = stream_chat_with_fallback(config, &enriched, on_chunk).await?;
    memory.add_interaction_with_metadata(
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
    )?;
    Ok((response, fallback))
}

fn enrich_request(memory: &MemoryStore, request: &ChatRequest) -> Result<ChatRequest, MemoryError> {
    let mut interactions = memory.recent_interactions(CONTEXT_LIMIT)?;
    interactions.reverse();
    let transcript = interactions
        .into_iter()
        .map(|item| format!("User: {}\nAssistant: {}", item.user_text, item.ai_text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut enriched = request.clone();
    if !transcript.is_empty() {
        enriched.system_instruction = format!(
            "{}\n\nRecent conversation context:\n{}",
            enriched.system_instruction.trim(),
            transcript
        )
        .trim()
        .to_owned();
    }
    Ok(enriched)
}

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
6. Verify code changes when possible. If compile or test commands fail (exit status is not 0), analyze the stdout/stderr to locate the bug, edit the code to fix it, and verify again. Do not stop or give up until the errors are resolved.
7. Use web_search when the user asks to look something up online or needs current information.
8. Use memory_recall to search past interactions before asking the user to repeat context.
9. Use note_write to save information to ~/.config/mint/notes/ when asked to remember something.
10. Use run_plugin to interact with Google Workspace (Gmail, Calendar), Notion, Docker, Obsidian, Spotify, or System Metrics.
11. Keep thought short and concrete. Use Thai for the final summary when the task is written in Thai.
12. Commands that open URLs, files, folders, or launch apps (e.g. xdg-open, open) run in the background. Once they succeed (exit: 0), you are done. Use the 'finish' action immediately."#;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentApproval {
    WriteFile {
        path: String,
        content: String,
        diff: String,
    },
    ApplyPatch {
        path: String,
        hunks: Vec<CodePatchHunk>,
        diff: String,
    },
    RunShell { command: String },
    NoteWrite { path: String, content: String },
    RunPlugin { name: String, instruction: String },
    McpTool { server: String, tool: String, arguments: Value },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ApprovalOutcome {
    Approved,
    Denied,
    Intercepted(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum AgentProgress {
    Thinking { elapsed_secs: u64 },
    Thought { thought: String },
    ToolStart { action: String, input: Value },
    ToolEnd { result: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentResult {
    pub provider: String,
    pub model: String,
    pub summary: String,
    pub verification: String,
    pub fallback: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AgentDecision {
    #[serde(default)]
    thought: String,
    action: String,
    #[serde(default)]
    input: AgentInput,
}

#[derive(Debug, Default, Deserialize, Serialize)]
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
    #[serde(default)]
    note_path: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    instruction: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentPatch {
    path: PathBuf,
    #[serde(default)]
    hunks: Vec<CodePatchHunk>,
}

pub async fn orchestrate_agent_loop<Approve, Progress, Chunk>(
    config: &MintConfig,
    task: &str,
    root: &Path,
    image_data_uri: Option<String>,
    fast_mode: bool,
    mut approve: Approve,
    mut progress: Progress,
    mut on_chunk: Chunk,
) -> Result<AgentResult, OrchestrationError>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send,
    Progress: FnMut(AgentProgress) + Send,
    Chunk: FnMut(String) + Send,
{
    let started_at = Instant::now();
    let root = root
        .canonicalize()
        .map_err(|e| OrchestrationError::Agent(format!("unable to resolve workspace root {}: {}", root.display(), e)))?;
    let skills = crate::skills::learned_skills_context().unwrap_or_default();
    let mut observation = initial_observation(task, &root, &skills);
    let mut pending_image = image_data_uri;

    let mut system_prompt = SYSTEM_PROMPT.to_string();
    if let Ok(memory) = MemoryStore::open_default() {
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

    #[allow(unused_assignments)]
    let mut final_provider = config.ai_provider.clone();
    #[allow(unused_assignments)]
    let mut final_model = "".to_string();
    let mut final_fallback = None;

    for step in 1..=MAX_STEPS {
        progress(AgentProgress::Thinking {
            elapsed_secs: started_at.elapsed().as_secs(),
        });

        let (response, fallback) = send_chat_with_fallback(
            config,
            &ChatRequest {
                message: observation.clone(),
                system_instruction: system_prompt.clone(),
                image_data_uri: pending_image.take(),
                audio_data_uri: None,
            },
        )
        .await?;

        final_provider = response.provider.clone();
        final_model = response.model.clone();
        if fallback.is_some() {
            final_fallback = fallback.clone();
        }

        let decision = match parse_decision_or_finish(&response.text) {
            Ok(decision) => decision,
            Err(_) => {
                let (repaired, _) = send_chat_with_fallback(
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
                )
                .await?;
                parse_decision_or_finish(&repaired.text).map_err(|e| {
                    OrchestrationError::Agent(format!("unable to repair invalid agent response: {}", e))
                })?
            }
        };

        if !fast_mode && decision.action != "finish" && !decision.thought.trim().is_empty() {
            progress(AgentProgress::Thought {
                thought: decision.thought.trim().to_owned(),
            });
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

            on_chunk(summary.clone());

            let memory = MemoryStore::open_default()?;
            memory.add_interaction_with_metadata(task, &summary, &final_provider, &final_model)?;
            memory.save_workspace_session(&root.to_string_lossy(), &summary, &verification)?;

            return Ok(AgentResult {
                provider: final_provider,
                model: final_model,
                summary,
                verification,
                fallback: final_fallback,
            });
        }

        let input_val = serde_json::to_value(&decision.input).unwrap_or(Value::Null);
        progress(AgentProgress::ToolStart {
            action: decision.action.clone(),
            input: input_val,
        });

        let result = match execute_tool(&root, config, &decision, &mut approve).await {
            Ok(result) => result,
            Err(error) => {
                format!("Error: {}", error)
            }
        };

        progress(AgentProgress::ToolEnd {
            result: result.clone(),
        });

        let mut final_result = truncate(&result);
        if decision.action == "run_shell" || decision.action == "verify" {
            let mut failed = false;
            for line in result.lines() {
                if line.starts_with("exit: ") {
                    let exit_code = line.replace("exit: ", "").trim().to_string();
                    if exit_code != "0" && exit_code != "unknown" {
                        failed = true;
                    }
                    break;
                }
            }
            if failed {
                final_result.push_str(
                    "\n\n[System Tip: The command failed with a non-zero exit code. \
                     Analyze the stdout/stderr above to locate the error, read the offending files, \
                     apply corrected edits (using apply_patch), and run the verification command again. \
                     Do not finish or stop until the compilation or test errors are resolved!]"
                );
            }
        }

        observation = format!(
            "Task: {task}\nWorkspace: {}\nStep {step} completed.\nPrevious action: {}\nObservation:\n{}",
            root.display(),
            decision.action,
            final_result
        );
    }

    Err(OrchestrationError::Agent(format!(
        "code agent reached the limit of {} steps",
        MAX_STEPS
    )))
}

async fn execute_tool<Approve>(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    approve_cb: &mut Approve,
) -> Result<String, OrchestrationError>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send,
{
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" => {
            let path = workspace_path(root, &input.path)?;
            let files = list_code_files(&path, input.limit.unwrap_or(100), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string_pretty(&files)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "read_file" => {
            let path = workspace_path(root, required(&input.path, "path")?)?;
            Ok(read_code_file(
                &path,
                input.start_line.unwrap_or(1),
                input.end_line.unwrap_or(240),
                config,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "search_code" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(&search_code(
                &path,
                required(&input.query, "query")?,
                input.limit.unwrap_or(20),
                config,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "symbols" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(&build_symbol_index(
                &path,
                input.limit.unwrap_or(100),
                config,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "semantic_index" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &index_semantic_code(&path, config).await
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "semantic_search" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &search_semantic_code(
                    &path,
                    required(&input.query, "query")?,
                    input.limit.unwrap_or(5),
                    config,
                )
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "knowledge_search" => {
            Ok(serde_json::to_string_pretty(
                &KnowledgeStore::open_default()
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?
                    .search(required(&input.query, "query")?, input.limit.unwrap_or(5))
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "web_search" => {
            let query = required(&input.query, "query")?;
            let limit = input.limit.unwrap_or(5);
            match crate::web_search::search(query, limit, config).await {
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
            let query_lower = query.to_ascii_lowercase();
            let mut results = Vec::new();

            if let Ok(memory) = MemoryStore::open_default() {
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
            if file_name.contains("..") || file_name.contains('/') {
                return Err(OrchestrationError::Agent("note_write path must be a simple filename".into()));
            }
            let notes_dir = dirs::config_dir()
                .ok_or_else(|| OrchestrationError::Agent("cannot determine config directory".into()))?
                .join("mint")
                .join("notes");
            let note_path = notes_dir.join(file_name);

            let approved = approve_cb(&AgentApproval::NoteWrite {
                path: file_name.to_owned(),
                content: input.content.clone(),
            }).map_err(|e| OrchestrationError::Agent(e))?;

            match approved {
                ApprovalOutcome::Approved => {
                    std::fs::create_dir_all(&notes_dir).map_err(|e| {
                        OrchestrationError::Agent(format!("cannot create notes directory: {}", e))
                    })?;
                    std::fs::write(&note_path, &input.content).map_err(|e| {
                        OrchestrationError::Agent(format!("cannot write note: {}", e))
                    })?;
                    Ok(format!("Note saved to {}", note_path.display()))
                }
                ApprovalOutcome::Denied => Ok(format!("User denied note write: {}", file_name)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "run_plugin" => {
            let name = required(&input.name, "name")?;
            let instruction = required(&input.instruction, "instruction")?;
            let approved = approve_cb(&AgentApproval::RunPlugin {
                name: name.to_owned(),
                instruction: instruction.to_owned(),
            }).map_err(|e| OrchestrationError::Agent(e))?;

            match approved {
                ApprovalOutcome::Approved => Ok(execute_native_plugin(config, name, instruction).await
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => Ok(format!("User denied plugin execution: {}", name)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "mcp_tool" => {
            let server = required(&input.server, "server")?;
            let tool = required(&input.tool, "tool")?;
            let approved = approve_cb(&AgentApproval::McpTool {
                server: server.to_owned(),
                tool: tool.to_owned(),
                arguments: input.arguments.clone(),
            }).map_err(|e| OrchestrationError::Agent(e))?;

            match approved {
                ApprovalOutcome::Approved => {
                    Ok(serde_json::to_string_pretty(&crate::mcp::call_mcp_tool(
                        config,
                        server,
                        tool,
                        input.arguments.clone(),
                    ).map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                }
                ApprovalOutcome::Denied => Ok(format!("User denied MCP tool call: {} {}", server, tool)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "run_shell" => {
            let command = required(&input.command, "command")?;
            let approved = approve_cb(&AgentApproval::RunShell {
                command: command.to_owned(),
            }).map_err(|e| OrchestrationError::Agent(e))?;

            match approved {
                ApprovalOutcome::Approved => run_shell(root, config, command),
                ApprovalOutcome::Denied => Ok(format!("User denied shell command: {}", command)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "verify" => {
            if input.commands.is_empty() {
                return Err(OrchestrationError::Agent("verify requires at least one command".into()));
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
                .ok_or_else(|| OrchestrationError::Agent("apply_patch requires patch input".into()))?;
            if patch.hunks.is_empty() {
                return Err(OrchestrationError::Agent("apply_patch requires at least one hunk".into()));
            }
            let edit = build_code_patch(root, patch.path.clone(), &patch.hunks, config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let diff = proposal.edits.iter().map(|e| e.diff.clone()).collect::<Vec<_>>().join("\n");

            let approved = approve_cb(&AgentApproval::ApplyPatch {
                path: patch.path.to_string_lossy().into_owned(),
                hunks: patch.hunks.clone(),
                diff,
            }).map_err(|e| OrchestrationError::Agent(e))?;

            match approved {
                ApprovalOutcome::Approved => {
                    let applied = apply_code_edits(root, &[edit], &proposal.approval_token, config)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(serde_json::to_string_pretty(&applied)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                }
                ApprovalOutcome::Denied => Ok(format!("User denied file edit: {}", edit.path.display())),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "write_file" => {
            let path_str = required(&input.path, "path")?;
            let edit = CodeEdit {
                path: PathBuf::from(path_str),
                content: input.content.clone(),
            };
            let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let diff = proposal.edits.iter().map(|e| e.diff.clone()).collect::<Vec<_>>().join("\n");

            let approved = approve_cb(&AgentApproval::WriteFile {
                path: path_str.to_owned(),
                content: input.content.clone(),
                diff,
            }).map_err(|e| OrchestrationError::Agent(e))?;

            match approved {
                ApprovalOutcome::Approved => {
                    let applied = apply_code_edits(root, &[edit], &proposal.approval_token, config)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(serde_json::to_string_pretty(&applied)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                }
                ApprovalOutcome::Denied => Ok(format!("User denied file edit: {}", path_str)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        other => Err(OrchestrationError::Agent(format!("unsupported code-agent action '{}'", other))),
    }
}

fn run_shell(root: &Path, config: &MintConfig, command: &str) -> Result<String, OrchestrationError> {
    let output = run_shell_command(command, root, true, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
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
    if let Ok(memory) = MemoryStore::open_default() {
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
    use std::process::Command;
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

fn parse_decision(raw: &str) -> Result<AgentDecision, OrchestrationError> {
    if let Ok(decision) = parse_agent_json(raw) {
        return Ok(decision);
    }
    parse_shorthand_finish(raw).map_err(|e| OrchestrationError::Agent(e.to_string()))
}

fn parse_agent_json<T: serde::de::DeserializeOwned>(raw: &str) -> Result<T, OrchestrationError> {
    serde_json::from_str(raw).or_else(|_| {
        let start = raw
            .find('{')
            .ok_or_else(|| OrchestrationError::Agent("missing JSON object".into()))?;
        let end = raw
            .rfind('}')
            .ok_or_else(|| OrchestrationError::Agent("missing JSON object".into()))?;
        serde_json::from_str(&raw[start..=end])
            .map_err(|error| OrchestrationError::Agent(error.to_string()))
    })
}

fn parse_shorthand_finish(raw: &str) -> Result<AgentDecision, serde_json::Error> {
    let value: Value = serde_json::from_str(raw)?;
    let finish = value
        .get("finish")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(AgentDecision {
        thought: value
            .get("thought")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        action: "finish".into(),
        input: serde_json::from_value(finish)?,
    })
}

fn parse_decision_or_finish(raw: &str) -> Result<AgentDecision, OrchestrationError> {
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

fn workspace_path(root: &Path, value: &str) -> Result<PathBuf, OrchestrationError> {
    let path = root.join(if value.trim().is_empty() { "." } else { value });
    let path = path
        .canonicalize()
        .map_err(|e| OrchestrationError::Agent(format!("unable to resolve workspace path {}: {}", path.display(), e)))?;
    if !path.starts_with(root) {
        return Err(OrchestrationError::Agent(format!("path is outside workspace: {}", path.display())));
    }
    Ok(path)
}

fn required<'a>(value: &'a str, name: &str) -> Result<&'a str, OrchestrationError> {
    if value.trim().is_empty() {
        return Err(OrchestrationError::Agent(format!("{} is required", name)));
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_request_without_history() {
        let store = MemoryStore::open(
            std::env::temp_dir().join(format!("mint-orchestrator-{}.sqlite", std::process::id())),
        );
        let request = ChatRequest {
            message: "hello".into(),
            system_instruction: "system".into(),
            image_data_uri: None,
            audio_data_uri: None,
        };
        assert_eq!(
            enrich_request(&store, &request).unwrap().system_instruction,
            "system"
        );
    }
}

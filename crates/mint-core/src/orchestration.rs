use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::process::Command;
use std::time::Instant;
use thiserror::Error;

use crate::chat::{
    ChatMessage, ChatRole, ContentBlock, send_chat_with_fallback, stream_chat_with_fallback,
};
use crate::code_tools::{
    CodeEdit, CodePatchHunk, apply_code_edits, build_code_patch, list_code_files,
    propose_code_edits, read_code_file, search_code,
};
use crate::config::ToolCallingMode;
use crate::knowledge::KnowledgeStore;
use crate::plugins::execute_native_plugin;
use crate::prompts::tool_catalog::tool_catalog;
use crate::semantic::{index_semantic_code, search_semantic_code};
use crate::shell::run_shell_command;
use crate::symbols::build_symbol_index;
use crate::{
    Capability, ChatError, ChatRequest, ChatResponse, DEFAULT_CONVERSATION_ID, MemoryError,
    MemoryStore, MintConfig, assert_path_capability, classify_shell_command, send_chat,
    stream_chat,
};

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

pub async fn resolve_github_links(message: &str, config: &MintConfig) -> String {
    // Check if a GitHub MCP server is configured in Settings
    let github_mcp_configured = crate::mcp::configured_mcp_servers(config)
        .ok()
        .map(|servers| servers.contains_key("github"))
        .unwrap_or(false);

    if github_mcp_configured {
        // If GitHub MCP is active, we let it handle the repo via tool calls
        // to avoid duplicate/redundant context.
        return message.to_string();
    }

    use std::sync::OnceLock;
    static RE: OnceLock<regex::Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        regex::Regex::new(r"https?://(?:www\.)?github\.com/([a-zA-Z0-9\-_.]+)/([a-zA-Z0-9\-_.]+)")
            .unwrap()
    });

    let mut resolved_msg = message.to_string();
    let mut resolved_repos = std::collections::HashSet::new();

    for caps in re.captures_iter(message) {
        if let (Some(owner_match), Some(repo_match)) = (caps.get(1), caps.get(2)) {
            let owner = owner_match.as_str();
            let mut repo = repo_match.as_str().to_string();
            if repo.ends_with(".git") {
                repo = repo[..repo.len() - 4].to_string();
            }
            let repo_clean: String = repo
                .chars()
                .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == '.')
                .collect();

            let repo_key = format!("{owner}/{repo_clean}");
            if resolved_repos.insert(repo_key.clone())
                && let Ok(summary) =
                    crate::code_tools::fetch_github_repo_summary(owner, &repo_clean).await
            {
                resolved_msg.push_str(&format!(
                        "\n\n--- Auto-Resolved GitHub Metadata for {} ---\n{}\n--------------------------------------------",
                        repo_key, summary
                    ));
            }
        }
    }
    resolved_msg
}

pub async fn orchestrate_chat(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<ChatResponse, OrchestrationError> {
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let response = send_chat(config, &enriched).await?;
    memory.add_interaction_for_chat_with_fallback(
        request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
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
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let response = stream_chat(config, &enriched, on_chunk).await?;
    memory.add_interaction_for_chat_with_fallback(
        request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    Ok(response)
}

pub async fn orchestrate_chat_with_fallback(
    config: &MintConfig,
    request: &ChatRequest,
) -> Result<(ChatResponse, Option<String>), OrchestrationError> {
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let (response, fallback) = send_chat_with_fallback(config, &enriched).await?;
    memory.add_interaction_for_chat_with_fallback(
        request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
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
    let mut resolved_request = request.clone();
    resolved_request.message = resolve_github_links(&request.message, config).await;
    let memory = MemoryStore::open_default()?;
    let enriched = enrich_request(config, &memory, &resolved_request)?;
    let (response, fallback) = stream_chat_with_fallback(config, &enriched, on_chunk).await?;
    memory.add_interaction_for_chat_with_fallback(
        request_chat_id(request),
        &request.message,
        &response.text,
        &response.provider,
        &response.model,
        response.fallback_provider.as_deref(),
    )?;
    spawn_auto_memory_update(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    crate::linked_folders::spawn_linked_folder_note(
        config.clone(),
        request.message.clone(),
        response.text.clone(),
    );
    Ok((response, fallback))
}

fn enrich_request(
    config: &MintConfig,
    memory: &MemoryStore,
    request: &ChatRequest,
) -> Result<ChatRequest, MemoryError> {
    let mut interactions =
        memory.recent_interactions_for_chat(request_chat_id(request), CONTEXT_LIMIT)?;
    interactions.reverse();
    let transcript = interactions
        .into_iter()
        .map(|item| format!("User: {}\nAssistant: {}", item.user_text, item.ai_text))
        .collect::<Vec<_>>()
        .join("\n\n");
    let mut enriched = request.clone();

    let mut profile_instructions = String::new();
    if let Ok(Some(name)) = memory.get_profile("name")
        && !name.trim().is_empty()
    {
        profile_instructions.push_str(&format!("User Name: {}\n", name.trim()));
    }
    if let Ok(Some(preferences)) = memory.get_profile("preferences")
        && !preferences.trim().is_empty()
    {
        profile_instructions.push_str(&format!(
            "User Preferences & Profile:\n{}\n",
            preferences.trim()
        ));
    }

    if !profile_instructions.is_empty() {
        enriched.system_instruction = format!(
            "{}\n\nUser Profile Information:\n{}",
            enriched.system_instruction.trim(),
            profile_instructions.trim()
        )
        .trim()
        .to_owned();
    }

    // Inject active AI model/provider context to system instructions
    let active_model_info = format!(
        "\n\n[Active Environment Context]\n\
         You are running on: {}\n\
         Using AI Model: {}\n",
        config.ai_provider,
        config.active_model()
    );
    enriched.system_instruction.push_str(&active_model_info);

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

fn request_chat_id(request: &ChatRequest) -> &str {
    request
        .chat_id
        .as_deref()
        .map(str::trim)
        .filter(|chat_id| !chat_id.is_empty())
        .unwrap_or(DEFAULT_CONVERSATION_ID)
}

use crate::prompts::agent::build_system_prompt;

const MAX_STEPS: usize = 32;
const MAX_OBSERVATION_BYTES: usize = 16_000;
/// Compact `native_messages` once reported token usage crosses this fraction
/// of the active model's context window.
const COMPACTION_TRIGGER_RATIO: f64 = 0.8;
/// Number of most-recent Assistant/Tool step-pairs kept verbatim (uncompacted)
/// in `native_messages`.
const COMPACTION_KEEP_RECENT_STEPS: usize = 3;

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
    RunShell {
        command: String,
        mode: String,
    },
    NoteWrite {
        path: String,
        content: String,
    },
    RunPlugin {
        name: String,
        instruction: String,
    },
    McpTool {
        server: String,
        tool: String,
        arguments: Value,
    },
    UserApproval {
        title: String,
        prompt: String,
    },
    AskUser {
        question: String,
        #[serde(default)]
        options: Vec<String>,
    },
    ExitPlanMode {
        plan: String,
    },
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
    Thinking {
        elapsed_secs: u64,
        #[serde(skip_serializing_if = "Option::is_none")]
        agent_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        model_name: Option<String>,
    },
    Thought {
        thought: String,
    },
    ToolStart {
        action: String,
        input: Value,
    },
    ToolEnd {
        action: String,
        input: Value,
        result: String,
    },
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentDirectoryEntry {
    name: String,
    path: PathBuf,
    kind: &'static str,
    size: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct AgentDecision {
    #[serde(default)]
    thought: String,
    action: String,
    #[serde(default, deserialize_with = "deserialize_agent_input")]
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
    options: Vec<String>,
    #[serde(default)]
    city: String,
    #[serde(default)]
    expression: String,
    #[serde(default)]
    command: String,
    #[serde(default)]
    commands: Vec<String>,
    #[serde(default)]
    steps: Vec<String>,
    #[serde(default)]
    file_content: String,
    #[serde(default)]
    summary: String,
    #[serde(default)]
    verification: String,
    #[serde(default)]
    plan: String,
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
    #[serde(default)]
    title: String,
    #[serde(default)]
    status: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    selector: String,
    #[serde(default)]
    text: String,
    #[serde(default)]
    x: Option<f64>,
    #[serde(default)]
    y: Option<f64>,
    #[serde(default)]
    button: String,
    #[serde(default)]
    key: String,
    // Video tools input fields
    #[serde(default)]
    input: String,
    #[serde(default)]
    output: String,
    #[serde(default)]
    start: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(default)]
    width: Option<i32>,
    #[serde(default)]
    height: Option<i32>,
    #[serde(default)]
    threshold_db: Option<f64>,
    #[serde(default)]
    min_silence_secs: Option<f64>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    target_language: Option<String>,
    #[serde(default)]
    srt_content: Option<String>,
    #[serde(default)]
    preset: Option<String>,
    #[serde(default)]
    max_clips: Option<u32>,
    #[serde(default)]
    target_duration: Option<f64>,
    #[serde(default)]
    inputs: Vec<String>,
    #[serde(default)]
    order: Vec<usize>,
    #[serde(default)]
    music_input: Option<String>,
    #[serde(default)]
    video_input: Option<String>,
    #[serde(default)]
    music_volume: Option<f32>,
    #[serde(default)]
    zoom_factor: Option<f32>,
    // Image & Video generation input fields
    #[serde(default)]
    prompt: String,
    #[serde(default)]
    aspect_ratio: String,
    #[serde(default)]
    provider: String,
    #[serde(default)]
    duration: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct AgentPatch {
    path: PathBuf,
    #[serde(default)]
    hunks: Vec<CodePatchHunk>,
}

fn deserialize_agent_input<'de, D>(deserializer: D) -> Result<AgentInput, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(Option::<AgentInput>::deserialize(deserializer)?.unwrap_or_default())
}

fn resolve_agent_config(
    config: &MintConfig,
    agent_id: Option<&str>,
    trajectory: &[String],
) -> (MintConfig, String, Option<String>, Option<String>) {
    if !config.enable_agent_collaboration {
        return (config.clone(), "".to_string(), None, None);
    }

    let enabled_agents: Vec<&crate::config::AgentConfig> =
        config.agents.iter().filter(|a| a.enabled).collect();
    if enabled_agents.is_empty() {
        return (config.clone(), "".to_string(), None, None);
    }

    let active_agent = if let Some(id) = agent_id {
        enabled_agents.iter().find(|a| a.id == id).copied()
    } else {
        // Multi-Agent Pipeline Collaboration (Planner -> Coder -> Reviewer)
        let plan_created = trajectory
            .iter()
            .any(|step| step.contains("- Action: create_plan"));
        if !plan_created {
            if let Some(planner) = enabled_agents.iter().find(|a| a.id == "planner") {
                Some(*planner)
            } else {
                enabled_agents.iter().find(|a| a.id == "coder").copied()
            }
        } else {
            // Check if edits have been made to verify
            let edits_made = trajectory.iter().any(|step| {
                step.contains("- Action: write_file") || step.contains("- Action: apply_patch")
            });
            if edits_made {
                if let Some(last_step) = trajectory.last() {
                    if last_step.contains("- Action: write_file")
                        || last_step.contains("- Action: apply_patch")
                    {
                        if let Some(reviewer) = enabled_agents.iter().find(|a| a.id == "reviewer") {
                            Some(*reviewer)
                        } else {
                            enabled_agents.iter().find(|a| a.id == "coder").copied()
                        }
                    } else {
                        enabled_agents.iter().find(|a| a.id == "coder").copied()
                    }
                } else {
                    enabled_agents.iter().find(|a| a.id == "coder").copied()
                }
            } else {
                enabled_agents.iter().find(|a| a.id == "coder").copied()
            }
        }
    };

    let Some(agent) = active_agent else {
        return (config.clone(), "".to_string(), None, None);
    };

    let mut cfg_clone = config.clone();
    cfg_clone.ai_provider = agent.provider.clone();
    cfg_clone.gemini_model = agent.model.clone();
    cfg_clone.openai_model = agent.model.clone();
    cfg_clone.anthropic_model = agent.model.clone();
    cfg_clone.openrouter_model = agent.model.clone();
    cfg_clone.deepseek_model = agent.model.clone();
    cfg_clone.hf_model = agent.model.clone();
    cfg_clone.local_model_name = agent.model.clone();
    cfg_clone.ollama_model = agent.model.clone();

    if let Some(key) = &agent.api_key
        && !key.trim().is_empty()
    {
        match agent.provider.as_str() {
            "gemini" => cfg_clone.api_key = key.clone(),
            "openai" => cfg_clone.openai_api_key = key.clone(),
            "anthropic" => cfg_clone.anthropic_api_key = key.clone(),
            "openrouter" => cfg_clone.openrouter_api_key = key.clone(),
            "deepseek" => cfg_clone.deepseek_api_key = key.clone(),
            "huggingface" => cfg_clone.hf_api_key = key.clone(),
            _ => {}
        }
    }

    (
        cfg_clone,
        agent.system_instruction.clone(),
        Some(agent.name.clone()),
        Some(agent.model.clone()),
    )
}

/// Returns a boxed `dyn Future` trait object rather than being a plain
/// `async fn` (whose return type would otherwise be an opaque, compiler-
/// inferred type). This is required, not just a style choice: `execute_tool`
/// (called from this function's own loop body) can itself recurse back into
/// `orchestrate_agent_loop` via the `dispatch_subagent` tool, and Rust cannot
/// check whether an opaque `async fn` return type satisfies `Send` from code
/// that lives inside that same function's own defining scope — it's a
/// "cannot check whether the hidden type of opaque type satisfies auto
/// traits" compile error. Making the return type concrete (not opaque) up
/// front sidesteps that entirely. `.await` works identically on this as on a
/// plain `async fn`, so none of this function's existing callers need to
/// change.
pub fn orchestrate_agent_loop<'a, Approve, Progress, Chunk>(
    config: &'a MintConfig,
    task: &'a str,
    root: &'a Path,
    image_data_uri: Option<String>,
    audio_data_uri: Option<String>,
    video_data_uri: Option<String>,
    chat_id: Option<&'a str>,
    agent_id: Option<&'a str>,
    user_name: Option<&'a str>,
    fast_mode: bool,
    plan_mode: bool,
    mut approve: Approve,
    mut progress: Progress,
    mut on_chunk: Chunk,
) -> Pin<Box<dyn Future<Output = Result<AgentResult, OrchestrationError>> + Send + 'a>>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send + 'a,
    Progress: FnMut(AgentProgress) + Send + 'a,
    Chunk: FnMut(String) + Send + 'a,
{
    Box::pin(async move {
        let started_at = Instant::now();
        let root = root.canonicalize().map_err(|e| {
            OrchestrationError::Agent(format!(
                "unable to resolve workspace root {}: {}",
                root.display(),
                e
            ))
        })?;
        let resolved_task = resolve_github_links(task, config).await;
        let chat_id = chat_id
            .map(str::trim)
            .filter(|chat_id| !chat_id.is_empty())
            .unwrap_or(DEFAULT_CONVERSATION_ID);
        // Subagent runs use a synthetic `{parent_chat_id}::subagent::{name}` chat id
        // (see the `dispatch_subagent` arm in `execute_tool`) so their own memory
        // interaction doesn't leak into the parent conversation's history. That
        // same marker doubles as the depth-limit signal here: a subagent's own
        // nested loop never offers `dispatch_subagent` in its tool catalog, so it
        // can't recurse into further subagents.
        let allow_subagent_dispatch = !chat_id.contains("::subagent::");
        let skills =
            crate::skills::learned_skills_context(Some(&root), Some(chat_id)).unwrap_or_default();
        let mut observation = initial_observation(&resolved_task, &root, &skills);
        let mut pending_image = image_data_uri;
        let mut pending_audio = audio_data_uri;
        let mut pending_video = video_data_uri;

        let mut plan_mode = plan_mode;
        // Determined once from the base `config` (not the per-step `active_config`
        // multi-agent collaboration can substitute) — matches how `system_prompt`
        // itself is only rebuilt on plan-mode transitions, not every step.
        let system_prompt_native = config.tool_calling_mode() == ToolCallingMode::Native;
        let mut system_prompt =
            build_system_prompt(config, plan_mode, system_prompt_native, user_name);
        let hooks = crate::hooks::list_hooks(config);

        append_memory_context(&mut system_prompt, chat_id);

        #[allow(unused_assignments)]
        let mut final_provider = config.ai_provider.clone();
        #[allow(unused_assignments)]
        let mut final_model = "".to_string();
        let mut final_fallback = None;
        let mut action_counts = BTreeMap::<String, usize>::new();
        // Track the most recent step (if any) that successfully modified a file
        // (`apply_patch`/`write_file`) and the most recent step that ran `verify`,
        // so `finish` can be rejected when code was changed but never checked —
        // see the gate right before the `finish` handling block below.
        let mut last_modify_step: Option<usize> = None;
        let mut last_verify_step: Option<usize> = None;
        let mut trajectory: Vec<String> = Vec::new();
        // Structured history for native tool-calling providers, maintained alongside
        // `trajectory`/`observation` (which remain the source of truth for the
        // JSON-prompt fallback path and for the web-search/media-append scans in the
        // finish handler below, which operate on flattened text either way).
        let mut native_messages: Vec<ChatMessage> = Vec::new();
        // Gemini's `thoughtSignature` (see `ToolCall::thought_signature`) arrives on
        // `response.tool_calls`, keyed by call id, but the `ContentBlock::ToolUse`
        // that gets pushed onto `native_messages` for this call is only rebuilt
        // later from `step_tool_results` (itself derived from `decisions`, which
        // drops the original `ToolCall`). Stashing it here by call id — persisting
        // across the whole loop, since every past turn must keep replaying its own
        // signature on every subsequent request — avoids threading it through
        // `AgentDecision` just for this one Gemini-specific quirk.
        let mut step_thought_signatures: std::collections::HashMap<String, String> =
            std::collections::HashMap::new();
        let mut warned_json_prompt_fallback = false;

        'steps: for step in 1..=MAX_STEPS {
            let (active_config, agent_instruction, active_agent_name, active_model_name) =
                resolve_agent_config(config, agent_id, &trajectory);

            progress(AgentProgress::Thinking {
                elapsed_secs: started_at.elapsed().as_secs(),
                agent_name: active_agent_name,
                model_name: active_model_name.clone(),
            });

            let mut active_system_prompt = system_prompt.clone();
            if let Some(ref model_name) = active_model_name {
                active_system_prompt.push_str(&format!(
                    "\n\n[Active Environment Context]\n\
                 You are running on: {}\n\
                 Using AI Model: {}\n",
                    active_config.ai_provider, model_name
                ));
            }
            if !agent_instruction.is_empty() {
                active_system_prompt.push_str(&format!(
                    "\n\nYour Current Role & System Instructions:\n{}",
                    agent_instruction
                ));
            }

            let tool_mode = active_config.tool_calling_mode();

            if tool_mode == ToolCallingMode::JsonPrompt
                && active_config.ai_provider == "ollama"
                && !warned_json_prompt_fallback
            {
                warned_json_prompt_fallback = true;
                progress(AgentProgress::Thought {
                    thought: format!(
                        "[Warning] Model '{}' is not on the verified native tool-calling allowlist; \
                     falling back to prompt-based JSON mode, which is less reliable than native \
                     tool calling. Switch to a Llama 3.1+/Qwen2.5+-family model for more reliable \
                     agent runs.",
                        active_config.ollama_model
                    ),
                });
            }

            let (response, fallback) = if tool_mode == ToolCallingMode::Native {
                if native_messages.is_empty() {
                    // Native tool-calling providers build the request from `messages`
                    // and ignore ChatRequest.image_data_uri/audio_data_uri/video_data_uri
                    // entirely, so any pending attachment must be embedded as content
                    // blocks on the first user turn here, not left for the (unused)
                    // top-level fields — otherwise Agent Mode would silently drop
                    // attachments that work fine with Agent Mode off.
                    let mut content = vec![ContentBlock::Text {
                        text: observation.clone(),
                    }];
                    if let Some(image_data) = pending_image.take() {
                        for img in image_data.split_whitespace() {
                            content.push(ContentBlock::Image {
                                data_uri: img.to_owned(),
                            });
                        }
                    }
                    if let Some(audio_data) = pending_audio.take() {
                        for aud in audio_data.split_whitespace() {
                            content.push(ContentBlock::Audio {
                                data_uri: aud.to_owned(),
                            });
                        }
                    }
                    if let Some(video_data) = pending_video.take() {
                        for vid in video_data.split_whitespace() {
                            content.push(ContentBlock::Video {
                                data_uri: vid.to_owned(),
                            });
                        }
                    }
                    native_messages.push(ChatMessage {
                        role: ChatRole::User,
                        content,
                    });
                }
                send_chat_with_fallback(
                    &active_config,
                    &ChatRequest {
                        message: String::new(),
                        system_instruction: active_system_prompt.clone(),
                        chat_id: Some(chat_id.to_owned()),
                        image_data_uri: None,
                        audio_data_uri: None,
                        video_data_uri: None,
                        document_attachment: None,
                        workspace_path: None,
                        agent_id: None,
                        plan_mode: false,
                        messages: Some(native_messages.clone()),
                        tools: Some(tool_catalog(
                            &active_config,
                            plan_mode,
                            &root,
                            allow_subagent_dispatch,
                        )),
                    },
                )
                .await?
            } else {
                send_chat_with_fallback(
                    &active_config,
                    &ChatRequest {
                        message: observation.clone(),
                        system_instruction: active_system_prompt.clone(),
                        chat_id: Some(chat_id.to_owned()),
                        image_data_uri: pending_image.take(),
                        audio_data_uri: pending_audio.take(),
                        video_data_uri: pending_video.take(),
                        document_attachment: None,
                        workspace_path: None,
                        agent_id: None,
                        plan_mode: false,
                        messages: None,
                        tools: None,
                    },
                )
                .await?
            };

            final_provider = response.provider.clone();
            final_model = response.model.clone();
            if fallback.is_some() {
                // `fallback` (this function's own return value) is the provider
                // that actually served this response; `response.fallback_provider`
                // is a same-shaped but differently-populated field that
                // `send_chat_with_fallback` sets to the *original* provider that
                // failed over — using it here showed e.g. "gemini → fallback:
                // gemini • Qwen..." in the CLI badge instead of "gemini →
                // fallback: huggingface • Qwen...".
                final_fallback = fallback.clone();
            }
            if let Some(calls) = &response.tool_calls {
                for call in calls {
                    if let Some(signature) = &call.thought_signature {
                        step_thought_signatures.insert(call.id.clone(), signature.clone());
                    }
                }
            }

            // `decisions` is normally a single `(call_id, AgentDecision)`, matching the
            // original one-action-per-step design. Native tool-calling can return
            // several tool calls in one model turn, in which case they're executed
            // sequentially and all their results are fed back before the next call —
            // `finish` never appears alongside real tool calls (see below), so this
            // never conflicts with the early-return finish handling.
            let decisions: Vec<(String, AgentDecision)> = if tool_mode == ToolCallingMode::Native {
                match response.tool_calls.clone() {
                    Some(calls) if !calls.is_empty() => calls
                        .into_iter()
                        .enumerate()
                        .map(|(index, call)| {
                            let thought = if index == 0 {
                                response.text.trim().to_string()
                            } else {
                                String::new()
                            };
                            let input: AgentInput =
                                serde_json::from_value(call.input).unwrap_or_default();
                            (
                                call.id,
                                AgentDecision {
                                    thought,
                                    action: call.name,
                                    input,
                                },
                            )
                        })
                        .collect(),
                    // No tool calls means the model answered directly — treat exactly
                    // like the JSON-prompt path's fallback for plain, non-JSON text
                    // (see `parse_decision_or_finish`): finish with that text as the
                    // summary.
                    _ => vec![(
                        format!("call_{step}_finish"),
                        AgentDecision {
                            thought: String::new(),
                            action: "finish".to_string(),
                            input: AgentInput {
                                summary: response.text.trim().to_string(),
                                ..AgentInput::default()
                            },
                        },
                    )],
                }
            } else {
                let decision = match parse_decision_or_finish(&response.text) {
                    Ok(decision) => decision,
                    Err(_) => {
                        let (repaired, _) = send_chat_with_fallback(
                        &active_config,
                        &ChatRequest {
                            message: format!(
                                "Your previous response was not valid Mint agent JSON.\n\
                                 Return exactly one corrected JSON object with an action and input. \
                                 Do not use markdown.\n\nPrevious response:\n{}",
                                truncate(&response.text)
                            ),
                            system_instruction: active_system_prompt.clone(),
                            chat_id: Some(chat_id.to_owned()),
                            image_data_uri: None,
                            audio_data_uri: None,
                            video_data_uri: None,
                            document_attachment: None,
                            workspace_path: None,
                            agent_id: None,
                            plan_mode: false,
                            messages: None,
                            tools: None,
                        },
                    )
                    .await?;
                        parse_decision_or_finish(&repaired.text).map_err(|e| {
                            OrchestrationError::Agent(format!(
                                "unable to repair invalid agent response: {}",
                                e
                            ))
                        })?
                    }
                };
                vec![(format!("call_{step}"), decision)]
            };

            let mut step_tool_results: Vec<(String, String, Value, String)> = Vec::new();
            // Screenshots (and any future binary/image tool result) are kept out of
            // `step_tool_results`'s plain-text `final_result` — that string is what
            // both the text `trajectory` and the JSON-prompt `observation` are built
            // from, and a multi-hundred-KB base64 PNG would blow straight through
            // `MAX_OBSERVATION_BYTES` and get silently chopped mid-base64. The full
            // data URI is stashed here by `call_id` instead, and re-attached as a
            // real `ContentBlock::Image` when building `native_messages` below, so
            // the model actually sees the pixels instead of a truncated text blob.
            let mut step_images: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            if decisions_are_parallel_subagent_batch(&decisions) {
                step_tool_results = run_parallel_subagent_batch(
                    &decisions,
                    step,
                    &root,
                    config,
                    chat_id,
                    &mut approve,
                    &mut progress,
                    &mut action_counts,
                    &mut trajectory,
                )
                .await;
            } else {
                for (call_id, decision) in decisions {
                    if !fast_mode
                        && decision.action != "finish"
                        && !decision.thought.trim().is_empty()
                    {
                        progress(AgentProgress::Thought {
                            thought: decision.thought.trim().to_owned(),
                        });
                    }

                    if decision.action == "finish" {
                        let mut summary = decision.input.summary.trim().to_owned();
                        let is_thai_task =
                            task.chars().any(|c| ('\u{0e00}'..='\u{0e7f}').contains(&c));
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
                                let err_msg = "Error: Your finish action summary was empty. \
                                       You MUST provide a final answer, explanation, or response to the user's query \
                                       in the 'summary' field of the 'finish' action input. Do not leave it empty.";
                                trajectory.push(format!(
                                    "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                                    decision.thought.trim(),
                                    decision.action,
                                    err_msg
                                ));
                                let history_str = trajectory.join("\n\n");
                                observation = format!(
                                    "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                                    root.display(),
                                    history_str
                                );
                                reject_native_finish(
                                    tool_mode,
                                    &mut native_messages,
                                    &response.text,
                                    err_msg,
                                );
                                continue 'steps;
                            }
                            if unverified_modification(
                                last_modify_step,
                                last_verify_step,
                                &decision.input.verification,
                            ) {
                                let err_msg = "Error: You modified a file (apply_patch/write_file) in this \
                                       run but finished without verifying it. Call the verify tool \
                                       with build/test/lint commands appropriate for this project \
                                       before finishing. If no check genuinely applies (e.g. no test \
                                       suite, documentation-only change), say so explicitly in the \
                                       finish action's 'verification' field and finish again.";
                                trajectory.push(format!(
                                    "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                                    decision.thought.trim(),
                                    decision.action,
                                    err_msg
                                ));
                                let history_str = trajectory.join("\n\n");
                                observation = format!(
                                    "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                                    root.display(),
                                    history_str
                                );
                                reject_native_finish(
                                    tool_mode,
                                    &mut native_messages,
                                    &response.text,
                                    err_msg,
                                );
                                continue 'steps;
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
                                if !summary_lower.contains("google")
                                    && !summary_lower.contains("brave")
                                {
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

                        // Auto-append generated media (image/video) and model feedback to summary if LLM omitted it
                        let mut media_blocks = Vec::new();
                        for step in trajectory.iter() {
                            for line in step.lines() {
                                let trimmed = line.trim();
                                if trimmed.starts_with("![Generated Image](")
                                    || trimmed.starts_with("✓ Image generated successfully")
                                    || trimmed.starts_with("Saved to:")
                                    || trimmed.starts_with("<video")
                                    || trimmed.starts_with("✓ Video generated successfully")
                                {
                                    if !summary.contains(trimmed) {
                                        media_blocks.push(trimmed.to_string());
                                    }
                                }
                            }
                        }
                        if !media_blocks.is_empty() {
                            summary.push_str("\n\n");
                            summary.push_str(&media_blocks.join("\n\n"));
                        }

                        let verification =
                            meaningful_verification(&decision.input.verification).to_owned();

                        on_chunk(summary.clone());

                        let memory = MemoryStore::open_default()?;
                        memory.add_interaction_for_chat_with_fallback(
                            chat_id,
                            task,
                            &summary,
                            &final_provider,
                            &final_model,
                            final_fallback.as_deref(),
                        )?;
                        memory.save_workspace_session(
                            &root.to_string_lossy(),
                            &summary,
                            &verification,
                        )?;
                        spawn_auto_memory_update(config.clone(), task.to_string(), summary.clone());
                        crate::linked_folders::spawn_linked_folder_note(
                            config.clone(),
                            task.to_string(),
                            summary.clone(),
                        );
                        if config.auto_skill_writing && looks_skill_worthy(step, &action_counts) {
                            spawn_auto_skill_write(
                                config.clone(),
                                task.to_string(),
                                summary.clone(),
                                root.clone(),
                                skills.clone(),
                            );
                        }

                        return Ok(AgentResult {
                            provider: final_provider,
                            model: final_model,
                            summary,
                            verification,
                            fallback: final_fallback,
                        });
                    }

                    let action_key = action_fingerprint(&decision);
                    let action_count = {
                        let count = action_counts.entry(action_key).or_insert(0);
                        *count += 1;
                        *count
                    };

                    // Set only on the real `execute_tool` path below (PreHookOutcome::Allowed);
                    // stays false for plan-mode/hook blocks and the duplicate-shell skip, since
                    // those don't actually run anything and shouldn't count toward verification.
                    let mut action_succeeded = false;
                    let result = if decision.action == "exit_plan_mode" {
                        let plan_text = decision.input.plan.trim().to_owned();
                        match approve(&AgentApproval::ExitPlanMode {
                    plan: plan_text.clone(),
                }) {
                    Ok(ApprovalOutcome::Approved) => {
                        plan_mode = false;
                        system_prompt = build_system_prompt(config, plan_mode, system_prompt_native, user_name);
                        "Plan approved by the user. Plan mode is now OFF — write_file, apply_patch, run_shell, and other previously blocked tools are now available. Proceed with implementing the plan.".to_string()
                    }
                    Ok(ApprovalOutcome::Denied) => {
                        "The user rejected this plan. Plan mode is still ON. Continue investigating or revise the plan, then call exit_plan_mode again when ready.".to_string()
                    }
                    Ok(ApprovalOutcome::Intercepted(feedback)) => {
                        format!(
                            "The user did not approve the plan yet and left this feedback: {}\n\nPlan mode is still ON. Revise the plan accordingly and call exit_plan_mode again when ready.",
                            feedback
                        )
                    }
                    Err(error) => format!("Error requesting plan approval: {}", error),
                }
                    } else if plan_mode && !plan_mode_allows(&decision.action, &decision.input) {
                        format!(
                            "Blocked: '{}' is not available in plan mode (read-only investigation only). Call exit_plan_mode with your proposed plan once you are ready to implement it; the user will approve or reject it.",
                            decision.action
                        )
                    } else if decision.action == "run_shell" && action_count > 1 {
                        format!(
                            "Skipped duplicate shell command: {}\n\n[System Tip: This exact shell command already ran once in this task. Do not run it again. Use the finish action now and tell the user the action was completed.]",
                            decision.input.command.trim()
                        )
                    } else {
                        let input_val =
                            serde_json::to_value(&decision.input).unwrap_or(Value::Null);
                        progress(AgentProgress::ToolStart {
                            action: decision.action.clone(),
                            input: input_val.clone(),
                        });

                        match crate::hooks::run_pre_tool_hooks(
                            &hooks,
                            &decision.action,
                            &input_val,
                            &root,
                        ) {
                            crate::hooks::PreHookOutcome::Blocked(reason) => {
                                format!(
                                    "Blocked by hook: {}\n\n[System Tip: A configured PreToolUse hook rejected this action. Adjust your approach or ask the user for guidance.]",
                                    reason
                                )
                            }
                            crate::hooks::PreHookOutcome::Allowed => {
                                let (tool_result, success) = match execute_tool(
                                    &root,
                                    config,
                                    &decision,
                                    chat_id,
                                    &mut approve,
                                )
                                .await
                                {
                                    Ok(result) => (result, true),
                                    Err(error) => (format!("Error: {}", error), false),
                                };
                                action_succeeded = success;
                                let hook_messages = crate::hooks::run_post_tool_hooks(
                                    &hooks,
                                    &decision.action,
                                    &input_val,
                                    &tool_result,
                                    success,
                                    &root,
                                );
                                if hook_messages.is_empty() {
                                    tool_result
                                } else {
                                    format!(
                                        "{}\n\n{}",
                                        tool_result,
                                        hook_messages
                                            .iter()
                                            .map(|message| format!("[Hook] {}", message))
                                            .collect::<Vec<_>>()
                                            .join("\n")
                                    )
                                }
                            }
                        }
                    };

                    progress(AgentProgress::ToolEnd {
                        action: decision.action.clone(),
                        input: serde_json::to_value(&decision.input).unwrap_or(Value::Null),
                        result: result.clone(),
                    });

                    if action_succeeded {
                        match decision.action.as_str() {
                            "apply_patch" | "write_file" => last_modify_step = Some(step),
                            // Counts even if the commands it ran failed — an attempted check
                            // still counts as verification having been attempted; a failing
                            // exit code is separately flagged below and prompts a fix.
                            "verify" => last_verify_step = Some(step),
                            _ => {}
                        }
                    }

                    let mut final_result = if decision.action == "browser_screenshot"
                        && result.starts_with("data:image/")
                    {
                        step_images.insert(call_id.clone(), result.clone());
                        "[Screenshot captured — see attached image]".to_string()
                    } else {
                        truncate(&result)
                    };
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
                    if decision.action == "apply_patch" || decision.action == "write_file" {
                        final_result.push_str(
                    "\n\n[System Tip: The file edit was approved and applied successfully. \
                     Before finishing, verify this change with the verify tool (build/test/lint, \
                     whatever fits this project) — finish will be rejected until you do, unless you \
                     state in the finish action's verification field why no check applies (e.g. no \
                     test suite, documentation-only change). Do not broaden the scope, do not make \
                     additional unrelated edits, and do not reread the same file unless you need one \
                     concise verification read.]",
                );
                    }
                    if action_count >= 3 {
                        final_result.push_str(
                    "\n\n[System Tip: You repeated the same tool action three or more times. \
                     Stop repeating it. If you already have enough information or the requested edit is done, \
                     use the finish action now. Otherwise choose a different necessary action.]",
                );
                    }

                    trajectory.push(format!(
                        "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
                        decision.thought.trim(),
                        decision.action,
                        final_result
                    ));

                    step_tool_results.push((
                        call_id,
                        decision.action.clone(),
                        serde_json::to_value(&decision.input).unwrap_or(Value::Null),
                        final_result,
                    ));
                } // end `for (call_id, decision) in decisions`
            } // end `else` (sequential path)

            if tool_mode == ToolCallingMode::Native {
                let mut assistant_content: Vec<ContentBlock> = Vec::new();
                let response_text = response.text.trim();
                if !response_text.is_empty() {
                    assistant_content.push(ContentBlock::Text {
                        text: response_text.to_string(),
                    });
                }
                let mut tool_result_content: Vec<ContentBlock> = Vec::new();
                for (call_id, action, input_value, final_result) in &step_tool_results {
                    assistant_content.push(ContentBlock::ToolUse {
                        id: call_id.clone(),
                        name: action.clone(),
                        input: input_value.clone(),
                        thought_signature: step_thought_signatures.get(call_id).cloned(),
                    });
                    tool_result_content.push(ContentBlock::ToolResult {
                        tool_use_id: call_id.clone(),
                        content: final_result.clone(),
                        is_error: false,
                    });
                    if let Some(data_uri) = step_images.get(call_id) {
                        tool_result_content.push(ContentBlock::Image {
                            data_uri: data_uri.clone(),
                        });
                    }
                }
                if !assistant_content.is_empty() {
                    native_messages.push(ChatMessage {
                        role: ChatRole::Assistant,
                        content: assistant_content,
                    });
                }
                if !tool_result_content.is_empty() {
                    native_messages.push(ChatMessage {
                        role: ChatRole::Tool,
                        content: tool_result_content,
                    });
                }

                if let Some(total_tokens) = response.total_tokens {
                    let window = active_config.context_window_tokens();
                    if (total_tokens as f64) >= (window as f64) * COMPACTION_TRIGGER_RATIO {
                        match compact_native_messages(&active_config, &native_messages).await {
                            Ok(Some(compacted)) => {
                                native_messages = compacted;
                                progress(AgentProgress::Thought {
                                    thought: format!(
                                        "[Context] Compacted earlier steps to stay under the \
                                     context window ({total_tokens}/{window} tokens before \
                                     compaction)."
                                    ),
                                });
                            }
                            // Nothing worth compacting yet (too little history) — routine, no warning.
                            Ok(None) => {}
                            Err(error) => {
                                progress(AgentProgress::Thought {
                                    thought: format!(
                                        "[Context] Context is approaching the model's window \
                                     ({total_tokens}/{window} tokens) but compaction failed: \
                                     {error}. Continuing without compacting this step."
                                    ),
                                });
                            }
                        }
                    }
                }
            }

            let history_str = trajectory.join("\n\n");
            observation = format!(
                "Task: {task}\nWorkspace: {}\n\nHere is the history of what you have done so far in this agent loop:\n\n{}\n\nProceed to the next step. If you have completed the task, use the 'finish' action.",
                root.display(),
                history_str
            );
        }

        Err(OrchestrationError::Agent(format!(
            "code agent reached the limit of {} steps",
            MAX_STEPS
        )))
    })
}

/// Whether `action` may run while plan mode is active. `run_shell` gets a
/// special case: only commands the safety classifier already tags as
/// read-only are allowed, everything else requires exiting plan mode first.
/// `finish` is always allowed regardless of plan mode. The rest of the
/// allowlist is shared with the system-prompt builder and the native
/// tool-calling catalog via `crate::prompts::agent::PLAN_MODE_ALLOWED_ACTIONS`
/// so the three can never drift on which actions are plan-mode-safe.
fn plan_mode_allows(action: &str, input: &AgentInput) -> bool {
    if action == "run_shell" {
        return classify_shell_command(&input.command).mode.as_str() == "readOnly";
    }
    action == "finish" || crate::prompts::agent::PLAN_MODE_ALLOWED_ACTIONS.contains(&action)
}

/// Runs one subagent to completion and returns its formatted result text.
/// Extracted from `execute_tool`'s `"dispatch_subagent"` arm so both the plain
/// sequential path (a single subagent call, or one mixed in with other actions)
/// and the parallel path (`decisions_are_parallel_subagent_batch`, driven by
/// `buffer_unordered` in the main step loop) share one implementation. Takes
/// the same `&mut dyn FnMut(...)` trait object as `execute_tool` — see the
/// comment on that function for why a trait object rather than a generic —
/// which lets the parallel path pass a small Mutex-backed adapter closure so
/// concurrently-running subagents still share one real approval gate.
async fn dispatch_one_subagent(
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    name: &str,
    task: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    let Some(definition) = crate::subagents::find_subagent(name, Some(root)) else {
        return Err(OrchestrationError::Agent(format!(
            "no subagent named '{name}' found (check .agents/subagents/ or \
             ~/.config/mint/mint-agents/)"
        )));
    };

    let mut sub_config = config.clone();
    if let Some(provider) = &definition.provider {
        sub_config.ai_provider = provider.clone();
    }
    if let Some(model) = &definition.model {
        match sub_config.ai_provider.as_str() {
            "anthropic" => sub_config.anthropic_model = model.clone(),
            "openai" => sub_config.openai_model = model.clone(),
            "openrouter" => sub_config.openrouter_model = model.clone(),
            "deepseek" => sub_config.deepseek_model = model.clone(),
            "huggingface" => sub_config.hf_model = model.clone(),
            "local_openai" => sub_config.local_model_name = model.clone(),
            "ollama" => sub_config.ollama_model = model.clone(),
            "gemini" => sub_config.gemini_model = model.clone(),
            _ => {}
        }
    }

    // The subagent's own persona/instructions are folded into the task
    // framing rather than replacing Mint's system prompt (which
    // `orchestrate_agent_loop` builds internally and isn't a parameter),
    // so the subagent still follows the same tool-use protocol and
    // safety rules as any other agent run.
    let sub_task = format!(
        "{}\n\nTask from parent agent: {task}",
        definition.system_prompt
    );
    let sub_chat_id = format!("{chat_id}::subagent::{}", definition.name);

    // Recursing into `orchestrate_agent_loop` from inside `execute_tool`
    // (which it itself calls) requires boxing this one call — Rust
    // can't compute a finite size for a directly self-referential
    // async fn cycle otherwise. `approve_cb` is reborrowed rather than
    // moved so the subagent's mutating actions still go through the
    // same approval gate as the caller's; `progress`/`chunk` are no-ops
    // so the subagent's internal steps never reach the parent's UI or
    // context — only its final summary is returned below.
    // `orchestrate_agent_loop` itself returns a boxed `dyn Future`
    // (see its doc comment) specifically so this recursive call can
    // just be awaited directly, with no manual boxing needed here.
    let result = orchestrate_agent_loop(
        &sub_config,
        &sub_task,
        root,
        None,
        None,
        None,
        Some(&sub_chat_id),
        None,
        None,
        true,
        false,
        &mut *approve_cb,
        |_| {},
        |_| {},
    )
    .await;

    match result {
        Ok(agent_result) => Ok(format!(
            "[Subagent '{}' result]\n{}",
            definition.name, agent_result.summary
        )),
        Err(error) => Err(OrchestrationError::Agent(format!(
            "subagent '{}' failed: {error}",
            definition.name
        ))),
    }
}

/// Runs a step's `dispatch_subagent` decisions concurrently (see
/// `decisions_are_parallel_subagent_batch` for when this is used instead of
/// the normal one-at-a-time loop), capped at `PARALLEL_SUBAGENT_LIMIT` in
/// flight at once. `progress`/`action_counts`/`trajectory` are only touched
/// after every future has completed — back in single-threaded code, in the
/// decisions' original order — so this never needs to share those across
/// concurrent tasks. `approve` is the one resource genuinely needed *during*
/// concurrent execution (a subagent's own risky actions still need real user
/// approval), so it's wrapped in a `std::sync::Mutex` for the batch: each
/// concurrent subagent gets a small adapter closure over a shared `&Mutex`,
/// serializing just the moment of invoking it rather than the whole call.
#[allow(clippy::too_many_arguments)]
async fn run_parallel_subagent_batch(
    decisions: &[(String, AgentDecision)],
    step: usize,
    root: &Path,
    config: &MintConfig,
    chat_id: &str,
    approve: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
    progress: &mut (dyn FnMut(AgentProgress) + Send),
    action_counts: &mut BTreeMap<String, usize>,
    trajectory: &mut Vec<String>,
) -> Vec<(String, String, Value, String)> {
    let approve_mutex = std::sync::Mutex::new(approve);

    let mut dispatches = Vec::with_capacity(decisions.len());
    for (index, (call_id, decision)) in decisions.iter().enumerate() {
        let call_id = call_id.clone();
        let thought = decision.thought.clone();
        let action = decision.action.clone();
        let input_val = serde_json::to_value(&decision.input).unwrap_or(Value::Null);
        let action_key = action_fingerprint(decision);
        let name = decision.input.name.clone();
        let task_text = decision.input.instruction.clone();
        let approve_mutex = &approve_mutex;
        dispatches.push(async move {
            let result: Result<String, OrchestrationError> = if name.trim().is_empty() {
                Err(OrchestrationError::Agent("name is required".into()))
            } else if task_text.trim().is_empty() {
                Err(OrchestrationError::Agent("instruction is required".into()))
            } else {
                let mut adapter = |approval: &AgentApproval| -> Result<ApprovalOutcome, String> {
                    let mut guard = approve_mutex.lock().unwrap();
                    (*guard)(approval)
                };
                dispatch_one_subagent(root, config, chat_id, &name, &task_text, &mut adapter).await
            };
            (
                index, call_id, thought, action, input_val, action_key, result,
            )
        });
    }

    let mut results = futures_util::stream::iter(dispatches)
        .buffer_unordered(PARALLEL_SUBAGENT_LIMIT)
        .collect::<Vec<_>>()
        .await;
    results.sort_by_key(|(index, ..)| *index);

    let mut step_tool_results = Vec::with_capacity(results.len());
    for (_, call_id, thought, action, input_val, action_key, result) in results {
        progress(AgentProgress::ToolStart {
            action: action.clone(),
            input: input_val.clone(),
        });
        let tool_result = match result {
            Ok(text) => text,
            Err(error) => format!("Error: {}", error),
        };
        progress(AgentProgress::ToolEnd {
            action: action.clone(),
            input: input_val.clone(),
            result: tool_result.clone(),
        });

        let action_count = {
            let count = action_counts.entry(action_key).or_insert(0);
            *count += 1;
            *count
        };
        let mut final_result = truncate(&tool_result);
        if action_count >= 3 {
            final_result.push_str(
                "\n\n[System Tip: You repeated the same tool action three or more times. \
                 Stop repeating it. If you already have enough information or the requested edit is done, \
                 use the finish action now. Otherwise choose a different necessary action.]",
            );
        }

        trajectory.push(format!(
            "Step {step}:\n- Thought: {}\n- Action: {}\n- Observation: {}",
            thought.trim(),
            action,
            final_result
        ));
        step_tool_results.push((call_id, action, input_val, final_result));
    }
    step_tool_results
}

// Takes a trait object rather than being generic over `Approve` like it used
// to be: `dispatch_subagent` recurses back into `orchestrate_agent_loop`
// (which is itself generic over its own `Approve`), and reborrowing a generic
// `&mut Approve` into that recursive call makes the type grow by one `&mut`
// layer per nesting level during monomorphization — `&mut Approve`, then
// `&mut &mut Approve`, then `&mut &mut &mut Approve`, forever, since the type
// system has no way to know the runtime depth limit that
// `tool_catalog`/`allow_subagent_dispatch` enforces. A `&mut dyn FnMut(...)`
// trait object is a single fixed type regardless of recursion depth, so it
// doesn't hit that.
async fn execute_tool(
    root: &Path,
    config: &MintConfig,
    decision: &AgentDecision,
    chat_id: &str,
    approve_cb: &mut (dyn FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send),
) -> Result<String, OrchestrationError> {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" => {
            let path = agent_read_path(root, &input.path, config)?;
            let entries = list_directory_entries(&path, input.limit.unwrap_or(100), config)?;
            Ok(serde_json::to_string_pretty(&entries)
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
            Ok(serde_json::to_string_pretty(
                &search_code(
                    &path,
                    required(&input.query, "query")?,
                    input.limit.unwrap_or(20),
                    config,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "symbols" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &build_symbol_index(&path, input.limit.unwrap_or(100), config)
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
            )
            .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "semantic_index" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(
                &index_semantic_code(&path, config)
                    .await
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
        "knowledge_search" => Ok(serde_json::to_string_pretty(
            &KnowledgeStore::open_default()
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?
                .search(required(&input.query, "query")?, input.limit.unwrap_or(5))
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
        )
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
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
                                let image_line = h
                                    .image_url
                                    .as_deref()
                                    .map(|img| format!("   Image: {img}\n"))
                                    .unwrap_or_default();
                                format!(
                                    "{}. {}\n   URL: {}\n{}{}\n",
                                    i + 1,
                                    h.title,
                                    h.url,
                                    image_line,
                                    h.snippet
                                )
                            })
                            .collect::<Vec<_>>()
                            .join("\n");
                        Ok(format!(
                            "{formatted}\n\nNote: Web search succeeded using {provider} Search. In your finish summary, you MUST:\n1. Answer the user's question using the information above.\n2. Mention that you found this information via {provider} Search (e.g. \"I found this information using {provider} Search.\").\n3. INLINE IMAGES: For each result that includes an 'Image:' URL, embed it in your summary using standard markdown image syntax:\n   ![result title](image_url)\n   Place the image tag on its OWN LINE, immediately AFTER the bullet point or paragraph that references that result.\n   Only embed images when they add visual value — e.g. food, restaurants, travel, products, people, art, profiles.\n   Do NOT embed images for code snippets, math, API docs, or pure text answers.\nDo NOT list source URLs manually — the UI will display them automatically."
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
        "image_search" => {
            let query = required(&input.query, "query")?;
            let limit = input.limit.unwrap_or(6);
            match crate::image_search::image_search(query, limit, config).await {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Image search succeeded. In your finish summary, you MUST include the exact ```image_search_json ... ``` code block from above in your response so the user sees the image results UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Image search failed for '{query}': {e}")),
            }
        }
        "weather" => {
            let city = if !input.city.trim().is_empty() {
                input.city.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.path.trim().is_empty() {
                input.path.trim()
            } else {
                "Thailand"
            };
            match crate::weather::weather(city).await {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Weather lookup succeeded. In your finish summary, you MUST include the exact ```weather_json ... ``` code block from above in your response so the user sees the weather card UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Weather lookup failed for {city}: {e}")),
            }
        }
        "stock" => {
            let symbol = if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.name.trim().is_empty() {
                input.name.trim()
            } else if !input.path.trim().is_empty() {
                input.path.trim()
            } else {
                "AAPL"
            };
            match crate::stock::stock(symbol).await {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Stock lookup succeeded. In your finish summary, you MUST include the exact ```stock_json ... ``` code block from above in your response so the user sees the stock card UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Stock lookup failed for {symbol}: {e}")),
            }
        }
        "calculation" => {
            let expr = if !input.expression.trim().is_empty() {
                input.expression.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else if !input.command.trim().is_empty() {
                input.command.trim()
            } else {
                "0"
            };
            match crate::calculation::calculate(expr) {
                Ok(report) => Ok(format!(
                    "{}\n\nNote: Calculation succeeded. In your finish summary, you MUST include the exact ```calculation_json ... ``` code block from above in your response so the user sees the calculation card UI.",
                    report.data
                )),
                Err(e) => Ok(format!("Calculation failed for {expr}: {e}")),
            }
        }
        "browser_open" => {
            let url = if !input.url.is_empty() {
                &input.url
            } else if !input.path.is_empty() {
                &input.path
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_open requires 'url'".into(),
                ));
            };
            if crate::is_browser_running(config).await {
                let result = crate::browser::navigate(config, url)
                    .await
                    .map_err(OrchestrationError::Agent)?;
                Ok(result)
            } else {
                let opened = if cfg!(target_os = "macos") {
                    std::process::Command::new("open").arg(url).spawn().is_ok()
                } else if cfg!(target_os = "windows") {
                    std::process::Command::new("cmd")
                        .args(["/C", "start", url])
                        .spawn()
                        .is_ok()
                } else {
                    std::process::Command::new("xdg-open")
                        .arg(url)
                        .spawn()
                        .is_ok()
                };
                if opened {
                    Ok(format!(
                        "Mint Auto is not active. Opened {url} in your default browser instead."
                    ))
                } else {
                    Err(OrchestrationError::Agent(
                        "Failed to open URL in default browser.".into(),
                    ))
                }
            }
        }
        "browser_click" => {
            let selector = if !input.selector.is_empty() {
                &input.selector
            } else if !input.path.is_empty() {
                &input.path
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_click requires 'selector'".into(),
                ));
            };
            let result = crate::browser::click(config, selector)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_type" => {
            let selector = if !input.selector.is_empty() {
                &input.selector
            } else if !input.path.is_empty() {
                &input.path
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_type requires 'selector'".into(),
                ));
            };
            let text = if !input.text.is_empty() {
                &input.text
            } else if !input.query.is_empty() {
                &input.query
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_type requires 'text'".into(),
                ));
            };
            let result = crate::browser::type_text(config, selector, text)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_read" => {
            let result = crate::browser::read_page_text(config)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_mouse_move" => {
            let x = input.x.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_move requires 'x'".into())
            })?;
            let y = input.y.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_move requires 'y'".into())
            })?;
            let result = crate::browser::mouse_move(config, x, y)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_mouse_click" => {
            let x = input.x.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_click requires 'x'".into())
            })?;
            let y = input.y.ok_or_else(|| {
                OrchestrationError::Agent("browser_mouse_click requires 'y'".into())
            })?;
            let button = if input.button.is_empty() {
                "left"
            } else {
                &input.button
            };
            let result = crate::browser::mouse_click(config, x, y, button)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_key_press" => {
            let key = if !input.key.is_empty() {
                &input.key
            } else {
                return Err(OrchestrationError::Agent(
                    "browser_key_press requires 'key'".into(),
                ));
            };
            let result = crate::browser::key_press(config, key)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(result)
        }
        "browser_screenshot" => {
            let data = crate::browser::screenshot(config)
                .await
                .map_err(OrchestrationError::Agent)?;
            Ok(format!("data:image/png;base64,{data}"))
        }
        "memory_recall" => {
            let query = required(&input.query, "query")?;
            let query_lower = query.to_ascii_lowercase();
            let mut results = Vec::new();

            if let Ok(memory) = MemoryStore::open_default() {
                if let Ok(interactions) = memory.recent_interactions_for_chat(chat_id, 50) {
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
        "git_status" => run_git(root, &["status", "--short", "--branch"]),
        "git_diff" => {
            if input.path.trim().is_empty() {
                run_git(root, &["diff", "--"])
            } else {
                let path = workspace_path(root, &input.path)?;
                let relative = path.strip_prefix(root).unwrap_or(&path).to_string_lossy();
                run_git(root, &["diff", "--", relative.as_ref()])
            }
        }
        "git_log" => {
            let limit = input.limit.unwrap_or(5).clamp(1, 50).to_string();
            run_git(root, &["log", "-n", &limit, "--oneline", "--decorate"])
        }
        "git_branch" => run_git(root, &["branch", "--show-current"]),
        "create_plan" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "objective": input.summary,
            "steps": input.steps,
        }))
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
        "update_plan" => Ok(serde_json::to_string_pretty(&serde_json::json!({
            "steps": input.steps,
            "status": input.status,
        }))
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
        "request_user_approval" => {
            let title = if input.title.trim().is_empty() {
                "User approval"
            } else {
                input.title.trim()
            };
            let prompt = required(&input.summary, "summary")?;
            let approved = approve_cb(&AgentApproval::UserApproval {
                title: title.to_owned(),
                prompt: prompt.to_owned(),
            })
            .map_err(OrchestrationError::Agent)?;
            match approved {
                ApprovalOutcome::Approved => Ok(format!("User approved: {title}")),
                ApprovalOutcome::Denied => Ok(format!("User denied: {title}")),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "ask_user" => {
            let question = required(&input.query, "query")?;
            let options: Vec<String> = input
                .options
                .iter()
                .map(|o| o.trim().to_owned())
                .filter(|o| !o.is_empty())
                .take(3)
                .collect();
            let approved = approve_cb(&AgentApproval::AskUser {
                question: question.to_owned(),
                options,
            })
            .map_err(OrchestrationError::Agent)?;
            match approved {
                ApprovalOutcome::Approved => Ok("User approved the prompt.".into()),
                ApprovalOutcome::Denied => Ok("User declined to answer.".into()),
                ApprovalOutcome::Intercepted(answer) => Ok(format!("User answered: {answer}")),
            }
        }
        "detect_project" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(&detect_project(&path))
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "list_tests" => {
            let path = workspace_path(root, &input.path)?;
            Ok(serde_json::to_string_pretty(&list_tests(&path, config)?)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
        }
        "read_diagnostics" => {
            let path = workspace_path(root, &input.path)?;
            read_diagnostics(&path, config)
        }
        "view_image" => {
            let path = workspace_path(root, required(&input.path, "path")?)?;
            view_image(&path, config)
        }
        "note_write" => {
            let file_name = if !input.note_path.is_empty() {
                input.note_path.as_str()
            } else {
                required(&input.path, "path")?
            };
            if file_name.contains("..") || file_name.contains('/') {
                return Err(OrchestrationError::Agent(
                    "note_write path must be a simple filename".into(),
                ));
            }
            let notes_dir = dirs::config_dir()
                .ok_or_else(|| {
                    OrchestrationError::Agent("cannot determine config directory".into())
                })?
                .join("mint")
                .join("notes");
            let note_path = notes_dir.join(file_name);

            let approved = approve_cb(&AgentApproval::NoteWrite {
                path: file_name.to_owned(),
                content: input.file_content.clone(),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => {
                    std::fs::create_dir_all(&notes_dir).map_err(|e| {
                        OrchestrationError::Agent(format!("cannot create notes directory: {}", e))
                    })?;
                    std::fs::write(&note_path, &input.file_content).map_err(|e| {
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
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => Ok(execute_native_plugin(config, name, instruction)
                    .await
                    .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => Ok(format!("User denied plugin execution: {}", name)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "dispatch_subagent" => {
            let name = required(&input.name, "name")?;
            let task = required(&input.instruction, "instruction")?;
            dispatch_one_subagent(root, config, chat_id, name, task, approve_cb).await
        }
        "mcp_tool" => {
            let server = required(&input.server, "server")?;
            let tool = required(&input.tool, "tool")?;
            let approved = approve_cb(&AgentApproval::McpTool {
                server: server.to_owned(),
                tool: tool.to_owned(),
                arguments: input.arguments.clone(),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => Ok(serde_json::to_string_pretty(
                    &crate::mcp::call_mcp_tool(config, server, tool, input.arguments.clone())
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => {
                    Ok(format!("User denied MCP tool call: {} {}", server, tool))
                }
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "mcp_list_tools" => {
            let server = required(&input.server, "server")?;
            let approved = approve_cb(&AgentApproval::McpTool {
                server: server.to_owned(),
                tool: "list_tools".to_owned(),
                arguments: serde_json::json!({}),
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => Ok(serde_json::to_string_pretty(
                    &crate::mcp::list_server_tools(config, server)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?,
                )
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?),
                ApprovalOutcome::Denied => Ok(format!("User denied MCP list tools: {}", server)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "run_shell" => {
            let command = required(&input.command, "command")?;
            let mode = classify_shell_command(command).mode.as_str().to_owned();
            let approved = approve_cb(&AgentApproval::RunShell {
                command: command.to_owned(),
                mode,
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => run_shell(root, config, command),
                ApprovalOutcome::Denied => Ok(format!("User denied shell command: {}", command)),
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "verify" => {
            if input.commands.is_empty() {
                return Err(OrchestrationError::Agent(
                    "verify requires at least one command".into(),
                ));
            }
            let mut output = Vec::new();
            for command in &input.commands {
                output.push(run_shell(root, config, command)?);
            }
            Ok(output.join("\n\n"))
        }
        "apply_patch" => {
            let patch = input.patch.as_ref().ok_or_else(|| {
                OrchestrationError::Agent("apply_patch requires patch input".into())
            })?;
            if patch.hunks.is_empty() {
                return Err(OrchestrationError::Agent(
                    "apply_patch requires at least one hunk".into(),
                ));
            }
            let edit = build_code_patch(root, patch.path.clone(), &patch.hunks, config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let diff = proposal
                .edits
                .iter()
                .map(|e| e.diff.clone())
                .collect::<Vec<_>>()
                .join("\n");

            let approved = approve_cb(&AgentApproval::ApplyPatch {
                path: patch.path.to_string_lossy().into_owned(),
                hunks: patch.hunks.clone(),
                diff,
            })
            .map_err(OrchestrationError::Agent)?;

            match approved {
                ApprovalOutcome::Approved => {
                    let applied = apply_code_edits(root, &[edit], &proposal.approval_token, config)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
                    Ok(serde_json::to_string_pretty(&applied)
                        .map_err(|e| OrchestrationError::Agent(e.to_string()))?)
                }
                ApprovalOutcome::Denied => {
                    Ok(format!("User denied file edit: {}", edit.path.display()))
                }
                ApprovalOutcome::Intercepted(obs) => Ok(obs),
            }
        }
        "write_file" => {
            let path_str = required(&input.path, "path")?;
            validate_new_workspace_file(root, config, Path::new(path_str))?;
            let edit = CodeEdit {
                path: PathBuf::from(path_str),
                content: input.file_content.clone(),
            };
            let proposal = propose_code_edits(root, std::slice::from_ref(&edit), config)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let diff = proposal
                .edits
                .iter()
                .map(|e| e.diff.clone())
                .collect::<Vec<_>>()
                .join("\n");

            let approved = approve_cb(&AgentApproval::WriteFile {
                path: path_str.to_owned(),
                content: input.file_content.clone(),
                diff,
            })
            .map_err(OrchestrationError::Agent)?;

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
        "video_trim" | "video.trim" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::TrimRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                start: input.start.unwrap_or(0.0),
                end: input.end.unwrap_or(0.0),
            };
            let res = crate::video_edit::video_trim(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_remove_silence" | "video.remove_silence" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::RemoveSilenceRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                threshold_db: input.threshold_db.unwrap_or(-30.0),
                min_silence_secs: input.min_silence_secs.unwrap_or(0.5),
            };
            let res = crate::video_edit::video_remove_silence(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_resize" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ResizeRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                width: input.width.unwrap_or(1920),
                height: input.height.unwrap_or(1080),
            };
            let res = crate::video_edit::video_resize(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_merge" => {
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::MergeRequest {
                inputs: if input.inputs.is_empty() {
                    input.commands.clone()
                } else {
                    input.inputs.clone()
                },
                output: output_path.to_string(),
            };
            let res = crate::video_edit::video_merge(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_export" | "video.export" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ExportRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                resolution: input.preset.clone(),
                fps: None,
                codec: None,
                crf: None,
            };
            let res = crate::video_edit::video_export(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "video_extract_audio" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ExtractAudioRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
            };
            let out = crate::video_edit::video_extract_audio(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(format!("Audio extracted to {}", out.output_path))
        }
        "speech_transcribe" | "subtitle_generate" | "subtitle.generate" => {
            let input_path = required(&input.input, "input")?;
            let req = crate::speech::TranscribeRequest {
                input: input_path.to_string(),
                language: input.language.clone(),
                prompt: None,
            };
            let res = crate::speech::transcribe(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "subtitle_translate" | "subtitle.translate" => {
            let srt = input.srt_content.as_deref().unwrap_or_default();
            let target = input.target_language.as_deref().unwrap_or("th");
            let req = crate::subtitle::TranslateSubtitleRequest {
                srt_content: srt.to_string(),
                target_language: target.to_string(),
            };
            let translated = crate::subtitle::translate_subtitles(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(translated)
        }
        "subtitle_burn" => {
            let input_video = required(&input.input, "input")?;
            let output_video = required(&input.output, "output")?;
            let srt_input = input.srt_content.as_deref().unwrap_or_default();
            let req = crate::subtitle::BurnSubtitleRequest {
                input_video: input_video.to_string(),
                srt_input: srt_input.to_string(),
                output_video: output_video.to_string(),
                style: None,
                preset: input.preset.clone(),
            };
            let res = crate::subtitle::burn_subtitles(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "timeline_reorder" | "timeline.reorder" => {
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ReorderClipsRequest {
                inputs: input.inputs.clone(),
                order: input.order.clone(),
                output: output_path.to_string(),
            };
            let res = crate::video_edit::timeline_reorder(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "effect_zoom_on_speaker" | "effect.zoom_on_speaker" => {
            let input_path = required(&input.input, "input")?;
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::ZoomSpeakerRequest {
                input: input_path.to_string(),
                output: output_path.to_string(),
                zoom_factor: input.zoom_factor.unwrap_or(1.25),
            };
            let res = crate::video_edit::effect_zoom_on_speaker(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "audio_duck_music" | "audio.duck_music" => {
            let video_in = input.video_input.as_deref().unwrap_or(&input.input);
            let music_in = input.music_input.as_deref().unwrap_or("");
            let output_path = required(&input.output, "output")?;
            let req = crate::video_edit::DuckMusicRequest {
                video_input: video_in.to_string(),
                music_input: music_in.to_string(),
                output: output_path.to_string(),
                music_volume: input.music_volume.unwrap_or(0.2),
            };
            let res = crate::video_edit::audio_duck_music(&req)
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "make_shorts" | "video.make_shorts" => {
            let input_path = required(&input.input, "input")?;
            let req = crate::auto_shorts::MakeShortsRequest {
                input: input_path.to_string(),
                output_dir: if input.output.is_empty() {
                    None
                } else {
                    Some(input.output.clone())
                },
                max_clips: input.max_clips.unwrap_or(3),
                target_duration: input.target_duration.unwrap_or(60.0),
                burn_subtitles: true,
                width: input.width.unwrap_or(1080),
                height: input.height.unwrap_or(1920),
            };
            let res = crate::auto_shorts::make_shorts(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            Ok(serde_json::to_string(&res).unwrap_or_default())
        }
        "generate_image" | "image_studio.generate" | "image_generate" => {
            let prompt_text = if !input.prompt.trim().is_empty() {
                input.prompt.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else {
                required(&input.text, "prompt")?
            };
            let req = crate::image_gen::ImageGenRequest {
                prompt: prompt_text.to_string(),
                aspect_ratio: if input.aspect_ratio.is_empty() {
                    Some("1:1".to_string())
                } else {
                    Some(input.aspect_ratio.clone())
                },
                provider: if input.provider.is_empty() {
                    None
                } else {
                    Some(input.provider.clone())
                },
                num_images: Some(1),
                ..Default::default()
            };
            let res = crate::image_gen::generate_images(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            let data_uris: Vec<String> = res.images.iter().map(|i| i.data_uri.clone()).collect();
            if let Ok(saved) = crate::pictures::save_chat_images(
                data_uris,
                Some(res.provider.clone()),
                Some(prompt_text.to_string()),
            ) {
                if let Some(first_saved) = saved.first() {
                    let img_url = format!("/api/pictures/{}", first_saved.filename);
                    let saved_path = first_saved.path.display();
                    let img_md = format!(
                        "![Generated Image]({})\n\n✓ Image generated successfully with model `{}` ({})\nSaved to: {}\n\nNote: In your final response or finish summary, you MUST copy the exact image markdown (`![Generated Image]({})`), model feedback (`✓ Image generated successfully...`), and saved path line (`Saved to: {}`) so the user can see them in their chat bubble.",
                        img_url, res.model, res.provider, saved_path, img_url, saved_path
                    );
                    return Ok(img_md);
                }
            }
            if let Some(first) = res.images.first() {
                let img_md = format!(
                    "![Generated Image]({})\n\n✓ Image generated successfully with model `{}` ({})",
                    first.data_uri, res.model, res.provider
                );
                Ok(img_md)
            } else {
                Ok("No image returned from provider".to_string())
            }
        }
        "generate_video" | "veo.generate" | "video_generate" => {
            let prompt_text = if !input.prompt.trim().is_empty() {
                input.prompt.trim()
            } else if !input.query.trim().is_empty() {
                input.query.trim()
            } else {
                required(&input.text, "prompt")?
            };
            let req = crate::video_gen::VideoGenRequest {
                prompt: prompt_text.to_string(),
                negative_prompt: None,
                aspect_ratio: if input.aspect_ratio.is_empty() {
                    "16:9".to_string()
                } else {
                    input.aspect_ratio.clone()
                },
                duration: input.duration.unwrap_or(5.0) as u32,
                model: None,
                provider: if input.provider.is_empty() {
                    "veo".to_string()
                } else {
                    input.provider.clone()
                },
            };
            let res = crate::video_gen::generate_video(config, &req)
                .await
                .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
            if let Some(first) = res.videos.first() {
                let vid_md = format!(
                    "<video controls src=\"{}\" width=\"100%\" style=\"max-height:400px; border-radius:8px;\"></video>\n\n✓ Video generated successfully with Veo `{}` ({})",
                    first.path.to_string_lossy(),
                    res.model,
                    res.provider
                );
                Ok(vid_md)
            } else {
                Ok("No video returned from provider".to_string())
            }
        }
        other => Err(OrchestrationError::Agent(format!(
            "unsupported code-agent action '{}'",
            other
        ))),
    }
}

/// Bridge for callers outside this module (e.g. the Gemini Live realtime voice
/// session) that receive an action name and raw JSON input rather than a parsed
/// `AgentDecision`, and so can't construct one directly since its fields are
/// private to this module.
pub(crate) async fn execute_tool_from_json<Approve>(
    root: &Path,
    config: &MintConfig,
    action: &str,
    input: Value,
    chat_id: &str,
    approve_cb: &mut Approve,
) -> Result<String, OrchestrationError>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send,
{
    let decision = AgentDecision {
        thought: String::new(),
        action: action.to_string(),
        input: serde_json::from_value(input).unwrap_or_default(),
    };
    execute_tool(root, config, &decision, chat_id, approve_cb).await
}

fn validate_new_workspace_file(
    root: &Path,
    config: &MintConfig,
    path: &Path,
) -> Result<(), OrchestrationError> {
    let root = assert_path_capability(root, Capability::Write, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let target = assert_path_capability(&root.join(path), Capability::Write, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    if !target.starts_with(&root) {
        return Err(OrchestrationError::Agent(format!(
            "write_file path escapes workspace root: {}",
            target.display()
        )));
    }
    if target.exists() {
        return Err(OrchestrationError::Agent(format!(
            "write_file can only create new files. Use apply_patch for existing file: {}",
            target.display()
        )));
    }
    Ok(())
}

fn run_git(root: &Path, args: &[&str]) -> Result<String, OrchestrationError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(root)
        .output()
        .map_err(|e| OrchestrationError::Agent(format!("unable to run git: {e}")))?;
    Ok(format!(
        "exit: {}\nstdout:\n{}\nstderr:\n{}",
        output.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ))
}

fn detect_project(root: &Path) -> Value {
    let mut languages = Vec::new();
    let mut managers = Vec::new();
    let mut diagnostics = Vec::new();
    if root.join("Cargo.toml").exists() {
        languages.push("rust");
        managers.push("cargo");
        diagnostics.push("cargo check");
    }
    if root.join("package.json").exists() {
        languages.push("javascript/typescript");
        managers.push(if root.join("pnpm-lock.yaml").exists() {
            "pnpm"
        } else if root.join("yarn.lock").exists() {
            "yarn"
        } else {
            "npm"
        });
        diagnostics.push("npm run build or npm run typecheck");
    }
    if root.join("pyproject.toml").exists() || root.join("requirements.txt").exists() {
        languages.push("python");
        managers.push("pip/uv");
        diagnostics.push("pytest or python -m compileall");
    }
    serde_json::json!({
        "root": root,
        "languages": languages,
        "packageManagers": managers,
        "diagnostics": diagnostics,
    })
}

fn list_tests(root: &Path, config: &MintConfig) -> Result<Value, OrchestrationError> {
    let files = list_code_files(root, usize::MAX, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let test_files = files
        .into_iter()
        .filter(|file| {
            let path = file.path.to_string_lossy();
            path.contains("/tests/")
                || path.ends_with("_test.rs")
                || path.ends_with(".test.ts")
                || path.ends_with(".test.tsx")
                || path.ends_with(".spec.ts")
                || path.ends_with(".spec.tsx")
                || path.ends_with("_test.py")
        })
        .map(|file| file.path)
        .collect::<Vec<_>>();
    let package_scripts = package_test_scripts(root);
    Ok(serde_json::json!({
        "testFiles": test_files,
        "packageScripts": package_scripts,
        "cargo": root.join("Cargo.toml").exists(),
    }))
}

fn package_test_scripts(root: &Path) -> BTreeMap<String, String> {
    let path = root.join("package.json");
    let Ok(raw) = std::fs::read_to_string(path) else {
        return BTreeMap::new();
    };
    let Ok(value) = serde_json::from_str::<Value>(&raw) else {
        return BTreeMap::new();
    };
    value
        .get("scripts")
        .and_then(Value::as_object)
        .into_iter()
        .flatten()
        .filter(|(name, _)| {
            let lower = name.to_ascii_lowercase();
            lower.contains("test")
                || lower.contains("check")
                || lower.contains("lint")
                || lower.contains("build")
                || lower.contains("type")
        })
        .filter_map(|(name, command)| Some((name.clone(), command.as_str()?.to_owned())))
        .collect()
}

fn read_diagnostics(root: &Path, config: &MintConfig) -> Result<String, OrchestrationError> {
    let command = if root.join("Cargo.toml").exists() {
        Some("cargo check")
    } else {
        let scripts = package_test_scripts(root);
        if scripts.contains_key("typecheck") {
            Some("npm run -s typecheck")
        } else if scripts.contains_key("check") {
            Some("npm run -s check")
        } else if scripts.contains_key("build") {
            Some("npm run -s build")
        } else {
            None
        }
    };
    match command {
        Some(command) => run_shell(root, config, command),
        None => Ok("No diagnostics command detected.".into()),
    }
}

fn view_image(path: &Path, config: &MintConfig) -> Result<String, OrchestrationError> {
    let path = assert_path_capability(path, Capability::Read, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let mime = match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        _ => {
            return Err(OrchestrationError::Agent(format!(
                "unsupported image type: {}",
                path.display()
            )));
        }
    };
    let metadata = std::fs::metadata(&path)
        .map_err(|e| OrchestrationError::Agent(format!("cannot stat image: {e}")))?;
    if metadata.len() > 2_000_000 {
        return Ok(format!(
            "Image exists but is too large to inline ({} bytes): {}",
            metadata.len(),
            path.display()
        ));
    }
    let bytes = std::fs::read(&path)
        .map_err(|e| OrchestrationError::Agent(format!("cannot read image: {e}")))?;
    serde_json::to_string_pretty(&serde_json::json!({
        "path": path,
        "bytes": bytes.len(),
        "mime": mime,
        "dataUri": format!("data:{mime};base64,{}", BASE64_STANDARD.encode(bytes)),
    }))
    .map_err(|e| OrchestrationError::Agent(e.to_string()))
}

/// Renders a slice of `native_messages` back into readable text for the
/// compaction summarizer prompt. Self-contained to `ChatMessage`/`ContentBlock`
/// rather than reusing the parallel `trajectory: Vec<String>` log, since that
/// log gets one entry per *tool call* while `native_messages` gets one
/// Assistant/Tool pair per *step* (a step can batch multiple tool calls) —
/// keeping the two aligned would need extra bookkeeping for no real benefit.
fn render_messages_as_text(messages: &[ChatMessage]) -> String {
    messages
        .iter()
        .map(|message| {
            let rendered = message
                .content
                .iter()
                .map(|block| match block {
                    ContentBlock::Text { text } => text.clone(),
                    ContentBlock::ToolUse { name, input, .. } => {
                        format!("Called {name} with {input}")
                    }
                    ContentBlock::ToolResult { content, .. } => format!("Result: {content}"),
                    ContentBlock::Image { .. } => "[image]".to_string(),
                    ContentBlock::Audio { .. } => "[audio]".to_string(),
                    ContentBlock::Video { .. } => "[video]".to_string(),
                })
                .collect::<Vec<_>>()
                .join("\n");
            format!("{:?}: {rendered}", message.role)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// Compacts the older portion of `native_messages` into a single synthetic
/// step-pair summary once the conversation is approaching the model's context
/// window, keeping the last `COMPACTION_KEEP_RECENT_STEPS` step-pairs verbatim.
///
/// `messages[0]` is always the initial task/observation message, and every
/// message after it is a strict repeating `[Assistant, Tool]` pair — one pair
/// per outer agent-loop step, even when that step batched multiple tool calls
/// (see the loop body). Cutting only on pair boundaries means the result
/// always preserves valid role alternation for every provider, with no
/// special-casing needed elsewhere.
///
/// `Ok(None)` means there was nothing worth compacting yet (too little history)
/// — not a failure, just a no-op. `Err` means compaction was attempted but the
/// summarization call itself failed; compaction is a best-effort optimization,
/// so callers should fall back to the uncompacted messages rather than failing
/// the agent run, but may want to surface the failure differently than a
/// routine no-op.
async fn compact_native_messages(
    config: &MintConfig,
    messages: &[ChatMessage],
) -> Result<Option<Vec<ChatMessage>>, ChatError> {
    let step_pairs = messages.len().saturating_sub(1) / 2;
    if step_pairs <= COMPACTION_KEEP_RECENT_STEPS || messages.is_empty() {
        return Ok(None);
    }
    let compact_pairs = step_pairs - COMPACTION_KEEP_RECENT_STEPS;
    let compact_message_count = compact_pairs * 2;
    let compacted_range = &messages[1..1 + compact_message_count];

    let transcript = render_messages_as_text(compacted_range);
    let summary_prompt = format!(
        "Summarize the following part of an autonomous coding agent's work log concisely but \
         completely. Preserve: exact file paths touched and their resulting state, exact \
         commands run and whether they succeeded, key findings from searches/reads, and any \
         decisions or open threads still relevant to finishing the task. Omit verbose \
         stdout/stderr detail that isn't load-bearing. Write it as dense prose, not a copy of \
         the log.\n\n{transcript}"
    );

    let (summary_response, _) = send_chat_with_fallback(
        config,
        &ChatRequest {
            message: summary_prompt,
            system_instruction: "You compress agent work logs into dense, factual summaries."
                .into(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        },
    )
    .await?;

    let mut compacted = Vec::with_capacity(messages.len() - compact_message_count + 3);
    compacted.push(messages[0].clone());
    compacted.push(ChatMessage {
        role: ChatRole::Assistant,
        content: vec![ContentBlock::ToolUse {
            id: "compacted_summary".into(),
            name: "conversation_summary".into(),
            input: serde_json::json!({}),
            thought_signature: None,
        }],
    });
    compacted.push(ChatMessage {
        role: ChatRole::Tool,
        content: vec![ContentBlock::ToolResult {
            tool_use_id: "compacted_summary".into(),
            content: format!(
                "[Summary of steps 1-{compact_pairs}, compacted to save context]\n{}",
                summary_response.text.trim()
            ),
            is_error: false,
        }],
    });
    compacted.extend_from_slice(&messages[1 + compact_message_count..]);
    Ok(Some(compacted))
}

fn run_shell(
    root: &Path,
    config: &MintConfig,
    command: &str,
) -> Result<String, OrchestrationError> {
    let output = run_shell_command(command, root, true, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    let status_str = output
        .status
        .map_or_else(|| "unknown".into(), |status| status.to_string());

    let mut hint = "";
    let cmd_lower = command.to_lowercase();
    if output.success
        && (cmd_lower.contains("open")
            || cmd_lower.contains("launch")
            || cmd_lower.contains("chrome")
            || cmd_lower.contains("firefox"))
    {
        hint = "\nNote: Opening URLs, files, folders, or launching applications are background processes. Even if there are warnings or stdout/stderr outputs, since the command exited successfully with status 0, the operation has succeeded and you should now use the 'finish' action to inform the user.";
    }

    let warning_line = output
        .sandbox_warning
        .as_deref()
        .map(|warning| format!("\n[Warning] {warning}"))
        .unwrap_or_default();

    Ok(format!(
        "exit: {}\nmode: {}\nsandboxed: {}{}\nstdout:\n{}\nstderr:\n{}{}",
        status_str, output.mode, output.sandboxed, warning_line, output.stdout, output.stderr, hint
    ))
}

/// Cheap pre-filter run before the (costlier) auto-skill-writing reflection call:
/// only tasks that took several steps and did real work (edited files, ran shell
/// commands, drove the browser, or delegated to a subagent) are worth asking the
/// LLM to judge for skill-worthiness. Keeps trivial one-shot chats from spawning an
/// extra reflection call every time `auto_skill_writing` is enabled.
fn looks_skill_worthy(step: usize, action_counts: &BTreeMap<String, usize>) -> bool {
    const SUBSTANTIVE_ACTIONS: &[&str] = &[
        "apply_patch",
        "write_file",
        "run_shell",
        "browser_open",
        "browser_click",
        "browser_type",
        "dispatch_subagent",
    ];
    step >= 3
        && action_counts
            .keys()
            .any(|key| SUBSTANTIVE_ACTIONS.iter().any(|action| key.starts_with(action)))
}

fn action_fingerprint(decision: &AgentDecision) -> String {
    let input = &decision.input;
    match decision.action.as_str() {
        "list_files" | "read_file" | "symbols" => {
            format!("{}:{}", decision.action, input.path.trim())
        }
        "search_code" | "semantic_search" | "web_search" | "knowledge_search" | "memory_recall" => {
            format!(
                "{}:{}:{}",
                decision.action,
                input.path.trim(),
                input.query.trim()
            )
        }
        "git_status" | "git_branch" | "detect_project" | "list_tests" | "read_diagnostics" => {
            format!("{}:{}", decision.action, input.path.trim())
        }
        "git_diff" => format!("git_diff:{}", input.path.trim()),
        "git_log" => format!("git_log:{}", input.limit.unwrap_or(5)),
        "create_plan" | "update_plan" => format!("{}:{}", decision.action, input.steps.join("\n")),
        "request_user_approval" => format!("request_user_approval:{}", input.summary.trim()),
        "ask_user" => format!("ask_user:{}", input.query.trim()),
        "view_image" => format!("view_image:{}", input.path.trim()),
        "run_shell" => format!("run_shell:{}", input.command.trim()),
        "verify" => format!("verify:{}", input.commands.join("\n")),
        "apply_patch" => input
            .patch
            .as_ref()
            .map(|patch| format!("apply_patch:{}", patch.path.display()))
            .unwrap_or_else(|| "apply_patch:<missing>".to_owned()),
        "write_file" => format!("write_file:{}", input.path.trim()),
        other => other.to_owned(),
    }
}

/// Appends saved cross-session memory — the user's profile/preferences (Settings
/// → Memory) and this chat's recent interaction history — onto `system_prompt`.
/// Shared by the typed-chat agent loop and the Gemini Live bridge so a Live
/// session starts with the same "who is this user, what have we already
/// discussed" context instead of starting blank every call.
pub(crate) fn append_memory_context(system_prompt: &mut String, chat_id: &str) {
    let Ok(memory) = MemoryStore::open_default() else {
        return;
    };

    let mut profile_instructions = String::new();
    if let Ok(Some(name)) = memory.get_profile("name")
        && !name.trim().is_empty()
    {
        profile_instructions.push_str(&format!("User Name: {}\n", name.trim()));
    }
    if let Ok(Some(preferences)) = memory.get_profile("preferences")
        && !preferences.trim().is_empty()
    {
        profile_instructions.push_str(&format!(
            "User Preferences & Profile:\n{}\n",
            preferences.trim()
        ));
    }
    if !profile_instructions.is_empty() {
        *system_prompt = format!(
            "{}\n\nUser Profile Information:\n{}",
            system_prompt.trim(),
            profile_instructions.trim()
        );
    }

    if let Ok(mut interactions) = memory.recent_interactions_for_chat(chat_id, 6) {
        interactions.reverse();
        let transcript = interactions
            .into_iter()
            .map(|item| format!("User: {}\nAssistant: {}", item.user_text, item.ai_text))
            .collect::<Vec<_>>()
            .join("\n\n");
        if !transcript.is_empty() {
            *system_prompt = format!(
                "{}\n\nRecent conversation context:\n{}",
                system_prompt.trim(),
                transcript
            );
        }
    }
}

fn initial_observation(task: &str, root: &Path, skills: &str) -> String {
    let now = chrono::Local::now()
        .format("%Y-%m-%d %H:%M:%S %Z")
        .to_string();
    let mut observation = format!(
        "Current Time: {now}\nTask: {task}\nWorkspace: {}\nLearned skills:\n{}\n",
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
    let finish = value.get("finish").cloned().unwrap_or(Value::Null);
    let input = match finish {
        Value::Object(_) => serde_json::from_value(finish)?,
        Value::String(s) => AgentInput {
            summary: s,
            ..AgentInput::default()
        },
        _ => AgentInput::default(),
    };
    Ok(AgentDecision {
        thought: value
            .get("thought")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        action: "finish".into(),
        input,
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

fn list_directory_entries(
    path: &Path,
    limit: usize,
    config: &MintConfig,
) -> Result<Vec<AgentDirectoryEntry>, OrchestrationError> {
    let path = assert_path_capability(path, Capability::Read, config)
        .map_err(|e| OrchestrationError::Agent(e.to_string()))?;
    if !path.is_dir() {
        return Err(OrchestrationError::Agent(format!(
            "path is not a directory: {}",
            path.display()
        )));
    }

    let mut entries = Vec::new();
    let read_dir = std::fs::read_dir(&path).map_err(|e| {
        OrchestrationError::Agent(format!(
            "unable to read directory {}: {}",
            path.display(),
            e
        ))
    })?;
    for entry in read_dir.take(limit.max(1)) {
        let entry = entry.map_err(|e| {
            OrchestrationError::Agent(format!("unable to read directory entry: {e}"))
        })?;
        let entry_path = entry.path();
        let file_type = entry.file_type().map_err(|e| {
            OrchestrationError::Agent(format!(
                "unable to read file type for {}: {}",
                entry_path.display(),
                e
            ))
        })?;
        let size = if file_type.is_file() {
            entry.metadata().ok().map(|metadata| metadata.len())
        } else {
            None
        };
        entries.push(AgentDirectoryEntry {
            name: entry.file_name().to_string_lossy().into_owned(),
            path: entry_path,
            kind: if file_type.is_dir() {
                "directory"
            } else if file_type.is_file() {
                "file"
            } else if file_type.is_symlink() {
                "symlink"
            } else {
                "other"
            },
            size,
        });
    }
    entries.sort_by(|a, b| {
        a.kind
            .cmp(b.kind)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok(entries)
}

fn agent_read_path(
    root: &Path,
    value: &str,
    config: &MintConfig,
) -> Result<PathBuf, OrchestrationError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed == "." {
        return workspace_path(root, ".");
    }
    if let Ok(path) = workspace_path(root, trimmed) {
        return Ok(path);
    }

    let requested = Path::new(trimmed);
    let mut candidates = Vec::new();
    if let Some(home) = dirs::home_dir() {
        if trimmed == "~" {
            candidates.push(home.clone());
        } else if let Some(rest) = trimmed.strip_prefix("~/") {
            candidates.push(home.join(rest));
        } else if requested.components().count() == 1 {
            candidates.push(home.join(trimmed));
        }
    }
    if requested.is_absolute() {
        candidates.push(requested.to_path_buf());
    }

    for candidate in candidates {
        let Ok(path) = candidate.canonicalize() else {
            continue;
        };
        if assert_path_capability(&path, Capability::Read, config).is_ok() {
            return Ok(path);
        }
    }

    Err(OrchestrationError::Agent(format!(
        "unable to resolve readable path: {trimmed}"
    )))
}

/// When a `finish` attempt is rejected (empty summary, missing verification, ...),
/// native tool-calling mode's `messages` history must record both the model's
/// attempted finish and the rejection, or the next request would just resend the
/// same history with no signal anything was wrong: the JSON-prompt path gets the
/// rejection via `observation` (rebuilt from `trajectory` at each call site above),
/// but native mode stops reading `observation` once `native_messages` is non-empty
/// (see the `native_messages.is_empty()` guard near the top of the step loop).
/// No-op outside native mode, where `observation` alone is sufficient.
fn reject_native_finish(
    tool_mode: ToolCallingMode,
    native_messages: &mut Vec<ChatMessage>,
    response_text: &str,
    rejection: &str,
) {
    if tool_mode != ToolCallingMode::Native {
        return;
    }
    if !response_text.trim().is_empty() {
        native_messages.push(ChatMessage {
            role: ChatRole::Assistant,
            content: vec![ContentBlock::Text {
                text: response_text.trim().to_string(),
            }],
        });
    }
    native_messages.push(ChatMessage {
        role: ChatRole::User,
        content: vec![ContentBlock::Text {
            text: rejection.to_string(),
        }],
    });
}

/// Maximum number of `dispatch_subagent` calls run concurrently when a single
/// model turn requests several of them at once (see `orchestrate_agent_loop`'s
/// parallel-dispatch branch). Kept low: each subagent makes its own AI-provider
/// API calls, so a higher cap risks tripping the configured provider's rate
/// limit, and most tasks don't naturally decompose into more than a couple of
/// truly independent pieces anyway.
const PARALLEL_SUBAGENT_LIMIT: usize = 2;

/// Whether this step's decisions should run as a concurrency-limited batch of
/// subagent dispatches instead of the normal one-at-a-time loop: 2 or more
/// decisions, every one of them a `dispatch_subagent` call with nothing else
/// mixed in. A lone subagent call, or one mixed with other actions, stays on
/// the sequential path — keeps ordering between subagent results and other
/// tool results simple, and avoids parallelizing tools that were never
/// verified to be safe to run concurrently.
fn decisions_are_parallel_subagent_batch(decisions: &[(String, AgentDecision)]) -> bool {
    decisions.len() >= 2
        && decisions
            .iter()
            .all(|(_, d)| d.action == "dispatch_subagent")
}

/// Whether `finish` should be rejected because the run modified a file
/// (`apply_patch`/`write_file`) without a subsequent `verify` call and without
/// an explicit written reason in the `finish` action's `verification` field.
fn unverified_modification(
    last_modify_step: Option<usize>,
    last_verify_step: Option<usize>,
    verification_field: &str,
) -> bool {
    let Some(modify_step) = last_modify_step else {
        return false;
    };
    let verified_since = last_verify_step.is_some_and(|verify_step| verify_step >= modify_step);
    !verified_since && meaningful_verification(verification_field).is_empty()
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
    let path = path.canonicalize().map_err(|e| {
        OrchestrationError::Agent(format!(
            "unable to resolve workspace path {}: {}",
            path.display(),
            e
        ))
    })?;
    if !path.starts_with(root) {
        return Err(OrchestrationError::Agent(format!(
            "path is outside workspace: {}",
            path.display()
        )));
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

pub fn spawn_auto_memory_update(config: MintConfig, user_text: String, ai_text: String) {
    tokio::spawn(async move {
        if let Err(e) = auto_extract_and_update_memory(&config, &user_text, &ai_text).await {
            eprintln!("Auto memory update failed: {:?}", e);
        }
    });
}

/// Fire-and-forget: after a task finishes and passes [`looks_skill_worthy`], ask the
/// model (in a second, separate call) whether the task was a genuinely reusable
/// problem worth turning into a skill, and if so write
/// `<root>/.agents/skills/<slug>/SKILL.md`. Mirrors [`spawn_auto_memory_update`] —
/// never blocks or fails the already-returned [`AgentResult`].
pub fn spawn_auto_skill_write(
    config: MintConfig,
    task: String,
    summary: String,
    root: PathBuf,
    existing_skills: String,
) {
    tokio::spawn(async move {
        if let Err(e) = auto_write_skill(&config, &task, &summary, &root, &existing_skills).await
        {
            eprintln!("Auto skill write failed: {:?}", e);
        }
    });
}

async fn auto_write_skill(
    config: &MintConfig,
    task: &str,
    summary: &str,
    root: &Path,
    existing_skills: &str,
) -> Result<(), OrchestrationError> {
    let system_instruction = r#"You are a background agent that decides whether a just-completed
coding/agent task is worth turning into a reusable skill for future sessions.

A task is skill-worthy only if it was non-trivial (took real investigation or multiple
steps to solve) AND the solution generalizes beyond this one-off instance (a pattern,
workaround, command sequence, or gotcha that will plausibly recur). Do NOT save trivial
tasks, one-off questions, or anything already covered by an existing skill listed below
(reuse that skill's slug to update it instead of creating a near-duplicate).

You must return strictly valid JSON with no other text, markers, or markdown, and do NOT
wrap it in ```json fences. Two shapes are allowed:

Not worth saving:
{"should_save": false}

Worth saving:
{
  "should_save": true,
  "slug": "kebab-case-name",
  "description": "one-line summary of when this skill applies",
  "content": "full SKILL.md body as markdown, starting with YAML frontmatter:\n---\ndescription: one-line summary\n---\nthen step-by-step reusable instructions"
}"#
        .to_string();

    let message = format!(
        "Existing skills already known (avoid duplicating these; reuse a slug below to update it instead):\n{}\n\nTask:\n{}\n\nOutcome:\n{}",
        existing_skills, task, summary
    );

    let request = ChatRequest {
        message,
        system_instruction,
        chat_id: None,
        image_data_uri: None,
        audio_data_uri: None,
        video_data_uri: None,
        document_attachment: None,
        workspace_path: None,
        agent_id: None,
        plan_mode: false,
        messages: None,
        tools: None,
    };

    let response = send_chat(config, &request).await?;
    let text_reply = response.text.trim();

    let clean_json = if text_reply.starts_with("```") {
        let lines: Vec<&str> = text_reply.lines().collect();
        let mut filtered = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if !trimmed.starts_with("```") {
                filtered.push(trimmed);
            }
        }
        filtered.join("\n")
    } else {
        text_reply.to_string()
    };

    let Ok(value) = serde_json::from_str::<serde_json::Value>(&clean_json) else {
        return Ok(());
    };
    let Some(obj) = value.as_object() else {
        return Ok(());
    };
    if !obj
        .get("should_save")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return Ok(());
    }
    let (Some(slug), Some(content)) = (
        obj.get("slug").and_then(|v| v.as_str()),
        obj.get("content").and_then(|v| v.as_str()),
    ) else {
        return Ok(());
    };

    let slug = slugify(slug);
    if slug.is_empty() {
        return Ok(());
    }

    let skill_dir = root.join(".agents").join("skills").join(&slug);
    std::fs::create_dir_all(&skill_dir)
        .map_err(|e| OrchestrationError::Agent(format!("unable to create {skill_dir:?}: {e}")))?;
    std::fs::write(skill_dir.join("SKILL.md"), content)
        .map_err(|e| OrchestrationError::Agent(format!("unable to write SKILL.md: {e}")))?;

    Ok(())
}

/// Lowercases, replaces runs of non-alphanumeric characters with a single `-`, and
/// trims leading/trailing `-` — turns an arbitrary model-provided name into a safe
/// directory name under `.agents/skills/`.
fn slugify(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
            last_was_dash = false;
        } else if !last_was_dash && !slug.is_empty() {
            slug.push('-');
            last_was_dash = true;
        }
    }
    if slug.ends_with('-') {
        slug.pop();
    }
    slug
}

pub async fn auto_extract_and_update_memory(
    config: &MintConfig,
    user_text: &str,
    ai_text: &str,
) -> Result<(), OrchestrationError> {
    let memory = MemoryStore::open_default()?;

    // Retrieve current profile values
    let current_name = memory
        .get_profile("name")
        .unwrap_or(None)
        .unwrap_or_default();
    let current_pref = memory
        .get_profile("preferences")
        .unwrap_or(None)
        .unwrap_or_default();

    // System instruction for memory extraction
    let system_instruction = r#"You are a background agent responsible for updating a user's profile memory.
Analyze the latest conversation turn below.
Determine if the user shared their name, nickname, or any preferences, hobbies, or instructions on how they want the assistant to behave (e.g. language, formatting preference, details).
Update the existing Profile Name and Profile Preferences accordingly.
Keep existing preferences, add new ones, and resolve conflicts. Do not add metadata (like "preferred name") unless it is a generic preference. Keep formatting simple (e.g. list style or bullet points).
You must return the updated profile strictly as a valid JSON object with keys:
- "name": (string) updated name or same if not changed.
- "preferences": (string) updated preferences list or same if not changed.

Format the response strictly as valid JSON, with no other text, markers, or markdown.
Do NOT wrap the JSON in ```json ... ``` code blocks. Just output the raw JSON object.

Example response:
{
  "name": "Pheem",
  "preferences": "Always explain code step-by-step. Prefers TypeScript. Default language is Thai."
}"#.to_string();

    let message = format!(
        "Current Name: {}\nCurrent Preferences:\n{}\n\nLatest Turn:\nUser: {}\nAssistant: {}",
        current_name, current_pref, user_text, ai_text
    );

    let request = ChatRequest {
        message,
        system_instruction,
        chat_id: None,
        image_data_uri: None,
        audio_data_uri: None,
        video_data_uri: None,
        document_attachment: None,
        workspace_path: None,
        agent_id: None,
        plan_mode: false,
        messages: None,
        tools: None,
    };

    // Send the chat request to LLM
    let response = send_chat(config, &request).await?;
    let text_reply = response.text.trim();

    // Attempt to parse the JSON response
    let clean_json = if text_reply.starts_with("```") {
        let lines: Vec<&str> = text_reply.lines().collect();
        let mut filtered = Vec::new();
        for line in lines {
            let trimmed = line.trim();
            if !trimmed.starts_with("```") {
                filtered.push(trimmed);
            }
        }
        filtered.join("\n")
    } else {
        text_reply.to_string()
    };

    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&clean_json)
        && let Some(obj) = value.as_object()
    {
        if let Some(new_name) = obj.get("name").and_then(|v| v.as_str()) {
            let trimmed_name = new_name.trim();
            if !trimmed_name.is_empty() && trimmed_name != current_name {
                memory.set_profile("name", trimmed_name)?;
            }
        }
        if let Some(new_pref) = obj.get("preferences").and_then(|v| v.as_str()) {
            let trimmed_pref = new_pref.trim();
            if !trimmed_pref.is_empty() && trimmed_pref != current_pref {
                memory.set_profile("preferences", trimmed_pref)?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AgentConfig;

    #[test]
    fn slugify_lowercases_and_collapses_separators() {
        assert_eq!(slugify("Retry Flaky Playwright Tests!!"), "retry-flaky-playwright-tests");
        assert_eq!(slugify("  leading/trailing --dashes--  "), "leading-trailing-dashes");
        assert_eq!(slugify("already-a-slug"), "already-a-slug");
        assert_eq!(slugify("***"), "");
    }

    #[test]
    fn skill_worthy_requires_enough_steps_and_substantive_work() {
        let mut counts = BTreeMap::new();
        counts.insert("read_file:foo.rs".to_string(), 1);
        counts.insert("apply_patch:foo.rs".to_string(), 1);

        // Too few steps, even with a substantive action.
        assert!(!looks_skill_worthy(2, &counts));
        // Enough steps, has a substantive action.
        assert!(looks_skill_worthy(3, &counts));

        let mut read_only = BTreeMap::new();
        read_only.insert("read_file:foo.rs".to_string(), 5);
        read_only.insert("web_search::rust patterns".to_string(), 2);
        // Enough steps, but nothing substantive happened.
        assert!(!looks_skill_worthy(5, &read_only));
    }

    #[test]
    fn preserves_request_without_history() {
        let store = MemoryStore::open(
            std::env::temp_dir().join(format!("mint-orchestrator-{}.sqlite", std::process::id())),
        );
        let request = ChatRequest {
            message: "hello".into(),
            system_instruction: "system".into(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
            plan_mode: false,
            messages: None,
            tools: None,
        };
        let config = MintConfig::default();
        assert!(
            enrich_request(&config, &store, &request)
                .unwrap()
                .system_instruction
                .starts_with("system")
        );
    }

    #[test]
    fn agent_decision_allows_null_input() {
        let decision = parse_decision(r#"{"thought":"done","action":"finish","input":null}"#)
            .expect("null input should parse as default input");

        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());
    }

    #[test]
    fn agent_decision_allows_missing_input() {
        let decision = parse_decision(r#"{"thought":"done","action":"finish"}"#)
            .expect("missing input should parse as default input");

        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());
    }

    #[test]
    fn shorthand_finish_allows_null_or_missing_finish() {
        let decision = parse_decision(r#"{"thought":"done","finish":null}"#)
            .expect("null finish should parse");
        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());

        let decision =
            parse_decision(r#"{"thought":"done"}"#).expect("missing finish should parse");
        assert_eq!(decision.action, "finish");
        assert!(decision.input.summary.is_empty());
    }

    #[test]
    fn shorthand_finish_allows_string_finish_as_summary() {
        let decision = parse_decision(r#"{"thought":"done","finish":"all done!"}"#)
            .expect("string finish should parse as summary");
        assert_eq!(decision.action, "finish");
        assert_eq!(decision.input.summary, "all done!");
    }

    #[test]
    fn write_file_policy_rejects_existing_workspace_file() {
        let root =
            std::env::temp_dir().join(format!("mint-write-file-policy-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let target = root.join("existing.txt");
        std::fs::write(&target, "already here").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };

        let result = validate_new_workspace_file(&root, &config, Path::new("existing.txt"));

        assert!(
            matches!(result, Err(OrchestrationError::Agent(message)) if message.contains("Use apply_patch"))
        );
        let _ = std::fs::remove_file(target);
        let _ = std::fs::remove_dir(root);
    }

    #[test]
    fn agent_list_files_includes_directories() {
        let root = std::env::temp_dir().join(format!(
            "mint-agent-list-directories-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(root.join("Bunny Girl")).unwrap();
        std::fs::write(root.join("note.txt"), "hello").unwrap();
        let config = MintConfig {
            allowed_read_paths: vec![root.clone()],
            allowed_write_paths: vec![root.clone()],
            blocked_paths: vec![],
            blocked_file_names: vec![],
            ..MintConfig::default()
        };

        let entries = list_directory_entries(&root, 100, &config).unwrap();

        assert!(entries.iter().any(|entry| {
            entry.name == "Bunny Girl" && entry.kind == "directory" && entry.size.is_none()
        }));
        assert!(entries.iter().any(|entry| {
            entry.name == "note.txt" && entry.kind == "file" && entry.size == Some(5)
        }));
        let _ = std::fs::remove_file(root.join("note.txt"));
        let _ = std::fs::remove_dir(root.join("Bunny Girl"));
        let _ = std::fs::remove_dir(root);
    }

    #[test]
    fn grep_is_classified_as_read_only() {
        let classification = classify_shell_command("ls ~/Downloads | grep -F \"Bunny Girl\"");

        assert_eq!(classification.mode.as_str(), "readOnly");
    }

    #[test]
    fn test_resolve_agent_config() {
        let mut config = MintConfig::default();
        config.enable_agent_collaboration = true;
        config.agents = vec![
            AgentConfig {
                id: "planner".into(),
                name: "Planner".into(),
                provider: "openai".into(),
                model: "gpt-4o".into(),
                api_key: Some("test-key".into()),
                system_instruction: "Planner instruction".into(),
                enabled: true,
            },
            AgentConfig {
                id: "coder".into(),
                name: "Coder".into(),
                provider: "gemini".into(),
                model: "gemini-2.5-flash".into(),
                api_key: None,
                system_instruction: "Coder instruction".into(),
                enabled: true,
            },
        ];

        let (cfg, instr, name, model) = resolve_agent_config(&config, Some("planner"), &[]);
        assert_eq!(cfg.ai_provider, "openai");
        assert_eq!(cfg.openai_model, "gpt-4o");
        assert_eq!(cfg.openai_api_key, "test-key");
        assert_eq!(instr, "Planner instruction");
        assert_eq!(name, Some("Planner".to_string()));
        assert_eq!(model, Some("gpt-4o".to_string()));

        let (cfg, instr, name, _model) = resolve_agent_config(&config, None, &[]);
        assert_eq!(cfg.ai_provider, "openai");
        assert_eq!(instr, "Planner instruction");
        assert_eq!(name, Some("Planner".to_string()));

        let trajectory =
            vec!["Step 1:\n- Action: create_plan\n- Observation: all planned".to_string()];
        let (cfg, instr, name, model) = resolve_agent_config(&config, None, &trajectory);
        assert_eq!(cfg.ai_provider, "gemini");
        assert_eq!(instr, "Coder instruction");
        assert_eq!(name, Some("Coder".to_string()));
        assert_eq!(model, Some("gemini-2.5-flash".to_string()));
    }

    fn step_pair(index: usize) -> [ChatMessage; 2] {
        [
            ChatMessage {
                role: ChatRole::Assistant,
                content: vec![ContentBlock::ToolUse {
                    id: format!("call_{index}"),
                    name: "read_file".into(),
                    input: serde_json::json!({ "path": format!("file{index}.rs") }),
                    thought_signature: None,
                }],
            },
            ChatMessage {
                role: ChatRole::Tool,
                content: vec![ContentBlock::ToolResult {
                    tool_use_id: format!("call_{index}"),
                    content: format!("contents of file{index}.rs"),
                    is_error: false,
                }],
            },
        ]
    }

    fn native_messages_with_steps(step_count: usize) -> Vec<ChatMessage> {
        let mut messages = vec![ChatMessage::text(ChatRole::User, "do the task")];
        for i in 0..step_count {
            messages.extend(step_pair(i));
        }
        messages
    }

    #[tokio::test]
    async fn compact_native_messages_is_a_noop_when_history_is_short() {
        // COMPACTION_KEEP_RECENT_STEPS = 3, so exactly 3 step-pairs is nothing to compact yet.
        let messages = native_messages_with_steps(3);
        let config = MintConfig::default();
        let result = compact_native_messages(&config, &messages).await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn compact_native_messages_is_a_noop_for_empty_history() {
        let config = MintConfig::default();
        let result = compact_native_messages(&config, &[]).await.unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn render_messages_as_text_includes_tool_calls_and_results() {
        let messages = native_messages_with_steps(1);
        let rendered = render_messages_as_text(&messages);
        assert!(rendered.contains("Called read_file with"));
        assert!(rendered.contains("Result: contents of file0.rs"));
        assert!(rendered.contains("do the task"));
    }

    #[test]
    fn no_modification_means_finish_never_needs_verification() {
        // A pure Q&A run that never touched apply_patch/write_file can finish
        // immediately, regardless of what (if anything) is in `verification`.
        assert!(!unverified_modification(None, None, ""));
        assert!(!unverified_modification(None, Some(1), ""));
    }

    #[test]
    fn modification_without_any_verify_call_blocks_finish() {
        assert!(unverified_modification(Some(2), None, ""));
    }

    #[test]
    fn verify_called_before_the_modification_does_not_count() {
        // Editing again after the last verify invalidates that earlier check.
        assert!(unverified_modification(Some(3), Some(1), ""));
    }

    #[test]
    fn verify_called_after_the_modification_satisfies_the_gate() {
        assert!(!unverified_modification(Some(2), Some(3), ""));
    }

    #[test]
    fn verify_on_the_same_step_as_the_modification_satisfies_the_gate() {
        // Both markers can land on the same step if a single turn issues
        // multiple tool calls (apply_patch then verify back to back).
        assert!(!unverified_modification(Some(2), Some(2), ""));
    }

    #[test]
    fn explicit_written_justification_satisfies_the_gate_without_a_verify_call() {
        assert!(!unverified_modification(
            Some(2),
            None,
            "Documentation-only change; no test suite in this repo."
        ));
    }

    #[test]
    fn placeholder_verification_text_does_not_satisfy_the_gate() {
        // Mirrors `meaningful_verification`'s own placeholder filter — "n/a" etc.
        // must not be treated as a real justification.
        assert!(unverified_modification(Some(2), None, "n/a"));
        assert!(unverified_modification(Some(2), None, "not run"));
    }

    fn decision_with_action(action: &str) -> (String, AgentDecision) {
        (
            "call_0".to_string(),
            AgentDecision {
                thought: String::new(),
                action: action.to_string(),
                input: AgentInput::default(),
            },
        )
    }

    #[test]
    fn two_or_more_subagent_only_decisions_qualify_for_the_parallel_batch() {
        let decisions = vec![
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
        ];
        assert!(decisions_are_parallel_subagent_batch(&decisions));

        let decisions = vec![
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
        ];
        assert!(decisions_are_parallel_subagent_batch(&decisions));
    }

    #[test]
    fn a_lone_subagent_dispatch_stays_sequential() {
        let decisions = vec![decision_with_action("dispatch_subagent")];
        assert!(!decisions_are_parallel_subagent_batch(&decisions));
    }

    #[test]
    fn subagent_dispatch_mixed_with_another_action_stays_sequential() {
        let decisions = vec![
            decision_with_action("dispatch_subagent"),
            decision_with_action("dispatch_subagent"),
            decision_with_action("read_file"),
        ];
        assert!(!decisions_are_parallel_subagent_batch(&decisions));
    }

    #[test]
    fn empty_decisions_never_qualify_for_the_parallel_batch() {
        assert!(!decisions_are_parallel_subagent_batch(&[]));
    }
}

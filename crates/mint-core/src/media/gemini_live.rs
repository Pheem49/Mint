//! Bridge to Google's Gemini Live API (realtime speech-to-speech over WebSocket).
//!
//! This is a thin duplex proxy: mic audio pushed in from the frontend is forwarded to
//! Gemini as it arrives, and audio/transcript/tool-call events coming back from Gemini
//! are forwarded out. Tool calls are dispatched through the same `execute_tool_from_json`
//! bridge the text-based agent loop uses (`crate::orchestration`), so voice-driven actions
//! (video edit, subtitles, etc.) run through the exact same code path as typed chat.
//!
//! NOTE: the Live API's WebSocket URL, message shapes (`setup`/`realtimeInput`/
//! `serverContent`/`toolCall`) and model ids evolve; verify these against Google's current
//! docs if the session fails to connect or the server rejects the setup message.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64_STANDARD};
use futures_util::{SinkExt, StreamExt};
use serde::Serialize;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::sync::CancellationToken;

use crate::MintConfig;
use crate::orchestration::{AgentApproval, ApprovalOutcome, execute_tool_from_json};

const LIVE_WS_URL: &str = "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContent";
const DEFAULT_LIVE_MODEL: &str = "gemini-2.5-flash-native-audio-preview-12-2025";
const DEFAULT_LIVE_VOICE: &str = "Puck";

/// Events streamed back to the frontend for the lifetime of a Gemini Live session.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum GeminiLiveEvent {
    /// Base64 PCM16 audio chunk from the model's spoken reply.
    AudioChunk {
        data: String,
    },
    /// An incremental transcript fragment (a few words at a time, not a whole
    /// sentence) — Gemini streams these as it hears/speaks, so the frontend
    /// must accumulate fragments of the same role until `TurnComplete` rather
    /// than treat each one as the full line.
    Transcript {
        role: String,
        text: String,
    },
    /// The current turn (user's utterance or the model's reply) has finished —
    /// the next `Transcript` starts a new line instead of continuing this one.
    TurnComplete,
    /// The user barged in while the model was speaking; queued playback must stop immediately.
    Interrupted,
    /// Progress/result of a tool call the model triggered mid-conversation.
    ToolStatus {
        action: String,
        input: Value,
        result: Option<String>,
        error: Option<String>,
    },
    /// Tool approval request for Web / remote sessions where approvals travel over the Live stream.
    ApprovalRequested {
        token: String,
        approval: AgentApproval,
    },
    Error {
        message: String,
    },
    Closed,
}

#[derive(Debug, Default, Clone)]
pub(crate) struct LiveTurnAccumulator {
    pub(crate) user_segments: Vec<String>,
    pub(crate) assistant_segments: Vec<String>,
    pub(crate) tool_activity: Vec<crate::orchestration::AgentProgress>,
}

impl LiveTurnAccumulator {
    pub(crate) fn is_empty(&self) -> bool {
        self.user_segments.iter().all(|s| s.trim().is_empty())
            && self.assistant_segments.iter().all(|s| s.trim().is_empty())
            && self.tool_activity.is_empty()
    }

    pub(crate) fn should_flush_before_user_transcript(&self) -> bool {
        !self.assistant_segments.iter().all(|s| s.trim().is_empty())
            || !self.tool_activity.is_empty()
    }

    pub(crate) fn push_user_transcript(&mut self, text: &str) {
        if !text.is_empty() {
            self.user_segments.push(text.to_string());
        }
    }

    pub(crate) fn push_assistant_transcript(&mut self, text: &str) {
        if !text.is_empty() {
            self.assistant_segments.push(text.to_string());
        }
    }

    pub(crate) fn record_tool_call(
        &mut self,
        call_id: Option<String>,
        action: &str,
        input: Value,
        result: &str,
        elapsed_ms: Option<u64>,
        is_error: bool,
    ) {
        self.tool_activity
            .push(crate::orchestration::AgentProgress::ToolStart {
                call_id: call_id.clone(),
                action: action.to_string(),
                input: input.clone(),
                subagent: None,
            });
        self.tool_activity
            .push(crate::orchestration::AgentProgress::ToolEnd {
                elapsed_ms,
                call_id,
                status: Some(if is_error {
                    crate::integrations::mcp_result::ToolStatus::Failed
                } else {
                    crate::integrations::mcp_result::ToolStatus::Success
                }),
                action: action.to_string(),
                input,
                result: result.to_string(),
                subagent: None,
            });
    }

    pub(crate) fn flush_to_memory(
        &mut self,
        memory: &crate::MemoryStore,
        chat_id: &str,
        workspace_root: &Path,
        model: &str,
    ) -> Option<i64> {
        if self.is_empty() {
            self.user_segments.clear();
            self.assistant_segments.clear();
            return None;
        }
        let scoped_id =
            crate::memory::scoped_chat_id(chat_id, Some(workspace_root.to_string_lossy().as_ref()));
        let mut user_text = self.user_segments.concat().trim().to_string();
        if user_text.is_empty() {
            user_text = "[Voice]".to_string();
        }
        let mut ai_text = self.assistant_segments.concat().trim().to_string();
        if ai_text.is_empty() && !self.tool_activity.is_empty() {
            ai_text = "✓ Completed voice tool action.".to_string();
        }
        let activity = std::mem::take(&mut self.tool_activity);
        self.user_segments.clear();
        self.assistant_segments.clear();

        let interaction_id = memory
            .add_interaction_for_chat(&scoped_id, &user_text, &ai_text, "gemini", model)
            .ok()?;
        if !activity.is_empty()
            && let Ok(activity_json) = serde_json::to_string(&activity)
        {
            let _ = memory.set_interaction_agent_activity_json(interaction_id, &activity_json);
        }
        Some(interaction_id)
    }
}

/// Handle for pushing mic audio into a running session.
pub struct GeminiLiveHandle {
    audio_tx: mpsc::UnboundedSender<Vec<u8>>,
    cancellation: CancellationToken,
}

impl Drop for GeminiLiveHandle {
    fn drop(&mut self) {
        self.cancellation.cancel();
    }
}

impl GeminiLiveHandle {
    /// Push a chunk of raw PCM16 (16kHz, mono) audio captured from the mic.
    pub fn push_audio(&self, pcm: Vec<u8>) -> Result<(), String> {
        self.audio_tx
            .send(pcm)
            .map_err(|_| "Gemini Live session is no longer running".to_string())
    }
}

/// Starts a Gemini Live realtime voice session in the background and returns a handle
/// used to feed it mic audio. Session events are delivered through `on_event` until the
/// session ends (a final `Closed`, possibly preceded by `Error`, is always sent).
pub fn start_session<Approve, Event>(
    config: MintConfig,
    workspace_root: PathBuf,
    chat_id: String,
    mut approve_cb: Approve,
    on_event: Event,
) -> GeminiLiveHandle
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send + 'static,
    Event: Fn(GeminiLiveEvent) + Send + Sync + 'static,
{
    let (audio_tx, audio_rx) = mpsc::unbounded_channel::<Vec<u8>>();

    let on_event = Arc::new(on_event);
    let session_events = Arc::clone(&on_event);
    let turn_accumulator = Arc::new(std::sync::Mutex::new(LiveTurnAccumulator::default()));
    let session_turn = Arc::clone(&turn_accumulator);
    let close_workspace = workspace_root.clone();
    let close_chat_id = chat_id.clone();
    let close_model = resolve_live_model(&config);
    spawn_cancellable_live_session(
        chat_id.clone(),
        audio_tx,
        async move {
            let mut audio_rx = audio_rx;
            if let Err(message) = run_session(
                &config,
                &workspace_root,
                &chat_id,
                &mut approve_cb,
                &*session_events,
                &mut audio_rx,
                &session_turn,
            )
            .await
            {
                session_events(GeminiLiveEvent::Error { message });
            }
        },
        move || {
            if let Ok(mut turn) = turn_accumulator.lock()
                && !turn.is_empty()
                && let Ok(memory) = crate::MemoryStore::open_default()
            {
                let _ =
                    turn.flush_to_memory(&memory, &close_chat_id, &close_workspace, &close_model);
            }
            on_event(GeminiLiveEvent::Closed);
        },
    )
}

fn spawn_cancellable_live_session<F>(
    chat_id: String,
    audio_tx: mpsc::UnboundedSender<Vec<u8>>,
    session: F,
    on_closed: impl FnOnce() + Send + 'static,
) -> GeminiLiveHandle
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let cancellation = CancellationToken::new();
    let session_cancellation = cancellation.clone();
    spawn_live_session(async move {
        crate::orchestration::run_control::scope_with_token(
            &chat_id,
            session_cancellation.clone(),
            async move {
                tokio::select! {
                    biased;
                    _ = session_cancellation.cancelled() => {},
                    _ = session => {},
                }
            },
        )
        .await;
        on_closed();
    });
    GeminiLiveHandle {
        audio_tx,
        cancellation,
    }
}

/// Spawns a live voice task session.
pub(crate) fn spawn_live_session<F>(future: F) -> tokio::task::JoinHandle<F::Output>
where
    F: std::future::Future + Send + 'static,
    F::Output: Send + 'static,
{
    tokio::spawn(future)
}

fn resolve_api_key(config: &MintConfig) -> String {
    if config.api_key.trim().is_empty() {
        std::env::var("GEMINI_API_KEY").unwrap_or_default()
    } else {
        config.api_key.clone()
    }
}

fn resolve_live_model(config: &MintConfig) -> String {
    config
        .extra
        .get("geminiLiveModel")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_LIVE_MODEL)
        .to_string()
}

fn resolve_live_voice(config: &MintConfig) -> String {
    config
        .extra
        .get("geminiLiveVoice")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or(DEFAULT_LIVE_VOICE)
        .to_string()
}

fn agent_action_fallback_declaration() -> Value {
    json!({
        "name": "agent_action",
        "description": "Fallback action dispatcher: execute one of Mint's built-in actions by specifying its action name and input object.",
        "parameters": {
            "type": "OBJECT",
            "properties": {
                "action": {
                    "type": "STRING",
                    "description": "The action name, e.g. video_trim, video_extract_audio, speech_transcribe, list_files, read_file, run_shell."
                },
                "input": {
                    "type": "OBJECT",
                    "description": "Arguments for the action, e.g. {\"input\": \"...\", \"output\": \"...\", \"start\": 0, \"end\": 10}."
                }
            },
            "required": ["action"]
        }
    })
}

fn live_tool_declarations(config: &MintConfig, workspace_root: &Path) -> Value {
    let catalog = crate::prompts::tool_catalog::tool_catalog(config, false, workspace_root, false);
    let mut declarations: Vec<Value> = catalog
        .into_iter()
        .filter(|tool| {
            !matches!(
                tool.name.as_str(),
                "finish" | "enter_plan_mode" | "exit_plan_mode"
            )
        })
        .map(|tool| {
            json!({
                "name": tool.name,
                "description": tool.description,
                "parameters": crate::chat::gemini_sanitize_schema(&tool.parameters),
            })
        })
        .collect();
    declarations.push(agent_action_fallback_declaration());
    json!({ "functionDeclarations": declarations })
}

fn resolve_live_tool_call(call: &Value) -> (String, Value) {
    let name = call["name"].as_str().unwrap_or_default();
    if name == "agent_action" {
        let action = call["args"]["action"]
            .as_str()
            .unwrap_or_default()
            .to_string();
        let input = call["args"]
            .get("input")
            .cloned()
            .unwrap_or_else(|| json!({}));
        (action, input)
    } else {
        let input = call.get("args").cloned().unwrap_or_else(|| json!({}));
        (name.to_string(), input)
    }
}

fn dispatch_server_content_events<Event>(value: &Value, on_event: &Event)
where
    Event: Fn(GeminiLiveEvent),
{
    if let Some(parts) = value["serverContent"]["modelTurn"]["parts"].as_array() {
        for part in parts {
            if let Some(data) = part["inlineData"]["data"].as_str() {
                on_event(GeminiLiveEvent::AudioChunk {
                    data: data.to_string(),
                });
            }
        }
    }
    if let Some(text) = value["serverContent"]["outputTranscription"]["text"].as_str()
        && !text.is_empty()
    {
        on_event(GeminiLiveEvent::Transcript {
            role: "assistant".into(),
            text: text.into(),
        });
    }
    if let Some(text) = value["serverContent"]["inputTranscription"]["text"].as_str()
        && !text.is_empty()
    {
        on_event(GeminiLiveEvent::Transcript {
            role: "user".into(),
            text: text.into(),
        });
    }
    if value["serverContent"]["interrupted"].as_bool() == Some(true) {
        on_event(GeminiLiveEvent::Interrupted);
    }
    if value["serverContent"]["turnComplete"].as_bool() == Some(true) {
        on_event(GeminiLiveEvent::TurnComplete);
    }
}

const MAX_LIVE_BASE_PROMPT_CHARS: usize = 12_000;
const MAX_LIVE_MEMORY_CONTEXT_CHARS: usize = 6_000;
const MAX_LIVE_SKILLS_CONTEXT_CHARS: usize = 4_000;
const LIVE_VOICE_MODE_OVERRIDE: &str = "LIVE VOICE MODE OVERRIDE:\n- You are in a real-time spoken voice conversation (Gemini Live). Speak naturally and concisely to the user.\n- When the user asks you to perform an action, call the matching tool function directly (e.g. `list_files`, `read_file`, `git_status`, `web_search`, `video_trim`, `run_shell`, etc.) rather than describing the action in words.\n- Do NOT call a `finish` tool — once your tool calls complete (or if no tool is needed), simply speak your final response out loud to the user.";

pub(crate) fn build_live_system_instruction(
    config: &MintConfig,
    workspace_root: &Path,
    chat_id: &str,
    memory_store: Option<&crate::MemoryStore>,
) -> String {
    let user_name = memory_store.and_then(|memory| memory.get_profile("name").ok().flatten());
    let mut base_prompt =
        crate::prompts::agent::build_system_prompt(config, false, true, user_name.as_deref(), None);
    if base_prompt.chars().count() > MAX_LIVE_BASE_PROMPT_CHARS {
        base_prompt = base_prompt
            .chars()
            .take(MAX_LIVE_BASE_PROMPT_CHARS)
            .collect();
    }

    let mut sections = vec![base_prompt.trim().to_string()];

    if let Some(memory) = memory_store {
        let mut memory_buf = String::new();
        crate::orchestration::append_memory_context(
            &mut memory_buf,
            memory,
            chat_id,
            Some(workspace_root.to_string_lossy().as_ref()),
            None,
            config.semantic_fact_recall,
            true,
        );
        let trimmed_mem = memory_buf.trim();
        if !trimmed_mem.is_empty() {
            let mem_chars: Vec<char> = trimmed_mem.chars().collect();
            let budgeted_mem = if mem_chars.len() > MAX_LIVE_MEMORY_CONTEXT_CHARS {
                // Keep profile/facts at the head and the newest conversation turns at the tail.
                let head_len = 1_500;
                let tail_len = MAX_LIVE_MEMORY_CONTEXT_CHARS.saturating_sub(head_len + 40);
                let head: String = mem_chars[..head_len].iter().collect();
                let tail: String = mem_chars[mem_chars.len() - tail_len..].iter().collect();
                format!("{head}\n...[older turns omitted]...\n{tail}")
            } else {
                trimmed_mem.to_string()
            };
            sections.push(budgeted_mem);
        }
    }

    if let Ok(skills) = crate::skills::learned_skills_context(Some(workspace_root), Some(chat_id)) {
        let trimmed_skills = skills.trim();
        if !trimmed_skills.is_empty() {
            let skill_chars: Vec<char> = trimmed_skills.chars().collect();
            let budgeted_skills = if skill_chars.len() > MAX_LIVE_SKILLS_CONTEXT_CHARS {
                let prefix: String = skill_chars[..MAX_LIVE_SKILLS_CONTEXT_CHARS]
                    .iter()
                    .collect();
                format!("Learned skills:\n{prefix}\n...[truncated]")
            } else {
                format!("Learned skills:\n{trimmed_skills}")
            };
            sections.push(budgeted_skills);
        }
    }

    sections.push(LIVE_VOICE_MODE_OVERRIDE.to_string());
    sections.join("\n\n")
}

async fn run_session<Approve, Event>(
    config: &MintConfig,
    workspace_root: &Path,
    chat_id: &str,
    approve_cb: &mut Approve,
    on_event: &Event,
    audio_rx: &mut mpsc::UnboundedReceiver<Vec<u8>>,
    turn_accumulator: &Arc<std::sync::Mutex<LiveTurnAccumulator>>,
) -> Result<(), String>
where
    Approve: FnMut(&AgentApproval) -> Result<ApprovalOutcome, String> + Send,
    Event: Fn(GeminiLiveEvent) + Send + Sync,
{
    let api_key = resolve_api_key(config);
    if api_key.trim().is_empty() {
        return Err("Gemini API key is not configured".into());
    }
    let model = resolve_live_model(config);
    let voice = resolve_live_voice(config);

    eprintln!("[gemini-live] connecting (model={model}, voice={voice})...");
    let url = format!("{LIVE_WS_URL}?key={api_key}");
    let ws_stream = match tokio::time::timeout(
        std::time::Duration::from_secs(15),
        tokio_tungstenite::connect_async(url),
    )
    .await
    {
        Ok(Ok((stream, _))) => stream,
        Ok(Err(e)) => {
            eprintln!("[gemini-live] connect failed: {e}");
            return Err(format!("unable to connect to Gemini Live: {e}"));
        }
        Err(_) => {
            eprintln!("[gemini-live] connect timed out after 15s");
            return Err("timed out connecting to Gemini Live".into());
        }
    };
    eprintln!("[gemini-live] connected, sending setup...");
    let (mut ws_write, mut ws_read) = ws_stream.split();

    let memory_store = crate::MemoryStore::open_default().ok();
    let system_instruction =
        build_live_system_instruction(config, workspace_root, chat_id, memory_store.as_ref());

    // Confirmed against a live server rejection: `responseModalities` must be nested under
    // `generationConfig` — the server explicitly rejects it at the top level of `setup`
    // ("Unknown name \"responseModalities\" at 'setup': Cannot find field.").
    let setup = json!({
        "setup": {
            "model": format!("models/{model}"),
            "generationConfig": {
                "responseModalities": ["AUDIO"],
                "speechConfig": {
                    "voiceConfig": {
                        "prebuiltVoiceConfig": { "voiceName": voice }
                    }
                }
            },
            "systemInstruction": { "parts": [{ "text": system_instruction }] },
            "tools": [live_tool_declarations(config, workspace_root)],
            "outputAudioTranscription": {},
            "inputAudioTranscription": {},
            // Default end-of-speech detection is quick to decide the user is done
            // talking, which was cutting long utterances into 2-3 separate turns on
            // a normal mid-sentence breath pause. LOW end-of-speech sensitivity plus
            // a longer silence requirement gives the user more room to pause without
            // being cut off. Per Google's Live API docs as of writing; if the server
            // rejects this field, check docs for the current schema/enum names.
            "realtimeInputConfig": {
                "automaticActivityDetection": {
                    "startOfSpeechSensitivity": "START_SENSITIVITY_HIGH",
                    "endOfSpeechSensitivity": "END_SENSITIVITY_LOW",
                    "prefixPaddingMs": 200,
                    "silenceDurationMs": 800
                }
            }
        }
    });
    ws_write
        .send(Message::Text(setup.to_string().into()))
        .await
        .map_err(|e| format!("failed to send Gemini Live setup: {e}"))?;
    let session_started_at = std::time::Instant::now();
    eprintln!("[gemini-live] setup sent, waiting for events...");

    let mut heartbeat = tokio::time::interval(std::time::Duration::from_secs(3));
    let mut audio_chunks_sent: u64 = 0;
    let mut server_messages_received: u64 = 0;

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                eprintln!(
                    "[gemini-live] still waiting... {}s elapsed, {} audio chunks sent, {} server messages received so far",
                    session_started_at.elapsed().as_secs(), audio_chunks_sent, server_messages_received
                );
            }
            chunk = audio_rx.recv() => {
                match chunk {
                    Some(pcm) => {
                        audio_chunks_sent += 1;
                        let payload = json!({
                            "realtimeInput": {
                                "audio": {
                                    "data": BASE64_STANDARD.encode(&pcm),
                                    "mimeType": "audio/pcm;rate=16000"
                                }
                            }
                        });
                        if ws_write.send(Message::Text(payload.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                    None => {
                        eprintln!("[gemini-live] mic audio channel closed, ending session");
                        let _ = ws_write.send(Message::Close(None)).await;
                        break;
                    }
                }
            }
            msg = ws_read.next() => {
                let Some(msg) = msg else {
                    eprintln!("[gemini-live] connection closed by server (stream ended)");
                    break;
                };
                server_messages_received += 1;
                let msg = match msg {
                    Ok(msg) => msg,
                    Err(e) => {
                        eprintln!("[gemini-live] websocket read error: {e}");
                        return Err(format!("Gemini Live websocket error: {e}"));
                    }
                };
                // The server sends JSON payloads as `Binary` frames, not `Text` frames,
                // despite the content being UTF-8 text — both must be accepted or every
                // server event (audio, transcripts, tool calls) is silently dropped.
                let raw = match msg {
                    Message::Text(text) => text.to_string(),
                    Message::Binary(bytes) => match String::from_utf8(bytes.to_vec()) {
                        Ok(text) => text,
                        Err(e) => {
                            eprintln!("[gemini-live] received non-UTF8 binary frame, ignoring: {e}");
                            continue;
                        }
                    },
                    Message::Close(frame) => {
                        let reason = frame
                            .map(|f| format!("{} ({})", f.code, f.reason))
                            .unwrap_or_else(|| "no reason given".to_string());
                        eprintln!("[gemini-live] server sent close frame: {reason}");
                        return Err(format!("Gemini Live closed the connection: {reason}"));
                    }
                    _ => continue,
                };
                let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                    eprintln!("[gemini-live] received non-JSON text frame, ignoring: {}", &raw[..raw.len().min(200)]);
                    continue;
                };

                if value.get("setupComplete").is_some() {
                    eprintln!("[gemini-live] setup complete, session is live");
                }

                if let Some(err_obj) = value.get("error") {
                    let message = err_obj
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("Gemini Live reported an error")
                        .to_string();
                    eprintln!("[gemini-live] server error: {message}");
                    return Err(message);
                }

                dispatch_server_content_events(&value, &|ev| {
                    match &ev {
                        GeminiLiveEvent::Transcript { role, text } => {
                            if let Ok(mut turn) = turn_accumulator.lock() {
                                if role == "user" {
                                    if turn.should_flush_before_user_transcript()
                                        && let Some(ref memory) = memory_store
                                    {
                                        let _ = turn.flush_to_memory(
                                            memory,
                                            chat_id,
                                            workspace_root,
                                            &model,
                                        );
                                    }
                                    turn.push_user_transcript(text);
                                } else if role == "assistant" {
                                    turn.push_assistant_transcript(text);
                                }
                            }
                            on_event(ev);
                        }
                        GeminiLiveEvent::Interrupted | GeminiLiveEvent::TurnComplete => {
                            if let Some(ref memory) = memory_store
                                && let Ok(mut turn) = turn_accumulator.lock()
                            {
                                let _ = turn.flush_to_memory(memory, chat_id, workspace_root, &model);
                            }
                            on_event(ev);
                        }
                        _ => on_event(ev),
                    }
                });

                if let Some(calls) = value["toolCall"]["functionCalls"].as_array() {
                    for call in calls {
                        let id = call["id"].as_str().unwrap_or_default().to_string();
                        let name = call["name"].as_str().unwrap_or_default().to_string();
                        let (action, input) = resolve_live_tool_call(call);
                        let call_started = std::time::Instant::now();

                        let outcome = execute_tool_from_json(
                            workspace_root,
                            config,
                            &action,
                            input.clone(),
                            chat_id,
                            approve_cb,
                        )
                        .await;
                        let elapsed_ms = Some(call_started.elapsed().as_millis() as u64);

                        let (result_str, error_str, response_payload) = match &outcome {
                            Ok(result) => (Some(result.clone()), None, json!({ "result": result })),
                            Err(err) => {
                                let message = err.to_string();
                                (None, Some(message.clone()), json!({ "error": message }))
                            }
                        };
                        if let Ok(mut turn) = turn_accumulator.lock() {
                            let summary = result_str
                                .as_deref()
                                .or(error_str.as_deref())
                                .unwrap_or_default();
                            turn.record_tool_call(
                                if id.is_empty() { None } else { Some(id.clone()) },
                                &action,
                                input.clone(),
                                summary,
                                elapsed_ms,
                                error_str.is_some(),
                            );
                        }
                        on_event(GeminiLiveEvent::ToolStatus {
                            action: action.clone(),
                            input,
                            result: result_str,
                            error: error_str,
                        });

                        let tool_response = json!({
                            "toolResponse": {
                                "functionResponses": [{
                                    "id": id,
                                    "name": name,
                                    "response": response_payload
                                }]
                            }
                        });
                        if ws_write.send(Message::Text(tool_response.to_string().into())).await.is_err() {
                            break;
                        }
                    }
                }

                if value.get("goAway").is_some() {
                    // Server is about to close the connection (e.g. session time limit) — stop cleanly.
                    eprintln!("[gemini-live] server sent goAway, closing session");
                    break;
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod approval_lifecycle_tests {
    use super::*;
    use crate::ApprovalRegistry;
    use std::time::Duration;
    use tokio::sync::oneshot;

    #[tokio::test]
    async fn stopping_voice_releases_pending_approval_and_emits_closed() {
        let registry = Arc::new(ApprovalRegistry::default());
        let pending = registry.clone();
        let (token_tx, token_rx) = oneshot::channel();
        let (closed_tx, mut closed_rx) = oneshot::channel();
        let (audio_tx, _audio_rx) = mpsc::unbounded_channel();
        let handle = spawn_cancellable_live_session(
            uuid::Uuid::new_v4().to_string(),
            audio_tx,
            async move {
                pending
                    .request(|token| {
                        token_tx.send(token.to_owned()).unwrap();
                        Ok(())
                    })
                    .await
                    .unwrap();
            },
            move || {
                let _ = closed_tx.send(());
            },
        );
        let token = token_rx.await.unwrap();
        drop(handle);
        let result = tokio::time::timeout(Duration::from_secs(5), &mut closed_rx).await;
        if result.is_err() {
            registry.resolve(&token, false, None);
            let _ = closed_rx.await;
        }
        assert!(
            matches!(result, Ok(Ok(()))),
            "voice stop did not release approval wait: {result:?}"
        );
        assert!(!registry.resolve(&token, true, None));
    }

    #[tokio::test]
    async fn stopping_voice_closes_a_session_waiting_on_transport() {
        let (ready_tx, ready_rx) = oneshot::channel();
        let (io_tx, io_rx) = oneshot::channel();
        let (closed_tx, mut closed_rx) = oneshot::channel();
        let (audio_tx, _audio_rx) = mpsc::unbounded_channel();
        let handle = spawn_cancellable_live_session(
            uuid::Uuid::new_v4().to_string(),
            audio_tx,
            async move {
                ready_tx.send(()).unwrap();
                let _ = io_rx.await;
            },
            move || {
                let _ = closed_tx.send(());
            },
        );
        ready_rx.await.unwrap();
        drop(handle);
        let result = tokio::time::timeout(Duration::from_secs(5), &mut closed_rx).await;
        if result.is_err() {
            let _ = io_tx.send(());
            let _ = closed_rx.await;
        }
        assert!(
            matches!(result, Ok(Ok(()))),
            "voice stop waited for transport: {result:?}"
        );
    }

    #[test]
    fn live_tool_declarations_include_real_catalog_tools_without_unsupported_schema_fields() {
        let config = MintConfig::default();
        let workspace = std::path::Path::new(".");
        let decl = live_tool_declarations(&config, workspace);
        let funcs = decl["functionDeclarations"]
            .as_array()
            .expect("functionDeclarations array");
        let names: Vec<&str> = funcs.iter().filter_map(|f| f["name"].as_str()).collect();

        assert!(
            names.contains(&"list_files"),
            "missing list_files: {names:?}"
        );
        assert!(names.contains(&"read_file"), "missing read_file: {names:?}");
        assert!(
            names.contains(&"git_status"),
            "missing git_status: {names:?}"
        );
        assert!(
            names.contains(&"web_search"),
            "missing web_search: {names:?}"
        );
        assert!(
            names.contains(&"video_trim"),
            "missing video_trim: {names:?}"
        );
        assert!(
            names.contains(&"agent_action"),
            "missing fallback agent_action: {names:?}"
        );
        assert!(
            !names.contains(&"finish"),
            "live voice mode must not expose finish tool"
        );
        assert!(
            !names.contains(&"enter_plan_mode"),
            "live voice mode must not expose enter_plan_mode"
        );
        assert!(
            !names.contains(&"exit_plan_mode"),
            "live voice mode must not expose exit_plan_mode"
        );

        let serialized = serde_json::to_string(&decl).unwrap();
        assert!(
            !serialized.contains("\"additionalProperties\""),
            "Gemini Live tool schemas must strip additionalProperties"
        );
    }

    #[test]
    fn resolve_live_tool_call_handles_both_direct_tools_and_agent_action_wrapper() {
        let direct_call = json!({
            "id": "call-1",
            "name": "read_file",
            "args": { "path": "README.md", "startLine": 1 }
        });
        let (action, input) = resolve_live_tool_call(&direct_call);
        assert_eq!(action, "read_file");
        assert_eq!(input, json!({ "path": "README.md", "startLine": 1 }));

        let wrapped_call = json!({
            "id": "call-2",
            "name": "agent_action",
            "args": {
                "action": "video_trim",
                "input": { "input": "a.mp4", "output": "b.mp4", "start": 0, "end": 5 }
            }
        });
        let (action2, input2) = resolve_live_tool_call(&wrapped_call);
        assert_eq!(action2, "video_trim");
        assert_eq!(
            input2,
            json!({ "input": "a.mp4", "output": "b.mp4", "start": 0, "end": 5 })
        );
    }

    #[test]
    fn server_content_interrupted_emits_interrupted_event_distinct_from_turn_complete() {
        let events = std::sync::Mutex::new(Vec::new());
        let interrupted_msg = json!({
            "serverContent": {
                "interrupted": true
            }
        });
        dispatch_server_content_events(&interrupted_msg, &|ev| events.lock().unwrap().push(ev));
        {
            let recorded = events.lock().unwrap();
            assert_eq!(recorded.len(), 1);
            assert!(
                matches!(recorded[0], GeminiLiveEvent::Interrupted),
                "expected Interrupted event, got: {:?}",
                recorded[0]
            );
        }

        events.lock().unwrap().clear();
        let turn_complete_msg = json!({
            "serverContent": {
                "turnComplete": true
            }
        });
        dispatch_server_content_events(&turn_complete_msg, &|ev| events.lock().unwrap().push(ev));
        {
            let recorded = events.lock().unwrap();
            assert_eq!(recorded.len(), 1);
            assert!(
                matches!(recorded[0], GeminiLiveEvent::TurnComplete),
                "expected TurnComplete event, got: {:?}",
                recorded[0]
            );
        }
    }

    #[test]
    fn live_turn_accumulator_flushes_transcripts_and_tool_activity_to_memory_store() {
        let db_path = std::env::temp_dir().join(format!(
            "mint-live-turn-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let memory = crate::MemoryStore::open(&db_path);
        let workspace = std::path::Path::new("/tmp/mint-live-test-ws");
        let chat_id = "chat-live-1";
        let model = "gemini-2.5-flash-native-audio-preview-12-2025";

        let mut turn = LiveTurnAccumulator::default();
        assert!(turn.is_empty());
        assert_eq!(
            turn.flush_to_memory(&memory, chat_id, workspace, model),
            None
        );

        turn.push_user_transcript("ช่วยดูไฟล์ ");
        turn.push_user_transcript("README ให้หน่อย");
        turn.record_tool_call(
            Some("call-1".to_string()),
            "read_file",
            json!({ "path": "README.md" }),
            "file contents here",
            Some(12),
            false,
        );
        turn.push_assistant_transcript("อ่านไฟล์ README ");
        turn.push_assistant_transcript("เรียบร้อยแล้วครับ");

        let interaction_id = turn
            .flush_to_memory(&memory, chat_id, workspace, model)
            .expect("should persist non-empty turn");
        assert!(interaction_id > 0);
        assert!(turn.is_empty(), "accumulator must reset after flush");

        let scoped_id =
            crate::memory::scoped_chat_id(chat_id, Some(workspace.to_string_lossy().as_ref()));
        let snapshot = memory
            .conversation_snapshot(&scoped_id, None, 10)
            .expect("snapshot");
        assert_eq!(snapshot.interactions.len(), 1);
        let saved = &snapshot.interactions[0];
        assert_eq!(saved.user_text, "ช่วยดูไฟล์ README ให้หน่อย");
        assert_eq!(saved.ai_text, "อ่านไฟล์ README เรียบร้อยแล้วครับ");
        assert_eq!(saved.provider, "gemini");
        assert_eq!(saved.model, model);
        let activity = saved
            .agent_activity
            .as_ref()
            .and_then(|v| v.as_array())
            .expect("agent_activity should be saved as array");
        assert_eq!(activity.len(), 2, "expected ToolStart and ToolEnd");
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn live_turn_accumulator_flushes_previous_turn_when_new_user_speech_arrives_after_assistant_reply()
     {
        let db_path = std::env::temp_dir().join(format!(
            "mint-live-turn-barge-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let memory = crate::MemoryStore::open(&db_path);
        let workspace = std::path::Path::new("/tmp/mint-live-barge-ws");
        let chat_id = "chat-live-barge";
        let model = "gemini-2.5-flash-native-audio-preview-12-2025";

        let mut turn = LiveTurnAccumulator::default();
        turn.push_user_transcript("สวัสดีครับ");
        turn.push_assistant_transcript("สวัสดีครับ มีอะไรให้ช่วยไหมครับ");

        // User starts speaking Turn 2 before `interrupted`/`turnComplete` arrives:
        assert!(
            turn.should_flush_before_user_transcript(),
            "once assistant has replied/acted, a new user utterance after assistant output should flush Turn 1 first"
        );
        let first_id = turn
            .flush_to_memory(&memory, chat_id, workspace, model)
            .expect("should flush Turn 1");
        assert!(first_id > 0);

        turn.push_user_transcript("เดี๋ยวก่อนนะ ขอเปลี่ยนคำถาม");
        assert!(
            !turn.should_flush_before_user_transcript(),
            "consecutive user fragments in Turn 2 must accumulate together"
        );
        turn.push_assistant_transcript("ได้ครับ ว่ามาได้เลย");
        let second_id = turn
            .flush_to_memory(&memory, chat_id, workspace, model)
            .expect("should flush Turn 2");
        assert!(second_id > first_id);

        let scoped_id =
            crate::memory::scoped_chat_id(chat_id, Some(workspace.to_string_lossy().as_ref()));
        let snapshot = memory
            .conversation_snapshot(&scoped_id, None, 10)
            .expect("snapshot");
        assert_eq!(snapshot.interactions.len(), 2);
        assert_eq!(snapshot.interactions[0].user_text, "สวัสดีครับ");
        assert_eq!(snapshot.interactions[0].ai_text, "สวัสดีครับ มีอะไรให้ช่วยไหมครับ");
        assert_eq!(snapshot.interactions[1].user_text, "เดี๋ยวก่อนนะ ขอเปลี่ยนคำถาม");
        assert_eq!(snapshot.interactions[1].ai_text, "ได้ครับ ว่ามาได้เลย");
        let _ = std::fs::remove_file(&db_path);
    }

    #[test]
    fn approval_requested_event_serializes_with_camel_case_type() {
        let ev = GeminiLiveEvent::ApprovalRequested {
            token: "tok-123".to_string(),
            approval: AgentApproval::RunShell {
                command: "cargo test".to_string(),
                mode: "workspace-write".to_string(),
                background: false,
            },
        };
        let v = serde_json::to_value(&ev).expect("serialize ApprovalRequested");
        assert_eq!(v["type"], "approvalRequested");
        assert_eq!(v["token"], "tok-123");
        assert_eq!(v["approval"]["RunShell"]["command"], "cargo test");
    }

    #[test]
    fn build_live_system_instruction_preserves_memory_skills_and_voice_override_under_budget() {
        let db_path = std::env::temp_dir().join(format!(
            "mint-live-sysprompt-test-{}.sqlite3",
            uuid::Uuid::new_v4()
        ));
        let memory = crate::MemoryStore::open(&db_path);
        let workspace = std::path::Path::new("/tmp/mint-live-budget-ws");
        let chat_id = "chat-budget-1";
        let scoped =
            crate::memory::scoped_chat_id(chat_id, Some(workspace.to_string_lossy().as_ref()));

        memory.set_profile("name", "Pheem").unwrap();
        for idx in 0..8 {
            let filler = format!("Turn {idx}: {}", "รายละเอียดระบบเดิม ".repeat(60));
            memory
                .add_interaction_for_chat(&scoped, &filler, &filler, "gemini", "gemini-2.5-flash")
                .unwrap();
        }
        memory
            .add_interaction_for_chat(
                &scoped,
                "โปรเจกต์นี้ใช้พอร์ต 4173 นะ",
                "รับทราบครับ พอร์ต 4173",
                "gemini",
                "gemini-2.5-flash",
            )
            .unwrap();

        let config = MintConfig::default();
        let instruction = build_live_system_instruction(&config, workspace, chat_id, Some(&memory));

        assert!(
            instruction.contains("4173"),
            "memory context must not be truncated out by a long base prompt"
        );
        assert!(
            instruction.contains("LIVE VOICE MODE OVERRIDE:"),
            "voice mode override must always be present"
        );
        assert!(
            instruction
                .trim_end()
                .ends_with("simply speak your final response out loud to the user."),
            "voice mode override must be pinned at the very end of system_instruction"
        );
        assert!(
            instruction.chars().count() <= 24_000,
            "instruction should stay within total 24k char budget, got {}",
            instruction.chars().count()
        );

        let _ = std::fs::remove_file(&db_path);
    }
}

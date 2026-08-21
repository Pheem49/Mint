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
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::Message;

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
    /// Progress/result of a tool call the model triggered mid-conversation.
    ToolStatus {
        action: String,
        input: Value,
        result: Option<String>,
        error: Option<String>,
    },
    Error {
        message: String,
    },
    Closed,
}

/// Handle for pushing mic audio into a running session.
pub struct GeminiLiveHandle {
    audio_tx: mpsc::UnboundedSender<Vec<u8>>,
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

    tokio::spawn(async move {
        let mut audio_rx = audio_rx;
        if let Err(message) = run_session(
            &config,
            &workspace_root,
            &chat_id,
            &mut approve_cb,
            &on_event,
            &mut audio_rx,
        )
        .await
        {
            on_event(GeminiLiveEvent::Error { message });
        }
        on_event(GeminiLiveEvent::Closed);
    });

    GeminiLiveHandle { audio_tx }
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

fn agent_action_tool_declaration() -> Value {
    json!({
        "functionDeclarations": [{
            "name": "agent_action",
            "description": "Execute one of Mint's built-in actions (video editing, subtitle generation, file/browser/shell operations, and more). Use this instead of describing the action in words.",
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
        }]
    })
}

async fn run_session<Approve, Event>(
    config: &MintConfig,
    workspace_root: &Path,
    chat_id: &str,
    approve_cb: &mut Approve,
    on_event: &Event,
    audio_rx: &mut mpsc::UnboundedReceiver<Vec<u8>>,
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

    let user_name = crate::MemoryStore::open_default()
        .ok()
        .and_then(|memory| memory.get_profile("name").ok().flatten());

    let mut system_instruction = format!(
        "{}\n\nWhen the user asks you to perform an action, call the `agent_action` function with `action` set to the action name and `input` set to a JSON object of its arguments, rather than describing the action in words.",
        crate::prompts::agent::build_system_prompt(config, false, true, user_name.as_deref(), None)
    );
    // Same cross-session context (saved profile/preferences + this chat's recent
    // history) the typed-chat agent loop gets, so a Live call isn't a blank slate
    // that's forgotten everything the user already told Mint.
    crate::orchestration::append_memory_context(&mut system_instruction, chat_id);
    if let Ok(skills) = crate::skills::learned_skills_context(Some(workspace_root), Some(chat_id))
        && !skills.trim().is_empty()
    {
        system_instruction = format!(
            "{}\n\nLearned skills:\n{}",
            system_instruction.trim(),
            skills.trim()
        );
    }
    // Recent conversation history and skills have no size cap of their own (skills
    // alone can run up to 16KB — see MAX_CONTEXT_BYTES in skills.rs) — a long chat
    // history would otherwise bloat every Live session's setup message, adding to
    // the delay before the model starts responding. The core instructions and tool
    // declaration are built first, so truncating off the end only drops overflow
    // context, never the instructions themselves.
    const MAX_LIVE_SYSTEM_INSTRUCTION_CHARS: usize = 12_000;
    if system_instruction.chars().count() > MAX_LIVE_SYSTEM_INSTRUCTION_CHARS {
        system_instruction = system_instruction
            .chars()
            .take(MAX_LIVE_SYSTEM_INSTRUCTION_CHARS)
            .collect();
    }

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
            "tools": [agent_action_tool_declaration()],
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

                if let Some(parts) = value["serverContent"]["modelTurn"]["parts"].as_array() {
                    for part in parts {
                        if let Some(data) = part["inlineData"]["data"].as_str() {
                            on_event(GeminiLiveEvent::AudioChunk { data: data.to_string() });
                        }
                    }
                }
                if let Some(text) = value["serverContent"]["outputTranscription"]["text"].as_str() {
                    if !text.is_empty() {
                        on_event(GeminiLiveEvent::Transcript { role: "assistant".into(), text: text.into() });
                    }
                }
                if let Some(text) = value["serverContent"]["inputTranscription"]["text"].as_str() {
                    if !text.is_empty() {
                        on_event(GeminiLiveEvent::Transcript { role: "user".into(), text: text.into() });
                    }
                }
                if value["serverContent"]["turnComplete"].as_bool() == Some(true)
                    || value["serverContent"]["interrupted"].as_bool() == Some(true)
                {
                    on_event(GeminiLiveEvent::TurnComplete);
                }

                if let Some(calls) = value["toolCall"]["functionCalls"].as_array() {
                    for call in calls {
                        let id = call["id"].as_str().unwrap_or_default().to_string();
                        let name = call["name"].as_str().unwrap_or_default().to_string();
                        let action = call["args"]["action"].as_str().unwrap_or_default().to_string();
                        let input = call["args"]["input"].clone();

                        let outcome = execute_tool_from_json(
                            workspace_root,
                            config,
                            &action,
                            input.clone(),
                            chat_id,
                            approve_cb,
                        )
                        .await;

                        let (result_str, error_str, response_payload) = match &outcome {
                            Ok(result) => (Some(result.clone()), None, json!({ "result": result })),
                            Err(err) => {
                                let message = err.to_string();
                                (None, Some(message.clone()), json!({ "error": message }))
                            }
                        };
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

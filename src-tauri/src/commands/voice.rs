use crate::commands::approvals;
use crate::commands::workspace::workspace_root;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use mint_core::{
    GeminiLiveEvent, GeminiLiveHandle, MicRecordingHandle, load_config,
    start_gemini_live_session as core_start_gemini_live_session,
    start_recording as core_start_mic_recording, stop_recording as core_stop_mic_recording,
    transcribe_recording as core_transcribe_mic_recording,
};
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};
use tauri::AppHandle;
use tauri::ipc::Channel;

#[derive(Default)]
pub struct GeminiLiveState {
    pub sessions: Mutex<HashMap<String, GeminiLiveHandle>>,
}

#[derive(Default)]
pub struct MicRecordingState {
    pub active: Mutex<Option<MicRecordingHandle>>,
}

#[derive(serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub(crate) struct GeminiLiveStartRequest {
    #[serde(default)]
    workspace_path: Option<String>,
    #[serde(default)]
    chat_id: Option<String>,
}

static COUNTER: AtomicU64 = AtomicU64::new(1);

/// Starts a Gemini Live realtime voice session (beta, opt-in voice mode) and returns a
/// session id used by `send_gemini_live_audio_chunk`/`stop_gemini_live_session`. Session
/// events (audio replies, transcripts, tool-call status) stream back through `on_event`.
/// Tool calls triggered by voice go through the same approval flow as typed chat, reusing
/// the existing `tool-approval-requested` event / `ApprovalsState` bridge.
#[tauri::command]
pub(crate) async fn start_gemini_live_session(
    app: AppHandle,
    state: tauri::State<'_, GeminiLiveState>,
    request: GeminiLiveStartRequest,
    on_event: Channel<GeminiLiveEvent>,
) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    let root = workspace_root(request.workspace_path.as_deref())?;
    let chat_id = request.chat_id.unwrap_or_default();
    let session_id = format!("gemini-live-{}", COUNTER.fetch_add(1, Ordering::SeqCst));

    let approve_cb = approvals::callback(app.clone(), chat_id.clone());

    let handle = core_start_gemini_live_session(config, root, chat_id, approve_cb, move |event| {
        let _ = on_event.send(event);
    });

    state
        .sessions
        .lock()
        .map_err(|e| e.to_string())?
        .insert(session_id.clone(), handle);
    Ok(session_id)
}

#[tauri::command]
pub(crate) async fn send_gemini_live_audio_chunk(
    state: tauri::State<'_, GeminiLiveState>,
    session_id: String,
    chunk_base64: String,
) -> Result<(), String> {
    let pcm = BASE64
        .decode(chunk_base64)
        .map_err(|e| format!("invalid audio chunk: {e}"))?;
    let sessions = state.sessions.lock().map_err(|e| e.to_string())?;
    let handle = sessions
        .get(&session_id)
        .ok_or_else(|| "Gemini Live session not found".to_string())?;
    handle.push_audio(pcm)
}

#[tauri::command]
pub(crate) async fn stop_gemini_live_session(
    state: tauri::State<'_, GeminiLiveState>,
    session_id: String,
) -> Result<(), String> {
    state
        .sessions
        .lock()
        .map_err(|e| e.to_string())?
        .remove(&session_id);
    Ok(())
}

/// Starts native push-to-talk mic recording (Rust-side, via `cpal`) for the desktop
/// build's voice input button. Call `stop_mic_recording_and_transcribe` to stop and
/// get the transcript back.
#[tauri::command]
pub(crate) fn start_mic_recording(
    state: tauri::State<'_, MicRecordingState>,
) -> Result<(), String> {
    let mut active = state.active.lock().map_err(|e| e.to_string())?;
    if active.is_some() {
        return Err("A recording is already in progress".into());
    }
    let handle = core_start_mic_recording().map_err(|e| e.to_string())?;
    *active = Some(handle);
    Ok(())
}

/// Stops the in-progress recording and transcribes it using whichever provider is
/// configured for chat (`MintConfig.ai_provider`). Rejects with a clear message if
/// that provider doesn't support audio input.
#[tauri::command]
pub(crate) async fn stop_mic_recording_and_transcribe(
    state: tauri::State<'_, MicRecordingState>,
) -> Result<String, String> {
    let handle = state
        .active
        .lock()
        .map_err(|e| e.to_string())?
        .take()
        .ok_or_else(|| "No recording is in progress".to_string())?;

    // stop_recording() blocks joining the recorder thread — run it off the async
    // executor so it doesn't stall this command's tokio task.
    let wav_bytes = tokio::task::spawn_blocking(move || core_stop_mic_recording(handle))
        .await
        .map_err(|e| format!("recording thread panicked: {e}"))?
        .map_err(|e| e.to_string())?;

    let config = load_config().map_err(|error| error.to_string())?;
    core_transcribe_mic_recording(&config, wav_bytes)
        .await
        .map_err(|e| e.to_string())
}

/// Stops and discards a canceled recording without invoking a transcription provider.
#[tauri::command]
pub(crate) async fn cancel_mic_recording(
    state: tauri::State<'_, MicRecordingState>,
) -> Result<(), String> {
    let handle = state.active.lock().map_err(|e| e.to_string())?.take();
    if let Some(handle) = handle {
        tokio::task::spawn_blocking(move || core_stop_mic_recording(handle))
            .await
            .map_err(|e| format!("recording thread panicked: {e}"))?
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#![recursion_limit = "256"]

// Grouped by theme into subdirectories (agent/media/integrations/search/system)
// rather than 40 flat sibling files — browser/cron/orchestration/prompts were
// already organized this way. Each group module just declares its members;
// the `pub use group::member;` lines below re-export every member at its old
// flat path (`crate::chat`, `crate::config`, ...) so this reorganization is
// transparent to every existing `crate::chat::X` / `mint_core::chat::X`
// reference elsewhere in the workspace — nothing outside this file changed.
pub mod agent;
pub mod avatar_bridge;
pub mod browser;
pub mod cron;
pub mod integrations;
pub mod live_sync;
pub mod media;
pub mod orchestration;
pub mod prompts;
pub mod search;
pub mod slash;
pub mod system;

pub use agent::agent_loop;
pub use agent::chat;
pub use agent::code_tools;
pub use agent::memory;
pub use agent::safety;
pub use agent::skills;
pub use agent::subagents;
pub use agent::tasks;

pub use media::auto_shorts;
pub use media::gemini_live;
pub use media::image_gen;
pub use media::image_search;
pub use media::mic_transcribe;
pub use media::pictures;
pub use media::speech;
pub use media::subtitle;
pub use media::timeline;
pub use media::tts;
pub use media::video_edit;
pub use media::video_gen;

pub use integrations::bridge_health;
pub use integrations::hooks;
pub use integrations::mcp;
pub use integrations::oauth;
pub use integrations::plugins;

pub use search::files;
pub use search::knowledge;
pub use search::linked_folders;
pub use search::semantic;
pub use search::symbols;
pub use search::web_search;

pub use system::auth;
pub use system::bg_shell;
pub use system::calculation;
pub use system::config;
pub use system::docker_sandbox;
pub use system::shell;
pub use system::stock;
pub use system::weather;

pub use agent_loop::{AgentActionFuture, AgentLoopError, parse_agent_json, run_agent_loop};
pub use auth::{
    AuthError, AuthUser, create_session, destroy_session, get_user, login_user,
    profile_pictures_dir, register_user, save_avatar_file, session_user_id, update_profile,
    user_db_path,
};
pub use chat::{
    ChatError, ChatMessage, ChatRequest, ChatResponse, ChatRole, ContentBlock, ToolCall, ToolSpec,
    send_chat, send_chat_with_fallback, stream_chat, stream_chat_with_fallback,
};

pub use auto_shorts::{
    AiEditStepResult, AiEditVideoRequest, AiEditVideoResult, AutoShortsError, MakeShortsRequest,
    MakeShortsResult, ShortClipInfo, ai_edit_video, make_shorts,
};
pub use browser::{
    BrowserTab, click, get_element_coordinates, is_browser_running, key_press, list_tabs,
    mouse_click, mouse_move, navigate, read_page_text, screenshot, spawn_automation_browser,
    type_text, type_text_native,
};
pub use calculation::{CalculationError, CalculationReport, calculate};
pub use code_tools::{
    AppliedCodeEdit, CodeEdit, CodeEditPreview, CodeEditProposal, CodeFile, CodeInspectionError,
    CodePatchHunk, CodePlan, CodeSearchHit, RepositorySummary, apply_code_edits, build_code_patch,
    fetch_github_repo_summary, inspect_code_plan, list_code_files, parse_github_url,
    propose_code_edits, read_code_file, repository_summary, search_code,
};
pub use config::{
    AgentConfig, ConfigError, CustomProvider, CustomProviderHeader, CustomProviderModel,
    MintConfig, PermissionDecision, PermissionRule, ToolCallingMode, config_path,
    initialize_config, load_config, permission_decision_for, save_config, set_config_value,
};
pub use cron::{
    CronError, CronJob, CronJobDraft, CronStore, localize_schedule, start_cron_scheduler,
};
pub use docker_sandbox::{docker_available, has_session, start_session, stop_session};
pub use files::{FileOperationError, PathKind, PathMatch, create_folder, find_paths};
pub use gemini_live::{
    GeminiLiveEvent, GeminiLiveHandle, start_session as start_gemini_live_session,
};
pub use hooks::{
    HookEntry, HookError, HookEvent, PreHookOutcome, add_hook, clear_hooks, list_hooks,
    remove_hook, run_post_tool_hooks, run_pre_tool_hooks,
};
pub use image_gen::{
    GeneratedImage, ImageGenError, ImageGenRequest, ImageGenResponse, generate_images,
};
pub use image_search::{ImageHit, ImageSearchError, ImageSearchReport, image_search};
pub use knowledge::{
    KnowledgeError, KnowledgeHit, KnowledgeSource, KnowledgeStore, extract_document_text,
};
pub use linked_folders::{
    LinkedFolder, LinkedFolderDraft, LinkedFolderError, add_linked_folder,
    configured_linked_folders, list_linked_folders, remove_linked_folder, spawn_linked_folder_note,
};
pub use mcp::{
    McpError, McpServer, add_mcp_server, allow_mcp_tool, allow_tool_in, call_configured_mcp_tool,
    call_mcp_tool, clear_mcp_servers, clear_servers_in, close_all_mcp_sessions, close_mcp_session,
    configured_mcp_servers, disallow_mcp_tool, disallow_tool_in, drain_mcp_notifications,
    get_server_prompt, list_mcp_servers, list_server_prompts, list_server_resources,
    list_server_tools, mcp_tool_allowlist, read_server_resource, reauth_mcp_server,
    remove_mcp_server, remove_server_in, set_mcp_server_disabled, set_server_disabled_in,
    update_mcp_server, update_server_in, upsert_server_in,
};
pub use memory::{
    CHAT_CLI_ID, ChatSession, DEFAULT_CONVERSATION_ID, Fact, InteractionMemory, LearnedSkill,
    MemoryError, MemoryStore, WorkspaceSession, memory_path, scoped_chat_id, subagent_name,
};
pub use mic_transcribe::{
    MicRecordingHandle, MicTranscribeError, start_recording, stop_recording, transcribe_recording,
};
pub use orchestration::{
    AgentApproval, AgentProgress, AgentResult, ApprovalOutcome, AskUserOption, OrchestrationError,
    orchestrate_agent_loop, orchestrate_chat, orchestrate_chat_stream,
    orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback,
};
pub use pictures::{
    PictureEntry, PictureError, delete_saved_picture, list_saved_pictures, parse_data_uri,
    save_chat_images, save_sent_image,
};
pub use plugins::{
    NativePlugin, PluginError, execute_native_plugin, native_plugin_enabled, native_plugins,
    set_native_plugin_enabled_in,
};
pub use safety::{
    Capability, SafetyError, SafetyTier, ShellClassification, ShellCommandMode,
    assert_path_capability, classify_shell_command, shell_mode_allowed,
};
pub use semantic::{
    SemanticChunk, SemanticError, SemanticHit, SemanticIndex, index_semantic_code,
    search_semantic_code,
};
pub use shell::{
    SandboxAvailability, ShellError, ShellOutput, run_shell_command, sandbox_availability,
};
pub use skills::{SkillError, learn_skill, learned_skills_context};
pub use speech::{
    DetectSilenceRequest, SilenceRange, SpeechError, TranscribeRequest, TranscriptSegment,
    TranscriptionResult, detect_silence, transcribe,
};
pub use stock::{StockError, StockReport, stock};
pub use subagents::{
    SubagentDefinition, SubagentDraft, delete_subagent, find_subagent, list_subagents,
    save_subagent,
};
pub use subtitle::{
    BurnSubtitleRequest, SubtitleError, SubtitleStyle, TranslateSubtitleRequest, burn_subtitles,
    generate_srt, secs_to_srt_timestamp, translate_subtitles,
};
pub use symbols::{CodeSymbol, SymbolError, SymbolIndex, build_symbol_index};
pub use tasks::{Task, TaskError, TaskStore, tasks_path};
pub use timeline::{
    RenderTimelineRequest, RenderTimelineResult, Timeline, TimelineAudio, TimelineClip,
    TimelineEffect, TimelineError, TimelineOutput, TimelineSubtitle, render_timeline,
    timeline_from_json, timeline_to_json,
};
pub use tts::{TtsUrl, google_tts_urls};
pub use video_edit::{
    CropRequest, DuckMusicRequest, ExportRequest, ExtractAudioRequest, MergeRequest,
    RemoveSilenceRequest, ReorderClipsRequest, ResizeRequest, TrimRequest, VideoEditError,
    VideoEditResult, VideoInfo, ZoomSpeakerRequest, audio_duck_music, effect_zoom_on_speaker,
    timeline_reorder, video_crop, video_export, video_extract_audio, video_load, video_merge,
    video_remove_silence, video_resize, video_trim,
};
pub use video_gen::{VideoGenError, VideoGenRequest, VideoGenResponse, generate_video};
pub use weather::{WeatherError, WeatherReport, weather};
pub mod api_server;
pub use api_server::start_api_server;
pub use channels::start_channels;
pub use integrations::channels;

pub static HTTP_CLIENT: std::sync::LazyLock<reqwest::Client> =
    std::sync::LazyLock::new(reqwest::Client::new);

pub static ACTIVE_AGENTS: std::sync::LazyLock<
    std::sync::Mutex<std::collections::HashMap<String, tokio::task::AbortHandle>>,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

pub fn cancel_agent(chat_id: &str) -> bool {
    let mut agents = ACTIVE_AGENTS.lock().unwrap();
    if let Some(handle) = agents.remove(chat_id) {
        handle.abort();
        true
    } else {
        false
    }
}

/// Pending web-side approval requests, keyed by a random token — the HTTP
/// equivalent of desktop's `ApprovalsState` (`src-tauri/src/lib.rs`). A
/// `oneshot::Sender` can only be consumed once, so entries are removed (not
/// just read) on resolution, same as desktop's `submit_tool_approval`.
pub static PENDING_APPROVALS: std::sync::LazyLock<
    std::sync::Mutex<
        std::collections::HashMap<String, tokio::sync::oneshot::Sender<ApprovalOutcome>>,
    >,
> = std::sync::LazyLock::new(|| std::sync::Mutex::new(std::collections::HashMap::new()));

/// Resolves a pending approval by token — mirrors desktop's
/// `submit_tool_approval` command exactly (a non-empty `answer` always wins
/// over `approved`, used for the "ask a clarifying question" flow). Returns
/// `false` if the token was never pending (already resolved, expired, or
/// bogus) rather than erroring, matching `cancel_agent`'s lenient style.
pub fn resolve_pending_approval(token: &str, approved: bool, answer: Option<String>) -> bool {
    let Some(tx) = PENDING_APPROVALS.lock().unwrap().remove(token) else {
        return false;
    };
    let outcome = match answer.filter(|a| !a.trim().is_empty()) {
        Some(ans) => ApprovalOutcome::Intercepted(ans),
        None if approved => ApprovalOutcome::Approved,
        None => ApprovalOutcome::Denied,
    };
    let _ = tx.send(outcome);
    true
}

/// Plain-text notices from detached background tasks (currently just
/// [`linked_folders::spawn_linked_folder_note`]) that a UI surface can drain
/// and display whenever convenient — mirrors how `crates/mint-cli`'s `/bg`
/// job queue works, just at the process level since linked-folder notes can
/// be written from any of the CLI/desktop/web chat paths, not CLI-only code.
pub static LINKED_FOLDER_NOTICES: std::sync::LazyLock<std::sync::Mutex<Vec<String>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(Vec::new()));

pub fn push_linked_folder_notice(message: String) {
    LINKED_FOLDER_NOTICES.lock().unwrap().push(message);
}

pub fn take_linked_folder_notices() -> Vec<String> {
    std::mem::take(&mut *LINKED_FOLDER_NOTICES.lock().unwrap())
}

#[cfg(test)]
mod linked_folder_notice_tests {
    use super::*;

    #[test]
    fn pushed_notices_are_drained_exactly_once() {
        // Isolate from other tests touching the same process-global queue.
        let _ = take_linked_folder_notices();

        push_linked_folder_notice("Saved note to Food (Food/mint-notes/2026-08-09.md)".into());
        push_linked_folder_notice(
            "Saved note to YouTube (YouTube/mint-notes/2026-08-09.md)".into(),
        );

        let notices = take_linked_folder_notices();
        assert_eq!(notices.len(), 2);
        assert!(notices[0].contains("Food"));

        // Draining empties the queue.
        assert!(take_linked_folder_notices().is_empty());
    }
}

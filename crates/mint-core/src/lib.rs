#![recursion_limit = "256"]

pub mod agent_loop;
pub mod auth;
pub mod browser;
pub mod chat;
pub mod code_tools;
pub mod config;
pub mod files;
pub mod hooks;
pub mod image_gen;
pub mod image_search;
pub mod knowledge;
pub mod mcp;
pub mod memory;
pub mod orchestration;
pub mod pictures;
pub mod prompts;
pub mod auto_shorts;
pub mod oauth;
pub mod plugins;
pub mod safety;
pub mod semantic;
pub mod shell;
pub mod skills;
pub mod speech;
pub mod subtitle;
pub mod symbols;
pub mod tasks;
pub mod timeline;
pub mod tts;
pub mod video_edit;
pub mod video_gen;
pub mod weather;
pub mod stock;
pub mod calculation;
pub mod web_search;
pub mod workflows;

pub use agent_loop::{AgentActionFuture, AgentLoopError, parse_agent_json, run_agent_loop};
pub use auth::{
    AuthError, AuthUser, create_session, destroy_session, get_user, login_user,
    profile_pictures_dir, register_user, save_avatar_file, session_user_id, update_profile,
    user_db_path,
};
pub use chat::{
    ChatError, ChatRequest, ChatResponse, send_chat, send_chat_with_fallback, stream_chat,
    stream_chat_with_fallback,
};

pub use browser::{
    BrowserTab, click, get_element_coordinates, is_browser_running, key_press, list_tabs,
    mouse_click, mouse_move, navigate, read_page_text, screenshot, spawn_automation_browser,
    type_text, type_text_native,
};
pub use code_tools::{
    AppliedCodeEdit, CodeEdit, CodeEditPreview, CodeEditProposal, CodeFile, CodeInspectionError,
    CodePatchHunk, CodePlan, CodeSearchHit, RepositorySummary, apply_code_edits, build_code_patch,
    fetch_github_repo_summary, inspect_code_plan, list_code_files, parse_github_url,
    propose_code_edits, read_code_file, repository_summary, search_code,
};
pub use config::{
    AgentConfig, ConfigError, CustomProvider, CustomProviderHeader, CustomProviderModel,
    MintConfig, config_path, initialize_config, load_config, save_config, set_config_value,
};
pub use files::{FileOperationError, PathKind, PathMatch, create_folder, find_paths};
pub use hooks::{
    HookEntry, HookError, HookEvent, PreHookOutcome, add_hook, clear_hooks, list_hooks,
    remove_hook, run_post_tool_hooks, run_pre_tool_hooks,
};
pub use image_gen::{
    GeneratedImage, ImageGenError, ImageGenRequest, ImageGenResponse, generate_images,
};
pub use knowledge::{
    KnowledgeError, KnowledgeHit, KnowledgeSource, KnowledgeStore, extract_document_text,
};
pub use mcp::{
    McpError, McpServer, add_mcp_server, call_configured_mcp_tool, call_mcp_tool,
    clear_mcp_servers, configured_mcp_servers, list_mcp_servers, remove_mcp_server,
};
pub use memory::{
    CHAT_CLI_ID, ChatSession, DEFAULT_CONVERSATION_ID, InteractionMemory, LearnedSkill,
    MemoryError, MemoryStore, WorkspaceSession, memory_path,
};
pub use orchestration::{
    AgentApproval, AgentProgress, AgentResult, ApprovalOutcome, OrchestrationError,
    orchestrate_agent_loop, orchestrate_chat, orchestrate_chat_stream,
    orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback,
};
pub use pictures::{
    PictureEntry, PictureError, delete_saved_picture, list_saved_pictures, parse_data_uri,
    save_chat_images, save_sent_image,
};
pub use plugins::{NativePlugin, PluginError, execute_native_plugin, native_plugins};
pub use safety::{
    Capability, SafetyError, SafetyTier, ShellClassification, ShellCommandMode,
    assert_path_capability, classify_shell_command, shell_mode_allowed,
};
pub use semantic::{
    SemanticChunk, SemanticError, SemanticHit, SemanticIndex, index_semantic_code,
    search_semantic_code,
};
pub use shell::{ShellError, ShellOutput, run_shell_command};
pub use skills::{SkillError, learn_skill, learned_skills_context};
pub use symbols::{CodeSymbol, SymbolError, SymbolIndex, build_symbol_index};
pub use tasks::{Task, TaskError, TaskStore, tasks_path};
pub use tts::{TtsUrl, google_tts_urls};
pub use timeline::{
    RenderTimelineRequest, RenderTimelineResult, Timeline, TimelineAudio, TimelineClip,
    TimelineEffect, TimelineError, TimelineOutput, TimelineSubtitle, render_timeline,
    timeline_from_json, timeline_to_json,
};
pub use video_edit::{
    CropRequest, DuckMusicRequest, ExportRequest, ExtractAudioRequest, MergeRequest,
    RemoveSilenceRequest, ReorderClipsRequest, ResizeRequest, TrimRequest, VideoEditError,
    VideoEditResult, VideoInfo, ZoomSpeakerRequest, audio_duck_music, effect_zoom_on_speaker,
    timeline_reorder, video_crop, video_export, video_extract_audio, video_load, video_merge,
    video_remove_silence, video_resize, video_trim,
};
pub use auto_shorts::{
    AiEditStepResult, AiEditVideoRequest, AiEditVideoResult, AutoShortsError, MakeShortsRequest,
    MakeShortsResult, ShortClipInfo, ai_edit_video, make_shorts,
};
pub use speech::{
    DetectSilenceRequest, SilenceRange, SpeechError, TranscribeRequest, TranscriptSegment,
    TranscriptionResult, detect_silence, transcribe,
};
pub use subtitle::{
    BurnSubtitleRequest, SubtitleError, SubtitleStyle, TranslateSubtitleRequest,
    burn_subtitles, generate_srt, secs_to_srt_timestamp, translate_subtitles,
};
pub use video_gen::{VideoGenError, VideoGenRequest, VideoGenResponse, generate_video};
pub use weather::{WeatherError, WeatherReport, weather};
pub use stock::{StockError, StockReport, stock};
pub use image_search::{ImageHit, ImageSearchError, ImageSearchReport, image_search};
pub use calculation::{CalculationError, CalculationReport, calculate};
pub use workflows::{WorkflowError, load_workflows, save_workflows, workflows_path};
pub mod api_server;
pub use api_server::start_api_server;
pub mod channels;
pub use channels::start_channels;

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

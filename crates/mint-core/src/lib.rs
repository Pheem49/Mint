#![recursion_limit = "256"]

pub mod agent_loop;
pub mod chat;
pub mod code_tools;
pub mod config;
pub mod files;
pub mod knowledge;
pub mod mcp;
pub mod memory;
pub mod orchestration;
pub mod pictures;
pub mod plugins;
pub mod safety;
pub mod semantic;
pub mod shell;
pub mod skills;
pub mod symbols;
pub mod tasks;
pub mod tts;
pub mod weather;
pub mod web_search;
pub mod workflows;

pub use agent_loop::{AgentActionFuture, AgentLoopError, parse_agent_json, run_agent_loop};
pub use chat::{
    ChatError, ChatRequest, ChatResponse, send_chat, send_chat_with_fallback, stream_chat,
    stream_chat_with_fallback,
};

pub use code_tools::{
    AppliedCodeEdit, CodeEdit, CodeEditPreview, CodeEditProposal, CodeFile, CodeInspectionError,
    CodePatchHunk, CodePlan, CodeSearchHit, RepositorySummary, apply_code_edits, build_code_patch,
    inspect_code_plan, list_code_files, propose_code_edits, read_code_file, repository_summary,
    search_code,
};
pub use config::{
    ConfigError, MintConfig, config_path, initialize_config, load_config, save_config,
    set_config_value,
};
pub use files::{FileOperationError, PathKind, PathMatch, create_folder, find_paths};
pub use knowledge::{KnowledgeError, KnowledgeHit, KnowledgeSource, KnowledgeStore};
pub use mcp::{
    McpError, McpServer, add_mcp_server, call_configured_mcp_tool, call_mcp_tool,
    clear_mcp_servers, configured_mcp_servers, list_mcp_servers, remove_mcp_server,
};
pub use memory::{
    InteractionMemory, LearnedSkill, MemoryError, MemoryStore, WorkspaceSession, memory_path,
};
pub use orchestration::{
    OrchestrationError, orchestrate_chat, orchestrate_chat_stream,
    orchestrate_chat_stream_with_fallback, orchestrate_chat_with_fallback,
};
pub use pictures::{
    PictureEntry, PictureError, list_saved_pictures, parse_data_uri, save_chat_images,
    save_sent_image,
};
pub use plugins::{NativePlugin, PluginError, execute_native_plugin, native_plugins};
pub use safety::{
    Capability, SafetyError, SafetyTier, ShellClassification, assert_path_capability,
    classify_shell_command,
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
pub use weather::{WeatherError, WeatherReport, weather};
pub use workflows::{WorkflowError, load_workflows, workflows_path};

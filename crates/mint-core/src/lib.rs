pub mod chat;
pub mod config;
pub mod files;
pub mod memory;
pub mod orchestration;
pub mod plugins;
pub mod safety;
pub mod tasks;

pub use chat::{ChatError, ChatRequest, ChatResponse, send_chat, stream_chat};
pub use config::{
    ConfigError, MintConfig, config_path, load_config, save_config, set_config_value,
};
pub use files::{FileOperationError, PathKind, PathMatch, create_folder, find_paths};
pub use memory::{InteractionMemory, MemoryError, MemoryStore, memory_path};
pub use orchestration::{OrchestrationError, orchestrate_chat, orchestrate_chat_stream};
pub use plugins::{NativePlugin, PluginError, execute_native_plugin, native_plugins};
pub use safety::{
    Capability, SafetyError, SafetyTier, ShellClassification, assert_path_capability,
    classify_shell_command,
};
pub use tasks::{Task, TaskError, TaskStore, tasks_path};

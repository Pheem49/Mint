//! Core system concerns: user identity/auth, app config, shell execution
//! (foreground and background), and small self-contained info tools
//! (calculator, stock quotes, weather).

pub mod auth;
pub mod bg_shell;
pub mod calculation;
pub mod config;
pub mod docker_sandbox;
pub mod folder_picker;
pub mod html_preview;
pub mod knowledge_engine;
pub mod project_detector;
pub mod shell;
pub mod stock;
pub mod weather;
pub mod workspace;
pub mod workspace_history;

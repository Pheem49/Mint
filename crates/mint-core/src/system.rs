//! Core system concerns: user identity/auth, app config, shell execution
//! (foreground and background), and small self-contained info tools
//! (calculator, stock quotes, weather).

pub mod auth;
pub mod bg_shell;
pub mod calculation;
pub mod config;
pub mod shell;
pub mod stock;
pub mod weather;

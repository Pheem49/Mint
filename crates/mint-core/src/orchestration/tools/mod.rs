pub(super) mod avatar;
pub(super) mod code_search;
pub(super) mod files;
pub(super) mod git;
pub(super) mod media;
pub(super) mod misc;
pub(super) mod planning;
pub(super) mod plugins_mcp;
pub(super) mod project;
pub(super) mod shell;
pub(super) mod web;

pub(super) mod mods;

pub(super) mod dispatch;
pub use dispatch::execute_mod_requests;
pub(crate) use dispatch::execute_tool_from_json;
pub(super) use dispatch::*;

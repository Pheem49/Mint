use anyhow::Result;
use mint_core::{
    McpServer, add_mcp_server, allow_mcp_tool, call_configured_mcp_tool, clear_mcp_servers,
    disallow_mcp_tool, list_mcp_servers, reauth_mcp_server, remove_mcp_server,
    set_mcp_server_disabled, update_mcp_server,
};
use serde_json::Value;
use std::collections::BTreeMap;

pub fn list() -> Result<BTreeMap<String, McpServer>> {
    Ok(list_mcp_servers()?)
}

pub fn add(name: &str, command: &str, args: Vec<String>, env: Vec<String>) -> Result<()> {
    Ok(add_mcp_server(name, command, args, env)?)
}

pub fn remove(name: &str) -> Result<bool> {
    Ok(remove_mcp_server(name)?)
}

pub fn clear() -> Result<()> {
    Ok(clear_mcp_servers()?)
}

pub fn call(server_name: &str, tool_name: &str, arguments: Value) -> Result<Value> {
    Ok(call_configured_mcp_tool(server_name, tool_name, arguments)?)
}

pub fn reauth(server_name: &str) -> Result<bool> {
    Ok(reauth_mcp_server(server_name)?)
}

pub fn allow(server_name: &str, tool_name: &str) -> Result<bool> {
    Ok(allow_mcp_tool(server_name, tool_name)?)
}

pub fn disallow(server_name: &str, tool_name: &str) -> Result<bool> {
    Ok(disallow_mcp_tool(server_name, tool_name)?)
}

/// `false` when the server isn't configured (no-op).
pub fn set_disabled(name: &str, disabled: bool) -> Result<bool> {
    Ok(set_mcp_server_disabled(name, disabled)?)
}

/// Partial edit; `false` when the server isn't configured. `env` entries are
/// `KEY=VALUE`; `icon: Some(None)` clears the icon.
pub fn edit(
    name: &str,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<Vec<String>>,
    icon: Option<Option<String>>,
) -> Result<bool> {
    Ok(update_mcp_server(name, command, args, env, icon)?)
}

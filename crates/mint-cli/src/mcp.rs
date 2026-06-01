use anyhow::Result;
use mint_core::{
    McpServer, add_mcp_server, call_configured_mcp_tool, clear_mcp_servers, list_mcp_servers,
    remove_mcp_server,
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

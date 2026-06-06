use anyhow::Result;
use mint_core::{
    McpServer, add_mcp_server, call_configured_mcp_tool, clear_mcp_servers, list_mcp_servers,
    load_config, remove_mcp_server, save_config,
};
use serde_json::{Value, json};
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

pub fn allow(server_name: &str, tool_name: &str) -> Result<bool> {
    let mut config = load_config()?;
    let allowed = config
        .extra
        .entry("allowedMcpTools".into())
        .or_insert_with(|| json!({}));
    if !allowed.is_object() {
        *allowed = json!({});
    }

    let servers = allowed
        .as_object_mut()
        .expect("allowedMcpTools was normalized to an object");
    let tools = servers
        .entry(server_name.to_owned())
        .or_insert_with(|| json!([]));
    if !tools.is_array() {
        *tools = json!([]);
    }

    let tools = tools
        .as_array_mut()
        .expect("server allowlist was normalized to an array");
    let already_allowed = tools
        .iter()
        .filter_map(Value::as_str)
        .any(|value| value == "*" || value == tool_name);
    if already_allowed {
        return Ok(false);
    }

    tools.push(json!(tool_name));
    save_config(&config)?;
    Ok(true)
}

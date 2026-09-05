use anyhow::{Result, anyhow};
use mint_core::{
    McpRegistryEntry, McpServer, add_mcp_server, allow_mcp_tool, call_configured_mcp_tool,
    clear_mcp_servers, disallow_mcp_tool, expand_registry_entry, list_mcp_servers, load_config,
    mcp_registry, mcp_registry_entry, reauth_mcp_server, remove_mcp_server, save_config,
    set_mcp_server_disabled, update_mcp_server, upsert_server_in,
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

pub fn registry() -> &'static [McpRegistryEntry] {
    mcp_registry()
}

fn parse_kv(pairs: Vec<String>) -> Result<BTreeMap<String, String>> {
    pairs
        .into_iter()
        .map(|p| {
            p.split_once('=')
                .map(|(k, v)| (k.to_string(), v.to_string()))
                .ok_or_else(|| anyhow!("env must be KEY=VALUE, got '{p}'"))
        })
        .collect()
}

/// Add a catalog entry as a configured server. `arg_inputs` are the values for
/// the entry's `argInputs` (in order); `env` is `KEY=VALUE`. Returns the saved
/// server name.
pub fn registry_add(
    key: &str,
    name: Option<&str>,
    arg_inputs: Vec<String>,
    env: Vec<String>,
    allow_all: bool,
) -> Result<String> {
    let entry = mcp_registry_entry(key).ok_or_else(|| {
        anyhow!("No MCP catalog entry '{key}'. Run `mint mcp registry` to list them.")
    })?;

    let env_map = parse_kv(env)?;
    let missing: Vec<&str> = entry
        .required_env
        .iter()
        .map(|e| e.key.as_str())
        .filter(|k| !env_map.contains_key(*k))
        .collect();
    if !missing.is_empty() {
        return Err(anyhow!(
            "'{key}' needs: {} — pass each with --env KEY=VALUE",
            missing.join(", ")
        ));
    }

    let server_name = name.unwrap_or(entry.key.as_str()).to_string();
    let server = expand_registry_entry(entry, &arg_inputs, env_map);

    let mut config = load_config()?;
    upsert_server_in(&mut config, &server_name, server)?;
    save_config(&config)?;

    if allow_all {
        allow(&server_name, "*")?;
    }
    Ok(server_name)
}

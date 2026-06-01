use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, anyhow, bail};
use mint_core::{MintConfig, load_config, save_config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

pub fn list() -> Result<BTreeMap<String, McpServer>> {
    servers(&load_config()?)
}

pub fn add(name: &str, command: &str, args: Vec<String>, env: Vec<String>) -> Result<()> {
    let mut config = load_config()?;
    let mut servers = servers(&config)?;
    servers.insert(
        name.into(),
        McpServer {
            command: command.into(),
            args,
            env: parse_env(env)?,
        },
    );
    save_servers(&mut config, servers)
}

pub fn remove(name: &str) -> Result<bool> {
    let mut config = load_config()?;
    let mut servers = servers(&config)?;
    let removed = servers.remove(name).is_some();
    save_servers(&mut config, servers)?;
    Ok(removed)
}

pub fn clear() -> Result<()> {
    let mut config = load_config()?;
    save_servers(&mut config, BTreeMap::new())
}

pub fn call(server_name: &str, tool_name: &str, arguments: Value) -> Result<Value> {
    let server = list()?
        .remove(server_name)
        .ok_or_else(|| anyhow!("MCP server '{server_name}' is not configured"))?;
    let mut child = Command::new(&server.command)
        .args(&server.args)
        .envs(&server.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("unable to start MCP server '{}'", server.command))?;
    write_message(
        &mut child,
        &json!({"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"mint-cli","version":env!("CARGO_PKG_VERSION")}}}),
    )?;
    write_message(
        &mut child,
        &json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
    )?;
    write_message(
        &mut child,
        &json!({"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":tool_name,"arguments":arguments}}),
    )?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| anyhow!("MCP stdout unavailable"))?;
    for line in BufReader::new(stdout).lines() {
        let value: Value = serde_json::from_str(&line?)?;
        if value["id"] == 2 {
            let _ = child.kill();
            if let Some(error) = value.get("error") {
                bail!("MCP tool call failed: {error}");
            }
            return Ok(value.get("result").cloned().unwrap_or(Value::Null));
        }
    }
    bail!("MCP server exited without returning a tool result")
}

fn servers(config: &MintConfig) -> Result<BTreeMap<String, McpServer>> {
    Ok(config
        .extra
        .get("mcpServers")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default())
}

fn save_servers(config: &mut MintConfig, servers: BTreeMap<String, McpServer>) -> Result<()> {
    config
        .extra
        .insert("mcpServers".into(), serde_json::to_value(servers)?);
    Ok(save_config(config)?)
}

fn parse_env(values: Vec<String>) -> Result<BTreeMap<String, String>> {
    values
        .into_iter()
        .map(|value| {
            let (key, value) = value
                .split_once('=')
                .ok_or_else(|| anyhow!("MCP environment value must use KEY=VALUE format"))?;
            Ok((key.into(), value.into()))
        })
        .collect()
}

fn write_message(child: &mut std::process::Child, value: &Value) -> Result<()> {
    let stdin = child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow!("MCP stdin unavailable"))?;
    writeln!(stdin, "{value}")?;
    stdin.flush()?;
    Ok(())
}

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::mpsc,
    time::{Duration, Instant},
};

use mint_core::MintConfig;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

const MCP_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PluginInfo {
    pub name: &'static str,
    pub description: &'static str,
    pub migrated: bool,
}

pub fn configured_mcp_servers(
    config: &MintConfig,
) -> Result<BTreeMap<String, McpServerConfig>, String> {
    config
        .extra
        .get("mcpServers")
        .cloned()
        .map(serde_json::from_value)
        .transpose()
        .map_err(|error| format!("invalid mcpServers config: {error}"))
        .map(Option::unwrap_or_default)
}

pub fn list_plugins() -> Vec<PluginInfo> {
    vec![
        PluginInfo {
            name: "desktop-actions",
            description: "Allowlisted URL and desktop application launcher",
            migrated: true,
        },
        PluginInfo {
            name: "mcp-stdio",
            description: "Configured MCP server bridge over stdio JSON-RPC",
            migrated: true,
        },
    ]
}

pub fn call_mcp_tool(
    config: &MintConfig,
    server_name: &str,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, String> {
    let servers = configured_mcp_servers(config)?;
    let server = servers
        .get(server_name)
        .ok_or_else(|| format!("MCP server '{server_name}' is not configured"))?;
    let mut process = start_server(server)?;
    let result = exchange(
        &mut process,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/call",
            "params": { "name": tool_name, "arguments": arguments }
        }),
    );
    let _ = process.kill();
    result
}

fn start_server(server: &McpServerConfig) -> Result<Child, String> {
    let mut process = Command::new(&server.command)
        .args(&server.args)
        .envs(&server.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("unable to start MCP server '{}': {error}", server.command))?;
    write_message(
        &mut process,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "mint", "version": "2.0.0-alpha.1" }
            }
        }),
    )?;
    write_message(
        &mut process,
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )?;
    Ok(process)
}

fn exchange(process: &mut Child, request: Value) -> Result<Value, String> {
    write_message(process, &request)?;
    let stdout = process
        .stdout
        .take()
        .ok_or_else(|| "MCP server stdout is unavailable".to_string())?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let _ = sender.send(value);
            }
        }
    });

    let started = Instant::now();
    while started.elapsed() < MCP_TIMEOUT {
        let remaining = MCP_TIMEOUT.saturating_sub(started.elapsed());
        let response = receiver
            .recv_timeout(remaining)
            .map_err(|_| "MCP server response timed out".to_string())?;
        if response["id"] == 2 {
            if let Some(error) = response.get("error") {
                return Err(format!("MCP tool call failed: {error}"));
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
    }
    Err("MCP server response timed out".into())
}

fn write_message(process: &mut Child, message: &Value) -> Result<(), String> {
    let stdin = process
        .stdin
        .as_mut()
        .ok_or_else(|| "MCP server stdin is unavailable".to_string())?;
    writeln!(stdin, "{message}")
        .map_err(|error| format!("unable to write MCP request: {error}"))?;
    stdin
        .flush()
        .map_err(|error| format!("unable to flush MCP request: {error}"))
}

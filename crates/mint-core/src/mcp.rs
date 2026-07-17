use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    process::{Child, Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{ConfigError, MintConfig, load_config, save_config};

const MCP_TIMEOUT: Duration = Duration::from_secs(30);
static OAUTH_DETECTED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServer {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
pub enum McpError {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error("invalid MCP configuration: {0}")]
    InvalidConfig(#[from] serde_json::Error),
    #[error("MCP environment value must use KEY=VALUE format")]
    InvalidEnvironment,
    #[error("MCP server '{0}' is not configured")]
    MissingServer(String),
    #[error("MCP tool '{server}/{tool}' is not allowed by policy. To allow it, please run: /mcp allow {server} {tool} (or /mcp allow {server} * to allow all tools on this server)")]
    NotAllowed { server: String, tool: String },
    #[error("unable to start MCP server '{command}': {source}")]
    Start {
        command: String,
        source: std::io::Error,
    },
    #[error("MCP server stdin is unavailable")]
    MissingStdin,
    #[error("MCP server stdout is unavailable")]
    MissingStdout,
    #[error("unable to write MCP request: {0}")]
    Write(std::io::Error),
    #[error("MCP server response timed out")]
    Timeout,
    #[error("MCP tool call failed: {0}")]
    Tool(Value),
}

pub fn configured_mcp_servers(
    config: &MintConfig,
) -> Result<BTreeMap<String, McpServer>, McpError> {
    Ok(config
        .extra
        .get("mcpServers")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default())
}

pub fn list_mcp_servers() -> Result<BTreeMap<String, McpServer>, McpError> {
    configured_mcp_servers(&load_config()?)
}

pub fn add_mcp_server(
    name: &str,
    command: &str,
    args: Vec<String>,
    env: Vec<String>,
) -> Result<(), McpError> {
    let mut config = load_config()?;
    let mut servers = configured_mcp_servers(&config)?;
    servers.insert(
        name.into(),
        McpServer {
            command: command.into(),
            args,
            env: parse_env(env)?,
        },
    );
    save_mcp_servers(&mut config, servers)
}

pub fn remove_mcp_server(name: &str) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let mut servers = configured_mcp_servers(&config)?;
    let removed = servers.remove(name).is_some();
    save_mcp_servers(&mut config, servers)?;
    Ok(removed)
}

pub fn clear_mcp_servers() -> Result<(), McpError> {
    let mut config = load_config()?;
    save_mcp_servers(&mut config, BTreeMap::new())
}

pub fn call_mcp_tool(
    config: &MintConfig,
    server_name: &str,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    if !mcp_tool_allowed(config, server_name, tool_name) {
        return Err(McpError::NotAllowed {
            server: server_name.into(),
            tool: tool_name.into(),
        });
    }
    let servers = configured_mcp_servers(config)?;
    let server = servers
        .get(server_name)
        .ok_or_else(|| McpError::MissingServer(server_name.into()))?;
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

fn mcp_tool_allowed(config: &MintConfig, server_name: &str, tool_name: &str) -> bool {
    config
        .extra
        .get("allowedMcpTools")
        .and_then(|value| value.as_object())
        .map(|servers| {
            servers
                .get("*")
                .is_some_and(|tools| tool_allowed(tools, tool_name))
                || servers
                    .get(server_name)
                    .is_some_and(|tools| tool_allowed(tools, tool_name))
        })
        .unwrap_or(false)
}

fn tool_allowed(tools: &Value, tool_name: &str) -> bool {
    tools
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str())
                .any(|value| value == "*" || value == tool_name)
        })
        .unwrap_or(false)
}

pub fn call_configured_mcp_tool(
    server_name: &str,
    tool_name: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    call_mcp_tool(&load_config()?, server_name, tool_name, arguments)
}

fn save_mcp_servers(
    config: &mut MintConfig,
    servers: BTreeMap<String, McpServer>,
) -> Result<(), McpError> {
    config
        .extra
        .insert("mcpServers".into(), serde_json::to_value(servers)?);
    Ok(save_config(config)?)
}

fn parse_env(values: Vec<String>) -> Result<BTreeMap<String, String>, McpError> {
    values
        .into_iter()
        .map(|value| {
            let (key, value) = value.split_once('=').ok_or(McpError::InvalidEnvironment)?;
            Ok((key.into(), value.into()))
        })
        .collect()
}

fn find_url(line: &str) -> Option<String> {
    let start_idx = line.find("http://").or_else(|| line.find("https://"))?;
    let rest = &line[start_idx..];
    let end_idx = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>')
        .unwrap_or(rest.len());
    let url = rest[..end_idx].to_string();

    let lower = url.to_lowercase();
    if lower.contains("registry.npmjs.org") || lower.contains("npmjs.com") || lower.contains("github.com/modelcontextprotocol") {
        return None;
    }

    if lower.contains("oauth")
        || lower.contains("auth")
        || lower.contains("login")
        || lower.contains("google.com")
        || lower.contains("authorize")
    {
        Some(url)
    } else {
        None
    }
}

fn open_url_in_browser(url: &str) -> std::io::Result<()> {
    #[cfg(target_os = "linux")]
    {
        Command::new("xdg-open").arg(url).spawn().map(|_| ())
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().map(|_| ())
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd").args(&["/C", "start", url]).spawn().map(|_| ())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = url;
        Ok(())
    }
}

fn start_server(server: &McpServer) -> Result<Child, McpError> {
    let mut process = Command::new(&server.command)
        .args(&server.args)
        .envs(&server.env)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| McpError::Start {
            command: server.command.clone(),
            source,
        })?;

    if let Some(stderr) = process.stderr.take() {
        std::thread::spawn(move || {
            let reader = BufReader::new(stderr);
            for line in reader.lines().map_while(Result::ok) {
                if let Some(url) = find_url(&line) {
                    println!("\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n", url);
                    OAUTH_DETECTED.store(true, Ordering::Relaxed);
                    let _ = open_url_in_browser(&url);
                }
            }
        });
    }

    write_message(
        &mut process,
        &json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "mint", "version": env!("CARGO_PKG_VERSION") }
            }
        }),
    )?;
    write_message(
        &mut process,
        &json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }),
    )?;
    Ok(process)
}

fn exchange(process: &mut Child, request: Value) -> Result<Value, McpError> {
    write_message(process, &request)?;
    let stdout = process.stdout.take().ok_or(McpError::MissingStdout)?;
    let (sender, receiver) = mpsc::channel();
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines().map_while(Result::ok) {
            if let Some(url) = find_url(&line) {
                println!("\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n", url);
                OAUTH_DETECTED.store(true, Ordering::Relaxed);
                let _ = open_url_in_browser(&url);
            }
            if let Ok(value) = serde_json::from_str::<Value>(&line) {
                let _ = sender.send(value);
            }
        }
    });

    let timeout = if OAUTH_DETECTED.load(Ordering::Relaxed) {
        Duration::from_secs(120)
    } else {
        MCP_TIMEOUT
    };

    let started = Instant::now();
    while started.elapsed() < timeout {
        let response = receiver
            .recv_timeout(timeout.saturating_sub(started.elapsed()))
            .map_err(|_| McpError::Timeout)?;
        if response["id"] == 2 {
            OAUTH_DETECTED.store(false, Ordering::Relaxed);
            if let Some(error) = response.get("error") {
                return Err(McpError::Tool(error.clone()));
            }
            return Ok(response.get("result").cloned().unwrap_or(Value::Null));
        }
    }
    OAUTH_DETECTED.store(false, Ordering::Relaxed);
    Err(McpError::Timeout)
}

pub fn list_server_tools(
    config: &MintConfig,
    server_name: &str,
) -> Result<Value, McpError> {
    let servers = configured_mcp_servers(config)?;
    let server = servers
        .get(server_name)
        .ok_or_else(|| McpError::MissingServer(server_name.into()))?;
    let mut process = start_server(server)?;
    let result = exchange(
        &mut process,
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "tools/list",
            "params": {}
        }),
    );
    let _ = process.kill();
    result
}

fn write_message(process: &mut Child, message: &Value) -> Result<(), McpError> {
    let stdin = process.stdin.as_mut().ok_or(McpError::MissingStdin)?;
    writeln!(stdin, "{message}").map_err(McpError::Write)?;
    stdin.flush().map_err(McpError::Write)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_environment_without_equals_separator() {
        assert!(matches!(
            parse_env(vec!["TOKEN".into()]),
            Err(McpError::InvalidEnvironment)
        ));
    }

    #[test]
    fn rejects_mcp_tool_not_in_allowlist() {
        let config = MintConfig::default();
        assert!(matches!(
            call_mcp_tool(&config, "fake", "ping", json!({})),
            Err(McpError::NotAllowed { .. })
        ));
    }
}

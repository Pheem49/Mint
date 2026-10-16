use std::{
    collections::{BTreeMap, HashMap, VecDeque},
    io::{BufRead, BufReader, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::{
        Arc, LazyLock, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    time::Duration,
};

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{ConfigError, MintConfig, load_config, save_config};

#[path = "mcp_calls.rs"]
mod calls;
#[path = "mcp_catalog.rs"]
pub mod catalog;
#[path = "mcp_device.rs"]
pub mod device;
use calls::PROGRESS_SUBSCRIBERS;
pub(crate) use calls::call_mcp_tool_reviewed;
pub use calls::{McpProgress, call_mcp_tool_async, cancel_mcp_calls};

const MCP_TIMEOUT: Duration = Duration::from_secs(30);

/// Extended per-request timeout used only while a session is mid-OAuth (its
/// reader threads saw an auth URL) — the user needs time to finish the browser
/// flow before the pending request gives up.
const MCP_OAUTH_TIMEOUT: Duration = Duration::from_secs(120);

/// Upper bound on un-drained server notifications a session buffers; the oldest
/// are dropped past this. Nothing drains `notifications` yet (see
/// `drain_mcp_notifications`), so this just caps memory on a long-lived session
/// talking to a chatty server.
const MAX_BUFFERED_NOTIFICATIONS: usize = 64;

static MCP_ASYNC_RUNTIME: LazyLock<tokio::runtime::Runtime> = LazyLock::new(|| {
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("mint-mcp-remote")
        .build()
        .expect("failed to start MCP async runtime")
});

fn block_on_mcp<F: std::future::Future<Output = T> + Send + 'static, T: Send + 'static>(f: F) -> T {
    let (tx, rx) = std::sync::mpsc::channel();
    MCP_ASYNC_RUNTIME.spawn(async move {
        let res = f.await;
        let _ = tx.send(res);
    });
    rx.recv().expect("MCP async task failed")
}

/// Live MCP server sessions, keyed by server name. Held as `Arc<Mutex<_>>` per
/// session (rather than one lock guarding the whole map for a call's full
/// duration) so calls to *different* servers don't serialize behind each
/// other — only calls to the *same* server do, which matches the natural
/// constraint of one stdio pipe pair per child process.
static SESSIONS: LazyLock<Mutex<HashMap<String, Arc<Mutex<McpSession>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
// Startup can involve a slow initialize/OAuth exchange. Serialize only callers
// starting the same server, never unrelated sessions.
static SESSION_STARTS: LazyLock<Mutex<HashMap<String, Arc<Mutex<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServer {
    #[serde(default)]
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    /// Set by the Desktop/Web MCP settings toggle to keep a server configured
    /// but temporarily off. A disabled server is hidden from the agent's server
    /// list and every `tools/*`, `resources/*`, and `prompts/*` call against it
    /// is refused before a process is spawned.
    #[serde(default, skip_serializing_if = "is_false")]
    pub disabled: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transport: Option<String>,
    #[serde(
        default,
        rename = "timeoutSecs",
        skip_serializing_if = "Option::is_none"
    )]
    pub timeout_secs: Option<u64>,
}

impl McpServer {
    pub fn is_remote(&self) -> bool {
        self.url
            .as_deref()
            .map(|u| !u.trim().is_empty())
            .unwrap_or(false)
    }

    pub fn remote_url(&self) -> Option<&str> {
        self.url.as_deref().filter(|u| !u.trim().is_empty())
    }
}

fn is_false(value: &bool) -> bool {
    !*value
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
    #[error("MCP server '{0}' is disabled. Re-enable it in Settings > Plugins to use it.")]
    Disabled(String),
    #[error(
        "MCP tool '{server}/{tool}' is not allowed by policy. To allow it, please run: /mcp allow {server} {tool} (or /mcp allow {server} * to allow all tools on this server)"
    )]
    NotAllowed { server: String, tool: String },
    #[error("unable to start MCP server '{command}': {source}")]
    Start {
        command: String,
        source: std::io::Error,
    },
    #[error("remote MCP error: {0}")]
    Remote(String),
    #[error("MCP server stdin is unavailable")]
    MissingStdin,
    #[error("MCP server stdout is unavailable")]
    MissingStdout,
    #[error("unable to write MCP request: {0}")]
    Write(std::io::Error),
    #[error("MCP server response timed out")]
    Timeout,
    #[error("Stopped waiting for MCP; cancellation requested. Remote completion is unconfirmed")]
    Cancelled,
    #[error(
        "Stopped waiting for MCP; cancellation could not be delivered. Remote completion is unconfirmed"
    )]
    CancelledUndelivered,
    #[error(
        "MCP server response timed out; cancellation could not be delivered. Remote completion is unconfirmed"
    )]
    TimeoutUndelivered,
    #[error("invalid MCP timeout: expected 1–3600 seconds")]
    InvalidTimeout,
    #[error("MCP tool call failed: {0}")]
    Tool(Value),
    #[error("MCP preflight failed; no tool was executed: {0}")]
    Preflight(String),
}

pub fn configured_mcp_servers(
    config: &MintConfig,
) -> Result<BTreeMap<String, McpServer>, McpError> {
    let servers: BTreeMap<String, McpServer> = config
        .extra
        .get("mcpServers")
        .cloned()
        .map(serde_json::from_value)
        .transpose()?
        .unwrap_or_default();
    if servers
        .values()
        .any(|s| s.timeout_secs.is_some_and(|t| !(1..=3600).contains(&t)))
    {
        return Err(McpError::InvalidTimeout);
    }
    Ok(servers)
}

pub fn set_mcp_timeout(name: &str, timeout_secs: u64) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let changed = set_server_timeout_in(&mut config, name, timeout_secs)?;
    if changed {
        save_config(&config)?;
    }
    Ok(changed)
}

pub fn parse_mcp_timeout(value: &str) -> Result<u64, McpError> {
    value
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|v| (1..=3600).contains(v))
        .ok_or(McpError::InvalidTimeout)
}

pub fn set_server_timeout_in(
    config: &mut MintConfig,
    name: &str,
    timeout_secs: u64,
) -> Result<bool, McpError> {
    if !(1..=3600).contains(&timeout_secs) {
        return Err(McpError::InvalidTimeout);
    }
    let mut servers = configured_mcp_servers(config)?;
    let Some(server) = servers.get_mut(name) else {
        return Ok(false);
    };
    server.timeout_secs = Some(timeout_secs);
    write_servers(config, servers)?;
    Ok(true)
}

pub fn list_mcp_servers() -> Result<BTreeMap<String, McpServer>, McpError> {
    configured_mcp_servers(&load_config()?)
}

// ── Server-config mutation ────────────────────────────────────────────────────
//
// Two layers so every surface can share the logic without double-saving:
//   * `*_in(&mut MintConfig, …)` — pure, no persistence. Used by the shared
//     slash engine (`crate::slash`), which reports `ConfigChanged` and lets the
//     host `save_config`.
//   * the `pub fn foo(…)` wrappers below — `load_config → *_in → save_config`,
//     for the CLI (`mint mcp …`) and any caller that owns the whole round-trip.

/// Insert or replace a server entry in `config.extra["mcpServers"]` (no save).
pub fn upsert_server_in(
    config: &mut MintConfig,
    name: &str,
    server: McpServer,
) -> Result<(), McpError> {
    let mut servers = configured_mcp_servers(config)?;
    servers.insert(name.to_string(), server);
    write_servers(config, servers)
}

/// Remove a server entry (no save). Returns whether it existed.
pub fn remove_server_in(config: &mut MintConfig, name: &str) -> Result<bool, McpError> {
    let mut servers = configured_mcp_servers(config)?;
    let removed = servers.remove(name).is_some();
    write_servers(config, servers)?;
    Ok(removed)
}

/// Drop every configured server (no save).
pub fn clear_servers_in(config: &mut MintConfig) -> Result<(), McpError> {
    write_servers(config, BTreeMap::new())
}

/// Flip `mcpServers[name].disabled` (no save); kills a live session on disable.
/// Returns whether the server existed.
pub fn set_server_disabled_in(
    config: &mut MintConfig,
    name: &str,
    disabled: bool,
) -> Result<bool, McpError> {
    let mut servers = configured_mcp_servers(config)?;
    let Some(server) = servers.get_mut(name) else {
        return Ok(false);
    };
    server.disabled = disabled;
    write_servers(config, servers)?;
    if disabled {
        close_mcp_session(name);
    }
    Ok(true)
}

/// Partial edit of an existing server (no save); `None` fields are left as-is.
/// Returns whether the server existed.
pub fn update_server_in(
    config: &mut MintConfig,
    name: &str,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<BTreeMap<String, String>>,
    icon: Option<Option<String>>,
) -> Result<bool, McpError> {
    let mut servers = configured_mcp_servers(config)?;
    let Some(server) = servers.get_mut(name) else {
        return Ok(false);
    };
    if let Some(command) = command {
        server.command = command;
    }
    if let Some(args) = args {
        server.args = args;
    }
    if let Some(env) = env {
        server.env = env;
    }
    if let Some(icon) = icon {
        server.icon = icon;
    }
    write_servers(config, servers)?;
    Ok(true)
}

pub fn add_mcp_server(
    name: &str,
    command: &str,
    args: Vec<String>,
    env: Vec<String>,
) -> Result<(), McpError> {
    let mut config = load_config()?;
    upsert_server_in(
        &mut config,
        name,
        McpServer {
            command: command.into(),
            args,
            env: parse_env(env)?,
            icon: None,
            disabled: false,
            url: None,
            headers: None,
            transport: None,
            timeout_secs: None,
        },
    )?;
    Ok(save_config(&config)?)
}

pub fn add_remote_mcp_server(
    name: &str,
    url: &str,
    headers: Option<BTreeMap<String, String>>,
) -> Result<(), McpError> {
    let mut config = load_config()?;
    upsert_server_in(
        &mut config,
        name,
        McpServer {
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            icon: None,
            disabled: false,
            url: Some(url.to_string()),
            headers,
            transport: Some("sse".to_string()),
            timeout_secs: None,
        },
    )?;
    Ok(save_config(&config)?)
}

pub fn remove_mcp_server(name: &str) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let removed = remove_server_in(&mut config, name)?;
    save_config(&config)?;
    Ok(removed)
}

pub fn clear_mcp_servers() -> Result<(), McpError> {
    let mut config = load_config()?;
    clear_servers_in(&mut config)?;
    Ok(save_config(&config)?)
}

/// `load_config → set_server_disabled_in → save_config`. Returns whether the
/// server existed (a no-op, no save, if not).
pub fn set_mcp_server_disabled(name: &str, disabled: bool) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let existed = set_server_disabled_in(&mut config, name, disabled)?;
    if existed {
        save_config(&config)?;
    }
    Ok(existed)
}

/// `load_config → update_server_in → save_config`. `env` entries are `KEY=VALUE`
/// strings, parsed here. Returns whether the server existed.
pub fn update_mcp_server(
    name: &str,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<Vec<String>>,
    icon: Option<Option<String>>,
) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let env = env.map(parse_env).transpose()?;
    let existed = update_server_in(&mut config, name, command, args, env, icon)?;
    if existed {
        save_config(&config)?;
    }
    Ok(existed)
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
    let config = config.clone();
    let server_name = server_name.to_owned();
    let tool_name = tool_name.to_owned();
    block_on_mcp(async move {
        call_mcp_tool_async(&config, &server_name, &tool_name, arguments, "", |_| {}).await
    })
}

/// Lists the resources a configured MCP server exposes (`resources/list`).
pub fn list_server_resources(config: &MintConfig, server_name: &str) -> Result<Value, McpError> {
    with_session(config, server_name, |session| {
        session.request("resources/list", json!({}))
    })
}

/// Reads one resource from a configured MCP server (`resources/read`).
pub fn read_server_resource(
    config: &MintConfig,
    server_name: &str,
    uri: &str,
) -> Result<Value, McpError> {
    with_session(config, server_name, |session| {
        session.request("resources/read", json!({ "uri": uri }))
    })
}

/// Lists the prompts a configured MCP server exposes (`prompts/list`).
pub fn list_server_prompts(config: &MintConfig, server_name: &str) -> Result<Value, McpError> {
    with_session(config, server_name, |session| {
        session.request("prompts/list", json!({}))
    })
}

/// Fetches one prompt from a configured MCP server (`prompts/get`).
pub fn get_server_prompt(
    config: &MintConfig,
    server_name: &str,
    name: &str,
    arguments: Value,
) -> Result<Value, McpError> {
    with_session(config, server_name, |session| {
        session.request(
            "prompts/get",
            json!({ "name": name, "arguments": arguments }),
        )
    })
}

/// Drains (and clears) server-initiated notifications received on the given
/// server's session since the last call — e.g. `notifications/tools/list_changed`.
/// Returns an empty `Vec` if there's no live session for `server_name`.
pub fn drain_mcp_notifications(server_name: &str) -> Vec<Value> {
    // Never keep the registry locked while waiting for a session: raw
    // requests can hold that session while a checked writer needs the registry.
    let session = SESSIONS.lock().unwrap().get(server_name).cloned();
    match session {
        Some(session) => session
            .lock()
            .unwrap()
            .notifications
            .lock()
            .unwrap()
            .drain(..)
            .collect(),
        None => Vec::new(),
    }
}

/// Re-runs a configured server's OAuth flow in the foreground, for servers
/// (like `@pouyanafisi/gmail-mcp`) that expose a conventional `<command>
/// <args...> auth` invocation separate from normal stdio-MCP mode — the fix
/// for a stale/expired token that the persistent JSON-RPC session has no way
/// to trigger on its own. Output streams live to stdout/stderr as the child
/// runs (so the user sees the OAuth URL and any success/failure message);
/// any OAuth-looking URL is also auto-opened in the browser, same as the
/// persistent session's own detection.
///
/// Drops any existing persistent session first, since the stale one would
/// otherwise keep holding whatever process/tokens were there before.
pub fn reauth_mcp_server(server_name: &str) -> Result<bool, McpError> {
    let config = load_config()?;
    let servers = configured_mcp_servers(&config)?;
    let server = servers
        .get(server_name)
        .ok_or_else(|| McpError::MissingServer(server_name.to_string()))?;

    close_mcp_session(server_name);

    if server.is_remote() {
        return Ok(true);
    }

    let mut args = server.args.clone();
    args.push("auth".to_string());

    let mut child = Command::new(&server.command)
        .args(&args)
        .envs(&server.env)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|source| McpError::Start {
            command: server.command.clone(),
            source,
        })?;

    let stdout_handle = child
        .stdout
        .take()
        .map(|out| std::thread::spawn(move || stream_and_watch_for_oauth_url(out)));
    let stderr_handle = child
        .stderr
        .take()
        .map(|err| std::thread::spawn(move || stream_and_watch_for_oauth_url(err)));

    let status = child.wait().map_err(|source| McpError::Start {
        command: server.command.clone(),
        source,
    })?;
    if let Some(handle) = stdout_handle {
        let _ = handle.join();
    }
    if let Some(handle) = stderr_handle {
        let _ = handle.join();
    }

    Ok(status.success())
}

/// Echoes every line from a reauth child process's stdout/stderr to this
/// process's own stdout (so the user sees the tool's own OAuth prompts and
/// completion message), opening the browser the same way the persistent
/// session's reader threads do when an OAuth-looking URL appears.
fn stream_and_watch_for_oauth_url(pipe: impl std::io::Read) {
    for line in BufReader::new(pipe).lines().map_while(Result::ok) {
        println!("{line}");
        if let Some(url) = find_url(&line) {
            println!(
                "\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n",
                url
            );
            let _ = open_url_in_browser(&url);
        }
    }
}

/// Closes and removes one server's persistent session, if one is running.
pub fn close_mcp_session(server_name: &str) {
    // Release the registry before taking a session lock. Dispatch checks the
    // registry while holding stdin; legacy resource reads can hold both locks.
    let session = SESSIONS.lock().unwrap().remove(server_name);
    if let Some(session) = session {
        session.lock().unwrap().close();
    }
}

/// Closes and removes every server's persistent session.
pub fn close_all_mcp_sessions() {
    let sessions = SESSIONS
        .lock()
        .unwrap()
        .drain()
        .map(|(_, s)| s)
        .collect::<Vec<_>>();
    for session in sessions {
        session.lock().unwrap().close();
    }
}

/// Whether `server_name/tool_name` is covered by `allowedMcpTools` (an exact
/// tool entry, `"*"` for the server, or a `"*"` server key). The orchestration
/// layer uses this to skip the approval prompt for an already-trusted server.
pub fn is_mcp_tool_allowed(config: &MintConfig, server_name: &str, tool_name: &str) -> bool {
    mcp_tool_allowed(config, server_name, tool_name)
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

/// Serialize `servers` back into `config.extra["mcpServers"]` (no save).
fn write_servers(
    config: &mut MintConfig,
    servers: BTreeMap<String, McpServer>,
) -> Result<(), McpError> {
    config
        .extra
        .insert("mcpServers".into(), serde_json::to_value(servers)?);
    Ok(())
}

// ── Tool allowlist (`allowedMcpTools`) ────────────────────────────────────────

/// Add `tool` (or `"*"`) to `config.extra["allowedMcpTools"][server]` (no save).
/// Returns `false` when it was already covered — an exact match, or a `"*"`
/// entry already present for that server. Ported from
/// `crates/mint-cli/src/mcp.rs::allow` / `slash::allow_mcp_tool`.
pub fn allow_tool_in(config: &mut MintConfig, server: &str, tool: &str) -> bool {
    let allowed = config
        .extra
        .entry("allowedMcpTools".into())
        .or_insert_with(|| json!({}));
    if !allowed.is_object() {
        *allowed = json!({});
    }
    let servers = allowed.as_object_mut().expect("normalized to object");
    let list = servers
        .entry(server.to_owned())
        .or_insert_with(|| json!([]));
    if !list.is_array() {
        *list = json!([]);
    }
    let arr = list.as_array_mut().expect("normalized to array");
    if arr
        .iter()
        .any(|t| t.as_str() == Some(tool) || t.as_str() == Some("*"))
    {
        return false;
    }
    arr.push(Value::String(tool.to_owned()));
    true
}

/// Remove `tool` from `config.extra["allowedMcpTools"][server]` (no save).
/// `tool == "*"` clears the server's list entirely. Returns whether anything
/// changed.
pub fn disallow_tool_in(config: &mut MintConfig, server: &str, tool: &str) -> bool {
    let Some(arr) = config
        .extra
        .get_mut("allowedMcpTools")
        .and_then(|v| v.as_object_mut())
        .and_then(|m| m.get_mut(server))
        .and_then(|v| v.as_array_mut())
    else {
        return false;
    };
    let before = arr.len();
    if tool == "*" {
        arr.clear();
    } else {
        arr.retain(|t| t.as_str() != Some(tool));
    }
    before != arr.len()
}

/// Read view of `allowedMcpTools`: `server -> [tool, …]` (may contain `"*"`).
pub fn mcp_tool_allowlist(config: &MintConfig) -> BTreeMap<String, Vec<String>> {
    config
        .extra
        .get("allowedMcpTools")
        .and_then(|v| v.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(server, tools)| {
                    let tools = tools
                        .as_array()?
                        .iter()
                        .filter_map(|t| t.as_str().map(str::to_owned))
                        .collect();
                    Some((server.clone(), tools))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// `load_config → allow_tool_in → save_config`. Returns whether it was added.
pub fn allow_mcp_tool(server: &str, tool: &str) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let added = allow_tool_in(&mut config, server, tool);
    if added {
        save_config(&config)?;
    }
    Ok(added)
}

/// `load_config → disallow_tool_in → save_config`. Returns whether it changed.
pub fn disallow_mcp_tool(server: &str, tool: &str) -> Result<bool, McpError> {
    let mut config = load_config()?;
    let removed = disallow_tool_in(&mut config, server, tool);
    if removed {
        save_config(&config)?;
    }
    Ok(removed)
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

// ── Server registry ──────────────────────────────────────────────────────────
//
// A curated catalog of well-known MCP servers, authored once in
// `mcp-registry.json` at the repo root and read by every surface — the CLI
// (`mint mcp registry`), the TUI `/mcp` add flow, and the renderer's Add form
// (`import … from '../../../../mcp-registry.json'`). Purely a presets layer over
// the normal add path; nothing here is required to add a server.

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryArgInput {
    pub label: String,
    #[serde(default)]
    pub placeholder: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryEnvVar {
    pub key: String,
    pub label: String,
    /// A URL where the user can obtain this credential, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub help: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpRegistryEntry {
    /// Catalog id, and the default server name when added.
    pub key: String,
    pub name: String,
    #[serde(default)]
    pub desc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub icon: Option<String>,
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Extra positional args the user supplies (a path, a connection string).
    /// Their values are appended to `args` in order.
    #[serde(default, rename = "argInputs")]
    pub arg_inputs: Vec<McpRegistryArgInput>,
    #[serde(default, rename = "requiredEnv")]
    pub required_env: Vec<McpRegistryEnvVar>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
}

/// The catalog, parsed once from `mcp-registry.json` at the repo root. A parse
/// failure is a build-time authoring bug, so panic rather than ship an empty
/// list — same contract as `slash::catalog::SLASH_COMMANDS`.
static MCP_REGISTRY: LazyLock<Vec<McpRegistryEntry>> = LazyLock::new(|| {
    serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../mcp-registry.json"
    )))
    .expect("mcp-registry.json is valid JSON matching Vec<McpRegistryEntry>")
});

pub fn mcp_registry() -> &'static [McpRegistryEntry] {
    &MCP_REGISTRY
}

pub fn mcp_registry_entry(key: &str) -> Option<&'static McpRegistryEntry> {
    MCP_REGISTRY.iter().find(|e| e.key == key)
}

/// Build an [`McpServer`] from a registry entry: its base `args` plus
/// `extra_args` (the `arg_inputs` values, in order), the given `env`, and its
/// icon.
pub fn expand_registry_entry(
    entry: &McpRegistryEntry,
    extra_args: &[String],
    env: BTreeMap<String, String>,
) -> McpServer {
    let mut args = entry.args.clone();
    args.extend(extra_args.iter().cloned());
    McpServer {
        command: entry.command.clone(),
        args,
        env,
        icon: entry.icon.clone(),
        disabled: false,
        url: None,
        headers: None,
        transport: None,
        timeout_secs: None,
    }
}

fn find_url(line: &str) -> Option<String> {
    if serde_json::from_str::<Value>(line)
        .ok()
        .is_some_and(|value| value.get("result").is_some() && value.get("id").is_some())
    {
        return None;
    }
    let start_idx = line.find("http://").or_else(|| line.find("https://"))?;
    let rest = &line[start_idx..];
    let end_idx = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '<' || c == '>')
        .unwrap_or(rest.len());
    let url = rest[..end_idx].to_string();

    let lower = url.to_lowercase();
    if lower.contains("registry.npmjs.org")
        || lower.contains("npmjs.com")
        || lower.contains("github.com/modelcontextprotocol")
    {
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
        if Command::new("xdg-open").arg(url).spawn().is_ok() {
            return Ok(());
        }
        if Command::new("wslview").arg(url).spawn().is_ok() {
            return Ok(());
        }
        if Command::new("gio").args(["open", url]).spawn().is_ok() {
            return Ok(());
        }
        if Command::new("sensible-browser").arg(url).spawn().is_ok() {
            return Ok(());
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no browser launcher found (xdg-open, wslview, gio, sensible-browser)",
        ))
    }
    #[cfg(target_os = "macos")]
    {
        Command::new("open").arg(url).spawn().map(|_| ())
    }
    #[cfg(target_os = "windows")]
    {
        Command::new("cmd")
            .args(["/C", "start", "", url])
            .spawn()
            .map(|_| ())
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        let _ = url;
        Ok(())
    }
}

pub fn list_server_tools(config: &MintConfig, server_name: &str) -> Result<Value, McpError> {
    let config = config.clone();
    let server = server_name.to_owned();
    block_on_mcp(async move {
        calls::McpScope::new("")
            .run(catalog::list_tools(&config, &server, ""), MCP_TIMEOUT)
            .await
    })
}

/// Just the tool *names* a server exposes — for a UI "discover tools" picker
/// that feeds the `allowedMcpTools` allowlist. Loads config itself so hosts can
/// call it with only a name.
pub fn mcp_server_tool_names(server_name: &str) -> Result<Vec<String>, McpError> {
    let config = load_config()?;
    let result = list_server_tools(&config, server_name)?;
    Ok(result
        .get("tools")
        .and_then(Value::as_array)
        .map(|tools| {
            tools
                .iter()
                .filter_map(|t| t.get("name").and_then(Value::as_str).map(str::to_owned))
                .collect()
        })
        .unwrap_or_default())
}

/// A persistent MCP stdio connection: the child process stays alive across
/// calls instead of being spawned and killed for every single request, and a
/// single background reader thread (spawned once, not once per call) routes
/// each incoming line to whichever in-flight request it answers.
struct McpStdioSession {
    process: Child,
    stdin: Arc<Mutex<ChildStdin>>,
    connected: Arc<AtomicBool>,
    catalog_generation: Arc<AtomicU64>,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>,
    /// Server-initiated messages (a `method` but no `id`), e.g.
    /// `notifications/tools/list_changed` — captured rather than dropped, even
    /// though nothing consumes them yet (see `drain_mcp_notifications`). Bounded
    /// to the last `MAX_BUFFERED_NOTIFICATIONS`.
    notifications: Arc<Mutex<VecDeque<Value>>>,
    /// Set by *this session's* reader threads when they see an OAuth URL, so the
    /// next `request()` waits `MCP_OAUTH_TIMEOUT` instead of `MCP_TIMEOUT`.
    /// Per-session (not a process global) so one server's auth flow can't skew
    /// another server's request timeouts.
    oauth_pending: Arc<AtomicBool>,
}

struct McpRemoteSession {
    #[allow(dead_code)]
    url: String,
    post_endpoint: Arc<Mutex<String>>,
    headers: BTreeMap<String, String>,
    client: reqwest::Client,
    next_id: AtomicU64,
    pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>,
    notifications: Arc<Mutex<VecDeque<Value>>>,
    is_active: Arc<AtomicBool>,
    catalog_generation: Arc<AtomicU64>,
    server_info: Option<Value>,
    abort_handle: Option<tokio::task::AbortHandle>,
}

fn resolve_endpoint(base_url: &str, endpoint: &str) -> String {
    let endpoint = endpoint.trim();
    if endpoint.starts_with("http://") || endpoint.starts_with("https://") {
        return endpoint.to_string();
    }
    if let Ok(base) = reqwest::Url::parse(base_url) {
        if let Ok(joined) = base.join(endpoint) {
            return joined.to_string();
        }
    }
    if endpoint.starts_with('/') {
        if let Ok(base) = reqwest::Url::parse(base_url) {
            let origin = format!("{}://{}", base.scheme(), base.host_str().unwrap_or(""));
            let port = base.port().map(|p| format!(":{p}")).unwrap_or_default();
            return format!("{origin}{port}{endpoint}");
        }
    }
    format!(
        "{}/{}",
        base_url.trim_end_matches('/'),
        endpoint.trim_start_matches('/')
    )
}

impl McpRemoteSession {
    fn start(server: &McpServer) -> Result<Self, McpError> {
        let url = server
            .remote_url()
            .ok_or_else(|| McpError::Remote("missing remote URL".into()))?
            .to_string();

        let mut client_builder = reqwest::Client::builder().connect_timeout(MCP_TIMEOUT);
        let mut header_map = reqwest::header::HeaderMap::new();
        let headers = server.headers.clone().unwrap_or_default();
        for (k, v) in &headers {
            if let (Ok(name), Ok(val)) = (
                reqwest::header::HeaderName::from_bytes(k.as_bytes()),
                reqwest::header::HeaderValue::from_str(v),
            ) {
                header_map.insert(name, val);
            }
        }
        client_builder = client_builder.default_headers(header_map);
        let client = client_builder
            .build()
            .map_err(|e| McpError::Remote(format!("failed to build HTTP client: {e}")))?;

        let post_endpoint = Arc::new(Mutex::new(url.clone()));
        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let notifications: Arc<Mutex<VecDeque<Value>>> = Arc::new(Mutex::new(VecDeque::new()));
        let is_active = Arc::new(AtomicBool::new(true));
        let catalog_generation = Arc::new(AtomicU64::new(0));

        let sse_url = url.clone();
        let sse_client = client.clone();
        let sse_post_endpoint = Arc::clone(&post_endpoint);
        let sse_pending = Arc::clone(&pending);
        let sse_notifications = Arc::clone(&notifications);
        let sse_is_active = Arc::clone(&is_active);

        let (notify_endpoint_tx, notify_endpoint_rx) = mpsc::channel::<()>();
        let notify_endpoint_tx = Arc::new(Mutex::new(Some(notify_endpoint_tx)));

        let connect_res = block_on_mcp(async move {
            tokio::time::timeout(
                MCP_TIMEOUT,
                sse_client
                    .get(&sse_url)
                    .header(reqwest::header::ACCEPT, "text/event-stream")
                    .send(),
            )
            .await
            .ok()
            .and_then(Result::ok)
        });

        let mut abort_handle = None;

        if let Some(resp) = connect_res {
            if resp.status().is_success() {
                let base_url = url.clone();
                let post_ep = Arc::clone(&sse_post_endpoint);
                let reader_pending = Arc::clone(&sse_pending);
                let reader_notifications = Arc::clone(&sse_notifications);
                let stream_active = Arc::clone(&sse_is_active);
                let reader_generation = Arc::clone(&catalog_generation);
                let ep_tx = Arc::clone(&notify_endpoint_tx);

                let handle = MCP_ASYNC_RUNTIME.spawn(async move {
                    let mut stream = resp.bytes_stream();
                    let mut buffer = String::new();
                    let mut current_event: Option<String> = None;
                    let mut current_data: Vec<String> = Vec::new();

                    while let Some(chunk_res) = stream.next().await {
                        let bytes = match chunk_res {
                            Ok(b) => b,
                            Err(_) => break,
                        };
                        let text = match std::str::from_utf8(&bytes) {
                            Ok(t) => t,
                            Err(_) => continue,
                        };
                        buffer.push_str(text);

                        while let Some(pos) = buffer.find('\n') {
                            let line = buffer[..pos].trim_end_matches('\r').to_string();
                            buffer.drain(..=pos);

                            if line.is_empty() {
                                if !current_data.is_empty() {
                                    let data = current_data.join("\n");
                                    if current_event.as_deref() == Some("endpoint") {
                                        let resolved = resolve_endpoint(&base_url, &data);
                                        *post_ep.lock().unwrap() = resolved;
                                        if let Some(tx) = ep_tx.lock().unwrap().take() {
                                            let _ = tx.send(());
                                        }
                                    } else if let Ok(value) = serde_json::from_str::<Value>(&data) {
                                        match classify_mcp_line(&value) {
                                            McpLine::Response(id, response) => {
                                                if let Some(sender) =
                                                    reader_pending.lock().unwrap().remove(&id)
                                                {
                                                    let _ = sender.send(response);
                                                }
                                            }
                                            McpLine::Notification(notification) => {
                                                dispatch_notification(
                                                    &reader_generation,
                                                    &reader_notifications,
                                                    notification,
                                                );
                                            }
                                            McpLine::Other => {}
                                        }
                                    }
                                }
                                current_event = None;
                                current_data.clear();
                            } else if let Some(rest) = line.strip_prefix("event:") {
                                current_event = Some(rest.trim().to_string());
                            } else if let Some(rest) = line.strip_prefix("data:") {
                                current_data.push(rest.trim_start().to_string());
                            }
                        }
                    }
                    stream_active.store(false, Ordering::Relaxed);
                });
                abort_handle = Some(handle.abort_handle());

                let _ = notify_endpoint_rx.recv_timeout(Duration::from_millis(1500));
            }
        }

        let mut session = McpRemoteSession {
            url,
            post_endpoint,
            headers,
            client,
            next_id: AtomicU64::new(2),
            pending,
            notifications,
            is_active,
            catalog_generation,
            server_info: None,
            abort_handle,
        };

        let init_response = session.request(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "mint", "version": env!("CARGO_PKG_VERSION") }
            }),
        )?;

        session.server_info = init_response.get("serverInfo").cloned();

        let _ = session.notify("notifications/initialized", None);

        Ok(session)
    }

    fn is_alive(&mut self) -> bool {
        self.is_active.load(Ordering::Relaxed)
    }

    fn close(&mut self) {
        self.is_active.store(false, Ordering::Relaxed);
        if let Some(handle) = self.abort_handle.take() {
            handle.abort();
        }
    }

    fn notify(&mut self, method: &str, params: Option<Value>) -> Result<(), McpError> {
        let post_url = self.post_endpoint.lock().unwrap().clone();
        let mut payload = json!({
            "jsonrpc": "2.0",
            "method": method,
        });
        if let Some(p) = params {
            payload["params"] = p;
        }
        let client = self.client.clone();
        let headers = self.headers.clone();

        let _ = block_on_mcp(async move {
            let mut req = client.post(&post_url).timeout(MCP_TIMEOUT).json(&payload);
            for (k, v) in &headers {
                req = req.header(k, v);
            }
            let _ = req.send().await;
        });
        Ok(())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, McpError> {
        if !self.is_active.load(Ordering::Relaxed) {
            return Err(McpError::Remote(
                "Remote MCP session is disconnected".into(),
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, sender);

        let post_url = self.post_endpoint.lock().unwrap().clone();
        let payload = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let client = self.client.clone();
        let headers = self.headers.clone();

        let post_result = block_on_mcp(async move {
            let mut req = client.post(&post_url).timeout(MCP_TIMEOUT).json(&payload);
            for (k, v) in &headers {
                req = req.header(k, v);
            }
            req.send().await
        });

        let response = match post_result {
            Ok(resp) => resp,
            Err(e) => {
                self.pending.lock().unwrap().remove(&id);
                return Err(McpError::Remote(format!("HTTP POST request failed: {e}")));
            }
        };

        if !response.status().is_success() {
            let status = response.status();
            let body = block_on_mcp(async move { response.text().await.unwrap_or_default() });
            self.pending.lock().unwrap().remove(&id);
            return Err(McpError::Remote(format!("HTTP error {status}: {body}")));
        }

        let body_text = block_on_mcp(async move { response.text().await.unwrap_or_default() });
        if !body_text.trim().is_empty() {
            if let Ok(val) = serde_json::from_str::<Value>(&body_text) {
                if val.get("id").and_then(Value::as_u64) == Some(id) {
                    self.pending.lock().unwrap().remove(&id);
                    if let Some(err) = val.get("error") {
                        return Err(McpError::Tool(err.clone()));
                    }
                    return Ok(val.get("result").cloned().unwrap_or(Value::Null));
                }
            }
        }

        let res = receiver.recv_timeout(MCP_TIMEOUT).map_err(|_| {
            self.pending.lock().unwrap().remove(&id);
            McpError::Timeout
        })?;

        if let Some(error) = res.get("error") {
            return Err(McpError::Tool(error.clone()));
        }
        Ok(res.get("result").cloned().unwrap_or(Value::Null))
    }
}

/// How one incoming line from an MCP server's stdout should be routed.
/// A free function (not inlined into the reader loop) specifically so it's
/// unit-testable without a real subprocess.
#[derive(Debug, PartialEq)]
enum McpLine {
    /// A response to a specific in-flight request (has a numeric `id`).
    Response(u64, Value),
    /// A server-initiated message with no `id` (has a `method`).
    Notification(Value),
    /// Anything else (malformed, or an id-less/method-less object).
    Other,
}

fn classify_mcp_line(value: &Value) -> McpLine {
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        return McpLine::Response(id, value.clone());
    }
    if value.get("method").and_then(Value::as_str).is_some() {
        return McpLine::Notification(value.clone());
    }
    McpLine::Other
}

/// Appends a server notification, evicting the oldest so the queue never grows
/// past `MAX_BUFFERED_NOTIFICATIONS`. Free function so the eviction is testable
/// without a live subprocess.
fn dispatch_notification(
    generation: &AtomicU64,
    queue: &Mutex<VecDeque<Value>>,
    notification: Value,
) {
    if notification["method"] == "notifications/tools/list_changed" {
        generation.fetch_add(1, Ordering::AcqRel);
    }
    buffer_notification(queue, notification);
}

fn buffer_notification(queue: &Mutex<VecDeque<Value>>, notification: Value) {
    if notification.get("method").and_then(Value::as_str) == Some("notifications/progress")
        && let Some(token) = notification
            .pointer("/params/progressToken")
            .and_then(Value::as_str)
        && let Some(sender) = PROGRESS_SUBSCRIBERS.lock().unwrap().get(token)
    {
        let _ = sender.try_send(notification.clone());
    }
    let mut queue = queue.lock().unwrap();
    while queue.len() >= MAX_BUFFERED_NOTIFICATIONS {
        queue.pop_front();
    }
    queue.push_back(notification);
}

impl McpStdioSession {
    fn start(server: &McpServer) -> Result<Self, McpError> {
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

        let oauth_pending: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));

        if let Some(stderr) = process.stderr.take() {
            let stderr_oauth = Arc::clone(&oauth_pending);
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(url) = find_url(&line) {
                        println!(
                            "\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n",
                            url
                        );
                        stderr_oauth.store(true, Ordering::Relaxed);
                        let _ = open_url_in_browser(&url);
                    }
                }
            });
        }

        let stdin = process.stdin.take().ok_or(McpError::MissingStdin)?;
        let stdout = process.stdout.take().ok_or(McpError::MissingStdout)?;

        let pending: Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>> =
            Arc::new(Mutex::new(HashMap::new()));
        let notifications: Arc<Mutex<VecDeque<Value>>> = Arc::new(Mutex::new(VecDeque::new()));

        let reader_pending = Arc::clone(&pending);
        let reader_notifications = Arc::clone(&notifications);
        let reader_oauth = Arc::clone(&oauth_pending);
        let connected = Arc::new(AtomicBool::new(true));
        let reader_connected = Arc::clone(&connected);
        let catalog_generation = Arc::new(AtomicU64::new(0));
        let reader_generation = Arc::clone(&catalog_generation);
        std::thread::spawn(move || {
            // Isolates a panic in the read loop (e.g. a poisoned `pending`/
            // `notifications` lock from some other unrelated failure) so it's
            // logged with context instead of just the default panic hook's
            // generic message — otherwise every request still waiting on this
            // session silently sits out its full timeout with no clue why the
            // server stopped answering.
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                    if let Some(url) = find_url(&line) {
                        println!(
                            "\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n",
                            url
                        );
                        reader_oauth.store(true, Ordering::Relaxed);
                        let _ = open_url_in_browser(&url);
                    }
                    let Ok(value) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    match classify_mcp_line(&value) {
                        McpLine::Response(id, response) => {
                            if let Some(sender) = reader_pending.lock().unwrap().remove(&id) {
                                let _ = sender.send(response);
                            }
                        }
                        McpLine::Notification(notification) => {
                            dispatch_notification(
                                &reader_generation,
                                &reader_notifications,
                                notification,
                            );
                        }
                        McpLine::Other => {}
                    }
                }
            }));
            reader_connected.store(false, Ordering::Release);
            if let Err(payload) = result {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] MCP stdout reader thread panicked: {message}");
            }
        });

        let mut session = McpStdioSession {
            process,
            stdin: Arc::new(Mutex::new(stdin)),
            connected,
            catalog_generation,
            next_id: AtomicU64::new(2), // 1 is reserved for `initialize` below.
            pending,
            notifications,
            oauth_pending,
        };

        // Some servers (e.g. `@pouyanafisi/gmail-mcp`) don't tolerate
        // `notifications/initialized` arriving before they've actually sent
        // their `initialize` response, and silently stop answering every
        // request afterward if we don't wait here — so, unlike a regular
        // `request()` call, register id 1 and block on it before sending the
        // `initialized` notification.
        let (sender, receiver) = mpsc::channel();
        session.pending.lock().unwrap().insert(1, sender);
        session.write(&json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "mint", "version": env!("CARGO_PKG_VERSION") }
            }
        }))?;
        if receiver.recv_timeout(MCP_TIMEOUT).is_err() {
            session.pending.lock().unwrap().remove(&1);
            return Err(McpError::Timeout);
        }
        session.write(&json!({ "jsonrpc": "2.0", "method": "notifications/initialized" }))?;

        Ok(session)
    }

    /// Whether the child process is still running.
    fn is_alive(&mut self) -> bool {
        matches!(self.process.try_wait(), Ok(None))
    }

    fn write(&mut self, message: &Value) -> Result<(), McpError> {
        let mut stdin = self.stdin.lock().unwrap();
        writeln!(stdin, "{message}").map_err(McpError::Write)?;
        stdin.flush().map_err(McpError::Write)
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, McpError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (sender, receiver) = mpsc::channel();
        self.pending.lock().unwrap().insert(id, sender);

        if let Err(error) = self.write(&json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        })) {
            self.pending.lock().unwrap().remove(&id);
            return Err(error);
        }

        let timeout = if self.oauth_pending.load(Ordering::Relaxed) {
            MCP_OAUTH_TIMEOUT
        } else {
            MCP_TIMEOUT
        };
        let response = receiver.recv_timeout(timeout).map_err(|_| {
            self.pending.lock().unwrap().remove(&id);
            McpError::Timeout
        })?;
        self.oauth_pending.store(false, Ordering::Relaxed);
        if let Some(error) = response.get("error") {
            return Err(McpError::Tool(error.clone()));
        }
        Ok(response.get("result").cloned().unwrap_or(Value::Null))
    }
}

enum McpSessionBackend {
    Stdio(McpStdioSession),
    Remote(McpRemoteSession),
}

struct McpSession {
    backend: McpSessionBackend,
    notifications: Arc<Mutex<VecDeque<Value>>>,
    config_key: String,
    catalog: Arc<tokio::sync::Mutex<Option<catalog::Catalog>>>,
}

/// Pins both discovery and dispatch to one live catalog generation.
#[derive(Clone)]
pub(crate) struct SessionBinding {
    server: String,
    session: Arc<Mutex<McpSession>>,
    generation: u64,
    generation_counter: Arc<AtomicU64>,
}
impl SessionBinding {
    pub(crate) fn check(&self) -> Result<(), McpError> {
        self.check_fast()?;
        let mut session = self.session.lock().unwrap();
        if !session.is_alive() {
            return Err(McpError::Preflight(
                "Session disconnected; rediscover tools before dispatch".into(),
            ));
        }
        Ok(())
    }
    fn check_fast(&self) -> Result<(), McpError> {
        let registered = SESSIONS.lock().unwrap().get(&self.server).cloned();
        if !registered.is_some_and(|s| Arc::ptr_eq(&s, &self.session)) {
            return Err(McpError::Preflight(
                "Session was replaced; rediscover tools before dispatch".into(),
            ));
        }
        if self.generation_counter.load(Ordering::Acquire) != self.generation {
            return Err(McpError::Preflight(
                "Session or catalog changed; rediscover tools before dispatch".into(),
            ));
        }
        Ok(())
    }
    pub(crate) fn same_session(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.session, &other.session) && self.generation == other.generation
    }
}

impl McpSession {
    fn request(&mut self, method: &str, params: Value) -> Result<Value, McpError> {
        match &mut self.backend {
            McpSessionBackend::Stdio(s) => s.request(method, params),
            McpSessionBackend::Remote(s) => s.request(method, params),
        }
    }

    fn is_alive(&mut self) -> bool {
        match &mut self.backend {
            McpSessionBackend::Stdio(s) => s.is_alive(),
            McpSessionBackend::Remote(s) => s.is_alive(),
        }
    }

    fn close(&mut self) {
        match &mut self.backend {
            McpSessionBackend::Stdio(s) => {
                let _ = s.process.kill();
            }
            McpSessionBackend::Remote(s) => {
                s.close();
            }
        }
    }

    #[cfg(test)]
    fn process_id(&mut self) -> Option<u32> {
        match &mut self.backend {
            McpSessionBackend::Stdio(s) => Some(s.process.id()),
            McpSessionBackend::Remote(_) => None,
        }
    }

    #[cfg(test)]
    fn kill_process(&mut self) {
        match &mut self.backend {
            McpSessionBackend::Stdio(s) => {
                let _ = s.process.kill();
                let _ = s.process.wait();
            }
            McpSessionBackend::Remote(s) => s.close(),
        }
    }

    #[cfg(test)]
    fn oauth_pending(&self) -> Option<Arc<AtomicBool>> {
        match &self.backend {
            McpSessionBackend::Stdio(s) => Some(Arc::clone(&s.oauth_pending)),
            McpSessionBackend::Remote(_) => None,
        }
    }
}

impl Drop for McpSession {
    fn drop(&mut self) {
        self.close();
    }
}

pub fn test_remote_mcp_connection(
    url: &str,
    headers: Option<BTreeMap<String, String>>,
) -> Result<Value, McpError> {
    let url = url.trim();
    if !url.starts_with("http://") && !url.starts_with("https://") {
        return Err(McpError::Remote(
            "URL must start with http:// or https://".to_string(),
        ));
    }

    let server = McpServer {
        command: String::new(),
        args: Vec::new(),
        env: BTreeMap::new(),
        icon: None,
        disabled: false,
        url: Some(url.to_string()),
        headers,
        transport: Some("sse".to_string()),
        timeout_secs: None,
    };

    let mut session = McpRemoteSession::start(&server)?;
    let tools_res = session
        .request("tools/list", json!({}))
        .unwrap_or(json!({ "tools": [] }));
    let tools = tools_res
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let tools_count = tools.len();

    let server_info = session.server_info.clone().unwrap_or(json!({
        "name": "Remote MCP Server",
        "version": "1.0"
    }));

    Ok(json!({
        "ok": true,
        "server_info": server_info,
        "tools_count": tools_count,
        "tools": tools
    }))
}

/// Gets the live session for `server_name`, transparently (re)spawning one if
/// there isn't one yet or the previous process has exited. This lazy
/// respawn-on-next-use *is* the reconnect mechanism — there's no
/// exponential-backoff retry loop within a single call; if the respawn itself
/// fails, that error just propagates like any other `McpError::Start`.
fn get_or_start_session(
    config: &MintConfig,
    server_name: &str,
) -> Result<Arc<Mutex<McpSession>>, McpError> {
    let servers = configured_mcp_servers(config)?;
    let server = servers
        .get(server_name)
        .ok_or_else(|| McpError::MissingServer(server_name.into()))?;

    let config_key = serde_json::to_string(&json!([
        server.command,
        server.args,
        server.env,
        server.url,
        server.headers,
        server.transport
    ]))
    .unwrap();
    let start_lock = SESSION_STARTS
        .lock()
        .unwrap()
        .entry(server_name.into())
        .or_default()
        .clone();
    let _starting = start_lock.lock().unwrap();

    if server.disabled {
        // Turned off in Settings after a session was already running — kill it
        // so a stale process doesn't linger past the toggle.
        let session = SESSIONS.lock().unwrap().remove(server_name);
        if let Some(session) = session {
            session.lock().unwrap().close();
        }
        return Err(McpError::Disabled(server_name.into()));
    }

    let existing = SESSIONS.lock().unwrap().get(server_name).cloned();
    if let Some(session) = existing {
        let mut current = session.lock().unwrap();
        if current.config_key == config_key && current.is_alive() {
            drop(current);
            return Ok(session);
        }
        current.close();
    }

    let session = if server.is_remote() {
        let remote = McpRemoteSession::start(server)?;
        let notifications = Arc::clone(&remote.notifications);
        Arc::new(Mutex::new(McpSession {
            backend: McpSessionBackend::Remote(remote),
            notifications,
            config_key,
            catalog: Arc::new(tokio::sync::Mutex::new(None)),
        }))
    } else {
        let stdio = McpStdioSession::start(server)?;
        let notifications = Arc::clone(&stdio.notifications);
        Arc::new(Mutex::new(McpSession {
            backend: McpSessionBackend::Stdio(stdio),
            notifications,
            config_key,
            catalog: Arc::new(tokio::sync::Mutex::new(None)),
        }))
    };
    SESSIONS
        .lock()
        .unwrap()
        .insert(server_name.to_string(), Arc::clone(&session));
    Ok(session)
}

/// Runs `f` against `server_name`'s persistent session, only holding the
/// per-session lock (not the whole `SESSIONS` map) for the request's
/// duration, so concurrent calls to *other* servers aren't blocked by it.
fn with_session<T>(
    config: &MintConfig,
    server_name: &str,
    f: impl FnOnce(&mut McpSession) -> Result<T, McpError>,
) -> Result<T, McpError> {
    let session = get_or_start_session(config, server_name)?;
    let mut guard = session.lock().unwrap();
    f(&mut guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn invalid_device_arguments_are_blocked_before_tools_call() {
        let marker =
            std::env::temp_dir().join(format!("mint-invalid-call-{}", uuid::Uuid::new_v4()));
        let name = format!("validation-{}", uuid::Uuid::new_v4());
        let script = format!(
            r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line); m=r.get('method')
 if m=='initialize': result={{}}
 elif m=='tools/list': result={{'tools':[{{'name':'set_speed','description':'Set RPM','inputSchema':{{'type':'object','properties':{{'rpm':{{'type':'integer','minimum':0,'maximum':3000}}}},'required':['rpm'],'additionalProperties':False}}}}]}}
 elif m=='tools/call':
  open({:?},'w').close();result={{'content':[{{'type':'text','text':'accepted'}}]}}
 else: continue
 print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':result}}),flush=True)
"#,
            marker.to_string_lossy()
        );
        let mut config = MintConfig::default();
        config.extra.insert(
            "mcpServers".into(),
            json!({name.clone():{"command":"python3","args":["-u","-c",script]}}),
        );
        config
            .extra
            .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
        let result = call_mcp_tool(&config, &name, "set_speed", json!({"rpm":"fast"}));
        close_mcp_session(&name);
        let executed = marker.exists();
        if executed {
            std::fs::remove_file(marker).unwrap();
        }
        assert!(
            result.is_err(),
            "invalid arguments must fail locally: {result:?}"
        );
        assert!(!executed, "invalid arguments reached the device");
    }

    #[test]
    fn server_timeout_survives_config_round_trip() {
        let server: McpServer =
            serde_json::from_value(json!({"command":"node","timeoutSecs":75})).unwrap();
        assert_eq!(serde_json::to_value(server).unwrap()["timeoutSecs"], 75);
    }

    #[test]
    fn rejects_timeouts_outside_the_supported_range() {
        for value in [0, 3601] {
            let mut cfg = MintConfig::default();
            cfg.extra.insert(
                "mcpServers".into(),
                json!({"server":{"command":"node","timeoutSecs":value}}),
            );
            assert!(matches!(
                configured_mcp_servers(&cfg),
                Err(McpError::InvalidTimeout)
            ));
        }
    }

    #[test]
    fn result_links_do_not_trigger_the_oauth_browser_watcher() {
        assert_eq!(
            find_url(
                r#"{"jsonrpc":"2.0","id":2,"result":{"content":[{"type":"resource_link","uri":"https://example.test/authors/model"}]}}"#
            ),
            None
        );
        assert_eq!(
            find_url("Authorize at https://example.test/oauth"),
            Some("https://example.test/oauth".into())
        );
    }

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

    #[test]
    fn classify_mcp_line_routes_response_by_id() {
        let value = json!({"jsonrpc":"2.0","id":5,"result":{}});
        assert_eq!(classify_mcp_line(&value), McpLine::Response(5, value));
    }

    #[test]
    fn classify_mcp_line_routes_notification_by_method_without_id() {
        let value = json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"});
        assert_eq!(classify_mcp_line(&value), McpLine::Notification(value));
    }

    #[test]
    fn classify_mcp_line_prefers_id_over_method_when_both_present() {
        // A response can echo the request's "method" alongside its "result" in
        // some server implementations; id-based routing must win so it still
        // reaches its pending request instead of being misfiled as a
        // notification.
        let value = json!({"id": 7, "method": "tools/call", "result": {}});
        assert_eq!(classify_mcp_line(&value), McpLine::Response(7, value));
    }

    #[test]
    fn classify_mcp_line_ignores_lines_with_neither() {
        assert_eq!(classify_mcp_line(&json!({"jsonrpc":"2.0"})), McpLine::Other);
    }

    /// A minimal MCP-ish stdio server: for every line it receives that
    /// contains an `"id":<n>`, it replies with a canned result echoing that
    /// id. Lines without an id (e.g. `notifications/initialized`) get no
    /// reply, matching real JSON-RPC notification semantics.
    fn mock_echo_server() -> McpServer {
        McpServer {
            command: "python3".into(),
            args:vec!["-u".into(),"-c".into(),r#"
import sys,json
for line in sys.stdin:
 r=json.loads(line)
 if 'id' not in r: continue
 if r.get('method')=='tools/list': result={'tools':[{'name':n,'inputSchema':{'type':'object'}} for n in ['anything','x','ping','one','two']]}
 else: result={'echo':True}
 print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':result}),flush=True)
"#.into()],
            env: BTreeMap::new(),
            icon: None,
            disabled: false,
            url: None,
            headers: None,
            transport: None,
            timeout_secs: None,
        }
    }

    fn config_with_mock_echo_server(name: &str) -> MintConfig {
        let mut config = MintConfig::default();
        config
            .extra
            .insert("mcpServers".into(), json!({ name: mock_echo_server() }));
        config
            .extra
            .insert("allowedMcpTools".into(), json!({ name: ["*"] }));
        config
    }

    #[cfg(unix)]
    #[test]
    fn review_notification_drain_does_not_block_unrelated_tool_calls() {
        let blocked_name = format!("drain-blocked-{}", uuid::Uuid::new_v4());
        let healthy_name = format!("drain-healthy-{}", uuid::Uuid::new_v4());
        let blocked_config = config_with_mock_echo_server(&blocked_name);
        let healthy_config = config_with_mock_echo_server(&healthy_name);
        let blocked = get_or_start_session(&blocked_config, &blocked_name).unwrap();
        call_mcp_tool(&healthy_config, &healthy_name, "ping", json!({})).unwrap();

        let notification = json!({"jsonrpc":"2.0","method":"notifications/tools/list_changed"});
        // A raw resource/prompt request keeps this mutex while awaiting its
        // response. Hold it here so the scheduling does not depend on a server.
        let session = blocked.lock().unwrap();
        session
            .notifications
            .lock()
            .unwrap()
            .push_back(notification.clone());
        let completed = std::thread::scope(|threads| {
            let started = Arc::new(std::sync::Barrier::new(2));
            let ready = Arc::clone(&started);
            let drain_name = &blocked_name;
            let drain = threads.spawn(move || {
                ready.wait();
                drain_mcp_notifications(drain_name)
            });
            started.wait();
            // Let the drainer reach the contended session. Old code keeps the
            // registry locked there; fixed code releases it immediately.
            let deadline = std::time::Instant::now() + Duration::from_millis(100);
            while std::time::Instant::now() < deadline {
                if SESSIONS.try_lock().is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(1));
            }
            let (tx, rx) = mpsc::channel();
            let healthy_config = &healthy_config;
            let healthy_name = &healthy_name;
            threads.spawn(move || {
                let result = call_mcp_tool(healthy_config, healthy_name, "ping", json!({}));
                let _ = tx.send(result);
            });
            let completed = rx.recv_timeout(Duration::from_secs(2));
            // Always unblock and join both workers before asserting, even on
            // the regression path, so this test cannot deadlock the test suite.
            drop(session);
            assert_eq!(drain.join().unwrap(), vec![notification]);
            completed
        });
        close_mcp_session(&blocked_name);
        close_mcp_session(&healthy_name);
        assert!(
            completed.is_ok_and(|result| result.is_ok()),
            "notification drain blocked an unrelated server's tool call"
        );
        assert!(drain_mcp_notifications(&blocked_name).is_empty());
    }

    #[test]
    fn persistent_session_is_reused_across_calls_and_respawned_after_exit() {
        let name = "mock-echo-reuse-test";
        close_mcp_session(name);
        let config = config_with_mock_echo_server(name);

        let first = call_mcp_tool(&config, name, "anything", json!({})).unwrap();
        assert_eq!(first["echo"], true);
        let pid_after_first = SESSIONS
            .lock()
            .unwrap()
            .get(name)
            .unwrap()
            .lock()
            .unwrap()
            .process_id()
            .unwrap();

        let second = call_mcp_tool(&config, name, "anything", json!({})).unwrap();
        assert_eq!(second["echo"], true);
        let pid_after_second = SESSIONS
            .lock()
            .unwrap()
            .get(name)
            .unwrap()
            .lock()
            .unwrap()
            .process_id()
            .unwrap();
        assert_eq!(
            pid_after_first, pid_after_second,
            "the same child process should be reused across calls, not respawned"
        );

        // Kill the process out from under the session to simulate it dying on
        // its own, then confirm the next call transparently respawns instead
        // of erroring.
        {
            let sessions = SESSIONS.lock().unwrap();
            let mut session = sessions.get(name).unwrap().lock().unwrap();
            session.kill_process();
        }
        let third = call_mcp_tool(&config, name, "anything", json!({})).unwrap();
        assert_eq!(third["echo"], true);
        let pid_after_respawn = SESSIONS
            .lock()
            .unwrap()
            .get(name)
            .unwrap()
            .lock()
            .unwrap()
            .process_id()
            .unwrap();
        assert_ne!(
            pid_after_second, pid_after_respawn,
            "a dead session should be respawned with a fresh process"
        );

        close_mcp_session(name);
    }

    #[test]
    fn disabled_server_is_refused_and_its_running_session_is_dropped() {
        let name = "mock-echo-disabled-test";
        close_mcp_session(name);
        let mut config = config_with_mock_echo_server(name);

        // Enabled: a call works and leaves a live session behind.
        assert_eq!(
            call_mcp_tool(&config, name, "anything", json!({})).unwrap()["echo"],
            true
        );
        assert!(SESSIONS.lock().unwrap().contains_key(name));

        // Flip the stored server to `disabled: true`, as the settings toggle does.
        config.extra.insert(
            "mcpServers".into(),
            json!({ name: { "command": "sh", "disabled": true } }),
        );

        match call_mcp_tool(&config, name, "anything", json!({})) {
            Err(McpError::Disabled(server)) => assert_eq!(server, name),
            other => panic!("expected McpError::Disabled, got {other:?}"),
        }
        // The session that predated the toggle must be gone, not left running.
        assert!(!SESSIONS.lock().unwrap().contains_key(name));

        close_mcp_session(name);
    }

    #[test]
    fn buffer_notification_evicts_oldest_past_the_cap() {
        let queue: Mutex<VecDeque<Value>> = Mutex::new(VecDeque::new());
        for i in 0..(MAX_BUFFERED_NOTIFICATIONS as i64 + 10) {
            buffer_notification(&queue, json!({ "seq": i }));
        }
        let queue = queue.lock().unwrap();
        assert_eq!(queue.len(), MAX_BUFFERED_NOTIFICATIONS);
        // The first 10 were dropped; the window is the most recent ones.
        assert_eq!(queue.front().unwrap()["seq"], 10);
        assert_eq!(
            queue.back().unwrap()["seq"],
            MAX_BUFFERED_NOTIFICATIONS as i64 + 9
        );
    }

    #[test]
    fn oauth_pending_is_per_session_not_global() {
        let (a, b) = ("mock-echo-oauth-a", "mock-echo-oauth-b");
        close_mcp_session(a);
        close_mcp_session(b);
        call_mcp_tool(&config_with_mock_echo_server(a), a, "x", json!({})).unwrap();
        call_mcp_tool(&config_with_mock_echo_server(b), b, "x", json!({})).unwrap();

        let sessions = SESSIONS.lock().unwrap();
        let a_flag = sessions
            .get(a)
            .unwrap()
            .lock()
            .unwrap()
            .oauth_pending()
            .unwrap();
        let b_flag = sessions
            .get(b)
            .unwrap()
            .lock()
            .unwrap()
            .oauth_pending()
            .unwrap();
        drop(sessions);

        assert!(
            !Arc::ptr_eq(&a_flag, &b_flag),
            "sessions must not share the flag"
        );
        assert!(!a_flag.load(Ordering::Relaxed));

        // Marking server A mid-OAuth leaves server B's timeout untouched.
        a_flag.store(true, Ordering::Relaxed);
        assert!(!b_flag.load(Ordering::Relaxed));

        close_mcp_session(a);
        close_mcp_session(b);
    }

    #[test]
    fn disabled_flag_defaults_false_and_round_trips_through_config() {
        let servers: BTreeMap<String, McpServer> = serde_json::from_value(json!({
            "plain": { "command": "x" },
            "off": { "command": "y", "disabled": true }
        }))
        .unwrap();
        assert!(!servers["plain"].disabled);
        assert!(servers["off"].disabled);

        // `disabled: false` is not written back out (keeps configs tidy).
        let reserialized = serde_json::to_value(&servers["plain"]).unwrap();
        assert!(reserialized.get("disabled").is_none());
    }

    fn config_with_one_server(name: &str, server: Value) -> MintConfig {
        let mut config = MintConfig::default();
        config
            .extra
            .insert("mcpServers".into(), json!({ name: server }));
        config
    }

    #[test]
    fn set_server_disabled_in_flips_flag_and_reports_existence() {
        let mut config = config_with_one_server("srv", json!({ "command": "x" }));

        assert!(set_server_disabled_in(&mut config, "srv", true).unwrap());
        assert!(configured_mcp_servers(&config).unwrap()["srv"].disabled);
        assert!(set_server_disabled_in(&mut config, "srv", false).unwrap());
        assert!(!configured_mcp_servers(&config).unwrap()["srv"].disabled);

        // Unknown server: reported as missing, config untouched.
        assert!(!set_server_disabled_in(&mut config, "nope", true).unwrap());
    }

    #[test]
    fn update_server_in_edits_only_the_given_fields() {
        let mut config =
            config_with_one_server("srv", json!({ "command": "old", "args": ["--keep"] }));

        let existed = update_server_in(
            &mut config,
            "srv",
            Some("new".into()),
            None,
            None,
            Some(Some("🧪".into())),
        )
        .unwrap();
        assert!(existed);

        let srv = &configured_mcp_servers(&config).unwrap()["srv"];
        assert_eq!(srv.command, "new");
        assert_eq!(srv.args, vec!["--keep".to_string()]); // untouched
        assert_eq!(srv.icon.as_deref(), Some("🧪"));

        assert!(
            !update_server_in(&mut config, "gone", Some("x".into()), None, None, None).unwrap()
        );
    }

    #[test]
    fn mcp_registry_json_is_well_formed() {
        let entries = mcp_registry();
        assert!(!entries.is_empty());

        let mut seen = std::collections::HashSet::new();
        for e in entries {
            assert!(!e.key.is_empty() && !e.name.is_empty(), "{e:?}");
            assert!(!e.command.is_empty(), "{}: empty command", e.key);
            assert!(seen.insert(&e.key), "duplicate registry key: {}", e.key);
        }
        assert!(mcp_registry_entry("filesystem").is_some());
        assert!(mcp_registry_entry("nope").is_none());
    }

    #[test]
    fn expand_registry_entry_appends_extra_args() {
        let entry = mcp_registry_entry("git").expect("git entry present");
        let mut env = BTreeMap::new();
        env.insert("X".into(), "1".into());
        let server = expand_registry_entry(&entry.clone(), &["/repo".to_string()], env);
        assert_eq!(server.command, entry.command);
        assert_eq!(server.args.last().unwrap(), "/repo");
        assert_eq!(server.args.len(), entry.args.len() + 1);
        assert_eq!(server.env["X"], "1");
        assert!(!server.disabled);
    }

    #[test]
    fn allow_and_disallow_tool_in_are_idempotent_and_honor_wildcard() {
        let mut config = MintConfig::default();

        assert!(allow_tool_in(&mut config, "srv", "read"));
        assert!(!allow_tool_in(&mut config, "srv", "read")); // already there
        assert!(allow_tool_in(&mut config, "srv", "write"));
        assert_eq!(mcp_tool_allowlist(&config)["srv"], vec!["read", "write"]);

        // A `*` already present covers any tool.
        let mut wild = MintConfig::default();
        allow_tool_in(&mut wild, "srv", "*");
        assert!(!allow_tool_in(&mut wild, "srv", "anything"));

        // `disallow "*"` clears the server's list.
        assert!(disallow_tool_in(&mut config, "srv", "read"));
        assert!(disallow_tool_in(&mut config, "srv", "*"));
        assert!(mcp_tool_allowlist(&config)["srv"].is_empty());
        assert!(!disallow_tool_in(&mut config, "srv", "read")); // nothing to remove
    }

    #[test]
    fn remote_mcp_server_serialization_and_helpers() {
        let mut headers = BTreeMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer secret-token".to_string(),
        );

        let remote = McpServer {
            command: String::new(),
            args: Vec::new(),
            env: BTreeMap::new(),
            icon: Some("🌐".into()),
            disabled: false,
            url: Some("https://example.com/sse".into()),
            headers: Some(headers.clone()),
            transport: Some("sse".into()),
            timeout_secs: None,
        };

        assert!(remote.is_remote());
        assert_eq!(remote.remote_url(), Some("https://example.com/sse"));

        // Roundtrip JSON serialization
        let serialized = serde_json::to_string(&remote).expect("serializes");
        let deserialized: McpServer = serde_json::from_str(&serialized).expect("deserializes");
        assert!(deserialized.is_remote());
        assert_eq!(deserialized.remote_url(), Some("https://example.com/sse"));
        assert_eq!(
            deserialized
                .headers
                .as_ref()
                .and_then(|h| h.get("Authorization")),
            Some(&"Bearer secret-token".to_string())
        );
        assert_eq!(deserialized.transport.as_deref(), Some("sse"));

        // Stdio server is not remote
        let stdio = McpServer {
            command: "npx".into(),
            args: vec!["-y".into(), "test".into()],
            env: BTreeMap::new(),
            icon: None,
            disabled: false,
            url: None,
            headers: None,
            transport: None,
            timeout_secs: None,
        };
        assert!(!stdio.is_remote());
        assert_eq!(stdio.remote_url(), None);
    }

    #[test]
    fn upsert_remote_mcp_server_adds_to_config() {
        let mut config = MintConfig::default();
        let mut headers = BTreeMap::new();
        headers.insert("X-Api-Key".to_string(), "abc123xyz".to_string());

        upsert_server_in(
            &mut config,
            "remote-docs",
            McpServer {
                command: String::new(),
                args: Vec::new(),
                env: BTreeMap::new(),
                icon: Some("🌐".into()),
                disabled: false,
                url: Some("https://docs.example.com/mcp".into()),
                headers: Some(headers),
                transport: Some("sse".into()),
                timeout_secs: None,
            },
        )
        .expect("upserts remote mcp server");

        let servers = configured_mcp_servers(&config).expect("configured servers");
        let server = servers.get("remote-docs").expect("server exists");
        assert!(server.is_remote());
        assert_eq!(server.remote_url(), Some("https://docs.example.com/mcp"));
        assert_eq!(
            server.headers.as_ref().and_then(|h| h.get("X-Api-Key")),
            Some(&"abc123xyz".to_string())
        );
        assert_eq!(server.transport.as_deref(), Some("sse"));
    }
}

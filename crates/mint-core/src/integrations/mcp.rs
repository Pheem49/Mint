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

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

use crate::{ConfigError, MintConfig, load_config, save_config};

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

/// Live MCP server sessions, keyed by server name. Held as `Arc<Mutex<_>>` per
/// session (rather than one lock guarding the whole map for a call's full
/// duration) so calls to *different* servers don't serialize behind each
/// other — only calls to the *same* server do, which matches the natural
/// constraint of one stdio pipe pair per child process.
static SESSIONS: LazyLock<Mutex<HashMap<String, Arc<Mutex<McpSession>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpServer {
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
    with_session(config, server_name, |session| {
        session.request(
            "tools/call",
            json!({ "name": tool_name, "arguments": arguments }),
        )
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
    let sessions = SESSIONS.lock().unwrap();
    match sessions.get(server_name) {
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
    if let Some(session) = SESSIONS.lock().unwrap().remove(server_name) {
        let _ = session.lock().unwrap().process.kill();
    }
}

/// Closes and removes every server's persistent session.
pub fn close_all_mcp_sessions() {
    let mut sessions = SESSIONS.lock().unwrap();
    for (_, session) in sessions.drain() {
        let _ = session.lock().unwrap().process.kill();
    }
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

fn find_url(line: &str) -> Option<String> {
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
    with_session(config, server_name, |session| {
        session.request("tools/list", json!({}))
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
struct McpSession {
    process: Child,
    stdin: ChildStdin,
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

impl Drop for McpSession {
    fn drop(&mut self) {
        let _ = self.process.kill();
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
fn buffer_notification(queue: &Mutex<VecDeque<Value>>, notification: Value) {
    let mut queue = queue.lock().unwrap();
    while queue.len() >= MAX_BUFFERED_NOTIFICATIONS {
        queue.pop_front();
    }
    queue.push_back(notification);
}

impl McpSession {
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
                            buffer_notification(&reader_notifications, notification);
                        }
                        McpLine::Other => {}
                    }
                }
            }));
            if let Err(payload) = result {
                let message = crate::channels::panic_payload_message(&payload);
                eprintln!("[mint] MCP stdout reader thread panicked: {message}");
            }
        });

        let mut session = McpSession {
            process,
            stdin,
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
        writeln!(self.stdin, "{message}").map_err(McpError::Write)?;
        self.stdin.flush().map_err(McpError::Write)
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

    let mut sessions = SESSIONS.lock().unwrap();

    if server.disabled {
        // Turned off in Settings after a session was already running — kill it
        // so a stale process doesn't linger past the toggle.
        if let Some(session) = sessions.remove(server_name) {
            let _ = session.lock().unwrap().process.kill();
        }
        return Err(McpError::Disabled(server_name.into()));
    }

    if let Some(session) = sessions.get(server_name) {
        if session.lock().unwrap().is_alive() {
            return Ok(Arc::clone(session));
        }
    }
    let session = Arc::new(Mutex::new(McpSession::start(server)?));
    sessions.insert(server_name.to_string(), Arc::clone(&session));
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
            command: "sh".into(),
            args: vec![
                "-c".into(),
                r#"while IFS= read -r line; do
                    id=$(printf '%s' "$line" | grep -o '"id":[0-9]*' | head -1 | cut -d: -f2)
                    if [ -n "$id" ]; then
                        printf '{"jsonrpc":"2.0","id":%s,"result":{"echo":true}}\n' "$id"
                    fi
                done"#
                    .into(),
            ],
            env: BTreeMap::new(),
            icon: None,
            disabled: false,
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
            .process
            .id();

        let second = call_mcp_tool(&config, name, "anything", json!({})).unwrap();
        assert_eq!(second["echo"], true);
        let pid_after_second = SESSIONS
            .lock()
            .unwrap()
            .get(name)
            .unwrap()
            .lock()
            .unwrap()
            .process
            .id();
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
            let _ = session.process.kill();
            let _ = session.process.wait();
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
            .process
            .id();
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
        let a_flag = Arc::clone(&sessions.get(a).unwrap().lock().unwrap().oauth_pending);
        let b_flag = Arc::clone(&sessions.get(b).unwrap().lock().unwrap().oauth_pending);
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
}

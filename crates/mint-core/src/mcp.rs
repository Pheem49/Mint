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
static OAUTH_DETECTED: AtomicBool = AtomicBool::new(false);

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
            icon: None,
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
    /// though nothing consumes them yet (see `drain_mcp_notifications`).
    notifications: Arc<Mutex<VecDeque<Value>>>,
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

        if let Some(stderr) = process.stderr.take() {
            std::thread::spawn(move || {
                let reader = BufReader::new(stderr);
                for line in reader.lines().map_while(Result::ok) {
                    if let Some(url) = find_url(&line) {
                        println!(
                            "\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n",
                            url
                        );
                        OAUTH_DETECTED.store(true, Ordering::Relaxed);
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
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines().map_while(Result::ok) {
                if let Some(url) = find_url(&line) {
                    println!(
                        "\n\x1b[1;33m[MCP Authorization Needed]\x1b[0m Opening browser to authenticate: {}\n",
                        url
                    );
                    OAUTH_DETECTED.store(true, Ordering::Relaxed);
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
                        reader_notifications.lock().unwrap().push_back(notification);
                    }
                    McpLine::Other => {}
                }
            }
        });

        let mut session = McpSession {
            process,
            stdin,
            next_id: AtomicU64::new(2), // 1 is reserved for `initialize` below.
            pending,
            notifications,
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

        let timeout = if OAUTH_DETECTED.load(Ordering::Relaxed) {
            Duration::from_secs(120)
        } else {
            MCP_TIMEOUT
        };
        let response = receiver.recv_timeout(timeout).map_err(|_| {
            self.pending.lock().unwrap().remove(&id);
            McpError::Timeout
        })?;
        OAUTH_DETECTED.store(false, Ordering::Relaxed);
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
    let mut sessions = SESSIONS.lock().unwrap();
    if let Some(session) = sessions.get(server_name) {
        if session.lock().unwrap().is_alive() {
            return Ok(Arc::clone(session));
        }
    }
    let servers = configured_mcp_servers(config)?;
    let server = servers
        .get(server_name)
        .ok_or_else(|| McpError::MissingServer(server_name.into()))?;
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
}

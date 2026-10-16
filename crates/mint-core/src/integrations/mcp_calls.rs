//! Cancellable agent requests. Session lifecycle remains shared with the raw MCP API.
use super::*;
use std::time::Instant;

type Pending = Arc<Mutex<HashMap<u64, mpsc::Sender<Value>>>>;
type ProgressSender = tokio::sync::mpsc::Sender<Value>;
pub(super) static PROGRESS_SUBSCRIBERS: LazyLock<Mutex<HashMap<String, ProgressSender>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));
type ActiveCalls = HashMap<String, (String, Arc<AtomicBool>)>;
static ACTIVE_CALLS: LazyLock<Mutex<ActiveCalls>> = LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpProgress {
    pub progress: f64,
    pub total: Option<f64>,
    pub message: Option<String>,
    pub elapsed_secs: u64,
}

pub fn cancel_mcp_calls(chat_id: &str) -> usize {
    let calls = ACTIVE_CALLS.lock().unwrap();
    let prefix = format!("{chat_id}::subagent::");
    let mut count = 0;
    for (chat, token) in calls.values() {
        if chat == chat_id || chat.starts_with(&prefix) {
            token.store(true, Ordering::Release);
            count += 1;
        }
    }
    count
}

#[derive(Clone)]
enum Writer {
    Stdio(Arc<Mutex<ChildStdin>>),
    Remote {
        client: reqwest::Client,
        endpoint: String,
    },
}
impl Writer {
    async fn send(&self, payload: Value) -> Result<Option<Value>, McpError> {
        self.send_checked(payload, None).await
    }
    async fn send_checked(
        &self,
        payload: Value,
        check: Option<DispatchGuard>,
    ) -> Result<Option<Value>, McpError> {
        match self {
            Self::Stdio(stdin) => {
                let stdin = Arc::clone(stdin);
                tokio::task::spawn_blocking(move || {
                    let mut stdin = stdin.lock().unwrap();
                    if let Some(check) = check {
                        check.check()?;
                    }
                    writeln!(stdin, "{payload}").map_err(McpError::Write)?;
                    stdin.flush().map_err(McpError::Write)?;
                    Ok(None)
                })
                .await
                .map_err(|e| McpError::Remote(e.to_string()))?
            }
            Self::Remote { client, endpoint } => {
                if let Some(check) = check {
                    check.check()?;
                }
                let response = client
                    .post(endpoint)
                    .json(&payload)
                    .send()
                    .await
                    .map_err(|e| McpError::Remote(e.to_string()))?;
                if !response.status().is_success() {
                    return Err(McpError::Remote(format!("HTTP {}", response.status())));
                }
                let body = response
                    .text()
                    .await
                    .map_err(|e| McpError::Remote(e.to_string()))?;
                if body.trim().is_empty() {
                    Ok(None)
                } else {
                    serde_json::from_str(&body)
                        .map(Some)
                        .map_err(|e| McpError::Remote(format!("Invalid MCP JSON response: {e}")))
                }
            }
        }
    }
}

struct DispatchGuard {
    cancel: Arc<AtomicBool>,
    turn: Option<tokio_util::sync::CancellationToken>,
    binding: SessionBinding,
}
impl DispatchGuard {
    fn check(&self) -> Result<(), McpError> {
        if self.cancel.load(Ordering::Acquire)
            || self.turn.as_ref().is_some_and(|t| t.is_cancelled())
        {
            return Err(McpError::Cancelled);
        }
        self.binding.check_fast()
    }
}

struct RequestGuard {
    token: String,
    cancel: Arc<AtomicBool>,
    turn: Option<tokio_util::sync::CancellationToken>,
    request: Option<(u64, Pending, Writer)>,
}
impl RequestGuard {
    fn new(chat: &str) -> Self {
        let token = uuid::Uuid::new_v4().to_string();
        let cancel = Arc::new(AtomicBool::new(false));
        ACTIVE_CALLS
            .lock()
            .unwrap()
            .insert(token.clone(), (chat.into(), Arc::clone(&cancel)));
        Self {
            token,
            cancel,
            turn: crate::orchestration::run_control::token(),
            request: None,
        }
    }
    async fn cancel_remote(&mut self, reason: &str) -> bool {
        if let Some((id, pending, writer)) = self.request.take() {
            pending.lock().unwrap().remove(&id);
            tokio::time::timeout(Duration::from_secs(1),writer.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id,"reason":reason}}))).await.is_ok_and(|r|r.is_ok())
        } else {
            true
        }
    }
}
impl Drop for RequestGuard {
    fn drop(&mut self) {
        ACTIVE_CALLS.lock().unwrap().remove(&self.token);
        PROGRESS_SUBSCRIBERS.lock().unwrap().remove(&self.token);
        if let Some((id, pending, writer)) = self.request.take() {
            pending.lock().unwrap().remove(&id);
            MCP_ASYNC_RUNTIME.spawn(async move {
                let _=tokio::time::timeout(Duration::from_secs(1),writer.send(json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":id,"reason":"Client stopped waiting"}}))).await;
            });
        }
    }
}

/// Owns cancellation across discovery and the gaps between device status reads.
pub(crate) struct McpScope(RequestGuard);
impl McpScope {
    pub(crate) fn new(chat: &str) -> Self {
        Self(RequestGuard::new(chat))
    }
    pub(crate) async fn run<T>(
        &self,
        work: impl std::future::Future<Output = Result<T, McpError>>,
        timeout: Duration,
    ) -> Result<T, McpError> {
        if self.0.cancel.load(Ordering::Acquire)
            || self.0.turn.as_ref().is_some_and(|t| t.is_cancelled())
        {
            return Err(McpError::Cancelled);
        }
        tokio::select! {
            biased;
            _=async {while !self.0.cancel.load(Ordering::Acquire) && !self.0.turn.as_ref().is_some_and(|t|t.is_cancelled()) {tokio::time::sleep(Duration::from_millis(20)).await;}}=>Err(McpError::Cancelled),
            result=tokio::time::timeout(timeout,work)=>result.unwrap_or(Err(McpError::Timeout)),
        }
    }
}

/// Progress is optional; a deadline applies even when notifications keep arriving.
pub async fn call_mcp_tool_async(
    config: &MintConfig,
    server: &str,
    tool: &str,
    arguments: Value,
    chat: &str,
    progress: impl FnMut(McpProgress) + Send,
) -> Result<Value, McpError> {
    call_mcp_tool_reviewed(config, server, tool, arguments, chat, None, progress).await
}

pub(crate) async fn call_mcp_tool_reviewed(
    config: &MintConfig,
    server: &str,
    tool: &str,
    arguments: Value,
    chat: &str,
    expected: Option<&str>,
    progress: impl FnMut(McpProgress) + Send,
) -> Result<Value, McpError> {
    let timeout = Duration::from_secs(super::catalog::tool_timeout(config, server)?);
    McpScope::new(chat)
        .run(
            async {
                if !mcp_tool_allowed(config, server, tool) {
                    return Err(McpError::NotAllowed {
                        server: server.into(),
                        tool: tool.into(),
                    });
                }
                let bound = super::catalog::resolve_tool(config, server, tool, chat, None).await?;
                let definition = &bound.definition;
                if expected.is_some_and(|fp| fp != definition.fingerprint) {
                    return Err(McpError::Preflight(
                        "Tool definition changed; read it again before executing".into(),
                    ));
                }
                call_bound_tool(config, server, tool, arguments, chat, &bound, progress).await
            },
            timeout,
        )
        .await
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn call_bound_tool(
    config: &MintConfig,
    server: &str,
    tool: &str,
    arguments: Value,
    chat: &str,
    bound: &catalog::BoundTool,
    progress: impl FnMut(McpProgress) + Send,
) -> Result<Value, McpError> {
    let timeout = Duration::from_secs(catalog::tool_timeout(config, server)?);
    McpScope::new(chat)
        .run(
            async {
                if !mcp_tool_allowed(config, server, tool) {
                    return Err(McpError::NotAllowed {
                        server: server.into(),
                        tool: tool.into(),
                    });
                }
                let current =
                    catalog::resolve_tool(config, server, tool, chat, Some(&bound.binding)).await?;
                if current.definition.fingerprint != bound.definition.fingerprint {
                    return Err(McpError::Preflight(
                        "Definition changed on the pinned session; read it again before executing"
                            .into(),
                    ));
                }
                current.definition.validate(&arguments)?;
                request_on_session(
                    &current.binding,
                    "tools/call",
                    json!({"name":tool,"arguments":arguments}),
                    chat,
                    timeout,
                    progress,
                )
                .await
            },
            timeout,
        )
        .await
}

pub(crate) async fn request_on_session(
    binding: &SessionBinding,
    method: &str,
    mut params: Value,
    chat: &str,
    timeout: Duration,
    mut progress: impl FnMut(McpProgress) + Send,
) -> Result<Value, McpError> {
    let started = Instant::now();
    let mut guard = RequestGuard::new(chat);
    let cancelled = Arc::clone(&guard.cancel);
    let token = guard.token.clone();
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel(16);
    PROGRESS_SUBSCRIBERS
        .lock()
        .unwrap()
        .insert(token.clone(), progress_tx);
    let pinned = binding.clone();
    let turn_cancel = guard.turn.clone();
    let work = async {
        // Never hold the session mutex while awaiting an answer or sending cancellation.
        let (id, pending, writer, connected) = tokio::task::spawn_blocking(move || {
            pinned.check()?;
            let session = pinned.session.lock().unwrap();
            Ok::<_, McpError>(match &session.backend {
                McpSessionBackend::Stdio(s) => (
                    s.next_id.fetch_add(1, Ordering::Relaxed),
                    Arc::clone(&s.pending),
                    Writer::Stdio(Arc::clone(&s.stdin)),
                    Arc::clone(&s.connected),
                ),
                McpSessionBackend::Remote(s) => (
                    s.next_id.fetch_add(1, Ordering::Relaxed),
                    Arc::clone(&s.pending),
                    Writer::Remote {
                        client: s.client.clone(),
                        endpoint: s.post_endpoint.lock().unwrap().clone(),
                    },
                    Arc::clone(&s.is_active),
                ),
            })
        })
        .await
        .map_err(|e| McpError::Remote(e.to_string()))??;
        let (tx, rx) = mpsc::channel();
        pending.lock().unwrap().insert(id, tx);
        guard.request = Some((id, Arc::clone(&pending), writer.clone()));
        params["_meta"] = json!({"progressToken":token});
        let payload = json!({"jsonrpc":"2.0","id":id,"method":method,"params":params});
        // Send concurrently with progress consumption: a remote POST may take minutes.
        let send = writer.send_checked(
            payload,
            Some(DispatchGuard {
                cancel: Arc::clone(&cancelled),
                turn: turn_cancel.clone(),
                binding: binding.clone(),
            }),
        );
        tokio::pin!(send);
        let mut sent = false;
        let mut last_emit = Instant::now() - Duration::from_secs(1);
        let mut last_value = None;
        let mut buffered = None;
        loop {
            if let Ok(response) = rx.try_recv() {
                break response_result(response, id);
            }
            if !connected.load(Ordering::Acquire) {
                break Err(McpError::Remote(
                    "MCP session disconnected; remote completion is unconfirmed".into(),
                ));
            }
            tokio::select! {
                res=&mut send, if !sent => {
                    sent=true;
                    if let Some(response)=res? {break response_result(response,id);}
                }
                Some(notification)=progress_rx.recv() => {
                    let p=&notification["params"];
                    if let Some(value)=p["progress"].as_f64().filter(|v|v.is_finite() && *v>=0.0)
                        && last_value.is_none_or(|last|value>last) {
                        last_value=Some(value);
                        buffered=Some(McpProgress {progress:value,total:p["total"].as_f64().filter(|t|t.is_finite() && *t>0.0 && *t>=value),message:p["message"].as_str().map(str::to_owned),elapsed_secs:started.elapsed().as_secs()});
                    }
                }
                _=tokio::time::sleep(Duration::from_millis(20)) => {}
            }
            if last_emit.elapsed() >= Duration::from_millis(250)
                && let Some(update) = buffered.take()
            {
                progress(update);
                last_emit = Instant::now();
            }
        }
    };
    // Poll cancellation outside work so it also interrupts connect, POST, and body reads.
    let result = {
        tokio::pin!(work);
        tokio::select! {
            biased;
            _=async {while !cancelled.load(Ordering::Acquire) && !turn_cancel.as_ref().is_some_and(|t|t.is_cancelled()) {tokio::time::sleep(Duration::from_millis(20)).await;}} => Err(McpError::Cancelled),
            result=tokio::time::timeout(timeout,&mut work) => result.unwrap_or(Err(McpError::Timeout)),
        }
    };
    if matches!(result, Err(McpError::Cancelled) | Err(McpError::Timeout)) {
        let sent = guard
            .cancel_remote("Client cancelled or deadline expired")
            .await;
        if !sent {
            return Err(if matches!(result, Err(McpError::Timeout)) {
                McpError::TimeoutUndelivered
            } else {
                McpError::CancelledUndelivered
            });
        }
    } else {
        if let Some((id, pending, _)) = guard.request.take() {
            pending.lock().unwrap().remove(&id);
        }
    }
    result
}

fn response_result(response: Value, id: u64) -> Result<Value, McpError> {
    if response.get("id").and_then(Value::as_u64) != Some(id) {
        return Err(McpError::Remote("MCP response request ID mismatch".into()));
    }
    if let Some(error) = response.get("error") {
        return Err(McpError::Tool(error.clone()));
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| McpError::Remote("MCP response missing result".into()))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    fn server(script: &str, timeout: u64) -> (MintConfig, String) {
        let name = format!("mcp-call-{}", uuid::Uuid::new_v4());
        let mut cfg = MintConfig::default();
        cfg.extra.insert("mcpServers".into(),json!({name.clone():{"command":"python3","args":["-u","-c",script],"timeoutSecs":timeout}}));
        cfg.extra
            .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
        (cfg, name)
    }
    const HOLD: &str = r#"
import sys,json,time,threading
def emit(x): print(json.dumps(x),flush=True)
def heartbeat(token):
 for n in range(1,40):
  emit({'jsonrpc':'2.0','method':'notifications/progress','params':{'progressToken':token,'progress':n,'total':40,'message':'rendering'}})
  time.sleep(.1)
for line in sys.stdin:
 r=json.loads(line); method=r.get('method')
 if method=='initialize': emit({'jsonrpc':'2.0','id':r['id'],'result':{}})
 elif method=='tools/list': emit({'jsonrpc':'2.0','id':r['id'],'result':{'tools':[{'name':n,'inputSchema':{'type':'object'}} for n in ['hold','ping','render','wrong-id']]}})
 elif method=='tools/call':
  if r['params']['name']=='hold': threading.Thread(target=heartbeat,args=(r['params']['_meta']['progressToken'],),daemon=True).start()
  else: emit({'jsonrpc':'2.0','id':r['id'],'result':{'content':[{'type':'text','text':'pong'}]}})
"#;
    #[tokio::test]
    async fn cancellation_is_scoped_and_preserves_the_shared_session() {
        let (cfg, name) = server(HOLD, 5);
        let chat = uuid::Uuid::new_v4().to_string();
        let cfg2 = cfg.clone();
        let name2 = name.clone();
        let child = format!("{chat}::subagent::one");
        let (tx, rx) = tokio::sync::oneshot::channel();
        let held = tokio::spawn(async move {
            let mut tx = Some(tx);
            call_mcp_tool_async(&cfg2, &name2, "hold", json!({}), &child, move |_| {
                if let Some(tx) = tx.take() {
                    let _ = tx.send(());
                }
            })
            .await
        });
        tokio::time::timeout(Duration::from_secs(2), rx)
            .await
            .unwrap()
            .unwrap();
        let pong = call_mcp_tool_async(&cfg, &name, "ping", json!({}), "other-chat", |_| {})
            .await
            .unwrap();
        assert_eq!(pong["content"][0]["text"], "pong");
        assert!(cancel_mcp_calls(&chat) > 0);
        assert!(matches!(held.await.unwrap(), Err(McpError::Cancelled)));
        assert_eq!(cancel_mcp_calls(&chat), 0);
        assert_eq!(
            call_mcp_tool_async(&cfg, &name, "ping", json!({}), "other-chat", |_| {})
                .await
                .unwrap()["content"][0]["text"],
            "pong"
        );
        close_mcp_session(&name);
    }
    #[tokio::test]
    async fn progress_does_not_extend_deadline_and_pending_calls_are_removed() {
        let (cfg, name) = server(HOLD, 1);
        let chat = uuid::Uuid::new_v4().to_string();
        let started = Instant::now();
        let mut updates = Vec::new();
        assert!(matches!(
            call_mcp_tool_async(&cfg, &name, "hold", json!({}), &chat, |p| updates.push(p)).await,
            Err(McpError::Timeout)
        ));
        assert!(started.elapsed() < Duration::from_secs(3));
        assert!(!updates.is_empty());
        assert!(updates.len() <= 5);
        assert_eq!(cancel_mcp_calls(&chat), 0);
        let session = get_or_start_session(&cfg, &name).unwrap();
        let session = session.lock().unwrap();
        if let McpSessionBackend::Stdio(s) = &session.backend {
            assert!(s.pending.lock().unwrap().is_empty());
        }
        drop(session);
        close_mcp_session(&name);
    }
    #[tokio::test]
    async fn tools_can_run_longer_than_thirty_seconds_without_progress() {
        let (cfg, name) = server(
            r#"
import sys,json,time
for line in sys.stdin:
 r=json.loads(line)
 if r.get('method')=='initialize': print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{}}),flush=True)
 elif r.get('method')=='tools/list': print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{'tools':[{'name':n,'inputSchema':{'type':'object'}} for n in ['hold','ping','render','wrong-id']]}}),flush=True)
 elif r.get('method')=='tools/call':
  time.sleep(31)
  print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{'content':[{'type':'text','text':'rendered'}]}}),flush=True)
"#,
            40,
        );
        let result = call_mcp_tool_async(&cfg, &name, "render", json!({}), "long-test", |_| {})
            .await
            .unwrap();
        assert_eq!(result["content"][0]["text"], "rendered");
        close_mcp_session(&name);
    }
    #[tokio::test]
    async fn disconnected_stdio_is_reported_without_waiting_for_the_deadline() {
        let (cfg, name) = server(
            r#"
import sys,json
r=json.loads(sys.stdin.readline());print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{}}),flush=True)
for line in sys.stdin:
 r=json.loads(line)
 if r.get('method')=='tools/list': print(json.dumps({'jsonrpc':'2.0','id':r['id'],'result':{'tools':[{'name':n,'inputSchema':{'type':'object'}} for n in ['hold','ping','render','wrong-id']]}}),flush=True)
 elif r.get('method')=='tools/call': break
"#,
            1,
        );
        let result =
            call_mcp_tool_async(&cfg, &name, "render", json!({}), "disconnect-test", |_| {}).await;
        assert!(
            matches!(result,Err(McpError::Remote(ref reason)) if reason.contains("disconnected")),
            "{result:?}"
        );
        close_mcp_session(&name);
    }

    #[tokio::test]
    async fn slow_server_initialization_does_not_block_another_servers_deadline() {
        let marker = std::env::temp_dir().join(format!("mint-mcp-start-{}", uuid::Uuid::new_v4()));
        let script = format!(
            r#"
import sys,json,time
for line in sys.stdin:
 r=json.loads(line)
 if r.get('method')=='initialize':
  open({:?},'w').close()
  time.sleep(2)
  print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':{{}}}}),flush=True)
 elif r.get('method')=='tools/list': print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':{{'tools':[{{'name':n,'inputSchema':{{'type':'object'}}}} for n in ['ping']]}}}}),flush=True)
 elif r.get('method')=='tools/call': print(json.dumps({{'jsonrpc':'2.0','id':r['id'],'result':{{'content':[{{'type':'text','text':'slow done'}}]}}}}),flush=True)
"#,
            marker.to_string_lossy()
        );
        let (slow, slow_name) = server(&script, 5);
        let name2 = slow_name.clone();
        let task = tokio::spawn(async move {
            call_mcp_tool_async(&slow, &name2, "ping", json!({}), "slow-init", |_| {}).await
        });
        tokio::time::timeout(Duration::from_secs(3), async {
            while !marker.exists() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let (fast, fast_name) = server(HOLD, 1);
        let result =
            call_mcp_tool_async(&fast, &fast_name, "ping", json!({}), "fast-init", |_| {}).await;
        task.await.unwrap().unwrap();
        close_mcp_session(&slow_name);
        close_mcp_session(&fast_name);
        std::fs::remove_file(marker).unwrap();
        assert_eq!(result.unwrap()["content"][0]["text"], "pong");
    }

    async fn remote(
        sse: bool,
    ) -> (
        MintConfig,
        String,
        Arc<Mutex<Vec<Value>>>,
        tokio::task::JoinHandle<()>,
    ) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let requests = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&requests);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<Value>();
        let stream_rx = Arc::new(tokio::sync::Mutex::new(Some(rx)));
        let task = tokio::spawn(async move {
            loop {
                let (mut socket, _) = listener.accept().await.unwrap();
                let recorded = Arc::clone(&recorded);
                let tx = tx.clone();
                let rx = Arc::clone(&stream_rx);
                tokio::spawn(async move {
                    let mut bytes = Vec::new();
                    let mut chunk = [0u8; 4096];
                    let (header_end, length) = loop {
                        let n = socket.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&chunk[..n]);
                        if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                            let header = String::from_utf8_lossy(&bytes[..end]);
                            let length = header
                                .lines()
                                .find_map(|l| {
                                    l.to_ascii_lowercase()
                                        .strip_prefix("content-length:")
                                        .and_then(|v| v.trim().parse::<usize>().ok())
                                })
                                .unwrap_or(0);
                            break (end + 4, length);
                        }
                    };
                    if bytes.starts_with(b"GET ") {
                        if !sse {
                            let _=socket.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                            return;
                        }
                        let _=socket.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\nevent: endpoint\ndata: /rpc\n\n").await;
                        let Some(mut rx) = rx.lock().await.take() else {
                            return;
                        };
                        while let Some(message) = rx.recv().await {
                            if socket
                                .write_all(
                                    format!("event: message\ndata: {message}\n\n").as_bytes(),
                                )
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        return;
                    }
                    while bytes.len() < header_end + length {
                        let n = socket.read(&mut chunk).await.unwrap();
                        if n == 0 {
                            return;
                        }
                        bytes.extend_from_slice(&chunk[..n]);
                    }
                    let request: Value =
                        serde_json::from_slice(&bytes[header_end..header_end + length]).unwrap();
                    recorded.lock().unwrap().push(request.clone());
                    let method = request["method"].as_str().unwrap_or_default();
                    let mut result = json!({});
                    if method == "tools/list" {
                        result = json!({"tools":(["hold","render","wrong-id"].map(|name|json!({"name":name,"inputSchema":{"type":"object"}})))});
                    }
                    if method == "tools/call" {
                        match request["params"]["name"].as_str().unwrap() {
                            "hold" => {
                                tokio::time::sleep(Duration::from_secs(3)).await;
                            }
                            "render" => {
                                result = json!({"content":[{"type":"text","text":"rendered"}]});
                            }
                            _ => {}
                        }
                    }
                    if method == "tools/call" && sse {
                        let _=socket.write_all(b"HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await;
                        let _=tx.send(json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":request["params"]["_meta"]["progressToken"],"progress":1,"total":2,"message":"Rendering"}}));
                        tokio::time::sleep(Duration::from_millis(100)).await;
                        let _ =
                            tx.send(json!({"jsonrpc":"2.0","id":request["id"],"result":result}));
                    } else {
                        let body = if request.get("id").is_some() {
                            json!({"jsonrpc":"2.0","id":if request["params"]["name"]=="wrong-id" {json!(99999)} else {request["id"].clone()},"result":result}).to_string()
                        } else {
                            String::new()
                        };
                        let response = format!(
                            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                            body.len()
                        );
                        let _ = socket.write_all(response.as_bytes()).await;
                    }
                });
            }
        });
        let name = format!("remote-{}", uuid::Uuid::new_v4());
        let mut cfg = MintConfig::default();
        cfg.extra.insert(
            "mcpServers".into(),
            json!({name.clone():{"url":format!("http://{addr}/sse"),"timeoutSecs":1}}),
        );
        cfg.extra
            .insert("allowedMcpTools".into(), json!({name.clone():["*"]}));
        (cfg, name, requests, task)
    }
    #[tokio::test]
    async fn remote_sse_progress_and_response_are_routed_to_the_call() {
        let (cfg, name, requests, task) = remote(true).await;
        let mut updates = Vec::new();
        let result =
            call_mcp_tool_async(&cfg, &name, "render", json!({}), "remote-progress", |p| {
                updates.push(p)
            })
            .await
            .unwrap();
        assert_eq!(result["content"][0]["text"], "rendered");
        assert_eq!(updates[0].message.as_deref(), Some("Rendering"));
        assert!(
            requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "tools/call"
                    && r.pointer("/params/_meta/progressToken").is_some())
        );
        close_mcp_session(&name);
        task.abort();
    }
    #[tokio::test]
    async fn remote_post_timeout_covers_body_wait_and_delivers_cancellation() {
        let (cfg, name, requests, task) = remote(false).await;
        let started = Instant::now();
        let result =
            call_mcp_tool_async(&cfg, &name, "hold", json!({}), "remote-timeout", |_| {}).await;
        assert!(matches!(result, Err(McpError::Timeout)), "{result:?}");
        assert!(started.elapsed() < Duration::from_secs(2));
        tokio::time::timeout(Duration::from_secs(1), async {
            while !requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "notifications/cancelled")
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        let requests = requests.lock().unwrap();
        let call = requests
            .iter()
            .find(|r| r["method"] == "tools/call")
            .unwrap();
        let cancel = requests
            .iter()
            .find(|r| r["method"] == "notifications/cancelled")
            .unwrap();
        assert_eq!(cancel["params"]["requestId"], call["id"]);
        drop(requests);
        close_mcp_session(&name);
        task.abort();
    }
    #[tokio::test]
    async fn dropping_a_call_sends_cancellation_and_keeps_the_session_usable() {
        let (cfg, name, requests, task) = remote(false).await;
        let chat = uuid::Uuid::new_v4().to_string();
        let cfg2 = cfg.clone();
        let name2 = name.clone();
        let chat2 = chat.clone();
        let call = tokio::spawn(async move {
            call_mcp_tool_async(&cfg2, &name2, "hold", json!({}), &chat2, |_| {}).await
        });
        tokio::time::timeout(Duration::from_secs(2), async {
            while !requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "tools/call")
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        call.abort();
        assert!(call.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(2), async {
            while !requests
                .lock()
                .unwrap()
                .iter()
                .any(|r| r["method"] == "notifications/cancelled")
            {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap();
        assert_eq!(cancel_mcp_calls(&chat), 0);
        {
            let recorded = requests.lock().unwrap();
            let id = recorded
                .iter()
                .find(|r| r["method"] == "tools/call")
                .unwrap()["id"]
                .clone();
            assert_eq!(
                recorded
                    .iter()
                    .find(|r| r["method"] == "notifications/cancelled")
                    .unwrap()["params"]["requestId"],
                id
            );
        }
        let result = call_mcp_tool_async(&cfg, &name, "render", json!({}), "new-call", |_| {})
            .await
            .unwrap();
        assert_eq!(result["content"][0]["text"], "rendered");
        let session = get_or_start_session(&cfg, &name).unwrap();
        if let McpSessionBackend::Remote(remote) = &session.lock().unwrap().backend {
            assert!(remote.pending.lock().unwrap().is_empty());
        }
        close_mcp_session(&name);
        task.abort();
    }
    #[tokio::test]
    async fn remote_json_response_requires_the_exact_request_id() {
        let (cfg, name, _, task) = remote(false).await;
        let result =
            call_mcp_tool_async(&cfg, &name, "wrong-id", json!({}), "remote-id", |_| {}).await;
        assert!(
            matches!(result,Err(McpError::Remote(ref reason)) if reason.contains("ID mismatch")),
            "{result:?}"
        );
        close_mcp_session(&name);
        task.abort();
    }
}

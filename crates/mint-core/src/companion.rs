//! Local, optional companion transport. Never boots an API server or integrations.
use futures_util::{SinkExt, StreamExt};
use mint_companion_protocol::{
    Command, Event, HostDescriptor, Session, Status, Turn, TurnBook, VERSION,
};
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
};
use tokio::{net::TcpStream, sync::broadcast};
use tokio_tungstenite::tungstenite::{
    Message,
    handshake::server::{ErrorResponse, Request, Response},
};

struct Hub {
    host_id: String,
    book: Mutex<TurnBook>,
    memory: Mutex<Option<crate::MemoryStore>>,
    requests: Mutex<HashMap<String, Option<i64>>>,
    sessions: Mutex<Vec<Session>>,
    observed: Mutex<std::collections::HashSet<(String, i64)>>,
    seq: AtomicU64,
    events: broadcast::Sender<Event>,
}
impl Default for Hub {
    fn default() -> Self {
        Self {
            host_id: uuid::Uuid::new_v4().to_string(),
            book: Mutex::new(TurnBook::default()),
            memory: Mutex::new(None),
            requests: Mutex::new(HashMap::new()),
            sessions: Mutex::new(Vec::new()),
            observed: Mutex::new(std::collections::HashSet::new()),
            seq: AtomicU64::new(0),
            events: broadcast::channel(128).0,
        }
    }
}
impl Hub {
    fn update(
        &self,
        chat: &str,
        id: i64,
        status: Status,
        tool: Option<String>,
        text: Option<String>,
    ) {
        self.change(chat, id, status, tool, text, false);
    }
    fn change(
        &self,
        chat: &str,
        id: i64,
        status: Status,
        tool: Option<String>,
        text: Option<String>,
        append: bool,
    ) {
        // Sequence allocation and state publication share the lock: concurrent tools
        // cannot publish an older snapshot over a newer one.
        let mut book = self.book.lock().unwrap();
        let previous = book.0.get(&(chat.to_owned(), id));
        if previous.is_some_and(|turn| terminal(&turn.status)) {
            return;
        }
        let old_text = previous.map(|t| t.text.clone()).unwrap_or_default();
        let prompt = previous.map(|t| t.prompt.clone()).unwrap_or_default();
        let text = if append {
            old_text + text.as_deref().unwrap_or("")
        } else {
            text.unwrap_or(old_text)
        };
        let turn = Turn {
            chat_id: chat.into(),
            turn_id: id,
            seq: self.seq.fetch_add(1, Ordering::Relaxed) + 1,
            status,
            tool,
            text,
            prompt,
        };
        book.apply(turn.clone());
        self.observed.lock().unwrap().insert((chat.into(), id));
        // Keep live turns; completed history is also available in the existing DB.
        if book.0.len() > 256 {
            let oldest = book
                .0
                .iter()
                .filter(|(_, t)| terminal(&t.status))
                .min_by_key(|(_, t)| t.seq)
                .map(|(k, _)| k.clone());
            if let Some(key) = oldest {
                book.0.remove(&key);
                self.observed.lock().unwrap().remove(&key);
            }
        }
        let _ = self.events.send(Event::Update {
            host_id: self.host_id.clone(),
            turn,
        });
    }
    fn chunk(&self, chat: &str, id: i64, chunk: &str) {
        self.change(
            chat,
            id,
            Status::Responding,
            None,
            Some(chunk.to_owned()),
            true,
        );
    }
    fn begin(&self, chat: &str, id: i64, prompt: &str) {
        self.book.lock().unwrap().0.insert(
            (chat.into(), id),
            Turn {
                chat_id: chat.into(),
                turn_id: id,
                seq: 0,
                status: Status::Queued,
                tool: None,
                text: String::new(),
                prompt: prompt.into(),
            },
        );
        self.update(chat, id, Status::Queued, None, None);
    }
    fn snapshot(&self, selected: Option<&str>) -> Event {
        let turns = selected
            .map(|id| self.book.lock().unwrap().session(id))
            .unwrap_or_default();
        Event::Snapshot {
            host_id: self.host_id.clone(),
            version: VERSION,
            sessions: self.sessions.lock().unwrap().clone(),
            turns,
        }
    }
    fn refresh_sessions(&self) {
        let mut stored = self.memory.lock().unwrap();
        if stored.is_none() {
            *stored = crate::MemoryStore::open_default().ok();
        }
        if let Some(memory) = stored.as_ref()
            && let Ok(rows) = memory.list_chat_sessions()
        {
            *self.sessions.lock().unwrap() = rows
                .into_iter()
                .filter(|s| !s.id.contains("::subagent::"))
                .map(|s| Session {
                    chat_id: s.id,
                    title: s.title,
                    workspace: s.workspace_path,
                })
                .collect();
        }
    }
    fn restore_history(&self, chat: &str) {
        let memory = self.memory.lock().unwrap().clone();
        if let Some(memory) = memory
            && let Ok(rows) = memory.recent_interactions_for_chat(chat, 20)
        {
            let mut book = self.book.lock().unwrap();
            let observed = self.observed.lock().unwrap();
            for row in rows {
                let key = (chat.to_owned(), row.id);
                if observed.contains(&key) {
                    continue;
                }
                let status = match row.status.as_str() {
                    "queued" => Status::Queued,
                    "running" => Status::Thinking,
                    "failed" => Status::Failed,
                    "interrupted" => Status::Interrupted,
                    _ => Status::Completed,
                };
                if book
                    .0
                    .get(&key)
                    .is_some_and(|t| t.status == status && t.text == row.ai_text)
                {
                    continue;
                }
                book.apply(Turn {
                    chat_id: chat.into(),
                    turn_id: row.id,
                    seq: self.seq.fetch_add(1, Ordering::Relaxed) + 1,
                    status,
                    tool: None,
                    text: row.ai_text,
                    prompt: row.user_text,
                });
            }
        }
    }
}
fn connection_event(
    event: Event,
    selected: Option<&str>,
    requests: &std::collections::HashSet<String>,
) -> Option<Event> {
    match &event {
        Event::Update { turn, .. } if selected == Some(&turn.chat_id) => Some(event),
        Event::Error {
            request_id: Some(id),
            ..
        } if requests.contains(id) => Some(event),
        _ => None,
    }
}

fn terminal(status: &Status) -> bool {
    matches!(
        status,
        Status::Completed | Status::Failed | Status::Interrupted
    )
}
static HUB: OnceLock<Arc<Hub>> = OnceLock::new();
static STARTED: AtomicBool = AtomicBool::new(false);
fn hub() -> &'static Arc<Hub> {
    HUB.get_or_init(|| Arc::new(Hub::default()))
}

/// Start at most one optional listener per running Mint process.
pub fn start() {
    let Ok(runtime) = tokio::runtime::Handle::try_current() else {
        return;
    };
    if STARTED.swap(true, Ordering::AcqRel) {
        return;
    }
    let hub = Arc::clone(hub());
    runtime.spawn(async move {
        if let Err(error) = listen(hub).await {
            eprintln!("[companion] local connection unavailable: {error}");
            STARTED.store(false, Ordering::Release);
        }
    });
}
struct DescriptorFile(std::path::PathBuf);
#[derive(Default)]
struct DescriptorLifecycle {
    path: Option<std::path::PathBuf>,
    stopping: bool,
}
impl DescriptorLifecycle {
    fn shutdown(&mut self) {
        self.stopping = true;
        if let Some(path) = self.path.take() {
            let _ = std::fs::remove_file(path);
        }
    }
    fn can_publish(&self) -> bool {
        !self.stopping
    }
}
static DESCRIPTOR: Mutex<DescriptorLifecycle> = Mutex::new(DescriptorLifecycle {
    path: None,
    stopping: false,
});

/// Tauri exits the process directly, so its host cannot rely on runtime Drop.
pub fn shutdown() {
    if let Ok(mut lifecycle) = DESCRIPTOR.lock() {
        lifecycle.shutdown();
    }
}
impl Drop for DescriptorFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
        if let Ok(mut lifecycle) = DESCRIPTOR.lock()
            && lifecycle.path.as_ref() == Some(&self.0)
        {
            lifecycle.path = None;
        }
    }
}
async fn listen(hub: Arc<Hub>) -> std::io::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let Some(dir) = mint_companion_protocol::hosts_dir() else {
        return Err(std::io::Error::other("user config directory unavailable"));
    };
    std::fs::create_dir_all(&dir)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))?;
    }
    let token = uuid::Uuid::new_v4().simple().to_string();
    let name = std::env::current_exe()
        .ok()
        .and_then(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
        .unwrap_or_else(|| "Mint".into());
    let descriptor = HostDescriptor {
        version: VERSION,
        host_id: hub.host_id.clone(),
        name: format!("{name} · {}", std::process::id()),
        endpoint: format!("ws://{}/companion/v1", listener.local_addr()?),
        token: token.clone(),
    };
    let path = dir.join(format!("{}.json", hub.host_id));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    {
        use std::io::Write;
        let bytes = serde_json::to_vec(&descriptor)?;
        // Publishing and explicit shutdown share this lock. An async listener
        // starting during Desktop exit cannot recreate an orphan descriptor.
        let mut lifecycle = DESCRIPTOR
            .lock()
            .map_err(|_| std::io::Error::other("descriptor state unavailable"))?;
        if !lifecycle.can_publish() {
            return Err(std::io::Error::other("Mint is shutting down"));
        }
        let mut file = options.open(&path)?;
        if let Err(error) = file.write_all(&bytes) {
            drop(file);
            let _ = std::fs::remove_file(&path);
            return Err(error);
        }
        lifecycle.path = Some(path.clone());
    }
    let _file = DescriptorFile(path);
    hub.refresh_sessions();
    let mut refresh = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        tokio::select! {
            accepted = listener.accept() => {
                let (socket, addr) = accepted?;
                if addr.ip().is_loopback() { tokio::spawn(serve(socket, Arc::clone(&hub), token.clone())); }
            }
            _ = refresh.tick() => hub.refresh_sessions(),
        }
    }
}
async fn send(
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    event: &Event,
) -> Result<(), tokio_tungstenite::tungstenite::Error> {
    socket
        .send(Message::Text(
            serde_json::to_string(event).unwrap_or_default().into(),
        ))
        .await
}
async fn serve(socket: TcpStream, hub: Arc<Hub>, token: String) {
    let config = tokio_tungstenite::tungstenite::protocol::WebSocketConfig::default()
        .max_message_size(Some(64 * 1024))
        .max_frame_size(Some(64 * 1024));
    // Tungstenite requires its concrete HTTP ErrorResponse in this callback.
    #[allow(clippy::result_large_err)]
    let auth = move |request: &Request, response: Response| -> Result<Response, ErrorResponse> {
        if request.uri().path() != "/companion/v1"
            || request.headers().contains_key("Origin")
            || request
                .headers()
                .get("Authorization")
                .and_then(|v| v.to_str().ok())
                != Some(format!("Bearer {token}").as_str())
        {
            return Err(tokio_tungstenite::tungstenite::http::Response::builder()
                .status(401)
                .body(Some("Invalid companion credentials".into()))
                .unwrap());
        }
        Ok(response)
    };
    let Ok(Ok(mut socket)) = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        tokio_tungstenite::accept_hdr_async_with_config(socket, auth, Some(config)),
    )
    .await
    else {
        return;
    };
    let mut selected: Option<String> = None;
    let mut owned_requests = std::collections::HashSet::new();
    let mut events = hub.events.subscribe();
    let mut refresh = tokio::time::interval(std::time::Duration::from_secs(3));
    loop {
        tokio::select! {
            incoming = socket.next() => {
                let Some(Ok(message)) = incoming else { break; };
                if message.is_close() { break; }
                if let Message::Ping(bytes) = message { if socket.send(Message::Pong(bytes)).await.is_err() { break; } continue; }
                let Message::Text(text) = message else { continue; };
                let reply = match serde_json::from_str::<Command>(&text) {
                    Ok(Command::ListSessions) => hub.snapshot(selected.as_deref()),
                    Ok(Command::Subscribe { chat_id }) => {
                        if !hub.sessions.lock().unwrap().iter().any(|s| s.chat_id == chat_id) {
                            Event::Error { request_id: None, message: "Session no longer exists. Choose another session.".into() }
                        } else {
                            hub.restore_history(&chat_id); selected = Some(chat_id); hub.snapshot(selected.as_deref())
                        }
                    }
                    Ok(Command::SendChat { chat_id, request_id, message }) => { owned_requests.insert(request_id.clone()); accept_chat(&hub, chat_id, request_id, message, None) },
                    Ok(Command::Interact { chat_id, request_id, area }) => {
                        owned_requests.insert(request_id.clone());
                        match interaction(&area) { Some((message, instruction)) => accept_chat(&hub, chat_id, request_id, message, Some(instruction)), None => Event::Error { request_id: Some(request_id), message: "Unknown interaction area".into() } }
                    }
                    Err(_) => Event::Error { request_id: None, message: "Invalid companion command".into() },
                };
                if send(&mut socket, &reply).await.is_err() { break; }
            }
            event = events.recv() => {
                let reply = match event {
                    Ok(event) => match connection_event(event, selected.as_deref(), &owned_requests) { Some(event) => event, None => continue },
                    Err(broadcast::error::RecvError::Lagged(_)) => hub.snapshot(selected.as_deref()),
                    Err(_) => break,
                };
                if send(&mut socket, &reply).await.is_err() { break; }
            }
            _ = refresh.tick() => {
                if let Some(chat) = selected.as_deref() { hub.restore_history(chat); }
                if send(&mut socket, &hub.snapshot(selected.as_deref())).await.is_err() { break; }
            }
        }
    }
}
fn interaction(area: &str) -> Option<(String, String)> {
    let label = match area {
        "head" => "Pats Mint on the head",
        "cheek" => "Pokes Mint on the cheek",
        "left hand" => "Touches Mint's left hand",
        "right hand" => "Touches Mint's right hand",
        "body" => "Touches Mint",
        "lower body" => "Touches Mint's lower body",
        _ => return None,
    };
    Some((
        format!("*{label}*"),
        format!(
            "The user interacted with the Mint Live2D model: {area}. Respond briefly and playfully in the conversation's language."
        ),
    ))
}
fn accept_chat(
    hub: &Arc<Hub>,
    chat_id: String,
    request_id: String,
    message: String,
    instruction: Option<String>,
) -> Event {
    if message.trim().is_empty()
        || message.len() > 16_384
        || uuid::Uuid::parse_str(&request_id).is_err()
    {
        return Event::Error {
            request_id: Some(request_id),
            message: "Enter a message of at most 16 KiB.".into(),
        };
    }
    let session = hub
        .sessions
        .lock()
        .unwrap()
        .iter()
        .find(|s| s.chat_id == chat_id)
        .cloned();
    let Some(session) = session else {
        return Event::Error {
            request_id: Some(request_id),
            message: "Session no longer exists".into(),
        };
    };
    {
        let mut requests = hub.requests.lock().unwrap();
        if let Some(turn_id) = requests.get(&request_id) {
            return Event::Accepted {
                request_id,
                turn_id: *turn_id,
            };
        }
        if requests.len() >= 4096 {
            return Event::Error {
                request_id: Some(request_id),
                message: "Companion request limit reached for this host. Restart Mint to reset it."
                    .into(),
            };
        }
        requests.insert(request_id.clone(), None);
    }
    let execution_hub = Arc::clone(hub);
    let id = request_id.clone();
    let task_request_id = request_id.clone();
    tokio::spawn(async move {
        let listener_hub = Arc::clone(&execution_hub);
        let config = match crate::load_config() {
            Ok(c) => c,
            Err(error) => {
                let _ = execution_hub.events.send(Event::Error {
                    request_id: Some(id),
                    message: error.to_string(),
                });
                return;
            }
        };
        let request = crate::ChatRequest {
            message,
            system_instruction: instruction.unwrap_or_default(),
            chat_id: Some(chat_id.clone()),
            workspace_path: session.workspace,
            ..Default::default()
        };
        let result = crate::with_turn_start_listener(
            chat_id,
            move |turn_id| {
                listener_hub
                    .requests
                    .lock()
                    .unwrap()
                    .insert(id.clone(), Some(turn_id));
            },
            crate::orchestrate_chat_stream_with_fallback(&config, &request, |_| {}),
        )
        .await;
        if let Err(error) = result {
            let _ = execution_hub.events.send(Event::Error {
                request_id: Some(task_request_id),
                message: error.to_string(),
            });
        }
    });
    Event::Accepted {
        request_id,
        turn_id: None,
    }
}

pub(crate) fn queued(chat: &str, id: i64, prompt: &str) {
    if !chat.contains("::subagent::") {
        hub().begin(chat, id, prompt);
    }
}
pub(crate) fn update(chat: &str, id: i64, status: Status, text: Option<String>) {
    if !chat.contains("::subagent::") {
        hub().update(chat, id, status, None, text);
    }
}
pub(crate) fn chunk(chat: &str, id: i64, text: &str) {
    if !chat.contains("::subagent::") {
        hub().chunk(chat, id, text);
    }
}
pub(crate) fn progress(chat: &str, id: i64, event: &crate::AgentProgress) {
    if chat.contains("::subagent::") {
        return;
    }
    let (status, tool) = match event {
        crate::AgentProgress::ToolStart {
            action,
            subagent: None,
            ..
        } => (Status::Working, Some(action.clone())),
        crate::AgentProgress::ToolEnd { subagent: None, .. }
        | crate::AgentProgress::Thinking { .. } => (Status::Thinking, None),
        crate::AgentProgress::WaitingForNetwork { .. } => (Status::Waiting, None),
        _ => return,
    };
    hub().update(chat, id, status, tool, None);
}
#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio_tungstenite::{
        connect_async,
        tungstenite::{Message, client::IntoClientRequest},
    };

    #[test]
    fn explicit_shutdown_removes_descriptor_without_dropping_the_runtime() {
        let path =
            std::env::temp_dir().join(format!("mint-descriptor-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, "descriptor").unwrap();
        let mut lifecycle = DescriptorLifecycle {
            path: Some(path.clone()),
            stopping: false,
        };
        lifecycle.shutdown();
        let remains = path.exists();
        let _ = std::fs::remove_file(path);
        assert!(
            !remains,
            "normal Desktop exit must not rely on async runtime Drop"
        );
        assert!(lifecycle.path.is_none());
        lifecycle.shutdown();
    }
    #[test]
    fn shutdown_prevents_late_descriptor_publication() {
        let mut lifecycle = DescriptorLifecycle::default();
        lifecycle.shutdown();
        assert!(
            !lifecycle.can_publish(),
            "a listener starting during exit must not leave a new descriptor"
        );
    }

    #[test]
    fn hub_retains_completed_response_for_reconnect_without_leaking_other_sessions() {
        let hub = Hub::default();
        hub.update("one", 7, Status::Responding, None, Some("partial".into()));
        hub.update("two", 8, Status::Working, Some("run_shell".into()), None);
        hub.update(
            "one",
            7,
            Status::Completed,
            None,
            Some("final answer".into()),
        );
        let turns = hub.book.lock().unwrap().session("one");
        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].text, "final answer");
        assert_eq!(turns[0].status, Status::Completed);
    }

    #[test]
    fn completed_turn_cannot_be_reopened_by_late_progress() {
        let hub = Hub::default();
        hub.update("one", 7, Status::Completed, None, Some("done".into()));
        hub.update("one", 7, Status::Thinking, None, None);
        assert_eq!(
            hub.book.lock().unwrap().session("one")[0].status,
            Status::Completed
        );
    }
    #[test]
    fn request_error_reaches_its_connection() {
        let event = Event::Error {
            request_id: Some("mine".into()),
            message: "provider failed".into(),
        };
        assert!(
            connection_event(
                event.clone(),
                None,
                &std::collections::HashSet::from(["mine".into()])
            )
            .is_some()
        );
        assert!(connection_event(event, None, &std::collections::HashSet::new()).is_none());
    }
    #[test]
    fn snapshots_refresh_external_turns_from_shared_memory() {
        let path =
            std::env::temp_dir().join(format!("mint-companion-{}.sqlite", uuid::Uuid::new_v4()));
        let memory = crate::MemoryStore::open(&path);
        let id = memory.start_turn("one", "hello").unwrap();
        memory.claim_turn("one", id).unwrap();
        let hub = Hub::default();
        *hub.memory.lock().unwrap() = Some(memory.clone());
        hub.restore_history("one");
        memory
            .finish_turn(id, "answer", "test", "test", None)
            .unwrap();
        hub.restore_history("one");
        let turns = hub.book.lock().unwrap().session("one");
        assert_eq!(turns[0].status, Status::Completed);
        assert_eq!(turns[0].text, "answer");
        drop(memory);
        drop(hub);
        let _ = std::fs::remove_file(path);
    }
    async fn connect(
        token: &str,
    ) -> Result<
        tokio_tungstenite::WebSocketStream<tokio_tungstenite::MaybeTlsStream<TcpStream>>,
        tokio_tungstenite::tungstenite::Error,
    > {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (socket, _) = listener.accept().await.unwrap();
            serve(socket, std::sync::Arc::new(Hub::default()), "secret".into()).await;
        });
        let mut request = format!("ws://{addr}/companion/v1")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("Authorization", format!("Bearer {token}").parse().unwrap());
        connect_async(request).await.map(|(socket, _)| socket)
    }
    #[tokio::test]
    async fn authenticated_client_can_request_a_snapshot() {
        let result = connect("secret").await;
        assert!(result.is_ok(), "valid local client must connect");
        let mut socket = result.unwrap();
        socket
            .send(Message::Text(r#"{"type":"list_sessions"}"#.into()))
            .await
            .unwrap();
        let response = tokio::time::timeout(std::time::Duration::from_secs(2), socket.next())
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(matches!(
            serde_json::from_str::<Event>(&response.to_string()).unwrap(),
            Event::Snapshot { version: 1, .. }
        ));
    }
    #[tokio::test]
    async fn invalid_token_cannot_read_sessions() {
        assert!(connect("wrong").await.is_err());
    }
}

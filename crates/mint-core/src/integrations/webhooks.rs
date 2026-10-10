//! LINE and WhatsApp listener ownership, verification and message delivery.

use crate::MintConfig;
use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use serde_json::{Value, json};
use sha2::Sha256;
use std::{
    collections::HashSet,
    future::Future,
    net::SocketAddr,
    sync::{Mutex, OnceLock},
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

#[derive(Clone, Copy)]
pub(crate) enum Service {
    Line,
    Whatsapp,
}

impl Service {
    fn name(self) -> &'static str {
        match self {
            Self::Line => "line",
            Self::Whatsapp => "whatsapp",
        }
    }
    fn enabled(self, config: &MintConfig) -> bool {
        let (flag, token) = match self {
            Self::Line => ("enableLineBridge", "lineChannelAccessToken"),
            Self::Whatsapp => ("enableWhatsappBridge", "whatsappCloudAccessToken"),
        };
        config
            .extra
            .get(flag)
            .and_then(Value::as_bool)
            .unwrap_or(false)
            && !setting(config, token).is_empty()
    }
    fn address(self, config: &MintConfig) -> (String, u16) {
        match self {
            Self::Line => (config.line_webhook_host(), config.line_webhook_port()),
            Self::Whatsapp => (
                config.whatsapp_webhook_host(),
                config.whatsapp_webhook_port(),
            ),
        }
    }
    fn matches(self, method: &str, path: &str) -> bool {
        let (route, query) = path.split_once('?').unwrap_or((path, ""));
        match self {
            Self::Line => route == "/callback" && method == "POST",
            Self::Whatsapp => {
                matches!(route, "/" | "/webhook")
                    && (method == "POST"
                        || (method == "GET"
                            && query.split('&').any(|pair| pair.starts_with("hub."))))
            }
        }
    }
}

fn setting<'a>(config: &'a MintConfig, key: &str) -> &'a str {
    config
        .extra
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .unwrap_or_default()
}

static API_LISTENERS: OnceLock<Mutex<HashSet<SocketAddr>>> = OnceLock::new();
fn api_listeners() -> &'static Mutex<HashSet<SocketAddr>> {
    API_LISTENERS.get_or_init(Mutex::default)
}

/// Kept by the API serving future: cancellation automatically releases ownership.
pub(crate) struct ApiListenerLease(SocketAddr);
pub(crate) fn claim_api_listener(addr: SocketAddr) -> ApiListenerLease {
    api_listeners()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(addr);
    ApiListenerLease(addr)
}
impl Drop for ApiListenerLease {
    fn drop(&mut self) {
        api_listeners()
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(&self.0);
    }
}
fn covers(addr: SocketAddr, host: &str, port: u16) -> bool {
    if addr.port() != port {
        return false;
    }
    let ip = host.parse::<std::net::IpAddr>().ok();
    // An IPv4 wildcard covers IPv4 hosts only. IPv6 wildcard/dual-stack behavior
    // depends on the host, so share IPv6 only with explicitly IPv6 destinations.
    ip.is_some_and(|ip| {
        addr.ip() == ip || (addr.ip().is_unspecified() && addr.ip().is_ipv4() == ip.is_ipv4())
    }) || (host.eq_ignore_ascii_case("localhost")
        && (addr.ip().is_loopback() || addr.ip().is_unspecified()))
}
pub(crate) fn api_service(
    config: &MintConfig,
    addr: SocketAddr,
    method: &str,
    path: &str,
) -> Option<Service> {
    [Service::Line, Service::Whatsapp]
        .into_iter()
        .find(|service| {
            let (host, port) = service.address(config);
            let accepts_interface = covers(addr, &host, port)
                || (addr.port() == port
                    && host.parse::<std::net::IpAddr>().is_ok_and(|ip| {
                        ip.is_unspecified() && ip.is_ipv4() == addr.ip().is_ipv4()
                    }));
            service.enabled(config) && accepts_interface && service.matches(method, path)
        })
}

pub(super) async fn line_webhook_loop() -> Result<(), String> {
    listener_loop(Service::Line).await
}
pub(super) async fn whatsapp_webhook_loop() -> Result<(), String> {
    listener_loop(Service::Whatsapp).await
}

async fn listener_loop(service: Service) -> Result<(), String> {
    let config = crate::load_config().map_err(|e| e.to_string())?;
    let (host, port) = service.address(&config);
    let shared = api_listeners()
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .iter()
        .any(|addr| covers(*addr, &host, port));
    if !service.enabled(&config) || shared {
        tokio::time::sleep(Duration::from_secs(30)).await;
        return Ok(());
    }
    let listener = TcpListener::bind((host.as_str(), port))
        .await
        .map_err(|e| e.to_string())?;
    loop {
        let (mut socket, _) = listener.accept().await.map_err(|e| e.to_string())?;
        tokio::spawn(async move {
            // Bound slow and incomplete deliveries without blocking subsequent connections.
            let Ok(Some(request)) =
                tokio::time::timeout(Duration::from_secs(30), read_request(&mut socket)).await
            else {
                return;
            };
            let Ok(config) = crate::load_config() else {
                return;
            };
            handle(socket, service, config, request).await;
        });
    }
}

pub(crate) struct Request {
    pub method: String,
    pub path: String,
    pub headers: String,
    pub body: Vec<u8>,
}
const MAX_WEBHOOK_BYTES: usize = 2 * 1024 * 1024;
async fn read_request(socket: &mut TcpStream) -> Option<Request> {
    let mut bytes = Vec::with_capacity(4096);
    let mut chunk = [0; 4096];
    loop {
        let n = socket.read(&mut chunk).await.ok()?;
        if n == 0 {
            return None;
        }
        bytes.extend_from_slice(&chunk[..n]);
        if bytes.len() > MAX_WEBHOOK_BYTES {
            return None;
        }
        let Some(split) = bytes.windows(4).position(|w| w == b"\r\n\r\n") else {
            continue;
        };
        let headers = String::from_utf8(bytes[..split].to_vec()).ok()?;
        let length = header(&headers, "Content-Length")
            .unwrap_or("0")
            .parse::<usize>()
            .ok()?;
        let end = (split + 4).checked_add(length)?;
        if end > MAX_WEBHOOK_BYTES {
            return None;
        }
        if bytes.len() < end {
            continue;
        }
        let mut parts = headers.lines().next()?.split_whitespace();
        return Some(Request {
            method: parts.next()?.into(),
            path: parts.next()?.into(),
            headers,
            body: bytes[split + 4..end].to_vec(),
        });
    }
}
fn header<'a>(headers: &'a str, name: &str) -> Option<&'a str> {
    headers.lines().find_map(|line| {
        let (key, value) = line.split_once(':')?;
        key.trim().eq_ignore_ascii_case(name).then(|| value.trim())
    })
}
fn query(path: &str, key: &str) -> String {
    reqwest::Url::parse(&format!("http://localhost{path}"))
        .ok()
        .and_then(|url| {
            url.query_pairs()
                .find(|(k, _)| k == key)
                .map(|(_, value)| value.into_owned())
        })
        .unwrap_or_default()
}
fn verified(secret: &str, body: &[u8], signature: &str, hex: bool) -> bool {
    let digest = if hex {
        let signature = signature.strip_prefix("sha256=").unwrap_or(signature);
        if signature.len() != 64 || !signature.is_ascii() {
            return false;
        }
        let Ok(digest) = (0..32)
            .map(|i| u8::from_str_radix(&signature[i * 2..i * 2 + 2], 16))
            .collect::<Result<Vec<_>, _>>()
        else {
            return false;
        };
        digest
    } else {
        let Ok(digest) = STANDARD.decode(signature) else {
            return false;
        };
        digest
    };
    let Ok(mut mac) = Hmac::<Sha256>::new_from_slice(secret.as_bytes()) else {
        return false;
    };
    mac.update(body);
    mac.verify_slice(&digest).is_ok()
}
async fn respond(mut socket: TcpStream, status: &str, body: &str) {
    let response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    let _ = socket.write_all(response.as_bytes()).await;
    let _ = socket.shutdown().await;
}

pub(crate) async fn handle(
    socket: TcpStream,
    service: Service,
    config: MintConfig,
    request: Request,
) {
    if !service.matches(&request.method, &request.path) {
        respond(socket, "404 Not Found", "").await;
        return;
    }
    if !service.enabled(&config) {
        respond(socket, "503 Service Unavailable", "bridge disabled").await;
        return;
    }
    if matches!(service, Service::Whatsapp) && request.method == "GET" {
        let token = setting(&config, "whatsappVerifyToken");
        if !token.is_empty()
            && query(&request.path, "hub.mode") == "subscribe"
            && query(&request.path, "hub.verify_token") == token
        {
            crate::bridge_health::record_success(service.name());
            respond(socket, "200 OK", &query(&request.path, "hub.challenge")).await;
        } else {
            respond(socket, "403 Forbidden", "").await;
        }
        return;
    }
    if request.body.len() > MAX_WEBHOOK_BYTES {
        respond(socket, "413 Payload Too Large", "").await;
        return;
    }
    let valid = match service {
        Service::Line => {
            let secret = setting(&config, "lineChannelSecret");
            !secret.is_empty()
                && verified(
                    secret,
                    &request.body,
                    header(&request.headers, "X-Line-Signature").unwrap_or_default(),
                    false,
                )
        }
        Service::Whatsapp => {
            let secret = setting(&config, "whatsappAppSecret");
            secret.is_empty()
                || verified(
                    secret,
                    &request.body,
                    header(&request.headers, "X-Hub-Signature-256").unwrap_or_default(),
                    true,
                )
        }
    };
    if !valid {
        respond(socket, "401 Unauthorized", "").await;
        return;
    }
    let Ok(payload) = serde_json::from_slice::<Value>(&request.body) else {
        respond(socket, "400 Bad Request", "").await;
        return;
    };
    crate::bridge_health::record_success(service.name());
    // Close the HTTP delivery promptly, before agent or provider IO starts.
    respond(socket, "200 OK", "ok").await;
    deliver_payload(&config, service, &payload, &ProviderDelivery).await;
}

// External agent and provider IO stays behind this private delivery seam.
trait Delivery {
    fn authorize(&self, owner_key: &str, sender: &str) -> bool;
    fn answer(
        &self,
        text: &str,
        instruction: &str,
        chat_id: &str,
    ) -> impl Future<Output = String> + Send;
    fn send(&self, url: &str, token: &str, body: Value) -> impl Future<Output = ()> + Send;
}
struct ProviderDelivery;
impl Delivery for ProviderDelivery {
    fn authorize(&self, key: &str, sender: &str) -> bool {
        super::channels::authorize_sender(key, sender)
    }
    async fn answer(&self, text: &str, instruction: &str, chat_id: &str) -> String {
        super::channels::answer_channel(text, instruction, Some(chat_id.into())).await
    }
    async fn send(&self, url: &str, token: &str, body: Value) {
        let _ = crate::HTTP_CLIENT
            .post(url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await;
    }
}
async fn deliver_payload<D: Delivery>(
    config: &MintConfig,
    service: Service,
    payload: &Value,
    delivery: &D,
) {
    match service {
        Service::Line => {
            for event in payload["events"].as_array().into_iter().flatten() {
                if event["type"] != "message" || event["message"]["type"] != "text" {
                    continue;
                }
                let (Some(reply_token), Some(text), Some(sender)) = (
                    event["replyToken"].as_str(),
                    event["message"]["text"].as_str(),
                    event["source"]["userId"].as_str(),
                ) else {
                    continue;
                };
                if !delivery.authorize("lineOwnerUserId", sender) {
                    continue;
                }
                let answer = delivery
                    .answer(text, "Reply concisely for a LINE chat.", crate::CHAT_CLI_ID)
                    .await;
                let mut messages = Vec::new();
                if config.bridge_ack_enabled() {
                    messages.push(json!({"type":"text", "text":config.bridge_ack_message()}));
                }
                messages.push(json!({"type":"text", "text":answer}));
                delivery
                    .send(
                        "https://api.line.me/v2/bot/message/reply",
                        setting(config, "lineChannelAccessToken"),
                        json!({"replyToken":reply_token, "messages":messages}),
                    )
                    .await;
            }
        }
        Service::Whatsapp => {
            let url = format!(
                "https://graph.facebook.com/v23.0/{}/messages",
                setting(config, "whatsappPhoneNumberId")
            );
            let token = setting(config, "whatsappCloudAccessToken");
            for message in payload["entry"]
                .as_array()
                .into_iter()
                .flatten()
                .flat_map(|entry| entry["changes"].as_array().into_iter().flatten())
                .flat_map(|change| change["value"]["messages"].as_array().into_iter().flatten())
            {
                if message["type"] != "text" {
                    continue;
                }
                let (Some(sender), Some(text)) =
                    (message["from"].as_str(), message["text"]["body"].as_str())
                else {
                    continue;
                };
                if !delivery.authorize("whatsappOwnerPhone", sender) {
                    continue;
                }
                if config.bridge_ack_enabled() {
                    delivery.send(&url, token, json!({"messaging_product":"whatsapp", "to":sender, "type":"text", "text":{"body":config.bridge_ack_message()}})).await;
                }
                let answer = delivery
                    .answer(
                        text,
                        "Reply concisely for a WhatsApp chat.",
                        crate::CHAT_CLI_ID,
                    )
                    .await;
                delivery.send(&url, token, json!({"messaging_product":"whatsapp", "to":sender, "type":"text", "text":{"body":answer}})).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Mutex;

    #[test]
    fn api_listener_lease_releases_ownership_and_respects_interface() {
        let address: SocketAddr = "127.0.0.1:61983".parse().unwrap();
        let lease = claim_api_listener(address);
        assert!(api_listeners().lock().unwrap().contains(&address));
        assert!(covers(address, "localhost", 61983));
        assert!(!covers(address, "192.0.2.1", 61983));
        assert!(!covers(address, "127.0.0.1", 61984));
        drop(lease);
        assert!(!api_listeners().lock().unwrap().contains(&address));
    }

    #[derive(Default)]
    struct RecordingDelivery {
        sent: Mutex<Vec<Value>>,
        answered: Mutex<Vec<(String, String)>>,
    }
    impl Delivery for RecordingDelivery {
        fn authorize(&self, key: &str, sender: &str) -> bool {
            matches!(key, "lineOwnerUserId" | "whatsappOwnerPhone") && sender == "owner"
        }
        async fn answer(&self, text: &str, _: &str, chat_id: &str) -> String {
            self.answered
                .lock()
                .unwrap()
                .push((text.into(), chat_id.into()));
            "agent answer".into()
        }
        async fn send(&self, _: &str, _: &str, body: Value) {
            self.sent.lock().unwrap().push(body);
        }
    }

    fn config(ack: bool) -> MintConfig {
        let mut config = MintConfig::default();
        config.extra.extend([
            ("enableBridgeAckNotification".into(), json!(ack)),
            ("bridgeAckMessage".into(), json!("received")),
            ("lineChannelAccessToken".into(), json!("line-token")),
            ("whatsappCloudAccessToken".into(), json!("wa-token")),
            ("whatsappPhoneNumberId".into(), json!("phone")),
        ]);
        config
    }

    #[tokio::test]
    async fn core_migration_line_delivery_preserves_ack_reply_token_and_shared_conversation() {
        let delivery = RecordingDelivery::default();
        let payload = json!({"events": [
            {"type":"message", "replyToken":"reply-token", "source":{"userId":"owner"}, "message":{"type":"text", "text":"work"}},
            {"type":"message", "replyToken":"stranger-token", "source":{"userId":"stranger"}, "message":{"type":"text", "text":"forbidden"}},
            {"type":"message", "replyToken":"image-token", "source":{"userId":"owner"}, "message":{"type":"image", "text":"not text"}}
        ]});
        deliver_payload(&config(true), Service::Line, &payload, &delivery).await;
        assert_eq!(
            *delivery.sent.lock().unwrap(),
            [
                json!({"replyToken":"reply-token", "messages":[{"type":"text", "text":"received"}, {"type":"text", "text":"agent answer"}]})
            ]
        );
        assert_eq!(
            *delivery.answered.lock().unwrap(),
            [("work".into(), crate::CHAT_CLI_ID.into())]
        );
    }

    #[tokio::test]
    async fn core_migration_whatsapp_delivery_keeps_recipient_and_ack_order() {
        let delivery = RecordingDelivery::default();
        let payload = json!({"entry":[{"changes":[{"value":{"messages":[
            {"type":"text", "from":"owner", "text":{"body":"work"}},
            {"type":"text", "from":"stranger", "text":{"body":"forbidden"}}
        ]}}]}]});
        deliver_payload(&config(true), Service::Whatsapp, &payload, &delivery).await;
        assert_eq!(
            *delivery.sent.lock().unwrap(),
            [
                json!({"messaging_product":"whatsapp", "to":"owner", "type":"text", "text":{"body":"received"}}),
                json!({"messaging_product":"whatsapp", "to":"owner", "type":"text", "text":{"body":"agent answer"}})
            ]
        );
        assert_eq!(
            *delivery.answered.lock().unwrap(),
            [("work".into(), crate::CHAT_CLI_ID.into())]
        );
    }

    #[tokio::test]
    async fn core_migration_disabled_ack_does_not_add_a_notification_to_line_reply() {
        let delivery = RecordingDelivery::default();
        let payload = json!({"events":[{"type":"message", "replyToken":"reply", "source":{"userId":"owner"}, "message":{"type":"text", "text":"work"}}]});
        deliver_payload(&config(false), Service::Line, &payload, &delivery).await;
        assert_eq!(
            *delivery.sent.lock().unwrap(),
            [json!({"replyToken":"reply", "messages":[{"type":"text", "text":"agent answer"}]})]
        );
    }
}

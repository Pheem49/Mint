use std::collections::BTreeMap;

use base64::{Engine as _, engine::general_purpose::STANDARD};
use hmac::{Hmac, Mac};
use mint_core::{ChatRequest, load_config, orchestrate_chat};
use serde_json::{Value, json};
use sha2::Sha256;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};

type HmacSha256 = Hmac<Sha256>;

pub fn start_webhooks() {
    tauri::async_runtime::spawn(async {
        let _ = serve(3000, Service::Line).await;
    });
    tauri::async_runtime::spawn(async {
        let _ = serve(3001, Service::Whatsapp).await;
    });
}

#[derive(Clone, Copy)]
enum Service {
    Line,
    Whatsapp,
}

async fn serve(port: u16, service: Service) -> Result<(), String> {
    let listener = TcpListener::bind(("127.0.0.1", port))
        .await
        .map_err(|error| error.to_string())?;
    loop {
        let (stream, _) = listener.accept().await.map_err(|error| error.to_string())?;
        tauri::async_runtime::spawn(handle(stream, service));
    }
}

async fn handle(mut stream: TcpStream, service: Service) {
    let Ok(request) = read_request(&mut stream).await else {
        return;
    };
    let response = match service {
        Service::Line => handle_line(request).await,
        Service::Whatsapp => handle_whatsapp(request).await,
    };
    let (status, body) = response.unwrap_or_else(|error| ("500 Internal Server Error", error));
    let _ = stream.write_all(format!(
        "HTTP/1.1 {status}\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    ).as_bytes()).await;
}

async fn handle_line(request: HttpRequest) -> Result<(&'static str, String), String> {
    if request.method != "POST" || request.path != "/callback" {
        return Ok(("404 Not Found", "not found".into()));
    }
    let config = load_config().map_err(|error| error.to_string())?;
    if !extra_bool(&config.extra, "enableLineBridge") {
        return Ok(("503 Service Unavailable", "LINE bridge disabled".into()));
    }
    let secret = extra(&config.extra, "lineChannelSecret").ok_or("missing lineChannelSecret")?;
    let signature = request
        .headers
        .get("x-line-signature")
        .ok_or("missing LINE signature")?;
    verify_signature(secret.as_bytes(), &request.body, signature, false)?;
    let token = extra(&config.extra, "lineChannelAccessToken")
        .ok_or("missing lineChannelAccessToken")?
        .to_owned();
    let payload: Value =
        serde_json::from_slice(&request.body).map_err(|error| error.to_string())?;
    for event in payload["events"].as_array().cloned().unwrap_or_default() {
        let (Some(reply_token), Some(text)) = (
            event["replyToken"].as_str(),
            event["message"]["text"].as_str(),
        ) else {
            continue;
        };
        let answer = answer(text, "Reply concisely for a LINE chat.").await;
        let _ = mint_core::HTTP_CLIENT.clone().post("https://api.line.me/v2/bot/message/reply").bearer_auth(&token)
            .json(&json!({ "replyToken": reply_token, "messages": [{ "type": "text", "text": answer }] })).send().await;
    }
    Ok(("200 OK", "ok".into()))
}

async fn handle_whatsapp(request: HttpRequest) -> Result<(&'static str, String), String> {
    let config = load_config().map_err(|error| error.to_string())?;
    if request.method == "GET" {
        let query = parse_query(&request.path);
        let verify = extra(&config.extra, "whatsappVerifyToken").unwrap_or_default();
        return if query
            .get("hub.verify_token")
            .is_some_and(|token| token == verify)
        {
            Ok((
                "200 OK",
                query.get("hub.challenge").cloned().unwrap_or_default(),
            ))
        } else {
            Ok(("403 Forbidden", "verification failed".into()))
        };
    }
    if request.method != "POST" {
        return Ok(("404 Not Found", "not found".into()));
    }
    if !extra_bool(&config.extra, "enableWhatsappBridge") {
        return Ok(("503 Service Unavailable", "WhatsApp bridge disabled".into()));
    }
    if let (Some(secret), Some(signature)) = (
        extra(&config.extra, "whatsappAppSecret"),
        request.headers.get("x-hub-signature-256"),
    ) {
        verify_signature(secret.as_bytes(), &request.body, signature, true)?;
    }
    let access_token = extra(&config.extra, "whatsappCloudAccessToken")
        .ok_or("missing whatsappCloudAccessToken")?
        .to_owned();
    let phone_id = extra(&config.extra, "whatsappPhoneNumberId")
        .ok_or("missing whatsappPhoneNumberId")?
        .to_owned();
    let payload: Value =
        serde_json::from_slice(&request.body).map_err(|error| error.to_string())?;
    for message in payload["entry"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|entry| entry["changes"].as_array().into_iter().flatten())
        .flat_map(|change| change["value"]["messages"].as_array().into_iter().flatten())
    {
        let (Some(to), Some(text)) = (message["from"].as_str(), message["text"]["body"].as_str())
        else {
            continue;
        };
        let answer = answer(text, "Reply concisely for a WhatsApp chat.").await;
        let _ = mint_core::HTTP_CLIENT.clone().post(format!("https://graph.facebook.com/v23.0/{phone_id}/messages")).bearer_auth(&access_token)
            .json(&json!({ "messaging_product": "whatsapp", "to": to, "type": "text", "text": { "body": answer } })).send().await;
    }
    Ok(("200 OK", "ok".into()))
}

async fn answer(text: &str, system: &str) -> String {
    let Ok(config) = load_config() else {
        return "Mint config error".into();
    };
    orchestrate_chat(
        &config,
        &ChatRequest {
            message: text.into(),
            system_instruction: system.into(),
            chat_id: None,
            image_data_uri: None,
            audio_data_uri: None,
            document_attachment: None,
            workspace_path: None,
            agent_id: None,
        },
    )
    .await
    .map(|response| response.text)
    .unwrap_or_else(|error| format!("Mint error: {error}"))
}

struct HttpRequest {
    method: String,
    path: String,
    headers: BTreeMap<String, String>,
    body: Vec<u8>,
}

async fn read_request(stream: &mut TcpStream) -> Result<HttpRequest, String> {
    let mut bytes = Vec::new();
    let mut buffer = [0_u8; 4096];
    loop {
        let read = stream
            .read(&mut buffer)
            .await
            .map_err(|error| error.to_string())?;
        if read == 0 {
            break;
        }
        bytes.extend_from_slice(&buffer[..read]);
        if let Some(split) = bytes.windows(4).position(|window| window == b"\r\n\r\n") {
            let header = String::from_utf8_lossy(&bytes[..split]);
            let mut lines = header.lines();
            let mut first = lines.next().unwrap_or_default().split_whitespace();
            let method = first.next().unwrap_or_default().to_owned();
            let path = first.next().unwrap_or_default().to_owned();
            let headers = lines
                .filter_map(|line| line.split_once(':'))
                .map(|(key, value)| (key.trim().to_ascii_lowercase(), value.trim().to_owned()))
                .collect::<BTreeMap<_, _>>();
            let length = headers
                .get("content-length")
                .and_then(|value| value.parse().ok())
                .unwrap_or(0);
            let start = split + 4;
            while bytes.len() < start + length {
                let read = stream
                    .read(&mut buffer)
                    .await
                    .map_err(|error| error.to_string())?;
                if read == 0 {
                    break;
                }
                bytes.extend_from_slice(&buffer[..read]);
            }
            return Ok(HttpRequest {
                method,
                path,
                headers,
                body: bytes[start..start + length.min(bytes.len() - start)].to_vec(),
            });
        }
    }
    Err("invalid HTTP request".into())
}

fn verify_signature(secret: &[u8], body: &[u8], signature: &str, hex: bool) -> Result<(), String> {
    let mut mac = HmacSha256::new_from_slice(secret).map_err(|error| error.to_string())?;
    mac.update(body);
    let expected = if hex {
        hex_encode(&mac.finalize().into_bytes())
    } else {
        STANDARD.encode(mac.finalize().into_bytes())
    };
    (signature.trim_start_matches("sha256=") == expected)
        .then_some(())
        .ok_or_else(|| "webhook signature verification failed".into())
}

fn parse_query(path: &str) -> BTreeMap<String, String> {
    path.split_once('?')
        .map(|(_, query)| {
            query
                .split('&')
                .filter_map(|item| item.split_once('='))
                .map(|(key, value)| (key.to_owned(), value.to_owned()))
                .collect()
        })
        .unwrap_or_default()
}

fn extra<'a>(extra: &'a BTreeMap<String, Value>, key: &str) -> Option<&'a str> {
    extra
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
}
fn extra_bool(extra: &BTreeMap<String, Value>, key: &str) -> bool {
    extra.get(key).and_then(Value::as_bool).unwrap_or(false)
}
fn hex_encode(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

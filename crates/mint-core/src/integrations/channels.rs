use super::webhooks::{line_webhook_loop, whatsapp_webhook_loop};
use std::time::Duration;
use std::{
    collections::HashSet,
    sync::{Mutex, OnceLock},
};

use crate::{ChatRequest, MintConfig, load_config, orchestrate_chat, set_config_value};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use futures_util::{FutureExt, SinkExt, StreamExt};
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub fn start_channels() {
    if tokio::runtime::Handle::try_current().is_ok() {
        tokio::spawn(owned_restarting_loop("telegram", telegram_loop));
        tokio::spawn(owned_restarting_loop("discord", discord_loop));
        tokio::spawn(owned_restarting_loop("slack", slack_loop));
        tokio::spawn(owned_restarting_loop("line", line_webhook_loop));
        tokio::spawn(owned_restarting_loop("whatsapp", whatsapp_webhook_loop));
        tokio::spawn(owned_restarting_loop("signal", signal_loop));
        tokio::spawn(owned_restarting_loop("email", email_loop));
    } else {
        std::thread::spawn(|| {
            if let Ok(rt) = tokio::runtime::Runtime::new() {
                rt.block_on(async {
                    let h1 = tokio::spawn(owned_restarting_loop("telegram", telegram_loop));
                    let h2 = tokio::spawn(owned_restarting_loop("discord", discord_loop));
                    let h3 = tokio::spawn(owned_restarting_loop("slack", slack_loop));
                    let h4 = tokio::spawn(owned_restarting_loop("line", line_webhook_loop));
                    let h5 = tokio::spawn(owned_restarting_loop("whatsapp", whatsapp_webhook_loop));
                    let h6 = tokio::spawn(owned_restarting_loop("signal", signal_loop));
                    let h7 = tokio::spawn(owned_restarting_loop("email", email_loop));
                    let _ = tokio::join!(h1, h2, h3, h4, h5, h6, h7);
                });
            }
        });
    }
}

/// Keeps `run` alive forever, retrying after a 5s backoff on either an `Err`
/// return *or* a panic. Without `catch_unwind` here, a panic inside one bridge
/// loop (bad payload shape, an unexpected `None`, ...) would unwind straight
/// out of the `tokio::spawn`ed task in `start_channels` and never be seen
/// again: the task just vanishes, `systemctl status` still shows the process
/// `active` since nothing else crashed, and that one bridge silently stops
/// responding forever with no restart and no error surfaced anywhere. Each
/// call to `run()` is independent (no shared mutable state carried across
/// iterations), so resuming after a caught panic is safe — there's nothing
/// left in a torn-down state to worry about.
async fn restarting_loop<F, Fut>(name: &'static str, mut run: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    crate::bridge_health::record_started(name);
    loop {
        let outcome = std::panic::AssertUnwindSafe(run()).catch_unwind().await;
        let retry = match outcome {
            Ok(Ok(())) => false,
            Ok(Err(error)) => {
                eprintln!("[mint] {name} bridge loop error, retrying in 5s: {error}");
                crate::bridge_health::record_error(name, &error);
                true
            }
            Err(payload) => {
                let message = panic_payload_message(&payload);
                eprintln!("[mint] {name} bridge loop panicked, retrying in 5s: {message}");
                crate::bridge_health::record_error(name, &message);
                true
            }
        };
        if retry {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}

async fn owned_restarting_loop<F, Fut>(name: &'static str, run: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    static OWNERS: OnceLock<Mutex<HashSet<&'static str>>> = OnceLock::new();
    struct Lease(&'static str);
    impl Drop for Lease {
        fn drop(&mut self) {
            OWNERS
                .get()
                .unwrap()
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .remove(self.0);
        }
    }
    let owners = OWNERS.get_or_init(Mutex::default);
    if !owners
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(name)
    {
        return;
    }
    let _lease = Lease(name);
    restarting_loop(name, run).await;
}

pub(crate) fn panic_payload_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        (*message).to_string()
    } else if let Some(message) = payload.downcast_ref::<String>() {
        message.clone()
    } else {
        "unknown panic payload".to_string()
    }
}

async fn telegram_loop() -> Result<(), String> {
    let client = crate::HTTP_CLIENT.clone();
    let mut offset = 0_i64;
    loop {
        let Some(token) = enabled_value("enableTelegramBridge", "telegramBotToken") else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        let value: Value = client
            .get(format!("https://api.telegram.org/bot{token}/getUpdates"))
            .query(&[("timeout", "20"), ("offset", &offset.to_string())])
            .send()
            .await
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        crate::bridge_health::record_success("telegram");
        for update in value["result"].as_array().cloned().unwrap_or_default() {
            offset = update["update_id"].as_i64().unwrap_or(offset) + 1;
            let (Some(chat_id), Some(text)) = (
                update["message"]["chat"]["id"].as_i64(),
                update["message"]["text"].as_str(),
            ) else {
                continue;
            };
            let sender_id = update["message"]["from"]["id"]
                .as_i64()
                .map(|id| id.to_string())
                .unwrap_or_default();
            if !authorize_sender("telegramOwnerChatId", &sender_id) {
                continue;
            }
            // Every bridge shares `CHAT_CLI_ID` (rather than a per-platform id) so the
            // owner's memory/history is one continuous conversation regardless of
            // which front-end (terminal, Telegram, Discord, ...) they're using —
            // authorize_sender above already limits each bridge to a single owner,
            // so there's no cross-user mixing risk to guard against here.
            let shared_chat_id = crate::CHAT_CLI_ID.to_string();
            if let Ok(config) = load_config()
                && config.bridge_ack_enabled()
            {
                let _ = client
                    .post(format!(
                        "https://api.telegram.org/bot{token}/sendChatAction"
                    ))
                    .json(&json!({ "chat_id": chat_id, "action": "typing" }))
                    .send()
                    .await;
            }
            let answer = answer_channel(
                text,
                "Reply concisely for a Telegram chat.",
                Some(shared_chat_id),
            )
            .await;
            let _ = client
                .post(format!("https://api.telegram.org/bot{token}/sendMessage"))
                .json(&json!({ "chat_id": chat_id, "text": answer }))
                .send()
                .await;
        }
    }
}

async fn discord_loop() -> Result<(), String> {
    let Some(token) = enabled_value("enableDiscordBridge", "discordBotToken") else {
        tokio::time::sleep(Duration::from_secs(30)).await;
        return Ok(());
    };
    let (socket, _) = connect_async("wss://gateway.discord.gg/?v=10&encoding=json")
        .await
        .map_err(|error| error.to_string())?;
    let (mut writer, mut reader) = socket.split();
    let hello = read_json(&mut reader).await?;
    let interval = hello["d"]["heartbeat_interval"].as_u64().unwrap_or(45_000);
    writer.send(Message::Text(json!({
        "op": 2,
        "d": {
            "token": token,
            "intents": 37377,
            "properties": { "os": std::env::consts::OS, "browser": "mint", "device": "mint" }
        }
    }).to_string().into())).await.map_err(|error| error.to_string())?;
    crate::bridge_health::record_success("discord");
    let mut heartbeat = tokio::time::interval(Duration::from_millis(interval));
    let mut sequence = Value::Null;
    let mut bot_id = String::new();
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                writer.send(Message::Text(json!({ "op": 1, "d": sequence }).to_string().into()))
                    .await.map_err(|error| error.to_string())?;
                crate::bridge_health::record_success("discord");
            }
            item = reader.next() => {
                let value = parse_ws(item)?;
                if !value["s"].is_null() { sequence = value["s"].clone(); }
                if value["t"] == "READY" {
                    bot_id = value["d"]["user"]["id"].as_str().unwrap_or_default().to_owned();
                }
                if value["op"] == 1 {
                    writer.send(Message::Text(json!({ "op": 1, "d": sequence }).to_string().into()))
                        .await.map_err(|error| error.to_string())?;
                }
                if value["t"] == "MESSAGE_CREATE" && value["d"]["author"]["bot"] != true {
                    let (Some(channel), Some(text)) = (value["d"]["channel_id"].as_str(), value["d"]["content"].as_str()) else { continue };
                    let direct_message = value["d"]["guild_id"].is_null();
                    let mentioned = value["d"]["mentions"].as_array().is_some_and(|mentions| {
                        mentions.iter().any(|mention| mention["id"].as_str() == Some(&bot_id))
                    });
                    if !direct_message && !mentioned { continue }
                    let sender_id = value["d"]["author"]["id"].as_str().unwrap_or_default();
                    if !authorize_sender("discordOwnerUserId", sender_id) { continue }
                    if let Ok(config) = load_config()
                        && config.bridge_ack_enabled()
                    {
                        let _ = crate::HTTP_CLIENT.clone().post(format!("https://discord.com/api/v10/channels/{channel}/typing"))
                            .header("Authorization", format!("Bot {token}")).send().await;
                    }
                    let shared_chat_id = crate::CHAT_CLI_ID.to_string();
                    let reply = answer_channel(text, "Reply concisely for a Discord chat.", Some(shared_chat_id)).await;
                    let _ = crate::HTTP_CLIENT.clone().post(format!("https://discord.com/api/v10/channels/{channel}/messages"))
                        .header("Authorization", format!("Bot {token}")).json(&json!({ "content": reply })).send().await;
                }
            }
        }
    }
}

async fn slack_loop() -> Result<(), String> {
    let Some(app_token) = enabled_value("enableSlackBridge", "slackAppToken") else {
        tokio::time::sleep(Duration::from_secs(30)).await;
        return Ok(());
    };
    let bot_token = config_value("slackBotToken").ok_or("missing slackBotToken")?;
    let value: Value = crate::HTTP_CLIENT
        .clone()
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(&app_token)
        .send()
        .await
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    let url = value["url"]
        .as_str()
        .ok_or("Slack did not return a Socket Mode URL")?;
    let (socket, _) = connect_async(url)
        .await
        .map_err(|error| error.to_string())?;
    let (mut writer, mut reader) = socket.split();
    while let Some(item) = reader.next().await {
        let value = parse_ws(Some(item))?;
        crate::bridge_health::record_success("slack");
        if let Some(envelope) = value["envelope_id"].as_str() {
            writer
                .send(Message::Text(
                    json!({ "envelope_id": envelope }).to_string().into(),
                ))
                .await
                .map_err(|error| error.to_string())?;
        }
        let event = &value["payload"]["event"];
        let app_mention = event["type"] == "app_mention";
        let direct_message = event["type"] == "message" && event["channel_type"] == "im";
        if (!app_mention && !direct_message) || event["bot_id"].is_string() {
            continue;
        }
        let (Some(channel), Some(text)) = (event["channel"].as_str(), event["text"].as_str())
        else {
            continue;
        };
        let sender_id = event["user"].as_str().unwrap_or_default();
        if !authorize_sender("slackOwnerUserId", sender_id) {
            continue;
        }
        let shared_chat_id = crate::CHAT_CLI_ID.to_string();
        let reply = answer_channel(
            text,
            "Reply concisely for a Slack chat.",
            Some(shared_chat_id),
        )
        .await;
        let _ = crate::HTTP_CLIENT
            .clone()
            .post("https://slack.com/api/chat.postMessage")
            .bearer_auth(&bot_token)
            .json(&json!({ "channel": channel, "text": reply }))
            .send()
            .await;
    }
    Ok(())
}

/// Signal has no official bot API, so this talks to a self-hosted
/// `signal-cli-rest-api` (https://github.com/bbernhard/signal-cli-rest-api)
/// instance instead — same "bring your own infra" shape as the LINE/WhatsApp
/// webhook loops above, except here Mint is the client polling a local REST
/// API rather than a server receiving a public webhook. Linking the number
/// with `signal-cli`/`signal-cli-rest-api` itself is the user's setup step;
/// this loop only talks to it once it's already running and registered.
async fn signal_loop() -> Result<(), String> {
    let client = crate::HTTP_CLIENT.clone();
    loop {
        let Some(base_url) = enabled_value("enableSignalBridge", "signalApiUrl") else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        let Some(number) = config_value("signalNumber") else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        let base_url = base_url.trim_end_matches('/');
        let messages: Vec<Value> = client
            .get(format!("{base_url}/v1/receive/{number}"))
            .send()
            .await
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        crate::bridge_health::record_success("signal");
        for item in messages {
            let envelope = &item["envelope"];
            let source = envelope["sourceNumber"]
                .as_str()
                .or_else(|| envelope["source"].as_str());
            let text = envelope["dataMessage"]["message"].as_str();
            let (Some(source), Some(text)) = (source, text) else {
                continue;
            };
            if text.trim().is_empty() {
                continue;
            }
            if !authorize_sender("signalOwnerNumber", source) {
                continue;
            }
            let shared_chat_id = crate::CHAT_CLI_ID.to_string();
            let reply = answer_channel(
                text,
                "Reply concisely for a Signal chat.",
                Some(shared_chat_id),
            )
            .await;
            let _ = client
                .post(format!("{base_url}/v2/send"))
                .json(&json!({ "message": reply, "number": number, "recipients": [source] }))
                .send()
                .await;
        }
        tokio::time::sleep(Duration::from_secs(3)).await;
    }
}

/// Rides on the same Gmail OAuth connection the `gmail` native plugin uses
/// (`gmailClientId`/`gmailClientSecret`/`gmailRefreshToken` — connect once via
/// `mint gmail`, both features share it) rather than asking for separate IMAP/
/// SMTP credentials. Unlike the `gmail` plugin's `draft`-only action (deliberately
/// non-sending, since that tool can be invoked mid-task with an arbitrary
/// recipient), this loop sends real replies straight back to the sender — safe
/// here because `authorize_sender` limits that sender to a single verified
/// owner address, same guarantee every other bridge relies on.
///
/// Uses each message's `snippet` (Gmail's short plain-text preview) rather than
/// parsing full MIME bodies, matching the same trade-off the `gmail` plugin's
/// `read` action already makes — enough for a short chat-style exchange, not a
/// full email client.
async fn email_loop() -> Result<(), String> {
    let client = crate::HTTP_CLIENT.clone();
    loop {
        let Some(refresh_token) = enabled_value("enableEmailBridge", "gmailRefreshToken") else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        let (Some(client_id), Some(client_secret)) = (
            config_value("gmailClientId"),
            config_value("gmailClientSecret"),
        ) else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };
        let Ok(token) =
            crate::plugins::google_access_token(&client_id, &client_secret, &refresh_token).await
        else {
            tokio::time::sleep(Duration::from_secs(30)).await;
            continue;
        };

        let list: Value = client
            .get("https://gmail.googleapis.com/gmail/v1/users/me/messages")
            .bearer_auth(&token)
            .query(&[("q", "is:unread in:inbox"), ("maxResults", "10")])
            .send()
            .await
            .map_err(request_error)?
            .json()
            .await
            .map_err(request_error)?;
        crate::bridge_health::record_success("email");

        for item in list["messages"].as_array().cloned().unwrap_or_default() {
            let Some(id) = item["id"].as_str() else {
                continue;
            };
            let Ok(response) = client
                .get(format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}"
                ))
                .bearer_auth(&token)
                .query(&[("format", "full")])
                .send()
                .await
            else {
                continue;
            };
            let Ok(detail) = response.json::<Value>().await else {
                continue;
            };

            let headers = detail["payload"]["headers"]
                .as_array()
                .cloned()
                .unwrap_or_default();
            let header = |name: &str| -> Option<&str> {
                headers
                    .iter()
                    .find(|h| {
                        h["name"]
                            .as_str()
                            .is_some_and(|n| n.eq_ignore_ascii_case(name))
                    })
                    .and_then(|h| h["value"].as_str())
            };
            let sender_email = header("From")
                .map(extract_email_address)
                .unwrap_or_default();
            let subject = header("Subject").unwrap_or("(no subject)").to_string();
            let text = detail["snippet"].as_str().unwrap_or_default();

            // Mark it read regardless of outcome from here on, so an
            // unauthorized/empty/malformed message doesn't get re-fetched
            // and re-evaluated on every poll.
            let _ = client
                .post(format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/messages/{id}/modify"
                ))
                .bearer_auth(&token)
                .json(&json!({ "removeLabelIds": ["UNREAD"] }))
                .send()
                .await;

            if sender_email.is_empty() || text.trim().is_empty() {
                continue;
            }
            if !authorize_sender("emailOwnerAddress", &sender_email) {
                continue;
            }

            let shared_chat_id = crate::CHAT_CLI_ID.to_string();
            let reply =
                answer_channel(text, "Reply concisely for an email.", Some(shared_chat_id)).await;
            let reply_subject = if subject.trim().to_lowercase().starts_with("re:") {
                subject.trim().to_string()
            } else {
                format!("Re: {}", subject.trim())
            };
            let raw = URL_SAFE_NO_PAD.encode(format!(
                "To: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\n\r\n{}",
                crate::plugins::sanitize_header(&sender_email),
                crate::plugins::sanitize_header(&reply_subject),
                reply
            ));
            let _ = client
                .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
                .bearer_auth(&token)
                .json(&json!({ "raw": raw }))
                .send()
                .await;
        }
        tokio::time::sleep(Duration::from_secs(30)).await;
    }
}

/// Pulls the bare address out of a `"Display Name <addr@example.com>"` header,
/// or returns the header unchanged if it's already a bare address.
fn extract_email_address(header: &str) -> String {
    if let Some(start) = header.find('<')
        && let Some(end) = header[start..].find('>')
    {
        return header[start + 1..start + end].trim().to_lowercase();
    }
    header.trim().to_lowercase()
}

pub fn is_action_intent(text: &str) -> bool {
    let lower = text.to_lowercase();
    let action_keywords = [
        "แก้",
        "สร้าง",
        "ลบ",
        "รัน",
        "ตรวจ",
        "ค้นหา",
        "เช็ค",
        "ดูไฟล์",
        "อ่านไฟล์",
        "วิเคราะห์ไฟล์",
        "fix",
        "create",
        "build",
        "run",
        "delete",
        "edit",
        "update",
        "check",
        "find",
        "search",
        "read file",
        "git",
        "patch",
        "test",
        "analyze file",
    ];
    action_keywords
        .iter()
        .any(|&keyword| lower.contains(keyword))
}

pub async fn answer_channel(
    text: &str,
    system_instruction: &str,
    chat_id: Option<String>,
) -> String {
    let Ok(config) = load_config() else {
        return "Mint config error".into();
    };
    let workspace = config.active_workspace_path();
    let workspace_str = workspace.as_ref().map(|p| p.to_string_lossy().to_string());

    if is_action_intent(text)
        && let Some(ref root_path) = workspace
        && root_path.exists()
    {
        let agent_result = crate::orchestrate_agent_loop(
            &config,
            text,
            root_path,
            None,
            None,
            None,
            chat_id.as_deref(),
            None,
            None,
            None,
            true,
            false,
            |_approval| Ok(crate::ApprovalOutcome::Approved),
            |_progress| {},
            |_chunk| {},
        )
        .await;

        if let Ok(res) = agent_result
            && !res.summary.trim().is_empty()
        {
            return res.summary;
        }
    }

    orchestrate_chat(
        &config,
        &ChatRequest {
            message: text.into(),
            system_instruction: system_instruction.into(),
            chat_id,
            image_data_uri: None,
            audio_data_uri: None,
            video_data_uri: None,
            document_attachment: None,
            workspace_path: workspace_str,
            agent_id: None,
            plan_mode: false,
            pinned_mcp_server: None,
            messages: None,
            tools: None,
            temperature: config.temperature,
            ..Default::default()
        },
    )
    .await
    .map(|response| response.text)
    .unwrap_or_else(|error| format!("Mint error: {error}"))
}

async fn read_json<S>(reader: &mut S) -> Result<Value, String>
where
    S: StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>> + Unpin,
{
    parse_ws(reader.next().await)
}

fn parse_ws(
    item: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> Result<Value, String> {
    let message = item
        .ok_or("WebSocket closed")?
        .map_err(|error| error.to_string())?;
    serde_json::from_str(message.to_text().map_err(|error| error.to_string())?)
        .map_err(|error| error.to_string())
}

fn enabled_value(enabled_key: &str, value_key: &str) -> Option<String> {
    let config = load_config().ok()?;
    config
        .extra
        .get(enabled_key)
        .and_then(Value::as_bool)
        .unwrap_or(false)
        .then(|| extra_string(&config, value_key))
        .flatten()
}

fn config_value(key: &str) -> Option<String> {
    extra_string(&load_config().ok()?, key)
}

fn extra_string(config: &MintConfig, key: &str) -> Option<String> {
    config
        .extra
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

/// Guards every bridge against strangers: none of Telegram/Discord/Slack/
/// LINE/WhatsApp verify *who* is messaging the bot on their own — a
/// Telegram bot can be DMed by anyone who finds it, a Discord/Slack bot can
/// be @-mentioned by anyone sharing a server/workspace with it, and LINE/
/// WhatsApp accounts can be messaged by anyone with the number/QR code.
/// Without this check, any of those strangers could trigger
/// `answer_channel`'s agent loop — which auto-approves every action,
/// including `write_file`/`apply_patch`/`run_shell`, since there's no human
/// present on a bridge to click "approve" — on the *owner's* machine.
///
/// The fix needs zero setup: the first sender ever seen on a given platform
/// is claimed as its owner (persisted under `owner_key` in `config.extra`,
/// e.g. `"telegramOwnerChatId"`) and everyone else is rejected from then on.
/// To let a different sender claim it later, clear the stored id — e.g.
/// `mint config set telegramOwnerChatId ""`.
pub(super) fn authorize_sender(owner_key: &str, sender_id: &str) -> bool {
    if sender_id.is_empty() {
        return false;
    }
    let Ok(config) = load_config() else {
        return false;
    };
    match sender_authorization(extra_string(&config, owner_key).as_deref(), sender_id) {
        SenderAuthorization::Authorized => true,
        SenderAuthorization::Claim => {
            let _ = set_config_value(owner_key, Value::String(sender_id.to_string()));
            true
        }
        SenderAuthorization::Rejected => false,
    }
}

/// The decision core of [`authorize_sender`], split out so it's testable
/// without touching the real on-disk config (`load_config`/`set_config_value`
/// always resolve to the user's actual config file — there's no test-scoped
/// override for them, unlike `MemoryStore`/`CronStore`, which is exactly the
/// class of bug that let earlier `cargo test` runs leave real orphaned rows
/// in this user's own database; see the `cfg!(test)` guards in
/// `cron::store`).
#[derive(Debug, PartialEq, Eq)]
enum SenderAuthorization {
    /// Matches the stored owner.
    Authorized,
    /// No owner stored yet — this sender should be claimed as the owner.
    Claim,
    /// A different owner is already stored.
    Rejected,
}

fn sender_authorization(stored_owner: Option<&str>, sender_id: &str) -> SenderAuthorization {
    match stored_owner {
        Some(owner) if owner == sender_id => SenderAuthorization::Authorized,
        Some(_) => SenderAuthorization::Rejected,
        None => SenderAuthorization::Claim,
    }
}

fn request_error(error: reqwest::Error) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn core_migration_duplicate_bridge_start_has_one_owner_and_abort_releases_it() {
        use std::sync::{
            Arc,
            atomic::{AtomicUsize, Ordering},
        };
        let calls = Arc::new(AtomicUsize::new(0));
        let start = || {
            let calls = Arc::clone(&calls);
            tokio::spawn(owned_restarting_loop("ownership-regression", move || {
                calls.fetch_add(1, Ordering::SeqCst);
                std::future::pending::<Result<(), String>>()
            }))
        };
        let first = start();
        let second = start();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "duplicate startup ran the bridge twice"
        );
        first.abort();
        second.abort();
        let _ = first.await;
        let _ = second.await;
        let next = start();
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(
            calls.load(Ordering::SeqCst),
            2,
            "aborted runtime retained bridge ownership"
        );
        next.abort();
        let _ = next.await;
    }

    #[test]
    fn no_stored_owner_claims_the_sender() {
        assert_eq!(
            sender_authorization(None, "user-1"),
            SenderAuthorization::Claim
        );
    }

    #[test]
    fn matching_stored_owner_is_authorized() {
        assert_eq!(
            sender_authorization(Some("user-1"), "user-1"),
            SenderAuthorization::Authorized
        );
    }

    #[test]
    fn a_different_sender_than_the_stored_owner_is_rejected() {
        assert_eq!(
            sender_authorization(Some("user-1"), "user-2"),
            SenderAuthorization::Rejected
        );
    }

    #[test]
    fn an_empty_sender_id_is_rejected_without_touching_config() {
        // Bails out before `load_config()`/`set_config_value()` — a missing
        // sender id (a field the platform didn't send) must never fall
        // through to "no owner stored yet, claim it".
        assert!(!authorize_sender("telegramOwnerChatId", ""));
    }

    #[tokio::test]
    async fn restarting_loop_retries_after_a_panic_instead_of_dying_silently() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let calls = Arc::new(AtomicUsize::new(0));
        let calls_for_run = Arc::clone(&calls);
        let run = move || {
            let calls = Arc::clone(&calls_for_run);
            async move {
                if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                    panic!("simulated bridge loop panic");
                }
                Err::<(), String>("stop after second call".into())
            }
        };

        // `restarting_loop` never returns on its own, so this timeout always
        // fully elapses — it just bounds the wait. 6s covers the one 5s
        // backoff needed to observe a second call after the panic.
        let _ = tokio::time::timeout(
            Duration::from_secs(6),
            restarting_loop("test-panic-recovery-bridge", run),
        )
        .await;

        assert!(
            calls.load(Ordering::SeqCst) >= 2,
            "restarting_loop should have called run() again after it panicked"
        );
    }
}

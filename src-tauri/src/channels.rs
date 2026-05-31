use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use mint_core::{ChatRequest, MintConfig, load_config, orchestrate_chat};
use reqwest::Client;
use serde_json::{Value, json};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub fn start_channels() {
    tauri::async_runtime::spawn(restarting_loop(telegram_loop));
    tauri::async_runtime::spawn(restarting_loop(discord_loop));
    tauri::async_runtime::spawn(restarting_loop(slack_loop));
}

async fn restarting_loop<F, Fut>(mut run: F)
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<(), String>>,
{
    loop {
        if run().await.is_err() {
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
    }
}

async fn telegram_loop() -> Result<(), String> {
    let client = Client::new();
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
        for update in value["result"].as_array().cloned().unwrap_or_default() {
            offset = update["update_id"].as_i64().unwrap_or(offset) + 1;
            let (Some(chat_id), Some(text)) = (
                update["message"]["chat"]["id"].as_i64(),
                update["message"]["text"].as_str(),
            ) else {
                continue;
            };
            let answer = answer(text, "Reply concisely for a Telegram chat.").await;
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
    let mut heartbeat = tokio::time::interval(Duration::from_millis(interval));
    let mut sequence = Value::Null;
    let mut bot_id = String::new();
    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                writer.send(Message::Text(json!({ "op": 1, "d": sequence }).to_string().into()))
                    .await.map_err(|error| error.to_string())?;
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
                    let reply = answer(text, "Reply concisely for a Discord chat.").await;
                    let _ = Client::new().post(format!("https://discord.com/api/v10/channels/{channel}/messages"))
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
    let value: Value = Client::new()
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
        let reply = answer(text, "Reply concisely for a Slack chat.").await;
        let _ = Client::new()
            .post("https://slack.com/api/chat.postMessage")
            .bearer_auth(&bot_token)
            .json(&json!({ "channel": channel, "text": reply }))
            .send()
            .await;
    }
    Ok(())
}

async fn answer(text: &str, system_instruction: &str) -> String {
    let Ok(config) = load_config() else {
        return "Mint config error".into();
    };
    orchestrate_chat(
        &config,
        &ChatRequest {
            message: text.into(),
            system_instruction: system_instruction.into(),
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

fn request_error(error: reqwest::Error) -> String {
    error.to_string()
}

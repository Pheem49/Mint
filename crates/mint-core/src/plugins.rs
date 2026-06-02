use std::{
    fs,
    path::{Component, Path, PathBuf},
    process::Command,
    time::{SystemTime, UNIX_EPOCH},
};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Serialize;
use serde_json::{Value, json};
use thiserror::Error;

use crate::MintConfig;

#[derive(Debug, Error)]
pub enum PluginError {
    #[error("unknown native plugin: {0}")]
    UnknownPlugin(String),
    #[error("invalid instruction for {plugin}: {message}")]
    InvalidInstruction {
        plugin: &'static str,
        message: String,
    },
    #[error("unable to run {program}: {source}")]
    Execute {
        program: &'static str,
        source: std::io::Error,
    },
    #[error("{program} failed: {message}")]
    Failed {
        program: &'static str,
        message: String,
    },
    #[error("unable to access note {path}: {source}")]
    Note {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("plugin request failed: {message}")]
    RequestFailed {
        message: String,
    },
    #[error("missing configuration value: {message}")]
    MissingConfig {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize)]
pub struct NativePlugin {
    pub name: &'static str,
    pub description: &'static str,
}

pub fn native_plugins() -> Vec<NativePlugin> {
    vec![
        NativePlugin {
            name: "dev_tools",
            description: "Read git status, log, or branch information.",
        },
        NativePlugin {
            name: "docker",
            description: "List, start, stop, or restart local Docker containers.",
        },
        NativePlugin {
            name: "obsidian",
            description: "List, read, or append local Markdown notes.",
        },
        NativePlugin {
            name: "spotify",
            description: "Control Spotify through playerctl.",
        },
        NativePlugin {
            name: "system_metrics",
            description: "Read native RAM, CPU, and uptime metrics.",
        },
        NativePlugin {
            name: "gmail",
            description: "Search/read Gmail and create drafts safely.",
        },
        NativePlugin {
            name: "google_calendar",
            description: "List events and create calendar events via Google Calendar API.",
        },
        NativePlugin {
            name: "notion",
            description: "Create notes, read databases, and append blocks through Notion API.",
        },
    ]
}

pub async fn execute_native_plugin(
    config: &MintConfig,
    name: &str,
    instruction: &str,
) -> Result<String, PluginError> {
    match name {
        "dev_tools" => dev_tools(instruction),
        "docker" => docker(instruction),
        "obsidian" => obsidian(instruction),
        "spotify" => spotify(instruction),
        "system_metrics" => system_metrics(instruction),
        "gmail" => gmail(config, instruction).await,
        "google_calendar" => calendar(config, instruction).await,
        "notion" => notion(config, instruction).await,
        _ => Err(PluginError::UnknownPlugin(name.into())),
    }
}

fn dev_tools(instruction: &str) -> Result<String, PluginError> {
    let lower = instruction.to_lowercase();
    let args = if lower.contains("status") {
        vec!["status", "--short"]
    } else if lower.contains("log") || lower.contains("commit") {
        vec!["log", "-n", "5", "--oneline"]
    } else if lower.contains("branch") {
        vec!["branch"]
    } else {
        return invalid("dev_tools", "expected status, log, or branch");
    };
    run("git", &args)
}

fn docker(instruction: &str) -> Result<String, PluginError> {
    let parts = instruction.split_whitespace().collect::<Vec<_>>();
    match parts.as_slice() {
        ["list"] => run("docker", &["ps", "--format", "{{.Names}} ({{.Status}})"]),
        [action, container]
            if matches!(*action, "start" | "stop" | "restart")
                && container.chars().all(|character| {
                    character.is_ascii_alphanumeric() || "_.-".contains(character)
                }) =>
        {
            run("docker", &[*action, *container])
        }
        _ => invalid(
            "docker",
            "expected list, start <container>, stop <container>, or restart <container>",
        ),
    }
}

fn obsidian(instruction: &str) -> Result<String, PluginError> {
    let directory = notes_directory()?;
    if instruction.trim() == "list" {
        let mut notes = fs::read_dir(&directory)
            .map_err(|source| PluginError::Note {
                path: directory.clone(),
                source,
            })?
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.ends_with(".md"))
            .collect::<Vec<_>>();
        notes.sort();
        return Ok(notes.join("\n"));
    }
    if let Some(name) = instruction.strip_prefix("read:") {
        let path = note_path(&directory, name)?;
        return fs::read_to_string(&path).map_err(|source| PluginError::Note { path, source });
    }
    if let Some(payload) = instruction.strip_prefix("write:") {
        let Some((name, content)) = payload.split_once('|') else {
            return invalid("obsidian", "expected write: filename | content");
        };
        let path = note_path(&directory, name)?;
        let entry = format!("\n--- saved {} ---\n{}\n", timestamp(), content.trim());
        fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut file| std::io::Write::write_all(&mut file, entry.as_bytes()))
            .map_err(|source| PluginError::Note { path, source })?;
        return Ok("saved".into());
    }
    invalid(
        "obsidian",
        "expected list, read: filename, or write: filename | content",
    )
}

fn spotify(instruction: &str) -> Result<String, PluginError> {
    let parts = instruction.split_whitespace().collect::<Vec<_>>();
    let mut args = vec!["-p", "spotify"];
    match parts.as_slice() {
        [action @ ("play" | "pause" | "stop" | "next" | "previous")] => args.push(action),
        ["status"] | ["now_playing"] => args.extend([
            "metadata",
            "--format",
            "{{status}} | {{artist}} - {{title}}",
        ]),
        ["volume", level] => {
            let level = level
                .parse::<u8>()
                .ok()
                .filter(|level| *level <= 100)
                .ok_or_else(|| PluginError::InvalidInstruction {
                    plugin: "spotify",
                    message: "volume must be between 0 and 100".into(),
                })?;
            let level = format!("{:.2}", f32::from(level) / 100.0);
            return run("playerctl", &["-p", "spotify", "volume", &level]);
        }
        ["shuffle", state] if matches!(*state, "on" | "off" | "toggle") => {
            args.extend(["shuffle", state])
        }
        _ => {
            return invalid(
                "spotify",
                "expected play, pause, stop, next, previous, status, volume <0-100>, or shuffle <on|off|toggle>",
            );
        }
    }
    run("playerctl", &args)
}

fn system_metrics(instruction: &str) -> Result<String, PluginError> {
    let uptime = fs::read_to_string("/proc/uptime")
        .ok()
        .and_then(|raw| raw.split_whitespace().next()?.parse::<f64>().ok())
        .unwrap_or_default();
    let memory = fs::read_to_string("/proc/meminfo").unwrap_or_default();
    let total = meminfo_kib(&memory, "MemTotal").unwrap_or_default();
    let available = meminfo_kib(&memory, "MemAvailable").unwrap_or_default();
    let used = total.saturating_sub(available);
    let cpu_count = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    match instruction.trim() {
        "ram" => Ok(format!("RAM: {used} KiB used / {total} KiB total")),
        "cpu" => Ok(format!("CPU: {cpu_count} logical cores")),
        "uptime" => Ok(format!("uptime: {} minutes", (uptime / 60.0) as u64)),
        "" | "all" => Ok(format!(
            "RAM: {used} KiB used / {total} KiB total, CPU: {cpu_count} logical cores, uptime: {} minutes",
            (uptime / 60.0) as u64
        )),
        _ => invalid("system_metrics", "expected all, ram, cpu, or uptime"),
    }
}

fn notes_directory() -> Result<PathBuf, PluginError> {
    let path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("Documents")
        .join("Mint_Notes");
    fs::create_dir_all(&path).map_err(|source| PluginError::Note {
        path: path.clone(),
        source,
    })?;
    Ok(path)
}

fn note_path(directory: &Path, name: &str) -> Result<PathBuf, PluginError> {
    let mut name = name.trim().to_owned();
    if !name.ends_with(".md") {
        name.push_str(".md");
    }
    let path = PathBuf::from(&name);
    if path.components().count() != 1
        || !matches!(path.components().next(), Some(Component::Normal(_)))
    {
        return invalid("obsidian", "note name must not contain a path");
    }
    Ok(directory.join(path))
}

fn run(program: &'static str, args: &[&str]) -> Result<String, PluginError> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|source| PluginError::Execute { program, source })?;
    if !output.status.success() {
        return Err(PluginError::Failed {
            program,
            message: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn invalid<T>(plugin: &'static str, message: impl Into<String>) -> Result<T, PluginError> {
    Err(PluginError::InvalidInstruction {
        plugin,
        message: message.into(),
    })
}

fn meminfo_kib(raw: &str, key: &str) -> Option<u64> {
    raw.lines()
        .find(|line| line.starts_with(key))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}

fn timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

async fn gmail(config: &MintConfig, instruction: &str) -> Result<String, PluginError> {
    let token = google_access_token(
        config_value(config, "gmailClientId")?,
        config_value(config, "gmailClientSecret")?,
        config_value(config, "gmailRefreshToken")?,
    )
    .await?;
    let user = config_optional(config, "gmailUserId").unwrap_or("me");
    let input = parse_instruction(instruction, "search");
    match input["action"].as_str().unwrap_or("search") {
        "read" => gmail_read(&token, user, &input).await,
        "draft" => gmail_draft(&token, user, &input).await,
        _ => gmail_search(&token, user, &input).await,
    }
}

async fn gmail_search(token: &str, user: &str, input: &Value) -> Result<String, PluginError> {
    let query = input["query"].as_str().unwrap_or("in:inbox");
    let value: Value = crate::HTTP_CLIENT.clone()
        .get(format!(
            "https://gmail.googleapis.com/gmail/v1/users/{user}/messages"
        ))
        .bearer_auth(token)
        .query(&[("q", query), ("maxResults", "10")])
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    let messages = value["messages"].as_array().cloned().unwrap_or_default();
    Ok(if messages.is_empty() {
        "No Gmail messages found.".into()
    } else {
        format!(
            "Gmail matched message IDs:\n{}",
            messages
                .iter()
                .filter_map(|item| item["id"].as_str())
                .map(|id| format!("- {id}"))
                .collect::<Vec<_>>()
                .join("\n")
        )
    })
}

async fn gmail_read(token: &str, user: &str, input: &Value) -> Result<String, PluginError> {
    let id = input["id"].as_str().ok_or_else(|| PluginError::InvalidInstruction {
        plugin: "gmail",
        message: "missing Gmail message id".into(),
    })?;
    let value: Value = crate::HTTP_CLIENT.clone()
        .get(format!(
            "https://gmail.googleapis.com/gmail/v1/users/{user}/messages/{id}"
        ))
        .bearer_auth(token)
        .query(&[("format", "full")])
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    Ok(format!(
        "Gmail message {id}:\n{}",
        value["snippet"].as_str().unwrap_or("(No readable snippet)")
    ))
}

async fn gmail_draft(token: &str, user: &str, input: &Value) -> Result<String, PluginError> {
    let to = input["to"].as_str().ok_or_else(|| PluginError::InvalidInstruction {
        plugin: "gmail",
        message: "missing Gmail draft recipient".into(),
    })?;
    let subject = input["subject"].as_str().unwrap_or("(No subject)");
    let body = input["body"].as_str().unwrap_or_default();
    let raw = URL_SAFE_NO_PAD.encode(format!(
        "To: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\n\r\n{}",
        sanitize_header(to), sanitize_header(subject), body
    ));
    let value: Value = crate::HTTP_CLIENT.clone()
        .post(format!(
            "https://gmail.googleapis.com/gmail/v1/users/{user}/drafts"
        ))
        .bearer_auth(token)
        .json(&json!({ "message": { "raw": raw } }))
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    Ok(format!(
        "Created Gmail draft {} for {}.",
        value["id"].as_str().unwrap_or("(unknown id)"),
        sanitize_header(to)
    ))
}

async fn calendar(config: &MintConfig, instruction: &str) -> Result<String, PluginError> {
    if instruction.trim().is_empty() || instruction.trim() == "open" {
        return Ok("https://calendar.google.com/".into());
    }
    let token = google_access_token(
        config_value(config, "googleCalendarClientId")?,
        config_value(config, "googleCalendarClientSecret")?,
        config_value(config, "googleCalendarRefreshToken")?,
    )
    .await?;
    let id = config_optional(config, "googleCalendarId").unwrap_or("primary");
    let input = parse_instruction(instruction, "list");
    if input["action"].as_str() == Some("create") {
        return calendar_create(&token, id, &input).await;
    }
    let value: Value = crate::HTTP_CLIENT.clone()
        .get(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{id}/events"
        ))
        .bearer_auth(token)
        .query(&[
            ("singleEvents", "true"),
            ("orderBy", "startTime"),
            ("maxResults", "10"),
        ])
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    Ok(format!(
        "Google Calendar events:\n{}",
        value["items"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|event| format!("- {}", event["summary"].as_str().unwrap_or("(Untitled)")))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

async fn calendar_create(token: &str, id: &str, input: &Value) -> Result<String, PluginError> {
    let summary = input["summary"].as_str().ok_or_else(|| PluginError::InvalidInstruction {
        plugin: "google_calendar",
        message: "missing Calendar event summary".into(),
    })?;
    let start = input["start"].as_str().ok_or_else(|| PluginError::InvalidInstruction {
        plugin: "google_calendar",
        message: "missing Calendar event start".into(),
    })?;
    let end = input["end"].as_str().unwrap_or(start);
    let value: Value = crate::HTTP_CLIENT.clone()
        .post(format!(
            "https://www.googleapis.com/calendar/v3/calendars/{id}/events"
        ))
        .bearer_auth(token)
        .json(&json!({
            "summary": summary, "description": input["description"], "location": input["location"],
            "start": { "dateTime": start }, "end": { "dateTime": end }
        }))
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    Ok(format!(
        "Created Calendar event \"{summary}\".{}",
        value["htmlLink"]
            .as_str()
            .map(|link| format!("\n{link}"))
            .unwrap_or_default()
    ))
}

async fn notion(config: &MintConfig, instruction: &str) -> Result<String, PluginError> {
    let key = config_value(config, "notionApiKey")?;
    let input = parse_instruction(instruction, "create_page");
    match input["action"].as_str().unwrap_or("create_page") {
        "query_database" => notion_query(key, config_value(config, "notionDatabaseId")?).await,
        "append_block" => notion_append(key, &input).await,
        _ => notion_create(key, config, instruction, &input).await,
    }
}

async fn notion_create(
    key: &str,
    config: &MintConfig,
    instruction: &str,
    input: &Value,
) -> Result<String, PluginError> {
    let database = config_value(config, "notionDatabaseId")?;
    let property = config_optional(config, "notionTitleProperty").unwrap_or("Name");
    let title = input["title"]
        .as_str()
        .or_else(|| instruction.lines().next())
        .unwrap_or("Mint Note")
        .trim();
    let content = input["content"].as_str().unwrap_or(instruction);
    let value: Value = crate::HTTP_CLIENT.clone()
        .post("https://api.notion.com/v1/pages")
        .bearer_auth(key)
        .header("Notion-Version", "2022-06-28")
        .json(&json!({
            "parent": { "database_id": database },
            "properties": { (property): { "title": [{ "text": { "content": title } }] } },
            "children": [{ "object": "block", "type": "paragraph", "paragraph": {
                "rich_text": [{ "type": "text", "text": { "content": content } }]
            }}]
        }))
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    Ok(format!(
        "Created Notion page \"{title}\".{}",
        value["url"]
            .as_str()
            .map(|url| format!("\n{url}"))
            .unwrap_or_default()
    ))
}

async fn notion_query(key: &str, database: &str) -> Result<String, PluginError> {
    let value: Value = crate::HTTP_CLIENT.clone()
        .post(format!(
            "https://api.notion.com/v1/databases/{database}/query"
        ))
        .bearer_auth(key)
        .header("Notion-Version", "2022-06-28")
        .json(&json!({ "page_size": 10 }))
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    Ok(format!(
        "Notion matched page URLs:\n{}",
        value["results"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|page| page["url"].as_str())
            .map(|url| format!("- {url}"))
            .collect::<Vec<_>>()
            .join("\n")
    ))
}

async fn notion_append(key: &str, input: &Value) -> Result<String, PluginError> {
    let page = input["pageId"].as_str().ok_or_else(|| PluginError::InvalidInstruction {
        plugin: "notion",
        message: "missing Notion pageId".into(),
    })?;
    let content = input["content"].as_str().ok_or_else(|| PluginError::InvalidInstruction {
        plugin: "notion",
        message: "missing Notion append content".into(),
    })?;
    crate::HTTP_CLIENT.clone()
        .patch(format!("https://api.notion.com/v1/blocks/{page}/children"))
        .bearer_auth(key)
        .header("Notion-Version", "2022-06-28")
        .json(
            &json!({ "children": [{ "object": "block", "type": "paragraph", "paragraph": {
            "rich_text": [{ "type": "text", "text": { "content": content } }]
        }}] }),
        )
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?;
    Ok("Appended block to Notion page.".into())
}

async fn google_access_token(
    client_id: &str,
    secret: &str,
    refresh: &str,
) -> Result<String, PluginError> {
    let value: Value = crate::HTTP_CLIENT.clone()
        .post("https://oauth2.googleapis.com/token")
        .form(&[
            ("client_id", client_id),
            ("client_secret", secret),
            ("refresh_token", refresh),
            ("grant_type", "refresh_token"),
        ])
        .send()
        .await
        .map_err(request_error)?
        .error_for_status()
        .map_err(request_error)?
        .json()
        .await
        .map_err(request_error)?;
    value["access_token"]
        .as_str()
        .map(str::to_owned)
        .ok_or_else(|| PluginError::RequestFailed {
            message: "OAuth token response did not include access_token".into(),
        })
}

fn parse_instruction(instruction: &str, action: &str) -> Value {
    serde_json::from_str(instruction).unwrap_or_else(|_| json!({
        "action": action, "query": instruction.strip_prefix("search ").unwrap_or(instruction).trim(),
        "title": instruction.lines().next().unwrap_or("Mint Note"), "content": instruction
    }))
}

fn config_value<'a>(config: &'a MintConfig, key: &str) -> Result<&'a str, PluginError> {
    config_optional(config, key).ok_or_else(|| PluginError::MissingConfig {
        message: format!("missing config value '{key}'"),
    })
}

fn config_optional<'a>(config: &'a MintConfig, key: &str) -> Option<&'a str> {
    config
        .extra
        .get(key)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
}

fn sanitize_header(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_owned()
}

fn request_error(error: reqwest::Error) -> PluginError {
    PluginError::RequestFailed {
        message: format!("plugin request failed: {error}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blocks_obsidian_path_traversal() {
        let error = note_path(Path::new("/tmp"), "../secret").unwrap_err();
        assert!(matches!(error, PluginError::InvalidInstruction { .. }));
    }

    #[tokio::test]
    async fn rejects_unknown_plugins() {
        let config = MintConfig::default();
        assert!(matches!(
            execute_native_plugin(&config, "missing", "").await,
            Err(PluginError::UnknownPlugin(_))
        ));
    }
}

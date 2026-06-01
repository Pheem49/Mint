use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use mint_core::MintConfig;
use reqwest::Client;
use serde_json::{Value, json};

use crate::discord_rpc;

pub async fn execute_plugin(
    config: &MintConfig,
    name: &str,
    instruction: &str,
) -> Result<String, String> {
    match name {
        "gmail" => gmail(config, instruction).await,
        "google_calendar" => calendar(config, instruction).await,
        "notion" => notion(config, instruction).await,
        "discord" => discord_rpc::set_activity(config, instruction),
        other => {
            mint_core::execute_native_plugin(other, instruction).map_err(|error| error.to_string())
        }
    }
}

async fn gmail(config: &MintConfig, instruction: &str) -> Result<String, String> {
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

async fn gmail_search(token: &str, user: &str, input: &Value) -> Result<String, String> {
    let query = input["query"].as_str().unwrap_or("in:inbox");
    let value: Value = Client::new()
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

async fn gmail_read(token: &str, user: &str, input: &Value) -> Result<String, String> {
    let id = input["id"].as_str().ok_or("missing Gmail message id")?;
    let value: Value = Client::new()
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

async fn gmail_draft(token: &str, user: &str, input: &Value) -> Result<String, String> {
    let to = input["to"]
        .as_str()
        .ok_or("missing Gmail draft recipient")?;
    let subject = input["subject"].as_str().unwrap_or("(No subject)");
    let body = input["body"].as_str().unwrap_or_default();
    let raw = URL_SAFE_NO_PAD.encode(format!(
        "To: {}\r\nSubject: {}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\n\r\n{}",
        sanitize_header(to), sanitize_header(subject), body
    ));
    let value: Value = Client::new()
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

async fn calendar(config: &MintConfig, instruction: &str) -> Result<String, String> {
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
    let value: Value = Client::new()
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

async fn calendar_create(token: &str, id: &str, input: &Value) -> Result<String, String> {
    let summary = input["summary"]
        .as_str()
        .ok_or("missing Calendar event summary")?;
    let start = input["start"]
        .as_str()
        .ok_or("missing Calendar event start")?;
    let end = input["end"].as_str().unwrap_or(start);
    let value: Value = Client::new()
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

async fn notion(config: &MintConfig, instruction: &str) -> Result<String, String> {
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
) -> Result<String, String> {
    let database = config_value(config, "notionDatabaseId")?;
    let property = config_optional(config, "notionTitleProperty").unwrap_or("Name");
    let title = input["title"]
        .as_str()
        .or_else(|| instruction.lines().next())
        .unwrap_or("Mint Note")
        .trim();
    let content = input["content"].as_str().unwrap_or(instruction);
    let value: Value = Client::new()
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

async fn notion_query(key: &str, database: &str) -> Result<String, String> {
    let value: Value = Client::new()
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

async fn notion_append(key: &str, input: &Value) -> Result<String, String> {
    let page = input["pageId"].as_str().ok_or("missing Notion pageId")?;
    let content = input["content"]
        .as_str()
        .ok_or("missing Notion append content")?;
    Client::new()
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
) -> Result<String, String> {
    let value: Value = Client::new()
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
        .ok_or_else(|| "OAuth token response did not include access_token".into())
}

fn parse_instruction(instruction: &str, action: &str) -> Value {
    serde_json::from_str(instruction).unwrap_or_else(|_| json!({
        "action": action, "query": instruction.strip_prefix("search ").unwrap_or(instruction).trim(),
        "title": instruction.lines().next().unwrap_or("Mint Note"), "content": instruction
    }))
}

fn config_value<'a>(config: &'a MintConfig, key: &str) -> Result<&'a str, String> {
    config_optional(config, key).ok_or_else(|| format!("missing config value '{key}'"))
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

fn request_error(error: reqwest::Error) -> String {
    format!("plugin request failed: {error}")
}

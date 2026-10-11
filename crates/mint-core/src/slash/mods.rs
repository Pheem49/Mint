use super::{SlashChoice, SlashResponse};
use serde_json::Value;
use std::path::Path;

// Reuse the shared choice protocol so all surfaces get the same manager.
// Explicit /mods list, enable, disable, etc. retain their text-command behavior.
pub(super) fn picker(root: &Path, chat: &str, rest: &str) -> Option<Result<SlashResponse, String>> {
    let words = shlex::split(rest)?;
    let action = words.first().map(String::as_str).unwrap_or("manage");
    if !matches!(action, "manage" | "review" | "set") {
        return None;
    }
    Some(manage(
        root,
        chat,
        action,
        words.get(1..).unwrap_or_default(),
    ))
}

fn choices(title: String, options: Vec<SlashChoice>) -> SlashResponse {
    SlashResponse::NeedsChoice {
        command: "/mods".into(),
        title,
        options,
    }
}

fn option(label: impl Into<String>, value: impl Into<String>) -> SlashChoice {
    SlashChoice {
        label: label.into(),
        value: value.into(),
    }
}

fn manage(root: &Path, chat: &str, action: &str, args: &[String]) -> Result<SlashResponse, String> {
    let scoped = crate::scoped_chat_id(chat, Some(&root.to_string_lossy()));
    let id = args.first().map(String::as_str).unwrap_or("");
    if action == "set" {
        let state = args
            .get(1)
            .map(String::as_str)
            .ok_or("Choose Enable or Disable from /mods")?;
        match state {
            "on" => {
                let hash = args.get(2).ok_or("Review the current mod revision first")?;
                crate::mods::enable_reviewed(
                    root,
                    &scoped,
                    id,
                    "chat",
                    args.get(3).is_some_and(|v| v == "approve"),
                    Some(hash),
                )?;
            }
            "off" => crate::mods::disable(root, &scoped, id)?,
            _ => return Err("Choose Enable or Disable from /mods".into()),
        }
    }
    let mods = crate::mods::list(root, &scoped)?;
    if id.is_empty() {
        if mods.is_empty() {
            return Ok(SlashResponse::Message { markdown: "**Mods · 0 mods · 0 active**\n\nNo mods yet. Ask Mint to create one in mint-mods/.".into() });
        }
        let active = mods.iter().filter(|m| m["enabled"] == true).count();
        return Ok(choices(
            format!("Mods · {} mods · {active} active", mods.len()),
            mods.iter()
                .map(|m| {
                    option(
                        format!(
                            "{} ({}) · {}",
                            m["name"].as_str().unwrap_or(""),
                            m["id"].as_str().unwrap_or(""),
                            status(m)
                        ),
                        format!("manage {}", m["id"].as_str().unwrap_or("")),
                    )
                })
                .collect(),
        ));
    }
    let item = mods
        .iter()
        .find(|m| m["id"].as_str() == Some(id))
        .ok_or("Unknown mod")?;
    let name = item["name"].as_str().unwrap_or(id);
    let hash = item["hash"].as_str().unwrap_or("");
    let permissions = item["manifest"]["permissions"]
        .as_array()
        .map(|ps| ps.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        .unwrap_or_default();
    if action == "review" {
        if hash.is_empty() || args.get(1).map(String::as_str) != Some(hash) {
            return Err(
                "Mod source changed after review; reopen /mods to review the current revision"
                    .into(),
            );
        }
        return Ok(choices(
            format!(
                "Enable {name}?\nPermissions: {}\nRevision: {hash}\nThis chat only. Read source with /mods source {id}.",
                permissions.join(", ")
            ),
            vec![
                option("Back · keep disabled", format!("manage {id}")),
                option(
                    "Approve permissions & enable",
                    format!("set {id} on {hash} approve"),
                ),
            ],
        ));
    }
    let mut options = Vec::new();
    if item["enabled"] == true {
        options.push(option("Disable for this chat", format!("set {id} off")));
    } else if !hash.is_empty() && item["sourceError"].is_null() {
        options.push(option(
            "Enable for this chat",
            if permissions.is_empty() {
                format!("set {id} on {hash}")
            } else {
                format!("review {id} {hash}")
            },
        ));
    }
    options.push(option("Back to mods", "manage"));
    let mut title = format!(
        "{name} · {}\n{id} · v{} · This chat only",
        status(item),
        item["manifest"]["version"].as_str().unwrap_or("—")
    );
    for key in ["sourceError", "error", "reloadError"] {
        if let Some(error) = item[key].as_str() {
            title.push_str(&format!("\n{error}"));
        }
    }
    Ok(choices(title, options))
}

fn status(item: &Value) -> &'static str {
    if item["error"].is_string() || item["sourceError"].is_string() {
        "Error"
    } else if item["enabled"] == true {
        "Active"
    } else {
        "Off"
    }
}

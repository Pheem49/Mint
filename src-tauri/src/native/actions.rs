use crate::native::system::{SmartContext, run_system_automation, smart_context};
use mint_core::{KnowledgeStore, create_folder, find_paths, load_config};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopAction {
    #[serde(rename = "type")]
    pub kind: String,
    #[serde(default)]
    pub target: String,
    #[serde(default)]
    pub server: String,
    #[serde(default)]
    pub args: Value,
    #[serde(default)]
    pub approved: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActionResult {
    pub success: bool,
    pub message: String,
}

pub fn execute_action(
    config: &mint_core::MintConfig,
    action: DesktopAction,
) -> Result<ActionResult, String> {
    match action.kind.as_str() {
        "none" => Ok(success("no action requested")),
        "system_info" => Ok(success(&format!(
            "os={} arch={} family={}",
            std::env::consts::OS,
            std::env::consts::ARCH,
            std::env::consts::FAMILY
        ))),
        "open_url" => {
            if !(action.target.starts_with("https://")
                || action.target.starts_with("http://")
                || action.target.starts_with("file://"))
            {
                return Err("only http, https, and file URLs may be opened".into());
            }
            open_url_in_system_browser(&action.target)?;
            Ok(success("opened URL in the system browser"))
        }
        "search" => {
            let query = action.target.trim();
            if query.is_empty() {
                return Err("search query is required".into());
            }
            let url = format!("https://www.google.com/search?q={}", encode_query(query));
            spawn_detached("xdg-open", &[&url])?;
            Ok(success("opened web search"))
        }
        "open_app" => {
            let app = action.target.trim();
            if app.is_empty()
                || !app
                    .chars()
                    .all(|char| char.is_ascii_alphanumeric() || matches!(char, '-' | '_' | '.'))
            {
                return Err("application name contains unsupported characters".into());
            }
            spawn_detached(app, &[])?;
            Ok(success("opened application"))
        }
        "clipboard_write" => Err("clipboard actions are handled by the renderer".into()),
        "mcp_tool" => mint_core::call_mcp_tool(config, &action.server, &action.target, action.args)
            .map(|result| success(&result.to_string()))
            .map_err(|error| error.to_string()),
        "system_automation" => {
            run_system_automation(&action.target, action.approved).map(|message| success(&message))
        }
        "create_folder" => create_folder(std::path::Path::new(&action.target), config)
            .map(|path| success(&format!("created {}", path.display())))
            .map_err(|error| error.to_string()),
        "find_path" => {
            let roots = action.args["roots"]
                .as_array()
                .map(|roots| {
                    roots
                        .iter()
                        .filter_map(Value::as_str)
                        .map(PathBuf::from)
                        .collect::<Vec<_>>()
                })
                .filter(|roots| !roots.is_empty())
                .unwrap_or_else(|| {
                    let mut roots =
                        vec![std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))];
                    if let Some(home) = dirs::home_dir() {
                        roots.push(home);
                    }
                    roots
                });
            let limit = action.args["limit"].as_u64().unwrap_or(20).min(100) as usize;
            serde_json::to_string(&find_paths(&action.target, &roots, limit, config))
                .map(|message| success(&message))
                .map_err(|error| error.to_string())
        }
        "learn_file" => KnowledgeStore::open_default()
            .map_err(|error| error.to_string())?
            .index_file(std::path::Path::new(&action.target), config)
            .map(|chunks| success(&format!("indexed {chunks} knowledge chunks")))
            .map_err(|error| error.to_string()),
        other => Err(format!(
            "desktop action '{other}' has not migrated to the allowlisted Rust executor"
        )),
    }
}
pub fn integration_status(config: &mint_core::MintConfig) -> Value {
    let mcp_servers = config
        .extra
        .get("mcpServers")
        .and_then(Value::as_object)
        .map(|servers| servers.keys().cloned().collect::<Vec<_>>())
        .unwrap_or_default();
    json!({
        "automation": {
            "supportedActions": ["open_url", "open_app", "search", "system_info", "system_automation", "find_path", "create_folder", "learn_file"],
            "approvalRequired": true
        },
        "mcp": {
            "configuredServers": mcp_servers,
            "execution": "native-stdio"
        },
        "plugins": {
            "migrated": ["desktop-actions", "dev_tools", "docker", "obsidian", "spotify", "system_metrics"]
        }
    })
}
fn success(message: &str) -> ActionResult {
    ActionResult {
        success: true,
        message: message.into(),
    }
}

fn open_url_in_system_browser(url: &str) -> Result<(), String> {
    #[cfg(target_os = "linux")]
    {
        return spawn_detached("xdg-open", &[url]);
    }
    #[cfg(target_os = "macos")]
    {
        return spawn_detached("open", &[url]);
    }
    #[cfg(target_os = "windows")]
    {
        return spawn_detached("rundll32", &["url.dll,FileProtocolHandler", url]);
    }
    #[allow(unreachable_code)]
    Err("opening URLs is unsupported on this platform".into())
}

fn spawn_detached(program: &str, args: &[&str]) -> Result<(), String> {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("unable to start '{program}': {error}"))
}

fn encode_query(query: &str) -> String {
    query
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                (byte as char).to_string()
            }
            b' ' => "+".into(),
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_search_queries_for_urls() {
        assert_eq!(encode_query("mint cli"), "mint+cli");
        assert_eq!(encode_query("a/b"), "a%2Fb");
    }
}

#[tauri::command]
pub(crate) fn run_desktop_action(action: DesktopAction) -> Result<ActionResult, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    execute_action(&config, action)
}

#[tauri::command]
pub(crate) async fn get_smart_context() -> SmartContext {
    smart_context().await
}

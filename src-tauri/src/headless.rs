use std::{path::Path, time::Duration};

use mint_core::{
    ChatRequest, CodeEdit, KnowledgeStore, MintConfig, Task, TaskStore, load_config,
    propose_code_edits, send_chat,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use crate::browser::{click, navigate, read_page_text};

const MAX_STEPS: usize = 10;
const SYSTEM_PROMPT: &str = r#"You are Mint's native background task agent. Return only JSON:
{"thought":"short progress note","action":"done|propose_folder|propose_write_file|open_url|browser_read|browser_click|knowledge_search|propose_bash","target":"path, URL, selector, query, command, or final result","data":"optional file content"}
Use only one action per response. Background tasks never mutate the filesystem or execute shell commands. Use propose_folder, propose_write_file, and propose_bash so Mint can record a proposal for explicit user approval."#;

#[derive(Debug, Deserialize)]
struct AgentAction {
    #[serde(default)]
    thought: String,
    action: String,
    #[serde(default)]
    target: String,
    #[serde(default)]
    data: String,
}

pub fn start_headless_queue(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        if let Ok(store) = TaskStore::open_default() {
            let _ = store.resume_running();
        }
        loop {
            tokio::time::sleep(Duration::from_secs(15)).await;
            let Ok(config) = load_config() else {
                continue;
            };
            if config
                .extra
                .get("enableHeadlessTaskQueue")
                .and_then(Value::as_bool)
                != Some(true)
            {
                continue;
            }
            let _ = run_next_task(&app).await;
        }
    });
}

pub async fn run_next_task(app: &AppHandle) -> Result<Option<Task>, String> {
    let store = TaskStore::open_default().map_err(|error| error.to_string())?;
    let Some(task) = store.pending().map_err(|error| error.to_string())? else {
        return Ok(None);
    };
    store
        .update_status(&task.id, "running", None)
        .map_err(|error| error.to_string())?;
    checkpoint(&store, &task.id, "started", &task.description)?;
    emit(
        app,
        &format!("Started queued task: {}", task.description),
        "info",
    );

    match execute_task(&store, &task).await {
        Ok(result) => {
            store
                .add_artifact(
                    &task.id,
                    json!({ "type": "final_result", "content": result }),
                )
                .map_err(|error| error.to_string())?;
            let completed = store
                .update_status(&task.id, "completed", Some(Value::String(result.clone())))
                .map_err(|error| error.to_string())?;
            emit(app, &format!("Queued task completed: {result}"), "info");
            Ok(completed)
        }
        Err(error) => {
            let failed = store
                .fail_with_retry(&task.id, &error)
                .map_err(|store_error| store_error.to_string())?;
            emit(app, &format!("Queued task failed: {error}"), "warning");
            Ok(failed)
        }
    }
}

async fn execute_task(store: &TaskStore, task: &Task) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    let mut observation = format!("Task: {}\nChoose the first action.", task.description);
    for step in 1..=MAX_STEPS {
        let response = send_chat(
            &config,
            &ChatRequest {
                message: observation,
                system_instruction: SYSTEM_PROMPT.into(),
                image_data_uri: None,
                audio_data_uri: None,
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        let action: AgentAction = parse_action(&response.text)?;
        store
            .add_checkpoint(
                &task.id,
                json!({
                    "phase": "native_agent_step",
                    "step": step,
                    "thought": action.thought,
                    "action": action.action,
                    "target": action.target,
                }),
            )
            .map_err(|error| error.to_string())?;
        if action.action == "done" {
            return Ok(action.target);
        }
        observation = execute_action(&config, store, task, step, &action).await?;
        checkpoint(store, &task.id, "observation", &observation)?;
    }
    Err(format!(
        "task reached the native agent limit of {MAX_STEPS} steps"
    ))
}

async fn execute_action(
    config: &MintConfig,
    store: &TaskStore,
    task: &Task,
    step: usize,
    action: &AgentAction,
) -> Result<String, String> {
    match action.action.as_str() {
        "propose_folder" => {
            store
                .add_artifact(
                    &task.id,
                    json!({ "type": "folder_proposal", "path": action.target, "description": format!("Proposed by native task step {step}; explicit approval required") }),
                )
                .map_err(|error| error.to_string())?;
            Ok(format!(
                "folder proposal recorded but not applied: {}",
                action.target
            ))
        }
        "propose_write_file" => {
            let root = std::env::current_dir().map_err(|error| error.to_string())?;
            let proposal = propose_code_edits(
                &root,
                &[CodeEdit {
                    path: Path::new(&action.target).to_path_buf(),
                    content: action.data.clone(),
                }],
                config,
            )
            .map_err(|error| error.to_string())?;
            store
                .add_artifact(
                    &task.id,
                    json!({ "type": "file_edit_proposal", "proposal": proposal, "description": format!("Proposed by native task step {step}; explicit approval required") }),
                )
                .map_err(|error| error.to_string())?;
            Ok(format!(
                "file edit proposal recorded but not applied: {}",
                action.target
            ))
        }
        "open_url" => navigate(config, &action.target).await,
        "browser_read" => read_page_text(config).await,
        "browser_click" => click(config, &action.target).await,
        "knowledge_search" => {
            let hits = KnowledgeStore::open_default()
                .map_err(|error| error.to_string())?
                .search(&action.target, 5)
                .map_err(|error| error.to_string())?;
            serde_json::to_string(&hits).map_err(|error| error.to_string())
        }
        "propose_bash" => Ok(format!(
            "command proposal recorded but not executed: {}",
            action.target
        )),
        other => Err(format!("unsupported native background action '{other}'")),
    }
}

fn checkpoint(store: &TaskStore, id: &str, phase: &str, message: &str) -> Result<(), String> {
    store
        .add_checkpoint(id, json!({ "phase": phase, "message": message }))
        .map(|_| ())
        .map_err(|error| error.to_string())
}

fn parse_action(raw: &str) -> Result<AgentAction, String> {
    serde_json::from_str(raw)
        .or_else(|_| {
            let start = raw
                .find('{')
                .ok_or_else(|| "missing JSON object".to_string())?;
            let end = raw
                .rfind('}')
                .ok_or_else(|| "missing JSON object".to_string())?;
            serde_json::from_str(&raw[start..=end]).map_err(|error| error.to_string())
        })
        .map_err(|error| format!("provider did not return a valid agent action: {error}"))
}

fn emit(app: &AppHandle, message: &str, kind: &str) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.emit(
            "proactive-notification",
            json!({ "message": message, "type": kind }),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_wrapped_in_provider_text() {
        let action = parse_action("```json\n{\"action\":\"done\",\"target\":\"ok\"}\n```").unwrap();
        assert_eq!(action.action, "done");
        assert_eq!(action.target, "ok");
    }
}

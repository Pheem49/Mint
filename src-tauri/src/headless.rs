use std::{path::Path, time::Duration};

use mint_core::{
    CodeEdit, KnowledgeStore, MintConfig, Task, TaskStore, load_config, parse_agent_json,
    propose_code_edits, run_agent_loop,
};
use serde::Deserialize;
use serde_json::{Value, json};
use tauri::{AppHandle, Emitter, Manager};

use mint_core::browser::{click, navigate, read_page_text, type_text};

const MAX_STEPS: usize = 20;
const SYSTEM_PROMPT: &str = r#"You are Mint's native background task agent. Return only JSON:
{"thought":"short progress note","action":"done|propose_folder|propose_write_file|open_url|browser_read|browser_click|browser_type|knowledge_search|propose_bash","target":"path, URL, selector, query, command, or final result","data":"optional file content"}
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
    let observer_store = store.clone();
    let observer_task = task.clone();
    let executor_store = store.clone();
    let executor_task = task.clone();
    let executor_config = config.clone();
    run_agent_loop(
        &config,
        SYSTEM_PROMPT,
        format!("Task: {}\nChoose the first action.", task.description),
        MAX_STEPS,
        |raw| parse_agent_json(raw).map_err(|error| error.to_string()),
        |action: &AgentAction| (action.action == "done").then(|| action.target.clone()),
        move |step, action| {
            let store = executor_store.clone();
            let task = executor_task.clone();
            let config = executor_config.clone();
            Box::pin(async move {
                let observation = execute_action(&config, &store, &task, step, &action).await?;
                checkpoint(&store, &task.id, "observation", &observation)?;
                Ok(observation)
            })
        },
        move |step, action| {
            observer_store
                .add_checkpoint(
                    &observer_task.id,
                    json!({
                        "phase": "native_agent_step",
                        "step": step,
                        "thought": action.thought,
                        "action": action.action,
                        "target": action.target,
                    }),
                )
                .map(|_| ())
                .map_err(|error| error.to_string())
        },
    )
    .await
    .map_err(|error| error.to_string())
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
        "browser_type" => type_text(config, &action.target, &action.data).await,
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
        let action: AgentAction =
            parse_agent_json("```json\n{\"action\":\"done\",\"target\":\"ok\"}\n```").unwrap();
        assert_eq!(action.action, "done");
        assert_eq!(action.target, "ok");
    }
}

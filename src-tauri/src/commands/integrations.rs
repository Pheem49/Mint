use crate::commands::workspace::workspace_root;
use crate::native::integrations::{channel_inventory, list_plugins};
use crate::native::plugins::execute_plugin;
use mint_core::{
    CronJob, CronJobDraft, CronStore, MemoryStore, SubagentDefinition, SubagentDraft,
    delete_subagent as core_delete_subagent, list_subagents as core_list_subagents, load_config,
    reauth_mcp_server as core_reauth_mcp_server, save_subagent as core_save_subagent,
};
use serde_json::Value;

type LearnedSkillDto = mint_core::skill_inventory::AvailableSkill;

/// Re-runs a configured MCP server's OAuth login in the foreground (fixes an
/// expired/invalid refresh token, e.g. `invalid_grant` from a Gmail MCP
/// server). The underlying core call is blocking (spawns a child process and
/// waits on it while streaming its output), so it runs on a blocking thread
/// pool task rather than the async runtime.
#[tauri::command]
pub(crate) async fn reauth_mcp_server(server_name: String) -> Result<bool, String> {
    tokio::task::spawn_blocking(move || core_reauth_mcp_server(&server_name))
        .await
        .map_err(|error| format!("reauth task failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn list_mcp_server_tools(name: String) -> Result<Vec<String>, String> {
    tokio::task::spawn_blocking(move || mint_core::mcp_server_tool_names(&name))
        .await
        .map_err(|error| format!("list-tools task failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn test_mcp_connection(
    url: String,
    headers: Option<std::collections::BTreeMap<String, String>>,
) -> Result<serde_json::Value, String> {
    tokio::task::spawn_blocking(move || mint_core::test_remote_mcp_connection(&url, headers))
        .await
        .map_err(|error| format!("test-mcp-connection task failed: {error}"))?
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn read_mcp_artifact(
    id: String,
    chat_id: Option<String>,
) -> Result<String, String> {
    tokio::task::spawn_blocking(move || {
        use base64::Engine as _;
        let (entry, bytes) = mint_core::integrations::mcp_result::read_artifact(&id)?;
        if chat_id.as_deref() != Some(entry.chat_id.as_str()) {
            return Err("Artifact unavailable in this conversation".into());
        }
        let mime = if entry.kind == "image" {
            entry.mime_type
        } else {
            "application/octet-stream".into()
        };
        Ok(format!(
            "data:{mime};base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        ))
    })
    .await
    .map_err(|e| e.to_string())?
}

#[tauri::command]
pub(crate) fn list_learned_skills(
    workspace_path: Option<String>,
) -> Result<Vec<LearnedSkillDto>, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    let root = workspace_root(workspace_path.as_deref()).ok();
    mint_core::skill_inventory::list_available_skills(&store, root.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn add_learned_skill(name: String, content: String) -> Result<LearnedSkillDto, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    let skill = store
        .add_learned_skill(&name, "ui_manual", &content)
        .map_err(|e| e.to_string())?;
    Ok(skill.into())
}

#[tauri::command]
pub(crate) fn delete_learned_skill(name: String) -> Result<usize, String> {
    let store = MemoryStore::open_default().map_err(|e| e.to_string())?;
    store.delete_learned_skill(&name).map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn list_subagents(
    workspace_path: Option<String>,
) -> Result<Vec<SubagentDefinition>, String> {
    let root = workspace_root(workspace_path.as_deref()).ok();
    Ok(core_list_subagents(root.as_deref()))
}

#[tauri::command]
pub(crate) fn save_subagent(
    draft: SubagentDraft,
    workspace_path: Option<String>,
) -> Result<SubagentDefinition, String> {
    let root = workspace_root(workspace_path.as_deref()).ok();
    core_save_subagent(&draft, root.as_deref())
}

#[tauri::command]
pub(crate) fn delete_subagent(source_path: String) -> Result<(), String> {
    core_delete_subagent(&source_path)
}

#[tauri::command]
pub(crate) fn list_cron_jobs() -> Result<Vec<CronJob>, String> {
    CronStore::open_default()
        .and_then(|store| store.list())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn add_cron_job(draft: CronJobDraft) -> Result<CronJob, String> {
    let store = CronStore::open_default().map_err(|e| e.to_string())?;
    store
        .add(draft.name, draft.schedule, draft.task, draft.workspace)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn remove_cron_job(id: String) -> Result<bool, String> {
    CronStore::open_default()
        .and_then(|store| store.remove(&id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn set_cron_job_enabled(id: String, enabled: bool) -> Result<Option<CronJob>, String> {
    CronStore::open_default()
        .and_then(|store| store.set_enabled(&id, enabled))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub(crate) fn get_integration_inventory() -> Result<Value, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    Ok(serde_json::json!({
        "mcpServers": mint_core::configured_mcp_servers(&config)
            .map_err(|error| error.to_string())?
            .keys()
            .collect::<Vec<_>>(),
        "plugins": list_plugins(&config),
        "channels": channel_inventory(&config)
    }))
}

#[tauri::command]
pub(crate) async fn run_native_plugin(name: String, instruction: String) -> Result<String, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    execute_plugin(&config, &name, &instruction).await
}

use crate::native::actions::integration_status;
use crate::native::updater::{
    AvailableUpdate, UpdateChannelStatus, check as check_update, install as install_update,
    status as updater_status,
};
use crate::native::windows::{close_window, open_desktop_window, position_widget};
use mint_core::{MintConfig, classify_shell_command, config_path, load_config, save_config};
use serde::Serialize;
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct RuntimeStatus {
    backend: &'static str,
    config_path: String,
    active_provider: String,
    active_model: String,
    available_providers: Vec<String>,
    integrations: Value,
}

#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DetectedTools {
    pub docker: bool,
    pub git: bool,
    pub gh: bool,
    pub node: bool,
}

#[tauri::command]
pub(crate) fn get_runtime_status() -> Result<RuntimeStatus, String> {
    let config = load_config().map_err(|error| error.to_string())?;
    Ok(RuntimeStatus {
        backend: "rust",
        config_path: config_path()
            .map_err(|error| error.to_string())?
            .display()
            .to_string(),
        active_provider: config.ai_provider.clone(),
        active_model: config.active_model().to_string(),
        available_providers: config.available_providers(),
        integrations: integration_status(&config),
    })
}

#[tauri::command]
pub(crate) fn get_config() -> Result<MintConfig, String> {
    load_config().map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) fn get_updater_status() -> Result<UpdateChannelStatus, String> {
    Ok(updater_status(
        &load_config().map_err(|error| error.to_string())?,
    ))
}

#[tauri::command]
pub(crate) async fn check_for_updates(app: AppHandle) -> Result<AvailableUpdate, String> {
    check_update(&app, &load_config().map_err(|error| error.to_string())?).await
}

#[tauri::command]
pub(crate) async fn install_available_update(
    app: AppHandle,
    approved: bool,
) -> Result<String, String> {
    install_update(
        &app,
        &load_config().map_err(|error| error.to_string())?,
        approved,
    )
    .await
}

#[tauri::command]
pub(crate) fn update_config(app: AppHandle, config: MintConfig) -> Result<(), String> {
    save_config(&config).map_err(|error| error.to_string())?;
    let _ = app.emit("settings-changed", &config);
    if config.show_desktop_widget {
        let _ = open_desktop_window(&app, "widget");
        position_widget(&app);
    } else if app.get_webview_window("widget").is_some() {
        let _ = close_window(&app, "widget");
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn set_active_model(
    app: AppHandle,
    provider: String,
    model: Option<String>,
) -> Result<String, String> {
    let mut config = load_config().map_err(|error| error.to_string())?;
    let display_name = config
        .set_active_model(&provider, model.as_deref())
        .map_err(|error| error.to_string())?;
    let _ = app.emit("settings-changed", &config);
    Ok(display_name)
}

#[tauri::command]
pub(crate) fn inspect_shell_command(command: String) -> mint_core::ShellClassification {
    classify_shell_command(&command)
}

#[tauri::command]
pub(crate) async fn detect_system_tools() -> Result<DetectedTools, String> {
    Ok(DetectedTools {
        docker: mint_core::config::which("docker"),
        git: mint_core::config::which("git"),
        gh: mint_core::config::which("gh"),
        node: mint_core::config::which("node"),
    })
}

/// Fetch the live model list for `provider` from its API.
///
/// Returns the provider list with the same preset fallback policy
/// used by the CLI picker. The UI form can supply an API key not yet saved to
/// config.
#[tauri::command]
pub(crate) async fn fetch_provider_models(
    provider: String,
    api_key: String,
    base_url: Option<String>,
) -> Result<Vec<String>, String> {
    let config = load_config().map_err(|e| e.to_string())?;
    Ok(
        mint_core::slash::models::model_options_for_provider_with_credentials_async(
            &config,
            &provider,
            &api_key,
            base_url.as_deref(),
        )
        .await,
    )
}

use mint_core::MintConfig;
use serde::Serialize;
use tauri::{AppHandle, Runtime};
use tauri_plugin_updater::UpdaterExt;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateChannelStatus {
    pub current_version: &'static str,
    pub configured: bool,
    pub endpoint: String,
    pub public_key_configured: bool,
    pub automatic_install: bool,
    pub message: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AvailableUpdate {
    pub current_version: String,
    pub version: String,
    pub notes: Option<String>,
    pub available: bool,
}

pub fn status(config: &MintConfig) -> UpdateChannelStatus {
    let endpoint = string_value(config, "updaterEndpoint");
    let public_key_configured = !string_value(config, "updaterPublicKey").trim().is_empty();
    UpdateChannelStatus {
        current_version: env!("CARGO_PKG_VERSION"),
        configured: !endpoint.trim().is_empty() && public_key_configured,
        endpoint,
        public_key_configured,
        automatic_install: false,
        message: "Signed Tauri update channel status only; automatic installation is disabled.",
    }
}

pub async fn check<R: Runtime>(
    app: &AppHandle<R>,
    config: &MintConfig,
) -> Result<AvailableUpdate, String> {
    let updater = updater(app, config)?;
    let current_version = env!("CARGO_PKG_VERSION").to_owned();
    match updater.check().await.map_err(|error| error.to_string())? {
        Some(update) => Ok(AvailableUpdate {
            current_version,
            version: update.version.clone(),
            notes: update.body.clone(),
            available: true,
        }),
        None => Ok(AvailableUpdate {
            version: current_version.clone(),
            current_version,
            notes: None,
            available: false,
        }),
    }
}

pub async fn install<R: Runtime>(
    app: &AppHandle<R>,
    config: &MintConfig,
    approved: bool,
) -> Result<String, String> {
    require_install_approval(approved)?;
    let Some(update) = updater(app, config)?
        .check()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok("Mint is already up to date.".into());
    };
    let version = update.version.clone();
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|error| error.to_string())?;
    Ok(format!(
        "Installed Mint {version}. Restart Mint to use the new version."
    ))
}

fn require_install_approval(approved: bool) -> Result<(), String> {
    approved
        .then_some(())
        .ok_or_else(|| "update installation requires explicit user approval".into())
}

fn updater<R: Runtime>(
    app: &AppHandle<R>,
    config: &MintConfig,
) -> Result<tauri_plugin_updater::Updater, String> {
    let endpoint = string_value(config, "updaterEndpoint");
    let public_key = string_value(config, "updaterPublicKey");
    if endpoint.trim().is_empty() || public_key.trim().is_empty() {
        return Err(
            "configure updaterEndpoint and updaterPublicKey before checking updates".into(),
        );
    }
    app.updater_builder()
        .pubkey(public_key)
        .endpoints(vec![
            endpoint
                .parse()
                .map_err(|error| format!("invalid updater endpoint: {error}"))?,
        ])
        .map_err(|error| error.to_string())?
        .build()
        .map_err(|error| error.to_string())
}

fn string_value(config: &MintConfig, key: &str) -> String {
    config
        .extra
        .get(key)
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requires_endpoint_and_public_key_without_enabling_installation() {
        let mut config = MintConfig::default();
        config.extra.insert(
            "updaterEndpoint".into(),
            "https://updates.example.com/latest.json".into(),
        );
        config
            .extra
            .insert("updaterPublicKey".into(), "RWQexample".into());
        let status = status(&config);
        assert!(status.configured);
        assert!(!status.automatic_install);
    }

    #[test]
    fn rejects_install_without_explicit_approval() {
        assert_eq!(
            require_install_approval(false).unwrap_err(),
            "update installation requires explicit user approval"
        );
    }
}

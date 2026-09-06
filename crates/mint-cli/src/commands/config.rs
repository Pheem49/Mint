use anyhow::Result;
use clap::Subcommand;
use mint_core::{
    MintConfig, config_path, docker_available, initialize_config, load_config,
    sandbox_availability, set_config_value,
};

use crate::{active_model, onboard, setup, updater};

#[derive(Debug, Subcommand)]
pub enum ConfigCommand {
    /// Create the native config file and fill missing runtime defaults.
    Init,
    /// Print the config file path.
    Path,
    /// Print the config as JSON.
    Show,
    /// Set one JSON-compatible config value.
    Set { key: String, value: String },
    /// Show configured native providers and integrations.
    Doctor,
}

pub(crate) fn configured(config: &mint_core::MintConfig, keys: &[&str]) -> bool {
    keys.iter().all(|key| {
        config
            .extra
            .get(*key)
            .and_then(serde_json::Value::as_str)
            .is_some_and(|value| !value.trim().is_empty())
    })
}

pub fn handle_status(config: &MintConfig) -> Result<()> {
    println!("Mint native CLI");
    println!("provider: {}", config.ai_provider);
    println!("model: {}", active_model(&config.ai_provider, config));
    println!("config: {}", config_path()?.display());
    Ok(())
}

pub fn handle_config(command: ConfigCommand) -> Result<()> {
    match command {
        ConfigCommand::Init => {
            initialize_config()?;
            println!("{}", config_path()?.display());
        }
        ConfigCommand::Path => println!("{}", config_path()?.display()),
        ConfigCommand::Show => {
            println!("{}", serde_json::to_string_pretty(&load_config()?)?)
        }
        ConfigCommand::Set { key, value } => {
            let value = serde_json::from_str(&value).unwrap_or(serde_json::Value::String(value));
            println!(
                "{}",
                serde_json::to_string_pretty(&set_config_value(&key, value)?)?
            );
        }
        ConfigCommand::Doctor => {
            let config = load_config()?;
            println!(
                "{}",
                serde_json::to_string_pretty(&serde_json::json!({
                    "configPath": config_path()?,
                    "activeProvider": config.ai_provider,
                    "availableProviders": config.available_providers(),
                    "headlessTaskQueue": config.extra["enableHeadlessTaskQueue"],
                    "sandbox": {
                        "mode": config.sandbox_mode,
                        "command": config.sandbox_command,
                        "availability": sandbox_availability(&config),
                    },
                    "dockerSandbox": {
                        "backend": config.sandbox_backend,
                        "image": config.docker_sandbox_image,
                        "available": docker_available(),
                    },
                    "updater": {
                        "enabled": config.extra["enableAutoUpdate"],
                        "endpointConfigured": configured(&config, &["updaterEndpoint"]),
                        "publicKeyConfigured": configured(&config, &["updaterPublicKey"]),
                        "automaticInstall": false,
                    },
                    "channels": {
                        "telegram": configured(&config, &["telegramBotToken"]),
                        "discord": configured(&config, &["discordBotToken"]),
                        "slack": configured(&config, &["slackBotToken", "slackAppToken"]),
                        "line": configured(&config, &["lineChannelAccessToken", "lineChannelSecret"]),
                        "whatsappCloud": configured(&config, &["whatsappCloudAccessToken", "whatsappPhoneNumberId", "whatsappVerifyToken"]),
                    },
                    "plugins": {
                        "gmail": configured(&config, &["gmailClientId", "gmailClientSecret", "gmailRefreshToken"]),
                        "googleCalendar": configured(&config, &["googleCalendarClientId", "googleCalendarClientSecret", "googleCalendarRefreshToken"]),
                        "notion": configured(&config, &["notionApiKey"]),
                    }
                }))?
            );
        }
    }
    Ok(())
}

pub fn handle_providers() -> Result<()> {
    for provider in load_config()?.available_providers() {
        println!("{provider}");
    }
    Ok(())
}

pub fn handle_update(check: bool, dry_run: bool, approve: bool) -> Result<()> {
    updater::run(check, dry_run, approve)
}

pub async fn handle_onboard() -> Result<()> {
    onboard::run().await
}

pub async fn handle_setup() -> Result<()> {
    if let Some(target) = setup::run().await? {
        crate::commands::agent::launch_mint_target(target, false).await?;
    }
    Ok(())
}

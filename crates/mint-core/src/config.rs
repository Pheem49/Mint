use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("unable to determine the user config directory")]
    ConfigDirectoryUnavailable,
    #[error("unable to create config directory {path}: {source}")]
    CreateDirectory {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to read config file {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("unable to parse config file {path}: {source}")]
    Parse {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("unable to serialize config: {0}")]
    Serialize(serde_json::Error),
    #[error("unable to write config file {path}: {source}")]
    Write {
        path: PathBuf,
        source: std::io::Error,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct MintConfig {
    pub theme: String,
    pub accent_color: String,
    pub language: String,
    pub assistant_mode: String,
    pub ai_provider: String,
    pub api_key: String,
    pub gemini_model: String,
    pub anthropic_api_key: String,
    pub anthropic_model: String,
    pub openai_api_key: String,
    pub openai_model: String,
    pub openrouter_api_key: String,
    pub openrouter_model: String,
    pub deepseek_api_key: String,
    pub deepseek_model: String,
    pub hf_api_key: String,
    pub hf_model: String,
    pub local_api_base_url: String,
    pub local_model_name: String,
    pub ollama_host: String,
    pub ollama_model: String,
    pub show_desktop_widget: bool,
    pub safety_enabled: bool,
    pub sandbox_mode: String,
    pub sandbox_command: String,
    pub allowed_read_paths: Vec<PathBuf>,
    pub allowed_write_paths: Vec<PathBuf>,
    pub blocked_paths: Vec<PathBuf>,
    pub blocked_file_names: Vec<String>,
    pub disabled_tools: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl Default for MintConfig {
    fn default() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
        let current_directory = std::env::current_dir().unwrap_or_else(|_| home.clone());
        let allowed_paths = vec![
            home.clone(),
            current_directory,
            home.join("Desktop"),
            home.join("Documents"),
            home.join("Downloads"),
            home.join("Pictures"),
            home.join("Music"),
            home.join("Videos"),
        ];
        Self {
            theme: "dark".into(),
            accent_color: "#8b5cf6".into(),
            language: "th-TH".into(),
            assistant_mode: "chat".into(),
            ai_provider: "gemini".into(),
            api_key: String::new(),
            gemini_model: "gemini-2.5-flash".into(),
            anthropic_api_key: String::new(),
            anthropic_model: "claude-3-5-sonnet-latest".into(),
            openai_api_key: String::new(),
            openai_model: "gpt-4o".into(),
            openrouter_api_key: String::new(),
            openrouter_model: "openai/gpt-4o-mini".into(),
            deepseek_api_key: String::new(),
            deepseek_model: "deepseek-v4-flash".into(),
            hf_api_key: String::new(),
            hf_model: "meta-llama/Meta-Llama-3-8B-Instruct".into(),
            local_api_base_url: String::new(),
            local_model_name: "local-model".into(),
            ollama_host: String::new(),
            ollama_model: "llama3:latest".into(),
            show_desktop_widget: true,
            safety_enabled: true,
            sandbox_mode: "prefer".into(),
            sandbox_command: default_sandbox_command().into(),
            allowed_read_paths: allowed_paths.clone(),
            allowed_write_paths: allowed_paths,
            blocked_paths: vec![
                home.join(".ssh"),
                home.join(".gnupg"),
                home.join(".config/mint/mint-config.json"),
                home.join(".mint/mint-config.json"),
            ],
            blocked_file_names: vec![".env".into(), "id_rsa".into(), "id_ed25519".into()],
            disabled_tools: Vec::new(),
            extra: runtime_extra_defaults(),
        }
    }
}

impl MintConfig {
    pub fn available_providers(&self) -> Vec<&'static str> {
        let mut providers = Vec::new();
        if has_value(&self.anthropic_api_key) {
            providers.push("anthropic");
        }
        if has_value(&self.openai_api_key) {
            providers.push("openai");
        }
        if has_value(&self.openrouter_api_key) {
            providers.push("openrouter");
        }
        if has_value(&self.deepseek_api_key) {
            providers.push("deepseek");
        }
        if has_value(&self.api_key) {
            providers.push("gemini");
        }
        if has_value(&self.hf_api_key) {
            providers.push("huggingface");
        }
        if has_value(&self.local_api_base_url) {
            providers.push("local_openai");
        }
        providers.push("ollama");
        providers
    }
}

pub fn config_path() -> Result<PathBuf, ConfigError> {
    dirs::config_dir()
        .map(|directory| directory.join("mint").join("mint-config.json"))
        .ok_or(ConfigError::ConfigDirectoryUnavailable)
}

pub fn load_config() -> Result<MintConfig, ConfigError> {
    load_config_from(&config_path()?)
}

pub fn initialize_config() -> Result<MintConfig, ConfigError> {
    let config = load_config()?;
    save_config(&config)?;
    Ok(config)
}

pub fn save_config(config: &MintConfig) -> Result<(), ConfigError> {
    save_config_to(&config_path()?, config)
}

pub fn set_config_value(key: &str, value: Value) -> Result<MintConfig, ConfigError> {
    let mut raw = serde_json::to_value(load_config()?).map_err(ConfigError::Serialize)?;
    raw.as_object_mut()
        .expect("MintConfig always serializes to an object")
        .insert(key.to_owned(), value);
    let config = serde_json::from_value(raw).map_err(ConfigError::Serialize)?;
    save_config(&config)?;
    Ok(config)
}

fn load_config_from(path: &Path) -> Result<MintConfig, ConfigError> {
    if !path.exists() {
        let config = MintConfig::default();
        save_config_to(path, &config)?;
        return Ok(config);
    }

    let raw = fs::read_to_string(path).map_err(|source| ConfigError::Read {
        path: path.to_path_buf(),
        source,
    })?;
    let mut config: MintConfig =
        serde_json::from_str(&raw).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })?;
    for (key, value) in runtime_extra_defaults() {
        config.extra.entry(key).or_insert(value);
    }
    Ok(config)
}

fn save_config_to(path: &Path, config: &MintConfig) -> Result<(), ConfigError> {
    if let Some(directory) = path.parent() {
        fs::create_dir_all(directory).map_err(|source| ConfigError::CreateDirectory {
            path: directory.to_path_buf(),
            source,
        })?;
    }
    let raw = serde_json::to_string_pretty(config).map_err(ConfigError::Serialize)?;
    fs::write(path, format!("{raw}\n")).map_err(|source| ConfigError::Write {
        path: path.to_path_buf(),
        source,
    })
}

fn has_value(value: &str) -> bool {
    let value = value.trim();
    !value.is_empty() && !value.starts_with("your_") && !value.contains("key_here")
}

fn default_sandbox_command() -> &'static str {
    if cfg!(target_os = "macos") {
        "sandbox-exec"
    } else if cfg!(target_os = "linux") {
        "bwrap"
    } else {
        ""
    }
}

fn runtime_extra_defaults() -> BTreeMap<String, Value> {
    serde_json::from_value(serde_json::json!({
        "automationBrowser": "chromium",
        "browserDebugUrl": "http://127.0.0.1:9222/json/list",
        "browserExtensionContextUrl": "http://127.0.0.1:3212/context",
        "proactiveInterval": 60,
        "proactiveCooldown": 120,
        "enableHeadlessTaskQueue": false,
        "enableAutoUpdate": false,
        "updaterEndpoint": "",
        "updaterPublicKey": "",
        "enableVoiceReply": true,
        "enableCustomWorkflows": true,
        "ttsProvider": "google",
        "ttsVolume": 1.0,
        "ttsSpeed": 1.0,
        "ttsPitch": 1.0,
        "pluginCalendarEnabled": false,
        "pluginGmailEnabled": false,
        "pluginNotionEnabled": false,
        "telegramBotToken": "",
        "enableTelegramBridge": false,
        "discordBotToken": "",
        "discordApplicationId": "",
        "enableDiscordBridge": false,
        "slackBotToken": "",
        "slackAppToken": "",
        "enableSlackBridge": false,
        "lineChannelAccessToken": "",
        "lineChannelSecret": "",
        "enableLineBridge": false,
        "lineWebhookPort": 3000,
        "whatsappCloudAccessToken": "",
        "whatsappPhoneNumberId": "",
        "whatsappVerifyToken": "",
        "whatsappAppSecret": "",
        "enableWhatsappBridge": false,
        "googleSearchApiKey": "",
        "googleSearchCx": "",
        "braveSearchApiKey": "",
        "googleCalendarClientId": "",
        "googleCalendarClientSecret": "",
        "googleCalendarRefreshToken": "",
        "googleCalendarId": "primary",
        "gmailClientId": "",
        "gmailClientSecret": "",
        "gmailRefreshToken": "",
        "gmailUserId": "me",
        "notionApiKey": "",
        "notionDatabaseId": "",
        "notionPageId": "",
        "notionTitleProperty": "Name",
        "allowedShellModes": ["readOnly", "test", "mutating", "network"],
        "allowedNativePlugins": ["dev_tools", "system_metrics"],
        "allowedMcpTools": {},
        "mcpServers": {}
    }))
    .expect("runtime config defaults must be a JSON object")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_config_always_exposes_ollama() {
        assert_eq!(MintConfig::default().available_providers(), vec!["ollama"]);
    }

    #[test]
    fn configured_providers_are_reported_before_ollama() {
        let config = MintConfig {
            api_key: "gemini-secret".into(),
            openai_api_key: "openai-secret".into(),
            openrouter_api_key: "openrouter-secret".into(),
            deepseek_api_key: "deepseek-secret".into(),
            local_api_base_url: "http://localhost:1234/v1".into(),
            ..MintConfig::default()
        };
        assert_eq!(
            config.available_providers(),
            vec![
                "openai",
                "openrouter",
                "deepseek",
                "gemini",
                "local_openai",
                "ollama"
            ]
        );
    }

    #[test]
    fn config_round_trips_through_json() {
        let expected = MintConfig {
            ai_provider: "ollama".into(),
            ..MintConfig::default()
        };
        let json = serde_json::to_string(&expected).unwrap();
        let actual: MintConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn config_preserves_fields_that_have_not_migrated_yet() {
        let config: MintConfig =
            serde_json::from_str(r#"{"aiProvider":"gemini","pluginGmailEnabled":true}"#).unwrap();
        let json = serde_json::to_value(config).unwrap();
        assert_eq!(json["pluginGmailEnabled"], true);
    }

    #[test]
    fn default_config_includes_native_runtime_flags() {
        let config = MintConfig::default();
        assert_eq!(config.extra["enableHeadlessTaskQueue"], false);
        assert_eq!(config.extra["ttsProvider"], "google");
        assert_eq!(config.extra["lineWebhookPort"], 3000);
        assert_eq!(
            config.extra["allowedShellModes"],
            serde_json::json!(["readOnly", "test", "mutating", "network"])
        );
        assert_eq!(
            config.extra["allowedNativePlugins"],
            serde_json::json!(["dev_tools", "system_metrics"])
        );
        assert_eq!(config.extra["allowedMcpTools"], serde_json::json!({}));
    }
}

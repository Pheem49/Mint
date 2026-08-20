use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use regex::Regex;
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
    pub nanobanana_model: String,
    /// Active image-generation provider: "nanobanana" | "dalle" | "stability" | "ideogram" | "replicate"
    pub image_gen_provider: String,
    pub dalle_model: String,
    pub stability_api_key: String,
    pub stability_model: String,
    pub ideogram_api_key: String,
    pub ideogram_model: String,
    pub replicate_api_key: String,
    pub replicate_model: String,
    pub bfl_api_key: String,
    pub bfl_model: String,
    pub show_desktop_widget: bool,
    pub safety_enabled: bool,
    pub sandbox_mode: String,
    pub sandbox_command: String,
    pub allowed_read_paths: Vec<PathBuf>,
    pub allowed_write_paths: Vec<PathBuf>,
    pub blocked_paths: Vec<PathBuf>,
    pub blocked_file_names: Vec<String>,
    pub disabled_tools: Vec<String>,
    pub agents: Vec<AgentConfig>,
    pub enable_agent_collaboration: bool,
    /// When true, a background reflection call runs after a task finishes and may
    /// write a new `.agents/skills/<slug>/SKILL.md` file if the task looks like a
    /// non-trivial, reusable problem. Off by default: it costs an extra LLM call
    /// and writes files without interactive approval. See
    /// [`crate::orchestration::spawn_auto_skill_write`].
    pub auto_skill_writing: bool,
    /// User-defined OpenAI-compatible providers.
    pub custom_providers: Vec<CustomProvider>,
    /// Persistent "always allow"/"always deny" rules for agent-loop approvals,
    /// so a user doesn't have to re-approve the same shell command or file edit
    /// every single time. See [`MintConfig::permission_decision`].
    pub permission_rules: Vec<PermissionRule>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct AgentConfig {
    pub id: String,
    pub name: String,
    pub provider: String,
    pub model: String,
    pub api_key: Option<String>,
    pub system_instruction: String,
    pub enabled: bool,
}

/// A single model entry inside a [`CustomProvider`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderModel {
    /// The model identifier sent to the API, e.g. `"llama-3.1-8b"`.
    pub model_id: String,
    /// Human-readable label shown in the UI, e.g. `"Llama 3.1 8B"`.
    pub display_name: String,
}

/// An extra HTTP header to include with every request to a [`CustomProvider`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomProviderHeader {
    pub name: String,
    pub value: String,
}

/// A persistent approval decision for a specific agent-loop tool + subject,
/// so the same shell command, file path, plugin, or MCP tool doesn't have to
/// be approved interactively every time it recurs.
///
/// `pattern` is matched against a per-tool "subject" string (the exact shell
/// command for `run_shell`, the file path for `write_file`/`apply_patch`, the
/// note path for `note_write`, `"{name}: {instruction}"` for `run_plugin`, or
/// `"{server}:{tool}:{arguments}"` for `mcp_tool`) using a simple glob where
/// `*` matches any run of characters — everything else must match literally.
/// The instruction/arguments are included so a saved rule never covers a
/// future call to the same plugin/tool with different, possibly riskier
/// input than what the user actually reviewed.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PermissionRule {
    pub tool: String,
    pub pattern: String,
    pub decision: PermissionDecision,
    /// `None` = applies everywhere. `Some(root)` = only applies when the
    /// agent's current workspace root matches `root` exactly.
    #[serde(default)]
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PermissionDecision {
    Allow,
    Deny,
}

/// Matches `subject` against a simple glob `pattern` where `*` matches any
/// run of characters (including none) and every other character must match
/// literally. Implemented via `regex` (already a workspace dependency) rather
/// than a dedicated glob crate.
fn glob_match(pattern: &str, subject: &str) -> bool {
    let escaped = regex::escape(pattern).replace("\\*", ".*");
    Regex::new(&format!("^{escaped}$"))
        .map(|re| re.is_match(subject))
        .unwrap_or(false)
}

impl MintConfig {
    /// Looks up a persistent approval decision for `tool`/`subject` given the
    /// agent's current `project_root`. Precedence: any matching `Deny` (any
    /// scope) wins outright; else a matching project-scoped `Allow`; else a
    /// matching global `Allow`; else `None` (no saved rule — caller should
    /// fall back to an interactive prompt).
    pub fn permission_decision(
        &self,
        tool: &str,
        subject: &str,
        project_root: &Path,
    ) -> Option<PermissionDecision> {
        permission_decision_for(&self.permission_rules, tool, subject, project_root)
    }
}

/// Core matching logic behind [`MintConfig::permission_decision`], exposed as a
/// free function so callers that maintain their own in-memory rule list (e.g.
/// rules added earlier in the current process, not yet persisted) can reuse
/// the same precedence without round-tripping through a `MintConfig`.
pub fn permission_decision_for(
    rules: &[PermissionRule],
    tool: &str,
    subject: &str,
    project_root: &Path,
) -> Option<PermissionDecision> {
    let matches = |rule: &&PermissionRule| {
        rule.tool == tool
            && glob_match(&rule.pattern, subject)
            && rule
                .project_root
                .as_deref()
                .is_none_or(|root| root == project_root)
    };

    if rules
        .iter()
        .filter(matches)
        .any(|rule| rule.decision == PermissionDecision::Deny)
    {
        return Some(PermissionDecision::Deny);
    }
    if rules
        .iter()
        .filter(matches)
        .any(|rule| rule.decision == PermissionDecision::Allow && rule.project_root.is_some())
    {
        return Some(PermissionDecision::Allow);
    }
    if rules
        .iter()
        .filter(matches)
        .any(|rule| rule.decision == PermissionDecision::Allow && rule.project_root.is_none())
    {
        return Some(PermissionDecision::Allow);
    }
    None
}

/// A fully user-defined, OpenAI-compatible AI provider.
///
/// When this provider is active the `ai_provider` config field is set to
/// `"custom:<id>"` where `<id>` is [`CustomProvider::id`].
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct CustomProvider {
    /// Slug identifier (lowercase letters, numbers, hyphens, underscores).
    pub id: String,
    /// Human-readable name shown in the UI.
    pub display_name: String,
    /// Base URL of the OpenAI-compatible endpoint, e.g. `"http://localhost:20128/v1"`.
    pub base_url: String,
    /// API key sent as `Authorization: Bearer <key>`. Empty = no auth header.
    pub api_key: String,
    /// Models offered by this provider.
    pub models: Vec<CustomProviderModel>,
    /// Optional extra HTTP headers sent with every request.
    pub headers: Vec<CustomProviderHeader>,
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
            accent_color: "#10b981".into(),
            language: "th-TH".into(),
            assistant_mode: "chat".into(),
            ai_provider: "gemini".into(),
            api_key: String::new(),
            gemini_model: "gemini-2.5-flash".into(),
            anthropic_api_key: String::new(),
            anthropic_model: "claude-sonnet-5".into(),
            openai_api_key: String::new(),
            openai_model: "gpt-5.6-luna".into(),
            openrouter_api_key: String::new(),
            openrouter_model: "openai/gpt-5.6-terra".into(),
            deepseek_api_key: String::new(),
            deepseek_model: "deepseek-v4-flash".into(),
            hf_api_key: String::new(),
            hf_model: "Qwen/Qwen3.6-27B".into(),
            local_api_base_url: String::new(),
            local_model_name: "local-model".into(),
            ollama_host: String::new(),
            ollama_model: "llama3:latest".into(),
            nanobanana_model: "gemini-3.1-flash-image".into(),
            image_gen_provider: "nanobanana".into(),
            dalle_model: "gpt-image-1".into(),
            stability_api_key: String::new(),
            stability_model: "ultra".into(),
            ideogram_api_key: String::new(),
            ideogram_model: "V_3".into(),
            replicate_api_key: String::new(),
            replicate_model: "black-forest-labs/flux-1.1-pro".into(),
            bfl_api_key: String::new(),
            bfl_model: "flux-pro-1.1".into(),
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
            agents: default_agents(),
            enable_agent_collaboration: false,
            auto_skill_writing: false,
            custom_providers: Vec::new(),
            permission_rules: Vec::new(),
            extra: runtime_extra_defaults(),
        }
    }
}

fn default_agents() -> Vec<AgentConfig> {
    vec![
        AgentConfig {
            id: "planner".into(),
            name: "Planner".into(),
            provider: "gemini".into(),
            model: "gemini-2.5-flash".into(),
            api_key: None,
            system_instruction: "You are the Planner agent. Your task is to analyze the user request, inspect the workspace structure, and design a step-by-step implementation plan. Create or update the implementation_plan.md file to document your proposal.".into(),
            enabled: true,
        },
        AgentConfig {
            id: "coder".into(),
            name: "Coder".into(),
            provider: "gemini".into(),
            model: "gemini-2.5-flash".into(),
            api_key: None,
            system_instruction: "You are the Coder agent. Your task is to implement the changes specified in the approved implementation plan. Read relevant files, write clean and efficient code, and run terminal commands to build or verify. Stay focused on execution.".into(),
            enabled: true,
        },
        AgentConfig {
            id: "reviewer".into(),
            name: "Reviewer".into(),
            provider: "gemini".into(),
            model: "gemini-2.5-flash".into(),
            api_key: None,
            system_instruction: "You are the Reviewer agent. Your task is to verify the code modifications made by the Coder. Run the automated tests, check lint errors, and ensure the implementation is correct and complete.".into(),
            enabled: true,
        },
    ]
}

/// Local model families known to support native tool/function calling via
/// Ollama's `/api/chat` `tools` parameter. Not exhaustive — Ollama's ecosystem
/// evolves quickly, so this is a conservative allowlist rather than a guarantee.
const OLLAMA_NATIVE_TOOL_MODEL_PREFIXES: &[&str] = &[
    "llama3.1",
    "llama3.2",
    "llama3.3",
    "qwen2",
    "qwen2.5",
    "qwen3",
    "mistral-nemo",
    "mistral-large",
    "firefunction",
    "command-r",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCallingMode {
    /// Use the provider's native function/tool-calling API.
    Native,
    /// Fall back to prompting the model to emit a single JSON action per turn.
    JsonPrompt,
}

impl MintConfig {
    /// Whether the active provider/model should be driven via native
    /// function/tool-calling, or via the legacy JSON-prompt fallback.
    ///
    /// Anthropic, OpenAI-family providers (including OpenAI-compatible custom
    /// providers), and Gemini always get `Native`. Ollama only gets `Native` for
    /// model families known to support it reliably (see
    /// `OLLAMA_NATIVE_TOOL_MODEL_PREFIXES`); anything else falls back to
    /// `JsonPrompt` so the agent loop can warn the user instead of silently
    /// producing unreliable output. HuggingFace and unrecognized providers always
    /// use `JsonPrompt`. `extra["forceJsonPromptMode"] = true` overrides everything
    /// to `JsonPrompt`, as an escape hatch for debugging or unsupported setups.
    pub fn tool_calling_mode(&self) -> ToolCallingMode {
        if self
            .extra
            .get("forceJsonPromptMode")
            .and_then(Value::as_bool)
            .unwrap_or(false)
        {
            return ToolCallingMode::JsonPrompt;
        }
        match self.ai_provider.as_str() {
            "anthropic" | "gemini" | "openai" | "local_openai" | "openrouter" | "deepseek" => {
                ToolCallingMode::Native
            }
            p if p.starts_with("custom:") => ToolCallingMode::Native,
            "ollama" => {
                let model = self.ollama_model.to_ascii_lowercase();
                if OLLAMA_NATIVE_TOOL_MODEL_PREFIXES
                    .iter()
                    .any(|prefix| model.starts_with(prefix))
                {
                    ToolCallingMode::Native
                } else {
                    ToolCallingMode::JsonPrompt
                }
            }
            _ => ToolCallingMode::JsonPrompt,
        }
    }

    /// A conservative estimate of the active provider's context window, in
    /// tokens. Provider-level rather than tracking every exact model string,
    /// except where two tiers within a provider differ enough to matter
    /// (Anthropic's Haiku line vs. Opus/Sonnet/Fable). Erring smaller just
    /// triggers context compaction earlier (safe); erring larger risks an
    /// actual context-length error from the provider (not safe).
    pub fn context_window_tokens(&self) -> usize {
        match self.ai_provider.as_str() {
            "anthropic" => {
                if self.anthropic_model.to_ascii_lowercase().contains("haiku") {
                    200_000
                } else {
                    1_000_000
                }
            }
            "gemini" => 1_000_000,
            "openai" => 1_000_000,
            "ollama" => 8_000,
            // Huggingface/OpenRouter/DeepSeek/local/custom endpoints proxy many
            // different backing models with widely varying context windows —
            // 128k is a conservative floor rather than a real measurement.
            "huggingface" => 128_000,
            _ => 128_000,
        }
    }

    pub fn active_model(&self) -> &str {
        match self.ai_provider.as_str() {
            "anthropic" => &self.anthropic_model,
            "openai" => &self.openai_model,
            "openrouter" => &self.openrouter_model,
            "deepseek" => &self.deepseek_model,
            "huggingface" => &self.hf_model,
            "local_openai" => &self.local_model_name,
            "ollama" => &self.ollama_model,
            p if p.starts_with("custom:") => {
                // Return the first model id of the matching custom provider, if any.
                self.resolve_custom_provider(p)
                    .and_then(|cp| cp.models.first())
                    .map(|m| m.model_id.as_str())
                    .unwrap_or("")
            }
            _ => &self.gemini_model,
        }
    }

    /// Sets the active provider and model, saves the configuration to disk, and logs a system event interaction.
    pub fn set_active_model(
        &mut self,
        provider: &str,
        model: Option<&str>,
    ) -> Result<String, ConfigError> {
        let old_provider = self.ai_provider.clone();
        let old_active_model = self.active_model().to_string();

        self.ai_provider = provider.to_string();
        if let Some(m) = model {
            let m_trimmed = m.trim();
            if !m_trimmed.is_empty() {
                match provider {
                    "anthropic" => self.anthropic_model = m_trimmed.to_string(),
                    "openai" => self.openai_model = m_trimmed.to_string(),
                    "openrouter" => self.openrouter_model = m_trimmed.to_string(),
                    "deepseek" => self.deepseek_model = m_trimmed.to_string(),
                    "huggingface" => self.hf_model = m_trimmed.to_string(),
                    "local_openai" => self.local_model_name = m_trimmed.to_string(),
                    "ollama" => self.ollama_model = m_trimmed.to_string(),
                    "gemini" => self.gemini_model = m_trimmed.to_string(),
                    _ => {}
                }
            }
        }
        save_config(self)?;

        let active_model = self.active_model().to_string();
        let display_name = format!("{} • {}", self.ai_provider, active_model);

        let changed = old_provider != self.ai_provider || old_active_model != active_model;
        if changed {
            if let Ok(memory) = crate::MemoryStore::open_default() {
                let _ = memory.add_interaction_for_chat_with_fallback(
                    crate::CHAT_CLI_ID,
                    &display_name,
                    "",
                    "system",
                    "provider_change",
                    None,
                );
            }
        }

        Ok(display_name)
    }

    /// Returns the `CustomProvider` whose id matches `provider_key`.
    ///
    /// `provider_key` must be in `"custom:<id>"` format (the leading `"custom:"`
    /// prefix is stripped before the lookup).
    pub fn resolve_custom_provider(&self, provider_key: &str) -> Option<&CustomProvider> {
        let id = provider_key.strip_prefix("custom:").unwrap_or(provider_key);
        self.custom_providers.iter().find(|cp| cp.id == id)
    }

    pub fn available_providers(&self) -> Vec<String> {
        let mut providers = Vec::new();
        if has_value(&self.anthropic_api_key) {
            providers.push("anthropic".to_owned());
        }
        if has_value(&self.openai_api_key) {
            providers.push("openai".to_owned());
        }
        if has_value(&self.openrouter_api_key) {
            providers.push("openrouter".to_owned());
        }
        if has_value(&self.deepseek_api_key) {
            providers.push("deepseek".to_owned());
        }
        if has_value(&self.api_key) {
            providers.push("gemini".to_owned());
        }
        if has_value(&self.hf_api_key) {
            providers.push("huggingface".to_owned());
        }
        if has_value(&self.local_api_base_url) {
            providers.push("local_openai".to_owned());
        }
        providers.push("ollama".to_owned());
        // Include all user-defined custom providers that have a base URL configured.
        for cp in &self.custom_providers {
            if !cp.base_url.trim().is_empty() {
                providers.push(format!("custom:{}", cp.id));
            }
        }
        providers
    }

    pub fn active_workspace_path(&self) -> Option<PathBuf> {
        self.extra
            .get("activeWorkspacePath")
            .or_else(|| self.extra.get("defaultWorkspacePath"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok())
    }

    pub fn line_webhook_host(&self) -> String {
        self.extra
            .get("lineWebhookHost")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("127.0.0.1")
            .to_string()
    }

    pub fn line_webhook_port(&self) -> u16 {
        self.extra
            .get("lineWebhookPort")
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .map(|p| p as u16)
            .unwrap_or(3000)
    }

    pub fn whatsapp_webhook_host(&self) -> String {
        self.extra
            .get("whatsappWebhookHost")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("127.0.0.1")
            .to_string()
    }

    pub fn whatsapp_webhook_port(&self) -> u16 {
        self.extra
            .get("whatsappWebhookPort")
            .and_then(|v| {
                v.as_u64()
                    .or_else(|| v.as_str().and_then(|s| s.parse().ok()))
            })
            .map(|p| p as u16)
            .unwrap_or(3001)
    }

    pub fn bridge_ack_enabled(&self) -> bool {
        self.extra
            .get("enableBridgeAckNotification")
            .and_then(Value::as_bool)
            .unwrap_or(true)
    }

    pub fn bridge_ack_message(&self) -> String {
        self.extra
            .get("bridgeAckMessage")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("[Mint Agent] Remote command received, processing...")
            .to_string()
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
        let mut config = MintConfig::default();
        if which("gk") {
            let mut servers = BTreeMap::new();
            servers.insert(
                "gitkraken".to_string(),
                serde_json::json!({
                    "command": "gk",
                    "args": ["mcp"]
                }),
            );
            config
                .extra
                .insert("mcpServers".into(), serde_json::to_value(servers).unwrap());
        }
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

    let has_gitkraken = config
        .extra
        .get("mcpServers")
        .and_then(|v| v.get("gitkraken"))
        .is_some();
    if !has_gitkraken && which("gk") {
        let mut servers: BTreeMap<String, Value> = config
            .extra
            .get("mcpServers")
            .cloned()
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or_default();
        servers.insert(
            "gitkraken".to_string(),
            serde_json::json!({
                "command": "gk",
                "args": ["mcp"]
            }),
        );
        config
            .extra
            .insert("mcpServers".into(), serde_json::to_value(servers).unwrap());
        let _ = save_config_to(path, &config);
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
    })?;
    restrict_to_owner(path);
    Ok(())
}

/// This file holds every configured provider's API key in plaintext, so it's
/// restricted to owner-only (rw-------) rather than left at the default
/// umask-derived permissions, which are typically world-readable. Best
/// effort: a permission-set failure shouldn't block saving the config
/// itself, and there's no equivalent ACL-based restriction applied on
/// Windows here (out of scope for this pass — NTFS files already default to
/// the owning user's profile directory on a normal single-user system).
#[cfg(unix)]
fn restrict_to_owner(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
}

#[cfg(not(unix))]
fn restrict_to_owner(_path: &Path) {}

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
        "voiceMode": "legacy",
        "geminiLiveModel": "gemini-2.5-flash-native-audio-preview-12-2025",
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
        "searxngBaseUrl": "",
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
        "allowedNativePlugins": ["dev_tools", "system_metrics", "github"],
        "allowedMcpTools": {},
        "mcpServers": {}
    }))
    .expect("runtime config defaults must be a JSON object")
}

pub fn which(cmd: &str) -> bool {
    #[cfg(windows)]
    let check_cmd = "where";
    #[cfg(not(windows))]
    let check_cmd = "which";

    std::process::Command::new(check_cmd)
        .arg(cmd)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn glob_match_supports_exact_and_wildcard_patterns() {
        assert!(glob_match("cargo test", "cargo test"));
        assert!(!glob_match("cargo test", "cargo test -p mint-core"));
        assert!(glob_match("git *", "git status --short"));
        assert!(glob_match("*.rs", "main.rs"));
        assert!(!glob_match("*.rs", "main.rs.bak"));
    }

    #[test]
    fn permission_decision_falls_back_to_none_without_a_matching_rule() {
        let config = MintConfig::default();
        assert_eq!(
            config.permission_decision("run_shell", "cargo test", Path::new("/repo")),
            None
        );
    }

    #[test]
    fn permission_decision_prefers_project_allow_over_global_allow() {
        let config = MintConfig {
            permission_rules: vec![
                PermissionRule {
                    tool: "run_shell".into(),
                    pattern: "cargo test".into(),
                    decision: PermissionDecision::Allow,
                    project_root: None,
                },
                PermissionRule {
                    tool: "run_shell".into(),
                    pattern: "cargo test".into(),
                    decision: PermissionDecision::Allow,
                    project_root: Some(PathBuf::from("/repo")),
                },
            ],
            ..MintConfig::default()
        };
        assert_eq!(
            config.permission_decision("run_shell", "cargo test", Path::new("/repo")),
            Some(PermissionDecision::Allow)
        );
        // A different project root shouldn't get the project-scoped rule, but
        // still falls through to the global allow rule.
        assert_eq!(
            config.permission_decision("run_shell", "cargo test", Path::new("/other")),
            Some(PermissionDecision::Allow)
        );
    }

    #[test]
    fn permission_decision_deny_always_wins_over_allow() {
        let config = MintConfig {
            permission_rules: vec![
                PermissionRule {
                    tool: "run_shell".into(),
                    pattern: "rm *".into(),
                    decision: PermissionDecision::Allow,
                    project_root: None,
                },
                PermissionRule {
                    tool: "run_shell".into(),
                    pattern: "rm *".into(),
                    decision: PermissionDecision::Deny,
                    project_root: Some(PathBuf::from("/repo")),
                },
            ],
            ..MintConfig::default()
        };
        assert_eq!(
            config.permission_decision("run_shell", "rm old.txt", Path::new("/repo")),
            Some(PermissionDecision::Deny)
        );
    }

    #[test]
    fn permission_decision_does_not_cross_tools() {
        let config = MintConfig {
            permission_rules: vec![PermissionRule {
                tool: "write_file".into(),
                pattern: "notes.md".into(),
                decision: PermissionDecision::Allow,
                project_root: None,
            }],
            ..MintConfig::default()
        };
        assert_eq!(
            config.permission_decision("apply_patch", "notes.md", Path::new("/repo")),
            None
        );
    }

    #[test]
    fn context_window_tokens_uses_conservative_provider_defaults() {
        let window_for = |provider: &str| {
            MintConfig {
                ai_provider: provider.into(),
                ..MintConfig::default()
            }
            .context_window_tokens()
        };
        // Default anthropic_model is claude-sonnet-5 (non-Haiku).
        assert_eq!(window_for("anthropic"), 1_000_000);
        assert_eq!(window_for("gemini"), 1_000_000);
        assert_eq!(window_for("openai"), 1_000_000);
        assert_eq!(window_for("ollama"), 8_000);
        assert_eq!(window_for("huggingface"), 128_000);
        assert_eq!(window_for("openrouter"), 128_000);
        assert_eq!(window_for("custom:some-endpoint"), 128_000);
    }

    #[test]
    fn context_window_tokens_uses_smaller_window_for_anthropic_haiku() {
        let config = MintConfig {
            ai_provider: "anthropic".into(),
            anthropic_model: "claude-haiku-4-5".into(),
            ..MintConfig::default()
        };
        assert_eq!(config.context_window_tokens(), 200_000);
    }

    #[test]
    fn tool_calling_mode_is_native_for_first_class_providers() {
        for provider in ["anthropic", "gemini", "openai", "openrouter", "deepseek"] {
            let config = MintConfig {
                ai_provider: provider.into(),
                ..MintConfig::default()
            };
            assert_eq!(
                config.tool_calling_mode(),
                ToolCallingMode::Native,
                "{provider}"
            );
        }
    }

    #[test]
    fn tool_calling_mode_gates_ollama_by_model_allowlist() {
        let allowed = MintConfig {
            ai_provider: "ollama".into(),
            ollama_model: "qwen2.5:14b".into(),
            ..MintConfig::default()
        };
        assert_eq!(allowed.tool_calling_mode(), ToolCallingMode::Native);

        let unlisted = MintConfig {
            ai_provider: "ollama".into(),
            ollama_model: "gemma2:9b".into(),
            ..MintConfig::default()
        };
        assert_eq!(unlisted.tool_calling_mode(), ToolCallingMode::JsonPrompt);
    }

    #[test]
    fn force_json_prompt_mode_overrides_native_providers() {
        let mut config = MintConfig {
            ai_provider: "anthropic".into(),
            ..MintConfig::default()
        };
        config
            .extra
            .insert("forceJsonPromptMode".into(), serde_json::json!(true));
        assert_eq!(config.tool_calling_mode(), ToolCallingMode::JsonPrompt);
    }

    #[test]
    fn default_config_always_exposes_ollama() {
        assert_eq!(
            MintConfig::default().available_providers(),
            vec!["ollama".to_owned()]
        );
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
                "openai".to_owned(),
                "openrouter".to_owned(),
                "deepseek".to_owned(),
                "gemini".to_owned(),
                "local_openai".to_owned(),
                "ollama".to_owned(),
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
            serde_json::json!(["dev_tools", "system_metrics", "github"])
        );
        assert_eq!(config.extra["allowedMcpTools"], serde_json::json!({}));
    }

    #[test]
    fn test_gitkraken_auto_detection() {
        let path = std::env::temp_dir().join("mint-test-config.json");
        let _ = std::fs::remove_file(&path);

        let initial_config = MintConfig::default();
        save_config_to(&path, &initial_config).unwrap();

        let loaded = load_config_from(&path).unwrap();
        assert!(!loaded.theme.is_empty());

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn custom_provider_round_trips() {
        let config = MintConfig {
            custom_providers: vec![CustomProvider {
                id: "myprovider".into(),
                display_name: "My Provider".into(),
                base_url: "http://localhost:20128/v1".into(),
                api_key: "sk-abc".into(),
                models: vec![CustomProviderModel {
                    model_id: "llama-3.1-8b".into(),
                    display_name: "Llama 3.1 8B".into(),
                }],
                headers: vec![CustomProviderHeader {
                    name: "X-Custom".into(),
                    value: "test".into(),
                }],
            }],
            ..MintConfig::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let restored: MintConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(restored, config);
    }

    #[test]
    fn custom_provider_appears_in_available_providers() {
        let config = MintConfig {
            custom_providers: vec![CustomProvider {
                id: "myprovider".into(),
                base_url: "http://localhost:20128/v1".into(),
                ..CustomProvider::default()
            }],
            ..MintConfig::default()
        };
        assert!(
            config
                .available_providers()
                .contains(&"custom:myprovider".to_owned())
        );
    }

    #[test]
    fn custom_provider_without_base_url_excluded() {
        let config = MintConfig {
            custom_providers: vec![CustomProvider {
                id: "empty".into(),
                base_url: "".into(),
                ..CustomProvider::default()
            }],
            ..MintConfig::default()
        };
        assert!(
            !config
                .available_providers()
                .contains(&"custom:empty".to_owned())
        );
    }

    #[test]
    fn active_model_resolves_first_custom_model() {
        let config = MintConfig {
            ai_provider: "custom:myprovider".into(),
            custom_providers: vec![CustomProvider {
                id: "myprovider".into(),
                base_url: "http://localhost:20128/v1".into(),
                models: vec![CustomProviderModel {
                    model_id: "llama-3.1-8b".into(),
                    display_name: String::new(),
                }],
                ..CustomProvider::default()
            }],
            ..MintConfig::default()
        };
        assert_eq!(config.active_model(), "llama-3.1-8b");
    }

    #[test]
    fn resolve_custom_provider_returns_none_for_unknown() {
        let config = MintConfig::default();
        assert!(
            config
                .resolve_custom_provider("custom:nonexistent")
                .is_none()
        );
    }

    #[test]
    fn resolve_custom_provider_strips_prefix() {
        let config = MintConfig {
            custom_providers: vec![CustomProvider {
                id: "myp".into(),
                ..CustomProvider::default()
            }],
            ..MintConfig::default()
        };
        assert!(config.resolve_custom_provider("custom:myp").is_some());
        // Also works without prefix
        assert!(config.resolve_custom_provider("myp").is_some());
    }
}

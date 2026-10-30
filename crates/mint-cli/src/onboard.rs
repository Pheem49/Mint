use anyhow::{Result, bail};
use crossterm::ExecutableCommand;
use crossterm::cursor::{Hide, Show};
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::{
    Terminal,
    backend::{Backend, CrosstermBackend},
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph},
};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use crate::terminal_theme::{self, ACCENT, BLUE, MUTED, SUBTLE, TEXT};
use mint_core::slash::models::HUGGINGFACE_MODEL_PRESETS;
use mint_core::{MintConfig, load_config, save_config};

// ─────────────────────────────────────────────────────────────────────────────
// Public Types & Outcomes
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardOutcome {
    SavedAndLaunch,
    SavedAndExit,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardStep {
    CoreAi,
    Services,
    Config,
    Summary,
}

impl OnboardStep {
    pub fn index(&self) -> usize {
        match self {
            Self::CoreAi => 0,
            Self::Services => 1,
            Self::Config => 2,
            Self::Summary => 3,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ConfigServiceGroup {
    pub service_key: &'static str,
    pub service_name: &'static str,
    pub description: &'static str,
    pub field_indices: Vec<usize>,
    pub focused_field: usize,
}

#[derive(Debug, Clone)]
pub struct OnboardService {
    pub category: &'static str,
    pub name: &'static str,
    pub key: &'static str,
    pub description: &'static str,
    pub enabled: bool,
}

#[derive(Debug, Clone)]
pub struct FormField {
    pub id: &'static str,
    pub service_key: &'static str,
    pub service_name: &'static str,
    pub label: &'static str,
    pub hint: &'static str,
    pub value: String,
    pub is_secret: bool,
    pub show_secret: bool,
    pub field_type: FieldType,
    pub custom_value: String,
    pub is_custom: bool,
}

#[derive(Debug, Clone)]
pub enum FieldType {
    Text,
    Choice {
        options: Vec<String>,
        selected: usize,
    },
}

fn make_text_field(
    id: &'static str,
    service_key: &'static str,
    service_name: &'static str,
    label: &'static str,
    hint: &'static str,
    value: String,
    is_secret: bool,
) -> FormField {
    FormField {
        id,
        service_key,
        service_name,
        label,
        hint,
        value,
        is_secret,
        show_secret: !is_secret,
        field_type: FieldType::Text,
        custom_value: String::new(),
        is_custom: false,
    }
}

fn make_choice_field(
    id: &'static str,
    service_key: &'static str,
    service_name: &'static str,
    label: &'static str,
    hint: &'static str,
    current_val: &str,
    mut presets: Vec<String>,
) -> FormField {
    if !presets.iter().any(|m| m == "Custom model...") {
        presets.push("Custom model...".to_string());
    }
    let (selected, is_custom, custom_value) = if current_val.is_empty() {
        (0, false, String::new())
    } else if let Some(pos) = presets.iter().position(|m| m == current_val) {
        (pos, false, String::new())
    } else {
        (
            presets.len().saturating_sub(1),
            true,
            current_val.to_string(),
        )
    };

    let initial_val = if is_custom {
        custom_value.clone()
    } else {
        presets.get(selected).cloned().unwrap_or_default()
    };

    FormField {
        id,
        service_key,
        service_name,
        label,
        hint,
        value: initial_val,
        is_secret: false,
        show_secret: true,
        field_type: FieldType::Choice {
            options: presets,
            selected,
        },
        custom_value,
        is_custom,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Wizard State & Engine
// ─────────────────────────────────────────────────────────────────────────────

pub struct OnboardWizard {
    pub config: MintConfig,
    pub step: OnboardStep,
    #[allow(dead_code)]
    pub theme: terminal_theme::TuiTheme,

    // Step 1: Core AI (Google Gemini API Key + Model Selection)
    pub gemini_key: String,
    pub gemini_masked: bool,
    pub is_verifying: bool,
    pub verify_error: Option<String>,
    pub verify_rx: Option<mpsc::Receiver<Result<Vec<String>, String>>>,
    pub spinner_tick: usize,
    pub gemini_models: Vec<String>,
    pub gemini_model_idx: usize,
    pub custom_gemini_model: String,
    pub is_custom_gemini_model: bool,
    pub gemini_focus: usize, // 0: API Key, 1: Model List

    // Step 2: Services Checklist
    pub services: Vec<OnboardService>,
    #[allow(dead_code)]
    pub categories: Vec<&'static str>,
    pub service_cursor: usize,
    #[allow(dead_code)]
    pub service_scroll_offset: usize,

    // Step 3: Service Configurations (Grouped by service)
    pub fields: Vec<FormField>,
    pub service_groups: Vec<ConfigServiceGroup>,
    pub service_group_idx: usize,
    #[allow(dead_code)]
    pub field_scroll_offset: usize,
    pub service_fetch_rx: Option<mpsc::Receiver<(String, Vec<String>)>>,
    pub is_fetching_service_models: bool,

    // Step 4: Summary & Save
    pub summary_action: usize, // 0: Save & Launch, 1: Save & Exit, 2: Back
}

impl OnboardWizard {
    pub fn new(config: MintConfig) -> Self {
        let default_gemini_models = vec![
            "gemini-2.5-flash".to_string(),
            "gemini-2.5-pro".to_string(),
            "gemini-2.0-flash".to_string(),
            "gemini-1.5-pro".to_string(),
            "gemini-1.5-flash".to_string(),
            "Custom model...".to_string(),
        ];

        let current_model = if config.gemini_model.is_empty() {
            "gemini-2.5-flash".to_string()
        } else {
            config.gemini_model.clone()
        };

        let (gemini_model_idx, is_custom_gemini_model, custom_gemini_model) = if let Some(pos) =
            default_gemini_models
                .iter()
                .position(|m| m == &current_model)
        {
            (pos, false, String::new())
        } else {
            (default_gemini_models.len() - 1, true, current_model.clone())
        };

        let services = vec![
            // AI Providers
            OnboardService {
                category: "AI Providers",
                name: "Anthropic (Claude) API",
                key: "anthropic",
                description: "Claude 3.7 Sonnet, 3.5 Sonnet, Haiku models",
                enabled: !config.anthropic_api_key.is_empty(),
            },
            OnboardService {
                category: "AI Providers",
                name: "OpenAI API",
                key: "openai",
                description: "GPT-4o, o1, o3-mini models",
                enabled: !config.openai_api_key.is_empty(),
            },
            OnboardService {
                category: "AI Providers",
                name: "OpenRouter API",
                key: "openrouter",
                description: "Aggregated access to 200+ models",
                enabled: !config.openrouter_api_key.is_empty(),
            },
            OnboardService {
                category: "AI Providers",
                name: "DeepSeek API",
                key: "deepseek",
                description: "DeepSeek V3 / R1 reasoning models",
                enabled: !config.deepseek_api_key.is_empty(),
            },
            OnboardService {
                category: "AI Providers",
                name: "Hugging Face API",
                key: "huggingface",
                description: "Inference API for open-source LLMs",
                enabled: !config.hf_api_key.is_empty(),
            },
            OnboardService {
                category: "AI Providers",
                name: "Local OpenAI (e.g. LM Studio)",
                key: "local_openai",
                description: "OpenAI-compatible local server",
                enabled: !config.local_api_base_url.is_empty(),
            },
            OnboardService {
                category: "AI Providers",
                name: "Ollama",
                key: "ollama",
                description: "Self-hosted local models (Llama 3, Mistral)",
                enabled: !config.ollama_model.is_empty(),
            },
            // Search Engines
            OnboardService {
                category: "Search Engines",
                name: "Google Search API",
                key: "google_search",
                description: "Official Google Custom Search Engine",
                enabled: !config
                    .extra
                    .get("googleSearchApiKey")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty(),
            },
            OnboardService {
                category: "Search Engines",
                name: "Brave Search API",
                key: "brave_search",
                description: "Fast, privacy-focused search results",
                enabled: !config
                    .extra
                    .get("braveSearchApiKey")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty(),
            },
            OnboardService {
                category: "Search Engines",
                name: "SearXNG (self-hosted)",
                key: "searxng",
                description: "Self-hosted metasearch aggregator",
                enabled: !config
                    .extra
                    .get("searxngBaseUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty(),
            },
            // Messaging Bridges
            OnboardService {
                category: "Messaging Bridges",
                name: "Telegram Bot Bridge",
                key: "telegram",
                description: "Interact with Mint directly via Telegram",
                enabled: config
                    .extra
                    .get("enableTelegramBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            OnboardService {
                category: "Messaging Bridges",
                name: "Discord Bot Bridge",
                key: "discord",
                description: "Connect Mint to your Discord server",
                enabled: config
                    .extra
                    .get("enableDiscordBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            OnboardService {
                category: "Messaging Bridges",
                name: "Slack Bot Bridge",
                key: "slack",
                description: "Socket-mode Slack bridge for workspace channels",
                enabled: config
                    .extra
                    .get("enableSlackBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            OnboardService {
                category: "Messaging Bridges",
                name: "LINE Bot Bridge",
                key: "line",
                description: "Webhook-based LINE Messaging bridge",
                enabled: config
                    .extra
                    .get("enableLineBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            OnboardService {
                category: "Messaging Bridges",
                name: "WhatsApp Cloud Bridge",
                key: "whatsapp",
                description: "Official WhatsApp Cloud API bridge",
                enabled: config
                    .extra
                    .get("enableWhatsappBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            OnboardService {
                category: "Messaging Bridges",
                name: "Signal Bridge",
                key: "signal",
                description: "Signal messenger via signal-cli-rest-api",
                enabled: config
                    .extra
                    .get("enableSignalBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            OnboardService {
                category: "Messaging Bridges",
                name: "Email Bridge (Gmail)",
                key: "email",
                description: "OAuth-based Gmail inbox bridge for Mint",
                enabled: config
                    .extra
                    .get("enableEmailBridge")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false),
            },
            // Image Generation
            OnboardService {
                category: "Image Generation",
                name: "Google NanoBanana (Gemini Images)",
                key: "img_nanobanana",
                description: "Uses same Gemini API key (Imagen 3)",
                enabled: !config.api_key.is_empty() && !config.nanobanana_model.is_empty(),
            },
            OnboardService {
                category: "Image Generation",
                name: "OpenAI DALL·E",
                key: "img_dalle",
                description: "Uses same OpenAI key (DALL·E 3 / DALL·E 2)",
                enabled: !config.openai_api_key.is_empty() && !config.dalle_model.is_empty(),
            },
            OnboardService {
                category: "Image Generation",
                name: "Stability AI (Stable Diffusion)",
                key: "img_stability",
                description: "Stable Diffusion 3.5 Large, SD3 Medium, Core",
                enabled: !config.stability_api_key.is_empty(),
            },
            OnboardService {
                category: "Image Generation",
                name: "Ideogram v3",
                key: "img_ideogram",
                description: "Ideogram V_3, V_2, V_2_TURBO",
                enabled: !config.ideogram_api_key.is_empty(),
            },
            OnboardService {
                category: "Image Generation",
                name: "Replicate (FLUX / SDXL)",
                key: "img_replicate",
                description: "FLUX.1 Schnell, Dev, and community models",
                enabled: !config.replicate_api_key.is_empty(),
            },
            OnboardService {
                category: "Image Generation",
                name: "Black Forest Labs (FLUX API)",
                key: "img_bfl",
                description: "Official BFL API (flux-pro-1.1, flux-dev)",
                enabled: !config.bfl_api_key.is_empty(),
            },
            // Video Generation
            OnboardService {
                category: "Video Generation",
                name: "Google Veo (Gemini Videos)",
                key: "vid_veo",
                description: "Uses same Gemini API key (Veo 3.1 preview)",
                enabled: !config.api_key.is_empty()
                    && config
                        .extra
                        .get("veoModel")
                        .and_then(|v| v.as_str())
                        .is_some_and(|s| !s.is_empty()),
            },
            // Voice Mode
            OnboardService {
                category: "Voice Mode",
                name: "Realtime Live Model (Gemini Live)",
                key: "gemini_live",
                description: "Realtime duplex voice interactions with Gemini Live",
                enabled: config.extra.get("voiceMode").and_then(|v| v.as_str())
                    == Some("geminiLive"),
            },
        ];

        let categories = vec![
            "AI Providers",
            "Search Engines",
            "Messaging Bridges",
            "Image Generation",
            "Video Generation",
            "Voice Mode",
        ];

        let gemini_key = config.api_key.clone();

        let mut wizard = Self {
            config,
            step: OnboardStep::CoreAi,
            theme: terminal_theme::TuiTheme::Auto,
            gemini_key,
            gemini_masked: true,
            is_verifying: false,
            verify_error: None,
            verify_rx: None,
            spinner_tick: 0,

            gemini_models: default_gemini_models,
            gemini_model_idx,
            custom_gemini_model,
            is_custom_gemini_model,
            gemini_focus: 0,

            services,
            categories,
            service_cursor: 0,
            service_scroll_offset: 0,

            fields: Vec::new(),
            service_groups: Vec::new(),
            service_group_idx: 0,
            field_scroll_offset: 0,
            service_fetch_rx: None,
            is_fetching_service_models: false,

            summary_action: 0,
        };

        if !wizard.gemini_key.trim().is_empty() {
            wizard.trigger_gemini_verify();
        }

        wizard
    }

    pub fn take_config(self) -> MintConfig {
        self.config
    }

    pub fn is_service_enabled(&self, key: &str) -> bool {
        self.services.iter().any(|s| s.key == key && s.enabled)
    }

    pub fn build_config_fields(&mut self) {
        let mut fields = Vec::new();

        // 1. Anthropic
        if self.is_service_enabled("anthropic") {
            fields.push(make_text_field(
                "anthropic_key",
                "anthropic",
                "Anthropic (Claude)",
                "API Key",
                "sk-ant-...",
                self.config.anthropic_api_key.clone(),
                true,
            ));
            let anthropic_presets = vec![
                "claude-3-7-sonnet".to_string(),
                "claude-3-5-sonnet".to_string(),
                "claude-3-5-haiku".to_string(),
                "claude-3-opus".to_string(),
            ];
            fields.push(make_choice_field(
                "anthropic_model",
                "anthropic",
                "Anthropic (Claude)",
                "Model",
                "Select default Claude model",
                &self.config.anthropic_model,
                anthropic_presets,
            ));
        }

        // 2. OpenAI
        if self.is_service_enabled("openai") {
            fields.push(make_text_field(
                "openai_key",
                "openai",
                "OpenAI",
                "API Key",
                "sk-proj-...",
                self.config.openai_api_key.clone(),
                true,
            ));
            let openai_presets = vec![
                "gpt-4o".to_string(),
                "gpt-4o-mini".to_string(),
                "o1".to_string(),
                "o3-mini".to_string(),
            ];
            fields.push(make_choice_field(
                "openai_model",
                "openai",
                "OpenAI",
                "Model",
                "Select default OpenAI model",
                &self.config.openai_model,
                openai_presets,
            ));
        }

        // 3. OpenRouter
        if self.is_service_enabled("openrouter") {
            fields.push(make_text_field(
                "openrouter_key",
                "openrouter",
                "OpenRouter",
                "API Key",
                "sk-or-v1-...",
                self.config.openrouter_api_key.clone(),
                true,
            ));
            let openrouter_presets = vec![
                "openai/gpt-4o-mini".to_string(),
                "anthropic/claude-3.5-sonnet".to_string(),
                "meta-llama/llama-3.3-70b-instruct".to_string(),
                "deepseek/deepseek-chat".to_string(),
                "mistralai/mistral-large".to_string(),
                "google/gemini-2.5-flash".to_string(),
            ];
            fields.push(make_choice_field(
                "openrouter_model",
                "openrouter",
                "OpenRouter",
                "Model",
                "Select OpenRouter model slug",
                &self.config.openrouter_model,
                openrouter_presets,
            ));
        }

        // 4. DeepSeek
        if self.is_service_enabled("deepseek") {
            fields.push(make_text_field(
                "deepseek_key",
                "deepseek",
                "DeepSeek",
                "API Key",
                "sk-...",
                self.config.deepseek_api_key.clone(),
                true,
            ));
            let deepseek_presets = vec![
                "deepseek-chat".to_string(),
                "deepseek-reasoner".to_string(),
                "deepseek-v4-flash".to_string(),
            ];
            fields.push(make_choice_field(
                "deepseek_model",
                "deepseek",
                "DeepSeek",
                "Model",
                "Select DeepSeek model",
                &self.config.deepseek_model,
                deepseek_presets,
            ));
        }

        // 5. Hugging Face
        if self.is_service_enabled("huggingface") {
            fields.push(make_text_field(
                "hf_key",
                "huggingface",
                "Hugging Face",
                "User Access Token",
                "hf_...",
                self.config.hf_api_key.clone(),
                true,
            ));
            let hf_presets: Vec<String> = HUGGINGFACE_MODEL_PRESETS
                .iter()
                .map(|s| s.to_string())
                .collect();
            fields.push(make_choice_field(
                "hf_model",
                "huggingface",
                "Hugging Face",
                "Model",
                "Select preset Hugging Face model",
                &self.config.hf_model,
                hf_presets,
            ));
        }

        // 6. Local OpenAI
        if self.is_service_enabled("local_openai") {
            fields.push(make_text_field(
                "local_url",
                "local_openai",
                "Local OpenAI",
                "Base URL",
                "http://localhost:1234/v1",
                if self.config.local_api_base_url.is_empty() {
                    "http://localhost:1234/v1".to_string()
                } else {
                    self.config.local_api_base_url.clone()
                },
                false,
            ));
            fields.push(make_choice_field(
                "local_model",
                "local_openai",
                "Local OpenAI",
                "Model Identifier",
                "Local server model identifier",
                &self.config.local_model_name,
                vec!["default".to_string(), "local-model".to_string()],
            ));
        }

        // 7. Ollama
        if self.is_service_enabled("ollama") {
            fields.push(make_text_field(
                "ollama_host",
                "ollama",
                "Ollama",
                "Ollama Host",
                "http://localhost:11434",
                if self.config.ollama_host.is_empty() {
                    "http://localhost:11434".to_string()
                } else {
                    self.config.ollama_host.clone()
                },
                false,
            ));
            let mut ollama_presets = installed_ollama_models();
            if ollama_presets.is_empty() {
                ollama_presets = vec![
                    "llama3.2".to_string(),
                    "llama3.1".to_string(),
                    "qwen2.5-coder".to_string(),
                    "mistral".to_string(),
                    "deepseek-r1".to_string(),
                ];
            }
            fields.push(make_choice_field(
                "ollama_model",
                "ollama",
                "Ollama",
                "Model Tag",
                "Installed Ollama model tag",
                &self.config.ollama_model,
                ollama_presets,
            ));
        }

        // 8. Google Search
        if self.is_service_enabled("google_search") {
            let key = self
                .config
                .extra
                .get("googleSearchApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let cx = self
                .config
                .extra
                .get("googleSearchCx")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "google_search_key",
                "google_search",
                "Google Search",
                "API Key",
                "Google Custom Search API Key",
                key,
                true,
            ));
            fields.push(make_text_field(
                "google_search_cx",
                "google_search",
                "Google Search",
                "Search Engine ID (cx)",
                "Search engine identifier from cse.google.com",
                cx,
                false,
            ));
        }

        // 9. Brave Search
        if self.is_service_enabled("brave_search") {
            let key = self
                .config
                .extra
                .get("braveSearchApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "brave_search_key",
                "brave_search",
                "Brave Search",
                "API Key",
                "BSA...",
                key,
                true,
            ));
        }

        // 10. SearXNG
        if self.is_service_enabled("searxng") {
            let url = self
                .config
                .extra
                .get("searxngBaseUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:8080")
                .to_string();
            fields.push(make_text_field(
                "searxng_url",
                "searxng",
                "SearXNG",
                "Base URL",
                "http://localhost:8080",
                url,
                false,
            ));
        }

        // 11. Telegram Bridge
        if self.is_service_enabled("telegram") {
            let token = self
                .config
                .extra
                .get("telegramBotToken")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "telegram_token",
                "telegram",
                "Telegram Bridge",
                "Bot Token",
                "123456:ABC-DEF...",
                token,
                true,
            ));
        }

        // 12. Discord Bridge
        if self.is_service_enabled("discord") {
            let token = self
                .config
                .extra
                .get("discordBotToken")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let app_id = self
                .config
                .extra
                .get("discordApplicationId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "discord_token",
                "discord",
                "Discord Bridge",
                "Bot Token",
                "Bot token from Discord Developer Portal",
                token,
                true,
            ));
            fields.push(make_text_field(
                "discord_app_id",
                "discord",
                "Discord Bridge",
                "Application ID",
                "Client ID from OAuth2 General",
                app_id,
                false,
            ));
        }

        // 13. Slack Bridge
        if self.is_service_enabled("slack") {
            let bot = self
                .config
                .extra
                .get("slackBotToken")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let app = self
                .config
                .extra
                .get("slackAppToken")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "slack_bot_token",
                "slack",
                "Slack Bridge",
                "Bot Token (xoxb-)",
                "xoxb-...",
                bot,
                true,
            ));
            fields.push(make_text_field(
                "slack_app_token",
                "slack",
                "Slack Bridge",
                "App-level Token (xapp-)",
                "xapp-...",
                app,
                true,
            ));
        }

        // 14. LINE Bridge
        if self.is_service_enabled("line") {
            let token = self
                .config
                .extra
                .get("lineChannelAccessToken")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let secret = self
                .config
                .extra
                .get("lineChannelSecret")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "line_token",
                "line",
                "LINE Bridge",
                "Channel Access Token",
                "Long-lived access token",
                token,
                true,
            ));
            fields.push(make_text_field(
                "line_secret",
                "line",
                "LINE Bridge",
                "Channel Secret",
                "Channel secret from Basic Settings",
                secret,
                true,
            ));
        }

        // 15. WhatsApp Bridge
        if self.is_service_enabled("whatsapp") {
            let token = self
                .config
                .extra
                .get("whatsappCloudAccessToken")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let phone = self
                .config
                .extra
                .get("whatsappPhoneNumberId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "whatsapp_token",
                "whatsapp",
                "WhatsApp Bridge",
                "Access Token",
                "Permanent access token",
                token,
                true,
            ));
            fields.push(make_text_field(
                "whatsapp_phone",
                "whatsapp",
                "WhatsApp Bridge",
                "Phone Number ID",
                "Numerical ID from WhatsApp dashboard",
                phone,
                false,
            ));
        }

        // 16. Signal Bridge
        if self.is_service_enabled("signal") {
            let url = self
                .config
                .extra
                .get("signalApiUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("http://localhost:8080")
                .to_string();
            let number = self
                .config
                .extra
                .get("signalNumber")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "signal_url",
                "signal",
                "Signal Bridge",
                "API URL",
                "http://localhost:8080",
                url,
                false,
            ));
            fields.push(make_text_field(
                "signal_number",
                "signal",
                "Signal Bridge",
                "Phone Number (E.164)",
                "+15551234567",
                number,
                false,
            ));
        }

        // 17. Email Bridge
        if self.is_service_enabled("email") {
            let client_id = self
                .config
                .extra
                .get("gmailClientId")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let client_secret = self
                .config
                .extra
                .get("gmailClientSecret")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            fields.push(make_text_field(
                "gmail_client_id",
                "email",
                "Email Bridge (Gmail)",
                "Client ID",
                "Google Cloud OAuth Client ID",
                client_id,
                false,
            ));
            fields.push(make_text_field(
                "gmail_client_secret",
                "email",
                "Email Bridge (Gmail)",
                "Client Secret",
                "Google Cloud OAuth Client Secret",
                client_secret,
                true,
            ));
        }

        // 18. Image: NanoBanana
        if self.is_service_enabled("img_nanobanana") {
            let presets = vec![
                "imagen-3.0-generate-002".to_string(),
                "imagen-3.0-fast-generate-001".to_string(),
            ];
            fields.push(make_choice_field(
                "nanobanana_model",
                "img_nanobanana",
                "NanoBanana (Gemini Images)",
                "Model",
                "Imagen 3 image generation model",
                &self.config.nanobanana_model,
                presets,
            ));
        }

        // 19. Image: DALL·E
        if self.is_service_enabled("img_dalle") {
            let presets = vec!["dall-e-3".to_string(), "dall-e-2".to_string()];
            fields.push(make_choice_field(
                "dalle_model",
                "img_dalle",
                "OpenAI DALL·E",
                "Model",
                "DALL·E image generation model",
                &self.config.dalle_model,
                presets,
            ));
        }

        // 20. Image: Stability AI
        if self.is_service_enabled("img_stability") {
            fields.push(make_text_field(
                "stability_key",
                "img_stability",
                "Stability AI",
                "API Key",
                "sk-...",
                self.config.stability_api_key.clone(),
                true,
            ));
            let presets = vec![
                "sd3.5-large".to_string(),
                "sd3.5-large-turbo".to_string(),
                "sd3-medium".to_string(),
                "stable-image-core".to_string(),
            ];
            fields.push(make_choice_field(
                "stability_model",
                "img_stability",
                "Stability AI",
                "Model",
                "Stable Diffusion image model",
                &self.config.stability_model,
                presets,
            ));
        }

        // 21. Image: Ideogram
        if self.is_service_enabled("img_ideogram") {
            fields.push(make_text_field(
                "ideogram_key",
                "img_ideogram",
                "Ideogram v3",
                "API Key",
                "Ideogram API key",
                self.config.ideogram_api_key.clone(),
                true,
            ));
            let presets = vec![
                "V_3".to_string(),
                "V_2".to_string(),
                "V_2_TURBO".to_string(),
            ];
            fields.push(make_choice_field(
                "ideogram_model",
                "img_ideogram",
                "Ideogram v3",
                "Model Version",
                "Ideogram model version",
                &self.config.ideogram_model,
                presets,
            ));
        }

        // 22. Image: Replicate
        if self.is_service_enabled("img_replicate") {
            fields.push(make_text_field(
                "replicate_key",
                "img_replicate",
                "Replicate (FLUX / SDXL)",
                "API Token",
                "r8_...",
                self.config.replicate_api_key.clone(),
                true,
            ));
            let presets = vec![
                "black-forest-labs/flux-schnell".to_string(),
                "black-forest-labs/flux-dev".to_string(),
                "stability-ai/sdxl".to_string(),
            ];
            fields.push(make_choice_field(
                "replicate_model",
                "img_replicate",
                "Replicate (FLUX / SDXL)",
                "Model",
                "FLUX or SDXL model",
                &self.config.replicate_model,
                presets,
            ));
        }

        // 23. Image: BFL
        if self.is_service_enabled("img_bfl") {
            fields.push(make_text_field(
                "bfl_key",
                "img_bfl",
                "Black Forest Labs (FLUX API)",
                "API Key",
                "BFL API key",
                self.config.bfl_api_key.clone(),
                true,
            ));
            let presets = vec![
                "flux-pro-1.1".to_string(),
                "flux-pro-1.1-ultra".to_string(),
                "flux-dev".to_string(),
                "flux-pro-1.0-fill".to_string(),
            ];
            fields.push(make_choice_field(
                "bfl_model",
                "img_bfl",
                "Black Forest Labs (FLUX API)",
                "Model",
                "Official FLUX model",
                &self.config.bfl_model,
                presets,
            ));
        }

        // 24. Video: Veo
        if self.is_service_enabled("vid_veo") {
            let curr = self
                .config
                .extra
                .get("veoModel")
                .and_then(|v| v.as_str())
                .unwrap_or("veo-3.1-generate-preview");
            let presets = vec![
                "veo-3.1-generate-preview".to_string(),
                "veo-2.0-generate-preview".to_string(),
            ];
            fields.push(make_choice_field(
                "veo_model",
                "vid_veo",
                "Google Veo (Gemini Videos)",
                "Model",
                "Veo video generation model",
                curr,
                presets,
            ));
        }

        // 25. Voice: Gemini Live
        if self.is_service_enabled("gemini_live") {
            let curr = self
                .config
                .extra
                .get("geminiLiveModel")
                .and_then(|v| v.as_str())
                .unwrap_or("gemini-2.5-flash-native-audio-preview-12-2025");
            let presets = vec![
                "gemini-2.5-flash-native-audio-preview-12-2025".to_string(),
                "gemini-2.0-flash-exp".to_string(),
            ];
            fields.push(make_choice_field(
                "gemini_live_model",
                "gemini_live",
                "Realtime Live Model (Gemini Live)",
                "Realtime Voice Model",
                "Gemini Live realtime audio model",
                curr,
                presets,
            ));
        }

        let mut service_groups: Vec<ConfigServiceGroup> = Vec::new();
        for (i, field) in fields.iter().enumerate() {
            if let Some(group) = service_groups
                .iter_mut()
                .find(|g| g.service_key == field.service_key)
            {
                group.field_indices.push(i);
            } else {
                let desc = self
                    .services
                    .iter()
                    .find(|s| s.key == field.service_key)
                    .map(|s| s.description)
                    .unwrap_or(field.hint);
                service_groups.push(ConfigServiceGroup {
                    service_key: field.service_key,
                    service_name: field.service_name,
                    description: desc,
                    field_indices: vec![i],
                    focused_field: 0,
                });
            }
        }

        self.fields = fields;
        self.service_groups = service_groups;
        self.service_group_idx = 0;
        self.field_scroll_offset = 0;
    }

    pub fn apply_all_to_config(&mut self) {
        // Step 1: Gemini API Key
        self.config.api_key = self.gemini_key.trim().to_string();

        // Step 2: Gemini Model Selection
        if self.is_custom_gemini_model {
            self.config.gemini_model = self.custom_gemini_model.trim().to_string();
        } else if let Some(m) = self.gemini_models.get(self.gemini_model_idx) {
            if m != "Custom model..." {
                self.config.gemini_model = m.clone();
            }
        }

        // Step 3 & 4: Clear disabled services
        for svc in &self.services {
            if !svc.enabled {
                match svc.key {
                    "anthropic" => {
                        self.config.anthropic_api_key.clear();
                    }
                    "openai" => {
                        self.config.openai_api_key.clear();
                    }
                    "openrouter" => {
                        self.config.openrouter_api_key.clear();
                    }
                    "deepseek" => {
                        self.config.deepseek_api_key.clear();
                    }
                    "huggingface" => {
                        self.config.hf_api_key.clear();
                    }
                    "local_openai" => {
                        self.config.local_api_base_url.clear();
                    }
                    "ollama" => {
                        self.config.ollama_host.clear();
                        self.config.ollama_model.clear();
                    }
                    "google_search" => {
                        self.config.extra.insert(
                            "googleSearchApiKey".to_string(),
                            serde_json::Value::String(String::new()),
                        );
                        self.config.extra.insert(
                            "googleSearchCx".to_string(),
                            serde_json::Value::String(String::new()),
                        );
                    }
                    "brave_search" => {
                        self.config.extra.insert(
                            "braveSearchApiKey".to_string(),
                            serde_json::Value::String(String::new()),
                        );
                    }
                    "searxng" => {
                        self.config.extra.insert(
                            "searxngBaseUrl".to_string(),
                            serde_json::Value::String(String::new()),
                        );
                    }
                    "telegram" => {
                        self.config.extra.insert(
                            "enableTelegramBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "discord" => {
                        self.config.extra.insert(
                            "enableDiscordBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "slack" => {
                        self.config.extra.insert(
                            "enableSlackBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "line" => {
                        self.config.extra.insert(
                            "enableLineBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "whatsapp" => {
                        self.config.extra.insert(
                            "enableWhatsappBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "signal" => {
                        self.config.extra.insert(
                            "enableSignalBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "email" => {
                        self.config.extra.insert(
                            "enableEmailBridge".to_string(),
                            serde_json::Value::Bool(false),
                        );
                    }
                    "img_stability" => {
                        self.config.stability_api_key.clear();
                    }
                    "img_ideogram" => {
                        self.config.ideogram_api_key.clear();
                    }
                    "img_replicate" => {
                        self.config.replicate_api_key.clear();
                    }
                    "img_bfl" => {
                        self.config.bfl_api_key.clear();
                    }
                    "vid_veo" => {
                        if self
                            .config
                            .extra
                            .get("videoGenProvider")
                            .and_then(|v| v.as_str())
                            == Some("veo")
                        {
                            self.config.extra.remove("videoGenProvider");
                        }
                    }
                    "gemini_live" => {
                        if self.config.extra.get("voiceMode").and_then(|v| v.as_str())
                            == Some("geminiLive")
                        {
                            self.config.extra.remove("voiceMode");
                        }
                    }
                    _ => {}
                }
            }
        }

        // Apply updated form fields
        for field in &self.fields {
            let chosen_val = if field.is_custom {
                field.custom_value.trim().to_string()
            } else if let FieldType::Choice {
                ref options,
                selected,
            } = field.field_type
            {
                options
                    .get(selected)
                    .cloned()
                    .unwrap_or_else(|| field.value.clone())
            } else {
                field.value.trim().to_string()
            };

            match field.id {
                "anthropic_key" => self.config.anthropic_api_key = chosen_val,
                "anthropic_model" => self.config.anthropic_model = chosen_val,
                "openai_key" => self.config.openai_api_key = chosen_val,
                "openai_model" => self.config.openai_model = chosen_val,
                "openrouter_key" => self.config.openrouter_api_key = chosen_val,
                "openrouter_model" => self.config.openrouter_model = chosen_val,
                "deepseek_key" => self.config.deepseek_api_key = chosen_val,
                "deepseek_model" => self.config.deepseek_model = chosen_val,
                "hf_key" => self.config.hf_api_key = chosen_val,
                "hf_model" => self.config.hf_model = chosen_val,
                "local_url" => self.config.local_api_base_url = chosen_val,
                "local_model" => self.config.local_model_name = chosen_val,
                "ollama_host" => self.config.ollama_host = chosen_val,
                "ollama_model" => self.config.ollama_model = chosen_val,
                "google_search_key" => {
                    self.config.extra.insert(
                        "googleSearchApiKey".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "google_search_cx" => {
                    self.config.extra.insert(
                        "googleSearchCx".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "brave_search_key" => {
                    self.config.extra.insert(
                        "braveSearchApiKey".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "searxng_url" => {
                    self.config.extra.insert(
                        "searxngBaseUrl".to_string(),
                        serde_json::Value::String(chosen_val.trim_end_matches('/').to_string()),
                    );
                }
                "telegram_token" => {
                    self.config.extra.insert(
                        "telegramBotToken".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableTelegramBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "discord_token" => {
                    self.config.extra.insert(
                        "discordBotToken".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableDiscordBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "discord_app_id" => {
                    self.config.extra.insert(
                        "discordApplicationId".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "slack_bot_token" => {
                    self.config.extra.insert(
                        "slackBotToken".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableSlackBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "slack_app_token" => {
                    self.config.extra.insert(
                        "slackAppToken".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "line_token" => {
                    self.config.extra.insert(
                        "lineChannelAccessToken".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableLineBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "line_secret" => {
                    self.config.extra.insert(
                        "lineChannelSecret".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "whatsapp_token" => {
                    self.config.extra.insert(
                        "whatsappCloudAccessToken".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableWhatsappBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "whatsapp_phone" => {
                    self.config.extra.insert(
                        "whatsappPhoneNumberId".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "signal_url" => {
                    self.config.extra.insert(
                        "signalApiUrl".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableSignalBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "signal_number" => {
                    self.config.extra.insert(
                        "signalNumber".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "gmail_client_id" => {
                    self.config.extra.insert(
                        "gmailClientId".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "enableEmailBridge".to_string(),
                        serde_json::Value::Bool(true),
                    );
                }
                "gmail_client_secret" => {
                    self.config.extra.insert(
                        "gmailClientSecret".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                }
                "nanobanana_model" => self.config.nanobanana_model = chosen_val,
                "dalle_model" => self.config.dalle_model = chosen_val,
                "stability_key" => self.config.stability_api_key = chosen_val,
                "stability_model" => self.config.stability_model = chosen_val,
                "ideogram_key" => self.config.ideogram_api_key = chosen_val,
                "ideogram_model" => self.config.ideogram_model = chosen_val,
                "replicate_key" => self.config.replicate_api_key = chosen_val,
                "replicate_model" => self.config.replicate_model = chosen_val,
                "bfl_key" => self.config.bfl_api_key = chosen_val,
                "bfl_model" => self.config.bfl_model = chosen_val,
                "veo_model" => {
                    self.config.extra.insert(
                        "veoModel".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "videoGenProvider".to_string(),
                        serde_json::Value::String("veo".to_string()),
                    );
                }
                "gemini_live_model" => {
                    self.config.extra.insert(
                        "geminiLiveModel".to_string(),
                        serde_json::Value::String(chosen_val),
                    );
                    self.config.extra.insert(
                        "voiceMode".to_string(),
                        serde_json::Value::String("geminiLive".to_string()),
                    );
                }
                _ => {}
            }
        }
    }

    fn trigger_gemini_verify(&mut self) {
        let key = self.gemini_key.trim().to_string();
        if key.is_empty() {
            return;
        }

        self.is_verifying = true;
        self.verify_error = None;
        let (tx, rx) = mpsc::channel();
        self.verify_rx = Some(rx);

        let mut test_config = self.config.clone();
        test_config.api_key = key;

        tokio::spawn(async move {
            let models =
                mint_core::slash::models::model_options_for_provider_async(&test_config, "gemini")
                    .await;
            if models.is_empty() {
                let _ = tx.send(Err("Could not retrieve models with this key. Check internet connection or key validity.".to_string()));
            } else {
                let _ = tx.send(Ok(models));
            }
        });
    }

    fn trigger_active_service_fetch(&mut self) {
        if self.service_group_idx >= self.service_groups.len() {
            return;
        }
        let group = &self.service_groups[self.service_group_idx];
        let svc_key = group.service_key.to_string();

        let has_choice = group.field_indices.iter().any(|&idx| {
            matches!(
                self.fields.get(idx).map(|f| &f.field_type),
                Some(FieldType::Choice { .. })
            )
        });
        if !has_choice {
            return;
        }

        let key_val = if let Some(&first_idx) = group.field_indices.first() {
            if let Some(f) = self.fields.get(first_idx) {
                if matches!(f.field_type, FieldType::Text) {
                    f.value.trim().to_string()
                } else {
                    String::new()
                }
            } else {
                String::new()
            }
        } else {
            String::new()
        };

        let mut test_config = self.config.clone();
        match svc_key.as_str() {
            "anthropic" => test_config.anthropic_api_key = key_val,
            "openai" => test_config.openai_api_key = key_val,
            "deepseek" => test_config.deepseek_api_key = key_val,
            "openrouter" => test_config.openrouter_api_key = key_val,
            "local_openai" => test_config.local_api_base_url = key_val,
            "ollama" => test_config.ollama_host = key_val,
            "img_stability" => test_config.stability_api_key = key_val,
            "img_ideogram" => test_config.ideogram_api_key = key_val,
            "img_replicate" => test_config.replicate_api_key = key_val,
            "img_bfl" => test_config.bfl_api_key = key_val,
            _ => {}
        }

        let (tx, rx) = mpsc::channel();
        self.service_fetch_rx = Some(rx);
        self.is_fetching_service_models = true;

        tokio::spawn(async move {
            let models = fetch_models_for_provider_async(&test_config, &svc_key).await;
            let _ = tx.send((svc_key, models));
        });
    }

    fn poll_async_tasks(&mut self) {
        // Step 1: Gemini verification
        if let Some(rx) = &self.verify_rx {
            if let Ok(res) = rx.try_recv() {
                self.is_verifying = false;
                match res {
                    Ok(models) => {
                        let mut updated = models;
                        if !updated.iter().any(|m| m == "Custom model...") {
                            updated.push("Custom model...".to_string());
                        }
                        let curr_sel = if self.is_custom_gemini_model {
                            self.custom_gemini_model.clone()
                        } else {
                            self.gemini_models
                                .get(self.gemini_model_idx)
                                .cloned()
                                .unwrap_or_default()
                        };
                        self.gemini_models = updated;
                        if let Some(pos) = self.gemini_models.iter().position(|m| m == &curr_sel) {
                            self.gemini_model_idx = pos;
                            self.is_custom_gemini_model = false;
                        } else if self.is_custom_gemini_model {
                            self.gemini_model_idx = self.gemini_models.len().saturating_sub(1);
                        } else if self.gemini_model_idx >= self.gemini_models.len() {
                            self.gemini_model_idx = 0;
                        }
                    }
                    Err(err) => {
                        self.verify_error = Some(err);
                    }
                }
                self.verify_rx = None;
            }
        }

        // Step 3: Service models discovery
        if let Some(rx) = &self.service_fetch_rx {
            if let Ok((service_key, models)) = rx.try_recv() {
                self.is_fetching_service_models = false;
                if !models.is_empty() {
                    for f in &mut self.fields {
                        if f.service_key == service_key {
                            if let FieldType::Choice {
                                ref mut options,
                                ref mut selected,
                            } = f.field_type
                            {
                                let curr = if f.is_custom {
                                    f.custom_value.clone()
                                } else {
                                    options.get(*selected).cloned().unwrap_or_default()
                                };

                                let mut new_options = models.clone();
                                if !new_options.iter().any(|m| m == "Custom model...") {
                                    new_options.push("Custom model...".to_string());
                                }

                                if let Some(pos) = new_options.iter().position(|m| m == &curr) {
                                    *selected = pos;
                                    f.is_custom = false;
                                } else if f.is_custom {
                                    *selected = new_options.len().saturating_sub(1);
                                } else {
                                    *selected = 0;
                                }
                                *options = new_options;
                            }
                        }
                    }
                }
                self.service_fetch_rx = None;
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Key Handling
    // ─────────────────────────────────────────────────────────────────────────

    pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<OnboardOutcome> {
        let is_ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let is_shift = key.modifiers.contains(KeyModifiers::SHIFT);

        if is_ctrl && key.code == KeyCode::Char('c') {
            return Some(OnboardOutcome::Cancelled);
        }

        // Global Back: Esc or Ctrl+B
        if (is_ctrl && key.code == KeyCode::Char('b')) || key.code == KeyCode::Esc {
            match self.step {
                OnboardStep::CoreAi => return Some(OnboardOutcome::Cancelled),
                OnboardStep::Services => {
                    self.step = OnboardStep::CoreAi;
                    return None;
                }
                OnboardStep::Config => {
                    if self.service_group_idx > 0 {
                        self.service_group_idx -= 1;
                    } else {
                        self.step = OnboardStep::Services;
                    }
                    return None;
                }
                OnboardStep::Summary => {
                    if !self.service_groups.is_empty() {
                        self.service_group_idx = self.service_groups.len().saturating_sub(1);
                        self.step = OnboardStep::Config;
                    } else {
                        self.step = OnboardStep::Services;
                    }
                    return None;
                }
            }
        }

        match self.step {
            OnboardStep::CoreAi => self.handle_step1_core_ai_key(key, is_ctrl),
            OnboardStep::Services => self.handle_step2_services_key(key, is_ctrl),
            OnboardStep::Config => self.handle_step3_config_key(key, is_ctrl, is_shift),
            OnboardStep::Summary => self.handle_step4_summary_key(key, is_ctrl),
        }
    }

    fn handle_step1_core_ai_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        is_ctrl: bool,
    ) -> Option<OnboardOutcome> {
        if is_ctrl && key.code == KeyCode::Char('h') {
            self.gemini_masked = !self.gemini_masked;
            return None;
        }

        // Tab switches focus between API Key (0) and Model List (1)
        if key.code == KeyCode::Tab || key.code == KeyCode::BackTab {
            self.gemini_focus = if self.gemini_focus == 0 { 1 } else { 0 };
            if self.gemini_focus == 1
                && !self.gemini_key.trim().is_empty()
                && self.verify_rx.is_none()
                && !self.is_verifying
            {
                self.trigger_gemini_verify();
            }
            return None;
        }

        if self.gemini_focus == 0 {
            // Focused on API Key
            match key.code {
                KeyCode::Down => {
                    self.gemini_focus = 1;
                    if !self.gemini_key.trim().is_empty()
                        && self.verify_rx.is_none()
                        && !self.is_verifying
                    {
                        self.trigger_gemini_verify();
                    }
                }
                KeyCode::Enter => {
                    if !self.gemini_key.trim().is_empty()
                        && self.verify_rx.is_none()
                        && !self.is_verifying
                    {
                        self.trigger_gemini_verify();
                    }
                    self.gemini_focus = 1;
                }
                _ => {
                    Self::edit_text(&mut self.gemini_key, key, is_ctrl);
                    self.verify_error = None;
                }
            }
        } else {
            // Focused on Model List
            let len = self.gemini_models.len();
            match key.code {
                KeyCode::Up => {
                    if self.is_custom_gemini_model {
                        if len > 0 {
                            self.gemini_model_idx = self.gemini_model_idx.saturating_sub(1);
                            self.is_custom_gemini_model = false;
                        }
                    } else if self.gemini_model_idx > 0 {
                        self.gemini_model_idx -= 1;
                    } else {
                        // At top of model list, Up switches back to API Key
                        self.gemini_focus = 0;
                    }
                }
                KeyCode::Down => {
                    if len > 0 && self.gemini_model_idx < len - 1 {
                        self.gemini_model_idx += 1;
                        self.is_custom_gemini_model = self
                            .gemini_models
                            .get(self.gemini_model_idx)
                            .map(|s| s.as_str())
                            == Some("Custom model...");
                    }
                }
                KeyCode::Enter => {
                    if self.is_custom_gemini_model {
                        if !self.custom_gemini_model.trim().is_empty() {
                            self.config.gemini_model = self.custom_gemini_model.trim().to_string();
                            self.config.api_key = self.gemini_key.trim().to_string();
                            self.step = OnboardStep::Services;
                        }
                    } else {
                        if let Some(m) = self.gemini_models.get(self.gemini_model_idx) {
                            if m == "Custom model..." {
                                self.is_custom_gemini_model = true;
                                return None;
                            }
                            self.config.gemini_model = m.clone();
                        }
                        self.config.api_key = self.gemini_key.trim().to_string();
                        self.step = OnboardStep::Services;
                    }
                }
                _ => {
                    if self.is_custom_gemini_model {
                        Self::edit_text(&mut self.custom_gemini_model, key, is_ctrl);
                    }
                }
            }
        }
        None
    }

    fn handle_step2_services_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        _is_ctrl: bool,
    ) -> Option<OnboardOutcome> {
        match key.code {
            KeyCode::Up => {
                if !self.services.is_empty() {
                    self.service_cursor = if self.service_cursor > 0 {
                        self.service_cursor - 1
                    } else {
                        self.services.len() - 1
                    };
                }
            }
            KeyCode::Down => {
                if !self.services.is_empty() {
                    self.service_cursor = if self.service_cursor < self.services.len() - 1 {
                        self.service_cursor + 1
                    } else {
                        0
                    };
                }
            }
            KeyCode::Char(' ') => {
                if let Some(svc) = self.services.get_mut(self.service_cursor) {
                    svc.enabled = !svc.enabled;
                }
            }
            KeyCode::Char('a') => {
                let any_disabled = self.services.iter().any(|s| !s.enabled);
                for s in &mut self.services {
                    s.enabled = any_disabled;
                }
            }
            KeyCode::Char('i') => {
                for s in &mut self.services {
                    s.enabled = !s.enabled;
                }
            }
            KeyCode::Enter => {
                self.build_config_fields();
                if self.service_groups.is_empty() {
                    self.apply_all_to_config();
                    self.step = OnboardStep::Summary;
                } else {
                    self.service_group_idx = 0;
                    self.step = OnboardStep::Config;
                    self.trigger_active_service_fetch();
                }
            }
            _ => {}
        }
        None
    }

    fn handle_step3_config_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        is_ctrl: bool,
        _is_shift: bool,
    ) -> Option<OnboardOutcome> {
        if self.service_groups.is_empty() {
            self.apply_all_to_config();
            self.step = OnboardStep::Summary;
            return None;
        }

        let group_idx = self.service_group_idx;
        let num_fields = self.service_groups[group_idx].field_indices.len();
        let focused_field_rel = self.service_groups[group_idx].focused_field;
        let field_idx = self.service_groups[group_idx].field_indices[focused_field_rel];

        // Ctrl+H toggles show_secret if field is secret
        if is_ctrl && key.code == KeyCode::Char('h') {
            if let Some(field) = self.fields.get_mut(field_idx) {
                if field.is_secret {
                    field.show_secret = !field.show_secret;
                }
            }
            return None;
        }

        // Tab or BackTab switches between fields within this service
        if key.code == KeyCode::Tab || key.code == KeyCode::BackTab {
            if num_fields > 1 {
                let next_rel = if focused_field_rel == 0 { 1 } else { 0 };
                self.service_groups[group_idx].focused_field = next_rel;
                if next_rel == 1 {
                    self.trigger_active_service_fetch();
                }
            }
            return None;
        }

        let is_choice = matches!(
            self.fields.get(field_idx).map(|f| &f.field_type),
            Some(FieldType::Choice { .. })
        );

        if is_choice {
            let field = &mut self.fields[field_idx];
            if let FieldType::Choice {
                ref options,
                ref mut selected,
            } = field.field_type
            {
                let len = options.len();
                match key.code {
                    KeyCode::Up => {
                        if field.is_custom {
                            if len > 0 {
                                *selected = selected.saturating_sub(1);
                                field.is_custom = false;
                            }
                        } else if *selected > 0 {
                            *selected -= 1;
                        } else if num_fields > 1 && focused_field_rel == 1 {
                            // At top of model list, Up switches back to the Key input box
                            self.service_groups[group_idx].focused_field = 0;
                        }
                    }
                    KeyCode::Down => {
                        if len > 0 && *selected < len - 1 {
                            *selected += 1;
                            field.is_custom = options.get(*selected).map(|s| s.as_str())
                                == Some("Custom model...");
                        }
                    }
                    KeyCode::Enter => {
                        if field.is_custom {
                            if !field.custom_value.trim().is_empty() {
                                field.value = field.custom_value.trim().to_string();
                                self.advance_service_group();
                            }
                        } else if let Some(opt) = options.get(*selected) {
                            if opt == "Custom model..." {
                                field.is_custom = true;
                                return None;
                            }
                            field.value = opt.clone();
                            self.advance_service_group();
                        }
                    }
                    _ => {
                        if field.is_custom {
                            Self::edit_text(&mut field.custom_value, key, is_ctrl);
                        }
                    }
                }
            }
        } else {
            // Text field
            match key.code {
                KeyCode::Down => {
                    if num_fields > 1 && focused_field_rel == 0 {
                        self.service_groups[group_idx].focused_field = 1;
                        self.trigger_active_service_fetch();
                    }
                }
                KeyCode::Up => {
                    if num_fields > 1 && focused_field_rel == 1 {
                        self.service_groups[group_idx].focused_field = 0;
                    }
                }
                KeyCode::Enter => {
                    if num_fields > 1 && focused_field_rel == 0 {
                        // Advance to the model picker in the same service
                        self.service_groups[group_idx].focused_field = 1;
                        self.trigger_active_service_fetch();
                    } else {
                        // Advance to the next service
                        self.advance_service_group();
                    }
                }
                _ => {
                    if let Some(field) = self.fields.get_mut(field_idx) {
                        Self::edit_text(&mut field.value, key, is_ctrl);
                    }
                }
            }
        }
        None
    }

    fn advance_service_group(&mut self) {
        if self.service_group_idx + 1 < self.service_groups.len() {
            self.service_group_idx += 1;
            self.trigger_active_service_fetch();
        } else {
            self.apply_all_to_config();
            self.step = OnboardStep::Summary;
        }
    }

    fn handle_step4_summary_key(
        &mut self,
        key: crossterm::event::KeyEvent,
        _is_ctrl: bool,
    ) -> Option<OnboardOutcome> {
        match key.code {
            KeyCode::Left | KeyCode::Up => {
                self.summary_action = if self.summary_action > 0 {
                    self.summary_action - 1
                } else {
                    2
                };
            }
            KeyCode::Right | KeyCode::Down | KeyCode::Tab => {
                self.summary_action = if self.summary_action < 2 {
                    self.summary_action + 1
                } else {
                    0
                };
            }
            KeyCode::Enter => match self.summary_action {
                0 => {
                    self.apply_all_to_config();
                    let _ = save_config(&self.config);
                    return Some(OnboardOutcome::SavedAndLaunch);
                }
                1 => {
                    self.apply_all_to_config();
                    let _ = save_config(&self.config);
                    return Some(OnboardOutcome::SavedAndExit);
                }
                _ => {
                    if !self.service_groups.is_empty() {
                        self.service_group_idx = self.service_groups.len().saturating_sub(1);
                        self.step = OnboardStep::Config;
                    } else {
                        self.step = OnboardStep::Services;
                    }
                    return None;
                }
            },
            _ => {}
        }
        None
    }

    fn edit_text(target: &mut String, key: crossterm::event::KeyEvent, is_ctrl: bool) {
        if is_ctrl && key.code == KeyCode::Char('v') {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                if let Ok(text) = clipboard.get_text() {
                    target.push_str(text.trim());
                }
            }
            return;
        }
        if is_ctrl && key.code == KeyCode::Char('u') {
            target.clear();
            return;
        }

        match key.code {
            KeyCode::Backspace => {
                target.pop();
            }
            KeyCode::Char(c) if !is_ctrl => {
                target.push(c);
            }
            _ => {}
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // UI Rendering
    // ─────────────────────────────────────────────────────────────────────────

    pub fn render(&mut self, frame: &mut ratatui::Frame) {
        let area = frame.area();
        frame.render_widget(Clear, area);

        // Full-screen layout filling the entire terminal window
        let main_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // Stepper Header
                Constraint::Min(10),   // Body (Takes all remaining screen space)
                Constraint::Length(1), // Footer Shortcuts
            ])
            .split(area);

        self.render_stepper_header(frame, main_chunks[0]);

        match self.step {
            OnboardStep::CoreAi => self.render_step_1_core_ai(frame, main_chunks[1]),
            OnboardStep::Services => self.render_step_2_services(frame, main_chunks[1]),
            OnboardStep::Config => self.render_step_3_config(frame, main_chunks[1]),
            OnboardStep::Summary => self.render_step_4_summary(frame, main_chunks[1]),
        }

        self.render_footer(frame, main_chunks[2]);
    }

    fn render_stepper_header(&self, frame: &mut ratatui::Frame, area: Rect) {
        let current_idx = self.step.index();

        let step3_title = if self.step == OnboardStep::Config && !self.service_groups.is_empty() {
            let curr = self.service_group_idx + 1;
            let total = self.service_groups.len();
            format!("3. Config ({curr}/{total})")
        } else {
            "3. Config".to_string()
        };

        let step_spans = vec![
            Self::step_badge("1. Core AI", current_idx == 0, current_idx > 0),
            Span::styled(" ── ", Style::default().fg(MUTED)),
            Self::step_badge("2. Services", current_idx == 1, current_idx > 1),
            Span::styled(" ── ", Style::default().fg(MUTED)),
            Self::step_badge(&step3_title, current_idx == 2, current_idx > 2),
            Span::styled(" ── ", Style::default().fg(MUTED)),
            Self::step_badge("4. Finish", current_idx == 3, false),
        ];

        let header_line = Line::from(step_spans);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BLUE))
            .title(Span::styled(
                " Mint Setup Wizard ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ));

        let paragraph = Paragraph::new(header_line)
            .block(block)
            .alignment(Alignment::Center);

        frame.render_widget(paragraph, area);
    }

    fn step_badge(title: &str, is_current: bool, is_done: bool) -> Span<'static> {
        if is_current {
            Span::styled(
                format!(" [{title}] "),
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            )
        } else if is_done {
            Span::styled(
                format!(" ✔ {title} "),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(format!(" {title} "), Style::default().fg(MUTED))
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Screen 1: Core AI (Google Gemini API Key + Model Selection)
    // ─────────────────────────────────────────────────────────────────────────

    fn render_step_1_core_ai(&self, frame: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Step 1: Core AI (Google Gemini) ",
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2),                                               // Subtitle
                Constraint::Length(3),                                               // Key input
                Constraint::Length(1),                                               // Tip line
                Constraint::Min(8), // Vertical Model List
                Constraint::Length(if self.is_custom_gemini_model { 3 } else { 0 }), // Custom input
                Constraint::Length(2), // Bottom Action Hint
            ])
            .split(inner);

        // Subtitle
        let subtitle = Paragraph::new(
            "Gemini is the primary engine powering Mint CLI. Enter your API key and select a model.\nYour key is stored locally in ~/.config/mint/config.json."
        ).style(Style::default().fg(MUTED));
        frame.render_widget(subtitle, chunks[0]);

        // Key Input Box
        let key_disp = if self.gemini_key.is_empty() {
            Span::styled(
                "(Paste your Gemini API key here with Ctrl+V)",
                Style::default().fg(MUTED),
            )
        } else if self.gemini_masked {
            Span::styled(
                "•".repeat(self.gemini_key.chars().count().min(50)),
                Style::default().fg(TEXT),
            )
        } else {
            Span::styled(&self.gemini_key, Style::default().fg(TEXT))
        };

        let mask_hint = if self.gemini_masked {
            "[Ctrl+H: Show]"
        } else {
            "[Ctrl+H: Hide]"
        };

        let key_border_color = if self.gemini_focus == 0 {
            ACCENT
        } else {
            SUBTLE
        };
        let key_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(key_border_color))
            .title(Line::from(vec![
                Span::styled(
                    " API Key ",
                    Style::default()
                        .fg(if self.gemini_focus == 0 { ACCENT } else { TEXT })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(mask_hint, Style::default().fg(BLUE)),
            ]));

        frame.render_widget(Paragraph::new(key_disp).block(key_block), chunks[1]);

        // Tip line
        let tip_line = Line::from(vec![
            Span::styled("• Tip: ", Style::default().fg(MUTED)),
            Span::styled(
                "You can paste from clipboard with Ctrl+V. Press Tab or ↓ to jump to model list.",
                Style::default().fg(MUTED),
            ),
        ]);
        frame.render_widget(Paragraph::new(tip_line), chunks[2]);

        // Vertical Model List
        let model_items: Vec<ListItem> = self
            .gemini_models
            .iter()
            .enumerate()
            .map(|(i, model)| {
                let is_sel = i == self.gemini_model_idx;
                let radio = if is_sel { " ◉ " } else { " ○ " };
                let cursor = if is_sel { " ❯ " } else { "   " };

                let desc = match model.as_str() {
                    "gemini-2.5-flash" => "Recommended · Ultra-fast, multipurpose, thinking",
                    "gemini-2.5-pro" => "Complex reasoning & deep coding",
                    "gemini-3.6-flash" => "Latest experimental release",
                    "gemini-2.0-flash" => "Fast low-latency responses",
                    "gemini-1.5-pro" => "High context window fallback",
                    "gemini-1.5-flash" => "Lightweight fallback",
                    "Custom model..." => "Specify your own model identifier",
                    _ => "",
                };

                let name_style = if is_sel {
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(TEXT)
                };

                let line = Line::from(vec![
                    Span::styled(cursor, Style::default().fg(ACCENT)),
                    Span::styled(
                        radio,
                        Style::default()
                            .fg(if is_sel { ACCENT } else { MUTED })
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!("{:<20}", model), name_style),
                    Span::raw("  "),
                    Span::styled(desc, Style::default().fg(MUTED)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let spinner_chars = ["⠋", "⠙", "⠹", "⠸"];
        let model_status = if self.is_verifying {
            let sp = spinner_chars[self.spinner_tick % spinner_chars.len()];
            format!("[{sp} Discovering models...]")
        } else if self.verify_error.is_some() {
            "[⚠️ Offline presets]".to_string()
        } else {
            format!(
                "[✓ {} models loaded]",
                self.gemini_models.len().saturating_sub(1).max(1)
            )
        };

        let model_border_color = if self.gemini_focus == 1 {
            ACCENT
        } else {
            SUBTLE
        };
        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(model_border_color))
            .title(Line::from(vec![
                Span::styled(
                    " Default Model (↑/↓ to select) ",
                    Style::default()
                        .fg(if self.gemini_focus == 1 { ACCENT } else { TEXT })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" {model_status} "),
                    Style::default().fg(if self.is_verifying { ACCENT } else { BLUE }),
                ),
            ]));

        let mut list_state = ListState::default();
        list_state.select(Some(self.gemini_model_idx));
        frame.render_stateful_widget(
            List::new(model_items).block(list_block),
            chunks[3],
            &mut list_state,
        );

        // Custom Model Input Box
        if self.is_custom_gemini_model {
            let custom_block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(ACCENT))
                .title(Span::styled(
                    " Custom Model Identifier ",
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ));
            let custom_val = if self.custom_gemini_model.is_empty() {
                Span::styled(
                    "(Type custom model identifier, e.g. gemini-2.0-flash-exp)",
                    Style::default().fg(MUTED),
                )
            } else {
                Span::styled(&self.custom_gemini_model, Style::default().fg(TEXT))
            };
            frame.render_widget(Paragraph::new(custom_val).block(custom_block), chunks[4]);
        }

        // Bottom Action Hint
        let action_hint = Paragraph::new(Line::from(vec![
            Span::styled(" [ Esc: Cancel ] ", Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                " [ Enter: Continue to Services ➔ ] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(action_hint, chunks[5]);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Screen 2: Services Checklist (Unified Vertical List)
    // ─────────────────────────────────────────────────────────────────────────

    fn render_step_2_services(&mut self, frame: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Step 2: QuickStart Services & Providers ",
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2), // Subtitle
                Constraint::Min(10),   // Service list
                Constraint::Length(2), // Action hint
            ])
            .split(inner);

        // Subtitle
        let subtitle = Paragraph::new(
            "Select additional AI providers, search engines, and messaging bridges to enable.\nUse ↑/↓ Arrow keys to navigate, Space to toggle [x], and Enter to confirm."
        ).style(Style::default().fg(MUTED));
        frame.render_widget(subtitle, chunks[0]);

        // Build list items with category headers
        let mut list_items = Vec::new();
        let mut cur_category = "";

        for (i, svc) in self.services.iter().enumerate() {
            let is_cursor = i == self.service_cursor;
            let check_icon = if svc.enabled { " ◉ " } else { " ○ " };
            let check_color = if svc.enabled { ACCENT } else { MUTED };

            let cursor_prefix = if is_cursor { " ❯ " } else { "   " };
            let name_style = if is_cursor {
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
            } else if svc.enabled {
                Style::default().fg(TEXT)
            } else {
                Style::default().fg(MUTED)
            };

            let mut lines = Vec::new();
            if svc.category != cur_category {
                cur_category = svc.category;
                lines.push(Line::from(vec![Span::styled(
                    format!("  ── {} ──", svc.category),
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                )]));
            }

            lines.push(Line::from(vec![
                Span::styled(cursor_prefix, Style::default().fg(ACCENT)),
                Span::styled(
                    check_icon,
                    Style::default()
                        .fg(check_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(svc.name, name_style),
                Span::styled(
                    format!("  — {}", svc.description),
                    Style::default().fg(MUTED),
                ),
            ]));

            list_items.push(ListItem::new(lines));
        }

        let list_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(ACCENT))
            .title(Span::styled(
                " Available Services [Space: Toggle | a: All | i: Invert] ",
                Style::default().fg(ACCENT),
            ));

        let mut list_state = ListState::default();
        list_state.select(Some(self.service_cursor));
        frame.render_stateful_widget(
            List::new(list_items).block(list_block),
            chunks[1],
            &mut list_state,
        );

        // Action Line
        let enabled_count = self.services.iter().filter(|s| s.enabled).count();
        let action_hint = Paragraph::new(Line::from(vec![
            Span::styled(
                format!("Enabled: {enabled_count} services   "),
                Style::default().fg(TEXT),
            ),
            Span::styled(" [ ⬅ Back (Esc) ] ", Style::default().fg(MUTED)),
            Span::raw("   "),
            Span::styled(
                " [ Enter: Configure Selected Services ➔ ] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .alignment(Alignment::Right);
        frame.render_widget(action_hint, chunks[2]);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Screen 3: Configuration Form (Grouped per Service, Key + Model unified)
    // ─────────────────────────────────────────────────────────────────────────

    fn render_step_3_config(&self, frame: &mut ratatui::Frame, area: Rect) {
        if self.service_groups.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(SUBTLE))
                .title(Span::styled(
                    " Step 3: Service Configurations ",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ));
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let notice = Paragraph::new(
                "\n  ✔ Core AI (Gemini) is configured.\n\n  No additional services were selected in Step 2.\n  Press [Enter] to proceed to Summary and Save.",
            )
            .style(Style::default().fg(ACCENT));
            frame.render_widget(notice, inner);
            return;
        }

        let group = &self.service_groups[self.service_group_idx];
        let curr_idx = self.service_group_idx + 1;
        let total_services = self.service_groups.len();

        let title = format!(
            " Step 3: Configure {} ({curr_idx} of {total_services}) ",
            group.service_name
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                title,
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let num_fields = group.field_indices.len();
        let focused_rel = group.focused_field;

        if num_fields == 2 {
            let f0 = &self.fields[group.field_indices[0]];
            let f1 = &self.fields[group.field_indices[1]];

            if matches!(f0.field_type, FieldType::Text)
                && matches!(f1.field_type, FieldType::Choice { .. })
            {
                // Unified: Key on top, Model picker below
                let is_custom = f1.is_custom;
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(2),                             // Subtitle
                        Constraint::Length(3),                             // Field 0: API Key / URL
                        Constraint::Length(1),                             // Tip line
                        Constraint::Min(8),                                // Field 1: Model Picker
                        Constraint::Length(if is_custom { 3 } else { 0 }), // Custom input box
                        Constraint::Length(2),                             // Bottom Action Hint
                    ])
                    .split(inner);

                let subtitle = Paragraph::new(format!(
                    "{}.\nKeys and secrets are stored locally in ~/.config/mint/config.json.",
                    group.description
                ))
                .style(Style::default().fg(MUTED));
                frame.render_widget(subtitle, chunks[0]);

                // Field 0: Text Input
                let val_disp = if f0.value.is_empty() {
                    Span::styled(
                        format!("(Type or paste {} here)", f0.label),
                        Style::default().fg(MUTED),
                    )
                } else if f0.is_secret && !f0.show_secret {
                    Span::styled(
                        "•".repeat(f0.value.chars().count().min(50)),
                        Style::default().fg(TEXT),
                    )
                } else {
                    Span::styled(&f0.value, Style::default().fg(TEXT))
                };

                let mask_badge = if f0.is_secret {
                    if f0.show_secret {
                        " [Ctrl+H: Hide]"
                    } else {
                        " [Ctrl+H: Show]"
                    }
                } else {
                    ""
                };

                let f0_border = if focused_rel == 0 { ACCENT } else { SUBTLE };
                let f0_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(f0_border))
                    .title(Line::from(vec![
                        Span::styled(
                            format!(" {} ", f0.label),
                            Style::default()
                                .fg(if focused_rel == 0 { ACCENT } else { TEXT })
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(mask_badge, Style::default().fg(BLUE)),
                    ]));

                frame.render_widget(Paragraph::new(val_disp).block(f0_block), chunks[1]);

                let tip = Line::from(vec![
                    Span::styled("• Tip: ", Style::default().fg(MUTED)),
                    Span::styled(
                        "Press [Tab] or [↓] to switch between API key and model selection.",
                        Style::default().fg(MUTED),
                    ),
                ]);
                frame.render_widget(Paragraph::new(tip), chunks[2]);

                // Field 1: Model Choice Picker
                if let FieldType::Choice {
                    ref options,
                    selected,
                } = f1.field_type
                {
                    let list_items: Vec<ListItem> = options
                        .iter()
                        .enumerate()
                        .map(|(i, opt)| {
                            let is_sel = i == selected;
                            let radio = if is_sel { " ◉ " } else { " ○ " };
                            let cursor = if is_sel { " ❯ " } else { "   " };

                            let name_style = if is_sel {
                                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(TEXT)
                            };

                            let line = Line::from(vec![
                                Span::styled(cursor, Style::default().fg(ACCENT)),
                                Span::styled(
                                    radio,
                                    Style::default()
                                        .fg(if is_sel { ACCENT } else { MUTED })
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(opt, name_style),
                            ]);
                            ListItem::new(line)
                        })
                        .collect();

                    let spinner_chars = ["⠋", "⠙", "⠹", "⠸"];
                    let status_badge = if self.is_fetching_service_models {
                        let sp = spinner_chars[self.spinner_tick % spinner_chars.len()];
                        format!("[{sp} Discovering models...]")
                    } else {
                        format!(
                            "[✓ {} models loaded]",
                            options.len().saturating_sub(1).max(1)
                        )
                    };

                    let f1_border = if focused_rel == 1 { ACCENT } else { SUBTLE };
                    let list_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(f1_border))
                        .title(Line::from(vec![
                            Span::styled(
                                format!(" Default {} (↑/↓ to select) ", f1.label),
                                Style::default()
                                    .fg(if focused_rel == 1 { ACCENT } else { TEXT })
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(" {status_badge} "),
                                Style::default().fg(if self.is_fetching_service_models {
                                    ACCENT
                                } else {
                                    BLUE
                                }),
                            ),
                        ]));

                    let mut list_state = ListState::default();
                    list_state.select(Some(selected));
                    frame.render_stateful_widget(
                        List::new(list_items).block(list_block),
                        chunks[3],
                        &mut list_state,
                    );

                    if is_custom {
                        let custom_block = Block::default()
                            .borders(Borders::ALL)
                            .border_type(BorderType::Rounded)
                            .border_style(Style::default().fg(ACCENT))
                            .title(Span::styled(
                                format!(" Custom {} Identifier ", f1.label),
                                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                            ));
                        let custom_val = if f1.custom_value.is_empty() {
                            Span::styled(
                                "(Type custom model identifier)",
                                Style::default().fg(MUTED),
                            )
                        } else {
                            Span::styled(&f1.custom_value, Style::default().fg(TEXT))
                        };
                        frame.render_widget(
                            Paragraph::new(custom_val).block(custom_block),
                            chunks[4],
                        );
                    }
                }

                let next_label = if curr_idx < total_services {
                    " [ Enter: Next Service ➔ ] "
                } else {
                    " [ Enter: Continue to Finish ➔ ] "
                };

                let action_hint = Paragraph::new(Line::from(vec![
                    Span::styled(" [ Esc: Back ] ", Style::default().fg(MUTED)),
                    Span::raw("   "),
                    Span::styled(
                        next_label,
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Right);
                frame.render_widget(action_hint, chunks[5]);
                return;
            } else {
                // Two text fields (e.g. Bot Token + Chat ID)
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(2), // Subtitle
                        Constraint::Length(3), // Field 0
                        Constraint::Length(1), // Hint 0
                        Constraint::Length(3), // Field 1
                        Constraint::Length(1), // Hint 1
                        Constraint::Min(2),    // Spacing
                        Constraint::Length(2), // Action buttons
                    ])
                    .split(inner);

                let subtitle = Paragraph::new(format!(
                    "{}.\nKeys and secrets are stored locally in ~/.config/mint/config.json.",
                    group.description
                ))
                .style(Style::default().fg(MUTED));
                frame.render_widget(subtitle, chunks[0]);

                for (slot, (f, idx_rel)) in [(&f0, 0), (&f1, 1)].into_iter().enumerate() {
                    let val_disp = if f.value.is_empty() {
                        Span::styled(
                            format!("(Type or paste {} here)", f.label),
                            Style::default().fg(MUTED),
                        )
                    } else if f.is_secret && !f.show_secret {
                        Span::styled(
                            "•".repeat(f.value.chars().count().min(50)),
                            Style::default().fg(TEXT),
                        )
                    } else {
                        Span::styled(&f.value, Style::default().fg(TEXT))
                    };

                    let mask_badge = if f.is_secret {
                        if f.show_secret {
                            " [Ctrl+H: Hide]"
                        } else {
                            " [Ctrl+H: Show]"
                        }
                    } else {
                        ""
                    };

                    let border_color = if focused_rel == idx_rel {
                        ACCENT
                    } else {
                        SUBTLE
                    };
                    let b = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(border_color))
                        .title(Line::from(vec![
                            Span::styled(
                                format!(" {} ", f.label),
                                Style::default()
                                    .fg(if focused_rel == idx_rel { ACCENT } else { TEXT })
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(mask_badge, Style::default().fg(BLUE)),
                        ]));

                    let chunk_input = if slot == 0 { chunks[1] } else { chunks[3] };
                    let chunk_hint = if slot == 0 { chunks[2] } else { chunks[4] };
                    frame.render_widget(Paragraph::new(val_disp).block(b), chunk_input);
                    frame.render_widget(
                        Paragraph::new(format!("  Example / Hint: {}", f.hint))
                            .style(Style::default().fg(MUTED)),
                        chunk_hint,
                    );
                }

                let next_label = if curr_idx < total_services {
                    " [ Enter: Next Service ➔ ] "
                } else {
                    " [ Enter: Continue to Finish ➔ ] "
                };

                let action_hint = Paragraph::new(Line::from(vec![
                    Span::styled(" [ Esc: Back ] ", Style::default().fg(MUTED)),
                    Span::raw("   "),
                    Span::styled(
                        next_label,
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Right);
                frame.render_widget(action_hint, chunks[6]);
                return;
            }
        }

        // Single field service
        let f0 = &self.fields[group.field_indices[0]];
        match &f0.field_type {
            FieldType::Choice { options, selected } => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(2),                                // Subtitle
                        Constraint::Min(8),                                   // Model Picker
                        Constraint::Length(if f0.is_custom { 3 } else { 0 }), // Custom input box
                        Constraint::Length(2),                                // Bottom Action Hint
                    ])
                    .split(inner);

                let subtitle = Paragraph::new(format!(
                    "{}.\nKeys and secrets are stored locally in ~/.config/mint/config.json.",
                    group.description
                ))
                .style(Style::default().fg(MUTED));
                frame.render_widget(subtitle, chunks[0]);

                let list_items: Vec<ListItem> = options
                    .iter()
                    .enumerate()
                    .map(|(i, opt)| {
                        let is_sel = i == *selected;
                        let radio = if is_sel { " ◉ " } else { " ○ " };
                        let cursor = if is_sel { " ❯ " } else { "   " };

                        let name_style = if is_sel {
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)
                        } else {
                            Style::default().fg(TEXT)
                        };

                        let line = Line::from(vec![
                            Span::styled(cursor, Style::default().fg(ACCENT)),
                            Span::styled(
                                radio,
                                Style::default()
                                    .fg(if is_sel { ACCENT } else { MUTED })
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(opt, name_style),
                        ]);
                        ListItem::new(line)
                    })
                    .collect();

                let list_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(ACCENT))
                    .title(Span::styled(
                        format!(
                            " Default {} (↑/↓ to select) [✓ {} models loaded] ",
                            f0.label,
                            options.len().saturating_sub(1).max(1)
                        ),
                        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ));

                let mut list_state = ListState::default();
                list_state.select(Some(*selected));
                frame.render_stateful_widget(
                    List::new(list_items).block(list_block),
                    chunks[1],
                    &mut list_state,
                );

                if f0.is_custom {
                    let custom_block = Block::default()
                        .borders(Borders::ALL)
                        .border_type(BorderType::Rounded)
                        .border_style(Style::default().fg(ACCENT))
                        .title(Span::styled(
                            format!(" Custom {} Identifier ", f0.label),
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        ));
                    let custom_val = if f0.custom_value.is_empty() {
                        Span::styled("(Type custom model identifier)", Style::default().fg(MUTED))
                    } else {
                        Span::styled(&f0.custom_value, Style::default().fg(TEXT))
                    };
                    frame.render_widget(Paragraph::new(custom_val).block(custom_block), chunks[2]);
                }

                let next_label = if curr_idx < total_services {
                    " [ Enter: Next Service ➔ ] "
                } else {
                    " [ Enter: Continue to Finish ➔ ] "
                };

                let action_hint = Paragraph::new(Line::from(vec![
                    Span::styled(" [ Esc: Back ] ", Style::default().fg(MUTED)),
                    Span::raw("   "),
                    Span::styled(
                        next_label,
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Right);
                frame.render_widget(action_hint, chunks[3]);
            }
            FieldType::Text => {
                let chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .margin(1)
                    .constraints([
                        Constraint::Length(2), // Subtitle
                        Constraint::Length(3), // Input box
                        Constraint::Length(2), // Hint / info
                        Constraint::Min(2),    // Spacing
                        Constraint::Length(2), // Bottom Action Hint
                    ])
                    .split(inner);

                let subtitle = Paragraph::new(format!(
                    "{}.\nKeys and secrets are stored locally in ~/.config/mint/config.json.",
                    group.description
                ))
                .style(Style::default().fg(MUTED));
                frame.render_widget(subtitle, chunks[0]);

                let val_disp = if f0.value.is_empty() {
                    Span::styled(
                        format!("(Type or paste {} here)", f0.label),
                        Style::default().fg(MUTED),
                    )
                } else if f0.is_secret && !f0.show_secret {
                    Span::styled(
                        "•".repeat(f0.value.chars().count().min(50)),
                        Style::default().fg(TEXT),
                    )
                } else {
                    Span::styled(&f0.value, Style::default().fg(TEXT))
                };

                let mask_badge = if f0.is_secret {
                    if f0.show_secret {
                        " [Ctrl+H: Hide]"
                    } else {
                        " [Ctrl+H: Show]"
                    }
                } else {
                    ""
                };

                let f0_block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(ACCENT))
                    .title(Line::from(vec![
                        Span::styled(
                            format!(" {} ", f0.label),
                            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(mask_badge, Style::default().fg(BLUE)),
                    ]));

                frame.render_widget(Paragraph::new(val_disp).block(f0_block), chunks[1]);
                frame.render_widget(
                    Paragraph::new(format!("Example / Hint: {}", f0.hint))
                        .style(Style::default().fg(MUTED)),
                    chunks[2],
                );

                let next_label = if curr_idx < total_services {
                    " [ Enter: Next Service ➔ ] "
                } else {
                    " [ Enter: Continue to Finish ➔ ] "
                };

                let action_hint = Paragraph::new(Line::from(vec![
                    Span::styled(" [ Esc: Back ] ", Style::default().fg(MUTED)),
                    Span::raw("   "),
                    Span::styled(
                        next_label,
                        Style::default()
                            .fg(Color::Black)
                            .bg(ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]))
                .alignment(Alignment::Right);
                frame.render_widget(action_hint, chunks[4]);
            }
        }
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Screen 4: Summary & Confirmation
    // ─────────────────────────────────────────────────────────────────────────

    fn render_step_4_summary(&self, frame: &mut ratatui::Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(
                " Step 4: Summary & Confirmation ",
                Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(1), // Subtitle
                Constraint::Min(10),   // Overview card
                Constraint::Length(3), // Action buttons
            ])
            .split(inner);

        frame.render_widget(
            Paragraph::new("Review your workspace configuration before saving:")
                .style(Style::default().fg(MUTED)),
            chunks[0],
        );

        let mut summary_lines = Vec::new();
        let core_model = if self.is_custom_gemini_model {
            &self.custom_gemini_model
        } else {
            self.gemini_models
                .get(self.gemini_model_idx)
                .map(|s| s.as_str())
                .unwrap_or("gemini-2.5-flash")
        };
        let gemini_key_mask = format_masked_key(&self.gemini_key);
        summary_lines.push(Line::from(vec![
            Span::styled(
                "  ✔ Core AI:       ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("Google Gemini ({core_model}) • Key: {gemini_key_mask}"),
                Style::default().fg(TEXT),
            ),
        ]));

        let enabled_ai: Vec<&str> = self
            .services
            .iter()
            .filter(|s| s.category == "AI Providers" && s.enabled)
            .map(|s| s.name)
            .collect();
        if !enabled_ai.is_empty() {
            summary_lines.push(Line::from(vec![
                Span::styled(
                    "  ✔ AI Providers:  ",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(enabled_ai.join(", "), Style::default().fg(TEXT)),
            ]));
        }

        let enabled_search: Vec<&str> = self
            .services
            .iter()
            .filter(|s| s.category == "Search Engines" && s.enabled)
            .map(|s| s.name)
            .collect();
        if !enabled_search.is_empty() {
            summary_lines.push(Line::from(vec![
                Span::styled(
                    "  ✔ Web Search:    ",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(enabled_search.join(", "), Style::default().fg(TEXT)),
            ]));
        }

        let enabled_bridges: Vec<&str> = self
            .services
            .iter()
            .filter(|s| s.category == "Messaging Bridges" && s.enabled)
            .map(|s| s.name)
            .collect();
        if !enabled_bridges.is_empty() {
            summary_lines.push(Line::from(vec![
                Span::styled(
                    "  ✔ Bridges:       ",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(enabled_bridges.join(", "), Style::default().fg(TEXT)),
            ]));
        }

        let enabled_media: Vec<&str> = self
            .services
            .iter()
            .filter(|s| {
                (s.category == "Image Generation"
                    || s.category == "Video Generation"
                    || s.category == "Voice Mode")
                    && s.enabled
            })
            .map(|s| s.name)
            .collect();
        if !enabled_media.is_empty() {
            summary_lines.push(Line::from(vec![
                Span::styled(
                    "  ✔ Media & Voice: ",
                    Style::default().fg(BLUE).add_modifier(Modifier::BOLD),
                ),
                Span::styled(enabled_media.join(", "), Style::default().fg(TEXT)),
            ]));
        }

        summary_lines.push(Line::from(Span::raw("")));
        summary_lines.push(Line::from(vec![
            Span::styled("  Config Location: ", Style::default().fg(MUTED)),
            Span::styled("~/.config/mint/config.json", Style::default().fg(TEXT)),
        ]));

        let summary_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(SUBTLE))
            .title(Span::styled(" Config Overview ", Style::default().fg(TEXT)));
        frame.render_widget(
            Paragraph::new(summary_lines).block(summary_block),
            chunks[1],
        );

        let btn_launch = if self.summary_action == 0 {
            Span::styled(
                " [ Save & Launch Mint ] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" [ Save & Launch Mint ] ", Style::default().fg(ACCENT))
        };
        let btn_exit = if self.summary_action == 1 {
            Span::styled(
                " [ Save & Exit ] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(BLUE)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" [ Save & Exit ] ", Style::default().fg(BLUE))
        };
        let btn_back = if self.summary_action == 2 {
            Span::styled(
                " [ ⬅ Back to Edit ] ",
                Style::default()
                    .fg(Color::Black)
                    .bg(SUBTLE)
                    .add_modifier(Modifier::BOLD),
            )
        } else {
            Span::styled(" [ ⬅ Back to Edit ] ", Style::default().fg(MUTED))
        };

        let actions_line = Line::from(vec![
            btn_launch,
            Span::raw("    "),
            btn_exit,
            Span::raw("    "),
            btn_back,
        ]);
        let actions_p = Paragraph::new(actions_line).alignment(Alignment::Center);
        frame.render_widget(actions_p, chunks[2]);
    }

    fn render_footer(&self, frame: &mut ratatui::Frame, area: Rect) {
        let shortcuts = match self.step {
            OnboardStep::CoreAi => {
                "[Tab] Switch Field  [↑/↓] Select Model  [Ctrl+H] Mask  [Enter] Continue  [Esc] Cancel"
            }
            OnboardStep::Services => {
                "[↑/↓] Move  [Space] Toggle  [a] All  [i] Invert  [Enter] Next  [Esc] Back"
            }
            OnboardStep::Config => {
                "[Tab] Switch Field  [↑/↓] Select Model  [Ctrl+H] Mask  [Enter] Continue  [Esc] Back"
            }
            OnboardStep::Summary => "[←/→ / Tab] Select Action  [Enter] Confirm  [Esc] Back",
        };

        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(20), Constraint::Length(16)])
            .split(area);

        frame.render_widget(
            Paragraph::new(format!("  {shortcuts}")).style(Style::default().fg(MUTED)),
            chunks[0],
        );
        frame.render_widget(
            Paragraph::new("Mint v1.18.0  ")
                .style(Style::default().fg(SUBTLE))
                .alignment(Alignment::Right),
            chunks[1],
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Run Loops
    // ─────────────────────────────────────────────────────────────────────────

    pub fn run_with_terminal<B: Backend>(
        &mut self,
        terminal: &mut Terminal<B>,
    ) -> Result<OnboardOutcome> {
        loop {
            self.spinner_tick = (self.spinner_tick + 1) % 100;
            self.poll_async_tasks();

            terminal.draw(|f| self.render(f))?;

            if event::poll(Duration::from_millis(50))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == crossterm::event::KeyEventKind::Press {
                        if let Some(outcome) = self.handle_key(key) {
                            return Ok(outcome);
                        }
                    }
                }
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Standalone Entry Points & Backward-Compatible Prompt Helpers
// ─────────────────────────────────────────────────────────────────────────────

pub async fn run() -> Result<OnboardOutcome> {
    let config = load_config()?;
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    let _ = stdout.execute(EnterAlternateScreen);
    let _ = stdout.execute(Hide);

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut wizard = OnboardWizard::new(config);
    let outcome = wizard.run_with_terminal(&mut terminal);

    let _ = disable_raw_mode();
    let mut stdout = io::stdout();
    let _ = stdout.execute(LeaveAlternateScreen);
    let _ = stdout.execute(Show);

    outcome
}

async fn fetch_models_for_provider_async(config: &MintConfig, provider: &str) -> Vec<String> {
    match provider {
        "anthropic" | "openai" | "deepseek" | "openrouter" | "local_openai" => {
            mint_core::slash::models::model_options_for_provider_async(config, provider).await
        }
        "img_nanobanana" => {
            mint_core::media::image_models::image_model_options_for_provider_async(
                config,
                "nanobanana",
            )
            .await
        }
        "img_dalle" => {
            mint_core::media::image_models::image_model_options_for_provider_async(config, "dalle")
                .await
        }
        "img_stability" => {
            mint_core::media::image_models::image_model_options_for_provider_async(
                config,
                "stability",
            )
            .await
        }
        "img_ideogram" => {
            mint_core::media::image_models::image_model_options_for_provider_async(
                config, "ideogram",
            )
            .await
        }
        "img_replicate" => {
            mint_core::media::image_models::image_model_options_for_provider_async(
                config,
                "replicate",
            )
            .await
        }
        "img_bfl" => {
            mint_core::media::image_models::image_model_options_for_provider_async(config, "bfl")
                .await
        }
        "vid_veo" => {
            mint_core::media::video_models::video_model_options_for_provider_async(config, "veo")
                .await
        }
        "gemini_live" => mint_core::model_catalog::live_models(config, "").await,
        _ => Vec::new(),
    }
}

pub fn format_masked_key(key: &str) -> String {
    let key = key.trim();
    if key.is_empty() {
        return "none".to_string();
    }
    let len = key.chars().count();
    if len <= 10 {
        "***".to_string()
    } else {
        let first: String = key.chars().take(6).collect();
        let last: String = key.chars().skip(len - 4).collect();
        format!("{}...****...{}", first, last)
    }
}

/// Backward-compatible prompt helper used by `cron_wizard.rs` and `subagent_wizard.rs`.
pub(crate) fn prompt_choice(label: &str, options: &[String], default_idx: usize) -> Result<String> {
    let mut cursor = default_idx.min(options.len().saturating_sub(1));

    println!("{}", label);
    println!("  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Enter: Select]\x1b[0m");
    print_select_options(options, cursor);
    enable_raw_mode()?;

    loop {
        match event::poll(std::time::Duration::from_millis(100)) {
            Ok(true) => {
                if let Event::Key(key_event) = event::read()?
                    && key_event.kind == event::KeyEventKind::Press
                {
                    let is_ctrl_c = matches!(key_event.code, KeyCode::Char('c'))
                        && key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL);
                    if is_ctrl_c {
                        disable_raw_mode()?;
                        println!("\n\x1b[31mCancelled.\x1b[0m");
                        bail!("cancelled");
                    }

                    match key_event.code {
                        KeyCode::Up => {
                            cursor = if cursor > 0 {
                                cursor - 1
                            } else {
                                options.len() - 1
                            };
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", options.len());
                            print_select_options(options, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Down => {
                            cursor = if cursor < options.len() - 1 {
                                cursor + 1
                            } else {
                                0
                            };
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", options.len());
                            print_select_options(options, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Enter => {
                            disable_raw_mode()?;
                            println!();
                            return Ok(options[cursor].clone());
                        }
                        _ => {}
                    }
                }
            }
            Ok(false) => {}
            Err(error) => {
                disable_raw_mode()?;
                return Err(error.into());
            }
        }
    }
}

/// Backward-compatible prompt input used by `cron_wizard.rs` and `subagent_wizard.rs`.
pub(crate) fn prompt_input(label: &str, default: Option<&str>) -> Result<String> {
    print!("{}", label);
    if let Some(d) = default
        && !d.is_empty()
    {
        print!(" [\x1b[90m{}\x1b[0m]", d);
    }
    print!(": ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty()
        && let Some(d) = default
    {
        return Ok(d.to_string());
    }
    Ok(trimmed.to_string())
}

pub(crate) fn print_select_options(options: &[String], cursor: usize) {
    for (i, option) in options.iter().enumerate() {
        if i == cursor {
            println!(
                "  \x1b[38;2;96;165;250m❯\x1b[0m \x1b[38;2;96;165;250m{}\x1b[0m",
                option
            );
        } else {
            println!("    {}", option);
        }
    }
    let _ = io::stdout().flush();
}

#[allow(dead_code)]
pub(crate) fn installed_ollama_models() -> Vec<String> {
    if let Ok(output) = Command::new("ollama").arg("list").output() {
        if output.status.success() {
            let models: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .skip(1)
                .filter_map(|line| line.split_whitespace().next())
                .filter(|name| !name.trim().is_empty())
                .map(str::to_string)
                .collect();
            if !models.is_empty() {
                return models;
            }
        }
    }

    if let Some(body) = ollama_http_get("http://localhost:11434", "/api/tags") {
        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&body) {
            if let Some(models) = json.get("models").and_then(|m| m.as_array()) {
                return models
                    .iter()
                    .filter_map(|m| m.get("name").and_then(|n| n.as_str()))
                    .map(str::to_string)
                    .collect();
            }
        }
    }
    Vec::new()
}

#[allow(dead_code)]
fn ollama_http_get(base: &str, path: &str) -> Option<String> {
    let base = base.trim().trim_end_matches('/');
    let hostport = base
        .strip_prefix("http://")
        .or_else(|| base.strip_prefix("https://"))
        .unwrap_or(base);
    let hostport = if hostport.contains(':') {
        hostport.to_string()
    } else {
        format!("{hostport}:11434")
    };

    for addr in hostport.to_socket_addrs().ok()?.collect::<Vec<_>>() {
        let Ok(mut stream) = TcpStream::connect_timeout(&addr, Duration::from_secs(2)) else {
            continue;
        };
        let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
        let request =
            format!("GET {path} HTTP/1.1\r\nHost: {hostport}\r\nConnection: close\r\n\r\n");
        if stream.write_all(request.as_bytes()).is_err() {
            continue;
        }
        let mut raw = String::new();
        let _ = stream.read_to_string(&mut raw);
        let (head, body) = raw.split_once("\r\n\r\n").unwrap_or((raw.as_str(), ""));
        if head.lines().next().is_some_and(|l| l.contains("200")) {
            return Some(body.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wizard_initialization_defaults() {
        let config = MintConfig::default();
        let wizard = OnboardWizard::new(config);
        assert_eq!(wizard.step, OnboardStep::CoreAi);
        assert_eq!(wizard.step.index(), 0);
        assert!(!wizard.categories.is_empty());
        assert!(!wizard.services.is_empty());
        assert_eq!(wizard.gemini_model_idx, 0);
        assert_eq!(wizard.gemini_focus, 0);
    }

    #[test]
    fn test_wizard_step_transitions() {
        let config = MintConfig::default();
        let mut wizard = OnboardWizard::new(config);
        assert_eq!(wizard.step, OnboardStep::CoreAi);
        assert_eq!(wizard.step.index(), 0);

        wizard.step = OnboardStep::Services;
        assert_eq!(wizard.step, OnboardStep::Services);
        assert_eq!(wizard.step.index(), 1);

        wizard.build_config_fields();
        wizard.step = OnboardStep::Config;
        assert_eq!(wizard.step, OnboardStep::Config);
        assert_eq!(wizard.step.index(), 2);

        wizard.apply_all_to_config();
        wizard.step = OnboardStep::Summary;
        assert_eq!(wizard.step, OnboardStep::Summary);
        assert_eq!(wizard.step.index(), 3);
    }

    #[test]
    fn test_format_masked_key() {
        assert_eq!(format_masked_key(""), "none");
        assert_eq!(format_masked_key("12345"), "***");
        let masked = format_masked_key("AIzaSyD-1234567890abcdefg");
        assert!(masked.contains("...****..."));
    }

    #[test]
    fn test_service_enabling_and_config_fields() {
        let mut config = MintConfig::default();
        config.anthropic_api_key = "test-anthropic-key".to_string();
        let mut wizard = OnboardWizard::new(config);
        assert!(wizard.is_service_enabled("anthropic"));
        wizard.build_config_fields();
        assert!(!wizard.service_groups.is_empty());
        let anthropic_group = wizard
            .service_groups
            .iter()
            .find(|g| g.service_key == "anthropic")
            .expect("anthropic service group should exist");
        assert_eq!(anthropic_group.field_indices.len(), 2);

        assert!(wizard.fields.iter().any(|f| f.id == "anthropic_key"));
        assert!(wizard.fields.iter().any(|f| f.id == "anthropic_model"));
        let model_field = wizard
            .fields
            .iter()
            .find(|f| f.id == "anthropic_model")
            .unwrap();
        if let FieldType::Choice { options, .. } = &model_field.field_type {
            assert!(options.contains(&"Custom model...".to_string()));
        } else {
            panic!("anthropic_model must be FieldType::Choice");
        }
    }
}

use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use mint_core::{load_config, save_config};
use std::io::{self, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::process::Command;

struct OnboardService {
    category: &'static str,
    name: &'static str,
    key: &'static str,
    enabled: bool,
}

pub(crate) const GEMINI_MODEL_PRESETS: &[&str] = &[
    "gemini-3.6-flash",
    "gemini-3.5-flash",
    "gemini-3.5-flash-lite",
    "gemini-3.1-flash-lite",
    "gemini-2.5-flash",
];

pub(crate) const ANTHROPIC_MODEL_PRESETS: &[&str] = &[
    "claude-sonnet-5",
    "claude-opus-5",
    "claude-sonnet-4.6",
    "claude-opus-4.8",
    "claude-haiku-4.5",
];

pub(crate) const OPENAI_MODEL_PRESETS: &[&str] = &[
    "gpt-5.6-sol",
    "gpt-5.6-terra",
    "gpt-5.6-luna",
    "gpt-5.5-thinking",
    "gpt-5.5-pro",
];

pub(crate) const OPENROUTER_MODEL_PRESETS: &[&str] = &[
    "openai/gpt-5.6-terra",
    "anthropic/claude-sonnet-5",
    "google/gemini-3.6-flash",
    "x-ai/grok-4.5",
    "deepseek/deepseek-v4-pro",
];

pub(crate) const DEEPSEEK_MODEL_PRESETS: &[&str] = &[
    "deepseek-v4-flash",
    "deepseek-v4-pro",
    "deepseek-chat",
    "deepseek-reasoner",
];

pub(crate) const HUGGINGFACE_MODEL_PRESETS: &[&str] = &[
    "Qwen/Qwen3.6-27B",
    "deepseek-ai/DeepSeek-V4-Flash",
    "google/gemma-3-27b-it",
    "meta-llama/Llama-3.3-70B-Instruct",
    "microsoft/phi-4",
    "zai-org/GLM-5.2-FP8",
    "mistralai/Mistral-Large-Instruct",
    "openai/gpt-oss-120b",
];

// ── Image Generation Providers ──────────────────────────────────────────────
const NANOBANANA_IMAGE_MODEL_PRESETS: &[&str] = &[
    "gemini-3.1-flash-image",
    "gemini-3-pro-image",
    "gemini-2.5-flash-image",
];

const DALLE_MODEL_PRESETS: &[&str] = &["gpt-image-1", "dall-e-3"];

const STABILITY_MODEL_PRESETS: &[&str] = &[
    "ultra",
    "core",
    "sd3.5-large",
    "sd3.5-large-turbo",
    "sd3-medium",
];

const IDEOGRAM_MODEL_PRESETS: &[&str] = &["V_3", "V_2", "V_2_TURBO"];

const REPLICATE_MODEL_PRESETS: &[&str] = &[
    "black-forest-labs/flux-1.1-pro",
    "black-forest-labs/flux-kontext-pro",
    "black-forest-labs/flux-fill-pro",
    "black-forest-labs/flux-schnell",
    "stability-ai/sdxl",
    "timbrooks/instruct-pix2pix",
];

const BFL_MODEL_PRESETS: &[&str] = &[
    "flux-pro-1.1",
    "flux-pro-1.1-ultra",
    "flux-pro",
    "flux-dev",
    "flux-schnell",
    "flux-kontext-pro",
    "flux-kontext-max",
    "flux-fill-pro",
];

// ── Video Generation Providers ──────────────────────────────────────────────
const VEO_VIDEO_MODEL_PRESETS: &[&str] = &[
    "veo-3.1-generate-preview",
    "veo-3.1-fast-generate-preview",
    "veo-3.1-lite-generate-preview",
];

// ── Realtime Live Model (Gemini Live voice) ─────────────────────────────────
// Note: gemini-3.1-flash-live-preview requires requesting allowlist access from Google
// before it works; gemini-2.5-flash-native-audio-preview-12-2025 needs no such request as
// of writing, so it's listed first and used as the default. gemini-3.1-flash-tts-preview is
// deliberately NOT offered here — it's a generateContent-only TTS model, not a Live/
// BidiGenerateContent model, and will never work with this feature.
const GEMINI_LIVE_MODEL_PRESETS: &[&str] = &[
    "gemini-2.5-flash-native-audio-preview-12-2025",
    "gemini-3.1-flash-live-preview",
];

pub async fn run() -> Result<()> {
    let mut config = load_config()?;

    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[32m       Mint CLI Onboarding Wizard\x1b[0m");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("Welcome to Mint! Let's get your workspace configured.");
    println!();

    // ────────────────────────────────────────────────────────────────
    // Step 1: Core AI Activation (Gemini)
    // ────────────────────────────────────────────────────────────────
    println!("\x1b[33mStep 1: Core AI Activation (Gemini)\x1b[0m");
    println!("Mint is powered primarily by Google Gemini.");
    config.api_key = prompt_sensitive("Gemini API Key", &config.api_key)?;
    config.gemini_model = prompt_select_or_custom(
        "Gemini Model",
        static_model_options(GEMINI_MODEL_PRESETS),
        Some(&config.gemini_model),
        "Custom Gemini model...",
    )?;
    println!();

    // ────────────────────────────────────────────────────────────────
    // Step 2: QuickStart Provider Selection
    // ────────────────────────────────────────────────────────────────
    let mut services = vec![
        OnboardService {
            category: "AI Providers",
            name: "Anthropic (Claude) API",
            key: "anthropic",
            enabled: !config.anthropic_api_key.is_empty(),
        },
        OnboardService {
            category: "AI Providers",
            name: "OpenAI API",
            key: "openai",
            enabled: !config.openai_api_key.is_empty(),
        },
        OnboardService {
            category: "AI Providers",
            name: "OpenRouter API",
            key: "openrouter",
            enabled: !config.openrouter_api_key.is_empty(),
        },
        OnboardService {
            category: "AI Providers",
            name: "DeepSeek API",
            key: "deepseek",
            enabled: !config.deepseek_api_key.is_empty(),
        },
        OnboardService {
            category: "AI Providers",
            name: "Hugging Face API",
            key: "huggingface",
            enabled: !config.hf_api_key.is_empty(),
        },
        OnboardService {
            category: "AI Providers",
            name: "Local OpenAI (e.g. LM Studio)",
            key: "local_openai",
            enabled: !config.local_api_base_url.is_empty(),
        },
        OnboardService {
            category: "AI Providers",
            name: "Ollama",
            key: "ollama",
            enabled: !config.ollama_model.is_empty(),
        },
        OnboardService {
            category: "Search",
            name: "Google Search API",
            key: "google_search",
            enabled: !config
                .extra
                .get("googleSearchApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
        },
        OnboardService {
            category: "Search",
            name: "Brave Search API",
            key: "brave_search",
            enabled: !config
                .extra
                .get("braveSearchApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
        },
        OnboardService {
            category: "Search",
            name: "SearXNG (self-hosted)",
            key: "searxng",
            enabled: !config
                .extra
                .get("searxngBaseUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
        },
        OnboardService {
            category: "Messaging Bridges",
            name: "Telegram Bot Bridge",
            key: "telegram",
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
            enabled: config
                .extra
                .get("enableEmailBridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        // ── Image Generation ─────────────────────────────────────────────────
        OnboardService {
            category: "Image Generation",
            name: "Google NanoBanana (Gemini Images)  [uses Gemini key]",
            key: "img_nanobanana",
            enabled: !config.api_key.is_empty() && !config.nanobanana_model.is_empty(),
        },
        OnboardService {
            category: "Image Generation",
            name: "OpenAI DALL·E  [uses OpenAI key]",
            key: "img_dalle",
            enabled: !config.openai_api_key.is_empty() && !config.dalle_model.is_empty(),
        },
        OnboardService {
            category: "Image Generation",
            name: "Stability AI (Stable Diffusion)",
            key: "img_stability",
            enabled: !config.stability_api_key.is_empty(),
        },
        OnboardService {
            category: "Image Generation",
            name: "Ideogram v3",
            key: "img_ideogram",
            enabled: !config.ideogram_api_key.is_empty(),
        },
        OnboardService {
            category: "Image Generation",
            name: "Replicate (FLUX / SDXL / custom)",
            key: "img_replicate",
            enabled: !config.replicate_api_key.is_empty(),
        },
        OnboardService {
            category: "Image Generation",
            name: "Black Forest Labs (FLUX API)",
            key: "img_bfl",
            enabled: !config.bfl_api_key.is_empty(),
        },
        // ── Video Generation ─────────────────────────────────────────────────
        OnboardService {
            category: "Video Generation",
            name: "Google Veo (Gemini Videos)  [uses Gemini key]",
            key: "vid_veo",
            enabled: !config.api_key.is_empty()
                && config
                    .extra
                    .get("veoModel")
                    .and_then(|v| v.as_str())
                    .map(|s| !s.is_empty())
                    .unwrap_or(false),
        },
        // ── Voice ─────────────────────────────────────────────────────────────
        OnboardService {
            category: "Voice",
            name: "Realtime Live Model (Gemini Live)  [uses Gemini key]",
            key: "gemini_live",
            enabled: config.extra.get("voiceMode").and_then(|v| v.as_str()) == Some("geminiLive"),
        },
    ];

    let mut cursor = 0;
    println!("\x1b[33mStep 2: QuickStart Provider Selection\x1b[0m");
    println!("Select which plugins or bridges you would like to configure:");
    println!(
        "  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Space: Toggle | a: All | i: Invert | Enter: Confirm]\x1b[0m"
    );
    println!();

    print_services(&services, cursor);
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
                        println!("\n\x1b[31mOnboarding cancelled.\x1b[0m");
                        return Ok(());
                    }

                    match key_event.code {
                        KeyCode::Up => {
                            if cursor > 0 {
                                cursor -= 1;
                            } else {
                                cursor = services.len() - 1;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", service_display_lines(&services));
                            print_services(&services, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Down => {
                            if cursor < services.len() - 1 {
                                cursor += 1;
                            } else {
                                cursor = 0;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", service_display_lines(&services));
                            print_services(&services, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Char(' ') => {
                            services[cursor].enabled = !services[cursor].enabled;
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", service_display_lines(&services));
                            print_services(&services, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Char('a') => {
                            for svc in &mut services {
                                svc.enabled = true;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", service_display_lines(&services));
                            print_services(&services, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Char('i') => {
                            for svc in &mut services {
                                svc.enabled = !svc.enabled;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", service_display_lines(&services));
                            print_services(&services, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Enter => {
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Ok(false) => {}
            Err(_) => {
                break;
            }
        }
    }
    disable_raw_mode()?;
    println!();

    // ────────────────────────────────────────────────────────────────
    // Step 3: Categorized Services Detail Entry
    // ────────────────────────────────────────────────────────────────
    println!("\x1b[33mStep 3: Service Configurations\x1b[0m");

    // Anthropic
    if is_selected("anthropic", &services) {
        println!("\n\x1b[36m--- Anthropic (Claude) API ---\x1b[0m");
        config.anthropic_api_key =
            prompt_sensitive("Anthropic API Key", &config.anthropic_api_key)?;
        config.anthropic_model = prompt_select_or_custom(
            "Anthropic Model",
            static_model_options(ANTHROPIC_MODEL_PRESETS),
            Some(&config.anthropic_model),
            "Custom Anthropic model...",
        )?;
    } else {
        config.anthropic_api_key = String::new();
    }

    // OpenAI
    if is_selected("openai", &services) {
        println!("\n\x1b[36m--- OpenAI API ---\x1b[0m");
        config.openai_api_key = prompt_sensitive("OpenAI API Key", &config.openai_api_key)?;
        config.openai_model = prompt_select_or_custom(
            "OpenAI Model",
            static_model_options(OPENAI_MODEL_PRESETS),
            Some(&config.openai_model),
            "Custom OpenAI model...",
        )?;
    } else {
        config.openai_api_key = String::new();
    }

    // OpenRouter
    if is_selected("openrouter", &services) {
        println!("\n\x1b[36m--- OpenRouter API ---\x1b[0m");
        println!(
            "\x1b[90mOpenRouter model uses a provider/model slug, for example: openai/gpt-4o-mini, anthropic/claude-3.5-sonnet, google/gemini-2.5-flash, meta-llama/llama-3.3-70b-instruct, mistralai/mistral-large\x1b[0m"
        );
        config.openrouter_api_key =
            prompt_sensitive("OpenRouter API Key", &config.openrouter_api_key)?;
        config.openrouter_model = prompt_select_or_custom(
            "OpenRouter Model Slug",
            static_model_options(OPENROUTER_MODEL_PRESETS),
            Some(&config.openrouter_model),
            "Custom model slug...",
        )?;
    } else {
        config.openrouter_api_key = String::new();
    }

    // DeepSeek
    if is_selected("deepseek", &services) {
        println!("\n\x1b[36m--- DeepSeek API ---\x1b[0m");
        println!(
            "\x1b[90mDeepSeek uses OpenAI-compatible model names. Prefer deepseek-v4-flash or deepseek-v4-pro; deepseek-chat and deepseek-reasoner are compatibility aliases scheduled for deprecation on 2026-07-24.\x1b[0m"
        );
        config.deepseek_api_key = prompt_sensitive("DeepSeek API Key", &config.deepseek_api_key)?;
        config.deepseek_model = prompt_select_or_custom(
            "DeepSeek Model",
            static_model_options(DEEPSEEK_MODEL_PRESETS),
            Some(&config.deepseek_model),
            "Custom DeepSeek model...",
        )?;
    } else {
        config.deepseek_api_key = String::new();
    }

    // Hugging Face
    if is_selected("huggingface", &services) {
        println!("\n\x1b[36m--- Hugging Face API ---\x1b[0m");
        config.hf_api_key = prompt_sensitive("Hugging Face API Key", &config.hf_api_key)?;
        config.hf_model = prompt_select_or_custom(
            "Hugging Face Model",
            static_model_options(HUGGINGFACE_MODEL_PRESETS),
            Some(&config.hf_model),
            "Custom Hugging Face model...",
        )?;
    } else {
        config.hf_api_key = String::new();
    }

    // Local OpenAI
    if is_selected("local_openai", &services) {
        println!("\n\x1b[36m--- Local OpenAI (e.g. LM Studio) ---\x1b[0m");
        config.local_api_base_url =
            prompt_input("Local OpenAI Base URL", Some(&config.local_api_base_url))?;
        config.local_model_name = prompt_input("Local Model Name", Some(&config.local_model_name))?;
    } else {
        config.local_api_base_url = String::new();
    }

    // Ollama
    if is_selected("ollama", &services) {
        println!("\n\x1b[36m--- Ollama ---\x1b[0m");
        config.ollama_host = prompt_input("Ollama Host", Some(&config.ollama_host))?;
        let ollama_models = installed_ollama_models();
        if ollama_models.is_empty() {
            println!(
                "\x1b[90mNo local Ollama models found. Run `ollama pull <model>` to install one, or type a model name manually.\x1b[0m"
            );
            config.ollama_model = prompt_input("Ollama Model Name", Some(&config.ollama_model))?;
        } else {
            println!(
                "\x1b[90mFound {} local Ollama model(s).\x1b[0m",
                ollama_models.len()
            );
            config.ollama_model = prompt_select_or_custom(
                "Ollama Local Model Name",
                ollama_models,
                Some(&config.ollama_model),
                "Custom local model name...",
            )?;
        }
        ensure_ollama_serving(&config.ollama_host);
    } else {
        config.ollama_host = String::new();
        config.ollama_model = String::new();
    }

    // Google Search
    if is_selected("google_search", &services) {
        println!("\n\x1b[36m--- Google Search API ---\x1b[0m");
        let current_key = config
            .extra
            .get("googleSearchApiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_cx = config
            .extra
            .get("googleSearchCx")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let key = prompt_sensitive("Google Search API Key", current_key)?;
        let cx = prompt_input("Google Search Engine ID (Cx)", Some(current_cx))?;
        config.extra.insert(
            "googleSearchApiKey".to_string(),
            serde_json::Value::String(key),
        );
        config
            .extra
            .insert("googleSearchCx".to_string(), serde_json::Value::String(cx));
    } else {
        config.extra.insert(
            "googleSearchApiKey".to_string(),
            serde_json::Value::String(String::new()),
        );
        config.extra.insert(
            "googleSearchCx".to_string(),
            serde_json::Value::String(String::new()),
        );
    }

    // Brave Search
    if is_selected("brave_search", &services) {
        println!("\n\x1b[36m--- Brave Search API ---\x1b[0m");
        let current_key = config
            .extra
            .get("braveSearchApiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let key = prompt_sensitive("Brave Search API Key", current_key)?;
        config.extra.insert(
            "braveSearchApiKey".to_string(),
            serde_json::Value::String(key),
        );
    } else {
        config.extra.insert(
            "braveSearchApiKey".to_string(),
            serde_json::Value::String(String::new()),
        );
    }

    // SearXNG
    if is_selected("searxng", &services) {
        println!("\n\x1b[36m--- SearXNG (self-hosted) ---\x1b[0m");
        let current_url = config
            .extra
            .get("searxngBaseUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let url = prompt_input(
            "SearXNG Instance URL (e.g. https://searx.example.com)",
            Some(current_url),
        )?;
        config.extra.insert(
            "searxngBaseUrl".to_string(),
            serde_json::Value::String(url.trim_end_matches('/').to_string()),
        );
    } else {
        config.extra.insert(
            "searxngBaseUrl".to_string(),
            serde_json::Value::String(String::new()),
        );
    }

    // Preferred search provider — only relevant if more than one is configured.
    {
        let configured: Vec<(&str, &str)> = [
            ("google", "Google Search"),
            ("brave", "Brave Search"),
            ("searxng", "SearXNG (self-hosted)"),
        ]
        .into_iter()
        .filter(|(key, _)| match *key {
            "google" => !config
                .extra
                .get("googleSearchApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
            "brave" => !config
                .extra
                .get("braveSearchApiKey")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
            _ => !config
                .extra
                .get("searxngBaseUrl")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .is_empty(),
        })
        .collect();

        let provider = if configured.len() > 1 {
            println!("\n\x1b[36m--- Preferred Search Provider ---\x1b[0m");
            let current = config
                .extra
                .get("searchProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let labels: Vec<String> = configured.iter().map(|(_, l)| l.to_string()).collect();
            let default_idx = configured
                .iter()
                .position(|(key, _)| *key == current)
                .unwrap_or(0);
            let picked_label = prompt_choice(
                "Which provider should Mint try first when multiple are configured?",
                &labels,
                default_idx,
            )?;
            configured
                .iter()
                .find(|(_, label)| *label == picked_label)
                .map(|(key, _)| key.to_string())
                .unwrap_or_default()
        } else {
            configured
                .first()
                .map(|(key, _)| key.to_string())
                .unwrap_or_default()
        };

        config.extra.insert(
            "searchProvider".to_string(),
            serde_json::Value::String(provider),
        );
    }

    // Telegram Bot
    if is_selected("telegram", &services) {
        println!("\n\x1b[36m--- Telegram Bot Bridge ---\x1b[0m");
        let current_token = config
            .extra
            .get("telegramBotToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let token = prompt_sensitive("Telegram Bot Token", current_token)?;
        config.extra.insert(
            "telegramBotToken".to_string(),
            serde_json::Value::String(token),
        );
        config.extra.insert(
            "enableTelegramBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableTelegramBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // Discord Bot
    if is_selected("discord", &services) {
        println!("\n\x1b[36m--- Discord Bot Bridge ---\x1b[0m");
        let current_token = config
            .extra
            .get("discordBotToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_id = config
            .extra
            .get("discordApplicationId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let token = prompt_sensitive("Discord Bot Token", current_token)?;
        let app_id = prompt_input("Discord Application ID", Some(current_id))?;
        config.extra.insert(
            "discordBotToken".to_string(),
            serde_json::Value::String(token),
        );
        config.extra.insert(
            "discordApplicationId".to_string(),
            serde_json::Value::String(app_id),
        );
        config.extra.insert(
            "enableDiscordBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableDiscordBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // Slack Bot
    if is_selected("slack", &services) {
        println!("\n\x1b[36m--- Slack Bot Bridge ---\x1b[0m");
        let current_token = config
            .extra
            .get("slackBotToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_app_token = config
            .extra
            .get("slackAppToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let token = prompt_sensitive("Slack Bot Token", current_token)?;
        let app_token = prompt_sensitive("Slack App Token (xapp-...)", current_app_token)?;
        config.extra.insert(
            "slackBotToken".to_string(),
            serde_json::Value::String(token),
        );
        config.extra.insert(
            "slackAppToken".to_string(),
            serde_json::Value::String(app_token),
        );
        config.extra.insert(
            "enableSlackBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableSlackBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // LINE Bot
    if is_selected("line", &services) {
        println!("\n\x1b[36m--- LINE Bot Bridge ---\x1b[0m");
        let current_token = config
            .extra
            .get("lineChannelAccessToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_secret = config
            .extra
            .get("lineChannelSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let token = prompt_sensitive("LINE Channel Access Token", current_token)?;
        let secret = prompt_sensitive("LINE Channel Secret", current_secret)?;
        config.extra.insert(
            "lineChannelAccessToken".to_string(),
            serde_json::Value::String(token),
        );
        config.extra.insert(
            "lineChannelSecret".to_string(),
            serde_json::Value::String(secret),
        );
        config.extra.insert(
            "enableLineBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableLineBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // WhatsApp Cloud
    if is_selected("whatsapp", &services) {
        println!("\n\x1b[36m--- WhatsApp Cloud Bridge ---\x1b[0m");
        let current_token = config
            .extra
            .get("whatsappCloudAccessToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_phone = config
            .extra
            .get("whatsappPhoneNumberId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_verify = config
            .extra
            .get("whatsappVerifyToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_secret = config
            .extra
            .get("whatsappAppSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let token = prompt_sensitive("WhatsApp Access Token", current_token)?;
        let phone = prompt_input("WhatsApp Phone Number ID", Some(current_phone))?;
        let verify = prompt_input("WhatsApp Verify Token", Some(current_verify))?;
        let secret = prompt_sensitive("WhatsApp App Secret", current_secret)?;
        config.extra.insert(
            "whatsappCloudAccessToken".to_string(),
            serde_json::Value::String(token),
        );
        config.extra.insert(
            "whatsappPhoneNumberId".to_string(),
            serde_json::Value::String(phone),
        );
        config.extra.insert(
            "whatsappVerifyToken".to_string(),
            serde_json::Value::String(verify),
        );
        config.extra.insert(
            "whatsappAppSecret".to_string(),
            serde_json::Value::String(secret),
        );
        config.extra.insert(
            "enableWhatsappBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableWhatsappBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // Signal Bridge (via a self-hosted signal-cli-rest-api instance — Signal
    // has no official bot API, so unlike the other bridges here Mint is the
    // one polling a REST endpoint rather than receiving a webhook/socket push)
    if is_selected("signal", &services) {
        println!("\n\x1b[36m--- Signal Bridge ---\x1b[0m");
        println!(
            "Needs a self-hosted signal-cli-rest-api instance with a number already \
             linked/registered: https://github.com/bbernhard/signal-cli-rest-api"
        );
        let current_url = config
            .extra
            .get("signalApiUrl")
            .and_then(|v| v.as_str())
            .unwrap_or("http://localhost:8080");
        let current_number = config
            .extra
            .get("signalNumber")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let api_url = prompt_input("signal-cli-rest-api URL", Some(current_url))?;
        let number = prompt_input(
            "Signal Number (E.164, e.g. +15551234567)",
            Some(current_number),
        )?;
        config.extra.insert(
            "signalApiUrl".to_string(),
            serde_json::Value::String(api_url),
        );
        config.extra.insert(
            "signalNumber".to_string(),
            serde_json::Value::String(number),
        );
        config.extra.insert(
            "enableSignalBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableSignalBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // Email Bridge (rides the same Gmail OAuth connection as the `gmail`
    // plugin — see `mint gmail auth` — rather than asking for separate
    // IMAP/SMTP credentials)
    if is_selected("email", &services) {
        println!("\n\x1b[36m--- Email Bridge (via Gmail) ---\x1b[0m");
        println!(
            "Reuses the same Gmail OAuth connection as the `gmail` plugin. Needs a \
             Google Cloud OAuth client (Client ID/Secret) — see \
             https://console.cloud.google.com/apis/credentials."
        );
        let current_id = config
            .extra
            .get("gmailClientId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_secret = config
            .extra
            .get("gmailClientSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let client_id = prompt_input("Gmail OAuth Client ID", Some(current_id))?;
        let client_secret = prompt_sensitive("Gmail OAuth Client Secret", current_secret)?;
        config.extra.insert(
            "gmailClientId".to_string(),
            serde_json::Value::String(client_id),
        );
        config.extra.insert(
            "gmailClientSecret".to_string(),
            serde_json::Value::String(client_secret),
        );
        let has_refresh_token = config
            .extra
            .get("gmailRefreshToken")
            .and_then(|v| v.as_str())
            .is_some_and(|value| !value.trim().is_empty());
        if !has_refresh_token {
            println!(
                "\x1b[33mNo Gmail refresh token yet — after saving, run `mint gmail auth` \
                 to finish connecting (opens a browser for Google's consent screen).\x1b[0m"
            );
        }
        config.extra.insert(
            "enableEmailBridge".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "enableEmailBridge".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // Image Generation providers
    // ────────────────────────────────────────────────────────────────────────

    // NanoBanana (Gemini Images)
    if is_selected("img_nanobanana", &services) {
        println!("\n\x1b[36m--- Google NanoBanana (Gemini Images) ---\x1b[0m");
        println!(
            "\x1b[90mUses the same Gemini API key as Step 1. Select the image generation model.\x1b[0m"
        );
        config.nanobanana_model = prompt_select_or_custom(
            "NanoBanana Model",
            static_model_options(NANOBANANA_IMAGE_MODEL_PRESETS),
            Some(&config.nanobanana_model),
            "Custom NanoBanana model...",
        )?;
    }

    // DALL·E
    if is_selected("img_dalle", &services) {
        println!("\n\x1b[36m--- OpenAI DALL·E ---\x1b[0m");
        println!(
            "\x1b[90mUses the same OpenAI API key. DALL·E 3 supports only 1 image per request; DALL·E 2 supports up to 10.\x1b[0m"
        );
        config.dalle_model = prompt_select_or_custom(
            "DALL·E Model",
            static_model_options(DALLE_MODEL_PRESETS),
            Some(&config.dalle_model),
            "Custom DALL·E model...",
        )?;
    }

    // Stability AI
    if is_selected("img_stability", &services) {
        println!("\n\x1b[36m--- Stability AI ---\x1b[0m");
        println!(
            "\x1b[90mGet your API key at https://platform.stability.ai/. Supports SD3.5 Large, SD3 Medium, and Stable Image Core.\x1b[0m"
        );
        config.stability_api_key =
            prompt_sensitive("Stability AI API Key", &config.stability_api_key)?;
        config.stability_model = prompt_select_or_custom(
            "Stability Model",
            static_model_options(STABILITY_MODEL_PRESETS),
            Some(&config.stability_model),
            "Custom Stability model...",
        )?;
    } else {
        config.stability_api_key = String::new();
    }

    // Ideogram
    if is_selected("img_ideogram", &services) {
        println!("\n\x1b[36m--- Ideogram ---\x1b[0m");
        println!(
            "\x1b[90mGet your API key at https://ideogram.ai/api. Supports V_3, V_2, and V_2_TURBO.\x1b[0m"
        );
        config.ideogram_api_key = prompt_sensitive("Ideogram API Key", &config.ideogram_api_key)?;
        config.ideogram_model = prompt_select_or_custom(
            "Ideogram Model",
            static_model_options(IDEOGRAM_MODEL_PRESETS),
            Some(&config.ideogram_model),
            "Custom Ideogram model...",
        )?;
    } else {
        config.ideogram_api_key = String::new();
    }

    // Replicate
    if is_selected("img_replicate", &services) {
        println!("\n\x1b[36m--- Replicate ---\x1b[0m");
        println!(
            "\x1b[90mGet your API token at https://replicate.com/account/api-tokens. Works with FLUX, SDXL, and any public image model.\x1b[0m"
        );
        config.replicate_api_key =
            prompt_sensitive("Replicate API Token", &config.replicate_api_key)?;
        config.replicate_model = prompt_select_or_custom(
            "Replicate Model (owner/model-name)",
            static_model_options(REPLICATE_MODEL_PRESETS),
            Some(&config.replicate_model),
            "Custom Replicate model...",
        )?;
    } else {
        config.replicate_api_key = String::new();
    }

    // Black Forest Labs (FLUX API)
    if is_selected("img_bfl", &services) {
        println!("\n\x1b[36m--- Black Forest Labs (FLUX API) ---\x1b[0m");
        println!(
            "\x1b[90mGet your API key at https://api.bfl.ml. Supports flux-pro-1.1, flux-pro-1.1-ultra, flux-dev, and flux-pro-1.0-fill.\x1b[0m"
        );
        config.bfl_api_key = prompt_sensitive("Black Forest Labs API Key", &config.bfl_api_key)?;
        config.bfl_model = prompt_select_or_custom(
            "FLUX Model",
            static_model_options(BFL_MODEL_PRESETS),
            Some(&config.bfl_model),
            "Custom FLUX model...",
        )?;
    } else {
        config.bfl_api_key = String::new();
    }

    // Google Veo (Gemini Videos)
    if is_selected("vid_veo", &services) {
        println!("\n\x1b[36m--- Google Veo (Gemini Videos) ---\x1b[0m");
        println!(
            "\x1b[90mUses the same Gemini API key as Step 1. Select the video generation model.\x1b[0m"
        );
        let current_veo_model = config
            .extra
            .get("veoModel")
            .and_then(|v| v.as_str())
            .unwrap_or("veo-3.1-generate-preview")
            .to_string();
        let selected_veo_model = prompt_select_or_custom(
            "Veo Model",
            static_model_options(VEO_VIDEO_MODEL_PRESETS),
            Some(&current_veo_model),
            "Custom Veo model...",
        )?;
        config.extra.insert(
            "veoModel".to_string(),
            serde_json::Value::String(selected_veo_model),
        );
        config.extra.insert(
            "videoGenProvider".to_string(),
            serde_json::Value::String("veo".to_string()),
        );
    } else if config
        .extra
        .get("videoGenProvider")
        .and_then(|v| v.as_str())
        == Some("veo")
    {
        config.extra.remove("videoGenProvider");
    }

    if is_selected("gemini_live", &services) {
        println!("\n\x1b[36m--- Realtime Live Model (Gemini Live) ---\x1b[0m");
        println!(
            "\x1b[90mUses the same Gemini API key as Step 1. Select the model used for realtime voice conversations.\x1b[0m"
        );
        let current_live_model = config
            .extra
            .get("geminiLiveModel")
            .and_then(|v| v.as_str())
            .unwrap_or("gemini-2.5-flash-native-audio-preview-12-2025")
            .to_string();
        let selected_live_model = prompt_select_or_custom(
            "Realtime Live Model",
            static_model_options(GEMINI_LIVE_MODEL_PRESETS),
            Some(&current_live_model),
            "Custom realtime live model...",
        )?;
        config.extra.insert(
            "geminiLiveModel".to_string(),
            serde_json::Value::String(selected_live_model),
        );
        config.extra.insert(
            "voiceMode".to_string(),
            serde_json::Value::String("geminiLive".to_string()),
        );
    } else if config.extra.get("voiceMode").and_then(|v| v.as_str()) == Some("geminiLive") {
        config.extra.remove("voiceMode");
    }

    save_config(&config)?;
    println!("\x1b[32m✅ Configuration saved successfully!\x1b[0m");
    Ok(())
}

fn is_selected(key: &str, services: &[OnboardService]) -> bool {
    services.iter().any(|s| s.key == key && s.enabled)
}

fn print_services(services: &[OnboardService], cursor: usize) {
    let mut current_category = "";
    for (i, svc) in services.iter().enumerate() {
        if svc.category != current_category {
            current_category = svc.category;
            println!("  \x1b[1;32m{}:\x1b[0m", svc.category);
        }
        let checkbox = if svc.enabled {
            "\x1b[32m◉\x1b[0m"
        } else {
            "\x1b[90m○\x1b[0m"
        };
        if i == cursor {
            println!(
                "    \x1b[36m❯\x1b[0m {} \x1b[36m{}\x1b[0m",
                checkbox, svc.name
            );
        } else {
            println!("      {} {}", checkbox, svc.name);
        }
    }
    let _ = io::stdout().flush();
}

fn service_display_lines(services: &[OnboardService]) -> usize {
    let categories = services
        .iter()
        .fold(Vec::<&str>::new(), |mut categories, service| {
            if categories.last().copied() != Some(service.category) {
                categories.push(service.category);
            }
            categories
        });
    services.len() + categories.len()
}

fn prompt_select_or_custom(
    label: &str,
    presets: Vec<String>,
    current: Option<&str>,
    custom_label: &str,
) -> Result<String> {
    let current = current.unwrap_or("").trim();
    let mut options: Vec<String> = Vec::new();
    if !current.is_empty() && !presets.iter().any(|preset| preset == current) {
        options.push(current.to_string());
    }
    options.extend(presets);
    options.push(custom_label.to_string());

    let mut cursor = if current.is_empty() {
        0
    } else {
        options
            .iter()
            .position(|option| option == current)
            .unwrap_or(0)
    };

    println!("{}", label);
    println!("  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Enter: Select]\x1b[0m");
    print_select_options(&options, cursor);
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
                        println!("\n\x1b[31mOnboarding cancelled.\x1b[0m");
                        bail!("onboarding cancelled");
                    }

                    match key_event.code {
                        KeyCode::Up => {
                            if cursor > 0 {
                                cursor -= 1;
                            } else {
                                cursor = options.len() - 1;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", options.len());
                            print_select_options(&options, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Down => {
                            if cursor < options.len() - 1 {
                                cursor += 1;
                            } else {
                                cursor = 0;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", options.len());
                            print_select_options(&options, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Enter => {
                            disable_raw_mode()?;
                            println!();
                            let selected = options[cursor].clone();
                            if selected == custom_label {
                                return prompt_input(label, Some(current));
                            }
                            return Ok(selected);
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

/// Simple arrow-key single-choice picker with no "type your own" fallback.
/// Unlike `prompt_select_or_custom`, every option is a literal, final answer.
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
                        println!("\n\x1b[31mOnboarding cancelled.\x1b[0m");
                        bail!("onboarding cancelled");
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

fn static_model_options(presets: &[&str]) -> Vec<String> {
    presets.iter().map(|value| value.to_string()).collect()
}

pub(crate) fn installed_ollama_models() -> Vec<String> {
    let output = match Command::new("ollama").arg("list").output() {
        Ok(output) if output.status.success() => output,
        _ => return Vec::new(),
    };
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .skip(1)
        .filter_map(|line| line.split_whitespace().next())
        .filter(|name| !name.trim().is_empty())
        .map(|name| name.to_string())
        .collect()
}

fn ensure_ollama_serving(host: &str) {
    let host = if host.trim().is_empty() {
        "http://localhost:11434"
    } else {
        host.trim_end_matches('/')
    };

    // Parse host:port for TCP check
    let addr = host
        .strip_prefix("http://")
        .or_else(|| host.strip_prefix("https://"))
        .unwrap_or(host);
    let addr = if !addr.contains(':') {
        format!("{}:11434", addr)
    } else {
        addr.to_string()
    };

    // Resolve once up front — `addr` may be a DNS name (e.g. a Docker
    // service name), which `SocketAddr::parse` can't handle, so we go
    // through `ToSocketAddrs` instead of silently falling back to
    // 127.0.0.1 on parse failure.
    let resolved_addr = addr.to_socket_addrs().ok().and_then(|mut it| it.next());
    let is_ollama_reachable = |timeout_secs: u64| -> bool {
        match resolved_addr {
            Some(socket_addr) => TcpStream::connect_timeout(
                &socket_addr,
                std::time::Duration::from_secs(timeout_secs),
            )
            .is_ok(),
            None => false,
        }
    };

    // Check if Ollama is already serving
    if is_ollama_reachable(1) {
        println!(
            "\x1b[32m✔ Ollama server is already running at {}\x1b[0m",
            host
        );
        return;
    }

    // Not running — try to start it
    print!("\x1b[33m⏳ Starting Ollama server...\x1b[0m");
    let _ = io::stdout().flush();

    match Command::new("ollama")
        .arg("serve")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(_) => {
            // Wait for server to become ready (up to 10 seconds)
            let mut ready = false;
            for _ in 0..20 {
                std::thread::sleep(std::time::Duration::from_millis(500));
                if is_ollama_reachable(1) {
                    ready = true;
                    break;
                }
            }
            if ready {
                println!(
                    "\r\x1b[32m✔ Ollama server started successfully at {}\x1b[0m          ",
                    host
                );
            } else {
                println!(
                    "\r\x1b[31m✘ Ollama server started but not responding yet. It may need more time.\x1b[0m"
                );
            }
        }
        Err(e) => {
            println!(
                "\r\x1b[31m✘ Failed to start Ollama server: {}. Please run `ollama serve` manually.\x1b[0m",
                e
            );
        }
    }
}

pub(crate) fn print_select_options(options: &[String], cursor: usize) {
    for (i, option) in options.iter().enumerate() {
        if i == cursor {
            println!("  \x1b[36m❯\x1b[0m \x1b[36m{}\x1b[0m", option);
        } else {
            println!("    {}", option);
        }
    }
    let _ = io::stdout().flush();
}

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

fn format_masked_key(key: &str) -> String {
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

fn prompt_sensitive(label: &str, existing: &str) -> Result<String> {
    if !existing.is_empty() {
        print!(
            "{} [keep existing ({})]: ",
            label,
            format_masked_key(existing)
        );
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if trimmed.is_empty() {
            return Ok(existing.to_string());
        }
        Ok(trimmed.to_string())
    } else {
        print!("{}: ", label);
        io::stdout().flush()?;
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        Ok(input.trim().to_string())
    }
}

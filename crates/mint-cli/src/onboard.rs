use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use mint_core::{load_config, save_config};
use std::io::{self, Write};

struct OnboardService {
    name: &'static str,
    key: &'static str,
    enabled: bool,
}

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
    config.gemini_model = prompt_input("Gemini Model", Some(&config.gemini_model))?;
    println!();

    // ────────────────────────────────────────────────────────────────
    // Step 2: QuickStart Provider Selection
    // ────────────────────────────────────────────────────────────────
    let mut services = vec![
        OnboardService {
            name: "Anthropic (Claude) API",
            key: "anthropic",
            enabled: !config.anthropic_api_key.is_empty(),
        },
        OnboardService {
            name: "OpenAI API",
            key: "openai",
            enabled: !config.openai_api_key.is_empty(),
        },
        OnboardService {
            name: "Hugging Face API",
            key: "huggingface",
            enabled: !config.hf_api_key.is_empty(),
        },
        OnboardService {
            name: "Local OpenAI (e.g. LM Studio)",
            key: "local_openai",
            enabled: !config.local_api_base_url.is_empty(),
        },
        OnboardService {
            name: "Ollama",
            key: "ollama",
            enabled: !config.ollama_model.is_empty(),
        },
        OnboardService {
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
            name: "Telegram Bot Bridge",
            key: "telegram",
            enabled: config
                .extra
                .get("enableTelegramBridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "Discord Bot Bridge",
            key: "discord",
            enabled: config
                .extra
                .get("enableDiscordBridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "Slack Bot Bridge",
            key: "slack",
            enabled: config
                .extra
                .get("enableSlackBridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "LINE Bot Bridge",
            key: "line",
            enabled: config
                .extra
                .get("enableLineBridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "WhatsApp Cloud Bridge",
            key: "whatsapp",
            enabled: config
                .extra
                .get("enableWhatsappBridge")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "Gmail Plugin",
            key: "gmail",
            enabled: config
                .extra
                .get("pluginGmailEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "Google Calendar Plugin",
            key: "calendar",
            enabled: config
                .extra
                .get("pluginCalendarEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        OnboardService {
            name: "Notion Plugin",
            key: "notion",
            enabled: config
                .extra
                .get("pluginNotionEnabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
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
                if let Event::Key(key_event) = event::read()? {
                    if key_event.kind == event::KeyEventKind::Press {
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
                                print!("\x1b[{}A\x1b[J", services.len());
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
                                print!("\x1b[{}A\x1b[J", services.len());
                                print_services(&services, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char(' ') => {
                                services[cursor].enabled = !services[cursor].enabled;
                                disable_raw_mode()?;
                                print!("\x1b[{}A\x1b[J", services.len());
                                print_services(&services, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char('a') => {
                                for svc in &mut services {
                                    svc.enabled = true;
                                }
                                disable_raw_mode()?;
                                print!("\x1b[{}A\x1b[J", services.len());
                                print_services(&services, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char('i') => {
                                for svc in &mut services {
                                    svc.enabled = !svc.enabled;
                                }
                                disable_raw_mode()?;
                                print!("\x1b[{}A\x1b[J", services.len());
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
        config.anthropic_model = prompt_input("Anthropic Model", Some(&config.anthropic_model))?;
    } else {
        config.anthropic_api_key = String::new();
    }

    // OpenAI
    if is_selected("openai", &services) {
        println!("\n\x1b[36m--- OpenAI API ---\x1b[0m");
        config.openai_api_key = prompt_sensitive("OpenAI API Key", &config.openai_api_key)?;
        config.openai_model = prompt_input("OpenAI Model", Some(&config.openai_model))?;
    } else {
        config.openai_api_key = String::new();
    }

    // Hugging Face
    if is_selected("huggingface", &services) {
        println!("\n\x1b[36m--- Hugging Face API ---\x1b[0m");
        config.hf_api_key = prompt_sensitive("Hugging Face API Key", &config.hf_api_key)?;
        config.hf_model = prompt_input("Hugging Face Model", Some(&config.hf_model))?;
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
        config.ollama_model = prompt_input("Ollama Model", Some(&config.ollama_model))?;
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

    // Gmail
    if is_selected("gmail", &services) {
        println!("\n\x1b[36m--- Gmail Plugin ---\x1b[0m");
        let current_client_id = config
            .extra
            .get("gmailClientId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_client_secret = config
            .extra
            .get("gmailClientSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_refresh_token = config
            .extra
            .get("gmailRefreshToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_user_id = config
            .extra
            .get("gmailUserId")
            .and_then(|v| v.as_str())
            .unwrap_or("me");
        let client_id = prompt_input("Gmail Client ID", Some(current_client_id))?;
        let client_secret = prompt_sensitive("Gmail Client Secret", current_client_secret)?;
        let refresh_token = prompt_sensitive("Gmail Refresh Token", current_refresh_token)?;
        let user_id = prompt_input("Gmail User ID", Some(current_user_id))?;
        config.extra.insert(
            "gmailClientId".to_string(),
            serde_json::Value::String(client_id),
        );
        config.extra.insert(
            "gmailClientSecret".to_string(),
            serde_json::Value::String(client_secret),
        );
        config.extra.insert(
            "gmailRefreshToken".to_string(),
            serde_json::Value::String(refresh_token),
        );
        config.extra.insert(
            "gmailUserId".to_string(),
            serde_json::Value::String(user_id),
        );
        config.extra.insert(
            "pluginGmailEnabled".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "pluginGmailEnabled".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // Google Calendar
    if is_selected("calendar", &services) {
        println!("\n\x1b[36m--- Google Calendar Plugin ---\x1b[0m");
        let current_client_id = config
            .extra
            .get("googleCalendarClientId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_client_secret = config
            .extra
            .get("googleCalendarClientSecret")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_refresh_token = config
            .extra
            .get("googleCalendarRefreshToken")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_cal_id = config
            .extra
            .get("googleCalendarId")
            .and_then(|v| v.as_str())
            .unwrap_or("primary");
        let client_id = prompt_input("Google Calendar Client ID", Some(current_client_id))?;
        let client_secret =
            prompt_sensitive("Google Calendar Client Secret", current_client_secret)?;
        let refresh_token =
            prompt_sensitive("Google Calendar Refresh Token", current_refresh_token)?;
        let cal_id = prompt_input("Google Calendar ID", Some(current_cal_id))?;
        config.extra.insert(
            "googleCalendarClientId".to_string(),
            serde_json::Value::String(client_id),
        );
        config.extra.insert(
            "googleCalendarClientSecret".to_string(),
            serde_json::Value::String(client_secret),
        );
        config.extra.insert(
            "googleCalendarRefreshToken".to_string(),
            serde_json::Value::String(refresh_token),
        );
        config.extra.insert(
            "googleCalendarId".to_string(),
            serde_json::Value::String(cal_id),
        );
        config.extra.insert(
            "pluginCalendarEnabled".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "pluginCalendarEnabled".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    // Notion
    if is_selected("notion", &services) {
        println!("\n\x1b[36m--- Notion Plugin ---\x1b[0m");
        let current_api_key = config
            .extra
            .get("notionApiKey")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_db_id = config
            .extra
            .get("notionDatabaseId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_page_id = config
            .extra
            .get("notionPageId")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let current_title = config
            .extra
            .get("notionTitleProperty")
            .and_then(|v| v.as_str())
            .unwrap_or("Name");
        let api_key = prompt_sensitive("Notion API Key", current_api_key)?;
        let db_id = prompt_input("Notion Database ID", Some(current_db_id))?;
        let page_id = prompt_input("Notion Page ID", Some(current_page_id))?;
        let title_prop = prompt_input("Notion Title Property", Some(current_title))?;
        config.extra.insert(
            "notionApiKey".to_string(),
            serde_json::Value::String(api_key),
        );
        config.extra.insert(
            "notionDatabaseId".to_string(),
            serde_json::Value::String(db_id),
        );
        config.extra.insert(
            "notionPageId".to_string(),
            serde_json::Value::String(page_id),
        );
        config.extra.insert(
            "notionTitleProperty".to_string(),
            serde_json::Value::String(title_prop),
        );
        config.extra.insert(
            "pluginNotionEnabled".to_string(),
            serde_json::Value::Bool(true),
        );
    } else {
        config.extra.insert(
            "pluginNotionEnabled".to_string(),
            serde_json::Value::Bool(false),
        );
    }

    println!();
    save_config(&config)?;
    println!("\x1b[32m✅ Configuration saved successfully!\x1b[0m");
    Ok(())
}

fn is_selected(key: &str, services: &[OnboardService]) -> bool {
    services.iter().any(|s| s.key == key && s.enabled)
}

fn print_services(services: &[OnboardService], cursor: usize) {
    for (i, svc) in services.iter().enumerate() {
        let checkbox = if svc.enabled {
            "\x1b[32m◉\x1b[0m"
        } else {
            "\x1b[90m○\x1b[0m"
        };
        if i == cursor {
            println!(
                "  \x1b[36m❯\x1b[0m {} \x1b[36m{}\x1b[0m",
                checkbox, svc.name
            );
        } else {
            println!("    {} {}", checkbox, svc.name);
        }
    }
    let _ = io::stdout().flush();
}

fn prompt_input(label: &str, default: Option<&str>) -> Result<String> {
    print!("{}", label);
    if let Some(d) = default {
        if !d.is_empty() {
            print!(" [\x1b[90m{}\x1b[0m]", d);
        }
    }
    print!(": ");
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    let trimmed = input.trim();
    if trimmed.is_empty() {
        if let Some(d) = default {
            return Ok(d.to_string());
        }
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

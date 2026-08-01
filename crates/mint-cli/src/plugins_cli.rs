//! CLI Plugin Manager (`mint plugins`) for Mint CLI.
//! Provides commands for listing, enabling/disabling, configuring API credentials,
//! and managing OAuth sign-in for all Built-in Plugins.

use anyhow::{anyhow, Result};
use clap::Subcommand;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use mint_core::{
    load_config, set_config_value,
    oauth::{build_auth_url, get_oauth_tokens, list_oauth_statuses, revoke_oauth_tokens},
    MintConfig,
};
use std::io::{self, Write};

#[derive(Debug, Subcommand)]
pub enum PluginsSubcommand {
    /// List all built-in plugins and their current status.
    List,
    /// Enable a built-in plugin by name.
    Enable { name: String },
    /// Disable a built-in plugin by name.
    Disable { name: String },
    /// Set API credential values for a plugin (e.g. key=value).
    Config {
        /// Plugin name (gmail, calendar, notion, spotify, etc.)
        name: String,
        /// Key-value pair (e.g. gmailClientId=xyz or notionApiKey=secret_123)
        #[arg(long)]
        set: Option<String>,
    },
    /// Initiate OAuth Sign-In flow in browser for a plugin.
    Login { provider: String },
    /// Disconnect OAuth account for a plugin.
    Logout { provider: String },
}

pub struct BuiltinPluginInfo {
    pub key: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub config_field: &'static str,
    pub is_oauth: bool,
    pub oauth_provider: &'static str,
}

pub const BUILTIN_PLUGINS: &[BuiltinPluginInfo] = &[
    BuiltinPluginInfo {
        key: "spotify",
        name: "Spotify",
        description: "Control music playback with AI via playerctl / Spotify OAuth",
        config_field: "pluginSpotifyEnabled",
        is_oauth: true,
        oauth_provider: "spotify",
    },
    BuiltinPluginInfo {
        key: "discord",
        name: "Discord RPC",
        description: "Show 'Using Mint Assistant' status on local Discord client",
        config_field: "pluginDiscordEnabled",
        is_oauth: false,
        oauth_provider: "",
    },
    BuiltinPluginInfo {
        key: "gmail",
        name: "Gmail",
        description: "Read, summarize, draft, and send emails via Google OAuth",
        config_field: "pluginGmailEnabled",
        is_oauth: true,
        oauth_provider: "google",
    },
    BuiltinPluginInfo {
        key: "calendar",
        name: "Google Calendar",
        description: "Read schedule, check availability, and create calendar events",
        config_field: "pluginCalendarEnabled",
        is_oauth: true,
        oauth_provider: "google",
    },
    BuiltinPluginInfo {
        key: "notion",
        name: "Notion",
        description: "Search pages, query databases, and create notes in Notion",
        config_field: "pluginNotionEnabled",
        is_oauth: true,
        oauth_provider: "notion",
    },
    BuiltinPluginInfo {
        key: "youtube_music",
        name: "YouTube Music",
        description: "Access personal playlists and listening history via Google OAuth",
        config_field: "pluginYoutubeMusicEnabled",
        is_oauth: true,
        oauth_provider: "google",
    },
    BuiltinPluginInfo {
        key: "vercel",
        name: "Vercel",
        description: "Manage web app deployments and projects via Vercel OAuth",
        config_field: "pluginVercelEnabled",
        is_oauth: true,
        oauth_provider: "vercel",
    },
    BuiltinPluginInfo {
        key: "github",
        name: "GitHub",
        description: "Access repositories, pull requests, and issues via GitHub OAuth",
        config_field: "pluginGithubEnabled",
        is_oauth: true,
        oauth_provider: "github",
    },
];

pub async fn run_plugins_command(subcommand: Option<PluginsSubcommand>) -> Result<()> {
    let mut config = load_config()?;

    match subcommand {
        Some(PluginsSubcommand::List) => list_plugins(&config)?,
        Some(PluginsSubcommand::Enable { name }) => enable_plugin(&mut config, &name)?,
        Some(PluginsSubcommand::Disable { name }) => disable_plugin(&mut config, &name)?,
        Some(PluginsSubcommand::Config { name, set }) => configure_plugin(&mut config, &name, set.as_deref())?,
        Some(PluginsSubcommand::Login { provider }) => login_plugin_oauth(&provider).await?,
        Some(PluginsSubcommand::Logout { provider }) => logout_plugin_oauth(&provider)?,
        None => run_interactive_plugins_wizard(&mut config).await?,
    }

    Ok(())
}

fn is_plugin_enabled(config: &MintConfig, field: &str) -> bool {
    config
        .extra
        .get(field)
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn list_plugins(config: &MintConfig) -> Result<()> {
    println!("\x1b[1;32m Mint Built-in Plugins & Integrations\x1b[0m\n");
    println!("  {:<16} {:<12} {:<24} {}", "PLUGIN", "STATUS", "OAUTH CONNECTED", "DESCRIPTION");
    println!("  {}", "─".repeat(80));

    let oauth_statuses = list_oauth_statuses();

    for p in BUILTIN_PLUGINS {
        let enabled = is_plugin_enabled(config, p.config_field);
        let status_str = if enabled { "\x1b[32mEnabled 🟢\x1b[0m" } else { "\x1b[90mDisabled ⚪\x1b[0m" };

        let oauth_info = if p.is_oauth {
            let matched = oauth_statuses.iter().find(|s| s.provider == p.oauth_provider);
            if let Some(st) = matched {
                if st.connected {
                    format!("\x1b[36mConnected ({})\x1b[0m", st.account_email.as_deref().unwrap_or("Yes"))
                } else {
                    "\x1b[90mNot Signed In\x1b[0m".to_string()
                }
            } else {
                "\x1b[90mNot Signed In\x1b[0m".to_string()
            }
        } else {
            "N/A".to_string()
        };

        println!("  {:<16} {:<22} {:<34} {}", p.name, status_str, oauth_info, p.description);
    }

    println!("\n  \x1b[90mCommands: mint plugins enable <name> | mint plugins login <name> | mint plugins config <name> --set key=value\x1b[0m\n");
    Ok(())
}

fn enable_plugin(_config: &mut MintConfig, name: &str) -> Result<()> {
    let matched = BUILTIN_PLUGINS
        .iter()
        .find(|p| p.key.eq_ignore_ascii_case(name) || p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow!("Plugin '{name}' not found. Run 'mint plugins list' to see available plugins."))?;

    set_config_value(matched.config_field, serde_json::Value::Bool(true))?;
    println!("\x1b[32m✔ Plugin '{}' enabled successfully.\x1b[0m", matched.name);
    Ok(())
}

fn disable_plugin(_config: &mut MintConfig, name: &str) -> Result<()> {
    let matched = BUILTIN_PLUGINS
        .iter()
        .find(|p| p.key.eq_ignore_ascii_case(name) || p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow!("Plugin '{name}' not found."))?;

    set_config_value(matched.config_field, serde_json::Value::Bool(false))?;
    println!("\x1b[33m✔ Plugin '{}' disabled.\x1b[0m", matched.name);
    Ok(())
}

fn configure_plugin(_config: &mut MintConfig, name: &str, set_kv: Option<&str>) -> Result<()> {
    let matched = BUILTIN_PLUGINS
        .iter()
        .find(|p| p.key.eq_ignore_ascii_case(name) || p.name.eq_ignore_ascii_case(name))
        .ok_or_else(|| anyhow!("Plugin '{name}' not found."))?;

    if let Some(kv) = set_kv {
        let (k, v) = kv
            .split_once('=')
            .ok_or_else(|| anyhow!("Invalid format. Use --set key=value"))?;

        set_config_value(k.trim(), serde_json::Value::String(v.trim().to_string()))?;
        println!("\x1b[32m✔ Set credential {} for plugin '{}'.\x1b[0m", k.trim(), matched.name);
    } else {
        println!("Plugin '{}' configuration instructions:", matched.name);
        println!("  Use: mint plugins config {} --set <key=value>", matched.key);
    }

    Ok(())
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    let _ = std::process::Command::new("cmd").args(["/C", "start", url]).spawn();
    #[cfg(target_os = "macos")]
    let _ = std::process::Command::new("open").arg(url).spawn();
    #[cfg(target_os = "linux")]
    let _ = std::process::Command::new("xdg-open").arg(url).spawn();
}

async fn login_plugin_oauth(provider: &str) -> Result<()> {
    let config = load_config().unwrap_or_default();
    let client_id = match provider {
        "google" | "gmail" | "google_calendar" | "youtube_music" => {
            config.extra.get("gmailClientId").and_then(serde_json::Value::as_str).map(|s| s.to_string())
        }
        "spotify" => config.extra.get("spotifyClientId").and_then(serde_json::Value::as_str).map(|s| s.to_string()),
        "github" => config.extra.get("githubClientId").and_then(serde_json::Value::as_str).map(|s| s.to_string()),
        "vercel" => config.extra.get("vercelClientId").and_then(serde_json::Value::as_str).map(|s| s.to_string()),
        _ => None,
    };
    let redirect_uri = "http://localhost:3000/api/oauth/callback";
    let (auth_url, _state) = build_auth_url(provider, redirect_uri, client_id.as_deref())
        .ok_or_else(|| anyhow!("Invalid OAuth provider: {provider}"))?;

    println!("\x1b[1;36m🔑 Mint OAuth Sign-In Authorization\x1b[0m");
    println!("  Opening browser for provider: \x1b[32m{}\x1b[0m", provider);
    println!("  If the browser doesn't open automatically, visit this URL:\n");
    println!("  \x1b[4;34m{}\x1b[0m\n", auth_url);

    open_browser(&auth_url);

    println!("  \x1b[90mWaiting for browser authorization callback on http://localhost:3000...\x1b[0m");
    
    // Poll for OAuth completion for up to 60 seconds
    for _ in 0..30 {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        if let Some(tokens) = get_oauth_tokens(provider) {
            if !tokens.access_token.is_empty() {
                println!("\n\x1b[1;32m🟢 OAuth Connection Successful!\x1b[0m");
                println!("  Provider: {}", tokens.provider);
                if let Some(email) = tokens.account_email {
                    println!("  Connected Account: {}", email);
                }
                return Ok(());
            }
        }
    }

    println!("\n\x1b[33m⚠️ OAuth sign-in timed out. Please try again with 'mint plugins login {}'.\x1b[0m", provider);
    Ok(())
}

/// Public wrapper for the interactive /plugins command in interactive.rs
pub async fn login_plugin_oauth_public(provider: &str) {
    if let Err(e) = login_plugin_oauth(provider).await {
        println!("\x1b[31m✘ OAuth sign-in failed: {e}\x1b[0m");
    }
}

fn logout_plugin_oauth(provider: &str) -> Result<()> {
    revoke_oauth_tokens(provider).map_err(|e| anyhow!(e))?;
    println!("\x1b[33m✔ Disconnected OAuth account for provider '{}'.\x1b[0m", provider);
    Ok(())
}

pub async fn run_interactive_plugins_wizard(config: &mut MintConfig) -> Result<()> {
    println!("\x1b[1;32m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[1;32m                    Mint Native Plugins & Integrations                      \x1b[0m");
    println!("\x1b[1;32m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m\n");

    println!("Select which plugins or bridges you would like to configure:");
    println!(
        "  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Space: Toggle | a: All | i: Invert | Enter: Confirm]\x1b[0m\n"
    );

    let mut items: Vec<(&'static BuiltinPluginInfo, bool)> = BUILTIN_PLUGINS
        .iter()
        .map(|p| (p, is_plugin_enabled(config, p.config_field)))
        .collect();

    let mut cursor = 0;

    print_plugin_checkbox_items(&items, cursor);
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
                        println!("\n\x1b[31mPlugin configuration cancelled.\x1b[0m");
                        return Ok(());
                    }

                    match key_event.code {
                        KeyCode::Up => {
                            if cursor > 0 {
                                cursor -= 1;
                            } else {
                                cursor = items.len() - 1;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", items.len() + 1); // +1 for category header "Built-in Plugins:"
                            print_plugin_checkbox_items(&items, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Down => {
                            if cursor < items.len() - 1 {
                                cursor += 1;
                            } else {
                                cursor = 0;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", items.len() + 1);
                            print_plugin_checkbox_items(&items, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Char(' ') => {
                            items[cursor].1 = !items[cursor].1;
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", items.len() + 1);
                            print_plugin_checkbox_items(&items, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Char('a') => {
                            for item in &mut items {
                                item.1 = true;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", items.len() + 1);
                            print_plugin_checkbox_items(&items, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Char('i') => {
                            for item in &mut items {
                                item.1 = !item.1;
                            }
                            disable_raw_mode()?;
                            print!("\x1b[{}A\x1b[J", items.len() + 1);
                            print_plugin_checkbox_items(&items, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Enter => {
                            disable_raw_mode()?;
                            println!();
                            break;
                        }
                        _ => {}
                    }
                }
            }
            Ok(false) => {}
            Err(_) => {
                disable_raw_mode()?;
                break;
            }
        }
    }

    // Save toggled states to config.json
    for (plugin, enabled) in &items {
        set_config_value(plugin.config_field, serde_json::Value::Bool(*enabled))?;
    }
    println!("\x1b[32m✔ Plugin preferences saved to config.json!\x1b[0m\n");

    // Prompt credentials/OAuth choice for newly enabled plugins
    for (plugin, enabled) in &items {
        if *enabled {
            let title = format!("\x1b[1;36m⚙️ Authentication Setup for {}\x1b[0m", plugin.name);
            if plugin.is_oauth {
                let options = &[
                    "Sign In via OAuth 🌐 (Browser popup authorization)",
                    "Enter API Keys / Credentials 🔑 (Manual input)",
                    "Skip for now",
                ];
                let selected_idx = prompt_select_option_menu(&title, options)?;
                match selected_idx {
                    1 => prompt_manual_plugin_fields(plugin.key)?,
                    2 => println!("\x1b[90mSkipped setup for {}.\x1b[0m\n", plugin.name),
                    _ => login_plugin_oauth(plugin.oauth_provider).await?,
                }
            } else {
                let options = &[
                    "Configure Credentials 🔑",
                    "Skip for now",
                ];
                let selected_idx = prompt_select_option_menu(&title, options)?;
                if selected_idx == 0 {
                    prompt_manual_plugin_fields(plugin.key)?;
                }
            }
        }
    }

    Ok(())
}

fn prompt_select_option_menu(title: &str, options: &[&str]) -> Result<usize> {
    println!("{}", title);
    println!("  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Enter: Select]\x1b[0m");

    let mut cursor = 0;
    print_select_options_list(options, cursor);
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
                        return Ok(options.len() - 1);
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
                            print_select_options_list(options, cursor);
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
                            print_select_options_list(options, cursor);
                            enable_raw_mode()?;
                        }
                        KeyCode::Enter => {
                            disable_raw_mode()?;
                            println!();
                            return Ok(cursor);
                        }
                        _ => {}
                    }
                }
            }
            Ok(false) => {}
            Err(_) => {
                disable_raw_mode()?;
                return Ok(options.len() - 1);
            }
        }
    }
}

fn print_select_options_list(options: &[&str], cursor: usize) {
    for (i, option) in options.iter().enumerate() {
        if i == cursor {
            println!("  \x1b[36m❯\x1b[0m \x1b[36m{}\x1b[0m", option);
        } else {
            println!("    {}", option);
        }
    }
    let _ = io::stdout().flush();
}

fn prompt_manual_plugin_fields(key: &str) -> Result<()> {
    match key {
        "gmail" => {
            println!("\x1b[33m--- Gmail API Credentials --- \x1b[0m");
            print_and_set_key("gmailClientId", "Google Client ID")?;
            print_and_set_key("gmailClientSecret", "Google Client Secret")?;
            print_and_set_key("gmailRefreshToken", "Gmail Refresh Token (optional)")?;
        }
        "calendar" => {
            println!("\x1b[33m--- Google Calendar API Credentials --- \x1b[0m");
            print_and_set_key("googleCalendarClientId", "Google Client ID")?;
            print_and_set_key("googleCalendarClientSecret", "Google Client Secret")?;
            print_and_set_key("googleCalendarRefreshToken", "Refresh Token (optional)")?;
        }
        "notion" => {
            println!("\x1b[33m--- Notion API Credentials --- \x1b[0m");
            print_and_set_key("notionApiKey", "Notion Integration API Key (secret_...)")?;
            print_and_set_key("notionDatabaseId", "Default Database ID (optional)")?;
        }
        "spotify" => {
            println!("\x1b[33m--- Spotify API Credentials --- \x1b[0m");
            print_and_set_key("spotifyClientId", "Spotify Client ID")?;
            print_and_set_key("spotifyClientSecret", "Spotify Client Secret")?;
        }
        "youtube_music" => {
            println!("\x1b[33m--- YouTube Music API Credentials --- \x1b[0m");
            print_and_set_key("gmailClientId", "Google Client ID")?;
            print_and_set_key("gmailClientSecret", "Google Client Secret")?;
        }
        "vercel" => {
            println!("\x1b[33m--- Vercel API Credentials --- \x1b[0m");
            print_and_set_key("vercelClientId", "Vercel Client ID (optional for OAuth)")?;
            print_and_set_key("vercelToken", "Vercel Access Token")?;
        }
        "github" => {
            println!("\x1b[33m--- GitHub API Credentials --- \x1b[0m");
            print_and_set_key("githubClientId", "GitHub Client ID (optional for OAuth)")?;
            print_and_set_key("githubClientSecret", "GitHub Client Secret (optional for OAuth)")?;
            print_and_set_key("githubToken", "GitHub Personal Access Token (PAT)")?;
        }
        _ => {
            println!("No manual API fields required for {}.", key);
        }
    }
    Ok(())
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

fn print_and_set_key(config_key: &str, label: &str) -> Result<()> {
    let config = load_config()?;
    let existing = config
        .extra
        .get(config_key)
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();

    if !existing.is_empty() {
        print!("{} [\x1b[90mkeep existing ({})\x1b[0m]: ", label, format_masked_key(&existing));
    } else {
        print!("{}: ", label);
    }
    io::stdout().flush()?;

    let mut val = String::new();
    io::stdin().read_line(&mut val)?;
    let val = val.trim();
    if !val.is_empty() {
        set_config_value(config_key, serde_json::Value::String(val.to_string()))?;
        println!("\x1b[32m✔ Saved {}\x1b[0m", label);
    } else if !existing.is_empty() {
        println!("\x1b[32m✔ Kept existing {}\x1b[0m", label);
    }
    Ok(())
}

fn print_plugin_checkbox_items(items: &[(&BuiltinPluginInfo, bool)], cursor: usize) {
    println!("\x1b[32mBuilt-in Plugins:\x1b[0m");
    for (i, (plugin, enabled)) in items.iter().enumerate() {
        let checkbox = if *enabled {
            "\x1b[32m◉\x1b[0m"
        } else {
            "\x1b[90m○\x1b[0m"
        };
        if i == cursor {
            println!(
                "  \x1b[36m❯\x1b[0m {} \x1b[36m{:<18}\x1b[0m \x1b[90m({})\x1b[0m",
                checkbox, plugin.name, plugin.description
            );
        } else {
            println!("    {} {:<18} \x1b[90m({})\x1b[0m", checkbox, plugin.name, plugin.description);
        }
    }
    let _ = io::stdout().flush();
}

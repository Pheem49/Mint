use super::*;

fn is_reachable(host: &str, port: u16) -> bool {
    use std::net::{TcpStream, ToSocketAddrs};
    use std::time::Duration;
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
        .is_some_and(|addr| TcpStream::connect_timeout(&addr, Duration::from_millis(300)).is_ok())
}

struct CompanionService {
    name: &'static str,
    host: &'static str,
    port: u16,
    mcp_key: &'static str,
    setup_doc: &'static str,
}

const COMPANION_SERVICES: &[CompanionService] = &[
    CompanionService {
        name: "n8n",
        host: "127.0.0.1",
        port: 5678,
        mcp_key: "n8n",
        setup_doc: "docs/N8N_INTEGRATION.md",
    },
    CompanionService {
        name: "SurfSense",
        host: "127.0.0.1",
        port: 3929,
        mcp_key: "surfsense",
        setup_doc: "docs/SURFSENSE_INTEGRATION.md",
    },
];

/// Flash a small inline status panel for the companion services (n8n, Mint
/// Notebook, ...) instead of printing a wall of warning text — reuses the
/// same "inline ratatui `Terminal`, draw once, `clear()` to finalize into
/// scrollback" pattern `picker.rs` uses for `/mcp` and `/models`, just
/// without the interactive redraw loop since there's nothing to select here.
fn render_companion_status_panel() {
    use ansi_to_tui::IntoText;
    use crossterm::tty::IsTty;

    let mcp_servers = crate::mcp::list().unwrap_or_default();
    let name_width = COMPANION_SERVICES
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(0);

    let mut lines = vec![format!("{BLUE}Companion Services{RESET}")];
    for service in COMPANION_SERVICES {
        let up = is_reachable(service.host, service.port);
        let (dot, color, status_label) = if up {
            ("●", MINT, "running")
        } else {
            ("○", DIM, "not running")
        };
        let mcp_connected = mcp_servers.contains_key(service.mcp_key);
        let mcp_label = if mcp_connected {
            format!("{MINT}MCP: connected{RESET}")
        } else {
            format!("{DIM}MCP: not configured{RESET}")
        };
        lines.push(format!(
            "  {color}{dot}{RESET} {:<name_width$}  {color}{status_label:<11}{RESET} {mcp_label}",
            service.name
        ));
        if !up || !mcp_connected {
            lines.push(format!("      {DIM}setup: {}{RESET}", service.setup_doc));
        }
    }
    let text = lines.join("\n");

    if !io::stdout().is_tty() {
        println!("{text}\n");
        return;
    }

    let height = (lines.len() as u16).saturating_add(1);
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let terminal = agent::with_raw_mode_for_cursor_query(move || {
        ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Inline(height),
            },
        )
    });
    let Ok(mut terminal) = terminal else {
        println!("{text}\n");
        return;
    };
    if let Ok(rendered) = text.into_text() {
        let _ = terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(ratatui::widgets::Paragraph::new(rendered), area);
        });
    }
    let _ = terminal.clear();
    println!();
}

fn parse_path_and_prompt(rest: &str) -> (String, String) {
    let rest = rest.trim();
    if rest.is_empty() {
        return (String::new(), String::new());
    }

    let mut chars = rest.chars();
    if let Some(first) = chars.next() {
        if first == '"' || first == '\'' {
            let quote = first;
            if let Some(end_idx) = rest[1..].find(quote) {
                let path = rest[1..1 + end_idx].to_string();
                let prompt = rest[1 + end_idx + 1..].trim().to_string();
                return (path, prompt);
            }
        }
    }

    if let Some((path, prompt)) = rest.split_once(char::is_whitespace) {
        (path.to_string(), prompt.trim().to_string())
    } else {
        (rest.to_string(), String::new())
    }
}
/// Newest-first list of saved plan files under `<root>/.agents/plans/`
/// (written by `agent::save_plan_file` when a plan is approved) — the
/// `YYYYMMDD-HHMMSS-slug.md` filename already sorts chronologically, so a
/// reverse lexicographic sort on the filename is enough.
fn list_plan_files(root: &Path) -> Vec<PathBuf> {
    let plans_dir = root.join(".agents").join("plans");
    let Ok(entries) = std::fs::read_dir(&plans_dir) else {
        return Vec::new();
    };
    let mut files: Vec<PathBuf> = entries
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("md"))
        .collect();
    files.sort_by(|a, b| b.file_name().cmp(&a.file_name()));
    files
}

/// First non-empty line of a file, truncated to 80 chars — same preview
/// style `/memory list` uses for its interaction previews.
fn first_line_preview(path: &Path) -> String {
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let first = content
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("");
    if first.chars().count() > 80 {
        let short: String = first.chars().take(80).collect();
        format!("{short}…")
    } else {
        first.to_string()
    }
}

/// Route `/…` commands. Returns `None` if the input is not a slash command.
pub async fn handle_slash_command(
    session: &mut InteractiveSession,
    query: &str,
) -> Option<SlashResult> {
    let trimmed = query.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    // Split into command word and optional rest
    let (cmd, rest) = trimmed
        .split_once(char::is_whitespace)
        .map(|(c, r)| (c, r.trim()))
        .unwrap_or((trimmed, ""));

    match cmd {
        "/help" => {
            println!("\n{BLUE}────────────────────────────────────────────{RESET}");
            println!("{MINT}  Mint Interactive Commands{RESET}");
            println!("{BLUE}────────────────────────────────────────────{RESET}");
            for spec in SLASH_COMMANDS.iter() {
                let label = if spec.usage.is_empty() {
                    spec.token.to_string()
                } else {
                    format!("{} {}", spec.token, spec.usage)
                };
                println!(
                    "  {MINT}{:<30}{RESET} {DIM}{}{RESET}",
                    label, spec.description
                );
            }
            println!();
            Some(SlashResult::Handled)
        }

        "/init" => Some(SlashResult::ForwardToAgent(
            mint_core::slash::INIT_AGENTS_MD_PROMPT.to_string(),
        )),

        "/fast" => {
            let choice = if rest.is_empty() {
                let options = vec![
                    "on (hide thinking traces)".to_string(),
                    "off (show thinking traces)".to_string(),
                ];
                let current = if session.fast_mode {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Select Fast Mode", &options, current) {
                    Ok(Some(sel)) => {
                        if sel.starts_with("on") {
                            Some(true)
                        } else {
                            Some(false)
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        println!("{ERROR}Error selecting fast mode:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                match rest {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => {
                        println!("{WARN}/fast usage: /fast [on|off]{RESET}\n");
                        None
                    }
                }
            };

            if let Some(mode) = choice {
                session.fast_mode = mode;
                if session.fast_mode {
                    println!("{DIM}[Fast] mode ON — thinking traces hidden{RESET}\n");
                } else {
                    println!("{DIM}[Fast] mode OFF{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/plan" if rest == "list" => {
            let files = list_plan_files(&session.current_dir);
            if files.is_empty() {
                println!("{DIM}No saved plans yet.{RESET}\n");
            } else {
                println!("\n{BLUE}Saved plans:{RESET}");
                for (idx, path) in files.iter().enumerate() {
                    println!(
                        "  {DIM}[{}]{RESET} {BLUE}{}{RESET}",
                        idx + 1,
                        first_line_preview(path)
                    );
                }
                println!();
            }
            Some(SlashResult::Handled)
        }

        "/plan" if rest.starts_with("show") => {
            let arg = rest.trim_start_matches("show").trim();
            let files = list_plan_files(&session.current_dir);
            let found = if let Ok(idx) = arg.parse::<usize>() {
                idx.checked_sub(1).and_then(|i| files.get(i))
            } else {
                files.iter().find(|p| {
                    p.file_name()
                        .and_then(|n| n.to_str())
                        .is_some_and(|n| n.contains(arg))
                })
            };
            match found {
                Some(path) => match std::fs::read_to_string(path) {
                    Ok(content) => println!("\n{content}\n"),
                    Err(e) => println!("{ERROR}Could not read plan file:{RESET} {e}\n"),
                },
                None => println!(
                    "{WARN}No saved plan matching \"{arg}\" — run /plan list to see saved plans.{RESET}\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/plan" => {
            let choice = if rest.is_empty() {
                let options = vec![
                    "on (read-only until plan is approved)".to_string(),
                    "off (edits/shell run immediately)".to_string(),
                ];
                let current = if session.plan_mode {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Select Plan Mode", &options, current) {
                    Ok(Some(sel)) => {
                        if sel.starts_with("on") {
                            Some(true)
                        } else {
                            Some(false)
                        }
                    }
                    Ok(None) => None,
                    Err(e) => {
                        println!("{ERROR}Error selecting plan mode:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                match rest {
                    "on" => Some(true),
                    "off" => Some(false),
                    _ => {
                        println!("{WARN}/plan usage: /plan [on|off]{RESET}\n");
                        None
                    }
                }
            };

            if let Some(mode) = choice {
                session.plan_mode = mode;
                if session.plan_mode {
                    println!(
                        "{DIM}[Plan] mode ON — agent will investigate read-only and present a plan via exit_plan_mode before editing files or running commands{RESET}\n"
                    );
                } else {
                    println!("{DIM}[Plan] mode OFF{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/models" => {
            let selected_provider = if rest.is_empty() {
                let providers = session.config.available_providers();
                match prompt_interactive_select(
                    "Select AI provider",
                    &providers,
                    &session.config.ai_provider,
                ) {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        println!("Cancelled provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                Some(rest.to_owned())
            };

            if let Some(provider) = selected_provider {
                // Switch provider first so `active_model()` reflects that
                // provider's own default/last-used model, then offer a
                // second picker (when a model list is known for it) so the
                // user isn't stuck with the default after choosing a provider.
                match session.config.set_active_model(&provider, None) {
                    Ok(mut display_name) => {
                        let mut model_options =
                            model_options_for_provider(&session.config, &provider);
                        let current_model = session.config.active_model().to_string();
                        if !current_model.is_empty()
                            && !model_options.iter().any(|m| m == &current_model)
                        {
                            model_options.insert(0, current_model.clone());
                        }

                        if !model_options.is_empty() {
                            match prompt_interactive_select(
                                &format!("Select {provider} model"),
                                &model_options,
                                &current_model,
                            ) {
                                Ok(Some(model)) if model != current_model => {
                                    match session.config.set_active_model(&provider, Some(&model)) {
                                        Ok(name) => display_name = name,
                                        Err(error) => {
                                            println!("{ERROR}Config error:{RESET} {error}")
                                        }
                                    }
                                }
                                Ok(_) => {}
                                Err(e) => {
                                    println!("{ERROR}Error selecting model:{RESET} {e}\n")
                                }
                            }
                        }

                        println!(
                            "\n{DIM}───{RESET} {MINT}{}{RESET} {DIM}───{RESET}\n",
                            display_name
                        );
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/multi-agent" => {
            if rest == "on" || rest == "off" {
                session.config.enable_agent_collaboration = rest == "on";
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Multi-Agent collaboration set to: {}{RESET}\n",
                        if session.config.enable_agent_collaboration {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /multi-agent [on|off]{RESET}\n");
            } else {
                println!("\n{BLUE}Multi-Agent System Settings:{RESET}");
                let collab_status = if session.config.enable_agent_collaboration {
                    format!("{MINT}Enabled (on){RESET}")
                } else {
                    format!("{DIM}Disabled (off){RESET}")
                };
                println!("  Global Collaboration: {collab_status}");
                println!("\n{BLUE}Configured Agents:{RESET}");
                if session.config.agents.is_empty() {
                    println!("  No agents configured.");
                } else {
                    for agent in &session.config.agents {
                        let status_label = if agent.enabled {
                            format!("{MINT}[Enabled]{RESET}")
                        } else {
                            format!("{DIM}[Disabled]{RESET}")
                        };
                        println!(
                            "  {:<15} {}  Provider: {:<12} Model: {}",
                            agent.name, status_label, agent.provider, agent.model
                        );
                        let truncated_instruction = if agent.system_instruction.chars().count() > 60
                        {
                            let head: String = agent.system_instruction.chars().take(60).collect();
                            format!("{}...", head.replace('\n', " "))
                        } else {
                            agent.system_instruction.replace('\n', " ")
                        };
                        println!("    {DIM}Instruction: {truncated_instruction}{RESET}");
                    }
                }
                println!();

                let options = vec![
                    "Keep current status".to_string(),
                    "on (enable collaboration)".to_string(),
                    "off (disable collaboration)".to_string(),
                ];
                let current = &options[0];
                match prompt_interactive_select("Toggle Collaboration Status?", &options, current) {
                    Ok(Some(sel)) => {
                        if sel.starts_with("on") {
                            session.config.enable_agent_collaboration = true;
                            match mint_core::save_config(&session.config) {
                                Ok(()) => println!(
                                    "{DIM}Multi-Agent collaboration set to: Enabled{RESET}\n"
                                ),
                                Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                            }
                        } else if sel.starts_with("off") {
                            session.config.enable_agent_collaboration = false;
                            match mint_core::save_config(&session.config) {
                                Ok(()) => println!(
                                    "{DIM}Multi-Agent collaboration set to: Disabled{RESET}\n"
                                ),
                                Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                            }
                        }
                    }
                    _ => {}
                }
            }
            Some(SlashResult::Handled)
        }

        "/avatar" => {
            use mint_core::avatar_bridge::{AvatarBridgeConfig, fetch_channel_state};

            // Soft toggle: `avatar_signal_disabled` hides the tool while keeping
            // the token so the paired viewer never needs re-linking.
            let set_signal_disabled = |session: &mut InteractiveSession, disabled: bool| {
                session.config.avatar_signal_disabled = disabled;
                match mint_core::save_config(&session.config) {
                    Ok(()) => {
                        if disabled {
                            println!(
                                "{DIM}[Avatar] avatar_signal disabled (token kept — run /avatar to re-enable).{RESET}\n"
                            );
                        } else {
                            println!("{DIM}[Avatar] avatar_signal enabled.{RESET}\n");
                        }
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            };

            // Resolve to a share target ("web"/"desktop"), or bail out for the
            // No / Off / On / status / bad-usage sub-actions.
            let target: &str = match rest {
                "web" | "link web" => "web",
                "desktop" | "link desktop" => "desktop",
                "on" | "enable" => {
                    set_signal_disabled(session, false);
                    return Some(SlashResult::Handled);
                }
                "off" | "disable" => {
                    set_signal_disabled(session, true);
                    return Some(SlashResult::Handled);
                }
                "" | "link" => {
                    let options = vec![
                        "No".to_string(),
                        "Off (disable avatar_signal)".to_string(),
                        "Web browser".to_string(),
                        "Desktop app".to_string(),
                    ];
                    match prompt_interactive_select("Avatar output", &options, &options[2]) {
                        Ok(Some(sel)) if sel == "No" => return Some(SlashResult::Handled),
                        Ok(Some(sel)) if sel.starts_with("Off") => {
                            set_signal_disabled(session, true);
                            return Some(SlashResult::Handled);
                        }
                        Ok(Some(sel)) if sel.starts_with("Desktop") => "desktop",
                        Ok(Some(_)) => "web",
                        // Cancelled / no TTY — the picker has an explicit "No",
                        // so treat a cancel as "do nothing".
                        Ok(None) => return Some(SlashResult::Handled),
                        Err(e) => {
                            println!("{ERROR}Error selecting target:{RESET} {e}\n");
                            return Some(SlashResult::Handled);
                        }
                    }
                }
                "status" => {
                    let cfg = AvatarBridgeConfig::from_mint_config(&session.config);
                    if !cfg.enabled {
                        println!("{WARN}[Avatar] No token yet — run /avatar link first.{RESET}\n");
                    } else {
                        println!(
                            "{DIM}[Avatar] avatar_signal tool: {}{RESET}",
                            if session.config.avatar_signal_disabled {
                                "disabled"
                            } else {
                                "enabled"
                            }
                        );
                        match fetch_channel_state(&cfg).await {
                            Ok(state) => {
                                let now_ms = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .map(|d| d.as_millis() as i64)
                                    .unwrap_or(0);
                                println!("\n{BLUE}[Avatar] Channel status:{RESET}");
                                println!(
                                    "  Model:       {}",
                                    state.model.unwrap_or_else(|| "not selected".into())
                                );
                                println!("  Viewers:     {}", state.connected_clients);
                                println!(
                                    "  Last event:  {}\n",
                                    state
                                        .last_agent_event_at
                                        .map(|t| format!("{}s ago", (now_ms - t).max(0) / 1000))
                                        .unwrap_or_else(|| "never".into())
                                );
                            }
                            Err(error) => println!("{ERROR}[Avatar] {error}{RESET}\n"),
                        }
                    }
                    return Some(SlashResult::Handled);
                }
                _ => {
                    println!(
                        "{WARN}Usage: /avatar [web|desktop|on|off|status] — connects agent activity to Project Avatar{RESET}\n"
                    );
                    return Some(SlashResult::Handled);
                }
            };

            // Web / Desktop chosen — make sure the tool is on and a token exists,
            // then print the connection instructions.
            session.config.avatar_signal_disabled = false;
            if session.config.avatar_token.is_empty() {
                session.config.avatar_token = AvatarBridgeConfig::generate_token();
            }
            if let Err(error) = mint_core::save_config(&session.config) {
                println!("{ERROR}Config error:{RESET} {error}\n");
                return Some(SlashResult::Handled);
            }
            let cfg = AvatarBridgeConfig::from_mint_config(&session.config);
            let token = session.config.avatar_token.clone();
            if target == "desktop" {
                println!(
                    "\n{MINT}[Avatar] Desktop app:{RESET}\n\
                     1. Open the Project Avatar desktop app.\n\
                     2. It has no address bar, so on first launch it generates its \
                     own token — paste this one into its \"Paste existing token...\" \
                     field instead so it connects to the same channel Mint pushes to:\n\
                     {DIM}{token}{RESET}\n\
                     It remembers the token after that; you won't need to paste it again.\n"
                );
            } else {
                println!(
                    "\n{MINT}[Avatar] Share link:{RESET}\n{}\n",
                    cfg.share_link().expect("token was just ensured non-empty")
                );
            }
            Some(SlashResult::Handled)
        }

        "/autoskill" => {
            if rest == "on" || rest == "off" {
                session.config.auto_skill_writing = rest == "on";
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Auto skill writing set to: {}{RESET}\n",
                        if session.config.auto_skill_writing {
                            "Enabled"
                        } else {
                            "Disabled"
                        }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /autoskill [on|off]{RESET}\n");
            } else {
                println!(
                    "{DIM}When enabled, the agent may write a new .agents/skills/<name>/SKILL.md after finishing a non-trivial, reusable task.{RESET}"
                );
                let options = vec!["on (enable)".to_string(), "off (disable)".to_string()];
                let current = if session.config.auto_skill_writing {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Auto Skill Writing", &options, current) {
                    Ok(Some(sel)) => {
                        session.config.auto_skill_writing = sel.starts_with("on");
                        match mint_core::save_config(&session.config) {
                            Ok(()) => println!(
                                "{DIM}Auto skill writing set to: {}{RESET}\n",
                                if session.config.auto_skill_writing {
                                    "Enabled"
                                } else {
                                    "Disabled"
                                }
                            ),
                            Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                        }
                    }
                    Ok(None) => println!("{DIM}Cancelled.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error selecting option:{RESET} {e}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/autorecall" => {
            let apply = |session: &mut InteractiveSession, on: bool| {
                session.config.memory_recall = on;
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Memory recall set to: {}{RESET}\n",
                        if on { "Enabled" } else { "Disabled" }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            };
            if rest == "on" || rest == "off" {
                apply(session, rest == "on");
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /autorecall [on|off]{RESET}\n");
            } else {
                println!(
                    "{DIM}When enabled, each turn full-text-searches this conversation's older messages and injects the few most relevant to what you just said.{RESET}"
                );
                let options = vec!["on (enable)".to_string(), "off (disable)".to_string()];
                let current = if session.config.memory_recall {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Memory Recall", &options, current) {
                    Ok(Some(sel)) => apply(session, sel.starts_with("on")),
                    Ok(None) => println!("{DIM}Cancelled.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error selecting option:{RESET} {e}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/autofacts" => {
            let apply = |session: &mut InteractiveSession, on: bool| {
                session.config.auto_fact_extraction = on;
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Auto fact extraction set to: {}{RESET}\n",
                        if on { "Enabled" } else { "Disabled" }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            };
            if rest == "on" || rest == "off" {
                apply(session, rest == "on");
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /autofacts [on|off]{RESET}\n");
            } else {
                println!(
                    "{DIM}When enabled, a turn that states a durable preference, naming, or standing instruction triggers a background pass that saves it to long-term memory.{RESET}"
                );
                let options = vec!["on (enable)".to_string(), "off (disable)".to_string()];
                let current = if session.config.auto_fact_extraction {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Auto Fact Extraction", &options, current) {
                    Ok(Some(sel)) => apply(session, sel.starts_with("on")),
                    Ok(None) => println!("{DIM}Cancelled.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error selecting option:{RESET} {e}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/factrecall" => {
            let apply = |session: &mut InteractiveSession, on: bool| {
                session.config.semantic_fact_recall = on;
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Semantic fact recall set to: {}{RESET}\n",
                        if on { "Enabled" } else { "Disabled" }
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}\n"),
                }
            };
            if rest == "on" || rest == "off" {
                apply(session, rest == "on");
            } else if !rest.is_empty() {
                println!("{WARN}Usage: /factrecall [on|off]{RESET}\n");
            } else {
                println!(
                    "{DIM}When your stored facts overflow their per-turn budget, this fills the overflow with the facts most relevant to your message instead of just the newest ones (the newest few are always kept).{RESET}"
                );
                let options = vec!["on (enable)".to_string(), "off (disable)".to_string()];
                let current = if session.config.semantic_fact_recall {
                    &options[0]
                } else {
                    &options[1]
                };
                match prompt_interactive_select("Semantic Fact Recall", &options, current) {
                    Ok(Some(sel)) => apply(session, sel.starts_with("on")),
                    Ok(None) => println!("{DIM}Cancelled.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error selecting option:{RESET} {e}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/remember" => {
            let (first, tail) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            let (scope, project_path, body) = if first.eq_ignore_ascii_case("here") {
                (
                    "project",
                    Some(session.current_dir.to_string_lossy().into_owned()),
                    tail,
                )
            } else {
                ("user", None, rest)
            };
            if body.is_empty() {
                println!("{WARN}Usage: /remember [here] <text>{RESET}\n");
                return Some(SlashResult::Handled);
            }
            match MemoryStore::open_default() {
                Ok(memory) => {
                    match memory.add_fact(scope, project_path.as_deref(), body, None, None) {
                        Ok(Some(_)) => println!(
                            "{DIM}Remembered{}.{RESET}\n",
                            if scope == "project" {
                                " (this project)"
                            } else {
                                ""
                            }
                        ),
                        Ok(None) => println!("{DIM}Already remembered.{RESET}\n"),
                        Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                    }
                }
                Err(e) => println!("{ERROR}Memory error:{RESET} {e}\n"),
            }
            Some(SlashResult::Handled)
        }

        "/image-provider" => {
            let mut available = Vec::new();
            if !session.config.api_key.trim().is_empty() {
                available.push("nanobanana");
            }
            if !session.config.openai_api_key.trim().is_empty() {
                available.push("dalle");
            }
            if !session.config.stability_api_key.trim().is_empty() {
                available.push("stability");
            }
            if !session.config.ideogram_api_key.trim().is_empty() {
                available.push("ideogram");
            }
            if !session.config.replicate_api_key.trim().is_empty() {
                available.push("replicate");
            }
            if available.is_empty() {
                available.push("nanobanana");
            }

            let selected_provider = if rest.is_empty() {
                let options: Vec<String> = available.iter().map(|s| s.to_string()).collect();
                match prompt_interactive_select(
                    "Select Image Generation provider",
                    &options,
                    &session.config.image_gen_provider,
                ) {
                    Ok(Some(p)) => Some(p),
                    Ok(None) => {
                        println!("Cancelled image provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                if available.contains(&rest) {
                    Some(rest.to_owned())
                } else {
                    println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                    None
                }
            };

            if let Some(provider) = selected_provider {
                session.config.image_gen_provider = provider;
                match mint_core::save_config(&session.config) {
                    Ok(()) => println!(
                        "{DIM}Switched default image provider to: {}{RESET}\n",
                        session.config.image_gen_provider
                    ),
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/video-provider" => {
            let mut available = Vec::new();
            if !session.config.api_key.trim().is_empty() || std::env::var("GEMINI_API_KEY").is_ok()
            {
                available.push("veo");
            }
            if available.is_empty() {
                available.push("veo");
            }

            let current_provider = session
                .config
                .extra
                .get("videoGenProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("veo")
                .to_string();

            let selected_provider = if rest.is_empty() {
                let options = vec!["Google Veo (Gemini Videos)".to_string()];
                let current_display = if current_provider == "veo" {
                    "Google Veo (Gemini Videos)"
                } else {
                    &current_provider
                };
                match prompt_interactive_select(
                    "Select Video Generation provider",
                    &options,
                    current_display,
                ) {
                    Ok(Some(p)) => {
                        if p == "Google Veo (Gemini Videos)" {
                            Some("veo".to_string())
                        } else {
                            Some(p)
                        }
                    }
                    Ok(None) => {
                        println!("Cancelled video provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else {
                if rest == "veo" || rest == "Google Veo (Gemini Videos)" {
                    Some("veo".to_string())
                } else {
                    println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                    None
                }
            };

            if let Some(provider) = selected_provider {
                session.config.extra.insert(
                    "videoGenProvider".to_string(),
                    serde_json::Value::String(provider.clone()),
                );
                match mint_core::save_config(&session.config) {
                    Ok(()) => {
                        let display_name = if provider == "veo" {
                            "Google Veo (Gemini Videos)"
                        } else {
                            &provider
                        };
                        println!(
                            "{DIM}Switched default video provider to: {}{RESET}\n",
                            display_name
                        );
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/search-provider" | "/searchProvider" => {
            let mut available: Vec<(&str, &str)> = Vec::new();
            let google_configured = session
                .config
                .extra
                .get("googleSearchApiKey")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty())
                && session
                    .config
                    .extra
                    .get("googleSearchCx")
                    .and_then(|v| v.as_str())
                    .is_some_and(|v| !v.trim().is_empty());
            if google_configured {
                available.push(("google", "Google Search API"));
            }
            if session
                .config
                .extra
                .get("braveSearchApiKey")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty())
            {
                available.push(("brave", "Brave Search API"));
            }
            if session
                .config
                .extra
                .get("searxngBaseUrl")
                .and_then(|v| v.as_str())
                .is_some_and(|v| !v.trim().is_empty())
            {
                available.push(("searxng", "SearXNG (self-hosted)"));
            }

            if available.is_empty() {
                println!(
                    "{ERROR}No search provider is configured yet.{RESET} Run onboarding or set a Google/Brave key or SearXNG URL in Settings first.\n"
                );
                return Some(SlashResult::Handled);
            }

            let current_provider = session
                .config
                .extra
                .get("searchProvider")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            let selected_provider = if rest.is_empty() {
                let options: Vec<String> = available
                    .iter()
                    .map(|(_, label)| label.to_string())
                    .collect();
                let current_display = available
                    .iter()
                    .find(|(key, _)| *key == current_provider)
                    .map(|(_, label)| *label)
                    .unwrap_or(available[0].1);
                match prompt_interactive_select(
                    "Select Web Search provider",
                    &options,
                    current_display,
                ) {
                    Ok(Some(label)) => available
                        .iter()
                        .find(|(_, l)| *l == label)
                        .map(|(key, _)| key.to_string()),
                    Ok(None) => {
                        println!("Cancelled search provider selection.\n");
                        None
                    }
                    Err(e) => {
                        println!("{ERROR}Error selecting provider:{RESET} {e}\n");
                        None
                    }
                }
            } else if let Some((key, _)) = available
                .iter()
                .find(|(key, label)| *key == rest || *label == rest)
            {
                Some(key.to_string())
            } else {
                println!("{ERROR}Provider '{rest}' is not configured or invalid.{RESET}\n");
                None
            };

            if let Some(provider) = selected_provider {
                session.config.extra.insert(
                    "searchProvider".to_string(),
                    serde_json::Value::String(provider.clone()),
                );
                match mint_core::save_config(&session.config) {
                    Ok(()) => {
                        let display_name = available
                            .iter()
                            .find(|(key, _)| *key == provider)
                            .map(|(_, label)| *label)
                            .unwrap_or(&provider);
                        println!(
                            "{DIM}Switched default web search provider to: {}{RESET}\n",
                            display_name
                        );
                    }
                    Err(error) => println!("{ERROR}Config error:{RESET} {error}"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/bg" => {
            if rest.is_empty() {
                println!("{ERROR}Usage:{RESET} /bg <query>\n");
                return Some(SlashResult::Handled);
            }
            let id = session
                .jobs
                .spawn(&session.config, &session.current_dir, rest);
            println!(
                "{DIM}Started background job #{id}. Keep chatting — check on it with {RESET}/jobs{DIM}, or view its result with {RESET}/jobs show {id}{DIM}.{RESET}"
            );
            println!(
                "{DIM}Note: /bg runs non-interactively, so file writes/shell/plugin/MCP actions needing approval are declined automatically.{RESET}\n"
            );
            Some(SlashResult::Handled)
        }

        "/jobs" => {
            if rest.is_empty() {
                let list = session.jobs.list();
                if list.is_empty() {
                    println!("{DIM}No background jobs yet. Start one with /bg <query>.{RESET}\n");
                } else {
                    println!("\n{BLUE}Background jobs{RESET}");
                    for job in &list {
                        let status_str = match job.status {
                            JobStatus::Running => format!("{MINT}running{RESET}"),
                            JobStatus::Done => "done".to_string(),
                            JobStatus::Failed => format!("{ERROR}failed{RESET}"),
                            JobStatus::Cancelled => format!("{DIM}cancelled{RESET}"),
                        };
                        let preview: String = job.query.chars().take(50).collect();
                        let suffix = if job.query.chars().count() > 50 {
                            "…"
                        } else {
                            ""
                        };
                        println!(
                            "  #{:<3} [{status_str}] {DIM}{:>4}s{RESET}  {preview}{suffix}",
                            job.id,
                            job.elapsed_secs(),
                        );
                    }
                    println!();
                }
                return Some(SlashResult::Handled);
            }

            let (sub, arg) = rest
                .split_once(char::is_whitespace)
                .map(|(s, a)| (s, a.trim()))
                .unwrap_or((rest, ""));

            match sub {
                "show" => match arg.parse::<u32>().ok().and_then(|id| session.jobs.show(id)) {
                    Some(job) => {
                        println!("\n{BLUE}Job #{} — {}{RESET}", job.id, job.status);
                        println!("{DIM}Query:{RESET} {}", job.query);
                        match job.result {
                            Some(result) => println!("\n{}\n", result),
                            None => println!("{DIM}(still running — no result yet){RESET}\n"),
                        }
                    }
                    None => println!("{ERROR}No such job.{RESET}\n"),
                },
                "cancel" => match arg.parse::<u32>().ok() {
                    Some(id) if session.jobs.cancel(id) => {
                        println!("{DIM}Cancelling job #{id}...{RESET}\n");
                    }
                    _ => println!("{ERROR}No such running job.{RESET}\n"),
                },
                _ => println!("{ERROR}Usage:{RESET} /jobs [show <id>|cancel <id>]\n"),
            }
            Some(SlashResult::Handled)
        }

        "/shells" => {
            if rest.is_empty() {
                let jobs = mint_core::bg_shell::list_jobs();
                if jobs.is_empty() {
                    println!(
                        "{DIM}No background shell jobs. The agent starts one when it calls run_shell with background: true.{RESET}\n"
                    );
                } else {
                    println!("\n{BLUE}Background shell jobs{RESET}");
                    for job in &jobs {
                        let status_str = match &job.status {
                            mint_core::bg_shell::JobStatus::Running => {
                                format!("{MINT}running{RESET}")
                            }
                            mint_core::bg_shell::JobStatus::Exited(0) => "exited(0)".to_string(),
                            mint_core::bg_shell::JobStatus::Exited(code) => {
                                format!("{ERROR}exited({code}){RESET}")
                            }
                            mint_core::bg_shell::JobStatus::Killed => {
                                format!("{DIM}killed{RESET}")
                            }
                            mint_core::bg_shell::JobStatus::Failed(err) => {
                                format!("{ERROR}failed: {err}{RESET}")
                            }
                        };
                        let preview: String = job.command.chars().take(50).collect();
                        let suffix = if job.command.chars().count() > 50 {
                            "…"
                        } else {
                            ""
                        };
                        println!(
                            "  {:<6} [{status_str}] {DIM}{:>4}s{RESET}  {preview}{suffix}",
                            job.id, job.elapsed_secs,
                        );
                    }
                    println!();
                }
                return Some(SlashResult::Handled);
            }

            let (sub, arg) = rest
                .split_once(char::is_whitespace)
                .map(|(s, a)| (s, a.trim()))
                .unwrap_or((rest, ""));

            match sub {
                "show" if !arg.is_empty() => match mint_core::bg_shell::snapshot(arg) {
                    Ok(job) => {
                        println!("\n{BLUE}Job {} — {}{RESET}", job.id, job.command);
                        if let Some(pid) = job.pid {
                            println!("{DIM}pid: {pid}  cwd: {}{RESET}", job.cwd.display());
                        }
                        println!("{DIM}stdout:{RESET}\n{}", job.stdout);
                        println!("{DIM}stderr:{RESET}\n{}\n", job.stderr);
                    }
                    Err(_) => println!("{ERROR}No such job.{RESET}\n"),
                },
                "kill" if !arg.is_empty() => match mint_core::bg_shell::kill_job(arg) {
                    Ok(msg) => println!("{DIM}{msg}{RESET}\n"),
                    Err(_) => println!("{ERROR}No such job.{RESET}\n"),
                },
                _ => println!("{ERROR}Usage:{RESET} /shells [show <id>|kill <id>]\n"),
            }
            Some(SlashResult::Handled)
        }

        "/clear" | "/reset" => {
            let options = vec![
                "No (keep history)".to_string(),
                "Yes (clear history)".to_string(),
            ];
            let choice = match prompt_interactive_select(
                "Clear conversation history?",
                &options,
                &options[0],
            ) {
                Ok(Some(sel)) => sel == options[1],
                _ => false,
            };

            if choice {
                if let Ok(memory) = MemoryStore::open_default() {
                    match memory.clear_interactions() {
                        Ok(count) => println!("{DIM}Cleared {count} interactions.{RESET}"),
                        Err(error) => println!("{ERROR}Memory error:{RESET} {error}"),
                    }
                }
                println!("{DIM}Conversation context cleared.{RESET}\n");
            } else {
                println!("{DIM}Cancelled.{RESET}\n");
            }
            Some(SlashResult::Handled)
        }

        "/cd" => {
            if rest.is_empty() {
                println!("{WARN}/cd requires a path{RESET}\n");
            } else {
                let new_dir = PathBuf::from(rest);
                if new_dir.is_dir() {
                    session.current_dir = new_dir.canonicalize().unwrap_or(new_dir);
                    println!(
                        "{DIM}Workspace: {}{RESET}\n",
                        format_path_with_tilde(&session.current_dir)
                    );
                } else {
                    println!("{ERROR}Directory not found:{RESET} {rest}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/veo" => {
            if rest.is_empty() {
                println!(
                    "{WARN}Usage: /veo <prompt> [--aspect <ratio>] [--duration <secs>]{RESET}\n"
                );
            } else {
                let mut prompt = rest.to_string();
                let mut aspect = "16:9".to_string();
                let mut duration = 5;

                // Simple flag parsing
                if let Some(pos) = prompt.find("--aspect") {
                    let rest_str = prompt[pos..].to_string();
                    let mut parts = rest_str.split_whitespace();
                    parts.next(); // Skip --aspect
                    if let Some(val) = parts.next() {
                        aspect = val.to_string();
                    }
                    prompt = prompt[..pos].trim().to_string();
                }
                if let Some(pos) = prompt.find("--duration") {
                    let rest_str = prompt[pos..].to_string();
                    let mut parts = rest_str.split_whitespace();
                    parts.next(); // Skip --duration
                    if let Some(val) = parts.next() {
                        if let Ok(parsed) = val.parse::<u32>() {
                            duration = parsed;
                        }
                    }
                    prompt = prompt[..pos].trim().to_string();
                }

                use indicatif::{ProgressBar, ProgressStyle};
                let spinner = ProgressBar::new_spinner();
                spinner.set_style(
                    ProgressStyle::default_spinner()
                        .tick_chars("⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏")
                        .template("{spinner:.magenta} {msg}")
                        .unwrap(),
                );
                let gen_request = mint_core::VideoGenRequest {
                    prompt: prompt.clone(),
                    negative_prompt: None,
                    aspect_ratio: aspect.clone(),
                    duration,
                    model: None,
                    provider: "veo".to_string(),
                };

                match mint_core::generate_video(&session.config, &gen_request).await {
                    Ok(result) => {
                        spinner.finish_and_clear();
                        if let Some(video) = result.videos.first() {
                            println!("{MINT}✓ Video generated successfully!{RESET}");
                            println!("{MINT}Saved to: {}{RESET}\n", video.path.display());
                        } else {
                            println!("{ERROR}✗ Video generation returned no videos.{RESET}\n");
                        }
                    }
                    Err(e) => {
                        spinner.finish_and_clear();
                        println!("{ERROR}✗ Video generation failed: {e}{RESET}\n");
                    }
                }
            }
            Some(SlashResult::Handled)
        }

        "/image" => {
            let (img_path, prompt) = parse_path_and_prompt(rest);

            if img_path.is_empty() {
                println!("{WARN}/image usage: /image <path> [prompt]{RESET}\n");
                return Some(SlashResult::Handled);
            }
            match image::load_image_as_data_uri(std::path::Path::new(&img_path)) {
                Ok(uri) => {
                    if let Some(ref mut current) = session.pending_image {
                        current.push(' ');
                        current.push_str(&uri);
                    } else {
                        session.pending_image = Some(uri);
                    }
                    if prompt.is_empty() {
                        println!("{DIM}Image attached — type your prompt and press Enter{RESET}\n");
                        Some(SlashResult::Handled)
                    } else {
                        Some(SlashResult::ForwardToAgent(prompt.to_owned()))
                    }
                }
                Err(e) => {
                    println!("{ERROR}Failed to load image:{RESET} {e}\n");
                    Some(SlashResult::Handled)
                }
            }
        }

        "/edit-image" => {
            let (img_path, instruction) = parse_path_and_prompt(rest);

            if img_path.is_empty() || instruction.is_empty() {
                println!("{WARN}Usage: /edit-image <image_path> <editing instruction>{RESET}\n");
                return Some(SlashResult::Handled);
            }
            match image::load_image_as_data_uri(std::path::Path::new(&img_path)) {
                Ok(uri) => {
                    println!(
                        "{DIM}Editing image with prompt: \"{}\"...{RESET}",
                        instruction
                    );
                    let req = mint_core::ImageGenRequest {
                        prompt: instruction.to_owned(),
                        negative_prompt: None,
                        aspect_ratio: None,
                        num_images: Some(1),
                        model: None,
                        provider: None,
                        image_data_uri: Some(uri),
                        mask_data_uri: None,
                        mode: Some("edit".to_owned()),
                    };
                    match mint_core::generate_images(&session.config, &req).await {
                        Ok(resp) => {
                            if let Some(first) = resp.images.first() {
                                println!(
                                    "{DIM}Image edited successfully using {} ({}){RESET}\n",
                                    resp.provider, resp.model
                                );
                                println!(
                                    "{DIM}Image data URI prefix: {}{RESET}\n",
                                    &first.data_uri[..first.data_uri.len().min(60)]
                                );
                            }
                        }
                        Err(e) => {
                            println!("{ERROR}Image editing failed: {e}{RESET}\n");
                        }
                    }
                }
                Err(e) => {
                    println!("{ERROR}Failed to load image: {e}{RESET}\n");
                }
            }
            Some(SlashResult::Handled)
        }

        "/generate-image" | "/gen-image" => {
            let prompt = rest.trim();
            if prompt.is_empty() {
                println!("{WARN}Usage: /generate-image <prompt>{RESET}\n");
                Some(SlashResult::Handled)
            } else {
                Some(SlashResult::ForwardToAgent(format!(
                    "Generate an image of {}",
                    prompt
                )))
            }
        }

        "/paste" => match image::read_clipboard_image() {
            Ok(Some(uri)) => {
                if let Some(ref mut current) = session.pending_image {
                    current.push(' ');
                    current.push_str(&uri);
                } else {
                    session.pending_image = Some(uri);
                }
                if rest.is_empty() {
                    println!(
                        "{DIM}Clipboard image attached — type your prompt and press Enter{RESET}\n"
                    );
                    Some(SlashResult::Handled)
                } else {
                    Some(SlashResult::ForwardToAgent(rest.to_owned()))
                }
            }
            Ok(None) => {
                println!("{WARN}No image found in clipboard.{RESET}\n");
                Some(SlashResult::Handled)
            }
            Err(e) => {
                println!("{ERROR}Clipboard error:{RESET} {e}\n");
                Some(SlashResult::Handled)
            }
        },

        "/learn" => {
            if rest.is_empty() {
                let mut skills = match MemoryStore::open_default() {
                    Ok(m) => match m.learned_skills(100) {
                        Ok(s) => s,
                        Err(e) => {
                            println!("{ERROR}Error loading custom skills: {e}{RESET}\n");
                            return Some(SlashResult::Handled);
                        }
                    },
                    Err(e) => {
                        println!("{ERROR}Memory error: {e}{RESET}\n");
                        return Some(SlashResult::Handled);
                    }
                };

                if let Some(home) = dirs::home_dir() {
                    let global_skills_path = home.join(".config").join("mint").join("mint-skills");
                    mint_core::skills::load_skills_from_dir(&global_skills_path, &mut skills);
                }
                let workspace_skills_path1 = session.current_dir.join(".agents").join("skills");
                mint_core::skills::load_skills_from_dir(&workspace_skills_path1, &mut skills);
                let workspace_skills_path2 = session.current_dir.join("skills");
                mint_core::skills::load_skills_from_dir(&workspace_skills_path2, &mut skills);

                let mut unique_skills = std::collections::BTreeMap::new();
                for skill in skills {
                    let loc = if skill.source_path.contains("/.config/mint/mint-skills") {
                        "Global"
                    } else if skill.source_path.contains("/skills")
                        || skill.source_path.contains("/.agents/skills")
                    {
                        "Workspace"
                    } else {
                        "Taught"
                    };
                    unique_skills.insert(skill.name.clone(), (skill, loc));
                }

                if unique_skills.is_empty() {
                    println!("No learned skills found. Use '/learn <path>' to add one.\n");
                } else {
                    println!("Learned AI Skills:");
                    for (name, (skill, loc)) in &unique_skills {
                        if *loc == "Taught" {
                            println!("  ● [{}] {}", loc, name);
                        } else {
                            println!("  ● [{}] {} (Source: {})", loc, name, skill.source_path);
                        }
                    }
                    println!();
                }
            } else {
                let path = PathBuf::from(rest);
                let path = if path.is_absolute() {
                    path
                } else {
                    session.current_dir.join(path)
                };
                match crate::skills::learn(&path) {
                    Ok(skill) => println!(
                        "{DIM}Learned skill: {} ({}){RESET}\n",
                        skill.name, skill.source_path
                    ),
                    Err(error) => println!("{ERROR}Learn error:{RESET} {error}\n"),
                }
            }
            Some(SlashResult::Handled)
        }

        "/skill" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));

            match subcmd {
                "" | "list" => {
                    let skills = load_all_available_skills(&session.current_dir);
                    if skills.is_empty() {
                        println!(
                            "No skills found. Use `/skill add <path|github-repo|url>` to add one.\n"
                        );
                    } else {
                        println!("Skills:");
                        for skill in &skills {
                            let loc = if skill.source_path.contains("/.config/mint/mint-skills") {
                                "Global"
                            } else if skill.source_path.contains("/skills")
                                || skill.source_path.contains("/.agents/skills")
                            {
                                "Workspace"
                            } else {
                                "Taught"
                            };
                            println!("  ● [{}] {}", loc, skill.name);
                        }
                        println!();
                    }
                }
                "add" | "install" => {
                    if args.is_empty() {
                        println!(
                            "{WARN}/skill add <path> requires a path to a skill file or folder{RESET}\n"
                        );
                    } else {
                        // First whitespace-separated token is the source;
                        // anything after is forwarded to `npx skills` as-is
                        // (e.g. `/skill add owner/repo --skill find-skills`).
                        let mut tokens = args.split_whitespace();
                        let source = tokens.next().unwrap_or("");
                        let extra_args: Vec<&str> = tokens.collect();
                        match crate::skills::add(source, &extra_args, &session.current_dir) {
                            Ok(msg) => println!("{DIM}{msg}{RESET}\n"),
                            Err(msg) => println!("{ERROR}{msg}{RESET}\n"),
                        }
                    }
                }
                _ => {
                    println!(
                        "{WARN}Usage: /skill [list] | /skill add <path> (or /skill install <path>) — <path> \
                         can also be a GitHub repo (owner/repo) or URL, resolved via `npx skills`{RESET}\n"
                    );
                }
            }
            Some(SlashResult::Handled)
        }

        "/memory" => {
            let memory = match MemoryStore::open_default() {
                Ok(m) => m,
                Err(e) => {
                    println!("{ERROR}Memory error:{RESET} {e}\n");
                    return Some(SlashResult::Handled);
                }
            };
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match memory.recent_interactions_for_chat(
                    &mint_core::scoped_chat_id(
                        CHAT_CLI_ID,
                        Some(&session.current_dir.to_string_lossy()),
                    ),
                    10,
                ) {
                    Ok(items) => {
                        if items.is_empty() {
                            println!("{DIM}No interactions yet.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Recent interactions:{RESET}");
                            for item in items.iter().rev() {
                                println!(
                                    "  {DIM}[{}]{RESET} {BLUE}You:{RESET} {}",
                                    &item.created_at[..16.min(item.created_at.len())],
                                    if item.user_text.chars().count() > 80 {
                                        let short: String =
                                            item.user_text.chars().take(80).collect();
                                        format!("{}…", short)
                                    } else {
                                        item.user_text.clone()
                                    }
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "get" => {
                    if args.is_empty() {
                        println!("{WARN}/memory get <key>{RESET}\n");
                    } else {
                        match memory.get_profile(args) {
                            Ok(Some(val)) => println!("{val}\n"),
                            Ok(None) => println!("{DIM}(not set){RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "set" => {
                    let (key, val) = args
                        .split_once(char::is_whitespace)
                        .map(|(k, v)| (k, v.trim()))
                        .unwrap_or((args, ""));
                    if key.is_empty() {
                        println!("{WARN}/memory set <key> <value>{RESET}\n");
                    } else {
                        match memory.set_profile(key, val) {
                            Ok(()) => println!("{DIM}Stored {key}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "skills" => match memory.learned_skills(20) {
                    Ok(skills) => {
                        if skills.is_empty() {
                            println!("{DIM}No learned skills.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Learned skills:{RESET}");
                            for s in &skills {
                                println!("  [{}] {} — {}", s.id, s.name, s.source_path);
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "clear" => match memory.clear_interactions_for_chat(&mint_core::scoped_chat_id(
                    CHAT_CLI_ID,
                    Some(&session.current_dir.to_string_lossy()),
                )) {
                    Ok(count) => println!("{DIM}Cleared {count} interactions.{RESET}\n"),
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "facts" => match memory.list_facts(50) {
                    Ok(facts) => {
                        if facts.is_empty() {
                            println!(
                                "{DIM}No stored facts yet. Add one with /remember <text>.{RESET}\n"
                            );
                        } else {
                            println!("\n{BLUE}Stored facts:{RESET}");
                            for f in &facts {
                                let owner = match &f.agent_id {
                                    Some(name) => format!("via {name}"),
                                    None if f.scope == "project" => "project".to_string(),
                                    None => "global".to_string(),
                                };
                                println!("  {DIM}[{}] ({}){RESET} {}", f.id, owner, f.body);
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "forget" => {
                    if args.is_empty() {
                        println!("{WARN}/memory forget <id-or-text>{RESET}\n");
                    } else {
                        match memory.forget_fact(args) {
                            Ok(0) => println!("{DIM}Nothing matched {args}.{RESET}\n"),
                            Ok(n) => println!("{DIM}Forgot {n} fact(s).{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "promote" => match args.parse::<i64>() {
                    Ok(id) => match memory.promote_fact(id) {
                        Ok(true) => {
                            println!("{DIM}Promoted fact {id} to shared memory.{RESET}\n")
                        }
                        Ok(false) => println!(
                            "{DIM}Fact {id} is not a subagent-scoped fact (nothing to promote).{RESET}\n"
                        ),
                        Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                    },
                    Err(_) => println!("{WARN}/memory promote <id>{RESET}\n"),
                },
                _ => println!(
                    "{WARN}/memory usage: list | facts | forget <id> | promote <id> | clear | get <key> | set <key> <val> | skills{RESET}\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/cron" => {
            let cron_jobs = match mint_core::CronStore::open_default() {
                Ok(store) => store,
                Err(e) => {
                    println!("{ERROR}Cron error:{RESET} {e}\n");
                    return Some(SlashResult::Handled);
                }
            };
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match cron_jobs.list() {
                    Ok(jobs) => {
                        if jobs.is_empty() {
                            println!("{DIM}No cron jobs.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Cron jobs:{RESET}");
                            for job in &jobs {
                                let status = if job.enabled {
                                    format!("{MINT}on{RESET}")
                                } else {
                                    format!("{DIM}off{RESET}")
                                };
                                println!(
                                    "  [{}] {} {DIM}({}){RESET} next: {} {DIM}last: {}{RESET}",
                                    job.id,
                                    job.name,
                                    status,
                                    format_local_time(&job.next_run),
                                    job.last_status.as_deref().unwrap_or("never run")
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "add" if args.trim().is_empty() => {
                    match crate::cron_wizard::run_add_wizard(&cron_jobs, &session.current_dir) {
                        Ok(job) => println!(
                            "\n{DIM}Created cron job {} — next run: {}{RESET}\n",
                            job.id,
                            format_local_time(&job.next_run)
                        ),
                        Err(e) => println!("\n{ERROR}Error:{RESET} {e}\n"),
                    }
                }
                "add" => {
                    // A trailing 4th `|`-separated segment is an optional IANA
                    // timezone name for `schedule` (see `mint cron add
                    // --timezone`) — its presence is judged purely by pipe
                    // count, so `task` can still contain literal `|`
                    // characters as long as no timezone is given.
                    let fields: Vec<&str> = args.splitn(4, '|').map(str::trim).collect();
                    let (name, schedule, task, timezone) = match fields.as_slice() {
                        [name, schedule, task] => (*name, *schedule, *task, None),
                        [name, schedule, task, tz] => (*name, *schedule, *task, Some(*tz)),
                        _ => {
                            println!(
                                "{WARN}/cron add <name> | <schedule> | <task> | [timezone]{RESET}\n{DIM}e.g. /cron add stock report | 0 8 * * * | fetch today's stock prices and summarize{RESET}\n{DIM}e.g. with a local time: /cron add stock report | 0 8 * * * | fetch today's stock prices and summarize | Asia/Bangkok{RESET}\n"
                            );
                            return Some(SlashResult::Handled);
                        }
                    };
                    if name.is_empty() || task.is_empty() {
                        println!(
                            "{WARN}/cron add <name> | <schedule> | <task> | [timezone]{RESET}\n"
                        );
                        return Some(SlashResult::Handled);
                    }
                    let schedule = match timezone {
                        Some(tz) if !tz.is_empty() => {
                            match mint_core::localize_schedule(schedule, tz, chrono::Utc::now()) {
                                Ok(utc_schedule) => utc_schedule,
                                Err(e) => {
                                    println!("{ERROR}Error:{RESET} {e}\n");
                                    return Some(SlashResult::Handled);
                                }
                            }
                        }
                        _ => schedule.to_string(),
                    };
                    match cron_jobs.add(name, schedule, task, session.current_dir.clone()) {
                        Ok(job) => println!(
                            "{DIM}Created cron job {} — next run: {}{RESET}\n",
                            job.id,
                            format_local_time(&job.next_run)
                        ),
                        Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                    }
                }
                "remove" => {
                    if args.is_empty() {
                        println!("{WARN}/cron remove <id>{RESET}\n");
                    } else {
                        match cron_jobs.remove(args) {
                            Ok(true) => println!("{DIM}Removed {args}.{RESET}\n"),
                            Ok(false) => println!("{WARN}No cron job with id {args}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                "enable" | "disable" => {
                    if args.is_empty() {
                        println!("{WARN}/cron {subcmd} <id>{RESET}\n");
                    } else {
                        match cron_jobs.set_enabled(args, subcmd == "enable") {
                            Ok(Some(_)) => println!("{DIM}{subcmd}d {args}.{RESET}\n"),
                            Ok(None) => println!("{WARN}No cron job with id {args}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                _ => println!(
                    "{WARN}/cron usage: list | add <name> | <schedule> | <task> | [timezone] | remove <id> | enable <id> | disable <id>{RESET}\n{DIM}For run-now, use `mint cron run-now <id>` in a terminal.{RESET}\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/link" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => match mint_core::list_linked_folders() {
                    Ok(folders) => {
                        if folders.is_empty() {
                            println!("{DIM}No linked folders.{RESET}\n");
                        } else {
                            println!("\n{BLUE}Linked folders:{RESET}");
                            for folder in folders.values() {
                                println!(
                                    "  {} → {} {DIM}{}{RESET}",
                                    folder.name,
                                    folder.path.display(),
                                    folder.description.as_deref().unwrap_or("")
                                );
                            }
                            println!();
                        }
                    }
                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                },
                "add" => {
                    let fields: Vec<&str> = args.splitn(3, '|').map(str::trim).collect();
                    match fields.as_slice() {
                        [name, path] | [name, path, ""] if !name.is_empty() && !path.is_empty() => {
                            match mint_core::add_linked_folder(name, Path::new(path), None) {
                                Ok(()) => println!("{DIM}Linked folder: {name}{RESET}\n"),
                                Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                            }
                        }
                        [name, path, description] if !name.is_empty() && !path.is_empty() => {
                            match mint_core::add_linked_folder(
                                name,
                                Path::new(path),
                                Some(description.to_string()),
                            ) {
                                Ok(()) => println!("{DIM}Linked folder: {name}{RESET}\n"),
                                Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                            }
                        }
                        _ => println!(
                            "{WARN}/link add <name> | <path> | <description>{RESET}\n{DIM}e.g. /link add Food | ~/notes/food | restaurant reviews and recipes{RESET}\n"
                        ),
                    }
                }
                "remove" => {
                    if args.is_empty() {
                        println!("{WARN}/link remove <name>{RESET}\n");
                    } else {
                        match mint_core::remove_linked_folder(args) {
                            Ok(true) => println!("{DIM}Removed {args}.{RESET}\n"),
                            Ok(false) => println!("{WARN}No linked folder named {args}.{RESET}\n"),
                            Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                        }
                    }
                }
                _ => println!(
                    "{WARN}/link usage: list | add <name> | <path> | <description> | remove <name>{RESET}\n"
                ),
            }
            Some(SlashResult::Handled)
        }

        "/subagent" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            match subcmd {
                "list" | "" => {
                    let subagents = mint_core::list_subagents(Some(&session.current_dir));
                    println!("\n{BLUE}Subagents:{RESET}");
                    for definition in &subagents {
                        let source = if definition.builtin {
                            "built-in".to_string()
                        } else if definition.source_path.contains(".agents/subagents") {
                            "workspace".to_string()
                        } else {
                            "global".to_string()
                        };
                        println!(
                            "  {} {DIM}({}){RESET} — {}",
                            definition.name, source, definition.description
                        );
                    }
                    println!();
                }
                "add" => match crate::subagent_wizard::run_add_wizard(Some(&session.current_dir)) {
                    Ok(definition) => {
                        println!("\n{DIM}Created subagent {}.{RESET}\n", definition.name)
                    }
                    Err(e) => println!("\n{ERROR}Error:{RESET} {e}\n"),
                },
                "remove" => {
                    if args.is_empty() {
                        println!("{WARN}/subagent remove <name>{RESET}\n");
                    } else {
                        match mint_core::find_subagent(args, Some(&session.current_dir)) {
                            Some(definition) if definition.builtin => println!(
                                "{WARN}Can't remove the built-in subagent {args}.{RESET}\n"
                            ),
                            Some(definition) => {
                                match mint_core::delete_subagent(&definition.source_path) {
                                    Ok(()) => println!("{DIM}Removed {args}.{RESET}\n"),
                                    Err(e) => println!("{ERROR}Error:{RESET} {e}\n"),
                                }
                            }
                            None => println!("{WARN}No subagent named {args}.{RESET}\n"),
                        }
                    }
                }
                _ => println!("{WARN}/subagent usage: list | add | remove <name>{RESET}\n"),
            }
            Some(SlashResult::Handled)
        }

        "/mcp" => {
            let (subcmd, args) = rest
                .split_once(char::is_whitespace)
                .map(|(c, a)| (c, a.trim()))
                .unwrap_or((rest, ""));
            handle_mcp_slash(session, subcmd, args);
            Some(SlashResult::Handled)
        }

        "/stats" => {
            let provider = &session.config.ai_provider;
            let model = active_model(provider, &session.config);
            let interactions = MemoryStore::open_default()
                .and_then(|m| {
                    m.recent_interactions_for_chat(
                        &mint_core::scoped_chat_id(
                            CHAT_CLI_ID,
                            Some(&session.current_dir.to_string_lossy()),
                        ),
                        1000,
                    )
                })
                .map(|v| v.len())
                .unwrap_or(0);
            println!("\n{BLUE}─ Session Stats ─────────────────────────{RESET}");
            println!("  Provider : {MINT}{provider}{RESET}");
            println!("  Model    : {model}");
            println!(
                "  Workspace: {}",
                format_path_with_tilde(&session.current_dir)
            );
            println!(
                "  Fast mode: {}",
                if session.fast_mode { "on" } else { "off" }
            );
            println!("  Memory   : {interactions} interactions");
            if let Some(ref img_data) = session.pending_image {
                let count = img_data.split_whitespace().count();
                if count > 1 {
                    println!("  Images   : {WARN}{} images attached{RESET}", count);
                } else {
                    println!("  Image    : {WARN}attached{RESET}");
                }
            }

            println!();
            Some(SlashResult::Handled)
        }

        "/release-notes" => {
            const RELEASE_NOTES: &str = include_str!("../../../../Release_Note.md");
            println!("\n{BLUE}────────────────────────────────────────────{RESET}");
            println!("{}", RELEASE_NOTES.trim());
            println!("{BLUE}────────────────────────────────────────────{RESET}\n");
            Some(SlashResult::Handled)
        }

        "/exit" | "/quit" => Some(SlashResult::Exit),

        "/plugins" | "/plugin" => {
            let (plugin_name, prompt) = rest
                .split_once(char::is_whitespace)
                .map(|(p, r)| (p, r.trim()))
                .unwrap_or((rest, ""));

            if plugin_name.is_empty() {
                // Collect all plugins with their OAuth status
                use crossterm::{
                    event::{self, Event, KeyCode},
                    terminal::{disable_raw_mode, enable_raw_mode},
                };
                use mint_core::oauth::list_oauth_statuses;

                let oauth_statuses = list_oauth_statuses();

                // Build native plugin list with statuses
                struct PluginEntry {
                    name: String,
                    desc: String,
                    is_oauth: bool,
                    oauth_provider: String,
                    connected: bool,
                    account: Option<String>,
                }

                let mut entries: Vec<PluginEntry> = mint_core::native_plugins()
                    .iter()
                    .map(|p| {
                        // Try to find matching OAuth status
                        let (connected, account) = if !p.oauth_provider.is_empty() {
                            if let Some(st) = oauth_statuses
                                .iter()
                                .find(|s| s.provider == p.oauth_provider)
                            {
                                (st.connected, st.account_email.clone())
                            } else {
                                (false, None)
                            }
                        } else {
                            (false, None)
                        };
                        PluginEntry {
                            name: p.name.to_string(),
                            desc: p.description.to_string(),
                            is_oauth: !p.oauth_provider.is_empty(),
                            oauth_provider: p.oauth_provider.to_string(),
                            connected,
                            account,
                        }
                    })
                    .collect();

                // Add custom skills as non-OAuth entries
                let mut custom_entries: Vec<PluginEntry> = Vec::new();
                if let Ok(memory) = MemoryStore::open_default() {
                    if let Ok(skills) = memory.learned_skills(100) {
                        for s in &skills {
                            custom_entries.push(PluginEntry {
                                name: s.name.clone(),
                                desc: format!("Custom skill • {}", s.source_path),
                                is_oauth: false,
                                oauth_provider: String::new(),
                                connected: false,
                                account: None,
                            });
                        }
                    }
                }

                // Flatten into display structs
                #[allow(dead_code)]
                struct PE {
                    name: String,
                    desc: String,
                    is_oauth: bool,
                    oauth_provider: String,
                    connected: bool,
                    account: Option<String>,
                    is_custom: bool,
                }
                let display: Vec<PE> = entries
                    .drain(..)
                    .map(|e| PE {
                        is_custom: false,
                        name: e.name,
                        desc: e.desc,
                        is_oauth: e.is_oauth,
                        oauth_provider: e.oauth_provider,
                        connected: e.connected,
                        account: e.account,
                    })
                    .chain(custom_entries.drain(..).map(|e| PE {
                        is_custom: true,
                        name: e.name,
                        desc: e.desc,
                        is_oauth: e.is_oauth,
                        oauth_provider: e.oauth_provider,
                        connected: e.connected,
                        account: e.account,
                    }))
                    .collect();

                // Print header + list (separator printed inside draw_list so line count stays accurate)

                // Simple interactive list
                let mut cursor = 0usize;
                let total = display.len();
                if total == 0 {
                    println!("{MINT}  Available Plugins & Skills{RESET}");
                    println!("{BLUE}────────────────────────────────────────────{RESET}");
                    println!("  {DIM}(No plugins found.){RESET}\n");
                    return Some(SlashResult::Handled);
                }

                // Truncate helper to ensure line items don't wrap and break terminal line counts
                fn truncate_str(s: &str, max_len: usize) -> String {
                    if s.chars().count() > max_len {
                        let end: String = s.chars().take(max_len.saturating_sub(3)).collect();
                        format!("{end}...")
                    } else {
                        s.to_string()
                    }
                }

                // Count exact terminal lines printed by draw_list
                fn count_menu_lines(display: &[PE]) -> usize {
                    display.len() // one line per item only
                }

                // Draw list — same clean style as /models (❯ selected, dimmed others, no section headers)
                let draw_list = |cur: usize| {
                    for (i, e) in display.iter().enumerate() {
                        let plain_status = if e.connected {
                            format!(
                                "● {}",
                                truncate_str(e.account.as_deref().unwrap_or("yes"), 18)
                            )
                        } else if e.is_oauth {
                            "○ Not Connected".to_string()
                        } else {
                            "■ Active".to_string()
                        };
                        let short_desc = truncate_str(&e.desc, 38);
                        if i == cur {
                            let status_colored = if e.connected {
                                format!("\x1b[32m{:<22}\x1b[0m", plain_status)
                            } else if e.is_oauth {
                                format!("{DIM}{:<22}{RESET}", plain_status)
                            } else {
                                format!("\x1b[36m{:<22}\x1b[0m", plain_status)
                            };
                            println!(
                                "  {BLUE}❯ {:<18}{RESET}{}  {DIM}{short_desc}{RESET}",
                                e.name, status_colored
                            );
                        } else {
                            println!(
                                "    {DIM}{:<18}  {:<22}  {short_desc}{RESET}",
                                e.name, plain_status
                            );
                        }
                    }
                };

                let total_lines = count_menu_lines(&display);
                // Title pinned above list — printed once, not cleared on redraw (matches /models style)
                println!(
                    "  {BLUE}Plugins & Integrations (↑/↓ navigate, Enter: manage, q: exit):{RESET}"
                );
                draw_list(cursor);
                let _ = enable_raw_mode();

                let selected_idx = loop {
                    match event::poll(std::time::Duration::from_millis(100)) {
                        Ok(true) => {
                            if let Ok(Event::Key(key_event)) = event::read() {
                                if key_event.kind == event::KeyEventKind::Press {
                                    let is_ctrl_c = matches!(key_event.code, KeyCode::Char('c'))
                                        && key_event
                                            .modifiers
                                            .contains(crossterm::event::KeyModifiers::CONTROL);
                                    if is_ctrl_c
                                        || matches!(
                                            key_event.code,
                                            KeyCode::Char('q') | KeyCode::Esc
                                        )
                                    {
                                        let _ = disable_raw_mode();
                                        break None;
                                    }
                                    match key_event.code {
                                        KeyCode::Up => {
                                            if cursor > 0 {
                                                cursor -= 1;
                                            } else {
                                                cursor = total - 1;
                                            }
                                            let _ = disable_raw_mode();
                                            print!("\x1b[{}A\x1b[0J", total_lines);
                                            let _ = std::io::Write::flush(&mut std::io::stdout());
                                            draw_list(cursor);
                                            let _ = enable_raw_mode();
                                        }
                                        KeyCode::Down => {
                                            if cursor < total - 1 {
                                                cursor += 1;
                                            } else {
                                                cursor = 0;
                                            }
                                            let _ = disable_raw_mode();
                                            print!("\x1b[{}A\x1b[0J", total_lines);
                                            let _ = std::io::Write::flush(&mut std::io::stdout());
                                            draw_list(cursor);
                                            let _ = enable_raw_mode();
                                        }
                                        KeyCode::Enter => {
                                            let _ = disable_raw_mode();
                                            break Some(cursor);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                        Ok(false) => {}
                        Err(_) => {
                            let _ = disable_raw_mode();
                            break None;
                        }
                    }
                };

                if let Some(idx) = selected_idx {
                    let selected = &display[idx];
                    println!();
                    if selected.is_custom {
                        println!("{BLUE}📄 Custom Skill: {}\x1b[0m", selected.name);
                        println!("{DIM}  Source: {}{RESET}", selected.desc);
                    } else {
                        println!("{DIM}  {}{RESET}\n", selected.desc);
                        let enabled =
                            mint_core::native_plugin_enabled(&session.config, &selected.name);
                        let toggle = if enabled {
                            "Disable plugin"
                        } else {
                            "Enable plugin"
                        };
                        let mut actions = vec![toggle.to_string()];
                        if selected.is_oauth {
                            actions.push(if selected.connected {
                                "Disconnect (OAuth)".to_string()
                            } else {
                                "Sign in (OAuth)".to_string()
                            });
                            actions.push("Configure credentials".to_string());
                        }
                        actions.push("Back".to_string());

                        match prompt_interactive_select(
                            &format!("Plugin: {}", selected.name),
                            &actions,
                            &actions[0],
                        ) {
                            Ok(Some(choice)) if choice.as_str() == toggle => {
                                match crate::plugins_cli::set_native_plugin_enabled(
                                    &selected.name,
                                    !enabled,
                                ) {
                                    Ok(()) => {
                                        println!(
                                            "{MINT}{} '{}'.{RESET}",
                                            if enabled { "Disabled" } else { "Enabled" },
                                            selected.name
                                        );
                                        reload_session_config(session);
                                    }
                                    Err(e) => println!("{ERROR}Plugin error:{RESET} {e}"),
                                }
                            }
                            Ok(Some(choice)) if choice.starts_with("Sign in") => {
                                println!("{DIM}  Opening browser for authorization...{RESET}\n");
                                crate::plugins_cli::login_plugin_oauth_public(
                                    &selected.oauth_provider,
                                )
                                .await;
                            }
                            Ok(Some(choice)) if choice.starts_with("Disconnect") => {
                                if let Err(e) = crate::plugins_cli::logout_plugin_oauth_public(
                                    &selected.oauth_provider,
                                ) {
                                    println!("{ERROR}{e}{RESET}");
                                }
                            }
                            Ok(Some(choice)) if choice == "Configure credentials" => {
                                let key = if selected.name == "google_calendar" {
                                    "calendar"
                                } else {
                                    selected.name.as_str()
                                };
                                let _ = crate::plugins_cli::configure_plugin_fields_public(key);
                                reload_session_config(session);
                            }
                            _ => {}
                        }
                    }
                    println!();
                }

                Some(SlashResult::Handled)
            } else if matches!(plugin_name, "enable" | "disable") {
                let target = prompt.trim();
                if target.is_empty() {
                    println!("{WARN}/plugin {plugin_name} usage:{RESET} <name>\n");
                } else if !mint_core::native_plugins().iter().any(|p| p.name == target) {
                    println!(
                        "{WARN}Unknown native plugin '{target}'.{RESET} Known: {}\n",
                        mint_core::native_plugins()
                            .iter()
                            .map(|p| p.name)
                            .collect::<Vec<_>>()
                            .join(", ")
                    );
                } else {
                    let enable = plugin_name == "enable";
                    match crate::plugins_cli::set_native_plugin_enabled(target, enable) {
                        Ok(()) => {
                            println!(
                                "{MINT}{} native plugin '{target}'.{RESET}\n",
                                if enable { "Enabled" } else { "Disabled" }
                            );
                            reload_session_config(session);
                        }
                        Err(e) => println!("{ERROR}Plugin error:{RESET} {e}\n"),
                    }
                }
                Some(SlashResult::Handled)
            } else {
                let agent_prompt = if prompt.is_empty() {
                    format!(
                        "Create a skill markdown file for the plugin '{}' at the path 'skills/{}.md' in the workspace. Write the appropriate instructions inside.",
                        plugin_name, plugin_name
                    )
                } else {
                    format!(
                        "Create a skill markdown file for the plugin '{}' at the path 'skills/{}.md' in the workspace based on these instructions: {}",
                        plugin_name, plugin_name, prompt
                    )
                };
                Some(SlashResult::ForwardToAgent(agent_prompt))
            }
        }

        "/code" => {
            if rest.is_empty() {
                println!("{WARN}/code requires a task description{RESET}\n");
                Some(SlashResult::Handled)
            } else {
                Some(SlashResult::ForwardToAgent(format!("[code] {rest}")))
            }
        }

        "/n8n" => {
            const N8N_URL: &str = "http://localhost:5678";
            let n8n_connected = crate::mcp::list()
                .map(|servers| servers.contains_key("n8n"))
                .unwrap_or(false);
            if rest.is_empty() {
                if !is_reachable("127.0.0.1", 5678) {
                    render_companion_status_panel();
                    return Some(SlashResult::Handled);
                }
                println!("{MINT}Opening n8n...{RESET}");
                println!("{DIM}{N8N_URL}{RESET}");
                if let Err(e) = crate::actions::open_system_handler(N8N_URL) {
                    println!("{ERROR}Couldn't open a browser automatically:{RESET} {e}");
                }
                if !n8n_connected {
                    println!(
                        "{DIM}Tip: 'n8n' isn't wired up as an MCP server yet, so /n8n <task> won't work until you enable MCP in n8n's Settings and connect it (full steps: docs/N8N_INTEGRATION.md){RESET}"
                    );
                }
                println!();
                Some(SlashResult::Handled)
            } else if !n8n_connected {
                render_companion_status_panel();
                Some(SlashResult::Handled)
            } else {
                Some(SlashResult::ForwardToAgent(format!(
                    "[n8n] Use the n8n MCP server's tools to accomplish this: {rest}"
                )))
            }
        }

        "/notebook" => {
            const SURFSENSE_URL: &str = "http://localhost:3929";
            let surfsense_connected = crate::mcp::list()
                .map(|servers| servers.contains_key("surfsense"))
                .unwrap_or(false);
            if rest.is_empty() {
                if !is_reachable("127.0.0.1", 3929) {
                    render_companion_status_panel();
                    return Some(SlashResult::Handled);
                }
                println!("{MINT}Opening SurfSense...{RESET}");
                println!("{DIM}{SURFSENSE_URL}{RESET}");
                if let Err(e) = crate::actions::open_system_handler(SURFSENSE_URL) {
                    println!("{ERROR}Couldn't open a browser automatically:{RESET} {e}");
                }
                if !surfsense_connected {
                    println!(
                        "{DIM}Tip: 'surfsense' isn't wired up as an MCP server yet, so /notebook <task> won't work until you run: mint mcp add surfsense uv --args --directory --args <path to your SurfSense clone>/surfsense_mcp --args run --args mcp_server --env SURFSENSE_API_KEY=<your key> (full steps: docs/SURFSENSE_INTEGRATION.md){RESET}"
                    );
                }
                println!();
                Some(SlashResult::Handled)
            } else if !surfsense_connected {
                render_companion_status_panel();
                Some(SlashResult::Handled)
            } else {
                Some(SlashResult::ForwardToAgent(format!(
                    "[notebook] Use the surfsense MCP server's tools to accomplish this: {rest}"
                )))
            }
        }

        _ => {
            // Unknown slash command — treat as normal message to the agent
            None
        }
    }
}

// ── /mcp interactive handling ────────────────────────────────────────────────

/// Pull the just-persisted config back into the session so subsequent turns
/// (and the picker's own status labels) see the change.
fn reload_session_config(session: &mut InteractiveSession) {
    if let Ok(config) = mint_core::load_config() {
        session.config = config;
    }
}

fn handle_mcp_slash(session: &mut InteractiveSession, subcmd: &str, args: &str) {
    match subcmd {
        "list" | "" => mcp_interactive_picker(session),

        "add" => {
            let mut parts = args.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(name), Some(command)) => {
                    let arg_list: Vec<String> = parts.map(str::to_string).collect();
                    match crate::mcp::add(name, command, arg_list, vec![]) {
                        Ok(()) => {
                            println!("{MINT}Added MCP server '{name}'.{RESET}\n");
                            reload_session_config(session);
                        }
                        Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                    }
                }
                _ => println!("{WARN}/mcp add usage:{RESET} <name> <command> [args...]\n"),
            }
        }

        "remove" if !args.is_empty() => match crate::mcp::remove(args) {
            Ok(true) => {
                println!("{MINT}Removed '{args}'.{RESET}\n");
                reload_session_config(session);
            }
            Ok(false) => println!("{WARN}No MCP server named '{args}'.{RESET}\n"),
            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
        },

        "clear" => match crate::mcp::clear() {
            Ok(()) => {
                println!("{MINT}Removed all MCP servers.{RESET}\n");
                reload_session_config(session);
            }
            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
        },

        "enable" | "disable" if !args.is_empty() => {
            let disable = subcmd == "disable";
            match crate::mcp::set_disabled(args, disable) {
                Ok(true) => {
                    println!(
                        "{MINT}{} '{args}'.{RESET}\n",
                        if disable { "Disabled" } else { "Enabled" }
                    );
                    reload_session_config(session);
                }
                Ok(false) => println!("{WARN}No MCP server named '{args}'.{RESET}\n"),
                Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
            }
        }

        "edit" => {
            let mut parts = args.splitn(3, char::is_whitespace);
            match (parts.next(), parts.next()) {
                (Some(name), Some(field)) => {
                    let value = parts.next().unwrap_or("").trim();
                    let result = match field.to_ascii_lowercase().as_str() {
                        "command" if !value.is_empty() => {
                            crate::mcp::edit(name, Some(value.to_string()), None, None, None)
                        }
                        "command" => {
                            println!(
                                "{WARN}/mcp edit <name> command <value> needs a value{RESET}\n"
                            );
                            return;
                        }
                        "args" => crate::mcp::edit(
                            name,
                            None,
                            Some(value.split_whitespace().map(str::to_string).collect()),
                            None,
                            None,
                        ),
                        "icon" => crate::mcp::edit(
                            name,
                            None,
                            None,
                            None,
                            Some((!value.is_empty()).then(|| value.to_string())),
                        ),
                        _ => {
                            println!("{WARN}Editable fields:{RESET} command | args | icon\n");
                            return;
                        }
                    };
                    match result {
                        Ok(true) => {
                            println!("{MINT}Updated '{name}'.{RESET}\n");
                            reload_session_config(session);
                        }
                        Ok(false) => println!("{WARN}No MCP server named '{name}'.{RESET}\n"),
                        Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                    }
                }
                _ => println!("{WARN}/mcp edit usage:{RESET} <name> command|args|icon <value>\n"),
            }
        }

        "allow" | "disallow" => {
            let mut parts = args.split_whitespace();
            match (parts.next(), parts.next()) {
                (Some(server), Some(tool)) => {
                    let (verb, result) = if subcmd == "allow" {
                        ("Allowed", crate::mcp::allow(server, tool))
                    } else {
                        ("Removed", crate::mcp::disallow(server, tool))
                    };
                    match result {
                        Ok(true) => {
                            println!("{MINT}{verb} {server}/{tool}.{RESET}\n");
                            reload_session_config(session);
                        }
                        Ok(false) => println!("{DIM}No change for {server}/{tool}.{RESET}\n"),
                        Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                    }
                }
                _ => println!("{WARN}/mcp {subcmd} usage:{RESET} <server> <tool>\n"),
            }
        }

        "reauth" if !args.is_empty() => {
            println!(
                "{DIM}Re-authenticating MCP server '{args}'... (a browser tab may open){RESET}\n"
            );
            match crate::mcp::reauth(args) {
                Ok(true) => println!("{MINT}Re-authentication succeeded for '{args}'.{RESET}\n"),
                Ok(false) => println!(
                    "{ERROR}Re-authentication failed for '{args}' (see output above).{RESET}\n"
                ),
                Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
            }
        }

        _ => println!(
            "{WARN}/mcp usage:{RESET} list | add <name> <cmd> [args...] | remove <name> | \
             enable|disable <name> | edit <name> command|args|icon <value> | \
             allow|disallow <server> <tool> | reauth <server> | clear\n"
        ),
    }
}

/// The arrow-key `/mcp` flow: an "add" row plus every configured server; picking
/// a server opens a per-server action menu.
fn mcp_interactive_picker(session: &mut InteractiveSession) {
    let servers = match crate::mcp::list() {
        Ok(servers) => servers,
        Err(e) => {
            println!("{ERROR}MCP error:{RESET} {e}\n");
            return;
        }
    };

    const ADD_ROW: &str = "＋ Add MCP server";
    const CANCEL: &str = "Cancel";

    let allowed = session
        .config
        .extra
        .get("allowedMcpTools")
        .and_then(|v| v.as_object())
        .cloned();

    let mut choices = vec![ADD_ROW.to_string()];
    let mut names = Vec::new();
    let width = servers.keys().map(|k| k.len()).max().unwrap_or(6) + 2;
    for (name, srv) in &servers {
        let tools = allowed
            .as_ref()
            .and_then(|m| m.get(name))
            .and_then(|v| v.as_array());
        let label = match tools {
            Some(a) if a.iter().any(|t| t.as_str() == Some("*")) => {
                format!("{MINT}[tools: *]{RESET}")
            }
            Some(a) if !a.is_empty() => format!(
                "{MINT}[tools: {}]{RESET}",
                a.iter()
                    .filter_map(|t| t.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => format!("{DIM}[no tools allowed]{RESET}"),
        };
        let disabled = if srv.disabled {
            format!(" {DIM}(disabled){RESET}")
        } else {
            String::new()
        };
        choices.push(format!(
            "{name:<width$}{label}{disabled} {DIM}({} {}){RESET}",
            srv.command,
            srv.args.join(" ")
        ));
        names.push(name.clone());
    }
    choices.push(CANCEL.to_string());

    let Ok(Some(pick)) = prompt_interactive_select("MCP Servers", &choices, ADD_ROW) else {
        return;
    };
    if pick == CANCEL {
        return;
    }
    if pick == ADD_ROW {
        mcp_add_flow(session);
        return;
    }
    let Some(pos) = choices.iter().position(|c| *c == pick) else {
        return;
    };
    let name = names[pos - 1].clone();
    let disabled = servers.get(&name).map(|s| s.disabled).unwrap_or(false);
    mcp_server_actions(session, &name, disabled);
}

fn mcp_add_flow(session: &mut InteractiveSession) {
    let name = crate::onboard::prompt_input("Server name", None).unwrap_or_default();
    let name = name.trim().to_string();
    if name.is_empty() {
        println!("{WARN}Cancelled (no name).{RESET}\n");
        return;
    }
    let command = crate::onboard::prompt_input("Command (e.g. npx)", None).unwrap_or_default();
    let command = command.trim().to_string();
    if command.is_empty() {
        println!("{WARN}Cancelled (no command).{RESET}\n");
        return;
    }
    let args = crate::onboard::prompt_input("Arguments (space-separated, blank for none)", None)
        .unwrap_or_default();
    let env_raw = crate::onboard::prompt_input(
        "Env vars (KEY=VALUE, comma-separated, blank for none)",
        None,
    )
    .unwrap_or_default();

    let args: Vec<String> = args.split_whitespace().map(str::to_string).collect();
    let env: Vec<String> = env_raw
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect();

    match crate::mcp::add(&name, &command, args, env) {
        Ok(()) => {
            println!("{MINT}Added MCP server '{name}'.{RESET}\n");
            reload_session_config(session);
        }
        Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
    }
}

fn mcp_server_actions(session: &mut InteractiveSession, name: &str, disabled: bool) {
    let toggle_label = if disabled {
        "Enable server"
    } else {
        "Disable server"
    };
    let actions = vec![
        "Check connection".to_string(),
        "Allow all tools (*)".to_string(),
        "Allow a specific tool".to_string(),
        "Disallow a tool".to_string(),
        "Re-authenticate (OAuth)".to_string(),
        toggle_label.to_string(),
        "Edit (command / args / icon)".to_string(),
        "Remove server".to_string(),
        "Back".to_string(),
    ];
    let Ok(Some(choice)) =
        prompt_interactive_select(&format!("MCP: {name}"), &actions, &actions[0])
    else {
        return;
    };

    match choice.as_str() {
        "Check connection" => match mint_core::list_server_tools(&session.config, name) {
            Ok(_) => println!("{MINT}● Connected{RESET}\n"),
            Err(e) => println!("{ERROR}● Not connected:{RESET} {e}\n"),
        },
        "Allow all tools (*)" => match crate::mcp::allow(name, "*") {
            Ok(_) => {
                println!("{MINT}Allowed all tools for '{name}'.{RESET}\n");
                reload_session_config(session);
            }
            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
        },
        "Allow a specific tool" => {
            let tool = crate::onboard::prompt_input("Tool name", None).unwrap_or_default();
            let tool = tool.trim();
            if tool.is_empty() {
                return;
            }
            match crate::mcp::allow(name, tool) {
                Ok(true) => {
                    println!("{MINT}Allowed {name}/{tool}.{RESET}\n");
                    reload_session_config(session);
                }
                Ok(false) => println!("{DIM}{name}/{tool} was already allowed.{RESET}\n"),
                Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
            }
        }
        "Disallow a tool" => {
            let tool = crate::onboard::prompt_input("Tool to remove (* to clear all)", None)
                .unwrap_or_default();
            let tool = tool.trim();
            if tool.is_empty() {
                return;
            }
            match crate::mcp::disallow(name, tool) {
                Ok(true) => {
                    println!("{MINT}Removed {name}/{tool} from the allowlist.{RESET}\n");
                    reload_session_config(session);
                }
                Ok(false) => println!("{DIM}{name}/{tool} was not in the allowlist.{RESET}\n"),
                Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
            }
        }
        "Re-authenticate (OAuth)" => {
            println!("{DIM}Re-authenticating '{name}'... (a browser tab may open){RESET}\n");
            match crate::mcp::reauth(name) {
                Ok(true) => println!("{MINT}Re-authentication succeeded.{RESET}\n"),
                Ok(false) => {
                    println!("{ERROR}Re-authentication failed (see output above).{RESET}\n")
                }
                Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
            }
        }
        other if other == toggle_label => match crate::mcp::set_disabled(name, !disabled) {
            Ok(_) => {
                println!(
                    "{MINT}{} '{name}'.{RESET}\n",
                    if disabled { "Enabled" } else { "Disabled" }
                );
                reload_session_config(session);
            }
            Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
        },
        "Edit (command / args / icon)" => mcp_edit_flow(session, name),
        "Remove server" => {
            if confirm(&format!("Remove MCP server '{name}'?")).unwrap_or(false) {
                match crate::mcp::remove(name) {
                    Ok(_) => {
                        println!("{MINT}Removed '{name}'.{RESET}\n");
                        reload_session_config(session);
                    }
                    Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
                }
            }
        }
        _ => {}
    }
}

fn mcp_edit_flow(session: &mut InteractiveSession, name: &str) {
    let Some(srv) = crate::mcp::list().ok().and_then(|m| m.get(name).cloned()) else {
        println!("{WARN}No MCP server named '{name}'.{RESET}\n");
        return;
    };
    let fields = vec![
        "command".to_string(),
        "args".to_string(),
        "icon".to_string(),
        "Back".to_string(),
    ];
    let Ok(Some(field)) = prompt_interactive_select(&format!("Edit {name}"), &fields, &fields[0])
    else {
        return;
    };
    let result = match field.as_str() {
        "command" => {
            let value =
                crate::onboard::prompt_input("Command", Some(&srv.command)).unwrap_or_default();
            let value = value.trim();
            if value.is_empty() {
                return;
            }
            crate::mcp::edit(name, Some(value.to_string()), None, None, None)
        }
        "args" => {
            let current = srv.args.join(" ");
            let value = crate::onboard::prompt_input("Arguments (space-separated)", Some(&current))
                .unwrap_or_default();
            crate::mcp::edit(
                name,
                None,
                Some(value.split_whitespace().map(str::to_string).collect()),
                None,
                None,
            )
        }
        "icon" => {
            let current = srv.icon.clone().unwrap_or_default();
            let value = crate::onboard::prompt_input("Icon (blank to clear)", Some(&current))
                .unwrap_or_default();
            let value = value.trim();
            crate::mcp::edit(
                name,
                None,
                None,
                None,
                Some((!value.is_empty()).then(|| value.to_string())),
            )
        }
        _ => return,
    };
    match result {
        Ok(_) => {
            println!("{MINT}Updated '{name}'.{RESET}\n");
            reload_session_config(session);
        }
        Err(e) => println!("{ERROR}MCP error:{RESET} {e}\n"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Commands that are dispatched but deliberately left undocumented —
    /// shortcuts for another command's own token, not gaps in `SLASH_COMMANDS`.
    const UNDOCUMENTED_ALIASES: &[&str] = &["/quit", "/reset", "/plugin", "/searchProvider"];

    /// Regression guard for the exact bug that motivated `SLASH_COMMANDS`:
    /// `/edit-image`, `/gen-image`, `/shells`, and `/subagent` all worked but
    /// were missing from `/help` (and briefly `/avatar` was missing from the
    /// autocomplete list) because command metadata used to live in two
    /// separate hand-maintained arrays with nothing tying them to the
    /// dispatcher below. This parses `handle_slash_command`'s own source for
    /// every top-level `"/xxx" => ` (and `"/xxx" | "/yyy" => `) match arm and
    /// asserts each token is documented in `SLASH_COMMANDS` — so a new arm
    /// added without a table entry fails `cargo test` instead of silently
    /// shipping undocumented.
    #[test]
    fn dispatcher_tokens_are_documented() {
        let source = include_str!("slash_commands.rs");
        let arm_re = regex::Regex::new(r#"(?m)^ {8}((?:"/[a-zA-Z-]+"\s*\|?\s*)+)=>"#).unwrap();
        let token_re = regex::Regex::new(r#""(/[a-zA-Z-]+)""#).unwrap();

        let mut dispatcher_tokens = std::collections::BTreeSet::new();
        for arm in arm_re.captures_iter(source) {
            for tok in token_re.captures_iter(&arm[1]) {
                dispatcher_tokens.insert(tok[1].to_string());
            }
        }

        // Sanity check the extraction itself isn't silently matching nothing
        // (e.g. after a reformat that changes indentation).
        assert!(
            dispatcher_tokens.len() > 20,
            "expected to extract dozens of dispatcher tokens, got {}: {:?} — \
             did the match arm indentation or format change?",
            dispatcher_tokens.len(),
            dispatcher_tokens,
        );

        let documented: std::collections::BTreeSet<&str> =
            SLASH_COMMANDS.iter().map(|s| s.token.as_str()).collect();

        let undocumented: Vec<&String> = dispatcher_tokens
            .iter()
            .filter(|t| !UNDOCUMENTED_ALIASES.contains(&t.as_str()))
            .filter(|t| !documented.contains(t.as_str()))
            .collect();

        assert!(
            undocumented.is_empty(),
            "dispatcher handles {undocumented:?} but SLASH_COMMANDS (in commands.rs) has no \
             entry for it — add one, or add it to UNDOCUMENTED_ALIASES if it's a deliberately \
             undocumented shortcut for another command's token.",
        );
    }
}

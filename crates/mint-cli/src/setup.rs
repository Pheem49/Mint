use anyhow::Result;
use crossterm::event::{self, Event, KeyCode};
use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
use mint_core::{load_config, native_plugins, save_config};
use std::io::{self, Write};

struct ToolOption {
    name: &'static str,
    key: &'static str,
    enabled: bool,
}

pub async fn run() -> Result<Option<String>> {
    let mut config = load_config()?;

    let mut options = vec![
        ToolOption {
            name: "list_files (List Workspace Files)",
            key: "list_files",
            enabled: !config.disabled_tools.contains(&"list_files".to_string()),
        },
        ToolOption {
            name: "read_file (Read File Content)",
            key: "read_file",
            enabled: !config.disabled_tools.contains(&"read_file".to_string()),
        },
        ToolOption {
            name: "search_code (Search Code Text)",
            key: "search_code",
            enabled: !config.disabled_tools.contains(&"search_code".to_string()),
        },
        ToolOption {
            name: "symbols (Index/Search Symbols)",
            key: "symbols",
            enabled: !config.disabled_tools.contains(&"symbols".to_string()),
        },
        ToolOption {
            name: "semantic_index (Semantic Indexing)",
            key: "semantic_index",
            enabled: !config
                .disabled_tools
                .contains(&"semantic_index".to_string()),
        },
        ToolOption {
            name: "semantic_search (Semantic Search)",
            key: "semantic_search",
            enabled: !config
                .disabled_tools
                .contains(&"semantic_search".to_string()),
        },
        ToolOption {
            name: "knowledge_search (Search Local Knowledge)",
            key: "knowledge_search",
            enabled: !config
                .disabled_tools
                .contains(&"knowledge_search".to_string()),
        },
        ToolOption {
            name: "web_search (Search Web)",
            key: "web_search",
            enabled: !config.disabled_tools.contains(&"web_search".to_string()),
        },
        ToolOption {
            name: "memory_recall (Recall Long-term Memory)",
            key: "memory_recall",
            enabled: !config.disabled_tools.contains(&"memory_recall".to_string()),
        },
        ToolOption {
            name: "git_status (Read Git Status)",
            key: "git_status",
            enabled: !config.disabled_tools.contains(&"git_status".to_string()),
        },
        ToolOption {
            name: "git_diff (Read Git Diff)",
            key: "git_diff",
            enabled: !config.disabled_tools.contains(&"git_diff".to_string()),
        },
        ToolOption {
            name: "git_log (Read Git Log)",
            key: "git_log",
            enabled: !config.disabled_tools.contains(&"git_log".to_string()),
        },
        ToolOption {
            name: "git_branch (Read Git Branch)",
            key: "git_branch",
            enabled: !config.disabled_tools.contains(&"git_branch".to_string()),
        },
        ToolOption {
            name: "create_plan (Create Task Plan)",
            key: "create_plan",
            enabled: !config.disabled_tools.contains(&"create_plan".to_string()),
        },
        ToolOption {
            name: "update_plan (Update Task Plan)",
            key: "update_plan",
            enabled: !config.disabled_tools.contains(&"update_plan".to_string()),
        },
        ToolOption {
            name: "request_user_approval (Request User Approval)",
            key: "request_user_approval",
            enabled: !config
                .disabled_tools
                .contains(&"request_user_approval".to_string()),
        },
        ToolOption {
            name: "ask_user (Ask User)",
            key: "ask_user",
            enabled: !config.disabled_tools.contains(&"ask_user".to_string()),
        },
        ToolOption {
            name: "detect_project (Detect Project Type)",
            key: "detect_project",
            enabled: !config
                .disabled_tools
                .contains(&"detect_project".to_string()),
        },
        ToolOption {
            name: "list_tests (List Tests)",
            key: "list_tests",
            enabled: !config.disabled_tools.contains(&"list_tests".to_string()),
        },
        ToolOption {
            name: "read_diagnostics (Read Diagnostics)",
            key: "read_diagnostics",
            enabled: !config
                .disabled_tools
                .contains(&"read_diagnostics".to_string()),
        },
        ToolOption {
            name: "view_image (View Image)",
            key: "view_image",
            enabled: !config.disabled_tools.contains(&"view_image".to_string()),
        },
        ToolOption {
            name: "note_write (Write Notes)",
            key: "note_write",
            enabled: !config.disabled_tools.contains(&"note_write".to_string()),
        },
        ToolOption {
            name: "run_plugin (Run Native Plugins)",
            key: "run_plugin",
            enabled: !config.disabled_tools.contains(&"run_plugin".to_string()),
        },
        ToolOption {
            name: "mcp_tool (Call MCP Tools)",
            key: "mcp_tool",
            enabled: !config.disabled_tools.contains(&"mcp_tool".to_string()),
        },
        ToolOption {
            name: "run_shell (Run Shell Commands)",
            key: "run_shell",
            enabled: !config.disabled_tools.contains(&"run_shell".to_string()),
        },
        ToolOption {
            name: "verify (Run Verification Checks)",
            key: "verify",
            enabled: !config.disabled_tools.contains(&"verify".to_string()),
        },
        ToolOption {
            name: "apply_patch (Patch Files)",
            key: "apply_patch",
            enabled: !config.disabled_tools.contains(&"apply_patch".to_string()),
        },
        ToolOption {
            name: "write_file (Write Files)",
            key: "write_file",
            enabled: !config.disabled_tools.contains(&"write_file".to_string()),
        },
    ];

    let mut cursor = 0;
    redraw_phase_1(&options, cursor);
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
                            println!("\n\x1b[31mSetup cancelled.\x1b[0m");
                            return Ok(None);
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if cursor > 0 {
                                    cursor -= 1;
                                } else {
                                    cursor = options.len() - 1;
                                }
                                disable_raw_mode()?;
                                redraw_phase_1(&options, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Down => {
                                if cursor < options.len() - 1 {
                                    cursor += 1;
                                } else {
                                    cursor = 0;
                                }
                                disable_raw_mode()?;
                                redraw_phase_1(&options, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char(' ') => {
                                options[cursor].enabled = !options[cursor].enabled;
                                disable_raw_mode()?;
                                redraw_phase_1(&options, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char('a') => {
                                for opt in &mut options {
                                    opt.enabled = true;
                                }
                                disable_raw_mode()?;
                                redraw_phase_1(&options, cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char('i') => {
                                for opt in &mut options {
                                    opt.enabled = !opt.enabled;
                                }
                                disable_raw_mode()?;
                                redraw_phase_1(&options, cursor);
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

    let disabled: Vec<String> = options
        .iter()
        .filter(|o| !o.enabled)
        .map(|o| o.key.to_string())
        .collect();

    config.disabled_tools = disabled;
    save_config(&config)?;

    println!("\x1b[32mSuccessfully updated agent tool configurations!\x1b[0m\n");

    let all_native = native_plugins();
    let allowed_plugins: std::collections::HashSet<String> = config
        .extra
        .get("allowedNativePlugins")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_else(|| {
            vec![
                "dev_tools".to_string(),
                "system_metrics".to_string(),
                "github".to_string(),
            ]
            .into_iter()
            .collect()
        });

    let mut native_options: Vec<ToolOption> = all_native
        .iter()
        .map(|p| {
            let has_all_permission = allowed_plugins.contains("*");
            let is_enabled = has_all_permission || allowed_plugins.contains(p.name);
            ToolOption {
                name: p.name,
                key: p.name,
                enabled: is_enabled,
            }
        })
        .collect();

    let mut native_cursor = 0;
    redraw_phase_2(&native_options, native_cursor);
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
                            println!("\n\x1b[31mSetup cancelled.\x1b[0m");
                            return Ok(None);
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if native_cursor > 0 {
                                    native_cursor -= 1;
                                } else {
                                    native_cursor = native_options.len() - 1;
                                }
                                disable_raw_mode()?;
                                redraw_phase_2(&native_options, native_cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Down => {
                                if native_cursor < native_options.len() - 1 {
                                    native_cursor += 1;
                                } else {
                                    native_cursor = 0;
                                }
                                disable_raw_mode()?;
                                redraw_phase_2(&native_options, native_cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char(' ') => {
                                native_options[native_cursor].enabled =
                                    !native_options[native_cursor].enabled;
                                disable_raw_mode()?;
                                redraw_phase_2(&native_options, native_cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char('a') => {
                                for opt in &mut native_options {
                                    opt.enabled = true;
                                }
                                disable_raw_mode()?;
                                redraw_phase_2(&native_options, native_cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Char('i') => {
                                for opt in &mut native_options {
                                    opt.enabled = !opt.enabled;
                                }
                                disable_raw_mode()?;
                                redraw_phase_2(&native_options, native_cursor);
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

    let enabled_natives: Vec<serde_json::Value> = native_options
        .iter()
        .filter(|o| o.enabled)
        .map(|o| serde_json::Value::String(o.key.to_string()))
        .collect();

    config.extra.insert(
        "allowedNativePlugins".to_string(),
        serde_json::Value::Array(enabled_natives),
    );
    save_config(&config)?;

    println!("\x1b[32mSuccessfully updated native plugins configurations!\x1b[0m\n");

    let run_options = vec![
        ToolOption {
            name: "1. CLI (Interactive Terminal Assistant)",
            key: "cli",
            enabled: false,
        },
        ToolOption {
            name: "2. Desktop App (Download & Install)",
            key: "app_link",
            enabled: false,
        },
        ToolOption {
            name: "3. Web (Vite Web App UI)",
            key: "web",
            enabled: false,
        },
    ];

    let mut run_cursor = 0;
    redraw_phase_3(&run_options, run_cursor);
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
                            println!("\n\x1b[31mRun selection cancelled.\x1b[0m");
                            return Ok(None);
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if run_cursor > 0 {
                                    run_cursor -= 1;
                                } else {
                                    run_cursor = run_options.len() - 1;
                                }
                                disable_raw_mode()?;
                                redraw_phase_3(&run_options, run_cursor);
                                enable_raw_mode()?;
                            }
                            KeyCode::Down => {
                                if run_cursor < run_options.len() - 1 {
                                    run_cursor += 1;
                                } else {
                                    run_cursor = 0;
                                }
                                disable_raw_mode()?;
                                redraw_phase_3(&run_options, run_cursor);
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

    Ok(Some(run_options[run_cursor].key.to_string()))
}

fn print_options(options: &[ToolOption], cursor: usize) {
    for (i, opt) in options.iter().enumerate() {
        let checkbox = if opt.enabled {
            "\x1b[32m◉\x1b[0m"
        } else {
            "\x1b[90m○\x1b[0m"
        };
        if i == cursor {
            println!(
                "  \x1b[36m❯\x1b[0m {} \x1b[36m{}\x1b[0m",
                checkbox, opt.name
            );
        } else {
            println!("    {} {}", checkbox, opt.name);
        }
    }
    let _ = io::stdout().flush();
}

fn print_run_options(options: &[ToolOption], cursor: usize) {
    for (i, opt) in options.iter().enumerate() {
        if i == cursor {
            println!("  \x1b[36m❯\x1b[0m \x1b[36m{}\x1b[0m", opt.name);
        } else {
            println!("    {}", opt.name);
        }
    }
    let _ = io::stdout().flush();
}

fn redraw_phase_1(options: &[ToolOption], cursor: usize) {
    print!("\x1b[2J\x1b[1;1H");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[32m       Mint CLI Tool Manager Wizard\x1b[0m");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("Configure which agent tools are enabled or disabled:");
    println!(
        "  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Space: Toggle | a: All | i: Invert | Enter: Confirm]\x1b[0m"
    );
    println!();
    print_options(options, cursor);
}

fn redraw_phase_2(options: &[ToolOption], cursor: usize) {
    print!("\x1b[2J\x1b[1;1H");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[32m       Configure Native Plugins Access\x1b[0m");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("Select which native plugins are allowed to run:");
    println!(
        "  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Space: Toggle | a: All | i: Invert | Enter: Confirm]\x1b[0m"
    );
    println!();
    print_options(options, cursor);
}

fn redraw_phase_3(options: &[ToolOption], cursor: usize) {
    print!("\x1b[2J\x1b[1;1H");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("\x1b[32m       Choose where to run Mint AI Agent\x1b[0m");
    println!("\x1b[36m━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\x1b[0m");
    println!("Select the environment you want to launch:");
    println!("  \x1b[90m[Keyboard Controls: ↑/↓: Navigate | Enter: Confirm]\x1b[0m");
    println!();
    print_run_options(options, cursor);
}

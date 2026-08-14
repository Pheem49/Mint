use super::*;

pub fn confirm(prompt: &str) -> Result<bool> {
    confirm_scoped(prompt, &SESSION_APPROVED)
}
/// Same picker as `confirm`, but scoped to the agent's "Security
/// Authorization" prompts via `SECURITY_SESSION_APPROVED` instead of the
/// general-purpose `SESSION_APPROVED` flag — see that flag's doc comment.
pub fn confirm_security(prompt: &str) -> Result<bool> {
    confirm_scoped(prompt, &SECURITY_SESSION_APPROVED)
}
fn confirm_scoped(
    prompt: &str,
    session_flag: &'static std::sync::atomic::AtomicBool,
) -> Result<bool> {
    let clean_prompt = prompt
        .replace(" [y/N] ", "")
        .replace(" [y/N]", "")
        .trim()
        .to_string();

    if session_flag.load(std::sync::atomic::Ordering::Relaxed) {
        println!("  {} {MINT}Approve (session-wide){RESET}", clean_prompt);
        return Ok(true);
    }

    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() || enable_raw_mode().is_err() {
        print!("  {} [y/N] ", clean_prompt);
        let _ = io::stdout().flush();
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        return Ok(matches!(
            answer.trim().to_ascii_lowercase().as_str(),
            "y" | "yes"
        ));
    }

    let _ = disable_raw_mode();
    println!("  \x1b[1;97m{}\x1b[0m", clean_prompt);

    let options_with_desc = [
        ("Approve (Once)", "Allow single execution"),
        (
            "Approve (Entire Session)",
            "Auto-approve throughout session",
        ),
        ("Deny", "Cancel action"),
    ];
    let mut selected = 0;

    let print_choices = |selected: usize| -> Result<()> {
        for (i, (opt, desc)) in options_with_desc.iter().enumerate() {
            if i == selected {
                println!(
                    "  {BLUE}❯ {}. {:<24}{RESET} {DIM}- {}{RESET}",
                    i + 1,
                    opt,
                    desc
                );
            } else {
                println!("    {DIM}{}. {:<24} - {}{RESET}", i + 1, opt, desc);
            }
        }
        io::stdout().flush()?;
        Ok(())
    };

    print_choices(selected)?;

    let _ = enable_raw_mode();

    let choice = loop {
        match event::poll(std::time::Duration::from_millis(100)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key_event)) => {
                    if key_event.kind == event::KeyEventKind::Press {
                        let is_ctrl_c = matches!(key_event.code, KeyCode::Char('c'))
                            && key_event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL);
                        if is_ctrl_c {
                            break 2;
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                if selected > 0 {
                                    selected -= 1;
                                } else {
                                    selected = options_with_desc.len() - 1;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options_with_desc.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Down => {
                                if selected < options_with_desc.len() - 1 {
                                    selected += 1;
                                } else {
                                    selected = 0;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options_with_desc.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Tab => {
                                if selected < options_with_desc.len() - 1 {
                                    selected += 1;
                                } else {
                                    selected = 0;
                                }
                                let _ = disable_raw_mode();
                                print!("\x1b[{}A\x1b[J", options_with_desc.len());
                                let _ = print_choices(selected);
                                let _ = enable_raw_mode();
                            }
                            KeyCode::Char('1') | KeyCode::Char('a') | KeyCode::Char('y') => {
                                break 0;
                            }
                            KeyCode::Char('2') | KeyCode::Char('s') => {
                                break 1;
                            }
                            KeyCode::Char('3') | KeyCode::Char('n') | KeyCode::Char('c') => {
                                break 2;
                            }
                            KeyCode::Enter => {
                                break selected;
                            }
                            KeyCode::Esc => {
                                break 2;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    break 2;
                }
            },
            Ok(false) => {}
            Err(_) => {
                break 2;
            }
        }
    };

    let _ = disable_raw_mode();
    print!("\x1b[{}A\x1b[J", options_with_desc.len() + 1);

    let result_str = match choice {
        0 => format!("{MINT}Approve (Once){RESET}"),
        1 => format!("{MINT}Approve (Entire Session){RESET}"),
        _ => format!("{ERROR}Deny{RESET}"),
    };
    println!("  {} {}", clean_prompt, result_str);
    let _ = io::stdout().flush();

    match choice {
        0 => Ok(true),
        1 => {
            session_flag.store(true, std::sync::atomic::Ordering::Relaxed);
            Ok(true)
        }
        _ => Ok(false),
    }
}
/// second picker after the provider is chosen. Empty means no known list
/// (e.g. `local_openai`), so `/models` skips straight to that provider's
/// current/default model instead of showing an empty picker.
pub fn model_options_for_provider(config: &mint_core::MintConfig, provider: &str) -> Vec<String> {
    match provider {
        "gemini" => onboard::GEMINI_MODEL_PRESETS,
        "anthropic" => onboard::ANTHROPIC_MODEL_PRESETS,
        "openai" => onboard::OPENAI_MODEL_PRESETS,
        "openrouter" => onboard::OPENROUTER_MODEL_PRESETS,
        "deepseek" => onboard::DEEPSEEK_MODEL_PRESETS,
        "huggingface" => onboard::HUGGINGFACE_MODEL_PRESETS,
        "ollama" => return onboard::installed_ollama_models(),
        p if p.starts_with("custom:") => {
            return config
                .resolve_custom_provider(p)
                .map(|cp| cp.models.iter().map(|m| m.model_id.clone()).collect())
                .unwrap_or_default();
        }
        _ => &[],
    }
    .iter()
    .map(|s| s.to_string())
    .collect()
}

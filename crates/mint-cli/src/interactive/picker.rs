use super::*;

/// Arrow-key picker used by ~13 call sites across the CLI (approvals,
/// `/mcp`, `/models`, mode toggles, etc.). Renders via a self-contained
/// `ratatui` inline `Terminal` — constructed fresh for this one call and
/// torn down before returning, never shared with the agent-turn
/// `InlineTui`/`LiveStatus` machinery in `agent.rs`. That's deliberate: most
/// call sites run from plain slash-command handling, with no agent turn (and
/// so no live `LiveStatus`) in scope at all, so there's nothing to share —
/// this needs to work standalone. The option count (and so the viewport
/// height) is fixed for the whole call, so unlike the agent-turn code this
/// never needs to reconstruct mid-loop, only redraw.
pub fn prompt_interactive_select(
    title: &str,
    options: &[String],
    current_selection: &str,
) -> Result<Option<String>> {
    use ansi_to_tui::IntoText;
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() || options.is_empty() {
        return Ok(None);
    }

    let mut selected = options
        .iter()
        .position(|p| p == current_selection)
        .unwrap_or(0);

    // Title line + one line per option — fixed for this call's whole
    // lifetime, so the viewport is sized once and only ever redrawn.
    let height = (options.len() as u16).saturating_add(1);
    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let Ok(mut terminal) = agent::with_raw_mode_for_cursor_query(move || {
        ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Inline(height),
            },
        )
    }) else {
        return Ok(None);
    };

    if crossterm::terminal::enable_raw_mode().is_err() {
        let _ = terminal.clear();
        return Ok(None);
    }

    let render = |terminal: &mut ratatui::Terminal<_>, selected: usize| {
        let mut lines = vec![format!(
            "{BLUE}{title} (Use ↑/↓ to navigate, Enter to select, Esc to cancel):{RESET}"
        )];
        for (i, opt) in options.iter().enumerate() {
            if i == selected {
                lines.push(format!("  {BLUE}❯ {opt}{RESET}"));
            } else {
                lines.push(format!("    {DIM}{opt}{RESET}"));
            }
        }
        if let Ok(text) = lines.join("\n").into_text() {
            let _ = terminal.draw(|frame| {
                let area = frame.area();
                frame.render_widget(ratatui::widgets::Paragraph::new(text), area);
            });
        }
    };

    render(&mut terminal, selected);

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
                            break None;
                        }

                        match key_event.code {
                            KeyCode::Up => {
                                selected = if selected > 0 {
                                    selected - 1
                                } else {
                                    options.len() - 1
                                };
                                render(&mut terminal, selected);
                            }
                            KeyCode::Down | KeyCode::Tab => {
                                selected = if selected < options.len() - 1 {
                                    selected + 1
                                } else {
                                    0
                                };
                                render(&mut terminal, selected);
                            }
                            KeyCode::Enter => {
                                break Some(selected);
                            }
                            KeyCode::Esc => {
                                break None;
                            }
                            _ => {}
                        }
                    }
                }
                Ok(_) => {}
                Err(_) => {
                    break None;
                }
            },
            Ok(false) => {}
            Err(_) => {
                break None;
            }
        }
    };

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = terminal.clear();

    Ok(choice.map(|idx| options[idx].clone()))
}

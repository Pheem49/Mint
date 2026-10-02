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

    if !io::stdout().is_tty() || !io::stdin().is_tty() || options.is_empty() {
        return Ok(None);
    }

    const MAX_VISIBLE: usize = 12;
    let total_options = options.len();
    let has_scroll = total_options > MAX_VISIBLE;

    // Viewport height: title line + visible items + (if scrolling: status line)
    let visible_capacity = total_options.min(MAX_VISIBLE);
    let height = (visible_capacity as u16) + if has_scroll { 2 } else { 1 };

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

    let mut filter_text = String::new();
    let get_filtered = |filter: &str| -> Vec<usize> {
        if filter.is_empty() {
            (0..options.len()).collect()
        } else {
            let q = filter.to_lowercase();
            options
                .iter()
                .enumerate()
                .filter(|(_, opt)| opt.to_lowercase().contains(&q))
                .map(|(idx, _)| idx)
                .collect()
        }
    };

    let mut filtered_indices = get_filtered(&filter_text);
    let mut selected_filtered = options
        .iter()
        .position(|p| p == current_selection)
        .and_then(|idx| filtered_indices.iter().position(|&fi| fi == idx))
        .unwrap_or(0);
    let mut scroll_offset = 0usize;

    let render = |terminal: &mut ratatui::Terminal<_>,
                  selected: usize,
                  scroll: &mut usize,
                  filter: &str,
                  filtered: &[usize]| {
        let total = filtered.len();
        if total > 0 {
            if selected < *scroll {
                *scroll = selected;
            } else if selected >= *scroll + visible_capacity {
                *scroll = selected + 1 - visible_capacity;
            }
        } else {
            *scroll = 0;
        }

        let nav_hint = if !filter.is_empty() {
            format!("(Filter: \"{filter}\" • Esc to clear, Enter to select)")
        } else if options.len() <= 9 {
            format!(
                "(Press 1-{}, ↑/↓ to navigate, Enter to select, Esc to cancel)",
                options.len()
            )
        } else {
            "(Use ↑/↓ to navigate, Enter to select, Esc to cancel)".to_string()
        };

        let mut lines = vec![format!("{BLUE}{title} {nav_hint}:{RESET}")];

        if total == 0 {
            lines.push(format!("  {DIM}(No matching options){RESET}"));
        } else {
            let start = *scroll;
            let end = (start + visible_capacity).min(total);
            for i in start..end {
                let orig_idx = filtered[i];
                let opt = &options[orig_idx];
                let prefix = if options.len() <= 9
                    && filter.is_empty()
                    && !opt.starts_with('[')
                    && !opt.starts_with(&format!("{}.", orig_idx + 1))
                {
                    format!("[{}] ", orig_idx + 1)
                } else {
                    String::new()
                };

                if i == selected {
                    lines.push(format!("  {BLUE}❯ {prefix}{opt}{RESET}"));
                } else {
                    lines.push(format!("    {DIM}{prefix}{opt}{RESET}"));
                }
            }
        }

        if has_scroll || !filter.is_empty() {
            if total > 0 {
                let start = *scroll + 1;
                let end = (*scroll + visible_capacity).min(total);
                lines.push(format!(
                    "{DIM}  [Showing {start}-{end} of {total}] • Use ↑/↓ to scroll • Type to filter{RESET}"
                ));
            } else {
                lines.push(format!(
                    "{DIM}  [0 of {total}] • Press Backspace or Esc to clear filter{RESET}"
                ));
            }
        }

        while lines.len() < height as usize {
            lines.push(String::new());
        }

        if let Ok(text) = lines.join("\n").into_text() {
            let _ = terminal.draw(|frame| {
                let area = frame.area();
                frame.render_widget(ratatui::widgets::Paragraph::new(text), area);
            });
        }
    };

    render(
        &mut terminal,
        selected_filtered,
        &mut scroll_offset,
        &filter_text,
        &filtered_indices,
    );

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
                            KeyCode::Char(c)
                                if c.is_ascii_digit()
                                    && c != '0'
                                    && filter_text.is_empty()
                                    && options.len() <= 9 =>
                            {
                                let idx = (c as usize) - ('1' as usize);
                                if idx < options.len() {
                                    break Some(idx);
                                }
                            }
                            KeyCode::Up => {
                                if !filtered_indices.is_empty() {
                                    selected_filtered = if selected_filtered > 0 {
                                        selected_filtered - 1
                                    } else {
                                        filtered_indices.len() - 1
                                    };
                                    render(
                                        &mut terminal,
                                        selected_filtered,
                                        &mut scroll_offset,
                                        &filter_text,
                                        &filtered_indices,
                                    );
                                }
                            }
                            KeyCode::Down | KeyCode::Tab => {
                                if !filtered_indices.is_empty() {
                                    selected_filtered =
                                        if selected_filtered < filtered_indices.len() - 1 {
                                            selected_filtered + 1
                                        } else {
                                            0
                                        };
                                    render(
                                        &mut terminal,
                                        selected_filtered,
                                        &mut scroll_offset,
                                        &filter_text,
                                        &filtered_indices,
                                    );
                                }
                            }
                            KeyCode::Enter => {
                                if let Some(&orig_idx) = filtered_indices.get(selected_filtered) {
                                    break Some(orig_idx);
                                }
                            }
                            KeyCode::Esc => {
                                if !filter_text.is_empty() {
                                    filter_text.clear();
                                    filtered_indices = get_filtered(&filter_text);
                                    selected_filtered = 0;
                                    scroll_offset = 0;
                                    render(
                                        &mut terminal,
                                        selected_filtered,
                                        &mut scroll_offset,
                                        &filter_text,
                                        &filtered_indices,
                                    );
                                } else {
                                    break None;
                                }
                            }
                            KeyCode::Backspace => {
                                if !filter_text.is_empty() {
                                    filter_text.pop();
                                    filtered_indices = get_filtered(&filter_text);
                                    selected_filtered = 0;
                                    scroll_offset = 0;
                                    render(
                                        &mut terminal,
                                        selected_filtered,
                                        &mut scroll_offset,
                                        &filter_text,
                                        &filtered_indices,
                                    );
                                }
                            }
                            KeyCode::Char(c) if !c.is_control() => {
                                filter_text.push(c);
                                filtered_indices = get_filtered(&filter_text);
                                selected_filtered = 0;
                                scroll_offset = 0;
                                render(
                                    &mut terminal,
                                    selected_filtered,
                                    &mut scroll_offset,
                                    &filter_text,
                                    &filtered_indices,
                                );
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

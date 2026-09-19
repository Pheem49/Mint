use super::*;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::tty::IsTty;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use std::sync::Mutex;
use std::time::Duration;

#[derive(Clone, Debug)]
pub struct ThoughtRecord {
    pub thought: String,
    pub elapsed_str: String,
    live_id: Option<String>,
}

static LAST_THOUGHT: Mutex<Option<ThoughtRecord>> = Mutex::new(None);

#[allow(dead_code)]
pub fn set_last_thought(thought: &str, elapsed_str: &str) {
    if let Ok(mut guard) = LAST_THOUGHT.lock() {
        *guard = Some(ThoughtRecord {
            thought: thought.to_string(),
            elapsed_str: elapsed_str.to_string(),
            live_id: None,
        });
    }
}

pub fn append_thought(thought: &str, elapsed_str: &str) {
    if let Ok(mut guard) = LAST_THOUGHT.lock() {
        if let Some(record) = guard.as_mut() {
            record
                .thought
                .push_str("\n\n── Next Step ──────────────────────────\n");
            record.thought.push_str(thought);
            record.elapsed_str = elapsed_str.to_string();
        } else {
            *guard = Some(ThoughtRecord {
                thought: thought.to_string(),
                elapsed_str: elapsed_str.to_string(),
                live_id: None,
            });
        }
    }
}

pub fn append_thought_delta(id: &str, delta: &str, elapsed_str: &str) {
    if let Ok(mut guard) = LAST_THOUGHT.lock() {
        match guard.as_mut() {
            Some(record) if record.live_id.as_deref() == Some(id) => {
                record.thought.push_str(delta);
                record.elapsed_str = elapsed_str.to_string();
            }
            Some(record) => {
                record
                    .thought
                    .push_str("\n\n── Next Step ──────────────────────────\n");
                record.thought.push_str(delta);
                record.elapsed_str = elapsed_str.to_string();
                record.live_id = Some(id.to_string());
            }
            None => {
                *guard = Some(ThoughtRecord {
                    thought: delta.to_string(),
                    elapsed_str: elapsed_str.to_string(),
                    live_id: Some(id.to_string()),
                });
            }
        }
    }
}

pub fn finish_thought(id: Option<&str>, thought: &str, elapsed_str: &str) {
    if let Ok(mut guard) = LAST_THOUGHT.lock()
        && let Some(record) = guard.as_mut()
        && id.is_some()
        && record.live_id.as_deref() == id
    {
        let marker = "\n\n── Next Step ──────────────────────────\n";
        if let Some(index) = record.thought.rfind(marker) {
            record.thought.truncate(index + marker.len());
            record.thought.push_str(thought);
        } else {
            record.thought = thought.to_string();
        }
        record.elapsed_str = elapsed_str.to_string();
        record.live_id = None;
        return;
    }
    append_thought(thought, elapsed_str);
}

pub fn get_last_thought() -> Option<ThoughtRecord> {
    LAST_THOUGHT.lock().ok().and_then(|guard| guard.clone())
}

pub fn clear_last_thought() {
    if let Ok(mut guard) = LAST_THOUGHT.lock() {
        *guard = None;
    }
}

pub fn format_thought_elapsed(duration: Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs < 60.0 {
        format!("{:.1}s", secs)
    } else {
        let minutes = duration.as_secs() / 60;
        let seconds = duration.as_secs() % 60;
        format!("{minutes}m {seconds:02}s")
    }
}

pub fn show_thought_viewer(thought: &str, elapsed_str: &str) -> Result<()> {
    if !io::stdout().is_tty() || !io::stdin().is_tty() || thought.trim().is_empty() {
        println!();
        println!(
            "\x1b[38;2;148;163;184m╭─ Thought Process ({elapsed_str}) ─────────────────────\x1b[0m"
        );
        for line in thought.trim().lines() {
            println!("\x1b[38;2;148;163;184m│\x1b[0m \x1b[38;2;203;213;225m{line}\x1b[0m");
        }
        println!(
            "\x1b[38;2;148;163;184m╰──────────────────────────────────────────────────\x1b[0m"
        );
        println!();
        return Ok(());
    }

    let (tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    let width = tw as usize;
    // Cap height between 8 and 18 lines so it fits comfortably in any terminal
    let height = (th.saturating_sub(4)).clamp(8, 18);
    let inner_height = (height as usize).saturating_sub(2);
    // Inside a bordered block: left border (1) + left pad (1) + text + right pad (1) + right border (1) = 4 cols
    let content_width = width.saturating_sub(6).max(20);

    // Pre-wrap thought lines
    let mut wrapped_lines: Vec<String> = Vec::new();
    for paragraph in thought.trim().lines() {
        let p_trimmed = paragraph.trim_end();
        if p_trimmed.is_empty() {
            wrapped_lines.push(String::new());
        } else {
            let options = textwrap::Options::new(content_width).break_words(true);
            let filled = textwrap::fill(p_trimmed, &options);
            for l in filled.lines() {
                wrapped_lines.push(l.to_string());
            }
        }
    }

    if wrapped_lines.is_empty() {
        return Ok(());
    }

    let max_scroll = wrapped_lines.len().saturating_sub(inner_height);
    let mut scroll_offset = 0usize;

    let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
    let Ok(mut terminal) = agent::with_raw_mode_for_cursor_query(move || {
        ratatui::Terminal::with_options(
            backend,
            ratatui::TerminalOptions {
                viewport: ratatui::Viewport::Inline(height),
            },
        )
    }) else {
        return Ok(());
    };

    if crossterm::terminal::enable_raw_mode().is_err() {
        let _ = terminal.clear();
        return Ok(());
    }

    let render = |terminal: &mut ratatui::Terminal<_>, scroll: usize| {
        let scroll_hint = if max_scroll > 0 {
            format!(
                " ↑/↓ scroll {}/{} · Esc to close ",
                scroll + 1,
                max_scroll + 1
            )
        } else {
            " Esc or Enter to close ".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Rgb(168, 85, 247)))
            .title(Span::styled(
                format!(" Thought Process ({elapsed_str}) "),
                Style::default()
                    .fg(Color::Rgb(192, 132, 252))
                    .add_modifier(Modifier::BOLD),
            ))
            .title_bottom(Span::styled(
                scroll_hint,
                Style::default().fg(Color::Rgb(148, 163, 184)),
            ));

        let visible = &wrapped_lines[scroll..(scroll + inner_height).min(wrapped_lines.len())];
        let mut paragraph_lines: Vec<Line> = Vec::with_capacity(inner_height);
        for l in visible {
            paragraph_lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(l.clone(), Style::default().fg(Color::Rgb(203, 213, 225))),
            ]));
        }
        for _ in visible.len()..inner_height {
            paragraph_lines.push(Line::from(""));
        }

        let paragraph = Paragraph::new(paragraph_lines).block(block);

        let _ = terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Clear, area);
            frame.render_widget(paragraph, area);
        });
    };

    render(&mut terminal, scroll_offset);

    loop {
        match event::poll(std::time::Duration::from_millis(100)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key_event)) => {
                    if key_event.kind == event::KeyEventKind::Press {
                        let ctrl = key_event.modifiers.contains(KeyModifiers::CONTROL);
                        match key_event.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => break,
                            KeyCode::Char('t') if ctrl => break,
                            KeyCode::Char('c') if ctrl => break,
                            KeyCode::Up | KeyCode::Char('k') => {
                                scroll_offset = scroll_offset.saturating_sub(1);
                                render(&mut terminal, scroll_offset);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                scroll_offset = (scroll_offset + 1).min(max_scroll);
                                render(&mut terminal, scroll_offset);
                            }
                            KeyCode::PageUp => {
                                scroll_offset = scroll_offset.saturating_sub(inner_height);
                                render(&mut terminal, scroll_offset);
                            }
                            KeyCode::PageDown => {
                                scroll_offset = (scroll_offset + inner_height).min(max_scroll);
                                render(&mut terminal, scroll_offset);
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                scroll_offset = 0;
                                render(&mut terminal, scroll_offset);
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                scroll_offset = max_scroll;
                                render(&mut terminal, scroll_offset);
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            },
            Ok(false) => {}
            Err(_) => break,
        }
    }

    let _ = terminal.clear();
    let _ = crossterm::terminal::disable_raw_mode();
    drop(terminal);
    Ok(())
}

//! Extended thinking state, live thought tail wrapping, and shimmer status line.
use super::*;

/// Sweep a white glow across the thinking text, fading to dim gray at the edges.
pub(super) fn shimmer_thinking_line(line: &str, tick: usize) -> Line<'static> {
    // Split at the first " (" to isolate the verb from the timer suffix.
    let (verb, suffix) = if line.contains("Compacting context")
        && let Some(idx) = line.find(" · ")
    {
        (&line[..idx], &line[idx..])
    } else if let Some(idx) = line.find(" (") {
        (&line[..idx], &line[idx..])
    } else {
        (line, "")
    };

    let chars: Vec<char> = verb.chars().collect();
    let count = chars.len();
    let period = (count * 2).max(16);
    let spot = (tick % period) as f32 / period as f32 * count as f32;

    let (dim_r, dim_g, dim_b) = crate::terminal_theme::THINKING_GLOW_DIM;
    let (bright_r, bright_g, bright_b) = crate::terminal_theme::THINKING_GLOW_BRIGHT;
    let mut spans: Vec<Span<'static>> = chars
        .iter()
        .enumerate()
        .map(|(i, &c)| {
            let dist = ((i as f32 - spot).abs() / (count as f32 * 0.35)).min(1.0);
            let brightness = ((1.0 - dist) * std::f32::consts::FRAC_PI_2).sin().powi(2);
            let r = (dim_r as f32 + (bright_r as f32 - dim_r as f32) * brightness).round() as u8;
            let g = (dim_g as f32 + (bright_g as f32 - dim_g as f32) * brightness).round() as u8;
            let b = (dim_b as f32 + (bright_b as f32 - dim_b as f32) * brightness).round() as u8;
            let style = Style::default()
                .fg(Color::Rgb(r, g, b))
                .add_modifier(Modifier::BOLD);
            Span::styled(c.to_string(), style)
        })
        .collect();

    if !suffix.is_empty() {
        spans.push(Span::styled(
            suffix.to_string(),
            Style::default().fg(crate::terminal_theme::MUTED),
        ));
    }

    Line::from(spans)
}

#[derive(Debug, Clone)]
pub(crate) struct ThoughtModalState {
    pub elapsed_str: String,
    pub lines: Vec<String>,
    pub scroll: usize,
}

#[derive(Debug)]
pub(super) struct ThinkingSegment {
    pub(super) id: Option<String>,
    pub(super) text: String,
    pub(super) elapsed: std::time::Duration,
    pub(super) elapsed_known: bool,
    pub(super) updated_at: Option<std::time::Instant>,
}

impl ThinkingSegment {
    pub(super) fn elapsed_label(&self) -> String {
        if self.elapsed_known {
            super::format_thought_elapsed(self.elapsed())
        } else {
            "time unavailable".into()
        }
    }
    pub(super) fn elapsed(&self) -> std::time::Duration {
        self.elapsed
            + self
                .updated_at
                .map(|time| time.elapsed())
                .unwrap_or_default()
    }
}

// Wrap from the tail so older paragraphs need not be laid out every frame.
pub(super) fn thought_tail(text: &str, width: usize, height: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for paragraph in text.trim_end_matches('\n').rsplit('\n') {
        let wrapped = textwrap::wrap(
            paragraph,
            textwrap::Options::new(width.max(1)).break_words(true),
        );
        for line in wrapped.into_iter().rev() {
            if rows.len() == height {
                break;
            }
            rows.push(line.into_owned());
        }
        if rows.len() == height {
            break;
        }
    }
    rows.reverse();
    rows
}

impl ChatViewState {
    pub(super) fn live_thought(&self) -> Option<&ThinkingSegment> {
        self.thoughts
            .last()
            .filter(|thought| thought.updated_at.is_some() && !thought.text.trim().is_empty())
    }

    pub(super) fn thought_total_elapsed_label(&self) -> String {
        if self.thoughts.iter().all(|thought| thought.elapsed_known) {
            super::format_thought_elapsed(self.thoughts.iter().map(ThinkingSegment::elapsed).sum())
        } else {
            "time unavailable".into()
        }
    }

    pub(super) fn thought_record(&self) -> Option<ThoughtModalState> {
        self.thoughts.last()?;
        Some(ThoughtModalState {
            elapsed_str: self.thought_total_elapsed_label(),
            lines: self
                .thoughts
                .iter()
                .enumerate()
                .map(|(index, thought)| {
                    format!(
                        "── Thought {} · {} ──────────────────────────\n{}",
                        index + 1,
                        thought.elapsed_label(),
                        thought.text
                    )
                })
                .collect::<Vec<_>>()
                .join("\n\n")
                .lines()
                .map(str::to_owned)
                .collect(),
            scroll: 0,
        })
    }

    pub(super) fn refresh_thought_modal(&mut self) {
        if let Some(mut updated) = self.thought_record()
            && let Some(modal) = self.thought_modal.as_mut()
        {
            updated.scroll = modal.scroll;
            *modal = updated;
        }
    }

    pub(super) fn finish_live_thought(&mut self) {
        if let Some(thought) = self.thoughts.last_mut()
            && thought.updated_at.is_some()
        {
            thought.elapsed = thought.elapsed();
            thought.updated_at = None;
            let elapsed = super::format_thought_elapsed(thought.elapsed);
            if !thought.text.trim().is_empty() {
                self.push_notice(format!("Thought · {elapsed} (Ctrl+T to view)"));
            }
            self.refresh_thought_modal();
        }
    }

    pub(super) fn handle_thought_key(&mut self, key: event::KeyEvent) -> bool {
        use event::{KeyCode, KeyModifiers};
        if let Some(modal) = self.thought_modal.as_mut() {
            match key.code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => self.thought_modal = None,
                KeyCode::Char('t' | 'c' | 'd') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.thought_modal = None
                }
                KeyCode::Up | KeyCode::Char('k') => modal.scroll = modal.scroll.saturating_sub(1),
                KeyCode::Down | KeyCode::Char('j') => modal.scroll = modal.scroll.saturating_add(1),
                KeyCode::PageUp => modal.scroll = modal.scroll.saturating_sub(10),
                KeyCode::PageDown => modal.scroll = modal.scroll.saturating_add(10),
                KeyCode::Home | KeyCode::Char('g') => modal.scroll = 0,
                KeyCode::End | KeyCode::Char('G') => modal.scroll = usize::MAX,
                _ => {}
            }
            return true;
        }
        if key.code == KeyCode::Char('t') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.open_thought_modal();
            return true;
        }
        false
    }

    pub fn open_thought_modal(&mut self) {
        self.thought_modal = self.thought_record();
        if self.thought_modal.is_none() {
            self.set_notice("No thought process recorded for this turn");
        } else {
            self.selection_mode = false;
            self.clear_notice();
        }
    }
}

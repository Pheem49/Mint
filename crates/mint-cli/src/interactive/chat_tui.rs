//! Full-screen adapter for interactive chat.
use super::{
    InteractiveInput, InteractiveSession, active_model, format_provider_display_name,
    format_workspace_with_branch,
};
use ansi_to_tui::IntoText;
use anyhow::{Result, bail};
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use crossterm::{ExecutableCommand, event, terminal};
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    buffer::Buffer,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState,
        Wrap,
    },
};
use std::io::{self, Write};
use std::path::Path;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
    mpsc,
};
use unicode_width::UnicodeWidthStr;

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;
const COPY_LIMIT: usize = 100 * 1024;
const BACK_TO_BOTTOM_LABEL: &str = " ↓ Back to bottom · End ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptRole {
    User,
    Assistant,
    Notice,
    Command,
    #[allow(dead_code)]
    System,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptEntry {
    pub role: TranscriptRole,
    pub text: String,
    pub rendered_lines: Vec<Line<'static>>,
    pub plain_lines: Vec<String>,
}

impl TranscriptEntry {
    pub fn new(role: TranscriptRole, text: impl Into<String>) -> Self {
        let text = text.into();
        let (rendered_lines, plain_lines) = Self::render_content(role, &text);
        Self {
            role,
            text,
            rendered_lines,
            plain_lines,
        }
    }

    fn render_content(role: TranscriptRole, text: &str) -> (Vec<Line<'static>>, Vec<String>) {
        if text.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let processed = match role {
            TranscriptRole::Assistant => {
                let clean = crate::agent::sanitize_latex(text);
                crate::agent::format_markdown_bold(&clean)
            }
            TranscriptRole::User
            | TranscriptRole::Notice
            | TranscriptRole::Command
            | TranscriptRole::System => crate::agent::format_markdown_bold(text),
        };

        if let Ok(parsed) = processed.into_text() {
            let rendered = parsed.lines;
            let plain: Vec<String> = rendered
                .iter()
                .map(|line| {
                    line.spans
                        .iter()
                        .map(|span| span.content.as_ref())
                        .collect::<String>()
                })
                .collect();
            (rendered, plain)
        } else {
            let rendered: Vec<Line<'static>> =
                text.lines().map(|l| Line::raw(l.to_string())).collect();
            let plain: Vec<String> = text.lines().map(str::to_owned).collect();
            (rendered, plain)
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ThoughtModalState {
    pub elapsed_str: String,
    pub lines: Vec<String>,
    pub scroll: usize,
}

#[derive(Debug, Default)]
pub(crate) struct ChatViewState {
    pub(crate) transcript: Vec<TranscriptEntry>,
    composer: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    draft_before_history: Vec<char>,
    slash_selected: usize,
    status: Vec<String>,
    model: String,
    provider: String,
    workspace: String,
    current_dir: std::path::PathBuf,
    plan_mode: bool,
    scroll_from_bottom: u16,
    selection_mode: bool,
    selection_anchor: usize,
    selection_head: usize,
    notice: Option<(String, std::time::Instant)>,
    dialog: Option<DialogState>,
    thought_modal: Option<ThoughtModalState>,
}

#[derive(Debug)]
pub(crate) struct DialogState {
    pub title: String,
    pub body: String,
    pub options: Vec<String>,
    pub filter: Vec<char>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub checked: Option<Vec<bool>>,
    pub input: Option<Vec<char>>,
    pub reply: mpsc::Sender<DialogAnswer>,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum DialogAnswer {
    Choice(usize),
    Choices(Vec<usize>),
    Text(String),
    Cancel,
}

impl DialogState {
    pub fn new_choice(
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
        checked: Option<Vec<bool>>,
        reply: mpsc::Sender<DialogAnswer>,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            options,
            filter: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            checked,
            input: None,
            reply,
        }
    }

    pub fn new_text(
        title: impl Into<String>,
        body: impl Into<String>,
        reply: mpsc::Sender<DialogAnswer>,
    ) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            options: Vec::new(),
            filter: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            checked: None,
            input: Some(Vec::new()),
            reply,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        if self.filter.is_empty() {
            (0..self.options.len()).collect()
        } else {
            let query: String = self.filter.iter().collect::<String>().to_lowercase();
            self.options
                .iter()
                .enumerate()
                .filter(|(_, opt)| opt.to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect()
        }
    }

    pub fn select_up(&mut self) {
        let total = self.filtered_indices().len();
        if total > 0 {
            self.selected = self.selected.saturating_sub(1);
        }
    }

    pub fn select_down(&mut self) {
        let total = self.filtered_indices().len();
        if total > 0 {
            self.selected = (self.selected + 1).min(total.saturating_sub(1));
        }
    }

    pub fn select_numeric(&mut self, digit: usize) -> bool {
        let total = self.filtered_indices().len();
        if digit >= 1 && digit <= total && digit <= 9 {
            self.selected = digit - 1;
            true
        } else {
            false
        }
    }

    pub fn toggle_checked(&mut self) {
        let filtered = self.filtered_indices();
        if let Some(&original_idx) = filtered.get(self.selected) {
            if let Some(checked) = self.checked.as_mut() {
                if let Some(val) = checked.get_mut(original_idx) {
                    *val = !*val;
                }
            }
        }
    }

    pub fn handle_key(&mut self, key: event::KeyEvent) -> Option<DialogAnswer> {
        use event::{KeyCode, KeyModifiers};

        // Text input mode
        if let Some(input) = self.input.as_mut() {
            match key.code {
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    input.push(c);
                    None
                }
                KeyCode::Backspace => {
                    input.pop();
                    None
                }
                KeyCode::Enter => {
                    let text = input.iter().collect();
                    Some(DialogAnswer::Text(text))
                }
                KeyCode::Esc => Some(DialogAnswer::Cancel),
                KeyCode::Char('c') | KeyCode::Char('d')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    Some(DialogAnswer::Cancel)
                }
                _ => None,
            }
        } else {
            // Choice / multi-choice mode
            match key.code {
                KeyCode::Up => {
                    self.select_up();
                    None
                }
                KeyCode::Down => {
                    self.select_down();
                    None
                }
                KeyCode::PageUp => {
                    for _ in 0..5 {
                        self.select_up();
                    }
                    None
                }
                KeyCode::PageDown => {
                    for _ in 0..5 {
                        self.select_down();
                    }
                    None
                }
                KeyCode::Char(' ') if self.checked.is_some() => {
                    self.toggle_checked();
                    None
                }
                // Numeric shortcuts: 1-9 when filter is empty
                KeyCode::Char(c @ '1'..='9') if self.filter.is_empty() => {
                    let digit = c.to_digit(10).unwrap() as usize;
                    if self.select_numeric(digit) && self.checked.is_none() {
                        let filtered = self.filtered_indices();
                        if let Some(&orig_idx) = filtered.get(self.selected) {
                            return Some(DialogAnswer::Choice(orig_idx));
                        }
                    }
                    None
                }
                // Type to filter: typing chars
                KeyCode::Char(c)
                    if !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                {
                    self.filter.push(c);
                    self.selected = 0;
                    self.scroll_offset = 0;
                    None
                }
                KeyCode::Backspace => {
                    if self.filter.pop().is_some() {
                        self.selected = 0;
                        self.scroll_offset = 0;
                    }
                    None
                }
                KeyCode::Enter => {
                    if let Some(checked) = &self.checked {
                        let choices = checked
                            .iter()
                            .enumerate()
                            .filter_map(|(idx, &c)| if c { Some(idx) } else { None })
                            .collect();
                        Some(DialogAnswer::Choices(choices))
                    } else {
                        let filtered = self.filtered_indices();
                        if let Some(&orig_idx) = filtered.get(self.selected) {
                            Some(DialogAnswer::Choice(orig_idx))
                        } else {
                            Some(DialogAnswer::Cancel)
                        }
                    }
                }
                KeyCode::Esc => {
                    if !self.filter.is_empty() {
                        self.filter.clear();
                        self.selected = 0;
                        self.scroll_offset = 0;
                        None
                    } else {
                        Some(DialogAnswer::Cancel)
                    }
                }
                KeyCode::Char('c') | KeyCode::Char('d')
                    if key.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    Some(DialogAnswer::Cancel)
                }
                _ => None,
            }
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TuiHandle {
    pub(crate) state: Arc<Mutex<ChatViewState>>,
    interrupted: Arc<AtomicBool>,
    queued: Arc<Mutex<Vec<String>>>,
}

impl TuiHandle {
    pub fn set_status(&self, lines: Vec<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.status = lines;
        }
    }
    pub fn clear_status(&self) {
        self.set_status(Vec::new());
    }
    pub fn push_notice(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state
                .transcript
                .push(TranscriptEntry::new(TranscriptRole::Notice, text));
            state.scroll_from_bottom = 0;
        }
    }
    pub fn push_assistant(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state
                .transcript
                .push(TranscriptEntry::new(TranscriptRole::Assistant, text));
            state.scroll_from_bottom = 0;
        }
    }
    pub fn push_command(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state
                .transcript
                .push(TranscriptEntry::new(TranscriptRole::Command, text));
            state.scroll_from_bottom = 0;
        }
    }
    #[allow(dead_code)]
    pub fn push_system(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state
                .transcript
                .push(TranscriptEntry::new(TranscriptRole::System, text));
            state.scroll_from_bottom = 0;
        }
    }
    pub fn set_notice(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.set_notice(text);
        }
    }
    pub fn take_interrupted(&self) -> bool {
        self.interrupted.swap(false, Ordering::Relaxed)
    }
    pub fn take_queued(&self) -> Vec<String> {
        self.queued
            .lock()
            .map(|mut queued| std::mem::take(&mut *queued))
            .unwrap_or_default()
    }
    pub fn draft(&self) -> Option<String> {
        self.state
            .lock()
            .ok()
            .and_then(|state| (!state.composer.is_empty()).then(|| state.composer.iter().collect()))
    }
    pub fn choose(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Option<usize> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = self.state.lock() {
            state.dialog = Some(DialogState::new_choice(title, body, options, None, reply));
        } else {
            return None;
        }
        match response.recv() {
            Ok(DialogAnswer::Choice(choice)) => Some(choice),
            _ => None,
        }
    }
    pub fn choose_many(
        &self,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Vec<usize> {
        let (reply, response) = mpsc::channel();
        let checked = vec![false; options.len()];
        if let Ok(mut state) = self.state.lock() {
            state.dialog = Some(DialogState::new_choice(
                title,
                body,
                options,
                Some(checked),
                reply,
            ));
        } else {
            return Vec::new();
        }
        match response.recv() {
            Ok(DialogAnswer::Choices(choices)) => choices,
            _ => Vec::new(),
        }
    }
    pub fn prompt_text(&self, title: impl Into<String>, body: impl Into<String>) -> Option<String> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = self.state.lock() {
            state.dialog = Some(DialogState::new_text(title, body, reply));
        } else {
            return None;
        }
        match response.recv() {
            Ok(DialogAnswer::Text(text)) if !text.trim().is_empty() => Some(text),
            _ => None,
        }
    }
}

impl ChatViewState {
    pub fn from_session(session: &InteractiveSession) -> Self {
        let mut state = Self {
            model: active_model(&session.config.ai_provider, &session.config).to_owned(),
            provider: format_provider_display_name(&session.config.ai_provider, &session.config),
            workspace: format_workspace_with_branch(&session.current_dir),
            current_dir: session.current_dir.clone(),
            plan_mode: session.plan_mode,
            history: session.history.clone(),
            ..Self::default()
        };
        state.reload_transcript(&session.chat_id, &session.current_dir);
        state
    }
    pub fn set_draft(&mut self, draft: String) {
        self.composer = draft.chars().collect();
        self.cursor = self.composer.len();
    }
    pub fn reload_transcript(&mut self, chat_id: &str, workspace: &Path) {
        self.transcript.clear();
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));
        if let Ok(memory) = mint_core::MemoryStore::open_default()
            && let Ok(rows) = memory.interactions_for_chat(&scoped)
        {
            for row in rows {
                self.transcript
                    .push(TranscriptEntry::new(TranscriptRole::User, row.user_text));
                self.transcript
                    .push(TranscriptEntry::new(TranscriptRole::Assistant, row.ai_text));
            }
        }
    }
    pub fn push_user(&mut self, text: String) {
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::User, text));
        self.scroll_from_bottom = 0;
    }
    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.scroll_from_bottom = 0;
        self.selection_mode = false;
    }
    pub fn sync_session(&mut self, session: &InteractiveSession) {
        self.model = active_model(&session.config.ai_provider, &session.config).to_owned();
        self.provider = format_provider_display_name(&session.config.ai_provider, &session.config);
        self.workspace = format_workspace_with_branch(&session.current_dir);
        self.current_dir = session.current_dir.clone();
        self.plan_mode = session.plan_mode;
        self.history = session.history.clone();
    }
    fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.draft_before_history = self.composer.clone();
        }
        let index = self
            .history_index
            .map(|index| index.saturating_sub(1))
            .unwrap_or(self.history.len() - 1);
        self.history_index = Some(index);
        self.composer = self.history[index].chars().collect();
        self.cursor = self.composer.len();
    }
    fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            let next = index + 1;
            self.history_index = Some(next);
            self.composer = self.history[next].chars().collect();
        } else {
            self.history_index = None;
            self.composer = std::mem::take(&mut self.draft_before_history);
        }
        self.cursor = self.composer.len();
    }
    fn suggestions(&self) -> Vec<(String, String)> {
        let cursor = self.cursor.min(self.composer.len());
        let before: String = self.composer[..cursor].iter().collect();
        if before.starts_with('/') && !before.contains(char::is_whitespace) {
            return super::SLASH_COMMANDS
                .iter()
                .filter(|command| command.token.starts_with(&before))
                .map(|command| (command.token.clone(), command.description.clone()))
                .collect();
        }
        if before.starts_with('$') && !before.contains(char::is_whitespace) {
            let prefix = before[1..].to_lowercase();
            return super::load_all_available_skills(&self.current_dir)
                .into_iter()
                .filter(|skill| skill.name.to_lowercase().starts_with(&prefix))
                .map(|skill| {
                    (
                        format!("${}", skill.name),
                        skill.description.unwrap_or_default(),
                    )
                })
                .collect();
        }
        let start = self.composer[..cursor]
            .iter()
            .rposition(|c| c.is_whitespace())
            .map_or(0, |index| index + 1);
        if self.composer.get(start) == Some(&'@') {
            let query: String = self.composer[start..cursor].iter().collect();
            return super::collect_mention_candidates(&query, &self.current_dir)
                .into_iter()
                .map(|candidate| (candidate.label, candidate.description))
                .collect();
        }
        Vec::new()
    }
    fn apply_selected_suggestion(&mut self) -> bool {
        let suggestions = self.suggestions();
        let Some((token, _)) = suggestions.get(self.slash_selected % suggestions.len().max(1))
        else {
            return false;
        };
        let cursor = self.cursor.min(self.composer.len());
        let start = if token.starts_with('@') {
            self.composer[..cursor]
                .iter()
                .rposition(|c| c.is_whitespace())
                .map_or(0, |index| index + 1)
        } else {
            0
        };
        let end = self.composer[cursor..]
            .iter()
            .position(|c| c.is_whitespace())
            .map_or(self.composer.len(), |offset| cursor + offset);
        let needs_space = end == self.composer.len();
        let replacement = if needs_space {
            format!("{token} ")
        } else {
            token.clone()
        };
        self.composer.splice(start..end, replacement.chars());
        self.cursor = start + token.chars().count() + 1;
        self.slash_selected = 0;
        true
    }
    pub fn set_notice(&mut self, text: impl Into<String>) {
        self.notice = Some((text.into(), std::time::Instant::now()));
    }
    pub fn clear_notice(&mut self) {
        self.notice = None;
    }
    pub fn active_notice(&self) -> Option<&str> {
        self.notice.as_ref().and_then(|(text, created)| {
            if text.starts_with("Terminal too small")
                || created.elapsed() < std::time::Duration::from_millis(2500)
            {
                Some(text.as_str())
            } else {
                None
            }
        })
    }
    pub fn notice(&mut self, text: impl Into<String>) {
        self.set_notice(text);
    }
    fn transcript_text(&self) -> Text<'static> {
        let mut lines = Vec::new();
        let selected_start = self.selection_anchor.min(self.selection_head);
        let selected_end = self.selection_anchor.max(self.selection_head);
        let mut line_index = 0usize;
        for entry in &self.transcript {
            let role_header = match entry.role {
                TranscriptRole::User => Some(("You", Color::Cyan)),
                TranscriptRole::Assistant => Some(("Mint", Color::Green)),
                TranscriptRole::Notice => None,
                TranscriptRole::Command => Some(("Command", Color::Magenta)),
                TranscriptRole::System => Some(("System", Color::Yellow)),
            };
            if let Some((label, color)) = role_header {
                let selected =
                    self.selection_mode && (selected_start..=selected_end).contains(&line_index);
                lines.push(Line::from(Span::styled(
                    format!("{label} ›"),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(if selected {
                            Modifier::REVERSED
                        } else {
                            Modifier::empty()
                        }),
                )));
                line_index += 1;
            }
            for content_line in &entry.rendered_lines {
                let selected =
                    self.selection_mode && (selected_start..=selected_end).contains(&line_index);
                let mut sel_line = content_line.clone();
                if entry.role == TranscriptRole::Notice {
                    for span in &mut sel_line.spans {
                        if span.style.fg.is_none() {
                            span.style = span.style.fg(Color::White);
                        }
                    }
                }
                // Short AI text replies have no explicit ANSI color — default
                // them to plain white so they don't appear in the terminal's
                // dim default foreground.
                if entry.role == TranscriptRole::Assistant {
                    for span in &mut sel_line.spans {
                        if span.style.fg.is_none() {
                            span.style = span.style.fg(Color::White);
                        }
                    }
                }
                if selected {
                    for span in &mut sel_line.spans {
                        span.style = span.style.add_modifier(Modifier::REVERSED);
                    }
                }
                lines.push(sel_line);
                line_index += 1;
            }
            let selected =
                self.selection_mode && (selected_start..=selected_end).contains(&line_index);
            lines.push(Line::styled(
                "",
                if selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                },
            ));
            line_index += 1;
        }
        Text::from(lines)
    }
    fn plain_transcript_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for entry in &self.transcript {
            if let Some(header) = match entry.role {
                TranscriptRole::User => Some("You ›"),
                TranscriptRole::Assistant => Some("Mint ›"),
                TranscriptRole::Notice => None,
                TranscriptRole::Command => Some("Command ›"),
                TranscriptRole::System => Some("System ›"),
            } {
                lines.push(header.to_owned());
            }
            lines.extend(entry.plain_lines.iter().cloned());
            lines.push(String::new());
        }
        lines
    }
    fn selected_text(&self) -> String {
        let lines = self.plain_transcript_lines();
        if lines.is_empty() {
            return String::new();
        }
        let start = self
            .selection_anchor
            .min(self.selection_head)
            .min(lines.len() - 1);
        let end = self
            .selection_anchor
            .max(self.selection_head)
            .min(lines.len() - 1);
        lines[start..=end].join("\n")
    }
    fn render(&self, frame: &mut ratatui::Frame<'_>) -> Option<Rect> {
        let suggestions = self.suggestions();
        let suggestion_height = if suggestions.is_empty() {
            0
        } else {
            (suggestions.len().min(5) as u16) + 1
        };
        let status_height = if self.status.is_empty() {
            0
        } else {
            (self.status.len() as u16).min(8)
        };
        fn gradient_logo_line(text: &str) -> Line<'static> {
            let chars: Vec<char> = text.chars().collect();
            let count = chars.len();
            if count == 0 {
                return Line::default();
            }
            // Gradient stops: Mint Green (105, 230, 166) -> Sky Blue (72, 202, 228) -> Deep Blue (0, 119, 182)
            let stops = [
                (105.0, 230.0, 166.0),
                (72.0, 202.0, 228.0),
                (0.0, 119.0, 182.0),
            ];
            let spans: Vec<Span<'static>> = chars
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    if c == ' ' {
                        Span::raw(" ")
                    } else {
                        let t = if count > 1 {
                            i as f32 / (count - 1) as f32
                        } else {
                            0.0
                        };
                        let (r, g, b) = if t <= 0.5 {
                            let local_t = t * 2.0;
                            let r = stops[0].0 + (stops[1].0 - stops[0].0) * local_t;
                            let g = stops[0].1 + (stops[1].1 - stops[0].1) * local_t;
                            let b = stops[0].2 + (stops[1].2 - stops[0].2) * local_t;
                            (r, g, b)
                        } else {
                            let local_t = (t - 0.5) * 2.0;
                            let r = stops[1].0 + (stops[2].0 - stops[1].0) * local_t;
                            let g = stops[1].1 + (stops[2].1 - stops[1].1) * local_t;
                            let b = stops[1].2 + (stops[2].2 - stops[1].2) * local_t;
                            (r, g, b)
                        };
                        Span::styled(
                            c.to_string(),
                            Style::default().fg(Color::Rgb(
                                r.round() as u8,
                                g.round() as u8,
                                b.round() as u8,
                            )),
                        )
                    }
                })
                .collect();
            Line::from(spans)
        }
        /// Animated shimmer for the “Thinking (5s · …)” status line.
        ///
        /// The bright spot travels left-to-right across the text: the character at
        /// `spot` (modulo text length) is rendered at full bright white, characters
        /// nearby fade in/out using a cosine envelope, and the rest are a dim gray.
        fn shimmer_thinking_line(line: &str, tick: usize) -> Line<'static> {
            // Split at the first " (" to isolate the verb from the timer suffix.
            let (verb, suffix) = if let Some(idx) = line.find(" (") {
                (&line[..idx], &line[idx..])
            } else {
                (line, "")
            };

            let chars: Vec<char> = verb.chars().collect();
            let count = chars.len();

            // Bright spot position — full cycle every ~(count * 2) ticks so the
            // shimmer takes a moment to complete a pass rather than zipping by.
            let period = (count * 2).max(16);
            let spot = (tick % period) as f32 / period as f32 * count as f32;

            let mut spans: Vec<Span<'static>> = chars
                .iter()
                .enumerate()
                .map(|(i, &c)| {
                    // Distance from the shimmer spot, normalised.
                    let dist = ((i as f32 - spot).abs() / (count as f32 * 0.35)).min(1.0);
                    // Cosine envelope: 1.0 at centre of spot, 0.0 at edges.
                    let brightness = ((1.0 - dist) * std::f32::consts::PI * 0.5).cos().powi(2);
                    // Interpolate: dim gray (100, 100, 110) -> bright white (255, 255, 255).
                    let r = (100.0 + 155.0 * brightness).round() as u8;
                    let g = (100.0 + 155.0 * brightness).round() as u8;
                    let b = (110.0 + 145.0 * brightness).round() as u8;
                    let style = Style::default()
                        .fg(Color::Rgb(r, g, b))
                        .add_modifier(Modifier::BOLD);
                    Span::styled(c.to_string(), style)
                })
                .collect();

            if !suffix.is_empty() {
                spans.push(Span::styled(
                    suffix.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        }

        fn format_tool_status_line(line: &str) -> Line<'static> {
            if line.trim_start().starts_with('│') {
                return Line::styled(line.to_string(), Style::default().fg(Color::DarkGray));
            }

            let (prefix, rest) = if let Some(branch_pos) = line.find('└') {
                let conn_end = branch_pos + '└'.len_utf8();
                (&line[..conn_end], line[conn_end..].trim_start())
            } else if line.starts_with("      ") {
                ("      ", &line[6..])
            } else {
                ("", line)
            };

            let mut spans = Vec::new();
            if !prefix.is_empty() {
                spans.push(Span::styled(
                    if prefix.contains('└') {
                        "    └ ".to_string()
                    } else {
                        "      ".to_string()
                    },
                    Style::default().fg(Color::DarkGray),
                ));
            }

            const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let first_char = rest.chars().next();
            let (icon_span, after_icon) = if first_char == Some('✓') {
                let span = Span::styled(
                    "✓ ".to_string(),
                    Style::default().fg(Color::Rgb(105, 230, 166)),
                );
                (Some(span), rest['✓'.len_utf8()..].trim_start())
            } else if let Some(c) = first_char
                && SPINNER_CHARS.contains(&c)
            {
                let span = Span::styled(
                    format!("{} ", c),
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                );
                (Some(span), rest[c.len_utf8()..].trim_start())
            } else {
                (None, rest)
            };

            if let Some(s) = icon_span {
                spans.push(s);
            }

            if let Some(timer_pos) = after_icon.rfind(" (") {
                let text = &after_icon[..timer_pos];
                let timer = &after_icon[timer_pos..];
                spans.push(Span::styled(
                    text.to_string(),
                    Style::default().fg(Color::White),
                ));
                spans.push(Span::styled(
                    timer.to_string(),
                    Style::default().fg(Color::DarkGray),
                ));
            } else {
                spans.push(Span::styled(
                    after_icon.to_string(),
                    Style::default().fg(Color::White),
                ));
            }

            Line::from(spans)
        }

        fn format_composer_row_spans(row: &str) -> Vec<Span<'static>> {
            let mut spans = Vec::new();
            let mut i = 0;
            let chars: Vec<char> = row.chars().collect();
            let mut last_end = 0;

            while i < chars.len() {
                if chars[i] == '[' {
                    let remaining: String = chars[i..].iter().collect();
                    if remaining.starts_with("[Image") || remaining.starts_with("[Pasted text") {
                        if let Some(end) = remaining.find(']') {
                            let badge = &remaining[..=end];
                            if i > last_end {
                                let text: String = chars[last_end..i].iter().collect();
                                spans.push(Span::raw(text));
                            }
                            spans.push(Span::styled(
                                badge.to_string(),
                                Style::default()
                                    .fg(Color::Cyan)
                                    .add_modifier(Modifier::BOLD),
                            ));
                            i += end + 1;
                            last_end = i;
                            continue;
                        }
                    }
                }
                i += 1;
            }
            if last_end < chars.len() {
                let text: String = chars[last_end..].iter().collect();
                spans.push(Span::raw(text));
            }
            spans
        }

        let horizontal_pad = if frame.area().width >= 80 {
            2
        } else if frame.area().width >= 60 {
            1
        } else {
            0
        };
        let main_area = if horizontal_pad > 0 {
            Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(horizontal_pad),
                    Constraint::Min(0),
                    Constraint::Length(horizontal_pad),
                ])
                .split(frame.area())[1]
        } else {
            frame.area()
        };

        let dialog_height = if let Some(dialog) = &self.dialog {
            let filtered_len = dialog.filtered_indices().len();
            let body_lines = if dialog.body.is_empty() {
                0
            } else {
                dialog
                    .body
                    .lines()
                    .map(|line| {
                        Paragraph::new(format!(" {line}"))
                            .wrap(Wrap { trim: false })
                            .line_count(main_area.width.max(1))
                    })
                    .sum()
            };
            let filter_lines = if dialog.filter.is_empty() { 0 } else { 1 };
            let content_lines = if dialog.input.is_some() {
                3
            } else {
                filtered_len.max(1) + 3
            };
            // The top border consumes a row in addition to the title, body,
            // spacer, options, and footer counted above.
            let total = 1 + 1 + body_lines + filter_lines + 1 + content_lines;
            let max_allowed = main_area.height.saturating_sub(7);
            (total as u16).min(max_allowed)
        } else {
            0
        };

        let thought_height = if self.thought_modal.is_some() {
            let max_allowed = main_area.height.saturating_sub(9).min(20).max(8);
            ((main_area.height * 4) / 10).clamp(8, max_allowed)
        } else {
            0
        };

        let composer_width = main_area.width.saturating_sub(6).max(1) as usize;
        let (composer_rows, cursor_row, cursor_col) =
            super::wrap_input_into_rows(&self.composer, composer_width, self.cursor);
        let composer_height = (composer_rows.len().max(1) as u16 + 2).clamp(3, 8);
        let rows = if dialog_height > 0 {
            let dialog_status_height =
                status_height.min(main_area.height.saturating_sub(6 + 1 + dialog_height));
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Min(1),
                    Constraint::Length(dialog_status_height),
                    Constraint::Length(dialog_height),
                ])
                .split(main_area)
        } else if thought_height > 0 {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Min(3),
                    Constraint::Length(status_height),
                    Constraint::Length(thought_height),
                ])
                .split(main_area)
        } else {
            Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(6),
                    Constraint::Min(3),
                    Constraint::Length(status_height),
                    Constraint::Length(suggestion_height),
                    Constraint::Length(composer_height),
                    Constraint::Length(1),
                ])
                .split(main_area)
        };

        let line1_text = format!(
            "[Mint] v{} | Active AI: {}",
            env!("CARGO_PKG_VERSION"),
            self.provider
        );
        let now = chrono::Local::now();
        let buddhist_year = now.format("%Y").to_string().parse::<i32>().unwrap_or(2026) + 543;
        let line2_text = format!(
            "{}/{:02}/{} {:02}:{:02} • {}",
            now.format("%d"),
            now.format("%m"),
            buddhist_year,
            now.format("%H"),
            now.format("%M"),
            self.model
        );
        let content_width = line1_text.chars().count().max(line2_text.chars().count());
        let box_width = (content_width + 4) as u16;

        let header_columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(37),
                Constraint::Length(box_width),
                Constraint::Min(0),
            ])
            .split(rows[0]);
        let logo = Text::from(vec![
            gradient_logo_line(" __  __ _       _    ___ _    ___ "),
            gradient_logo_line(r"|  \/  (_)_ __ | |_ / __| |  |_ _|"),
            gradient_logo_line(r"| |\/| | | '_ \|  _| (__| |__ | | "),
            gradient_logo_line(r"|_|  |_|_|_| |_|\__|\___|\___|___|"),
        ]);
        frame.render_widget(Paragraph::new(logo), header_columns[0]);

        let details = Text::from(vec![
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "[Mint] ",
                    Style::default()
                        .fg(Color::Rgb(105, 230, 166))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(
                        "v{} | Active AI: {}",
                        env!("CARGO_PKG_VERSION"),
                        self.provider
                    ),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw(" "),
                Span::styled(line2_text, Style::default().fg(Color::DarkGray)),
            ]),
        ]);
        let details_area = ratatui::layout::Rect {
            height: 4,
            ..header_columns[1]
        };
        frame.render_widget(
            Paragraph::new(details).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Color::DarkGray)),
            ),
            details_area,
        );
        let help_area = ratatui::layout::Rect {
            y: rows[0].y + 4,
            height: 1,
            ..rows[0]
        };
        frame.render_widget(
            Paragraph::new("Type naturally or /help for commands. F6: Classic CLI. Ctrl+D exits."),
            help_area,
        );

        let text = self.transcript_text();
        let total = Paragraph::new(text.clone())
            .wrap(Wrap { trim: false })
            .line_count(rows[1].width.max(1));
        let max_scroll = total.saturating_sub(rows[1].height as usize) as u16;
        let scroll = max_scroll.saturating_sub(self.scroll_from_bottom.min(max_scroll));
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((scroll, 0)),
            rows[1],
        );
        if max_scroll > 0 {
            let mut bar = ScrollbarState::new(total).position(scroll as usize);
            frame.render_stateful_widget(
                Scrollbar::new(ScrollbarOrientation::VerticalRight),
                rows[1],
                &mut bar,
            );
        }
        let back_to_bottom_area = if self.dialog.is_none()
            && self.thought_modal.is_none()
            && self.scroll_from_bottom.min(max_scroll) > 0
            && rows[1].height > 0
            && rows[1].width >= UnicodeWidthStr::width(BACK_TO_BOTTOM_LABEL) as u16 + 2
        {
            let width = UnicodeWidthStr::width(BACK_TO_BOTTOM_LABEL) as u16;
            let area = Rect::new(
                rows[1].x + (rows[1].width - width) / 2,
                rows[1].bottom() - 1,
                width,
                1,
            );
            frame.render_widget(
                Paragraph::new(BACK_TO_BOTTOM_LABEL).style(
                    Style::default()
                        .fg(Color::Rgb(105, 230, 166))
                        .bg(Color::Rgb(38, 48, 45))
                        .add_modifier(Modifier::BOLD),
                ),
                area,
            );
            Some(area)
        } else {
            None
        };
        if status_height > 0 {
            // Derive a tick counter from wall-clock time (ms / 80) so the
            // shimmer animation advances at ~12 fps regardless of the 30 ms
            // render loop frequency.
            let tick = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as usize
                / 80;

            let status_lines: Vec<Line> = self
                .status
                .iter()
                .map(|line| {
                    // Lines with a timing suffix: "Verb (elapsed · …)" —
                    // shimmer animation from bright white (left) to dim (right).
                    if line.find(" (").is_some()
                        && !line.contains('└')
                        && !line.starts_with("      ")
                        && !line.contains('✓')
                        && !line.chars().any(|c| {
                            ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'].contains(&c)
                        })
                    {
                        shimmer_thinking_line(line, tick)
                    // Tool status lines (connected by └ or indented by 6 spaces)
                    } else if line.contains('└') || line.starts_with("      ") {
                        format_tool_status_line(line)
                    } else if line.contains('●') || line.contains('○') {
                        // Header line with bullet: bullet in Cyan bold, text in White bold
                        let bullet_char = if line.contains('●') { '●' } else { '○' };
                        if let Some(pos) = line.find(bullet_char) {
                            let prefix = &line[..pos];
                            let text = &line[pos + bullet_char.len_utf8()..];
                            Line::from(vec![
                                Span::raw(prefix.to_string()),
                                Span::styled(
                                    bullet_char.to_string(),
                                    Style::default()
                                        .fg(Color::Cyan)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    text.to_string(),
                                    Style::default()
                                        .fg(Color::White)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ])
                        } else {
                            Line::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(Color::White)
                                    .add_modifier(Modifier::BOLD),
                            )
                        }
                    } else {
                        // Activity/verb lines: "Reading 1 file…", "Running 2
                        // commands…", etc. — bold white for clarity.
                        Line::styled(
                            line.to_string(),
                            Style::default()
                                .fg(Color::White)
                                .add_modifier(Modifier::BOLD),
                        )
                    }
                })
                .collect();
            let skip = status_lines.len().saturating_sub(rows[2].height as usize);
            let visible_lines: Vec<Line> = status_lines.into_iter().skip(skip).collect();
            frame.render_widget(Paragraph::new(visible_lines), rows[2]);
        }
        if let Some(dialog) = &self.dialog {
            let mut lines = Vec::new();
            let filtered = dialog.filtered_indices();
            let total_options = filtered.len();

            let title_suffix = if dialog.input.is_none() && !dialog.options.is_empty() {
                format!(
                    " ({}/{})",
                    (dialog.selected + 1).min(total_options),
                    total_options
                )
            } else {
                String::new()
            };

            lines.push(Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    &dialog.title,
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(title_suffix, Style::default().fg(Color::DarkGray)),
            ]));

            if !dialog.body.is_empty() {
                for line in dialog.body.lines() {
                    lines.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(line.to_owned(), Style::default().fg(Color::DarkGray)),
                    ]));
                }
            }

            let mut filter_row_idx = None;
            if !dialog.filter.is_empty() {
                filter_row_idx = Some(lines.len());
                let filter_str: String = dialog.filter.iter().collect();
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled("Filter: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        filter_str,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({total_options}/{})", dialog.options.len()),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
            }

            lines.push(Line::raw(""));

            let mut input_cursor = None;
            if let Some(input) = &dialog.input {
                let input_text: String = input.iter().collect();
                let input_row_idx = lines.len();
                lines.push(Line::from(vec![
                    Span::styled(
                        " › ",
                        Style::default()
                            .fg(Color::Rgb(105, 230, 166))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(input_text.clone()),
                    Span::styled("█", Style::default().fg(Color::Rgb(105, 230, 166))),
                ]));
                lines.push(Line::raw(""));
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        "Enter submit · Esc cancel",
                        Style::default().fg(Color::DarkGray),
                    ),
                ]));
                let visual_col = crate::markdown::unicode_width(&input_text);
                input_cursor = Some((visual_col, input_row_idx));
            } else {
                let header_height = Paragraph::new(lines.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(rows[3].width.max(1));
                let footer_height = 3; // blank + hint + bottom padding
                let inner_height = (rows[3].height as usize).saturating_sub(1);
                let available_rows = inner_height.saturating_sub(header_height + footer_height);
                // Show every choice when it fits. Reserve space for both
                // scroll indicators only when the list exceeds the viewport.
                let visible_rows = if total_options > available_rows {
                    available_rows.saturating_sub(2).max(1)
                } else {
                    available_rows.max(1)
                };

                let scroll_offset = if dialog.selected < dialog.scroll_offset {
                    dialog.selected
                } else if dialog.selected >= dialog.scroll_offset + visible_rows {
                    dialog.selected + 1 - visible_rows
                } else {
                    dialog.scroll_offset
                };
                let max_scroll = total_options.saturating_sub(visible_rows);
                let scroll_offset = scroll_offset.min(max_scroll);

                if total_options == 0 {
                    lines.push(Line::from(vec![
                        Span::raw("   "),
                        Span::styled(
                            "(No matching options)",
                            Style::default().fg(Color::DarkGray),
                        ),
                    ]));
                } else {
                    if scroll_offset > 0 {
                        lines.push(Line::from(vec![
                            Span::raw("   "),
                            Span::styled(
                                format!("▲ {scroll_offset} more above"),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }

                    let end_idx = (scroll_offset + visible_rows).min(total_options);
                    for idx in scroll_offset..end_idx {
                        let original_idx = filtered[idx];
                        let option_text = &dialog.options[original_idx];
                        let is_selected = idx == dialog.selected;

                        let shortcut = if dialog.filter.is_empty() && idx < 9 {
                            format!("{}. ", idx + 1)
                        } else {
                            "   ".to_string()
                        };

                        let marker = match &dialog.checked {
                            Some(checked)
                                if checked.get(original_idx).copied().unwrap_or(false) =>
                            {
                                "[✓] "
                            }
                            Some(_) => "[ ] ",
                            None => "",
                        };

                        let (label, desc) = if let Some((l, d)) = option_text.split_once(" — ") {
                            (l, Some(d))
                        } else if let Some((l, d)) = option_text.split_once('\t') {
                            (l, Some(d))
                        } else {
                            (option_text.as_str(), None)
                        };

                        if is_selected {
                            let mut spans = vec![
                                Span::styled(
                                    " › ",
                                    Style::default()
                                        .fg(Color::Rgb(105, 230, 166))
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    shortcut,
                                    Style::default()
                                        .fg(Color::Rgb(105, 230, 166))
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    marker,
                                    Style::default()
                                        .fg(Color::Rgb(105, 230, 166))
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    label,
                                    Style::default()
                                        .fg(Color::Rgb(105, 230, 166))
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ];
                            if let Some(desc) = desc {
                                spans.push(Span::styled("   ", Style::default()));
                                spans
                                    .push(Span::styled(desc, Style::default().fg(Color::DarkGray)));
                            }
                            lines.push(Line::from(spans));
                        } else {
                            let mut spans = vec![
                                Span::raw("   "),
                                Span::styled(shortcut, Style::default().fg(Color::DarkGray)),
                                Span::styled(marker, Style::default().fg(Color::DarkGray)),
                                Span::styled(label, Style::default().fg(Color::White)),
                            ];
                            if let Some(desc) = desc {
                                spans.push(Span::styled("   ", Style::default()));
                                spans
                                    .push(Span::styled(desc, Style::default().fg(Color::DarkGray)));
                            }
                            lines.push(Line::from(spans));
                        }
                    }

                    let remaining = total_options.saturating_sub(end_idx);
                    if remaining > 0 {
                        lines.push(Line::from(vec![
                            Span::raw("   "),
                            Span::styled(
                                format!("▼ {remaining} more below"),
                                Style::default().fg(Color::DarkGray),
                            ),
                        ]));
                    }
                }

                lines.push(Line::raw(""));
                let hint = if dialog.checked.is_some() {
                    " Space toggle · Enter confirm · 1-9 pick · Type to filter · Esc cancel"
                } else {
                    " ↑/↓ navigate · 1-9 pick · Type to filter · Enter select · Esc cancel"
                };
                lines.push(Line::styled(hint, Style::default().fg(Color::DarkGray)));
                lines.push(Line::raw(""));
            }

            frame.render_widget(
                Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .borders(Borders::TOP)
                        .border_style(Style::default().fg(Color::DarkGray)),
                ),
                rows[3],
            );

            if let Some((visual_col, row_idx)) = input_cursor {
                frame.set_cursor_position(Position::new(
                    rows[3].x + 3 + visual_col as u16,
                    rows[3].y + 1 + row_idx as u16,
                ));
            } else if let Some(row_idx) = filter_row_idx {
                let filter_str: String = dialog.filter.iter().collect();
                let visual_col = crate::markdown::unicode_width(&filter_str);
                frame.set_cursor_position(Position::new(
                    rows[3].x + 1 + 8 + visual_col as u16,
                    rows[3].y + 1 + row_idx as u16,
                ));
            }
        } else if let Some(modal) = &self.thought_modal {
            let panel_area = rows[3];
            let content_width = (panel_area.width as usize).saturating_sub(4).max(20);
            let mut wrapped_lines: Vec<String> = Vec::new();
            for paragraph in &modal.lines {
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

            let inner_height = (panel_area.height as usize).saturating_sub(2);
            let max_scroll = wrapped_lines.len().saturating_sub(inner_height);
            let scroll = modal.scroll.min(max_scroll);

            let scroll_hint = if max_scroll > 0 {
                format!(
                    " ↑/↓ scroll {}/{} · Esc / Ctrl+T to close ",
                    scroll + 1,
                    max_scroll + 1
                )
            } else {
                " Esc / Ctrl+T to close ".to_string()
            };

            let block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(Color::Rgb(168, 85, 247)))
                .title(Span::styled(
                    format!(" Thought Process ({}) ", modal.elapsed_str),
                    Style::default()
                        .fg(Color::Rgb(192, 132, 252))
                        .add_modifier(Modifier::BOLD),
                ))
                .title_bottom(Span::styled(
                    scroll_hint,
                    Style::default().fg(Color::DarkGray),
                ));

            let visible = &wrapped_lines[scroll..(scroll + inner_height).min(wrapped_lines.len())];
            let mut p_lines: Vec<Line> = Vec::with_capacity(inner_height);
            for l in visible {
                p_lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(l.clone(), Style::default().fg(Color::Rgb(226, 232, 240))),
                ]));
            }
            for _ in visible.len()..inner_height {
                p_lines.push(Line::from(""));
            }

            let paragraph = Paragraph::new(p_lines).block(block);
            frame.render_widget(paragraph, panel_area);
        } else {
            if suggestion_height > 0 {
                let selected = self.slash_selected.min(suggestions.len() - 1);
                let page_start = (selected / 5) * 5;
                let lines = std::iter::once(Line::styled(
                    "Suggestions · ↑/↓ select · Tab complete · Enter send",
                    Style::default().fg(Color::DarkGray),
                ))
                .chain(suggestions.iter().enumerate().skip(page_start).take(5).map(
                    |(index, (token, description))| {
                        Line::styled(
                            format!(
                                "{} {:<18} {description}",
                                if index == selected { "›" } else { " " },
                                token
                            ),
                            if index == selected {
                                Style::default()
                                    .fg(Color::Green)
                                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                            } else {
                                Style::default().fg(Color::DarkGray)
                            },
                        )
                    },
                ))
                .collect::<Vec<_>>();
                frame.render_widget(Paragraph::new(lines), rows[3]);
            }
            let input = if self.composer.is_empty() {
                Text::from(Line::from(vec![
                    Span::styled(
                        " › ",
                        Style::default()
                            .fg(Color::Rgb(105, 230, 166))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled("Ask anything...", Style::default().fg(Color::DarkGray)),
                ]))
            } else {
                Text::from(
                    composer_rows
                        .iter()
                        .enumerate()
                        .map(|(index, row)| {
                            let mut spans = vec![Span::styled(
                                if index == 0 { " › " } else { "   " },
                                Style::default()
                                    .fg(Color::Rgb(105, 230, 166))
                                    .add_modifier(Modifier::BOLD),
                            )];
                            spans.extend(format_composer_row_spans(row));
                            Line::from(spans)
                        })
                        .collect::<Vec<_>>(),
                )
            };
            frame.render_widget(
                Paragraph::new(input).wrap(Wrap { trim: false }).block(
                    Block::default()
                        .borders(Borders::TOP | Borders::BOTTOM)
                        .border_style(Style::default().fg(Color::DarkGray)),
                ),
                rows[4],
            );

            let mode_label = if self.plan_mode {
                " [Plan] "
            } else {
                " [Agent] "
            };
            let bg_running = mint_core::bg_shell::running_count();
            let jobs_prefix = if bg_running > 0 {
                format!(
                    "{} bg shell{} · ",
                    bg_running,
                    if bg_running == 1 { "" } else { "s" }
                )
            } else {
                String::new()
            };
            let path_text = format!("path: {}", self.workspace);
            let left_len = mode_label.chars().count() + self.model.chars().count();
            let right_len = jobs_prefix.chars().count() + path_text.chars().count();
            let available = rows[5].width as usize;
            let gap = available.saturating_sub(left_len + right_len);
            let footer = if let Some(notice) = self.active_notice() {
                Line::styled(
                    format!(" {notice}"),
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD),
                )
            } else if self.selection_mode {
                Line::styled(
                    " Selection mode · ↑/↓ scroll · Enter/y copy · Esc cancel",
                    Style::default().fg(Color::Yellow),
                )
            } else {
                Line::from(vec![
                    Span::styled(mode_label, Style::default().fg(Color::DarkGray)),
                    Span::styled(
                        &self.model,
                        Style::default()
                            .fg(Color::Rgb(105, 230, 166))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" ".repeat(gap)),
                    Span::styled(jobs_prefix, Style::default().fg(Color::Blue)),
                    Span::styled(path_text, Style::default().fg(Color::DarkGray)),
                ])
            };
            frame.render_widget(Paragraph::new(footer), rows[5]);

            if !self.selection_mode {
                let row_start = self.cursor.saturating_sub(cursor_col);
                let visual_col = crate::markdown::unicode_width(
                    &self.composer[row_start..self.cursor]
                        .iter()
                        .collect::<String>(),
                );
                frame.set_cursor_position(Position::new(
                    rows[4].x + 3 + visual_col as u16,
                    rows[4].y
                        + 1
                        + cursor_row.min(composer_height.saturating_sub(3) as usize) as u16,
                ));
            }
        }
        back_to_bottom_area
    }
}

pub(crate) struct ChatTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
    mouse_selection: MouseSelection,
    last_frame: Option<Buffer>,
    back_to_bottom_area: Option<Rect>,
    clipboard: TextClipboard,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
struct ScreenPoint {
    row: u16,
    column: u16,
}

impl ScreenPoint {
    fn from_mouse(mouse: event::MouseEvent) -> Self {
        Self {
            row: mouse.row,
            column: mouse.column,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct MouseSelection {
    anchor: Option<ScreenPoint>,
    head: Option<ScreenPoint>,
    dragging: bool,
    had_drag: bool,
}

impl MouseSelection {
    fn start(&mut self, point: ScreenPoint) {
        self.anchor = Some(point);
        self.head = Some(point);
        self.dragging = true;
        self.had_drag = false;
    }

    fn update(&mut self, point: ScreenPoint) {
        if self.dragging {
            self.head = Some(point);
            self.had_drag = true;
        }
    }

    fn finish(&mut self, point: ScreenPoint) -> bool {
        if !self.dragging {
            return false;
        }
        self.head = Some(point);
        self.dragging = false;
        if self.had_drag {
            true
        } else {
            self.clear();
            false
        }
    }

    fn clear(&mut self) {
        *self = Self::default();
    }

    fn range(&self) -> Option<(ScreenPoint, ScreenPoint)> {
        let anchor = self.anchor?;
        let head = self.head?;
        Some(if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        })
    }
}

#[derive(Default)]
struct TextClipboard {
    native: Option<Box<dyn NativeClipboard>>,
    initialization_attempted: bool,
}

trait NativeClipboard {
    fn set_text(&mut self, text: String) -> std::result::Result<(), String>;
}

impl NativeClipboard for arboard::Clipboard {
    fn set_text(&mut self, text: String) -> std::result::Result<(), String> {
        arboard::Clipboard::set_text(self, text).map_err(|error| error.to_string())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CopyMethod {
    Native,
    Osc52,
}

impl TextClipboard {
    fn copy<W: Write>(&mut self, text: &str, writer: &mut W) -> io::Result<CopyMethod> {
        if !self.initialization_attempted {
            self.initialization_attempted = true;
            self.native = arboard::Clipboard::new()
                .ok()
                .map(|clipboard| Box::new(clipboard) as Box<dyn NativeClipboard>);
        }
        if let Some(clipboard) = self.native.as_mut()
            && clipboard.set_text(text.to_owned()).is_ok()
        {
            return Ok(CopyMethod::Native);
        }

        write!(writer, "\x1b]52;c;{}\x07", BASE64.encode(text.as_bytes()))?;
        writer.flush()?;
        Ok(CopyMethod::Osc52)
    }
}

fn apply_mouse_selection(buffer: &mut Buffer, selection: MouseSelection) {
    for selected in selected_rows(buffer.area, selection) {
        for column in selected.left..=selected.right {
            if let Some(cell) = buffer.cell_mut(Position::new(column, selected.row)) {
                cell.modifier.insert(Modifier::REVERSED);
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SelectedRow {
    row: u16,
    left: u16,
    right: u16,
}

fn selected_rows(area: Rect, selection: MouseSelection) -> Vec<SelectedRow> {
    let Some((start, end)) = selection.range() else {
        return Vec::new();
    };
    if area.is_empty() {
        return Vec::new();
    }
    let top = start.row.max(area.top());
    let bottom = end.row.min(area.bottom().saturating_sub(1));
    if top > bottom {
        return Vec::new();
    }

    (top..=bottom)
        .filter_map(|row| {
            let left = if row == start.row {
                start.column
            } else {
                area.left()
            }
            .max(area.left());
            let right = if row == end.row {
                end.column
            } else {
                area.right().saturating_sub(1)
            }
            .min(area.right().saturating_sub(1));
            (left <= right).then_some(SelectedRow { row, left, right })
        })
        .collect()
}

fn selected_screen_text(buffer: &Buffer, selection: MouseSelection) -> String {
    let mut lines = Vec::new();
    for selected in selected_rows(buffer.area, selection) {
        let mut line = String::new();
        let mut column = buffer.area.left();
        while column < buffer.area.right() {
            let Some(cell) = buffer.cell(Position::new(column, selected.row)) else {
                break;
            };
            let symbol = cell.symbol();
            let width = UnicodeWidthStr::width(symbol).max(1) as u16;
            let symbol_right = column.saturating_add(width.saturating_sub(1));
            if symbol_right >= selected.left && column <= selected.right {
                line.push_str(symbol);
            }
            column = column.saturating_add(width);
        }
        lines.push(line.trim_end_matches(' ').to_owned());
    }
    lines.join("\n")
}

fn selected_cells_match(previous: &Buffer, current: &Buffer, selection: MouseSelection) -> bool {
    if previous.area != current.area {
        return false;
    }
    let rows = selected_rows(current.area, selection);
    if selection.range().is_some() && rows.is_empty() {
        return false;
    }
    for selected in rows {
        for column in selected.left..=selected.right {
            let position = Position::new(column, selected.row);
            if previous.cell(position).map(|cell| cell.symbol())
                != current.cell(position).map(|cell| cell.symbol())
            {
                return false;
            }
        }
    }
    true
}

fn route_mouse_wheel(state: &mut ChatViewState, kind: event::MouseEventKind) {
    match kind {
        event::MouseEventKind::ScrollUp => {
            if let Some(modal) = state.thought_modal.as_mut() {
                modal.scroll = modal.scroll.saturating_sub(3);
            } else {
                state.scroll_from_bottom = state.scroll_from_bottom.saturating_add(3);
            }
        }
        event::MouseEventKind::ScrollDown => {
            if let Some(modal) = state.thought_modal.as_mut() {
                modal.scroll = modal.scroll.saturating_add(3);
            } else {
                state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(3);
            }
        }
        _ => {}
    }
}

fn activate_back_to_bottom(
    state: &mut ChatViewState,
    area: Option<Rect>,
    kind: event::MouseEventKind,
    point: ScreenPoint,
) -> bool {
    if kind == event::MouseEventKind::Down(event::MouseButton::Left)
        && area.is_some_and(|area| area.contains(Position::new(point.column, point.row)))
    {
        state.scroll_from_bottom = 0;
        true
    } else {
        false
    }
}

fn setup_terminal_output<W: Write>(stdout: &mut W) -> io::Result<()> {
    crossterm::execute!(
        stdout,
        terminal::EnterAlternateScreen,
        // Mouse capture lets Mint distinguish wheel scrolling from physical
        // Up/Down keys and powers Mint's own drag-to-copy selection layer.
        event::EnableMouseCapture,
        event::EnableBracketedPaste
    )
}

pub(crate) fn restore_terminal() {
    let mut stdout = io::stdout();
    let _ = stdout.execute(event::DisableBracketedPaste);
    let _ = stdout.execute(event::DisableMouseCapture);
    let _ = stdout.execute(terminal::LeaveAlternateScreen);
    let _ = terminal::disable_raw_mode();
}
impl ChatTui {
    pub fn enter() -> Result<Self> {
        use crossterm::tty::IsTty;
        if !io::stdin().is_tty() || !io::stdout().is_tty() {
            bail!("stdin/stdout is not a TTY");
        }
        let (w, h) = terminal::size()?;
        if w < MIN_WIDTH || h < MIN_HEIGHT {
            bail!("terminal is too small ({w}x{h}; need {MIN_WIDTH}x{MIN_HEIGHT})");
        }
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        let setup = setup_terminal_output(&mut stdout);
        if let Err(error) = setup {
            restore_terminal();
            return Err(error.into());
        }
        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => Ok(Self {
                terminal,
                active: true,
                mouse_selection: MouseSelection::default(),
                last_frame: None,
                back_to_bottom_area: None,
                clipboard: TextClipboard::default(),
            }),
            Err(error) => {
                restore_terminal();
                Err(error.into())
            }
        }
    }
    fn draw_state(&mut self, state: &ChatViewState) -> Result<()> {
        let selection = self.mouse_selection;
        let previous = self.last_frame.as_ref();
        let mut invalidated = false;
        let mut back_to_bottom_area = None;
        let completed = self.terminal.draw(|frame| {
            back_to_bottom_area = state.render(frame);
            if selection.range().is_some() {
                let area = frame.area();
                let hint_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
                frame.render_widget(
                    Paragraph::new(Line::styled(
                        " Right-click copy · Esc clear",
                        Style::default().fg(Color::DarkGray),
                    )),
                    hint_area,
                );
            }
            invalidated = !selection.dragging
                && selection.range().is_some()
                && previous
                    .map(|buffer| !selected_cells_match(buffer, frame.buffer_mut(), selection))
                    .unwrap_or(false);
            if !invalidated {
                apply_mouse_selection(frame.buffer_mut(), selection);
            }
        })?;
        self.last_frame = Some(completed.buffer.clone());
        self.back_to_bottom_area = back_to_bottom_area;
        if invalidated {
            self.mouse_selection.clear();
        }
        Ok(())
    }

    fn handle_selection_mouse(
        &mut self,
        mouse: event::MouseEvent,
        state: &mut ChatViewState,
    ) -> Result<bool> {
        use event::{MouseButton, MouseEventKind};

        let point = ScreenPoint::from_mouse(mouse);
        if activate_back_to_bottom(state, self.back_to_bottom_area, mouse.kind, point) {
            self.mouse_selection.clear();
            return Ok(true);
        }
        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                let inside_frame = self
                    .last_frame
                    .as_ref()
                    .map(|buffer| buffer.area.contains(Position::new(point.column, point.row)))
                    .unwrap_or(false);
                if inside_frame {
                    self.mouse_selection.start(point);
                    state.selection_mode = false;
                    state.clear_notice();
                } else {
                    self.mouse_selection.clear();
                }
                Ok(true)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.mouse_selection.update(point);
                Ok(self.mouse_selection.dragging)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if !self.mouse_selection.finish(point) {
                    return Ok(true);
                }
                if self
                    .last_frame
                    .as_ref()
                    .map(|buffer| {
                        selected_screen_text(buffer, self.mouse_selection)
                            .trim()
                            .is_empty()
                    })
                    .unwrap_or(true)
                {
                    self.mouse_selection.clear();
                }
                Ok(true)
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let text = self
                    .last_frame
                    .as_ref()
                    .map(|buffer| selected_screen_text(buffer, self.mouse_selection))
                    .unwrap_or_default();
                if text.trim().is_empty() {
                    return Ok(false);
                }
                if text.len() > COPY_LIMIT {
                    state.set_notice("Selection exceeds the 100 KiB copy limit");
                    return Ok(true);
                }
                let character_count = text.chars().count();
                match self.clipboard.copy(&text, &mut io::stdout()) {
                    Ok(CopyMethod::Native) => state
                        .set_notice(format!("Copied {character_count} chars to host clipboard")),
                    Ok(CopyMethod::Osc52) => {
                        state.set_notice(format!("Copied {character_count} chars with OSC52"))
                    }
                    Err(error) => state.set_notice(format!("Could not copy selection: {error}")),
                }
                self.mouse_selection.clear();
                Ok(true)
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.mouse_selection.clear();
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn clear_mouse_selection(&mut self) {
        self.mouse_selection.clear();
    }
    pub fn read_input(
        &mut self,
        shared_state: &Arc<Mutex<ChatViewState>>,
    ) -> Result<Option<InteractiveInput>> {
        let mut state = shared_state
            .lock()
            .map_err(|_| anyhow::anyhow!("TUI state lock poisoned"))?;
        let mut pasted_image: Option<String> = None;
        let mut last_ctrl_d: Option<std::time::Instant> = None;
        const DOUBLE_PRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

        loop {
            if let Some(time) = last_ctrl_d {
                if time.elapsed() >= DOUBLE_PRESS_TIMEOUT {
                    last_ctrl_d = None;
                    if state.active_notice() == Some("Press Ctrl+D again to exit") {
                        state.clear_notice();
                    }
                }
            }

            self.draw_state(&state)?;

            if last_ctrl_d.is_some() || state.active_notice().is_some() {
                if !event::poll(std::time::Duration::from_millis(50))? {
                    continue;
                }
            }

            match event::read()? {
                event::Event::Resize(w, h) => {
                    self.clear_mouse_selection();
                    if w < MIN_WIDTH || h < MIN_HEIGHT {
                        state.set_notice(format!("Terminal too small: {w}x{h}"));
                    } else if state
                        .active_notice()
                        .map(|n| n.starts_with("Terminal too small"))
                        .unwrap_or(false)
                    {
                        state.clear_notice();
                    }
                }
                event::Event::Mouse(m) => {
                    if self.handle_selection_mouse(m, &mut state)? {
                        continue;
                    }
                    route_mouse_wheel(&mut state, m.kind);
                }
                event::Event::Paste(text) => {
                    self.clear_mouse_selection();
                    for c in text.trim_end_matches(['\r', '\n']).chars() {
                        let cursor = state.cursor;
                        state.composer.insert(cursor, c);
                        state.cursor += 1;
                    }
                }
                event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    use event::{KeyCode, KeyModifiers};
                    self.clear_mouse_selection();

                    if key.code == KeyCode::F(6) {
                        let text: String = state.composer.iter().collect();
                        return Ok(Some(InteractiveInput {
                            text,
                            pasted_image,
                            switch_mode: true,
                        }));
                    }

                    if let Some(modal) = state.thought_modal.as_mut() {
                        match key.code {
                            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                                state.thought_modal = None;
                            }
                            KeyCode::Char('t') | KeyCode::Char('c') | KeyCode::Char('d')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                state.thought_modal = None;
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                modal.scroll = modal.scroll.saturating_sub(1);
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                modal.scroll = modal.scroll.saturating_add(1);
                            }
                            KeyCode::PageUp => {
                                modal.scroll = modal.scroll.saturating_sub(10);
                            }
                            KeyCode::PageDown => {
                                modal.scroll = modal.scroll.saturating_add(10);
                            }
                            KeyCode::Home | KeyCode::Char('g') => {
                                modal.scroll = 0;
                            }
                            KeyCode::End | KeyCode::Char('G') => {
                                modal.scroll = usize::MAX;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    let is_ctrl_d = key.code == KeyCode::Char('d')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if !is_ctrl_d && last_ctrl_d.is_some() {
                        last_ctrl_d = None;
                        if state.active_notice() == Some("Press Ctrl+D again to exit") {
                            state.clear_notice();
                        }
                    }

                    if state.selection_mode {
                        match key.code {
                            KeyCode::Esc => state.selection_mode = false,
                            KeyCode::Char('c') | KeyCode::Char('d')
                                if key.modifiers.contains(KeyModifiers::CONTROL) =>
                            {
                                state.selection_mode = false;
                            }
                            KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                                state.selection_mode = false;
                                if let Some(record) = super::get_last_thought() {
                                    if !record.thought.trim().is_empty() {
                                        let lines: Vec<String> = record
                                            .thought
                                            .trim()
                                            .lines()
                                            .map(str::to_string)
                                            .collect();
                                        state.thought_modal = Some(ThoughtModalState {
                                            elapsed_str: record.elapsed_str,
                                            lines,
                                            scroll: 0,
                                        });
                                        state.clear_notice();
                                    }
                                }
                            }
                            KeyCode::Enter | KeyCode::Char('y') => {
                                let text = state.selected_text();
                                if text.len() > COPY_LIMIT {
                                    state.set_notice("Transcript exceeds the 100 KiB copy limit");
                                } else {
                                    match self.clipboard.copy(&text, &mut io::stdout()) {
                                        Ok(CopyMethod::Native) => {
                                            state.set_notice("Transcript copied to clipboard")
                                        }
                                        Ok(CopyMethod::Osc52) => {
                                            state.set_notice("Transcript copied with OSC52")
                                        }
                                        Err(error) => state.set_notice(format!(
                                            "Could not copy transcript: {error}"
                                        )),
                                    }
                                    state.selection_mode = false;
                                }
                            }
                            KeyCode::PageUp | KeyCode::Up => {
                                state.selection_head = state.selection_head.saturating_sub(1);
                                state.scroll_from_bottom =
                                    state.scroll_from_bottom.saturating_add(1)
                            }
                            KeyCode::PageDown | KeyCode::Down => {
                                let last = state.plain_transcript_lines().len().saturating_sub(1);
                                state.selection_head = (state.selection_head + 1).min(last);
                                state.scroll_from_bottom =
                                    state.scroll_from_bottom.saturating_sub(1)
                            }
                            _ => {}
                        }
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(record) = super::get_last_thought() {
                                if !record.thought.trim().is_empty() {
                                    let lines: Vec<String> =
                                        record.thought.trim().lines().map(str::to_string).collect();
                                    state.thought_modal = Some(ThoughtModalState {
                                        elapsed_str: record.elapsed_str,
                                        lines,
                                        scroll: 0,
                                    });
                                    state.clear_notice();
                                } else {
                                    state.set_notice("No thought process recorded for this turn");
                                }
                            } else {
                                state.set_notice("No thought process recorded for this turn");
                            }
                        }
                        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Ok(Some(uri)) = crate::image::read_clipboard_image() {
                                if let Some(current) = pasted_image.as_mut() {
                                    current.push(' ');
                                    current.push_str(&uri);
                                } else {
                                    pasted_image = Some(uri);
                                }
                                let mut cursor = state.cursor;
                                super::insert_image_placeholder(&mut state.composer, &mut cursor);
                                state.cursor = cursor;
                                state.set_notice("Image attached");
                            } else {
                                state.set_notice("No image found in clipboard");
                            }
                        }
                        KeyCode::PageUp => {
                            state.scroll_from_bottom = state.scroll_from_bottom.saturating_add(10)
                        }
                        KeyCode::PageDown => {
                            state.scroll_from_bottom = state.scroll_from_bottom.saturating_sub(10)
                        }
                        KeyCode::Home if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            state.scroll_from_bottom = u16::MAX
                        }
                        KeyCode::End => state.scroll_from_bottom = 0,
                        KeyCode::Up if !state.suggestions().is_empty() => {
                            state.slash_selected = state.slash_selected.saturating_sub(1)
                        }
                        KeyCode::Down if !state.suggestions().is_empty() => {
                            let last = state.suggestions().len().saturating_sub(1);
                            state.slash_selected = (state.slash_selected + 1).min(last);
                        }
                        KeyCode::Tab if !state.suggestions().is_empty() => {
                            state.apply_selected_suggestion();
                        }
                        KeyCode::Up if !state.composer.contains(&'\n') => state.history_previous(),
                        KeyCode::Down if !state.composer.contains(&'\n') => state.history_next(),
                        KeyCode::Left => state.cursor = state.cursor.saturating_sub(1),
                        KeyCode::Right => {
                            state.cursor = (state.cursor + 1).min(state.composer.len())
                        }
                        KeyCode::Backspace if state.cursor > 0 => {
                            state.cursor -= 1;
                            let cursor = state.cursor;
                            state.composer.remove(cursor);
                            state.slash_selected = 0;
                        }
                        KeyCode::Delete if state.cursor < state.composer.len() => {
                            let cursor = state.cursor;
                            state.composer.remove(cursor);
                        }
                        KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                            let cursor = state.cursor;
                            state.composer.insert(cursor, '\n');
                            state.cursor += 1;
                        }
                        KeyCode::Enter => {
                            let suggestions = state.suggestions();
                            let input: String = state.composer.iter().collect();
                            if input.starts_with('/')
                                && !suggestions.is_empty()
                                && !suggestions.iter().any(|(token, _)| token == input.trim())
                            {
                                state.apply_selected_suggestion();
                            }
                            let text: String = state.composer.drain(..).collect();
                            state.cursor = 0;
                            state.history_index = None;
                            state.draft_before_history.clear();
                            state.clear_notice();
                            if !text.trim().is_empty() {
                                return Ok(Some(InteractiveInput {
                                    text,
                                    pasted_image,
                                    switch_mode: false,
                                }));
                            }
                        }
                        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Some(time) = last_ctrl_d {
                                if time.elapsed() < DOUBLE_PRESS_TIMEOUT {
                                    return Ok(None);
                                }
                            }
                            last_ctrl_d = Some(std::time::Instant::now());
                            state.set_notice("Press Ctrl+D again to exit");
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !state.composer.is_empty() {
                                state.composer.clear();
                                state.cursor = 0;
                                state.slash_selected = 0;
                                state.clear_notice();
                            } else {
                                state.set_notice("Press Ctrl+D twice to exit");
                            }
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            if state
                                .active_notice()
                                .map(|n| !n.starts_with("Terminal too small"))
                                .unwrap_or(false)
                            {
                                state.clear_notice();
                            }
                            let cursor = state.cursor;
                            state.composer.insert(cursor, c);
                            state.cursor += 1;
                            state.slash_selected = 0;
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }
    pub fn handle(state: Arc<Mutex<ChatViewState>>) -> TuiHandle {
        TuiHandle {
            state,
            interrupted: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn handle_dialog_event(
        &mut self,
        input_event: event::Event,
        state: &mut ChatViewState,
    ) -> Result<()> {
        match input_event {
            event::Event::Mouse(mouse) => {
                if !self.handle_selection_mouse(mouse, state)? {
                    route_mouse_wheel(state, mouse.kind);
                }
            }
            event::Event::Resize(_, _) => self.clear_mouse_selection(),
            event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                self.clear_mouse_selection();
                if let Some(dialog) = state.dialog.as_mut()
                    && let Some(answer) = dialog.handle_key(key)
                    && let Some(dialog) = state.dialog.take()
                {
                    let _ = dialog.reply.send(answer);
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub fn prompt_choice(
        &mut self,
        handle: &TuiHandle,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_choice(title, body, options, None, reply));
        }
        loop {
            if let Ok(state) = handle.state.lock() {
                self.draw_state(&state)?;
            }
            if let Ok(answer) = response.try_recv() {
                return Ok(match answer {
                    DialogAnswer::Choice(index) => Some(index),
                    _ => None,
                });
            }
            if event::poll(std::time::Duration::from_millis(50))?
                && let Ok(mut state) = handle.state.lock()
            {
                self.handle_dialog_event(event::read()?, &mut state)?;
            }
        }
    }
    #[allow(dead_code)]
    pub fn prompt_multi_choice(
        &mut self,
        handle: &TuiHandle,
        title: impl Into<String>,
        body: impl Into<String>,
        options: Vec<String>,
    ) -> Result<Option<Vec<usize>>> {
        let (reply, response) = mpsc::channel();
        let checked = vec![false; options.len()];
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_choice(
                title,
                body,
                options,
                Some(checked),
                reply,
            ));
        }
        loop {
            if let Ok(state) = handle.state.lock() {
                self.draw_state(&state)?;
            }
            if let Ok(answer) = response.try_recv() {
                return Ok(match answer {
                    DialogAnswer::Choices(indices) => Some(indices),
                    _ => None,
                });
            }
            if event::poll(std::time::Duration::from_millis(50))?
                && let Ok(mut state) = handle.state.lock()
            {
                self.handle_dialog_event(event::read()?, &mut state)?;
            }
        }
    }
    pub fn prompt_text(
        &mut self,
        handle: &TuiHandle,
        title: impl Into<String>,
        body: impl Into<String>,
        initial: Option<&str>,
    ) -> Result<Option<String>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            let mut dialog = DialogState::new_text(title, body, reply);
            if let Some(init) = initial {
                dialog.input = Some(init.chars().collect());
            }
            state.dialog = Some(dialog);
        }
        loop {
            if let Ok(state) = handle.state.lock() {
                self.draw_state(&state)?;
            }
            if let Ok(answer) = response.try_recv() {
                return Ok(match answer {
                    DialogAnswer::Text(text) if !text.trim().is_empty() => Some(text),
                    _ => None,
                });
            }
            if event::poll(std::time::Duration::from_millis(50))?
                && let Ok(mut state) = handle.state.lock()
            {
                self.handle_dialog_event(event::read()?, &mut state)?;
            }
        }
    }
    pub async fn drive_agent<T: Send + 'static>(
        &mut self,
        handle: &TuiHandle,
        task: tokio::task::JoinHandle<T>,
    ) -> Result<T> {
        use event::{KeyCode, KeyModifiers};
        loop {
            if let Ok(state) = handle.state.lock() {
                self.draw_state(&state)?;
            }
            if task.is_finished() {
                return Ok(task.await?);
            }
            if event::poll(std::time::Duration::from_millis(30))? {
                match event::read()? {
                    event::Event::Resize(_, _) => self.clear_mouse_selection(),
                    event::Event::Mouse(mouse) => {
                        if let Ok(mut state) = handle.state.lock() {
                            if self.handle_selection_mouse(mouse, &mut state)? {
                                continue;
                            }
                            route_mouse_wheel(&mut state, mouse.kind);
                        }
                    }
                    event::Event::Paste(text) => {
                        self.clear_mouse_selection();
                        if let Ok(mut state) = handle.state.lock() {
                            let cursor = state.cursor;
                            for (offset, character) in text.chars().enumerate() {
                                state.composer.insert(cursor + offset, character);
                            }
                            state.cursor += text.chars().count();
                        }
                    }
                    event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                        self.clear_mouse_selection();
                        if let Ok(mut state) = handle.state.lock()
                            && let Some(dialog) = state.dialog.as_mut()
                        {
                            if let Some(answer) = dialog.handle_key(key) {
                                if let Some(dialog) = state.dialog.take() {
                                    let _ = dialog.reply.send(answer);
                                }
                            }
                            continue;
                        }
                        if key.code == KeyCode::Esc
                            || (key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL))
                        {
                            handle.interrupted.store(true, Ordering::Relaxed);
                            continue;
                        }
                        if let Ok(mut state) = handle.state.lock() {
                            match key.code {
                                KeyCode::End => state.scroll_from_bottom = 0,
                                KeyCode::Enter => {
                                    if !state.composer.is_empty() {
                                        let text: String = state.composer.drain(..).collect();
                                        state.cursor = 0;
                                        if let Ok(mut queued) = handle.queued.lock() {
                                            queued.push(text);
                                        }
                                    }
                                }
                                KeyCode::Backspace if state.cursor > 0 => {
                                    state.cursor -= 1;
                                    let cursor = state.cursor;
                                    state.composer.remove(cursor);
                                }
                                KeyCode::Left => state.cursor = state.cursor.saturating_sub(1),
                                KeyCode::Right => {
                                    state.cursor = (state.cursor + 1).min(state.composer.len())
                                }
                                KeyCode::Char(character)
                                    if !key
                                        .modifiers
                                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                                {
                                    let cursor = state.cursor;
                                    state.composer.insert(cursor, character);
                                    state.cursor += 1;
                                }
                                _ => {}
                            }
                        }
                    }
                    _ => {}
                }
            }
            tokio::task::yield_now().await;
        }
    }
    pub fn suspend(&mut self) {
        if !self.active {
            return;
        }
        let _ = self.terminal.show_cursor();
        restore_terminal();
        self.active = false;
    }
    pub fn resume(&mut self) -> Result<()> {
        if self.active {
            return Ok(());
        }
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        let setup = setup_terminal_output(&mut stdout);
        if let Err(error) = setup {
            restore_terminal();
            return Err(error.into());
        }
        if let Err(error) = self.terminal.clear() {
            restore_terminal();
            return Err(error.into());
        }
        self.active = true;
        Ok(())
    }
}
impl Drop for ChatTui {
    fn drop(&mut self) {
        self.suspend();
    }
}

#[cfg(test)]
mod dialog_tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::layout::Rect;

    fn make_key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn skill_suggestion_completes_name_before_task() {
        let workspace = std::env::temp_dir().join(format!(
            "mint-tui-skill-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let skill_dir = workspace.join("skills").join("tui-check-skill");
        std::fs::create_dir_all(&skill_dir).unwrap();
        std::fs::write(
            skill_dir.join("SKILL.md"),
            "---\ndescription: Test skill\n---\n",
        )
        .unwrap();
        let mut state = ChatViewState {
            current_dir: workspace.clone(),
            ..ChatViewState::default()
        };
        state.set_draft("$tui-check".to_owned());
        assert!(
            state
                .suggestions()
                .iter()
                .any(|(name, _)| name == "$tui-check-skill")
        );
        assert!(state.apply_selected_suggestion());
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "$tui-check-skill "
        );
        std::fs::remove_dir_all(workspace).unwrap();
    }

    #[test]
    fn mention_suggestion_replaces_only_the_word_at_cursor() {
        let mut state = ChatViewState {
            current_dir: std::env::current_dir().unwrap(),
            ..ChatViewState::default()
        };
        state.set_draft("Please check @work today".to_owned());
        state.cursor = "Please check @work".chars().count();
        assert!(
            state
                .suggestions()
                .iter()
                .any(|(name, _)| name == "@workspace")
        );
        assert!(state.apply_selected_suggestion());
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "Please check @workspace today"
        );
        assert_eq!(state.cursor, "Please check @workspace ".chars().count());
    }

    #[test]
    fn three_choice_dialog_shows_every_option_with_three_body_lines() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.dialog = Some(DialogState::new_choice(
            "Local shell command",
            "Command: uname -r && uname -a\nMode: mutating\nBackground: no",
            vec![
                "Yes".to_owned(),
                "Yes, allow for this session".to_owned(),
                "No".to_owned(),
            ],
            None,
            reply,
        ));
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("3. No"));
        assert!(!rendered.contains("more below"));
    }

    #[test]
    fn choice_dialog_uses_available_height_for_more_than_eight_options() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.dialog = Some(DialogState::new_choice(
            "Choose one",
            "",
            (1..=9).map(|index| format!("Option {index}")).collect(),
            None,
            reply,
        ));
        let mut terminal = Terminal::new(TestBackend::new(80, 32)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Option 9"));
        assert!(!rendered.contains("more below"));
    }

    #[test]
    fn long_approval_command_keeps_choices_and_hint_visible_in_narrow_terminal() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.status = vec!["Running shell command".to_owned(); 6];
        state.dialog = Some(DialogState::new_choice(
            "Local shell command",
            concat!(
                "Command: who -a; echo '--- LOGINCTL ---'; loginctl list-sessions 2>&1; ",
                "echo '--- SEATS/USERS ---'; loginctl list-users 2>&1; ",
                "echo '--- UPTIME ---'; uptime -p; uptime -s; ",
                "echo '--- SESSION ENV ---'; echo TYPE=$XDG_SESSION_TYPE ",
                "DESKTOP=$XDG_CURRENT_DESKTOP SESSION=$DESKTOP_SESSION ",
                "WAYLAND=$WAYLAND_DISPLAY DISPLAY=$DISPLAY; ",
                "echo '--- TTY/USER PROCS (top 25 by count) ---'; ",
                "ps -u $USER -o comm= | sort | uniq -c | sort -rn | head -25\n",
                "Mode: mutating\nBackground: no",
            ),
            vec![
                "Yes".to_owned(),
                "Yes, allow for this session".to_owned(),
                "No".to_owned(),
            ],
            None,
            reply,
        ));
        let mut terminal = Terminal::new(TestBackend::new(94, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("3. No"));
        assert!(rendered.contains("Enter select"));
        assert!(!rendered.contains("more below"));
    }

    #[test]
    fn live_shell_status_remains_visible_above_a_long_transcript() {
        let mut state = ChatViewState::default();
        state.transcript.push(TranscriptEntry::new(
            TranscriptRole::Assistant,
            "Earlier output\n".repeat(80),
        ));
        state.status = vec![
            "  ● Running 1 shell command…".to_owned(),
            "    └ ⠋ [run_shell] Running command: sleep 1".to_owned(),
            "  Unpacking…".to_owned(),
        ];
        let mut terminal = Terminal::new(TestBackend::new(94, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Running 1 shell command"));
        assert!(rendered.contains("[run_shell] Running command"));
    }

    fn point(column: u16, row: u16) -> ScreenPoint {
        ScreenPoint { row, column }
    }

    struct FakeClipboard {
        fail: bool,
    }

    impl NativeClipboard for FakeClipboard {
        fn set_text(&mut self, _text: String) -> std::result::Result<(), String> {
            if self.fail {
                Err("unavailable".to_owned())
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn mouse_selection_requires_a_drag_and_normalizes_direction() {
        let mut selection = MouseSelection::default();
        selection.start(point(7, 4));
        assert!(!selection.finish(point(7, 4)));
        assert_eq!(selection.range(), None);

        selection.start(point(7, 4));
        selection.update(point(2, 1));
        assert!(selection.finish(point(2, 1)));
        assert_eq!(selection.range(), Some((point(2, 1), point(7, 4))));
    }

    #[test]
    fn screen_selection_copies_visual_rows_and_trims_only_the_end() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 2));
        buffer.set_string(0, 0, " hello", Style::default());
        buffer.set_string(0, 1, "world", Style::default());
        let selection = MouseSelection {
            anchor: Some(point(1, 0)),
            head: Some(point(2, 1)),
            dragging: false,
            had_drag: true,
        };

        assert_eq!(selected_screen_text(&buffer, selection), "hello\nwor");
    }

    #[test]
    fn screen_selection_includes_a_wide_grapheme_from_its_continuation_cell() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 8, 1));
        buffer.set_string(0, 0, "A🙂B ก้", Style::default());
        let selection = MouseSelection {
            anchor: Some(point(2, 0)),
            head: Some(point(2, 0)),
            dragging: false,
            had_drag: true,
        };

        assert_eq!(selected_screen_text(&buffer, selection), "🙂");
    }

    #[test]
    fn selection_overlay_changes_style_without_changing_symbols() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 5, 1));
        buffer.set_string(0, 0, "Mint", Style::default().fg(Color::Green));
        let before = buffer
            .content
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect::<Vec<_>>();
        let selection = MouseSelection {
            anchor: Some(point(1, 0)),
            head: Some(point(2, 0)),
            dragging: false,
            had_drag: true,
        };

        apply_mouse_selection(&mut buffer, selection);

        let after = buffer
            .content
            .iter()
            .map(|cell| cell.symbol().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(after, before);
        assert!(!buffer[(0, 0)].modifier.contains(Modifier::REVERSED));
        assert!(buffer[(1, 0)].modifier.contains(Modifier::REVERSED));
        assert!(buffer[(2, 0)].modifier.contains(Modifier::REVERSED));
        assert!(!buffer[(3, 0)].modifier.contains(Modifier::REVERSED));
    }

    #[test]
    fn completed_selection_is_invalidated_when_selected_cells_change() {
        let mut previous = Buffer::empty(Rect::new(0, 0, 5, 1));
        previous.set_string(0, 0, "Mint", Style::default());
        let selection = MouseSelection {
            anchor: Some(point(1, 0)),
            head: Some(point(2, 0)),
            dragging: false,
            had_drag: true,
        };
        let unchanged = previous.clone();
        assert!(selected_cells_match(&previous, &unchanged, selection));

        let mut changed_inside = previous.clone();
        changed_inside.set_string(1, 0, "XX", Style::default());
        assert!(!selected_cells_match(&previous, &changed_inside, selection));

        let mut changed_outside = previous.clone();
        changed_outside.set_string(4, 0, "!", Style::default());
        assert!(selected_cells_match(&previous, &changed_outside, selection));
    }

    #[test]
    fn clipboard_prefers_native_and_falls_back_to_osc52() {
        let mut native = TextClipboard {
            native: Some(Box::new(FakeClipboard { fail: false })),
            initialization_attempted: true,
        };
        let mut native_output = Vec::new();
        assert_eq!(
            native.copy("Mint", &mut native_output).unwrap(),
            CopyMethod::Native
        );
        assert!(native_output.is_empty());

        let mut fallback = TextClipboard {
            native: Some(Box::new(FakeClipboard { fail: true })),
            initialization_attempted: true,
        };
        let mut osc52_output = Vec::new();
        assert_eq!(
            fallback.copy("Mint", &mut osc52_output).unwrap(),
            CopyMethod::Osc52
        );
        assert_eq!(
            String::from_utf8(osc52_output).unwrap(),
            "\u{1b}]52;c;TWludA==\u{7}"
        );
    }

    #[test]
    fn terminal_setup_keeps_wheel_and_history_navigation_distinct() {
        let mut output = Vec::new();
        setup_terminal_output(&mut output).expect("terminal setup should render ANSI commands");

        let output = String::from_utf8(output).expect("terminal commands should be UTF-8");
        assert!(output.contains("\u{1b}[?1049h"));
        assert!(
            output.contains("\u{1b}[?1000h")
                && output.contains("\u{1b}[?1002h")
                && output.contains("\u{1b}[?1003h")
                && output.contains("\u{1b}[?1006h"),
            "mouse reporting is required to distinguish wheel events from Up/Down keys"
        );
        assert!(output.contains("\u{1b}[?2004h"));
    }

    #[test]
    fn mouse_wheel_routes_to_transcript_or_open_thought_panel() {
        let mut state = ChatViewState::default();
        route_mouse_wheel(&mut state, event::MouseEventKind::ScrollUp);
        assert_eq!(state.scroll_from_bottom, 3);

        state.thought_modal = Some(ThoughtModalState {
            elapsed_str: "1.0s".to_owned(),
            lines: vec!["thought".to_owned()],
            scroll: 4,
        });
        route_mouse_wheel(&mut state, event::MouseEventKind::ScrollUp);
        assert_eq!(state.thought_modal.as_ref().unwrap().scroll, 1);
        route_mouse_wheel(&mut state, event::MouseEventKind::ScrollDown);
        assert_eq!(state.thought_modal.as_ref().unwrap().scroll, 4);
    }

    #[test]
    fn back_to_bottom_appears_only_while_scrolled_up_and_click_returns_to_latest() {
        let mut state = ChatViewState::default();
        state.push_user(
            (0..60)
                .map(|n| format!("line {n}"))
                .collect::<Vec<_>>()
                .join("\n"),
        );
        let mut terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
        let mut button = None;
        terminal.draw(|frame| button = state.render(frame)).unwrap();
        assert!(button.is_none());

        state.scroll_from_bottom = 10;
        terminal.draw(|frame| button = state.render(frame)).unwrap();
        let area = button.expect("button should appear above the composer");
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Back to bottom"));
        assert!(!activate_back_to_bottom(
            &mut state,
            button,
            event::MouseEventKind::Down(event::MouseButton::Left),
            ScreenPoint {
                row: area.y,
                column: area.x.saturating_sub(1)
            },
        ));
        assert_eq!(state.scroll_from_bottom, 10);
        assert!(activate_back_to_bottom(
            &mut state,
            button,
            event::MouseEventKind::Down(event::MouseButton::Left),
            ScreenPoint {
                row: area.y,
                column: area.x
            },
        ));
        assert_eq!(state.scroll_from_bottom, 0);
        terminal.draw(|frame| button = state.render(frame)).unwrap();
        assert!(button.is_none());
    }

    #[test]
    fn test_dialog_filtering() {
        let (tx, _rx) = mpsc::channel();
        let options = vec![
            "gpt-4o".to_string(),
            "claude-3-5-sonnet".to_string(),
            "gemini-2.0-flash".to_string(),
            "deepseek-r1".to_string(),
        ];
        let mut dialog = DialogState::new_choice("Pick model", "", options, None, tx);

        assert_eq!(dialog.filtered_indices(), vec![0, 1, 2, 3]);

        // Type 'g'
        dialog.handle_key(make_key(KeyCode::Char('g')));
        assert_eq!(dialog.filtered_indices(), vec![0, 2]);

        // Type 'e' -> "ge"
        dialog.handle_key(make_key(KeyCode::Char('e')));
        assert_eq!(dialog.filtered_indices(), vec![2]);

        // Backspace -> "g"
        dialog.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(dialog.filtered_indices(), vec![0, 2]);

        // Esc clears filter
        dialog.handle_key(make_key(KeyCode::Esc));
        assert_eq!(dialog.filtered_indices(), vec![0, 1, 2, 3]);
    }

    #[test]
    fn test_dialog_navigation_and_numeric_shortcuts() {
        let (tx, _rx) = mpsc::channel();
        let options = vec![
            "Option 1".to_string(),
            "Option 2".to_string(),
            "Option 3".to_string(),
        ];
        let mut dialog = DialogState::new_choice("Pick one", "", options, None, tx);

        assert_eq!(dialog.selected, 0);

        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(dialog.selected, 1);

        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(dialog.selected, 2);

        // Clamped at bottom
        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(dialog.selected, 2);

        dialog.handle_key(make_key(KeyCode::Up));
        assert_eq!(dialog.selected, 1);

        // Numeric shortcut: '1' selects index 0 and returns Choice(0)
        let answer = dialog.handle_key(make_key(KeyCode::Char('1')));
        assert_eq!(answer, Some(DialogAnswer::Choice(0)));

        // Numeric shortcut: '3' selects index 2 and returns Choice(2)
        let answer = dialog.handle_key(make_key(KeyCode::Char('3')));
        assert_eq!(answer, Some(DialogAnswer::Choice(2)));
    }

    #[test]
    fn test_dialog_multiselect() {
        let (tx, _rx) = mpsc::channel();
        let options = vec!["A".to_string(), "B".to_string(), "C".to_string()];
        let checked = vec![false, false, false];
        let mut dialog = DialogState::new_choice("Multi", "", options, Some(checked), tx);

        // Toggle first option with Space
        dialog.handle_key(make_key(KeyCode::Char(' ')));
        assert_eq!(dialog.checked.as_ref().unwrap(), &vec![true, false, false]);

        // Move to second option and toggle
        dialog.handle_key(make_key(KeyCode::Down));
        dialog.handle_key(make_key(KeyCode::Char(' ')));
        assert_eq!(dialog.checked.as_ref().unwrap(), &vec![true, true, false]);

        // Enter submits selected choices
        let answer = dialog.handle_key(make_key(KeyCode::Enter));
        assert_eq!(answer, Some(DialogAnswer::Choices(vec![0, 1])));
    }

    #[test]
    fn test_dialog_text_input() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_text("Input", "Prompt", tx);

        dialog.handle_key(make_key(KeyCode::Char('h')));
        dialog.handle_key(make_key(KeyCode::Char('i')));
        assert_eq!(dialog.input.as_ref().unwrap(), &vec!['h', 'i']);

        dialog.handle_key(make_key(KeyCode::Backspace));
        assert_eq!(dialog.input.as_ref().unwrap(), &vec!['h']);

        let answer = dialog.handle_key(make_key(KeyCode::Enter));
        assert_eq!(answer, Some(DialogAnswer::Text("h".to_string())));
    }

    #[test]
    fn test_dialog_cancellation() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog =
            DialogState::new_choice("Pick", "", vec!["A".into(), "B".into()], None, tx);

        let answer = dialog.handle_key(make_key(KeyCode::Esc));
        assert_eq!(answer, Some(DialogAnswer::Cancel));

        let ctrl_c = KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        let answer = dialog.handle_key(ctrl_c);
        assert_eq!(answer, Some(DialogAnswer::Cancel));

        let ctrl_d = KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::CONTROL,
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        };
        let answer = dialog.handle_key(ctrl_d);
        assert_eq!(answer, Some(DialogAnswer::Cancel));
    }

    #[test]
    fn test_transcript_entry_markdown_rendering() {
        let text = "**Bold Header**\n- Item 1\n---\n> [!TIP]\n> Pro tip here";
        let entry = TranscriptEntry::new(TranscriptRole::Assistant, text);

        // Check line synchronization
        assert_eq!(entry.rendered_lines.len(), entry.plain_lines.len());

        // Check bold rendering
        let first_rendered = &entry.rendered_lines[0];
        let has_bold = first_rendered
            .spans
            .iter()
            .any(|s| s.style.add_modifier.contains(Modifier::BOLD));
        assert!(has_bold, "First line should contain bold modifier");

        // Check divider line rendering (converted --- into sleek box drawing ─)
        let divider_line = &entry.plain_lines[2];
        assert!(
            divider_line.contains('─'),
            "--- should be converted to ─ divider: {divider_line}"
        );

        // Check TIP alert formatting
        let alert_line = &entry.plain_lines[3];
        assert!(
            alert_line.contains("│ TIP:"),
            "Alert should format as │ TIP: : {alert_line}"
        );

        // Check bullet formatting
        let bullet_line = &entry.plain_lines[1];
        assert!(
            bullet_line.contains('•'),
            "List item should format with bullet • : {bullet_line}"
        );
    }

    #[test]
    fn test_thought_modal_state_and_scrolling() {
        let mut modal = ThoughtModalState {
            elapsed_str: "3.5s".to_string(),
            lines: vec!["line 1".into(), "line 2".into(), "line 3".into()],
            scroll: 0,
        };

        assert_eq!(modal.scroll, 0);
        modal.scroll = modal.scroll.saturating_add(1);
        assert_eq!(modal.scroll, 1);
        modal.scroll = modal.scroll.saturating_sub(1);
        assert_eq!(modal.scroll, 0);
    }

    #[test]
    fn test_notice_expiration_and_clear() {
        let mut state = ChatViewState::default();
        assert_eq!(state.active_notice(), None);

        state.set_notice("Image attached");
        assert_eq!(state.active_notice(), Some("Image attached"));

        // Terminal too small warning doesn't auto-expire
        state.set_notice("Terminal too small: 40x10");
        assert_eq!(state.active_notice(), Some("Terminal too small: 40x10"));

        state.clear_notice();
        assert_eq!(state.active_notice(), None);

        // Manually test expiration logic
        state.notice = Some((
            "Old notice".into(),
            std::time::Instant::now() - std::time::Duration::from_millis(3000),
        ));
        assert_eq!(state.active_notice(), None);
    }
}

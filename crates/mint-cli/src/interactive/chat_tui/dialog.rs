//! Modal dialog state machine, key routing, and specialized picker views.
use super::*;

pub(super) fn theme_picker_lines(dialog: &DialogState, height: u16) -> Vec<Line<'static>> {
    let selected = crate::terminal_theme::TuiTheme::from_index(dialog.selected);
    let colors = crate::terminal_theme::preview_colors(selected);
    let mut lines = vec![
        Line::styled(
            " Theme",
            Style::default()
                .fg(crate::terminal_theme::BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Line::raw(format!(" {}", dialog.body)),
        Line::raw(""),
    ];
    for (index, label) in dialog.options.iter().enumerate() {
        let focused = index == dialog.selected;
        let active = dialog.theme_picker == Some(index);
        let style = Style::default()
            .fg(if focused {
                crate::terminal_theme::ACCENT
            } else {
                crate::terminal_theme::TEXT
            })
            .add_modifier(if focused {
                Modifier::BOLD
            } else {
                Modifier::empty()
            });
        lines.push(Line::from(vec![
            Span::styled(if focused { " › " } else { "   " }, style),
            Span::styled(format!("{}. {label}", index + 1), style),
            Span::styled(
                if active { "  ✓" } else { "" },
                Style::default().fg(crate::terminal_theme::ACCENT),
            ),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        format!(" Preview · {}", selected.label()),
        Style::default().fg(crate::terminal_theme::MUTED),
    ));
    let sample = Style::default().fg(colors.text).bg(colors.background);
    lines.push(Line::from(vec![
        Span::styled(" 1  ", sample.fg(colors.muted)),
        Span::styled("fn", sample.fg(colors.keyword)),
        Span::styled(" greet() {", sample),
    ]));
    lines.push(Line::from(vec![
        Span::styled(
            " 2 -",
            sample.fg(colors.removed).bg(colors.removed_background),
        ),
        Span::styled(
            " println!(\"Hello, World!\");",
            sample.fg(colors.removed).bg(colors.removed_background),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" 2 +", sample.fg(colors.added).bg(colors.added_background)),
        Span::styled(
            " println!(\"Hello, Mint!\");",
            sample.fg(colors.added).bg(colors.added_background),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" 3  ", sample.fg(colors.muted)),
        Span::styled("}", sample),
    ]));
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        " ↑/↓ preview · 1-3 select · Enter apply · Esc cancel",
        Style::default().fg(crate::terminal_theme::MUTED),
    ));
    lines.truncate(height.saturating_sub(1) as usize);
    lines
}

#[derive(Debug)]
pub(crate) struct DialogState {
    pub title: String,
    pub body: String,
    pub options: Vec<String>,
    pub filter: Vec<char>,
    pub selected: usize,
    pub scroll_offset: usize,
    pub body_scroll: usize,
    pub(super) body_max_scroll: std::cell::Cell<usize>,
    pub(super) body_page_size: std::cell::Cell<usize>,
    pub checked: Option<Vec<bool>>,
    pub input: Option<Vec<char>>,
    pub resume_picker: Option<ResumePickerCore>,
    pub theme_picker: Option<usize>,
    pub update_prompt: bool,
    pub is_tool_details: bool,
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
    pub fn is_tool_details(&self) -> bool {
        self.is_tool_details
    }

    pub fn new_tool_details(
        title: impl Into<String>,
        body: impl Into<String>,
        reply: mpsc::Sender<DialogAnswer>,
    ) -> Self {
        let mut dialog = Self::new_choice(title, body, vec!["Close".into()], None, reply);
        dialog.is_tool_details = true;
        dialog
    }

    pub fn new_update(current: &str, latest: &str, reply: mpsc::Sender<DialogAnswer>) -> Self {
        let mut dialog = Self::new_choice(
            format!("Update available · {current} → {latest}"),
            "Release notes: https://github.com/Pheem49/Mint/releases/latest",
            vec![
                "Update now (runs `mint update --approve`)".into(),
                "Skip".into(),
                "Skip until next version".into(),
            ],
            None,
            reply,
        );
        dialog.selected = 1;
        dialog.update_prompt = true;
        dialog
    }

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
            body_scroll: 0,
            body_max_scroll: std::cell::Cell::new(0),
            body_page_size: std::cell::Cell::new(0),
            checked,
            input: None,
            resume_picker: None,
            theme_picker: None,
            update_prompt: false,
            is_tool_details: false,
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
            body_scroll: 0,
            body_max_scroll: std::cell::Cell::new(0),
            body_page_size: std::cell::Cell::new(0),
            checked: None,
            input: Some(Vec::new()),
            resume_picker: None,
            theme_picker: None,
            update_prompt: false,
            is_tool_details: false,
            reply,
        }
    }

    pub fn new_resume_picker(
        sessions: Vec<mint_core::ChatSession>,
        current_workspace: String,
        current_branch: Option<String>,
        active_chat_id: String,
        reply: mpsc::Sender<DialogAnswer>,
    ) -> Self {
        let options = sessions
            .iter()
            .map(|session| session.title.clone())
            .collect();
        Self {
            title: "Resume Session".to_string(),
            body: String::new(),
            options,
            filter: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            body_scroll: 0,
            body_max_scroll: std::cell::Cell::new(0),
            body_page_size: std::cell::Cell::new(0),
            checked: None,
            input: None,
            resume_picker: Some(ResumePickerCore::new(
                sessions,
                current_workspace,
                current_branch,
                active_chat_id,
            )),
            theme_picker: None,
            update_prompt: false,
            is_tool_details: false,
            reply,
        }
    }

    pub fn new_theme_picker(active: usize, reply: mpsc::Sender<DialogAnswer>) -> Self {
        Self {
            title: "Theme".to_owned(),
            body: "Choose the text style that looks best with your terminal".to_owned(),
            options: ["Auto (match terminal)", "Dark mode", "Light mode"]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            filter: Vec::new(),
            selected: active.min(2),
            scroll_offset: 0,
            body_scroll: 0,
            body_max_scroll: std::cell::Cell::new(0),
            body_page_size: std::cell::Cell::new(0),
            checked: None,
            input: None,
            resume_picker: None,
            theme_picker: Some(active.min(2)),
            update_prompt: false,
            is_tool_details: false,
            reply,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        if let Some(picker) = &self.resume_picker {
            return picker.filtered_indices(&self.filter);
        }
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

        if self.body_max_scroll.get() > 0 {
            self.body_scroll = self.body_scroll.min(self.body_max_scroll.get());
            let page = self.body_page_size.get().max(1);
            match key.code {
                KeyCode::PageUp => {
                    self.body_scroll = self.body_scroll.saturating_sub(page);
                    return None;
                }
                KeyCode::PageDown => {
                    self.body_scroll = self
                        .body_scroll
                        .saturating_add(page)
                        .min(self.body_max_scroll.get());
                    return None;
                }
                KeyCode::Home => {
                    self.body_scroll = 0;
                    return None;
                }
                KeyCode::End => {
                    self.body_scroll = self.body_max_scroll.get();
                    return None;
                }
                _ => {}
            }
        }

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
            let filtered_for_picker = self.filtered_indices();
            if let Some(picker) = self.resume_picker.as_mut() {
                if picker.renaming {
                    match key.code {
                        KeyCode::Esc => {
                            picker.renaming = false;
                        }
                        KeyCode::Enter => {
                            let trimmed = picker.rename_query.trim().to_owned();
                            if !trimmed.is_empty()
                                && let Some(&orig_idx) = filtered_for_picker.get(self.selected)
                                && let Some(session) = picker.sessions.get_mut(orig_idx)
                            {
                                if let Ok(memory) = mint_core::MemoryStore::open_default() {
                                    let _ = memory.rename_chat_session(&session.id, &trimmed);
                                }
                                session.title = trimmed.clone();
                                if let Some(opt) = self.options.get_mut(orig_idx) {
                                    *opt = trimmed;
                                }
                            }
                            picker.renaming = false;
                            let new_len = self.filtered_indices().len();
                            self.selected = if new_len > 0 {
                                self.selected.min(new_len - 1)
                            } else {
                                0
                            };
                        }
                        KeyCode::Backspace => {
                            picker.rename_query.pop();
                        }
                        KeyCode::Char(c)
                            if !key
                                .modifiers
                                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
                        {
                            picker.rename_query.push(c);
                        }
                        _ => {}
                    }
                    return None;
                }
                if picker.showing_preview {
                    match key.code {
                        KeyCode::Char(' ') | KeyCode::Esc => {
                            picker.showing_preview = false;
                            return None;
                        }
                        KeyCode::Enter => {
                            if let Some(&orig_idx) = filtered_for_picker.get(self.selected) {
                                return Some(DialogAnswer::Choice(orig_idx));
                            }
                        }
                        _ => return None,
                    }
                }
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match key.code {
                        KeyCode::Char('a') => {
                            picker.show_all_projects = !picker.show_all_projects;
                            self.selected = 0;
                            self.scroll_offset = 0;
                            return None;
                        }
                        KeyCode::Char('b') => {
                            picker.only_current_branch = !picker.only_current_branch;
                            self.selected = 0;
                            self.scroll_offset = 0;
                            return None;
                        }
                        KeyCode::Char('r') => {
                            if let Some(&orig_idx) = filtered_for_picker.get(self.selected)
                                && let Some(session) = picker.sessions.get(orig_idx)
                            {
                                picker.renaming = true;
                                picker.rename_query = session.title.clone();
                            }
                            return None;
                        }
                        KeyCode::Char('c' | 'd') => return Some(DialogAnswer::Cancel),
                        _ => {}
                    }
                }
                if key.code == KeyCode::Tab {
                    picker.sort_by_created = !picker.sort_by_created;
                    self.selected = 0;
                    self.scroll_offset = 0;
                    return None;
                }
                if key.code == KeyCode::Char(' ')
                    && self.filter.is_empty()
                    && !filtered_for_picker.is_empty()
                {
                    picker.showing_preview = true;
                    return None;
                }
            }
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
                    if self.select_numeric(digit) && self.checked.is_none() && !self.update_prompt {
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
                    if self.theme_picker.is_some() || self.update_prompt {
                        return None;
                    }
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

pub(super) fn resume_picker_lines(
    dialog: &DialogState,
    width: u16,
    height: u16,
) -> Vec<Line<'static>> {
    let Some(picker) = &dialog.resume_picker else {
        return Vec::new();
    };
    picker.render_lines(
        &dialog.filter,
        dialog.selected,
        dialog.scroll_offset,
        width,
        height,
    )
}

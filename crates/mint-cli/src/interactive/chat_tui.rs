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

/// Sweep a white glow across the thinking text, fading to dim gray at the edges.
fn shimmer_thinking_line(line: &str, tick: usize) -> Line<'static> {
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

fn format_thinking_status(config: &mint_core::MintConfig) -> String {
    let model = active_model(&config.ai_provider, config);
    if !config.resolved_thinking_enabled_for_model(&config.ai_provider, model) {
        return "off".to_string();
    }
    match config.resolved_thinking_effort_for_model(model) {
        "extra_high" => "extra high".to_string(),
        effort => effort.to_string(),
    }
}

const MIN_WIDTH: u16 = 60;
const MIN_HEIGHT: u16 = 12;
const COPY_LIMIT: usize = 100 * 1024;
const BACK_TO_BOTTOM_LABEL: &str = " ↓ Back to bottom · End ";
const WATERMARK_FRAME_COUNT: u8 = 200;
const WATERMARK_TICK: std::time::Duration = std::time::Duration::from_millis(30);
const WATERMARK_HOLD: std::time::Duration = std::time::Duration::from_secs(1);
const WATERMARK_WIDTH: u16 = 39;
const WATERMARK_HEIGHT: u16 = 17;

fn watermark_animation_at(
    elapsed: std::time::Duration,
) -> (Option<u8>, Option<std::time::Duration>) {
    let half_turn_frames = WATERMARK_FRAME_COUNT / 2;
    let half_turn = WATERMARK_TICK * half_turn_frames as u32;
    let hold_end = half_turn + WATERMARK_HOLD;
    let full_turn_end = hold_end + half_turn;
    // Ease each 3-second half turn into and out of its resting face.
    let eased_frame = |step: u8| {
        let progress = step as f32 / half_turn_frames as f32;
        let eased = (1.0 - (progress * std::f32::consts::PI).cos()) * 0.5;
        (eased * half_turn_frames as f32).round() as u8
    };
    if elapsed < half_turn {
        let step = (elapsed.as_millis() / WATERMARK_TICK.as_millis()) as u8;
        (
            Some(eased_frame(step)),
            Some(WATERMARK_TICK * (step as u32 + 1)),
        )
    } else if elapsed < hold_end {
        (Some(half_turn_frames), Some(hold_end))
    } else if elapsed < full_turn_end {
        let step = ((elapsed - hold_end).as_millis() / WATERMARK_TICK.as_millis()) as u8;
        (
            Some((half_turn_frames + eased_frame(step)) % WATERMARK_FRAME_COUNT),
            Some(hold_end + WATERMARK_TICK * (step as u32 + 1)),
        )
    } else {
        (None, None)
    }
}

fn mint_m_watermark(frame: Option<u8>, height: usize) -> Text<'static> {
    const WIDTH: usize = WATERMARK_WIDTH as usize;
    const DESIGN_HEIGHT: usize = WATERMARK_HEIGHT as usize;
    #[derive(Clone, Copy)]
    enum Part {
        Void,
        Rim,
        Band,
        Orbit,
        Mark,
    }

    let height = height.clamp(1, DESIGN_HEIGHT);
    let angle = frame.unwrap_or(0) as f32 * std::f32::consts::TAU / WATERMARK_FRAME_COUNT as f32;
    let (sin, cos) = angle.sin_cos();
    let mut pixels = vec![vec![None::<(f32, Part)>; WIDTH]; height];

    // A dotted medallion with an M on its front and a leaf on its back. The
    // two faces and the rim share one depth buffer as they turn in 3D.
    for y in 0..DESIGN_HEIGHT {
        let world_y = y as f32 - 8.0;
        let projected_y = y * height.saturating_sub(1) / (DESIGN_HEIGHT - 1);
        for source_x in 0..WIDTH {
            let world_x = (source_x as f32 - 19.0) / 2.0;
            let radius = world_x.hypot(world_y);
            if radius > 8.15 {
                continue;
            }

            let stem = (world_x.abs() - 3.7).abs() < 0.5 && world_y.abs() <= 4.2;
            let diagonal_y = world_y + 4.2;
            let diagonal = (-4.2..=2.2).contains(&world_y)
                && [-3.7 + diagonal_y * 3.7 / 6.4, 3.7 - diagonal_y * 3.7 / 6.4]
                    .into_iter()
                    .any(|line_x| (world_x - line_x).abs() < 0.5);
            let front_mark = stem || diagonal;

            let leaf_x = world_x * 0.8 + world_y * 0.6;
            let leaf_y = -world_x * 0.6 + world_y * 0.8;
            let leaf_radius = (leaf_x / 4.2).powi(2) + (leaf_y / 2.5).powi(2);
            let back_mark =
                (leaf_radius - 1.0).abs() < 0.22 || (leaf_y.abs() < 0.32 && leaf_x.abs() < 3.8);

            for (z, mark) in [(-1.2, back_mark), (0.0, false), (1.2, front_mark)] {
                let part = if radius > 7.25 {
                    Part::Rim
                } else if radius > 6.2 {
                    Part::Band
                } else if mark {
                    Part::Mark
                } else if radius > 5.75 && radius < 6.05 && (source_x + y) % 3 != 0 {
                    Part::Orbit
                } else {
                    Part::Void
                };
                let projected_x = (19.0 + (world_x * cos + z * sin) * 2.0).round() as isize;
                if !(0..WIDTH as isize).contains(&projected_x) {
                    continue;
                }
                let depth = -world_x * sin + z * cos;
                let cell = &mut pixels[projected_y][projected_x as usize];
                if cell.is_none_or(|(existing_depth, _)| depth > existing_depth) {
                    *cell = Some((depth, part));
                }
            }
        }
    }

    let lines = pixels
        .into_iter()
        .map(|row| {
            let spans = row
                .into_iter()
                .map(|pixel| match pixel {
                    Some((_, Part::Void)) | None => Span::raw(" "),
                    Some((depth, part)) => {
                        let lit = frame.is_some() && depth > 0.5;
                        let (glyph, color) = match part {
                            Part::Mark => (
                                "⣿",
                                if lit {
                                    crate::terminal_theme::ACCENT
                                } else {
                                    crate::terminal_theme::WATERMARK_MARK
                                },
                            ),
                            Part::Rim => (
                                "⠿",
                                if lit {
                                    crate::terminal_theme::BLUE
                                } else {
                                    crate::terminal_theme::WATERMARK_RIM
                                },
                            ),
                            Part::Band => ("⠿", crate::terminal_theme::WATERMARK_BAND),
                            Part::Orbit => ("⠂", crate::terminal_theme::WATERMARK_ORBIT),
                            Part::Void => unreachable!(),
                        };
                        Span::styled(glyph, Style::default().fg(color))
                    }
                })
                .collect::<Vec<_>>();
            Line::from(spans)
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn theme_picker_lines(dialog: &DialogState, height: u16) -> Vec<Line<'static>> {
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
    chat_id: String,
    sync_cursor: i64,
    last_interaction_id: i64,
    composer: Vec<char>,
    cursor: usize,
    history: Vec<String>,
    history_index: Option<usize>,
    draft_before_history: Vec<char>,
    slash_selected: usize,
    status: Vec<String>,
    model: String,
    thinking_status: String,
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
    watermark_frame: u8,
    watermark_animating: bool,
    pub(crate) theme: crate::terminal_theme::TuiTheme,
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
    pub resume_picker: Option<ResumePickerDialog>,
    pub theme_picker: Option<usize>,
    pub reply: mpsc::Sender<DialogAnswer>,
}

#[derive(Debug)]
pub(crate) struct ResumePickerDialog {
    pub sessions: Vec<mint_core::ChatSession>,
    pub current_workspace: String,
    pub current_branch: Option<String>,
    pub active_chat_id: String,
    pub show_all_projects: bool,
    pub only_current_branch: bool,
    pub sort_by_created: bool,
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
            resume_picker: None,
            theme_picker: None,
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
            resume_picker: None,
            theme_picker: None,
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
        let show_all_projects = !sessions
            .iter()
            .any(|session| session.workspace_path.as_deref() == Some(&current_workspace));
        Self {
            title: "Resume Session".to_string(),
            body: String::new(),
            options,
            filter: Vec::new(),
            selected: 0,
            scroll_offset: 0,
            checked: None,
            input: None,
            resume_picker: Some(ResumePickerDialog {
                sessions,
                current_workspace,
                current_branch,
                active_chat_id,
                show_all_projects,
                only_current_branch: false,
                sort_by_created: false,
            }),
            theme_picker: None,
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
            checked: None,
            input: None,
            resume_picker: None,
            theme_picker: Some(active.min(2)),
            reply,
        }
    }

    pub fn filtered_indices(&self) -> Vec<usize> {
        if let Some(picker) = &self.resume_picker {
            let query: String = self.filter.iter().collect::<String>().to_lowercase();
            let mut indices: Vec<usize> = picker
                .sessions
                .iter()
                .enumerate()
                .filter(|(_, session)| {
                    if !picker.show_all_projects
                        && session.workspace_path.as_deref() != Some(&picker.current_workspace)
                    {
                        return false;
                    }
                    if picker.only_current_branch {
                        match picker.current_branch.as_deref() {
                            Some(branch) if session.git_branch.as_deref() == Some(branch) => {}
                            _ => return false,
                        }
                    }
                    if !query.is_empty() {
                        let searchable = [
                            session.title.as_str(),
                            session.id.as_str(),
                            session.git_branch.as_deref().unwrap_or_default(),
                            session.main_language.as_deref().unwrap_or_default(),
                        ]
                        .join(" ")
                        .to_lowercase();
                        if !searchable.contains(&query) {
                            return false;
                        }
                    }
                    true
                })
                .map(|(index, _)| index)
                .collect();
            indices.sort_by(|left, right| {
                let left = &picker.sessions[*left];
                let right = &picker.sessions[*right];
                let (left_time, right_time) = if picker.sort_by_created {
                    (&left.created_at, &right.created_at)
                } else {
                    (&left.updated_at, &right.updated_at)
                };
                right_time.cmp(left_time)
            });
            return indices;
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
            if let Some(picker) = self.resume_picker.as_mut() {
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
                    if self.theme_picker.is_some() {
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

fn resume_picker_lines(dialog: &DialogState, width: u16, height: u16) -> Vec<Line<'static>> {
    let Some(picker) = &dialog.resume_picker else {
        return Vec::new();
    };
    let filtered = dialog.filtered_indices();
    let count = filtered.len();
    let total = picker.sessions.len();
    let muted = Style::default().fg(crate::terminal_theme::MUTED);
    let selected_chip = Style::default()
        .fg(crate::terminal_theme::SELECTION_TEXT)
        .bg(crate::terminal_theme::SELECTION_BACKGROUND)
        .add_modifier(Modifier::BOLD);
    let selected_position = if count == 0 {
        0
    } else {
        (dialog.selected + 1).min(count)
    };

    let mut lines = vec![Line::from(vec![
        Span::styled(
            "Resume Session",
            Style::default()
                .fg(crate::terminal_theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!(" ({selected_position}/{count})"), muted),
    ])];

    let mut filters = vec![
        Span::styled("Project: ", muted),
        Span::styled(
            " Cwd ",
            if picker.show_all_projects {
                muted
            } else {
                selected_chip
            },
        ),
        Span::styled(
            " All ",
            if picker.show_all_projects {
                selected_chip
            } else {
                muted
            },
        ),
        Span::styled("  ·  Branch: ", muted),
        Span::styled(
            " All ",
            if picker.only_current_branch {
                muted
            } else {
                selected_chip
            },
        ),
        Span::styled(
            " Current ",
            if picker.only_current_branch {
                selected_chip
            } else {
                muted
            },
        ),
        Span::styled("  ·  Sort: ", muted),
        Span::styled(
            " Updated ",
            if picker.sort_by_created {
                muted
            } else {
                selected_chip
            },
        ),
        Span::styled(
            " Created ",
            if picker.sort_by_created {
                selected_chip
            } else {
                muted
            },
        ),
    ];
    if picker.only_current_branch && picker.current_branch.is_none() {
        filters.push(Span::styled(" (no branch)", muted));
    }
    lines.push(Line::from(filters));

    let query: String = dialog.filter.iter().collect();
    lines.push(Line::from(vec![
        Span::styled("⌕  ", Style::default().fg(crate::terminal_theme::SUBTLE)),
        Span::styled(
            if query.is_empty() {
                "Type to search".to_string()
            } else {
                query.clone()
            },
            if query.is_empty() {
                muted
            } else {
                Style::default().fg(crate::terminal_theme::TEXT)
            },
        ),
        if query.is_empty() {
            Span::raw("")
        } else {
            Span::styled("█", Style::default().fg(crate::terminal_theme::TEXT))
        },
    ]));

    let project_name = std::path::Path::new(&picker.current_workspace)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| picker.current_workspace.clone());
    let project_label = if picker.show_all_projects {
        "All projects"
    } else {
        &project_name
    };
    let session_word = if count == 1 { "session" } else { "sessions" };
    lines.push(Line::styled(
        format!("{project_label} · {count} of {total} {session_word}"),
        muted,
    ));
    lines.push(Line::styled(
        "─".repeat(width as usize),
        Style::default().fg(crate::terminal_theme::MUTED),
    ));

    let available_rows = (height as usize).saturating_sub(8).max(1);
    let scroll_offset = if dialog.selected < dialog.scroll_offset {
        dialog.selected
    } else if dialog.selected >= dialog.scroll_offset + available_rows {
        dialog.selected + 1 - available_rows
    } else {
        dialog.scroll_offset
    }
    .min(count.saturating_sub(available_rows));
    let end = (scroll_offset + available_rows).min(count);

    if count == 0 {
        lines.push(Line::styled("No matching sessions", muted));
    } else {
        for position in scroll_offset..end {
            let session = &picker.sessions[filtered[position]];
            let time = super::resume_picker::truncate_to_width(
                &super::resume_picker::format_relative_time(&session.updated_at),
                10,
            );
            let message_count = format!("{} msgs", session.message_count);
            let current_tag = if session.id == picker.active_chat_id {
                " (current)"
            } else {
                ""
            };
            let fixed_width = 2
                + 10
                + 2
                + UnicodeWidthStr::width(message_count.as_str())
                + 2
                + UnicodeWidthStr::width(current_tag);
            let title_width = (width as usize).saturating_sub(fixed_width);
            let title = super::resume_picker::truncate_to_width(&session.title, title_width);
            let is_selected = position == dialog.selected;
            if is_selected {
                let text = format!("› {time:>10}  {title}{current_tag}  {message_count}");
                let text_width = UnicodeWidthStr::width(text.as_str());
                let padded = format!(
                    "{text}{}",
                    " ".repeat((width as usize).saturating_sub(text_width))
                );
                lines.push(Line::styled(padded, selected_chip));
            } else {
                let row = format!("  {time:>10}  {title}{current_tag}  {message_count}");
                let row_width = UnicodeWidthStr::width(row.as_str());
                lines.push(Line::from(vec![
                    Span::styled("  ", Style::default().fg(crate::terminal_theme::SUBTLE)),
                    Span::styled(format!("{time:>10}  "), muted),
                    Span::styled(
                        format!("{title}{current_tag}"),
                        Style::default().fg(crate::terminal_theme::TEXT),
                    ),
                    Span::styled(format!("  {message_count}"), muted),
                    Span::raw(" ".repeat((width as usize).saturating_sub(row_width))),
                ]));
            }
        }
    }

    while lines.len() + 2 < height.saturating_sub(1) as usize {
        lines.push(Line::raw(""));
    }
    lines.push(Line::styled(
        "Enter resume · ↑/↓ navigate · Esc cancel",
        muted,
    ));
    lines.push(Line::styled(
        "Ctrl+A project · Ctrl+B branch · Tab sort · Type to search",
        muted,
    ));
    lines
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
            state.push_notice(text);
        }
    }
    pub fn push_assistant(&self, text: impl Into<String>) {
        if let Ok(mut state) = self.state.lock() {
            state.push_assistant(text);
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
            thinking_status: format_thinking_status(&session.config),
            provider: format_provider_display_name(&session.config.ai_provider, &session.config),
            workspace: format_workspace_with_branch(&session.current_dir),
            current_dir: session.current_dir.clone(),
            theme: crate::terminal_theme::TuiTheme::from_config(&session.config.tui_theme),
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
        if let Ok(memory) = mint_core::MemoryStore::open_default() {
            self.reload_transcript_with(&memory, chat_id, workspace);
        } else {
            self.chat_id = chat_id.to_owned();
            self.transcript.clear();
            self.last_interaction_id = 0;
            self.sync_cursor = 0;
        }
    }

    pub fn reload_transcript_with(
        &mut self,
        memory: &mint_core::MemoryStore,
        chat_id: &str,
        workspace: &Path,
    ) {
        self.chat_id = chat_id.to_owned();
        self.transcript.clear();
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));
        self.sync_cursor = memory.latest_conversation_sequence(&scoped).unwrap_or(0);
        if let Ok(rows) = memory.interactions_for_chat(&scoped) {
            self.last_interaction_id = rows.last().map(|row| row.id).unwrap_or(0);
            for row in rows {
                self.transcript
                    .push(TranscriptEntry::new(TranscriptRole::User, row.user_text));
                match row.status.as_str() {
                    "completed" => self
                        .transcript
                        .push(TranscriptEntry::new(TranscriptRole::Assistant, row.ai_text)),
                    "queued" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "Queued for this session…",
                    )),
                    "running" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "Mint is responding…",
                    )),
                    "failed" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "This turn failed. Send it again to retry.",
                    )),
                    _ => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "This turn was interrupted. Send it again to retry.",
                    )),
                }
            }
        } else {
            self.last_interaction_id = 0;
        }
    }

    pub fn advance_sync_cursor(&mut self) {
        if let Ok(memory) = mint_core::MemoryStore::open_default() {
            self.advance_sync_cursor_with(&memory);
        }
    }

    pub fn advance_sync_cursor_with(&mut self, memory: &mint_core::MemoryStore) {
        if self.chat_id.is_empty() {
            return;
        }
        let scoped =
            mint_core::scoped_chat_id(&self.chat_id, Some(&self.current_dir.to_string_lossy()));
        if let Ok(seq) = memory.latest_conversation_sequence(&scoped) {
            self.sync_cursor = seq;
        }
        if let Ok(rows) = memory.interactions_for_chat(&scoped) {
            if let Some(last) = rows.last() {
                self.last_interaction_id = last.id;
            }
        }
    }

    fn refresh_shared_transcript(&mut self) -> bool {
        if self.chat_id.is_empty() {
            return false;
        }
        let Ok(memory) = mint_core::MemoryStore::open_default() else {
            return false;
        };
        self.refresh_shared_transcript_with(&memory)
    }

    pub fn refresh_shared_transcript_with(&mut self, memory: &mint_core::MemoryStore) -> bool {
        if self.chat_id.is_empty() {
            return false;
        }
        let scoped =
            mint_core::scoped_chat_id(&self.chat_id, Some(&self.current_dir.to_string_lossy()));
        let Ok(changes) = memory.conversation_changes(&scoped, self.sync_cursor, 200) else {
            return false;
        };
        if changes.changes.is_empty() {
            return false;
        }

        // If any interaction was deleted (e.g. /clear on Web or Desktop), do a full reload.
        if changes
            .changes
            .iter()
            .any(|change| change.interaction.is_none())
        {
            let chat_id = self.chat_id.clone();
            let workspace = self.current_dir.clone();
            self.reload_transcript_with(memory, &chat_id, &workspace);
            return true;
        }

        let mut changed = false;
        for change in &changes.changes {
            let Some(row) = &change.interaction else {
                continue;
            };
            if row.id > self.last_interaction_id {
                // Incoming new interaction from an external client (Web/Desktop)
                self.transcript
                    .push(TranscriptEntry::new(TranscriptRole::User, &row.user_text));
                match row.status.as_str() {
                    "completed" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Assistant,
                        &row.ai_text,
                    )),
                    "queued" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "Queued for this session…",
                    )),
                    "running" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "Mint is responding…",
                    )),
                    "failed" => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "This turn failed. Send it again to retry.",
                    )),
                    _ => self.transcript.push(TranscriptEntry::new(
                        TranscriptRole::Notice,
                        "This turn was interrupted. Send it again to retry.",
                    )),
                }
                self.last_interaction_id = row.id;
                changed = true;
            } else if row.id == self.last_interaction_id {
                // Status update for an external interaction that was previously queued/running
                if row.status == "completed" {
                    if let Some(last) = self.transcript.last_mut() {
                        if last.role == TranscriptRole::Notice
                            && (last.text.starts_with("Mint is responding")
                                || last.text.starts_with("Queued for this session"))
                        {
                            *last = TranscriptEntry::new(TranscriptRole::Assistant, &row.ai_text);
                            changed = true;
                        } else if last.role != TranscriptRole::Assistant {
                            self.transcript.push(TranscriptEntry::new(
                                TranscriptRole::Assistant,
                                &row.ai_text,
                            ));
                            changed = true;
                        }
                    }
                } else if row.status == "running" {
                    if let Some(last) = self.transcript.last_mut() {
                        if last.role == TranscriptRole::Notice
                            && last.text.starts_with("Queued for this session")
                        {
                            *last =
                                TranscriptEntry::new(TranscriptRole::Notice, "Mint is responding…");
                            changed = true;
                        }
                    }
                } else if matches!(row.status.as_str(), "failed" | "interrupted") {
                    let msg = if row.status == "failed" {
                        "This turn failed. Send it again to retry."
                    } else {
                        "This turn was interrupted. Send it again to retry."
                    };
                    if let Some(last) = self.transcript.last_mut() {
                        if last.role == TranscriptRole::Notice
                            && (last.text.starts_with("Mint is responding")
                                || last.text.starts_with("Queued for this session"))
                        {
                            *last = TranscriptEntry::new(TranscriptRole::Notice, msg);
                            changed = true;
                        }
                    }
                }
            }
        }

        self.sync_cursor = changes.cursor;
        changed
    }
    pub fn push_user(&mut self, text: String) {
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::User, text));
        self.scroll_from_bottom = 0;
    }
    pub fn push_notice(&mut self, text: impl Into<String>) {
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::Notice, text));
        self.scroll_from_bottom = 0;
    }
    pub fn push_assistant(&mut self, text: impl Into<String>) {
        self.transcript
            .push(TranscriptEntry::new(TranscriptRole::Assistant, text));
        self.scroll_from_bottom = 0;
    }
    pub fn clear_transcript(&mut self) {
        self.transcript.clear();
        self.scroll_from_bottom = 0;
        self.selection_mode = false;
    }
    pub fn sync_session(&mut self, session: &InteractiveSession) {
        self.model = active_model(&session.config.ai_provider, &session.config).to_owned();
        self.thinking_status = format_thinking_status(&session.config);
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
    fn notice_visibility_changed_since_draw(&self, painted_visible: bool) -> bool {
        painted_visible != self.active_notice().is_some()
    }
    fn transcript_text(&self) -> Text<'static> {
        let mut lines = Vec::new();
        let selected_start = self.selection_anchor.min(self.selection_head);
        let selected_end = self.selection_anchor.max(self.selection_head);
        let mut line_index = 0usize;
        for entry in &self.transcript {
            let role_header = match entry.role {
                TranscriptRole::User => Some(("You", crate::terminal_theme::BLUE)),
                TranscriptRole::Assistant => Some(("Mint", crate::terminal_theme::ACCENT)),
                TranscriptRole::Notice => None,
                TranscriptRole::Command => Some(("Command", crate::terminal_theme::ACCENT)),
                TranscriptRole::System => Some(("System", crate::terminal_theme::WARNING)),
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
                            span.style = span.style.fg(crate::terminal_theme::TEXT);
                        }
                    }
                }
                // Short AI text replies have no explicit ANSI color — default
                // them to plain white so they don't appear in the terminal's
                // dim default foreground.
                if entry.role == TranscriptRole::Assistant {
                    for span in &mut sel_line.spans {
                        if span.style.fg.is_none() {
                            span.style = span.style.fg(crate::terminal_theme::TEXT);
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
    fn render(&self, frame: &mut ratatui::Frame<'_>) -> RenderAreas {
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
        fn accent_logo_line(text: &str) -> Line<'static> {
            Line::styled(
                text.to_owned(),
                Style::default().fg(crate::terminal_theme::ACCENT),
            )
        }

        fn format_tool_status_line(line: &str) -> Line<'static> {
            if line.trim_start().starts_with('│') {
                return Line::styled(
                    line.to_string(),
                    Style::default().fg(crate::terminal_theme::MUTED),
                );
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
                    Style::default().fg(crate::terminal_theme::MUTED),
                ));
            }

            const SPINNER_CHARS: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
            let first_char = rest.chars().next();
            let (icon_span, after_icon) = if first_char == Some('✓') {
                let span = Span::styled(
                    "✓ ".to_string(),
                    Style::default().fg(crate::terminal_theme::ACCENT),
                );
                (Some(span), rest['✓'.len_utf8()..].trim_start())
            } else if let Some(c) = first_char
                && SPINNER_CHARS.contains(&c)
            {
                let span = Span::styled(
                    format!("{} ", c),
                    Style::default()
                        .fg(crate::terminal_theme::BLUE)
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
                    Style::default().fg(crate::terminal_theme::TEXT),
                ));
                spans.push(Span::styled(
                    timer.to_string(),
                    Style::default().fg(crate::terminal_theme::MUTED),
                ));
            } else {
                spans.push(Span::styled(
                    after_icon.to_string(),
                    Style::default().fg(crate::terminal_theme::TEXT),
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
                                    .fg(crate::terminal_theme::BLUE)
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
            let max_allowed = main_area.height.saturating_sub(7);
            if dialog.theme_picker.is_some() {
                15.min(max_allowed)
            } else if dialog.resume_picker.is_some() {
                let visible_rows = filtered_len.clamp(1, 24);
                (8 + visible_rows as u16).min(max_allowed)
            } else {
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
                (total as u16).min(max_allowed)
            }
        } else {
            0
        };

        let thought_height = if self.thought_modal.is_some() {
            let max_allowed = main_area.height.saturating_sub(9).min(20).max(8);
            ((main_area.height * 4) / 10).clamp(8, max_allowed)
        } else {
            0
        };

        let composer_width = main_area.width.saturating_sub(4).max(1) as usize;
        let (composer_rows, cursor_row, cursor_col) =
            wrap_input_visual_into_rows(&self.composer, composer_width, self.cursor);
        let visible_composer_rows = composer_rows.len().clamp(1, 6);
        let composer_height = visible_composer_rows as u16 + 2;
        let composer_scroll = cursor_row
            .saturating_add(1)
            .saturating_sub(visible_composer_rows);
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
            accent_logo_line(" __  __ _       _    ___ _    ___"),
            accent_logo_line(r"|  \/  (_)_ __ | |_ / __| |  |_ _|"),
            accent_logo_line(r"| |\/| | | '_ \|  _| (__| |__ | |"),
            accent_logo_line(r"|_|  |_|_|_| |_|\___|\___|\___|___|"),
        ]);
        frame.render_widget(Paragraph::new(logo), header_columns[0]);

        let details = Text::from(vec![
            Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    "[Mint] ",
                    Style::default()
                        .fg(crate::terminal_theme::ACCENT)
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
                Span::styled(
                    line2_text,
                    Style::default().fg(crate::terminal_theme::MUTED),
                ),
            ]),
        ]);
        let details_area = ratatui::layout::Rect {
            y: rows[0].y,
            height: 4,
            ..header_columns[1]
        };
        frame.render_widget(
            Paragraph::new(details).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(crate::terminal_theme::MUTED)),
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
        if self.transcript.is_empty() && self.dialog.is_none() && self.thought_modal.is_none() {
            let watermark_height = WATERMARK_HEIGHT.min(rows[1].height);
            let watermark_width = WATERMARK_WIDTH.min(rows[1].width);
            let watermark_area = Rect::new(
                rows[1].x + rows[1].width.saturating_sub(watermark_width) / 2,
                rows[1].y + rows[1].height.saturating_sub(WATERMARK_HEIGHT) / 2,
                watermark_width,
                watermark_height,
            );
            let watermark = mint_m_watermark(
                self.watermark_animating.then_some(self.watermark_frame),
                watermark_height as usize,
            );
            frame.render_widget(
                Paragraph::new(watermark).alignment(ratatui::layout::Alignment::Center),
                watermark_area,
            );
        }
        if max_scroll > 0 {
            // Paragraph scroll ranges from 0 to content - viewport, while
            // Ratatui's scrollbar position ranges from 0 to content - 1.
            // Map between those ranges so both endpoints align with the track.
            let scrollbar_position = ((scroll as f64 / max_scroll as f64)
                * total.saturating_sub(1) as f64)
                .round() as usize;
            let mut bar = ScrollbarState::new(total)
                .viewport_content_length(rows[1].height as usize)
                .position(scrollbar_position);
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
                        .fg(crate::terminal_theme::ACCENT)
                        .bg(crate::terminal_theme::PANEL_BACKGROUND)
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
                    if (line.find(" (").is_some() || line.contains("Compacting context"))
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
                                        .fg(crate::terminal_theme::BLUE)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::styled(
                                    text.to_string(),
                                    Style::default()
                                        .fg(crate::terminal_theme::TEXT)
                                        .add_modifier(Modifier::BOLD),
                                ),
                            ])
                        } else {
                            Line::styled(
                                line.to_string(),
                                Style::default()
                                    .fg(crate::terminal_theme::TEXT)
                                    .add_modifier(Modifier::BOLD),
                            )
                        }
                    } else {
                        // Activity/verb lines: "Reading 1 file…", "Running 2
                        // commands…", etc. — bold white for clarity.
                        Line::styled(
                            line.to_string(),
                            Style::default()
                                .fg(crate::terminal_theme::TEXT)
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
            if dialog.theme_picker.is_some() {
                frame.render_widget(
                    Paragraph::new(theme_picker_lines(dialog, rows[3].height)).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(crate::terminal_theme::MUTED)),
                    ),
                    rows[3],
                );
            } else if dialog.resume_picker.is_some() {
                let lines = resume_picker_lines(dialog, rows[3].width, rows[3].height);
                frame.render_widget(
                    Paragraph::new(lines).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(crate::terminal_theme::MUTED)),
                    ),
                    rows[3],
                );
            } else {
                let mut lines = Vec::new();
                let filtered = dialog.filtered_indices();
                let total_options = filtered.len();
                let monochrome_picker = dialog.title == "Resume Session";

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
                            .fg(if monochrome_picker {
                                crate::terminal_theme::TEXT
                            } else {
                                crate::terminal_theme::BLUE
                            })
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        title_suffix,
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ),
                ]));

                if !dialog.body.is_empty() {
                    for line in dialog.body.lines() {
                        lines.push(Line::from(vec![
                            Span::raw(" "),
                            Span::styled(
                                line.to_owned(),
                                Style::default().fg(crate::terminal_theme::MUTED),
                            ),
                        ]));
                    }
                }

                let mut filter_row_idx = None;
                if !dialog.filter.is_empty() {
                    filter_row_idx = Some(lines.len());
                    let filter_str: String = dialog.filter.iter().collect();
                    lines.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(
                            "Filter: ",
                            Style::default().fg(crate::terminal_theme::MUTED),
                        ),
                        Span::styled(
                            filter_str,
                            Style::default()
                                .fg(if monochrome_picker {
                                    crate::terminal_theme::TEXT
                                } else {
                                    crate::terminal_theme::WARNING
                                })
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            format!(" ({total_options}/{})", dialog.options.len()),
                            Style::default().fg(crate::terminal_theme::MUTED),
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
                                .fg(crate::terminal_theme::ACCENT)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::raw(input_text.clone()),
                        Span::styled("█", Style::default().fg(crate::terminal_theme::ACCENT)),
                    ]));
                    lines.push(Line::raw(""));
                    lines.push(Line::from(vec![
                        Span::raw(" "),
                        Span::styled(
                            "Enter submit · Esc cancel",
                            Style::default().fg(crate::terminal_theme::MUTED),
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
                                Style::default().fg(crate::terminal_theme::MUTED),
                            ),
                        ]));
                    } else {
                        if scroll_offset > 0 {
                            lines.push(Line::from(vec![
                                Span::raw("   "),
                                Span::styled(
                                    format!("▲ {scroll_offset} more above"),
                                    Style::default().fg(crate::terminal_theme::MUTED),
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

                            let (label, desc) = if let Some((l, d)) = option_text.split_once(" — ")
                            {
                                (l, Some(d))
                            } else if let Some((l, d)) = option_text.split_once('\t') {
                                (l, Some(d))
                            } else {
                                (option_text.as_str(), None)
                            };

                            if is_selected {
                                let selected_color = if monochrome_picker {
                                    crate::terminal_theme::SELECTION_TEXT
                                } else {
                                    crate::terminal_theme::ACCENT
                                };
                                let selected_style = if monochrome_picker {
                                    Style::default()
                                        .fg(crate::terminal_theme::SELECTION_TEXT)
                                        .bg(crate::terminal_theme::SELECTION_BACKGROUND)
                                        .add_modifier(Modifier::BOLD)
                                } else {
                                    Style::default()
                                        .fg(selected_color)
                                        .add_modifier(Modifier::BOLD)
                                };
                                let mut spans = vec![
                                    Span::styled(" › ", selected_style),
                                    Span::styled(shortcut, selected_style),
                                    Span::styled(marker, selected_style),
                                    Span::styled(label, selected_style),
                                ];
                                if let Some(desc) = desc {
                                    spans.push(Span::styled("   ", selected_style));
                                    spans.push(Span::styled(desc, selected_style));
                                }
                                lines.push(Line::from(spans));
                            } else {
                                let mut spans = vec![
                                    Span::raw("   "),
                                    Span::styled(
                                        shortcut,
                                        Style::default().fg(crate::terminal_theme::MUTED),
                                    ),
                                    Span::styled(
                                        marker,
                                        Style::default().fg(crate::terminal_theme::MUTED),
                                    ),
                                    Span::styled(
                                        label,
                                        Style::default().fg(crate::terminal_theme::TEXT),
                                    ),
                                ];
                                if let Some(desc) = desc {
                                    spans.push(Span::styled("   ", Style::default()));
                                    spans.push(Span::styled(
                                        desc,
                                        Style::default().fg(crate::terminal_theme::MUTED),
                                    ));
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
                                    Style::default().fg(crate::terminal_theme::MUTED),
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
                    lines.push(Line::styled(
                        hint,
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ));
                    lines.push(Line::raw(""));
                }

                frame.render_widget(
                    Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(crate::terminal_theme::MUTED)),
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
                .border_style(Style::default().fg(crate::terminal_theme::BLUE))
                .title(Span::styled(
                    format!(" Thought Process ({}) ", modal.elapsed_str),
                    Style::default()
                        .fg(crate::terminal_theme::BLUE)
                        .add_modifier(Modifier::BOLD),
                ))
                .title_bottom(Span::styled(
                    scroll_hint,
                    Style::default().fg(crate::terminal_theme::MUTED),
                ));

            let visible = &wrapped_lines[scroll..(scroll + inner_height).min(wrapped_lines.len())];
            let mut p_lines: Vec<Line> = Vec::with_capacity(inner_height);
            for l in visible {
                p_lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(l.clone(), Style::default().fg(crate::terminal_theme::TEXT)),
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
                    Style::default().fg(crate::terminal_theme::MUTED),
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
                                    .fg(crate::terminal_theme::BLUE)
                                    .add_modifier(Modifier::BOLD | Modifier::REVERSED)
                            } else {
                                Style::default().fg(crate::terminal_theme::MUTED)
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
                            .fg(crate::terminal_theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        "Ask anything...",
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ),
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
                                    .fg(crate::terminal_theme::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            )];
                            spans.extend(format_composer_row_spans(row));
                            Line::from(spans)
                        })
                        .collect::<Vec<_>>(),
                )
            };
            frame.render_widget(
                Paragraph::new(input)
                    .wrap(Wrap { trim: false })
                    .scroll((composer_scroll as u16, 0))
                    .block(
                        Block::default()
                            .borders(Borders::TOP | Borders::BOTTOM)
                            .border_style(Style::default().fg(crate::terminal_theme::MUTED)),
                    ),
                rows[4],
            );

            let mode_label = if self.plan_mode {
                " [Plan] "
            } else {
                " [Agent] "
            };
            let bg_running =
                mint_core::bg_shell::running_count_for(std::path::Path::new(&self.workspace));
            let jobs_prefix = if bg_running > 0 {
                format!(
                    "{} background terminal{} · /shells · ",
                    bg_running,
                    if bg_running == 1 { "" } else { "s" }
                )
            } else {
                String::new()
            };
            let path_text = format!("path: {}", self.workspace);
            let thinking_text = format!("• {}", self.thinking_status);
            let left_len = mode_label.chars().count()
                + self.model.chars().count()
                + thinking_text.chars().count()
                + 1;
            let right_len = jobs_prefix.chars().count() + path_text.chars().count();
            let available = rows[5].width as usize;
            let gap = available.saturating_sub(left_len + right_len);
            let footer = if let Some(notice) = self.active_notice() {
                Line::styled(
                    format!(" {notice}"),
                    Style::default()
                        .fg(crate::terminal_theme::WARNING)
                        .add_modifier(Modifier::BOLD),
                )
            } else if self.selection_mode {
                Line::styled(
                    " Selection mode · ↑/↓ scroll · Enter/y copy · Esc cancel",
                    Style::default().fg(crate::terminal_theme::WARNING),
                )
            } else if bg_running > 0 && left_len + right_len > available {
                Line::styled(
                    format!(
                        " {bg_running} terminal{} · /shells ·{}",
                        if bg_running == 1 { "" } else { "s" },
                        mode_label
                    ),
                    Style::default().fg(crate::terminal_theme::BLUE),
                )
            } else {
                Line::from(vec![
                    Span::styled(
                        mode_label,
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ),
                    Span::styled(
                        &self.model,
                        Style::default()
                            .fg(crate::terminal_theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        thinking_text,
                        Style::default().fg(crate::terminal_theme::ACCENT),
                    ),
                    Span::raw(" ".repeat(gap)),
                    Span::styled(
                        jobs_prefix,
                        Style::default().fg(crate::terminal_theme::BLUE),
                    ),
                    Span::styled(path_text, Style::default().fg(crate::terminal_theme::MUTED)),
                ])
            };
            frame.render_widget(Paragraph::new(footer), rows[5]);

            if !self.selection_mode {
                let row_start = self.cursor.saturating_sub(cursor_col);
                let visual_col = UnicodeWidthStr::width(
                    self.composer[row_start..self.cursor]
                        .iter()
                        .collect::<String>()
                        .as_str(),
                );
                frame.set_cursor_position(Position::new(
                    rows[4].x + 3 + visual_col as u16,
                    rows[4].y
                        + 1
                        + cursor_row
                            .saturating_sub(composer_scroll)
                            .min(visible_composer_rows.saturating_sub(1))
                            as u16,
                ));
            }
        }
        RenderAreas {
            back_to_bottom: back_to_bottom_area,
            composer: rows.get(4).copied(),
            watermark: if self.transcript.is_empty()
                && self.dialog.is_none()
                && self.thought_modal.is_none()
            {
                Some(Rect::new(
                    rows[1].x
                        + rows[1]
                            .width
                            .saturating_sub(WATERMARK_WIDTH.min(rows[1].width))
                            / 2,
                    rows[1].y + rows[1].height.saturating_sub(WATERMARK_HEIGHT) / 2,
                    WATERMARK_WIDTH.min(rows[1].width),
                    WATERMARK_HEIGHT.min(rows[1].height),
                ))
            } else {
                None
            },
        }
    }
}

#[derive(Default)]
struct RenderAreas {
    back_to_bottom: Option<Rect>,
    composer: Option<Rect>,
    watermark: Option<Rect>,
}

pub(crate) struct ChatTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
    mouse_selection: MouseSelection,
    last_frame: Option<Buffer>,
    back_to_bottom_area: Option<Rect>,
    composer_area: Option<Rect>,
    watermark_area: Option<Rect>,
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

    fn endpoints(&self) -> Option<(ScreenPoint, ScreenPoint)> {
        Some((self.anchor?, self.head?))
    }
}

fn input_index_at(
    point: ScreenPoint,
    area: Rect,
    chars: &[char],
    row_width: usize,
    cursor_pos: usize,
    end_after_character: bool,
) -> Option<usize> {
    let first_text_row = area.y + 1;
    let visible_row = point.row.checked_sub(first_text_row)? as usize;
    let visible_rows = ((area.height as usize).saturating_sub(2)).min(6);
    if visible_row >= visible_rows {
        return None;
    }

    let (rows, cursor_row, _) = wrap_input_visual_into_rows(chars, row_width, cursor_pos);
    let scroll = cursor_row.saturating_add(1).saturating_sub(visible_rows);
    let row_index = visible_row + scroll;
    let row = rows.get(row_index)?;
    let mut start = 0usize;
    for prior_row in rows.iter().take(row_index) {
        start += prior_row.chars().count();
        if chars.get(start) == Some(&'\n') {
            start += 1;
        }
    }

    let text_x = area.x.saturating_add(3);
    let target_column = point.column.saturating_sub(text_x) as usize;
    let row_chars: Vec<char> = row.chars().collect();
    let mut visual_column = 0usize;
    for (index, character) in row_chars.iter().enumerate() {
        let width = UnicodeWidthStr::width(character.to_string().as_str()).max(1);
        if target_column < visual_column + width {
            return Some(start + index + usize::from(end_after_character));
        }
        visual_column += width;
    }
    Some(start + row_chars.len())
}

fn wrap_input_visual_into_rows(
    input_chars: &[char],
    row_width: usize,
    cursor_pos: usize,
) -> (Vec<String>, usize, usize) {
    let row_width = row_width.max(1);
    let mut rows: Vec<String> = Vec::new();
    let mut row_starts = Vec::new();
    let mut row_start = 0usize;
    let mut visual_width = 0usize;

    for (index, character) in input_chars.iter().copied().enumerate() {
        if character == '\n' {
            rows.push(input_chars[row_start..index].iter().collect());
            row_starts.push(row_start);
            row_start = index + 1;
            visual_width = 0;
            continue;
        }

        let character_width = UnicodeWidthStr::width(character.to_string().as_str());
        if visual_width > 0 && visual_width + character_width > row_width {
            rows.push(input_chars[row_start..index].iter().collect());
            row_starts.push(row_start);
            row_start = index;
            visual_width = 0;
        }
        visual_width += character_width;
    }

    if row_start < input_chars.len() || input_chars.last() == Some(&'\n') || rows.is_empty() {
        rows.push(input_chars[row_start..].iter().collect());
        row_starts.push(row_start);
    }

    let cursor_pos = cursor_pos.min(input_chars.len());
    let mut cursor_row = rows.len().saturating_sub(1);
    for (index, start) in row_starts.iter().copied().enumerate() {
        let end = start + rows[index].chars().count();
        let next_starts_here = row_starts.get(index + 1) == Some(&cursor_pos);
        if cursor_pos < end || (cursor_pos == end && !next_starts_here) {
            cursor_row = index;
            break;
        }
        if cursor_pos == start {
            cursor_row = index;
            break;
        }
    }
    let cursor_col = cursor_pos.saturating_sub(row_starts[cursor_row]);
    (rows, cursor_row, cursor_col)
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
    pub fn enter(workspace: &Path) -> Result<Self> {
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
        let workspace_name = workspace
            .file_name()
            .unwrap_or_else(|| workspace.as_os_str())
            .to_string_lossy();
        let workspace_name: String = workspace_name
            .chars()
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect();
        let title = if workspace_name.trim().is_empty() {
            "Mint Agent | workspace".to_owned()
        } else {
            format!("Mint Agent | {}", workspace_name.trim())
        };
        let setup = setup_terminal_output(&mut stdout);
        if let Err(error) = setup {
            restore_terminal();
            return Err(error.into());
        }
        match Terminal::new(CrosstermBackend::new(stdout)) {
            Ok(terminal) => {
                let _ = io::stdout().execute(terminal::SetTitle(title));
                Ok(Self {
                    terminal,
                    active: true,
                    mouse_selection: MouseSelection::default(),
                    last_frame: None,
                    back_to_bottom_area: None,
                    composer_area: None,
                    watermark_area: None,
                    clipboard: TextClipboard::default(),
                })
            }
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
        let mut areas = RenderAreas::default();
        let completed = self.terminal.draw(|frame| {
            areas = state.render(frame);
            if selection.range().is_some() && state.active_notice().is_none() {
                let area = frame.area();
                let hint_area = Rect::new(area.x, area.bottom().saturating_sub(1), area.width, 1);
                frame.render_widget(
                    Paragraph::new(Line::styled(
                        " Ctrl+C / right-click copy · Delete input · Esc clear",
                        Style::default().fg(crate::terminal_theme::MUTED),
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
            crate::terminal_theme::apply_tui_theme(frame.buffer_mut(), state.theme);
        })?;
        self.last_frame = Some(completed.buffer.clone());
        self.back_to_bottom_area = areas.back_to_bottom;
        self.composer_area = areas.composer;
        self.watermark_area = areas.watermark;
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

    fn composer_selection_range(&self, state: &ChatViewState) -> Option<(usize, usize)> {
        let area = self.composer_area?;
        let (anchor, head) = self.mouse_selection.endpoints()?;
        let (start, end) = if anchor <= head {
            (anchor, head)
        } else {
            (head, anchor)
        };
        let chars = &state.composer;
        let row_width = (area.width as usize).saturating_sub(4).max(1);
        let start = input_index_at(start, area, chars, row_width, state.cursor, false)?;
        let end = input_index_at(end, area, chars, row_width, state.cursor, true)?;
        (start < end).then_some((start.min(chars.len()), end.min(chars.len())))
    }

    fn copy_mouse_selection(&mut self, state: &mut ChatViewState) -> bool {
        if self.mouse_selection.range().is_none() {
            return false;
        }
        let text = if let Some((start, end)) = self.composer_selection_range(state) {
            state.composer[start..end].iter().collect::<String>()
        } else {
            self.last_frame
                .as_ref()
                .map(|buffer| selected_screen_text(buffer, self.mouse_selection))
                .unwrap_or_default()
        };
        if text.trim().is_empty() {
            self.clear_mouse_selection();
            state.set_notice("Nothing to copy from this selection");
            return true;
        }
        if text.len() > COPY_LIMIT {
            state.set_notice("Selection exceeds the 100 KiB copy limit");
        } else {
            let character_count = text.chars().count();
            match self.clipboard.copy(&text, &mut io::stdout()) {
                Ok(CopyMethod::Native | CopyMethod::Osc52) => {
                    state.set_notice(format!("Copied {character_count} chars to clipboard"))
                }
                Err(error) => state.set_notice(format!("Could not copy selection: {error}")),
            }
        }
        true
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
        let mut last_sync = std::time::Instant::now();
        let mut previous_bg_count =
            mint_core::bg_shell::running_count_for(std::path::Path::new(&state.workspace));
        let mut redraw = true;
        let mut painted_notice_visible = false;
        let mut watermark_started: Option<std::time::Instant> = None;
        const DOUBLE_PRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

        loop {
            let watermark_visible = state.transcript.is_empty()
                && state.dialog.is_none()
                && state.thought_modal.is_none();
            if state.watermark_animating && watermark_visible {
                let started = *watermark_started.get_or_insert_with(std::time::Instant::now);
                match watermark_animation_at(started.elapsed()).0 {
                    Some(frame) if frame != state.watermark_frame => {
                        state.watermark_frame = frame;
                        redraw = true;
                    }
                    None => {
                        state.watermark_frame = 0;
                        state.watermark_animating = false;
                        watermark_started = None;
                        redraw = true;
                    }
                    _ => {}
                }
            } else {
                watermark_started = None;
            }

            if let Some(time) = last_ctrl_d {
                if time.elapsed() >= DOUBLE_PRESS_TIMEOUT {
                    last_ctrl_d = None;
                    if state.active_notice() == Some("Press Ctrl+D again to exit") {
                        state.clear_notice();
                    }
                }
            }

            let bg_count =
                mint_core::bg_shell::running_count_for(std::path::Path::new(&state.workspace));
            if bg_count != previous_bg_count {
                previous_bg_count = bg_count;
                redraw = true;
            }
            for notice in mint_core::bg_shell::take_finished_notices() {
                state.push_notice(notice);
                redraw = true;
            }
            redraw |= state.notice_visibility_changed_since_draw(painted_notice_visible);

            if redraw {
                let notice_visible = state.active_notice().is_some();
                self.draw_state(&state)?;
                painted_notice_visible = notice_visible;
                redraw = false;
            }

            let watermark_animating = state.watermark_animating && watermark_visible;
            {
                let poll_interval = if watermark_animating {
                    let elapsed = watermark_started
                        .expect("active watermark has a start time")
                        .elapsed();
                    watermark_animation_at(elapsed)
                        .1
                        .unwrap_or(elapsed)
                        .saturating_sub(elapsed)
                        .min(std::time::Duration::from_millis(250))
                } else if last_ctrl_d.is_some() || state.active_notice().is_some() {
                    std::time::Duration::from_millis(50)
                } else {
                    std::time::Duration::from_millis(250)
                };
                if !event::poll(poll_interval)? {
                    if last_sync.elapsed() >= std::time::Duration::from_millis(1500)
                        && self.mouse_selection.range().is_none()
                        && !state.selection_mode
                    {
                        redraw |= state.refresh_shared_transcript();
                        last_sync = std::time::Instant::now();
                    }
                    continue;
                }
            }

            let input_event = event::read()?;
            if matches!(
                &input_event,
                event::Event::Mouse(m) if m.kind == event::MouseEventKind::Moved
            ) {
                continue;
            }
            redraw = true;
            match input_event {
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
                    if m.kind == event::MouseEventKind::Down(event::MouseButton::Left)
                        && self
                            .watermark_area
                            .is_some_and(|area| area.contains(Position::new(m.column, m.row)))
                    {
                        self.clear_mouse_selection();
                        state.watermark_frame = 0;
                        state.watermark_animating = true;
                        watermark_started = Some(std::time::Instant::now());
                        continue;
                    }
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

                    let is_ctrl_c = key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if is_ctrl_c && self.copy_mouse_selection(&mut state) {
                        continue;
                    }

                    if self.mouse_selection.range().is_some() {
                        if matches!(key.code, KeyCode::Delete | KeyCode::Backspace) {
                            if let Some((start, end)) = self.composer_selection_range(&state) {
                                state.composer.drain(start..end);
                                state.cursor = start;
                                state.slash_selected = 0;
                                state.clear_notice();
                            }
                            self.clear_mouse_selection();
                            continue;
                        }
                    }

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

    pub fn prompt_theme(&mut self, handle: &TuiHandle, current: usize) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_theme_picker(current, reply));
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

    pub fn prompt_resume_picker(
        &mut self,
        handle: &TuiHandle,
        sessions: Vec<mint_core::ChatSession>,
        current_dir: &std::path::Path,
        current_branch: Option<String>,
        active_chat_id: &str,
    ) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_resume_picker(
                sessions,
                current_dir.to_string_lossy().to_string(),
                current_branch,
                active_chat_id.to_string(),
                reply,
            ));
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
                        let mut selection_handled = false;
                        if let Ok(mut state) = handle.state.lock() {
                            if let Some(dialog) = state.dialog.as_mut() {
                                if let Some(answer) = dialog.handle_key(key) {
                                    if let Some(dialog) = state.dialog.take() {
                                        let _ = dialog.reply.send(answer);
                                    }
                                }
                                selection_handled = true;
                            } else if key.code == KeyCode::Char('c')
                                && key.modifiers.contains(KeyModifiers::CONTROL)
                                && self.copy_mouse_selection(&mut state)
                            {
                                selection_handled = true;
                            } else if self.mouse_selection.range().is_some()
                                && matches!(key.code, KeyCode::Delete | KeyCode::Backspace)
                            {
                                if let Some((start, end)) = self.composer_selection_range(&state) {
                                    state.composer.drain(start..end);
                                    state.cursor = start;
                                    state.slash_selected = 0;
                                }
                                self.clear_mouse_selection();
                                selection_handled = true;
                            }
                        }
                        if selection_handled {
                            continue;
                        }
                        self.clear_mouse_selection();
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
                                KeyCode::Delete if state.cursor < state.composer.len() => {
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

    #[test]
    fn mint_watermark_frames_fit_available_terminal_height() {
        for height in [3, 7, WATERMARK_HEIGHT as usize] {
            for frame in 0..WATERMARK_FRAME_COUNT {
                let text = mint_m_watermark(Some(frame), height);
                assert_eq!(text.lines.len(), height);
                assert!(text.lines.iter().all(|line| {
                    line.spans
                        .iter()
                        .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
                        .sum::<usize>()
                        == WATERMARK_WIDTH as usize
                }));
            }
        }
        assert_ne!(
            mint_m_watermark(Some(0), WATERMARK_HEIGHT as usize),
            mint_m_watermark(Some(6), WATERMARK_HEIGHT as usize)
        );
        let front = mint_m_watermark(None, WATERMARK_HEIGHT as usize);
        assert_eq!(front.lines[8].spans[19].content.as_ref(), " ");
        let back = mint_m_watermark(Some(WATERMARK_FRAME_COUNT / 2), WATERMARK_HEIGHT as usize);
        assert_eq!(back.lines[8].spans[19].content.as_ref(), "⣿");
    }

    #[test]
    fn watermark_animation_eases_to_leaf_holds_then_continues_forward_to_m() {
        let at = |ms| watermark_animation_at(std::time::Duration::from_millis(ms));
        assert_eq!(at(0).0, Some(0));
        // Slower movement near the endpoints, faster through the middle.
        assert_eq!(at(300).0, Some(2));
        assert_eq!(at(1500).0, Some(50));
        assert_eq!(at(2700).0, Some(98));
        assert_eq!(
            at(3000),
            (Some(100), Some(std::time::Duration::from_millis(4000)))
        );
        assert_eq!(
            at(3999),
            (Some(100), Some(std::time::Duration::from_millis(4000)))
        );
        assert_eq!(at(4000).0, Some(100));
        assert_eq!(at(4300).0, Some(102));
        assert_eq!(at(5500).0, Some(150));
        assert_eq!(at(6700).0, Some(198));
        assert_eq!(
            at(6999),
            (Some(0), Some(std::time::Duration::from_millis(7000)))
        );
        assert_eq!(at(7000), (None, None));
    }

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
    fn background_count_is_workspace_scoped_and_visible_in_narrow_tui() {
        let root = std::env::temp_dir().join(format!(
            "mint-tui-bg-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        let config = mint_core::MintConfig {
            safety_enabled: false,
            sandbox_mode: "off".into(),
            ..Default::default()
        };
        let first = mint_core::bg_shell::start_background(&root, &config, "sleep 30").unwrap();
        let second = mint_core::bg_shell::start_background(&root, &config, "sleep 30").unwrap();
        struct Cleanup(Vec<String>, std::path::PathBuf);
        impl Drop for Cleanup {
            fn drop(&mut self) {
                for id in &self.0 {
                    let _ = mint_core::bg_shell::kill_job(id);
                }
                let _ = std::fs::remove_dir(&self.1);
            }
        }
        let _cleanup = Cleanup(vec![first.id, second.id], root.clone());
        let mut state = ChatViewState::default();
        state.workspace = root.to_string_lossy().into_owned();
        for (width, expected) in [(160, "2 background terminals"), (40, "2 terminals")] {
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
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
            assert!(
                rendered.contains(expected),
                "missing job count at width {width}: {rendered}"
            );
            assert!(rendered.contains("/shells"));
        }
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
        buffer.set_string(
            0,
            0,
            "Mint",
            Style::default().fg(crate::terminal_theme::ACCENT),
        );
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
        terminal
            .draw(|frame| button = state.render(frame).back_to_bottom)
            .unwrap();
        assert!(button.is_none());

        state.scroll_from_bottom = 10;
        terminal
            .draw(|frame| button = state.render(frame).back_to_bottom)
            .unwrap();
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
        terminal
            .draw(|frame| button = state.render(frame).back_to_bottom)
            .unwrap();
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
        assert!(state.notice_visibility_changed_since_draw(true));
        assert!(!state.notice_visibility_changed_since_draw(false));
    }

    #[test]
    fn test_refresh_shared_transcript_preserves_tool_notice_order() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mint-test-tui-sync-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test_memory.db");
        let memory = mint_core::MemoryStore::open(db_path);
        let workspace = temp_dir.clone();
        let chat_id = "test-session";
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));

        let mut state = ChatViewState {
            chat_id: chat_id.to_string(),
            current_dir: workspace.clone(),
            ..ChatViewState::default()
        };
        state.reload_transcript_with(&memory, chat_id, &workspace);

        // Turn 1 runs locally in CLI:
        state.push_user("List files please".into());
        state.push_notice("• Listing 2 directories...");
        state.push_notice("✓ Completed in 1.2s • 1 tool • 0 files changed");
        state.push_assistant("Found 2 files.");
        state.push_notice("─ Worked for 1s • test ──────────────────────────────");

        // In DB, TurnLease records this interaction:
        let turn_id = memory.start_turn(&scoped, "List files please").unwrap();
        memory.claim_turn(&scoped, turn_id).unwrap();
        memory
            .finish_turn(turn_id, "Found 2 files.", "test", "model", None)
            .unwrap();

        // Advance cursor as local turn completion would:
        state.advance_sync_cursor_with(&memory);

        // Refresh shared transcript:
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(
            !redrawn,
            "Should not redraw since changes were from local turn"
        );

        // Now simulate an external message arriving from Web/Desktop on the same session:
        let ext_id = memory.start_turn(&scoped, "Message from Web").unwrap();
        memory.claim_turn(&scoped, ext_id).unwrap();
        memory
            .finish_turn(ext_id, "Reply to Web", "test", "model", None)
            .unwrap();

        let redrawn2 = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn2, "Should redraw when external turn arrives");

        // Verify transcript order:
        // Turn 1's tool notices must still be BEFORE Turn 1's assistant reply!
        // Not pushed to the very bottom!
        let texts: Vec<&str> = state.transcript.iter().map(|e| e.text.as_str()).collect();
        assert_eq!(texts.len(), 7);
        assert_eq!(texts[0], "List files please");
        assert_eq!(texts[1], "• Listing 2 directories...");
        assert_eq!(texts[2], "✓ Completed in 1.2s • 1 tool • 0 files changed");
        assert_eq!(texts[3], "Found 2 files.");
        assert_eq!(
            texts[4],
            "─ Worked for 1s • test ──────────────────────────────"
        );
        assert_eq!(texts[5], "Message from Web");
        assert_eq!(texts[6], "Reply to Web");

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_refresh_shared_transcript_external_turn_lifecycle() {
        let temp_dir = std::env::temp_dir().join(format!(
            "mint-test-tui-sync-lifecycle-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::create_dir_all(&temp_dir);
        let db_path = temp_dir.join("test_memory.db");
        let memory = mint_core::MemoryStore::open(db_path);
        let workspace = temp_dir.clone();
        let chat_id = "test-session-2";
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));

        let mut state = ChatViewState {
            chat_id: chat_id.to_string(),
            current_dir: workspace.clone(),
            ..ChatViewState::default()
        };
        state.reload_transcript_with(&memory, chat_id, &workspace);

        // Turn arrives from Web (status: queued):
        let ext_id = memory.start_turn(&scoped, "External prompt").unwrap();
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[0].text, "External prompt");
        assert_eq!(state.transcript[1].text, "Queued for this session…");

        // External turn updates to running:
        memory.claim_turn(&scoped, ext_id).unwrap();
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[0].text, "External prompt");
        assert_eq!(state.transcript[1].text, "Mint is responding…");

        // External turn completes:
        memory
            .finish_turn(ext_id, "External answer", "provider", "model", None)
            .unwrap();
        let redrawn = state.refresh_shared_transcript_with(&memory);
        assert!(redrawn);
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(state.transcript[0].text, "External prompt");
        assert_eq!(state.transcript[1].text, "External answer");
        assert_eq!(state.transcript[1].role, TranscriptRole::Assistant);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }
}

#[cfg(test)]
mod context_compaction_shimmer_tests {
    use super::*;
    #[test]
    fn full_screen_compaction_shimmers_only_the_label() {
        let text = "✦ Compacting context · 12s • Esc to interrupt";
        let first = shimmer_thinking_line(text, 0);
        let next = shimmer_thinking_line(text, 12);
        assert_ne!(first, next);
        assert_eq!(
            first.spans.last().unwrap().content,
            " · 12s • Esc to interrupt"
        );
        assert_eq!(first.spans.last(), next.spans.last());
    }
}

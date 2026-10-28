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

fn normalize_prompt_newlines(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

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
            TranscriptRole::User => {
                crate::agent::format_markdown_bold(&normalize_prompt_newlines(text))
            }
            TranscriptRole::Notice | TranscriptRole::Command | TranscriptRole::System => {
                crate::agent::format_markdown_bold(text)
            }
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SwarmUiStatus {
    Running,
    Success,
    Failed,
}

#[derive(Debug, Clone)]
pub(crate) struct SwarmAgentUiState {
    pub label: String,
    pub profile: String,
    pub task: String,
    pub current_action: Option<String>,
    pub status: SwarmUiStatus,
    pub started_at: std::time::Instant,
    pub finished_elapsed_ms: Option<u64>,
    pub summary_preview: Option<String>,
}

fn clip_text(text: &str, width: usize) -> String {
    if UnicodeWidthStr::width(text) <= width {
        return text.to_owned();
    }
    let mut used = 0;
    let mut clipped = String::new();
    for ch in text.chars() {
        let next = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
        if used + next > width.saturating_sub(1) {
            break;
        }
        used += next;
        clipped.push(ch);
    }
    if width > 0 {
        clipped.push('…');
    }
    clipped
}

fn format_subagent_tool_action(action: &str, input: &serde_json::Value) -> String {
    for key in [
        "description",
        "command",
        "cmd",
        "path",
        "query",
        "pattern",
        "url",
    ] {
        if let Some(val) = input.get(key).and_then(|v| v.as_str()) {
            if let Some(line) = val.lines().find(|l| !l.trim().is_empty()) {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("null") {
                    continue;
                }
                return match key {
                    "command" | "cmd" => format!("Running `{trimmed}`"),
                    "pattern" | "query" => format!("Searching for {trimmed}"),
                    "path" => format!("Reading {trimmed}"),
                    _ => trimmed.to_string(),
                };
            }
        }
    }
    action.replace('_', " ")
}

#[derive(Debug, Default)]
pub(crate) struct ChatViewState {
    pub(crate) transcript: Vec<TranscriptEntry>,
    pub(crate) swarm_agents: Vec<SwarmAgentUiState>,
    chat_id: String,
    mods: Vec<serde_json::Value>,
    mods_open: bool,
    mod_focus: usize,
    mod_input: Option<(String, String)>,
    sync_cursor: i64,
    last_interaction_id: i64,
    composer: Vec<char>,
    collapsed_pastes: Vec<CollapsedPaste>,
    expanded_paste: Option<std::ops::Range<usize>>,
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
    scroll_from_bottom: usize,
    selection_mode: bool,
    selection_anchor: usize,
    selection_head: usize,
    notice: Option<(String, std::time::Instant)>,
    dialog: Option<DialogState>,
    thought_modal: Option<ThoughtModalState>,
    thoughts: Vec<ThinkingSegment>,
    tools: Vec<super::tool_activity::ToolActivity>,
    tool_viewer: Option<usize>,
    watermark_frame: u8,
    watermark_animating: bool,
    pub(crate) theme: crate::terminal_theme::TuiTheme,
}

#[derive(Debug)]
struct ThinkingSegment {
    id: Option<String>,
    text: String,
    elapsed: std::time::Duration,
    elapsed_known: bool,
    updated_at: Option<std::time::Instant>,
}

impl ThinkingSegment {
    fn elapsed_label(&self) -> String {
        if self.elapsed_known {
            super::format_thought_elapsed(self.elapsed())
        } else {
            "time unavailable".into()
        }
    }
    fn elapsed(&self) -> std::time::Duration {
        self.elapsed
            + self
                .updated_at
                .map(|time| time.elapsed())
                .unwrap_or_default()
    }
}

// Wrap from the tail so older paragraphs need not be laid out every frame.
fn thought_tail(text: &str, width: usize, height: usize) -> Vec<String> {
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

#[derive(Debug)]
struct CollapsedPaste {
    range: std::ops::Range<usize>,
    text: String,
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
    body_max_scroll: std::cell::Cell<usize>,
    body_page_size: std::cell::Cell<usize>,
    pub checked: Option<Vec<bool>>,
    pub input: Option<Vec<char>>,
    pub resume_picker: Option<ResumePickerDialog>,
    pub theme_picker: Option<usize>,
    pub update_prompt: bool,
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
            body_scroll: 0,
            body_max_scroll: std::cell::Cell::new(0),
            body_page_size: std::cell::Cell::new(0),
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
            update_prompt: false,
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
    pub fn tool_progress(&self, progress: &mint_core::AgentProgress) {
        if let Ok(mut state) = self.state.lock() {
            state.record_tool_progress(progress);
        }
    }

    pub fn reset_thoughts(&self) {
        if let Ok(mut state) = self.state.lock() {
            state.finish_live_thought();
            state.thought_modal = None;
        }
    }

    pub fn append_thought_delta(&self, id: &str, delta: &str, elapsed: std::time::Duration) {
        if delta.is_empty() {
            return;
        }
        if let Ok(mut state) = self.state.lock() {
            if let Some(thought) = state.thoughts.last_mut()
                && thought.id.as_deref() == Some(id)
                && thought.updated_at.is_some()
            {
                thought.text.push_str(delta);
                thought.elapsed = elapsed.max(thought.elapsed());
                thought.updated_at = Some(std::time::Instant::now());
            } else {
                state.finish_live_thought();
                state.thoughts.push(ThinkingSegment {
                    id: Some(id.to_owned()),
                    text: delta.to_owned(),
                    elapsed,
                    elapsed_known: true,
                    updated_at: Some(std::time::Instant::now()),
                });
            }
            state.refresh_thought_modal();
        }
    }

    pub fn finish_thought(&self, id: Option<&str>, text: &str, elapsed: std::time::Duration) {
        if let Ok(mut state) = self.state.lock() {
            if let Some(thought) = state.thoughts.last_mut()
                && thought.updated_at.is_some()
                && (id.is_none() || thought.id.as_deref() == id)
            {
                thought.text = text.to_owned();
                thought.elapsed = elapsed;
                thought.updated_at = None;
            } else if !text.trim().is_empty() {
                state.finish_live_thought();
                state.thoughts.push(ThinkingSegment {
                    id: id.map(str::to_owned),
                    text: text.to_owned(),
                    elapsed,
                    elapsed_known: true,
                    updated_at: None,
                });
            } else {
                state.finish_live_thought();
                return;
            }
            state.push_notice(format!(
                "Thought · {} (Ctrl+T to view)",
                super::format_thought_elapsed(elapsed)
            ));
            state.refresh_thought_modal();
        }
    }

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
            .and_then(|state| (!state.composer.is_empty()).then(|| state.composer_text()))
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
    fn record_tool_progress(&mut self, progress: &mint_core::AgentProgress) -> Option<usize> {
        use mint_core::AgentProgress;
        match progress {
            AgentProgress::SubagentStarted {
                subagent,
                profile,
                task,
            } => {
                if let Some(existing) = self.swarm_agents.iter_mut().find(|a| a.label == *subagent)
                {
                    existing.profile = profile.clone();
                    existing.task = task.clone();
                    existing.status = SwarmUiStatus::Running;
                    existing.started_at = std::time::Instant::now();
                } else {
                    self.swarm_agents.push(SwarmAgentUiState {
                        label: subagent.clone(),
                        profile: profile.clone(),
                        task: task.clone(),
                        current_action: None,
                        status: SwarmUiStatus::Running,
                        started_at: std::time::Instant::now(),
                        finished_elapsed_ms: None,
                        summary_preview: None,
                    });
                }
                return None;
            }
            AgentProgress::SubagentFinished {
                subagent,
                status,
                elapsed_ms,
                summary_preview,
                ..
            } => {
                let ui_status = if status == "failed" {
                    SwarmUiStatus::Failed
                } else {
                    SwarmUiStatus::Success
                };
                if let Some(existing) = self.swarm_agents.iter_mut().find(|a| a.label == *subagent)
                {
                    existing.status = ui_status;
                    existing.finished_elapsed_ms = Some(*elapsed_ms);
                    if !summary_preview.is_empty() {
                        existing.summary_preview = Some(summary_preview.clone());
                    }
                } else {
                    self.swarm_agents.push(SwarmAgentUiState {
                        label: subagent.clone(),
                        profile: String::new(),
                        task: String::new(),
                        current_action: None,
                        status: ui_status,
                        started_at: std::time::Instant::now(),
                        finished_elapsed_ms: Some(*elapsed_ms),
                        summary_preview: if summary_preview.is_empty() {
                            None
                        } else {
                            Some(summary_preview.clone())
                        },
                    });
                }
                return None;
            }
            AgentProgress::ToolStart {
                subagent: Some(agent_label),
                action,
                input,
                ..
            } => {
                if let Some(agent) = self
                    .swarm_agents
                    .iter_mut()
                    .find(|a| a.label == *agent_label)
                {
                    agent.current_action = Some(format_subagent_tool_action(action, input));
                }
                return None;
            }
            AgentProgress::ToolEnd {
                subagent: Some(_), ..
            } => {
                return None;
            }
            _ => {}
        }
        let (call_id, action, input, subagent, end) = match progress {
            AgentProgress::ToolStart {
                call_id,
                action,
                input,
                subagent,
            } => (call_id, action, input, subagent, None),
            AgentProgress::ToolEnd {
                call_id,
                action,
                input,
                subagent,
                result,
                status,
                elapsed_ms,
            } => (
                call_id,
                action,
                input,
                subagent,
                Some((result, status, elapsed_ms)),
            ),
            _ => return None,
        };
        let existing = self.tools.iter().position(|tool| {
            (end.is_some() || call_id.is_some())
                && tool.running()
                && tool.matches(call_id.as_deref(), action, input, subagent.as_deref())
        });
        let index = existing.unwrap_or_else(|| {
            let entry = self.transcript.len();
            self.transcript
                .push(TranscriptEntry::new(TranscriptRole::Notice, ""));
            self.tools.push(super::tool_activity::ToolActivity {
                entry,
                call_id: call_id.clone(),
                action: action.clone(),
                input: input.clone(),
                subagent: subagent.clone(),
                started: std::time::Instant::now(),
                elapsed: None,
                result: None,
                status: None,
            });
            self.tools.len() - 1
        });
        if let Some((result, status, elapsed_ms)) = end {
            self.tools[index].finish(result, status.clone());
            if let Some(elapsed_ms) = elapsed_ms {
                self.tools[index].elapsed = Some(std::time::Duration::from_millis(*elapsed_ms));
            }
        }
        self.refresh_tool_card(index);
        if self.tool_viewer == Some(index) {
            self.refresh_tool_viewer(false);
        }
        Some(index)
    }

    fn refresh_tool_card(&mut self, index: usize) {
        let tool = &self.tools[index];
        let lines = tool.lines(80);
        let plain: Vec<String> = lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|span| span.content.as_ref())
                    .collect()
            })
            .collect();
        if let Some(entry) = self.transcript.get_mut(tool.entry) {
            *entry = TranscriptEntry {
                role: TranscriptRole::Notice,
                text: plain.join("\n"),
                rendered_lines: lines,
                plain_lines: plain,
            };
        }
    }

    fn finish_running_tools(&mut self) {
        for index in 0..self.tools.len() {
            if self.tools[index].running() {
                self.tools[index].finish(
                    "Interrupted before a result was received.",
                    Some(mint_core::integrations::mcp_result::ToolStatus::Cancelled),
                );
                self.refresh_tool_card(index);
            }
        }
        if self.tool_viewer.is_some() {
            self.refresh_tool_viewer(false);
        }
    }

    fn refresh_tool_viewer(&mut self, reset_scroll: bool) {
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.title.starts_with("Tool details ·"))
        {
            self.tool_viewer = None;
            return;
        }
        let Some(index) = self.tool_viewer else {
            return;
        };
        let Some(tool) = self.tools.get(index) else {
            return;
        };
        let body = tool.details();
        if let Some(dialog) = self.dialog.as_mut() {
            dialog.title = format!("Tool details · {}/{}", index + 1, self.tools.len());
            dialog.body = body;
            if reset_scroll {
                dialog.body_scroll = 0;
            }
        } else {
            let (reply, _) = mpsc::channel();
            self.dialog = Some(DialogState::new_choice(
                format!("Tool details · {}/{}", index + 1, self.tools.len()),
                body,
                vec!["Close".into()],
                None,
                reply,
            ));
        }
    }

    fn handle_tool_key(&mut self, key: event::KeyEvent) -> bool {
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| !dialog.title.starts_with("Tool details ·"))
        {
            self.tool_viewer = None;
            return false;
        }
        use event::{KeyCode, KeyModifiers};
        if self.dialog.is_none() {
            self.tool_viewer = None;
        }
        let toggle =
            key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL);
        if let Some(index) = self.tool_viewer {
            if toggle || key.code == KeyCode::Esc {
                self.tool_viewer = None;
                self.dialog = None;
                return true;
            }
            if matches!(key.code, KeyCode::Tab | KeyCode::BackTab) {
                self.tool_viewer = Some(if key.code == KeyCode::Tab {
                    (index + 1) % self.tools.len()
                } else {
                    (index + self.tools.len() - 1) % self.tools.len()
                });
                self.refresh_tool_viewer(true);
                return true;
            }
        } else if toggle && self.dialog.is_none() && self.thought_modal.is_none() {
            if self.tools.is_empty() {
                self.set_notice("No tool activity recorded yet");
            } else {
                let target = self.preferred_tool_viewer_index();
                self.tool_viewer = Some(target);
                self.refresh_tool_viewer(true);
            }
            return true;
        }
        false
    }

    fn preferred_tool_viewer_index(&self) -> usize {
        let has_recent_sources = self.transcript.iter().rev().take(6).any(|entry| {
            entry.text.starts_with("Sources:\n") || entry.text.contains("Ctrl+O view web_search")
        });

        if has_recent_sources {
            if let Some(pos) = self
                .tools
                .iter()
                .rposition(|tool| tool.action == "web_search")
            {
                return pos;
            }
        }
        self.tools.len() - 1
    }

    fn live_thought(&self) -> Option<&ThinkingSegment> {
        self.thoughts
            .last()
            .filter(|thought| thought.updated_at.is_some() && !thought.text.trim().is_empty())
    }

    fn thought_total_elapsed_label(&self) -> String {
        if self.thoughts.iter().all(|thought| thought.elapsed_known) {
            super::format_thought_elapsed(self.thoughts.iter().map(ThinkingSegment::elapsed).sum())
        } else {
            "time unavailable".into()
        }
    }

    fn thought_record(&self) -> Option<ThoughtModalState> {
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

    fn refresh_thought_modal(&mut self) {
        if let Some(mut updated) = self.thought_record()
            && let Some(modal) = self.thought_modal.as_mut()
        {
            updated.scroll = modal.scroll;
            *modal = updated;
        }
    }

    fn finish_live_thought(&mut self) {
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

    fn handle_thought_key(&mut self, key: event::KeyEvent) -> bool {
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
            self.thought_modal = self.thought_record();
            if self.thought_modal.is_none() {
                self.set_notice("No thought process recorded for this turn");
            } else {
                self.selection_mode = false;
                self.clear_notice();
            }
            return true;
        }
        false
    }

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
        self.composer = normalize_prompt_newlines(&draft).chars().collect();
        self.collapsed_pastes.clear();
        self.expanded_paste = None;
        self.cursor = self.composer.len();
    }
    fn composer_text(&self) -> String {
        self.composer_text_range(0..self.composer.len())
    }
    fn composer_text_range(&self, range: std::ops::Range<usize>) -> String {
        let mut text = String::new();
        let mut start = range.start;
        for paste in self
            .collapsed_pastes
            .iter()
            .filter(|paste| paste.range.start >= range.start && paste.range.end <= range.end)
        {
            text.extend(self.composer[start..paste.range.start].iter());
            text.push_str(&paste.text);
            start = paste.range.end;
        }
        text.extend(self.composer[start..range.end].iter());
        text
    }
    fn take_prompt(&mut self) -> String {
        let text = self.composer_text();
        self.composer.clear();
        self.collapsed_pastes.clear();
        self.expanded_paste = None;
        self.cursor = 0;
        text
    }
    fn open_paste(&mut self) -> bool {
        let Some(index) = self
            .collapsed_pastes
            .iter()
            .enumerate()
            .min_by_key(|(_, paste)| {
                if self.cursor < paste.range.start {
                    paste.range.start - self.cursor
                } else {
                    self.cursor.saturating_sub(paste.range.end)
                }
            })
            .map(|(index, _)| index)
        else {
            return false;
        };
        self.expand_paste(index);
        true
    }
    fn expand_paste(&mut self, index: usize) {
        let paste = self.collapsed_pastes.remove(index);
        let start = paste.range.start;
        self.replace_composer(paste.range, &paste.text);
        self.expanded_paste = Some(start..self.cursor);
        self.cursor = start;
    }
    fn toggle_paste(&mut self) {
        if let Some(range) = self.expanded_paste.take() {
            let text = self.composer_text_range(range.clone());
            let start = range.start;
            let badge = format!("[Pasted Content {} chars]", text.chars().count());
            self.replace_composer(range, &badge);
            self.collapsed_pastes.push(CollapsedPaste {
                range: start..self.cursor,
                text,
            });
            self.collapsed_pastes.sort_by_key(|paste| paste.range.start);
        } else {
            self.open_paste();
        }
    }
    fn replace_composer(&mut self, mut range: std::ops::Range<usize>, text: &str) {
        // Badges are atomic. Deleting any part removes the attached paste;
        // inserting inside one opens it before editing the actual text.
        if range.is_empty() {
            if let Some(index) = self
                .collapsed_pastes
                .iter()
                .position(|paste| range.start > paste.range.start && range.start < paste.range.end)
            {
                self.expand_paste(index);
                range = self.cursor..self.cursor;
            }
        } else {
            for paste in &self.collapsed_pastes {
                if range.start < paste.range.end && range.end > paste.range.start {
                    range.start = range.start.min(paste.range.start);
                    range.end = range.end.max(paste.range.end);
                }
            }
        }
        let start = range.start;
        let removed = range.end - start;
        let added = text.chars().count();
        self.collapsed_pastes.retain_mut(|paste| {
            if start < paste.range.end && range.end > paste.range.start {
                return false;
            }
            if paste.range.start >= range.end {
                paste.range.start = paste.range.start - removed + added;
                paste.range.end = paste.range.end - removed + added;
            }
            true
        });
        // Keep the open block's bounds attached to edits, including insertions
        // at either edge. Fully deleting it must also remove the F2 target.
        if let Some(open) = self.expanded_paste.as_mut() {
            if !range.is_empty() && start <= open.start && range.end >= open.end {
                self.expanded_paste = None;
            } else {
                if open.start > range.end || (!range.is_empty() && open.start == range.end) {
                    open.start = open.start - removed + added;
                } else if open.start >= start {
                    open.start = start;
                }
                if open.end >= range.end {
                    open.end = open.end - removed + added;
                } else if open.end >= start {
                    open.end = start + added;
                }
            }
        }
        self.composer.splice(range, text.chars());
        self.cursor = start + added;
        self.slash_selected = 0;
    }
    fn edit_composer_key(&mut self, key: event::KeyEvent) -> bool {
        use event::{KeyCode, KeyModifiers};
        match key.code {
            KeyCode::F(2) => {
                self.toggle_paste();
            }
            KeyCode::Left => {
                self.cursor = self
                    .collapsed_pastes
                    .iter()
                    .find(|paste| self.cursor > paste.range.start && self.cursor <= paste.range.end)
                    .map_or(self.cursor.saturating_sub(1), |paste| paste.range.start);
            }
            KeyCode::Right => {
                self.cursor = self
                    .collapsed_pastes
                    .iter()
                    .find(|paste| self.cursor >= paste.range.start && self.cursor < paste.range.end)
                    .map_or((self.cursor + 1).min(self.composer.len()), |paste| {
                        paste.range.end
                    });
            }
            KeyCode::Backspace if self.cursor > 0 => {
                self.replace_composer(self.cursor - 1..self.cursor, "");
            }
            KeyCode::Delete if self.cursor < self.composer.len() => {
                self.replace_composer(self.cursor..self.cursor + 1, "");
            }
            KeyCode::Enter if key.modifiers.contains(KeyModifiers::ALT) => {
                self.replace_composer(self.cursor..self.cursor, "\n");
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if self
                    .active_notice()
                    .is_some_and(|notice| !notice.starts_with("Terminal too small"))
                {
                    self.clear_notice();
                }
                self.replace_composer(self.cursor..self.cursor, &character.to_string());
            }
            _ => return false,
        }
        true
    }
    fn insert_paste(&mut self, text: &str) {
        let text = normalize_prompt_newlines(text);
        if let Some(index) = self
            .collapsed_pastes
            .iter()
            .position(|paste| self.cursor > paste.range.start && self.cursor < paste.range.end)
        {
            self.expand_paste(index);
        }
        let cursor = self.cursor.min(self.composer.len());
        if text.lines().count() > 6 {
            let badge = format!("[Pasted Content {} chars]", text.chars().count());
            self.replace_composer(cursor..cursor, &badge);
            self.collapsed_pastes.push(CollapsedPaste {
                range: cursor..self.cursor,
                text: text.clone(),
            });
            self.collapsed_pastes.sort_by_key(|paste| paste.range.start);
        } else {
            self.replace_composer(cursor..cursor, &text);
        }
        self.slash_selected = 0;
    }
    fn composer_width(&self, area: Rect) -> usize {
        let (main, _) = mod_layout(
            area,
            self.mods_open && self.dialog.is_none() && self.thought_modal.is_none(),
        );
        main.width.saturating_sub(4).max(1) as usize
    }
    fn navigate_composer(&mut self, key: event::KeyEvent, row_width: usize) -> bool {
        use event::{KeyCode, KeyModifiers};
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return false;
        }
        // Honor the transcript's visible End shortcut before draft navigation.
        if key.code == KeyCode::End && self.scroll_from_bottom > 0 {
            self.scroll_from_bottom = 0;
            return true;
        }
        if self.composer.is_empty() {
            return false;
        }
        let (rows, row, col) = wrap_input_visual_into_rows(&self.composer, row_width, self.cursor);
        let target_row = match key.code {
            KeyCode::Up if rows.len() > 1 => row.saturating_sub(1),
            KeyCode::Down if rows.len() > 1 => (row + 1).min(rows.len() - 1),
            KeyCode::Home | KeyCode::End => row,
            _ => return false,
        };
        let start = input_row_start(&self.composer, &rows, target_row);
        let target: Vec<char> = rows[target_row].chars().collect();
        self.cursor = match key.code {
            KeyCode::Home => start,
            KeyCode::End => start + target.len(),
            _ => {
                let column = UnicodeWidthStr::width(
                    rows[row].chars().take(col).collect::<String>().as_str(),
                );
                let mut width = 0;
                let mut count = 0;
                for character in target {
                    let next = UnicodeWidthStr::width(character.to_string().as_str());
                    if width + next > column {
                        break;
                    }
                    width += next;
                    count += 1;
                }
                start + count
            }
        };
        true
    }
    pub fn reload_transcript(&mut self, chat_id: &str, workspace: &Path) {
        if self.chat_id != chat_id || self.current_dir != workspace {
            self.stop_mods();
        }
        if let Ok(memory) = mint_core::MemoryStore::open_default() {
            self.reload_transcript_with(&memory, chat_id, workspace);
        } else {
            self.chat_id = chat_id.to_owned();
            self.transcript.clear();
            self.tools.clear();
            if self
                .dialog
                .as_ref()
                .is_some_and(|dialog| dialog.title.starts_with("Tool details ·"))
            {
                self.dialog = None;
            }
            self.thoughts.clear();
            self.thought_modal = None;
            self.tool_viewer = None;
            self.last_interaction_id = 0;
            self.sync_cursor = 0;
        }
    }

    fn push_saved_activity(
        &mut self,
        activity: Option<&serde_json::Value>,
        turn_status: &str,
    ) -> Vec<(String, String)> {
        let mut sources = Vec::new();
        let Some(events) = activity.and_then(serde_json::Value::as_array) else {
            return sources;
        };
        let first_tool = self.tools.len();
        for value in events {
            let Ok(event) = serde_json::from_value::<mint_core::AgentProgress>(value.clone())
            else {
                continue;
            };
            // In-flight external turns use the shared-sync placeholder until a terminal snapshot arrives.
            if !matches!(turn_status, "running" | "queued") {
                match &event {
                    mint_core::AgentProgress::ToolStart { .. } => {
                        self.record_tool_progress(&event);
                    }
                    mint_core::AgentProgress::ToolEnd { action, result, .. } => {
                        if let Some(index) = self.record_tool_progress(&event) {
                            self.tools[index].elapsed = value["data"]["elapsed_ms"]
                                .as_u64()
                                .or_else(|| value["data"]["elapsedMs"].as_u64())
                                .map(std::time::Duration::from_millis);
                            self.refresh_tool_card(index);
                        }
                        if action == "web_search"
                            && !result.starts_with("Web search error:")
                            && result != "No web search results found."
                        {
                            sources.extend(crate::agent::parse_web_search_sources(result));
                        }
                    }
                    mint_core::AgentProgress::SubagentStarted { .. }
                    | mint_core::AgentProgress::SubagentFinished { .. } => {
                        self.record_tool_progress(&event);
                    }
                    mint_core::AgentProgress::ExtendedThinking {
                        id,
                        thought,
                        elapsed_ms,
                    } if !thought.trim().is_empty() => {
                        self.thoughts.push(ThinkingSegment {
                            id: id.clone(),
                            text: thought.clone(),
                            elapsed: std::time::Duration::from_millis(elapsed_ms.unwrap_or(0)),
                            elapsed_known: elapsed_ms.is_some(),
                            updated_at: None,
                        });
                        let elapsed = self.thoughts.last().unwrap().elapsed_label();
                        self.push_notice(format!("Thought · {elapsed} (Ctrl+T to view)"));
                    }
                    mint_core::AgentProgress::Thought { thought } if !thought.trim().is_empty() => {
                        if mint_core::orchestration::is_internal_cot(thought) {
                            self.thoughts.push(ThinkingSegment {
                                id: None,
                                text: thought.clone(),
                                elapsed: std::time::Duration::ZERO,
                                elapsed_known: false,
                                updated_at: None,
                            });
                            self.push_notice("Thought · time unavailable (Ctrl+T to view)");
                        } else {
                            self.push_notice(thought.clone());
                        }
                    }
                    mint_core::AgentProgress::RunCompleted { summary } => {
                        self.push_notice(crate::agent::run_summary_lines(summary).join("\n"));
                    }
                    _ => {}
                }
            }
            // Suppress redundant ExecutionUpdate notices in TUI (handled by ToolActivity cards)
            if !matches!(event, mint_core::AgentProgress::ExecutionUpdate { .. }) {
                if let Some(notes) = crate::agent::mcp_progress_notes(&event) {
                    for note in notes {
                        self.push_notice(note);
                    }
                }
            }
        }
        for index in first_tool..self.tools.len() {
            if self.tools[index].running() {
                self.tools[index].finish(
                    "Interrupted before a result was recorded.",
                    Some(mint_core::integrations::mcp_result::ToolStatus::Cancelled),
                );
                self.tools[index].elapsed = None;
                self.refresh_tool_card(index);
            }
        }
        sources
    }

    pub fn reload_transcript_with(
        &mut self,
        memory: &mint_core::MemoryStore,
        chat_id: &str,
        workspace: &Path,
    ) {
        if self.chat_id != chat_id || self.current_dir != workspace {
            self.stop_mods();
        }
        self.chat_id = chat_id.to_owned();
        self.transcript.clear();
        self.tools.clear();
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.title.starts_with("Tool details ·"))
        {
            self.dialog = None;
        }
        self.thoughts.clear();
        self.thought_modal = None;
        self.tool_viewer = None;
        let scoped = mint_core::scoped_chat_id(chat_id, Some(&workspace.to_string_lossy()));
        self.sync_cursor = memory.latest_conversation_sequence(&scoped).unwrap_or(0);
        if let Ok(rows) = memory.interactions_for_chat(&scoped) {
            self.last_interaction_id = rows.last().map(|row| row.id).unwrap_or(0);
            for row in rows {
                self.transcript
                    .push(TranscriptEntry::new(TranscriptRole::User, row.user_text));
                let sources = self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
                match row.status.as_str() {
                    "completed" => {
                        self.transcript
                            .push(TranscriptEntry::new(TranscriptRole::Assistant, row.ai_text));
                        if !sources.is_empty() {
                            let text = crate::agent::format_tui_web_sources(
                                &sources,
                                crate::agent::MAX_TUI_WEB_SOURCES,
                            );
                            if !text.is_empty() {
                                self.transcript
                                    .push(TranscriptEntry::new(TranscriptRole::Notice, text));
                            }
                        }
                    }
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
                let sources = self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
                match row.status.as_str() {
                    "completed" => {
                        self.transcript.push(TranscriptEntry::new(
                            TranscriptRole::Assistant,
                            &row.ai_text,
                        ));
                        if !sources.is_empty() {
                            let text = crate::agent::format_tui_web_sources(
                                &sources,
                                crate::agent::MAX_TUI_WEB_SOURCES,
                            );
                            if !text.is_empty() {
                                self.transcript
                                    .push(TranscriptEntry::new(TranscriptRole::Notice, text));
                            }
                        }
                    }
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
                    let mut append = false;
                    if let Some(last) = self.transcript.last() {
                        if last.role == TranscriptRole::Notice
                            && (last.text.starts_with("Mint is responding")
                                || last.text.starts_with("Queued for this session"))
                        {
                            self.transcript.pop();
                            append = true;
                        } else if last.role != TranscriptRole::Assistant {
                            append = true;
                        }
                    }
                    if append {
                        let sources =
                            self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
                        self.transcript.push(TranscriptEntry::new(
                            TranscriptRole::Assistant,
                            &row.ai_text,
                        ));
                        if !sources.is_empty() {
                            let text = crate::agent::format_tui_web_sources(
                                &sources,
                                crate::agent::MAX_TUI_WEB_SOURCES,
                            );
                            if !text.is_empty() {
                                self.transcript
                                    .push(TranscriptEntry::new(TranscriptRole::Notice, text));
                            }
                        }
                        changed = true;
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
                            self.push_saved_activity(row.agent_activity.as_ref(), &row.status);
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
        self.swarm_agents.clear();
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
        self.tools.clear();
        self.swarm_agents.clear();
        if self
            .dialog
            .as_ref()
            .is_some_and(|dialog| dialog.title.starts_with("Tool details ·"))
        {
            self.dialog = None;
        }
        self.thoughts.clear();
        self.thought_modal = None;
        self.tool_viewer = None;
        self.scroll_from_bottom = 0;
        self.selection_mode = false;
    }
    pub fn sync_session(&mut self, session: &InteractiveSession) {
        if self.current_dir != session.current_dir {
            self.stop_mods();
        }
        self.model = active_model(&session.config.ai_provider, &session.config).to_owned();
        self.thinking_status = format_thinking_status(&session.config);
        self.provider = format_provider_display_name(&session.config.ai_provider, &session.config);
        self.workspace = format_workspace_with_branch(&session.current_dir);
        self.current_dir = session.current_dir.clone();
        self.plan_mode = session.plan_mode;
        self.history = session.history.clone();
    }
    fn stop_mods(&mut self) {
        if !self.chat_id.is_empty() {
            let chat =
                mint_core::scoped_chat_id(&self.chat_id, Some(&self.current_dir.to_string_lossy()));
            mint_core::mods::close_chat(&self.current_dir, &chat);
        }
        self.mods.clear();
        self.mods_open = false;
    }
    fn mod_input_value<'a>(&'a self, control: &'a crate::mods_ui::Control) -> &'a str {
        self.mod_input
            .as_ref()
            .filter(|(initial, _)| initial == &control.value)
            .map_or(&control.value, |(_, draft)| draft)
    }
    fn mod_input_draft(&mut self, control: &crate::mods_ui::Control) -> &mut String {
        if self
            .mod_input
            .as_ref()
            .is_none_or(|(initial, _)| initial != &control.value)
        {
            self.mod_input = Some((control.value.clone(), control.value.clone()));
        }
        &mut self.mod_input.as_mut().unwrap().1
    }
    fn history_previous(&mut self) {
        if self.history.is_empty() {
            return;
        }
        if self.history_index.is_none() {
            self.draft_before_history = self.composer_text().chars().collect();
        }
        let index = self
            .history_index
            .map(|index| index.saturating_sub(1))
            .unwrap_or(self.history.len() - 1);
        self.history_index = Some(index);
        self.set_draft(self.history[index].clone());
    }
    fn history_next(&mut self) {
        let Some(index) = self.history_index else {
            return;
        };
        if index + 1 < self.history.len() {
            let next = index + 1;
            self.history_index = Some(next);
            self.set_draft(self.history[next].clone());
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
            let prefix = before.to_lowercase();
            let query = &prefix[1..];
            let mut matches: Vec<_> = super::SLASH_COMMANDS
                .iter()
                .filter(|command| {
                    command.on_cli()
                        && command.token.starts_with('/')
                        && (command.token.to_lowercase().starts_with(&prefix)
                            || command.description.to_lowercase().contains(query))
                })
                .collect();
            matches.sort_by_key(|command| !command.token.to_lowercase().starts_with(&prefix));
            return matches
                .into_iter()
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
        let word_end = self.composer[cursor..]
            .iter()
            .position(|c| c.is_whitespace())
            .map_or(self.composer.len(), |offset| cursor + offset);
        let paste_start = self
            .collapsed_pastes
            .iter()
            .find(|paste| paste.range.start >= cursor)
            .map(|paste| paste.range.start);
        let end = paste_start.map_or(word_end, |start| word_end.min(start));
        let needs_space = end == self.composer.len() || paste_start == Some(end);
        let replacement = if needs_space {
            format!("{token} ")
        } else {
            token.clone()
        };
        self.replace_composer(start..end, &replacement);
        if !needs_space {
            self.cursor += 1;
        }
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
    fn transcript_text(&self, width: u16) -> Text<'static> {
        let mut lines = Vec::new();
        let selected_start = self.selection_anchor.min(self.selection_head);
        let selected_end = self.selection_anchor.max(self.selection_head);
        let mut line_index = 0usize;
        for (entry_index, entry) in self.transcript.iter().enumerate() {
            let role_header = match entry.role {
                TranscriptRole::User => None,
                TranscriptRole::Assistant => Some(("Mint", crate::terminal_theme::ACCENT)),
                TranscriptRole::Notice => None,
                TranscriptRole::Command => Some(("Command", crate::terminal_theme::ACCENT)),
                TranscriptRole::System => Some(("System", crate::terminal_theme::WARNING)),
            };
            if let Some((label, color)) = role_header {
                let selected =
                    self.selection_mode && (selected_start..=selected_end).contains(&line_index);
                let header = Line::from(Span::styled(
                    format!("{label} ›"),
                    Style::default()
                        .fg(color)
                        .add_modifier(Modifier::BOLD)
                        .add_modifier(if selected {
                            Modifier::REVERSED
                        } else {
                            Modifier::empty()
                        }),
                ));
                lines.push(header);
                line_index += 1;
            }
            let tool_lines = self
                .tools
                .iter()
                .find(|tool| tool.entry == entry_index)
                .map(|tool| tool.lines(width));
            let empty_user_line = [Line::default()];
            let content_lines = if let Some(lines) = tool_lines.as_ref() {
                lines.as_slice()
            } else if entry.role == TranscriptRole::User && entry.rendered_lines.is_empty() {
                &empty_user_line[..]
            } else {
                &entry.rendered_lines
            };
            for (content_index, content_line) in content_lines.iter().enumerate() {
                let selected =
                    self.selection_mode && (selected_start..=selected_end).contains(&line_index);
                let mut sel_line = content_line.clone();
                if entry.role == TranscriptRole::User && content_index == 0 {
                    sel_line.spans.insert(
                        0,
                        Span::styled(
                            "You › ",
                            Style::default()
                                .fg(crate::terminal_theme::BLUE)
                                .add_modifier(Modifier::BOLD),
                        ),
                    );
                }
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
                if entry.role == TranscriptRole::User {
                    lines.extend(highlighted_user_content(sel_line, width));
                } else {
                    lines.push(sel_line);
                }
                line_index += 1;
            }
            let selected =
                self.selection_mode && (selected_start..=selected_end).contains(&line_index);
            let mut style = Style::default();
            if selected {
                style = style.add_modifier(Modifier::REVERSED);
            }
            lines.push(Line::styled("", style));
            line_index += 1;
        }
        Text::from(lines)
    }
    fn plain_transcript_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        for entry in &self.transcript {
            if entry.role == TranscriptRole::User {
                let mut content = entry.plain_lines.iter();
                lines.push(format!(
                    "You › {}",
                    content.next().map_or("", String::as_str)
                ));
                lines.extend(content.cloned());
                lines.push(String::new());
                continue;
            }
            if let Some(header) = match entry.role {
                TranscriptRole::User => None,
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
    fn mod_viewport(&self, area: Rect) -> mint_core::mods::Viewport {
        let (body, _) = mod_layout(
            area,
            self.mods_open && self.dialog.is_none() && self.thought_modal.is_none(),
        );
        mint_core::mods::Viewport {
            surface: Some("terminal".into()),
            columns: (body.width > 0).then_some(body.width),
            rows: (body.height > 0).then_some(body.height),
            side_panel_open: self.mods_open,
            ..Default::default()
        }
    }
    fn render(&self, frame: &mut ratatui::Frame<'_>) -> RenderAreas {
        let suggestions = self.suggestions();
        let suggestion_height = if suggestions.is_empty() {
            0
        } else {
            (suggestions.len().min(5) as u16) + 1
        };
        let tick = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as usize
            / 80;
        let mut status_lines = self.status.clone();
        // Keep active commands last: the bounded status viewport retains its tail.
        status_lines.extend(super::tool_activity::running_lines(
            &self.tools,
            frame.area().width.saturating_sub(4) as usize,
            tick,
        ));
        let status_height = (status_lines.len() as u16).min(8);
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
                    if remaining.starts_with("[Image")
                        || remaining.starts_with("[Pasted text")
                        || remaining.starts_with("[Pasted Content")
                    {
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

        let (main_area, mod_side_area) = mod_layout(
            frame.area(),
            self.mods_open && self.dialog.is_none() && self.thought_modal.is_none(),
        );
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
                total.min(max_allowed as usize) as u16
            }
        } else {
            0
        };

        let thought_height = if self.thought_modal.is_some() {
            let max_allowed = main_area.height.saturating_sub(9).min(20);
            ((main_area.height * 4) / 10).max(8).min(max_allowed)
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
        let swarm_height = if self.swarm_agents.is_empty()
            || self.dialog.is_some()
            || self.thought_modal.is_some()
        {
            0
        } else {
            let max_avail = main_area
                .height
                .saturating_sub(6 + 3 + status_height + suggestion_height + composer_height + 1);
            if max_avail == 0 {
                0
            } else if main_area.height < 24 || max_avail < 3 {
                1
            } else {
                let needed = (self.swarm_agents.len().min(5) as u16) + 2;
                needed.min(max_avail)
            }
        };

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
                    Constraint::Length(swarm_height),
                    Constraint::Length(1),
                ])
                .split(main_area)
        };

        let mut rows = rows.to_vec();
        let live_height = if self.dialog.is_none()
            && self.thought_modal.is_none()
            && self.live_thought().is_some()
        {
            6.min(rows[1].height.saturating_sub(3))
        } else {
            0
        };
        let live_height = if live_height >= 3 { live_height } else { 0 };
        rows[1].height = rows[1].height.saturating_sub(live_height);
        let live_thought_area = Rect::new(rows[1].x, rows[1].bottom(), rows[1].width, live_height);
        let band_lines = crate::mods_ui::slot_lines(&self.mods, "above_composer");
        let band_height = (band_lines.len() as u16)
            .min(6)
            .min(rows[1].height.saturating_sub(2));
        rows[1].height = rows[1].height.saturating_sub(band_height);
        let mod_band_area = Rect::new(rows[1].x, rows[1].bottom(), rows[1].width, band_height);
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
            Paragraph::new("Type naturally or /help. F7: Mods. F6: Classic CLI. Ctrl+D exits."),
            help_area,
        );

        let transcript = TranscriptLayout::new(self.transcript_text(rows[1].width), rows[1].width);
        let total = transcript.height;
        let max_scroll = total.saturating_sub(rows[1].height as usize);
        let scroll = max_scroll.saturating_sub(self.scroll_from_bottom.min(max_scroll));
        frame.render_widget(transcript.paragraph_at(scroll), rows[1]);
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
        if live_thought_area.height > 0
            && let Some(thought) = self.live_thought()
        {
            let rows = thought_tail(
                &thought.text,
                live_thought_area.width.saturating_sub(2) as usize,
                live_thought_area.height.saturating_sub(2) as usize,
            );
            let block = Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(crate::terminal_theme::MUTED))
                .title(format!(
                    " Thinking · {} (Ctrl+T to view) ",
                    super::format_thought_elapsed(thought.elapsed())
                ));
            frame.render_widget(
                Paragraph::new(rows.into_iter().map(Line::raw).collect::<Vec<_>>())
                    .style(Style::default().fg(crate::terminal_theme::MUTED))
                    .block(block),
                live_thought_area,
            );
        }
        if status_height > 0 {
            // Derive a tick counter from wall-clock time (ms / 80) so the
            // shimmer animation advances at ~12 fps regardless of the 30 ms
            // render loop frequency.
            let tick = std::time::SystemTime::now()
                .duration_since(std::time::SystemTime::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as usize
                / 80;

            let status_lines: Vec<Line> = status_lines
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
                let monochrome_picker = dialog.title == "Resume Session" || dialog.update_prompt;

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

                let width = rows[3].width.max(1);
                let title_height = Paragraph::new(lines.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(width);
                let command_line_count = if dialog.title == "Local shell command"
                    && dialog.body.starts_with("Command: ")
                {
                    dialog
                        .body
                        .rfind("\nMode: ")
                        .map_or(0, |end| dialog.body[..end].lines().count())
                } else {
                    0
                };
                let detail_lines = self
                    .tool_viewer
                    .and_then(|index| self.tools.get(index))
                    .map(|tool| {
                        tool.detail_lines()
                            .into_iter()
                            .map(|mut line| {
                                line.spans.insert(0, Span::raw(" "));
                                line
                            })
                            .collect::<Vec<_>>()
                    });
                let body = Paragraph::new(detail_lines.unwrap_or_else(|| {
                    dialog
                        .body
                        .lines()
                        .enumerate()
                        .map(|(index, line)| {
                            if index < command_line_count {
                                let (label, command) = if index == 0 {
                                    (" Command: ", &line["Command: ".len()..])
                                } else {
                                    (" ", line)
                                };
                                return Line::from(vec![
                                    Span::styled(
                                        label,
                                        Style::default().fg(crate::terminal_theme::TEXT),
                                    ),
                                    Span::styled(
                                        command,
                                        Style::default().fg(crate::terminal_theme::ACCENT),
                                    ),
                                ]);
                            }
                            Line::styled(
                                format!(" {line}"),
                                Style::default().fg(crate::terminal_theme::TEXT),
                            )
                        })
                        .collect::<Vec<_>>()
                }))
                .wrap(Wrap { trim: false });
                let body_lines = if dialog.body.is_empty() {
                    0
                } else {
                    body.line_count(width)
                };
                let hint = if self.tool_viewer.is_some() {
                    " Shift+Tab previous · Tab next (wraps)\n PgUp/PgDn scroll · Ctrl+Y copy all · Esc/Ctrl+O close"
                } else if dialog.update_prompt {
                    " ↑/↓ navigate · 1-3 choose · Enter continue · Esc skip"
                } else if dialog.input.is_some() {
                    " Enter submit · Esc cancel"
                } else if dialog.checked.is_some() {
                    " Space toggle · Enter confirm · 1-9 pick · Type to filter · Esc cancel"
                } else {
                    " ↑/↓ navigate · 1-9 pick · Type to filter · Enter select · Esc cancel"
                };
                let normal_footer_height = (if dialog.input.is_some() { 1 } else { 2 })
                    + Paragraph::new(hint)
                        .wrap(Wrap { trim: false })
                        .line_count(width);
                let filter_height = if dialog.filter.is_empty() {
                    0
                } else {
                    Paragraph::new(format!(
                        " Filter: {} ({total_options}/{})",
                        dialog.filter.iter().collect::<String>(),
                        dialog.options.len()
                    ))
                    .wrap(Wrap { trim: false })
                    .line_count(width)
                };
                let option_height = if dialog.input.is_some() {
                    1
                } else {
                    total_options.clamp(1, 8)
                };
                let inner_height = rows[3].height.saturating_sub(1) as usize;
                let fixed_height = title_height + filter_height + 1 + option_height;
                let body_overflows =
                    body_lines > inner_height.saturating_sub(fixed_height + normal_footer_height);
                let hint = if body_overflows && dialog.input.is_none() && self.tool_viewer.is_none()
                {
                    if dialog.checked.is_some() {
                        " Space toggle · Enter confirm · Esc cancel"
                    } else {
                        " ↑/↓ choose · Enter select · Esc cancel"
                    }
                } else {
                    hint
                };
                let footer_height = if body_overflows && dialog.input.is_none() {
                    Paragraph::new(hint)
                        .wrap(Wrap { trim: false })
                        .line_count(width)
                } else {
                    normal_footer_height
                };
                let body_height = body_lines
                    .min(inner_height.saturating_sub(
                        fixed_height + footer_height + usize::from(body_overflows),
                    ));
                let max_body_scroll = body_lines.saturating_sub(body_height);
                dialog.body_max_scroll.set(max_body_scroll);
                dialog.body_page_size.set(body_height);
                let body_scroll = dialog.body_scroll.min(max_body_scroll);
                // Reserve visual rows in the controls paragraph; render the body
                // independently so its scroll cannot move the approval choices.
                lines.extend((0..body_height).map(|_| Line::raw("")));
                if body_overflows {
                    lines.push(Line::styled(
                        format!(
                            " {}/{} · PgUp/PgDn · Home/End · Wheel",
                            body_scroll + 1,
                            body_lines
                        ),
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ));
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

                    if !body_overflows {
                        lines.push(Line::raw(""));
                    }
                    lines.extend(hint.lines().map(|line| {
                        Line::styled(line, Style::default().fg(crate::terminal_theme::MUTED))
                    }));
                    if !body_overflows {
                        lines.push(Line::raw(""));
                    }
                }

                frame.render_widget(
                    Paragraph::new(lines).wrap(Wrap { trim: false }).block(
                        Block::default()
                            .borders(Borders::TOP)
                            .border_style(Style::default().fg(crate::terminal_theme::MUTED)),
                    ),
                    rows[3],
                );

                frame.render_widget(
                    body.scroll((body_scroll.min(u16::MAX as usize) as u16, 0)),
                    Rect::new(
                        rows[3].x,
                        rows[3].y + 1 + title_height as u16,
                        rows[3].width,
                        body_height as u16,
                    ),
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
            let mut wrapped_lines: Vec<(String, bool)> = Vec::new();
            for paragraph in &modal.lines {
                let p_trimmed = paragraph.trim_end();
                let separator = p_trimmed.starts_with("── Thought ")
                    && p_trimmed.ends_with(" ──────────────────────────");
                if p_trimmed.is_empty() {
                    wrapped_lines.push((String::new(), false));
                } else {
                    let options = textwrap::Options::new(content_width).break_words(true);
                    let filled = textwrap::fill(p_trimmed, &options);
                    for l in filled.lines() {
                        wrapped_lines.push((l.to_string(), separator));
                    }
                }
            }

            let inner_height = (panel_area.height as usize).saturating_sub(2);
            let max_scroll = wrapped_lines.len().saturating_sub(inner_height);
            let scroll = modal.scroll.min(max_scroll);

            let key_style = Style::default().fg(crate::terminal_theme::BLUE);
            let mut hint_spans = Vec::new();
            if max_scroll > 0 {
                hint_spans.push(Span::styled(" ↑/↓", key_style));
                hint_spans.push(Span::raw(format!(
                    " scroll {}/{} ·",
                    scroll + 1,
                    max_scroll + 1
                )));
            }
            hint_spans.extend([
                Span::styled(" Esc", key_style),
                Span::raw(" / "),
                Span::styled("Ctrl+T", key_style),
                Span::raw(" to close "),
            ]);
            let scroll_hint =
                Line::from(hint_spans).style(Style::default().fg(crate::terminal_theme::MUTED));

            let block = Block::default()
                .borders(Borders::TOP)
                .border_style(Style::default().fg(crate::terminal_theme::BLUE))
                .title(Span::styled(
                    format!(
                        " Thought Process · {} ",
                        if self.thoughts.is_empty() {
                            modal.elapsed_str.clone()
                        } else {
                            self.thought_total_elapsed_label()
                        }
                    ),
                    Style::default()
                        .fg(crate::terminal_theme::BLUE)
                        .add_modifier(Modifier::BOLD),
                ))
                .title_bottom(scroll_hint);

            let visible = &wrapped_lines[scroll..(scroll + inner_height).min(wrapped_lines.len())];
            let mut p_lines: Vec<Line> = Vec::with_capacity(inner_height);
            for (l, separator) in visible {
                p_lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        l.clone(),
                        Style::default().fg(if *separator {
                            crate::terminal_theme::ACCENT
                        } else {
                            crate::terminal_theme::TEXT
                        }),
                    ),
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
            let mut input_block = Block::default()
                .borders(Borders::TOP | Borders::BOTTOM)
                .border_style(Style::default().fg(crate::terminal_theme::MUTED));
            let bg_running = mint_core::bg_shell::running_count_for(&self.current_dir);
            if bg_running > 0 {
                input_block = input_block.title_top(
                    Line::styled(
                        format!(
                            " {bg_running} Background Terminal{} running ",
                            if bg_running == 1 { "" } else { "s" }
                        ),
                        Style::default().fg(crate::terminal_theme::BLUE),
                    )
                    .alignment(ratatui::layout::Alignment::Right),
                );
            }
            if composer_rows.len() > visible_composer_rows {
                input_block = input_block.title_bottom(format!(
                    " {}–{} / {} lines · ↑/↓ edit ",
                    composer_scroll + 1,
                    (composer_scroll + visible_composer_rows).min(composer_rows.len()),
                    composer_rows.len(),
                ));
            }
            if !self.collapsed_pastes.is_empty() || self.expanded_paste.is_some() {
                input_block = input_block.title_bottom(" F2: open/collapse paste ");
            }
            // Rows already wrap at the content width. A second word-wrap
            // pass would change blank-row and cursor/scroll positions.
            frame.render_widget(
                Paragraph::new(input)
                    .scroll((composer_scroll as u16, 0))
                    .block(input_block),
                rows[4],
            );

            if rows[5].height > 0 {
                if rows[5].height == 1 {
                    let running_count = self
                        .swarm_agents
                        .iter()
                        .filter(|a| a.status == SwarmUiStatus::Running)
                        .count();
                    const SPINNER_CHARS: &[char] =
                        &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let spinner = SPINNER_CHARS[tick % SPINNER_CHARS.len()];
                    let line = if running_count > 0 {
                        Line::from(vec![
                            Span::styled(
                                format!(" {spinner} "),
                                Style::default()
                                    .fg(crate::terminal_theme::BLUE)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!(
                                    "Running {running_count} subagent{} ({}/{} completed)",
                                    if running_count == 1 { "" } else { "s" },
                                    self.swarm_agents.len() - running_count,
                                    self.swarm_agents.len(),
                                ),
                                Style::default().fg(crate::terminal_theme::TEXT),
                            ),
                        ])
                    } else {
                        Line::from(vec![
                            Span::styled(
                                " ✓ ",
                                Style::default()
                                    .fg(crate::terminal_theme::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::styled(
                                format!("{} subagents finished", self.swarm_agents.len()),
                                Style::default().fg(crate::terminal_theme::MUTED),
                            ),
                        ])
                    };
                    frame.render_widget(Paragraph::new(line), rows[5]);
                } else {
                    let any_running = self
                        .swarm_agents
                        .iter()
                        .any(|a| a.status == SwarmUiStatus::Running);
                    let block = Block::default()
                        .borders(Borders::ALL)
                        .border_style(Style::default().fg(crate::terminal_theme::MUTED))
                        .title(Span::styled(
                            if any_running {
                                " Running Subagents "
                            } else {
                                " Subagents "
                            },
                            Style::default()
                                .fg(if any_running {
                                    crate::terminal_theme::BLUE
                                } else {
                                    crate::terminal_theme::TEXT
                                })
                                .add_modifier(Modifier::BOLD),
                        ));

                    const SPINNER_CHARS: &[char] =
                        &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
                    let spinner = SPINNER_CHARS[tick % SPINNER_CHARS.len()];
                    let inner_width = (rows[5].width as usize).saturating_sub(2);
                    let max_items = (rows[5].height as usize).saturating_sub(2);
                    let total_agents = self.swarm_agents.len();
                    let has_overflow = total_agents > max_items;
                    let take_count = if has_overflow {
                        max_items.saturating_sub(1)
                    } else {
                        max_items.min(total_agents)
                    };

                    let mut lines = Vec::new();
                    for a in self.swarm_agents.iter().take(take_count) {
                        let (icon_span, icon_width) = match a.status {
                            SwarmUiStatus::Running => (
                                Span::styled(
                                    format!("  {spinner} "),
                                    Style::default()
                                        .fg(crate::terminal_theme::BLUE)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                4,
                            ),
                            SwarmUiStatus::Success => (
                                Span::styled(
                                    "  ✓ ",
                                    Style::default()
                                        .fg(crate::terminal_theme::ACCENT)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                4,
                            ),
                            SwarmUiStatus::Failed => (
                                Span::styled(
                                    "  ✗ ",
                                    Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
                                ),
                                4,
                            ),
                        };

                        let label_str = format!("[{}] ", a.label);
                        let label_width = UnicodeWidthStr::width(label_str.as_str());
                        let label_span = Span::styled(
                            label_str,
                            Style::default()
                                .fg(crate::terminal_theme::ACCENT)
                                .add_modifier(Modifier::BOLD),
                        );

                        let suffix_str = match a.status {
                            SwarmUiStatus::Running => {
                                let secs = a.started_at.elapsed().as_secs();
                                if secs < 60 {
                                    format!(" · {secs}s")
                                } else {
                                    format!(" · {}m {}s", secs / 60, secs % 60)
                                }
                            }
                            SwarmUiStatus::Success => {
                                let ms = a.finished_elapsed_ms.unwrap_or(0);
                                let secs = (ms / 1000).max(1);
                                if secs < 60 {
                                    format!(" · done {secs}s")
                                } else {
                                    format!(" · done {}m {}s", secs / 60, secs % 60)
                                }
                            }
                            SwarmUiStatus::Failed => {
                                let ms = a.finished_elapsed_ms.unwrap_or(0);
                                let secs = (ms / 1000).max(1);
                                if secs < 60 {
                                    format!(" · failed {secs}s")
                                } else {
                                    format!(" · failed {}m {}s", secs / 60, secs % 60)
                                }
                            }
                        };
                        let suffix_width = UnicodeWidthStr::width(suffix_str.as_str());

                        let desc = if a.status == SwarmUiStatus::Success
                            && a.summary_preview.as_deref().is_some_and(|p| !p.is_empty())
                        {
                            a.summary_preview.as_deref().unwrap()
                        } else if let Some(act) = a
                            .current_action
                            .as_deref()
                            .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("null"))
                        {
                            act
                        } else if !a.task.is_empty() && !a.task.eq_ignore_ascii_case("null") {
                            &a.task
                        } else {
                            "Working on task..."
                        };

                        let prefix_width = icon_width + label_width;
                        let avail_for_desc =
                            inner_width.saturating_sub(prefix_width + suffix_width + 1);
                        let clipped_desc = clip_text(desc, avail_for_desc);
                        let used_width = prefix_width
                            + UnicodeWidthStr::width(clipped_desc.as_str())
                            + suffix_width;
                        let gap = inner_width.saturating_sub(used_width);

                        lines.push(Line::from(vec![
                            icon_span,
                            label_span,
                            Span::styled(
                                clipped_desc,
                                Style::default().fg(crate::terminal_theme::TEXT),
                            ),
                            Span::raw(" ".repeat(gap)),
                            Span::styled(
                                suffix_str,
                                Style::default().fg(crate::terminal_theme::MUTED),
                            ),
                        ]));
                    }

                    if has_overflow {
                        let overflow_count = total_agents - take_count;
                        lines.push(Line::from(vec![
                            Span::raw("      "),
                            Span::styled(
                                format!("+{overflow_count} more subagents..."),
                                Style::default().fg(crate::terminal_theme::MUTED),
                            ),
                        ]));
                    }

                    frame.render_widget(Paragraph::new(lines).block(block), rows[5]);
                }
            }

            let mode_label = if self.plan_mode {
                " [Plan] "
            } else {
                " [Agent] "
            };
            let path_text = format!("path: {}", self.workspace);
            let thinking_text = format!("• {}", self.thinking_status);
            let left_len = mode_label.chars().count()
                + self.model.chars().count()
                + thinking_text.chars().count()
                + 1;
            let right_len = path_text.chars().count();
            let available = rows[6].width as usize;
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
                    Span::styled(path_text, Style::default().fg(crate::terminal_theme::MUTED)),
                ])
            };
            frame.render_widget(Paragraph::new(footer), rows[6]);

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
        if mod_band_area.height > 0 {
            frame.render_widget(
                Paragraph::new(band_lines).style(Style::default().fg(crate::terminal_theme::TEXT)),
                mod_band_area,
            );
        }
        if self.mods_open && self.dialog.is_none() && self.thought_modal.is_none() {
            let area = mod_side_area.unwrap_or(Rect::new(
                main_area.x,
                main_area.y + 6,
                main_area.width,
                main_area
                    .height
                    .saturating_sub(6 + composer_height + swarm_height + 1),
            ));
            frame.render_widget(ratatui::widgets::Clear, area);
            let mut lines = crate::mods_ui::slot_lines(&self.mods, "side_panel");
            if lines.is_empty() {
                lines.push("No active panels. /mods lists mods.".into());
            }
            let controls = crate::mods_ui::controls(&self.mods);
            if let Some(control) = controls.get(self.mod_focus % controls.len().max(1)) {
                lines.push(Line::raw(""));
                lines.push(Line::raw(format!(
                    "› {}{}",
                    control.label,
                    if control.input {
                        format!(": {}", self.mod_input_value(control))
                    } else {
                        String::new()
                    }
                )));
            }
            lines.push(Line::raw(""));
            lines.push("Tab: control · Enter: activate".into());
            lines.push("F7/Esc: chat · /mods run-tools".into());
            lines.push("/mods hot-reload on|off".into());
            lines.push("/mods test <id>".into());
            frame.render_widget(
                Paragraph::new(lines)
                    .wrap(Wrap { trim: false })
                    .block(Block::default().borders(Borders::ALL).title(" Mods · F7 ")),
                area,
            );
        }
        RenderAreas {
            transcript: Some(rows[1]),
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

// Wrap first, then pad each visual row: a line background alone would leave
// the unused columns after short text unpainted in the transcript paragraph.
fn highlighted_user_content(line: Line<'static>, width: u16) -> Vec<Line<'static>> {
    use ratatui::widgets::Widget;

    let padding = u16::from(width > 2);
    let inner_width = width.saturating_sub(padding * 2).max(1);
    let background = Style::default()
        .fg(crate::terminal_theme::TEXT)
        .bg(crate::terminal_theme::USER_MESSAGE_BACKGROUND);
    let paragraph = Paragraph::new(line)
        .style(background)
        .wrap(Wrap { trim: false });
    let height = paragraph
        .line_count(inner_width)
        .max(1)
        .min(u16::MAX as usize) as u16;
    let area = Rect::new(0, 0, inner_width, height);
    let mut buffer = Buffer::empty(area);
    paragraph.render(area, &mut buffer);
    (0..height)
        .map(|y| {
            let mut spans = vec![Span::styled(" ".repeat(padding as usize), background)];
            let mut x = 0;
            while x < inner_width {
                let cell = &buffer[(x, y)];
                spans.push(Span::styled(cell.symbol().to_owned(), cell.style()));
                x += UnicodeWidthStr::width(cell.symbol()).max(1) as u16;
            }
            spans.push(Span::styled(" ".repeat(padding as usize), background));
            Line::from(spans)
        })
        .collect()
}

fn mod_layout(area: Rect, dock: bool) -> (Rect, Option<Rect>) {
    let pad = if area.width >= 80 {
        2
    } else if area.width >= 60 {
        1
    } else {
        0
    };
    let mut main = Rect::new(
        area.x + pad,
        area.y,
        area.width.saturating_sub(pad * 2),
        area.height,
    );
    let panel = if dock && main.width >= 100 {
        let width = 38u16.min(main.width / 2);
        let panel = Rect::new(main.right() - width, main.y, width, main.height);
        main.width -= width;
        Some(panel)
    } else {
        None
    };
    (main, panel)
}

#[derive(Default)]
struct RenderAreas {
    transcript: Option<Rect>,
    back_to_bottom: Option<Rect>,
    composer: Option<Rect>,
    watermark: Option<Rect>,
}

pub(crate) struct ChatTui {
    terminal: Terminal<CrosstermBackend<io::Stdout>>,
    active: bool,
    mouse_selection: MouseSelection,
    transcript_selection: Option<TranscriptSelection>,
    transcript_area: Option<Rect>,
    selection_scroll_at: std::time::Instant,
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

// Keep styled text and visual-row offsets, rather than cells for the whole history.
struct TranscriptLayout {
    text: Text<'static>,
    row_starts: Vec<usize>,
    height: usize,
    width: u16,
}

impl TranscriptLayout {
    fn new(text: Text<'static>, width: u16) -> Self {
        let mut height = 0;
        let row_starts = text
            .lines
            .iter()
            .map(|line| {
                let start = height;
                height += Paragraph::new(line.clone())
                    .wrap(Wrap { trim: false })
                    .line_count(width.max(1))
                    .max(1);
                start
            })
            .collect();
        Self {
            text,
            row_starts,
            height,
            width,
        }
    }
    fn paragraph_at(&self, row: usize) -> Paragraph<'static> {
        let line = self
            .row_starts
            .partition_point(|start| *start <= row)
            .saturating_sub(1);
        let offset = row.saturating_sub(self.row_starts.get(line).copied().unwrap_or(0));
        let text = Text {
            lines: self.text.lines.get(line..).unwrap_or_default().to_vec(),
            style: self.text.style,
            alignment: self.text.alignment,
        };
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .scroll((offset.min(u16::MAX as usize) as u16, 0))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct TranscriptPoint {
    row: usize,
    column: u16,
}

struct TranscriptSelection {
    layout: TranscriptLayout,
    area: Rect,
    offset: usize,
    pointer: ScreenPoint,
    anchor: TranscriptPoint,
    head: TranscriptPoint,
    dragging: bool,
    had_drag: bool,
}

impl TranscriptSelection {
    fn from_text(text: Text<'static>, area: Rect, offset: usize, pointer: ScreenPoint) -> Self {
        let layout = TranscriptLayout::new(text, area.width);
        let anchor = TranscriptPoint {
            row: offset
                .saturating_add(pointer.row.saturating_sub(area.y) as usize)
                .min(layout.height.saturating_sub(1)),
            column: pointer.column.saturating_sub(area.x),
        };
        Self {
            layout,
            area,
            offset,
            pointer,
            anchor,
            head: anchor,
            dragging: true,
            had_drag: false,
        }
    }
    fn update(&mut self, pointer: ScreenPoint) {
        if !self.dragging {
            return;
        }
        self.pointer = pointer;
        self.had_drag = true;
        self.head = TranscriptPoint {
            row: self
                .offset
                .saturating_add(
                    (pointer.row.clamp(self.area.y, self.area.bottom() - 1) - self.area.y) as usize,
                )
                .min(self.layout.height.saturating_sub(1)),
            column: pointer.column.clamp(self.area.x, self.area.right() - 1) - self.area.x,
        };
    }
    fn range(&self) -> (TranscriptPoint, TranscriptPoint) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
    fn scroll(&mut self, scroll: &mut usize) -> bool {
        if !self.dragging || !self.had_drag {
            return false;
        }
        let max = self.layout.height.saturating_sub(self.area.height as usize);
        let old = self.offset;
        if self.pointer.row <= self.area.y {
            self.offset = self.offset.saturating_sub(1);
        } else if self.pointer.row >= self.area.bottom().saturating_sub(1) {
            self.offset = self.offset.saturating_add(1).min(max);
        }
        if old == self.offset {
            return false;
        }
        *scroll = max.saturating_sub(self.offset);
        self.update(self.pointer);
        true
    }
    fn paint(&self, buffer: &mut Buffer, back_button: Option<Rect>) {
        use ratatui::widgets::Widget;
        let mut snapshot = Buffer::empty(self.area);
        self.layout
            .paragraph_at(self.offset)
            .render(self.area, &mut snapshot);
        let (start, end) = self.range();
        for y in self.area.y..self.area.bottom() {
            let row = self.offset + (y - self.area.y) as usize;
            for x in self.area.x..self.area.right() {
                let position = Position::new(x, y);
                if back_button.is_some_and(|area| area.contains(position))
                    || (self.layout.height > self.area.height as usize
                        && x == self.area.right() - 1)
                {
                    continue;
                }
                if let (Some(target), Some(source)) =
                    (buffer.cell_mut(position), snapshot.cell(position))
                {
                    *target = source.clone();
                    let column = x - self.area.x;
                    if row >= start.row
                        && row <= end.row
                        && (row != start.row || column >= start.column)
                        && (row != end.row || column <= end.column)
                    {
                        target.set_style(
                            Style::default()
                                .bg(crate::terminal_theme::BLUE)
                                .fg(crate::terminal_theme::TEXT),
                        );
                    }
                }
            }
        }
    }
    fn text(&self) -> String {
        use ratatui::widgets::Widget;
        let (start, end) = self.range();
        let mut output = String::new();
        let mut row = start.row;
        while row <= end.row {
            let height = (end.row - row + 1).min(64) as u16;
            let area = Rect::new(0, 0, self.layout.width, height);
            let mut buffer = Buffer::empty(area);
            self.layout.paragraph_at(row).render(area, &mut buffer);
            let selection = MouseSelection {
                anchor: Some(ScreenPoint {
                    row: 0,
                    column: if row == start.row { start.column } else { 0 },
                }),
                head: Some(ScreenPoint {
                    row: height - 1,
                    column: if row + height as usize - 1 == end.row {
                        end.column
                    } else {
                        self.layout.width - 1
                    },
                }),
                dragging: false,
                had_drag: true,
            };
            if !output.is_empty() || row > start.row {
                output.push('\n');
            }
            output.push_str(&selected_screen_text(&buffer, selection));
            if output.len() > COPY_LIMIT {
                break;
            }
            row += height as usize;
        }
        output
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
    let start = input_row_start(chars, &rows, row_index);

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

fn input_row_start(chars: &[char], rows: &[String], row_index: usize) -> usize {
    let mut start = 0;
    for row in rows.iter().take(row_index) {
        start += row.chars().count();
        if chars.get(start) == Some(&'\n') {
            start += 1;
        }
    }
    start
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
    if let Some(dialog) = state.dialog.as_mut() {
        match kind {
            event::MouseEventKind::ScrollUp => {
                dialog.body_scroll = dialog
                    .body_scroll
                    .min(dialog.body_max_scroll.get())
                    .saturating_sub(3);
            }
            event::MouseEventKind::ScrollDown => {
                dialog.body_scroll = dialog
                    .body_scroll
                    .saturating_add(3)
                    .min(dialog.body_max_scroll.get());
            }
            _ => {}
        }
        return;
    }
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
                    transcript_selection: None,
                    transcript_area: None,
                    selection_scroll_at: std::time::Instant::now(),
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
    pub(super) fn draw_state(&mut self, state: &ChatViewState) -> Result<()> {
        let selection = self.mouse_selection;
        let previous = self.last_frame.as_ref();
        let mut transcript_selection = self.transcript_selection.as_mut();
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
            invalidated = transcript_selection.is_none()
                && !selection.dragging
                && selection.range().is_some()
                && previous
                    .map(|buffer| !selected_cells_match(buffer, frame.buffer_mut(), selection))
                    .unwrap_or(false);
            if !invalidated {
                if let Some(selected) = transcript_selection.as_deref_mut() {
                    if let Some(area) = areas
                        .transcript
                        .filter(|area| area.width > 0 && area.height > 0)
                    {
                        selected.area = area;
                    }
                    selected.paint(frame.buffer_mut(), areas.back_to_bottom);
                } else {
                    apply_mouse_selection(frame.buffer_mut(), selection);
                }
            }
            crate::terminal_theme::apply_tui_theme(frame.buffer_mut(), state.theme);
        })?;
        self.last_frame = Some(completed.buffer.clone());
        self.transcript_area = areas.transcript;
        self.back_to_bottom_area = areas.back_to_bottom;
        self.composer_area = areas.composer;
        self.watermark_area = areas.watermark;
        if invalidated {
            self.clear_mouse_selection();
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
            self.clear_mouse_selection();
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
                    self.clear_mouse_selection();
                    self.mouse_selection.start(point);
                    if let Some(area) = self.transcript_area.filter(|area| {
                        area.width > 0
                            && area.height > 0
                            && area.contains(Position::new(point.column, point.row))
                    }) && state.dialog.is_none()
                        && state.thought_modal.is_none()
                        && !state.transcript.is_empty()
                    {
                        let text = state.transcript_text(area.width);
                        let layout = TranscriptLayout::new(text, area.width);
                        let max = layout.height.saturating_sub(area.height as usize);
                        let offset = max.saturating_sub(state.scroll_from_bottom.min(max));
                        self.transcript_selection = Some(TranscriptSelection::from_text(
                            layout.text,
                            area,
                            offset,
                            point,
                        ));
                    }
                    state.selection_mode = false;
                    state.clear_notice();
                } else {
                    self.clear_mouse_selection();
                }
                Ok(true)
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                self.mouse_selection.update(point);
                if let Some(selected) = self.transcript_selection.as_mut() {
                    selected.update(point);
                }
                Ok(self.mouse_selection.dragging)
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some(selected) = self.transcript_selection.as_mut() {
                    selected.update(point);
                    selected.dragging = false;
                }
                if !self.mouse_selection.finish(point) {
                    self.transcript_selection = None;
                    return Ok(true);
                }
                if self
                    .last_frame
                    .as_ref()
                    .map(|buffer| {
                        self.transcript_selection
                            .as_ref()
                            .map_or_else(
                                || selected_screen_text(buffer, self.mouse_selection),
                                TranscriptSelection::text,
                            )
                            .trim()
                            .is_empty()
                    })
                    .unwrap_or(true)
                {
                    self.clear_mouse_selection();
                }
                Ok(true)
            }
            MouseEventKind::Down(MouseButton::Right) => {
                let text = self
                    .last_frame
                    .as_ref()
                    .map(|buffer| {
                        self.transcript_selection.as_ref().map_or_else(
                            || selected_screen_text(buffer, self.mouse_selection),
                            TranscriptSelection::text,
                        )
                    })
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
                self.clear_mouse_selection();
                Ok(true)
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                self.clear_mouse_selection();
                Ok(false)
            }
            _ => Ok(false),
        }
    }

    fn clear_mouse_selection(&mut self) {
        self.mouse_selection.clear();
        self.transcript_selection = None;
    }

    fn scroll_mouse_selection(&mut self, state: &mut ChatViewState) -> bool {
        if self.selection_scroll_at.elapsed() < std::time::Duration::from_millis(50) {
            return false;
        }
        self.selection_scroll_at = std::time::Instant::now();
        self.transcript_selection
            .as_mut()
            .is_some_and(|selection| selection.scroll(&mut state.scroll_from_bottom))
    }

    fn composer_selection_range(&self, state: &ChatViewState) -> Option<(usize, usize)> {
        if self.transcript_selection.is_some() {
            return None;
        }
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
                .map(|buffer| {
                    self.transcript_selection.as_ref().map_or_else(
                        || selected_screen_text(buffer, self.mouse_selection),
                        TranscriptSelection::text,
                    )
                })
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

    fn handle_tool_viewer_key(&mut self, state: &mut ChatViewState, key: event::KeyEvent) -> bool {
        if key.code == event::KeyCode::Char('y')
            && key.modifiers.contains(event::KeyModifiers::CONTROL)
            && let Some(tool) = state.tool_viewer.and_then(|index| state.tools.get(index))
        {
            match self.clipboard.copy(&tool.details(), &mut io::stdout()) {
                Ok(_) => state.set_notice("Tool details copied to clipboard"),
                Err(error) => state.set_notice(format!("Could not copy tool details: {error}")),
            }
            return true;
        }
        if state.handle_tool_key(key) {
            return true;
        }
        if state.tool_viewer.is_some() {
            if let Some(dialog) = state.dialog.as_mut()
                && dialog.handle_key(key).is_some()
            {
                state.dialog = None;
                state.tool_viewer = None;
            }
            return true;
        }
        false
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
        state.mods = mint_core::mods::list(
            &state.current_dir,
            &mint_core::scoped_chat_id(&state.chat_id, Some(&state.current_dir.to_string_lossy())),
        )
        .unwrap_or_default();
        let mut previous_bg_count = mint_core::bg_shell::running_count_for(&state.current_dir);
        let mut redraw = true;
        let mut painted_notice_visible = false;
        let mut reported_mod_viewport = None;
        let mut watermark_started: Option<std::time::Instant> = None;
        const DOUBLE_PRESS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

        loop {
            let size = self.terminal.size()?;
            let viewport = state.mod_viewport(Rect::new(0, 0, size.width, size.height));
            if reported_mod_viewport.as_ref() != Some(&viewport) {
                let chat = mint_core::scoped_chat_id(
                    &state.chat_id,
                    Some(&state.current_dir.to_string_lossy()),
                );
                let _ =
                    mint_core::mods::update_viewport(&state.current_dir, &chat, viewport.clone());
                state.mods = mint_core::mods::list(&state.current_dir, &chat).unwrap_or_default();
                reported_mod_viewport = Some(viewport);
                redraw = true;
            }
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

            let bg_count = mint_core::bg_shell::running_count_for(&state.current_dir);
            if bg_count != previous_bg_count {
                previous_bg_count = bg_count;
                redraw = true;
            }
            for notice in mint_core::bg_shell::take_finished_notices() {
                state.push_notice(notice);
                redraw = true;
            }
            redraw |= state.notice_visibility_changed_since_draw(painted_notice_visible);

            redraw |= self.scroll_mouse_selection(&mut state);
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
                } else if self.mouse_selection.dragging
                    || last_ctrl_d.is_some()
                    || state.active_notice().is_some()
                {
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
                        let mods = mint_core::mods::list(
                            &state.current_dir,
                            &mint_core::scoped_chat_id(
                                &state.chat_id,
                                Some(&state.current_dir.to_string_lossy()),
                            ),
                        )
                        .unwrap_or_default();
                        if mods != state.mods {
                            state.mods = mods;
                            redraw = true;
                        }
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
                    state.insert_paste(&text);
                }
                event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                    use event::{KeyCode, KeyModifiers};

                    if self.handle_tool_viewer_key(&mut state, key) {
                        continue;
                    }

                    let is_ctrl_c = key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if is_ctrl_c && self.copy_mouse_selection(&mut state) {
                        continue;
                    }

                    if self.mouse_selection.range().is_some() {
                        if matches!(key.code, KeyCode::Delete | KeyCode::Backspace) {
                            if let Some((start, end)) = self.composer_selection_range(&state) {
                                state.replace_composer(start..end, "");
                                state.clear_notice();
                            }
                            self.clear_mouse_selection();
                            continue;
                        }
                    }

                    self.clear_mouse_selection();

                    if key.code == KeyCode::F(7) && state.dialog.is_none() {
                        state.mods_open = !state.mods_open;
                        state.mod_input = None;
                        continue;
                    }
                    if state.mods_open && state.dialog.is_none() && state.thought_modal.is_none() {
                        let controls = crate::mods_ui::controls(&state.mods);
                        match key.code {
                            KeyCode::Esc => state.mods_open = false,
                            KeyCode::Tab => {
                                state.mod_focus = (state.mod_focus + 1) % controls.len().max(1);
                                state.mod_input = None;
                            }
                            KeyCode::BackTab => {
                                state.mod_focus = (state.mod_focus + controls.len().max(1) - 1)
                                    % controls.len().max(1);
                                state.mod_input = None;
                            }
                            KeyCode::Enter => {
                                if let Some(control) =
                                    controls.get(state.mod_focus % controls.len().max(1))
                                {
                                    let chat = mint_core::scoped_chat_id(
                                        &state.chat_id,
                                        Some(&state.current_dir.to_string_lossy()),
                                    );
                                    let value = if control.input {
                                        serde_json::json!(state.mod_input_value(control))
                                    } else {
                                        serde_json::Value::Null
                                    };
                                    let result = mint_core::mods::action(
                                        &state.current_dir,
                                        &chat,
                                        &control.id,
                                        &control.instance,
                                        &control.action,
                                        value,
                                    );
                                    if let Err(error) = result {
                                        state.set_notice(error);
                                    }
                                    state.mod_input = None;
                                    state.mods = mint_core::mods::list(&state.current_dir, &chat)
                                        .unwrap_or_default();
                                }
                            }
                            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                                if let Some(control) = controls
                                    .get(state.mod_focus % controls.len().max(1))
                                    .filter(|c| c.input)
                                {
                                    state.mod_input_draft(control).push(c);
                                }
                            }
                            KeyCode::Backspace => {
                                if let Some(control) = controls
                                    .get(state.mod_focus % controls.len().max(1))
                                    .filter(|c| c.input)
                                {
                                    state.mod_input_draft(control).pop();
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }
                    if key.code == KeyCode::F(6) {
                        let text = state.composer_text();
                        return Ok(Some(InteractiveInput {
                            text,
                            pasted_image,
                            switch_mode: true,
                        }));
                    }

                    if state.handle_thought_key(key) {
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
                    let composer_width =
                        state.composer_width(Rect::new(0, 0, size.width, size.height));
                    if state.edit_composer_key(key) {
                        continue;
                    }
                    match key.code {
                        KeyCode::Char('v') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if let Ok(Some(uri)) = crate::image::read_clipboard_image() {
                                if let Some(current) = pasted_image.as_mut() {
                                    current.push(' ');
                                    current.push_str(&uri);
                                } else {
                                    pasted_image = Some(uri);
                                }
                                let mut image_draft = state.composer.clone();
                                let cursor = state.cursor;
                                let mut end = cursor;
                                super::insert_image_placeholder(&mut image_draft, &mut end);
                                state.replace_composer(
                                    cursor..cursor,
                                    &image_draft[cursor..end].iter().collect::<String>(),
                                );
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
                            state.scroll_from_bottom = usize::MAX
                        }
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
                        KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
                            if state.navigate_composer(key, composer_width) => {}
                        KeyCode::End => state.scroll_from_bottom = 0,
                        KeyCode::Up if !state.composer.contains(&'\n') => state.history_previous(),
                        KeyCode::Down if !state.composer.contains(&'\n') => state.history_next(),
                        KeyCode::Enter => {
                            let suggestions = state.suggestions();
                            let input: String = state.composer.iter().collect();
                            if input.starts_with('/')
                                && !suggestions.is_empty()
                                && !suggestions.iter().any(|(token, _)| token == input.trim())
                            {
                                state.apply_selected_suggestion();
                            }
                            let text = state.take_prompt();
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
                                    state.stop_mods();
                                    return Ok(None);
                                }
                            }
                            last_ctrl_d = Some(std::time::Instant::now());
                            state.set_notice("Press Ctrl+D again to exit");
                        }
                        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                            if !state.composer.is_empty() {
                                state.take_prompt();
                                state.slash_selected = 0;
                                state.clear_notice();
                            } else {
                                state.set_notice("Press Ctrl+D twice to exit");
                            }
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
        self.wait_for_choice(handle, response)
    }

    pub fn prompt_update(
        &mut self,
        handle: &TuiHandle,
        current: &str,
        latest: &str,
    ) -> Result<Option<usize>> {
        let (reply, response) = mpsc::channel();
        if let Ok(mut state) = handle.state.lock() {
            state.dialog = Some(DialogState::new_update(current, latest, reply));
        }
        self.wait_for_choice(handle, response)
    }

    fn wait_for_choice(
        &mut self,
        handle: &TuiHandle,
        response: mpsc::Receiver<DialogAnswer>,
    ) -> Result<Option<usize>> {
        loop {
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
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
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
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
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
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
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
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
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
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
    pub fn run_onboard_wizard(&mut self, config: &mut mint_core::MintConfig) -> Result<bool> {
        let mut wizard = crate::onboard::OnboardWizard::new(config.clone());
        let outcome = wizard.run_with_terminal(&mut self.terminal)?;
        match outcome {
            crate::onboard::OnboardOutcome::SavedAndLaunch
            | crate::onboard::OnboardOutcome::SavedAndExit => {
                *config = wizard.take_config();
                Ok(true)
            }
            crate::onboard::OnboardOutcome::Cancelled => Ok(false),
        }
    }
    pub async fn drive_agent<T: Send + 'static>(
        &mut self,
        handle: &TuiHandle,
        task: tokio::task::JoinHandle<T>,
    ) -> Result<T> {
        use event::{KeyCode, KeyModifiers};
        loop {
            if let Ok(mut state) = handle.state.lock() {
                self.scroll_mouse_selection(&mut state);
                self.draw_state(&state)?;
            }
            if task.is_finished() {
                if let Ok(mut state) = handle.state.lock() {
                    state.finish_live_thought();
                    state.finish_running_tools();
                }
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
                            state.insert_paste(&text);
                        }
                    }
                    event::Event::Key(key) if key.kind == event::KeyEventKind::Press => {
                        let mut selection_handled = false;
                        if let Ok(mut state) = handle.state.lock() {
                            if self.handle_tool_viewer_key(&mut state, key) {
                                selection_handled = true;
                            } else if let Some(dialog) = state.dialog.as_mut() {
                                if let Some(answer) = dialog.handle_key(key) {
                                    if let Some(dialog) = state.dialog.take() {
                                        let _ = dialog.reply.send(answer);
                                    }
                                }
                                selection_handled = true;
                            } else if state.handle_thought_key(key) {
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
                                    state.replace_composer(start..end, "");
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
                            let size = self.terminal.size()?;
                            let composer_width =
                                state.composer_width(Rect::new(0, 0, size.width, size.height));
                            if state.edit_composer_key(key) {
                                continue;
                            }
                            match key.code {
                                KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End
                                    if state.navigate_composer(key, composer_width) => {}
                                KeyCode::End => state.scroll_from_bottom = 0,
                                KeyCode::Enter => {
                                    if !state.composer.is_empty() {
                                        let text = state.take_prompt();
                                        if let Ok(mut queued) = handle.queued.lock() {
                                            queued.push(text);
                                        }
                                    }
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

    fn saved_activity_fixture(
        activity: serde_json::Value,
        status: &str,
    ) -> (std::path::PathBuf, mint_core::MemoryStore) {
        let root = std::env::temp_dir().join(format!(
            "mint-resume-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let memory = mint_core::MemoryStore::open(root.join("memory.sqlite"));
        let turn = memory.start_turn("resume-details", "Check files").unwrap();
        memory.claim_turn("resume-details", turn).unwrap();
        memory
            .set_interaction_agent_activity_json(turn, &activity.to_string())
            .unwrap();
        if status == "completed" {
            memory
                .finish_turn(turn, "Saved answer", "test", "model", None)
                .unwrap();
        } else {
            memory.end_turn(turn, status).unwrap();
        }
        (root, memory)
    }

    #[test]
    fn resume_restores_tool_results_thoughts_and_run_summary_without_duplicates() {
        let command = "python3 - <<'EOF'\nprint('complete command')\nEOF";
        let activity = serde_json::json!([
            {"type":"ExtendedThinking","data":{"id":"thought-one","thought":"Saved reasoning","elapsed_ms":1900}},
            {"type":"ToolStart","data":{"callId":"one","action":"run_shell","input":{"command":command}}},
            {"type":"ToolEnd","data":{"callId":"one","action":"run_shell","input":{"command":command},"result":"exit: 2\nstderr:\nfull saved error", "elapsed_ms":3400}},
            {"type":"RunCompleted","data":{"summary":{"runId":"r1","task":"Check files","outcome":"FAILED","totalTokens":100,"toolCallsCount":1,"filesChanged":[],"durationSecs":16.2,"toolTimeline":[{"step":1,"action":"run_shell","target":"python3","success":false,"retried":false}],"retriesCount":0}}}
        ]);
        let (root, memory) = saved_activity_fixture(activity, "completed");
        let mut state = ChatViewState::default();
        for _ in 0..2 {
            state.reload_transcript_with(&memory, "resume-details", &root);
            assert_eq!(state.tools.len(), 1);
            assert_eq!(state.thoughts.len(), 1);
            let text = state
                .transcript
                .iter()
                .map(|entry| entry.text.as_str())
                .collect::<Vec<_>>()
                .join("\n");
            assert!(text.contains("run_shell failed · 3.4s · exit 2"));
            assert_eq!(text.matches("Thought · 1.9s").count(), 1);
            assert!(text.contains("✗ Failed"));
            assert!(text.contains("16.2s"));
            assert!(!state.tools[0].running());
            assert!(state.handle_thought_key(ctrl_t()));
            assert_eq!(
                state.thought_modal.as_ref().unwrap().lines[1..],
                vec!["Saved reasoning"]
            );
            state.handle_thought_key(make_key(KeyCode::Esc));
            let mut open = make_key(KeyCode::Char('o'));
            open.modifiers = KeyModifiers::CONTROL;
            assert!(state.handle_tool_key(open));
            let body = &state.dialog.as_ref().unwrap().body;
            assert!(body.contains(command));
            assert!(body.contains("full saved error"));
            state.handle_tool_key(make_key(KeyCode::Esc));
        }
        assert!(!state.refresh_shared_transcript_with(&memory));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_switching_sessions_closes_previous_tool_details() {
        let (root, memory) = saved_activity_fixture(
            serde_json::json!([
                {"type":"ToolEnd","data":{"callId":"old","action":"run_shell","input":{"command":"echo previous-session"},"result":"exit: 0"}}
            ]),
            "completed",
        );
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        let mut key = make_key(KeyCode::Char('o'));
        key.modifiers = KeyModifiers::CONTROL;
        assert!(state.handle_tool_key(key));
        assert!(state.dialog.is_some());
        state.reload_transcript_with(&memory, "different-session", &root);
        assert!(
            state.dialog.is_none(),
            "previous session tool details remained open"
        );
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_legacy_activity_does_not_invent_zero_second_tool_duration() {
        let (root, memory) = saved_activity_fixture(
            serde_json::json!([
                {"type":"ToolStart","data":{"action":"run_shell","input":{"command":"echo legacy"}}},
                {"type":"ToolEnd","data":{"action":"run_shell","input":{"command":"echo legacy"},"result":"exit: 0\nlegacy output"}}
            ]),
            "completed",
        );
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        assert_eq!(state.tools.len(), 1);
        assert!(state.tools[0].heading().contains("time unavailable"));
        assert!(!state.tools[0].running());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_restores_web_search_sources_in_transcript() {
        let (root, memory) = saved_activity_fixture(
            serde_json::json!([
                {"type":"ToolStart","data":{"callId":"call_1","action":"web_search","input":{"query":"rust ratatui"}}},
                {"type":"ToolEnd","data":{"callId":"call_1","action":"web_search","input":{"query":"rust ratatui"},"result":"1. Ratatui Docs\n   URL: https://ratatui.rs\n   Terminal UI library"}}
            ]),
            "completed",
        );
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        let sources_entry = state.transcript.iter().find(|entry| {
            entry.role == TranscriptRole::Notice && entry.text.starts_with("Sources:\n")
        });
        assert!(
            sources_entry.is_some(),
            "Sources notice should be reconstructed upon resume"
        );
        let text = &sources_entry.unwrap().text;
        assert!(text.contains("1. Ratatui Docs\n   https://ratatui.rs"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn resume_interrupted_activity_keeps_partial_thought_and_never_restarts_spinner() {
        let events: Vec<mint_core::AgentProgress> = serde_json::from_value(serde_json::json!([
            {"type":"ThinkingDelta","data":{"id":"partial","delta":"unfinished ","elapsed_ms":400}},
            {"type":"ThinkingDelta","data":{"id":"partial","delta":"reasoning","elapsed_ms":700}},
            {"type":"ToolStart","data":{"callId":"unfinished","action":"run_shell","input":{"command":"sleep 100"}}}
        ])).unwrap();
        let activity =
            serde_json::to_value(mint_core::orchestration::compact_agent_progress(&events))
                .unwrap();
        let (root, memory) = saved_activity_fixture(activity, "interrupted");
        let mut state = ChatViewState::default();
        state.reload_transcript_with(&memory, "resume-details", &root);
        assert_eq!(state.tools.len(), 1);
        assert!(!state.tools[0].running());
        assert!(state.tools[0].heading().contains("interrupted"));
        assert!(super::super::tool_activity::running_lines(&state.tools, 80, 0).is_empty());
        assert!(state.handle_thought_key(ctrl_t()));
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["unfinished reasoning"]
        );
        state.reload_transcript_with(&memory, "another-session", &root);
        assert!(state.thoughts.is_empty());
        assert!(state.tools.is_empty());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn device_saved_activity_survives_tui_reload_and_external_sync() {
        let root = std::env::temp_dir().join(format!(
            "mint-device-tui-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&root).unwrap();
        let memory = mint_core::MemoryStore::open(root.join("memory.sqlite"));
        let chat = "device-history";
        let mut state = ChatViewState {
            chat_id: chat.into(),
            current_dir: root.clone(),
            ..Default::default()
        };
        state.reload_transcript_with(&memory, chat, &root);
        let turn = memory.start_turn(chat, "Move motor").unwrap();
        memory.claim_turn(chat, turn).unwrap();
        let activity = serde_json::json!([{"type":"DeviceState","data":{"callId":"one","server":"motor","tool":"move","update":{"state":"verified","target":{"rpm":100},"actual":{"rpm":100},"elapsedSecs":2,"message":"Observed motor"}}}]);
        memory
            .set_interaction_agent_activity_json(turn, &activity.to_string())
            .unwrap();
        memory
            .finish_turn(turn, "done", "test", "model", None)
            .unwrap();
        assert!(state.refresh_shared_transcript_with(&memory));
        assert!(
            state
                .transcript
                .iter()
                .any(|entry| entry.text.contains("Verified")),
            "external sync omitted device activity"
        );
        assert!(!state.refresh_shared_transcript_with(&memory));
        state.reload_transcript_with(&memory, chat, &root);
        assert_eq!(
            state
                .transcript
                .iter()
                .filter(|entry| entry.text.contains("Verified"))
                .count(),
            1
        );
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
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
        assert!(rendered.contains("Verified"));
        assert!(rendered.contains("rpm"));
        assert!(rendered.contains("100"));
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn device_and_discovery_events_render_inside_full_screen_tui() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = TuiHandle {
            state: Arc::clone(&state),
            interrupted: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(Vec::new())),
        };
        let mut events = vec![
            serde_json::json!({"type":"ToolDiscovery","data":{"server":"motor","tool":"set_speed","message":"Reading device schema"}}),
        ];
        for phase in ["accepted", "verified"] {
            events.push(serde_json::json!({"type":"DeviceState","data":{"callId":"one","server":"motor","tool":"set_speed","update":{"state":phase,"target":{"rpm":100},"actual":{"rpm":100},"elapsedSecs":2,"message":"Observed motor"}}}));
        }
        events.push(serde_json::json!({"type":"ToolArtifacts","data":{"callId":"one","artifacts":[],"warnings":["Sample attachment unavailable"]}}));
        for event in events {
            crate::agent::publish_mcp_progress(Some(&tui), &serde_json::from_value(event).unwrap());
        }
        let mut terminal = Terminal::new(TestBackend::new(100, 30)).unwrap();
        terminal
            .draw(|frame| {
                state.lock().unwrap().render(frame);
            })
            .unwrap();
        let rendered: String = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect();
        assert!(rendered.contains("Reading device schema"));
        assert!(rendered.contains("Accepted"));
        assert!(rendered.contains("Verified"));
        assert!(rendered.contains("target"));
        assert!(rendered.contains("actual"));
        assert!(rendered.contains("Sample attachment unavailable"));
    }

    #[test]
    fn tui_suppresses_redundant_execution_update_notices() {
        use mint_core::{ExecutionEvidence, ExecutionState};
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = TuiHandle {
            state: Arc::clone(&state),
            interrupted: Arc::new(AtomicBool::new(false)),
            queued: Arc::new(Mutex::new(Vec::new())),
        };
        let update = mint_core::AgentProgress::ExecutionUpdate {
            call_id: "test".into(),
            executions: vec![ExecutionEvidence {
                call_id: "test1".into(),
                command: "ls -la".into(),
                state: ExecutionState::Succeeded,
                exit_code: Some(0),
                job_id: None,
                revision: 1,
                check: false,
                condition_verified: None,
            }],
        };
        let published = crate::agent::publish_mcp_progress(Some(&tui), &update);
        assert!(!published, "ExecutionUpdate should be suppressed in TUI");
        let transcript = state.lock().unwrap().transcript.clone();
        assert!(
            !transcript
                .iter()
                .any(|entry| entry.text.contains("[Command]")),
            "TUI transcript must not receive redundant [Command] notices"
        );
    }

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

    #[test]
    fn short_sent_user_message_starts_after_label_on_the_same_row() {
        let mut state = ChatViewState::default();
        state
            .transcript
            .push(TranscriptEntry::new(TranscriptRole::User, "ok เข้าใจละ"));
        let text = state.transcript_text(58);
        let mut terminal = Terminal::new(TestBackend::new(58, 4)).unwrap();
        terminal
            .draw(|frame| {
                frame.render_widget(
                    Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
                    frame.area(),
                );
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let first: String = (0..58).map(|x| buffer[(x, 0)].symbol()).collect();
        assert!(first.contains("You › ok เข้าใจละ"));
        assert_eq!(
            buffer[(1, 1)].bg,
            Color::Reset,
            "short messages should occupy one shaded row"
        );
        assert_eq!(state.plain_transcript_lines(), vec!["You › ok เข้าใจละ", ""]);
    }

    #[test]
    fn sent_user_message_background_covers_wrapped_text_at_terminal_width() {
        let mut state = ChatViewState::default();
        state.transcript.push(TranscriptEntry::new(
            TranscriptRole::User,
            format!("{}\nข้อความของฉัน", "hello ".repeat(24)),
        ));
        state
            .transcript
            .push(TranscriptEntry::new(TranscriptRole::Assistant, "Reply"));
        for width in [60, 100] {
            let mut terminal = Terminal::new(TestBackend::new(width, 40)).unwrap();
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            let buffer = terminal.backend().buffer();
            let rows: Vec<String> = (0..40)
                .map(|y| (0..width).map(|x| buffer[(x, y)].symbol()).collect())
                .collect();
            let start = rows.iter().position(|row| row.contains("You ›")).unwrap();
            let end = rows.iter().position(|row| row.contains("Mint ›")).unwrap();
            let padding = if width >= 80 { 2 } else { 1 };
            for y in start..end - 1 {
                for x in padding..width - padding {
                    assert_eq!(
                        buffer[(x, y as u16)].bg,
                        Color::Rgb(64, 64, 64),
                        "sent message background must fill every visual row, including padding"
                    );
                }
                assert!(!rows[y].contains(['╭', '╮', '╰', '╯', '│']));
            }
            assert_eq!(
                buffer[(padding, end as u16)].bg,
                Color::Reset,
                "assistant messages must remain outside the user background"
            );
            let mut light_buffer = buffer.clone();
            crate::terminal_theme::apply_tui_theme(
                &mut light_buffer,
                crate::terminal_theme::TuiTheme::Light,
            );
            assert_eq!(
                light_buffer[(padding, start as u16)].bg,
                Color::Rgb(225, 225, 225)
            );
            let mut auto_buffer = buffer.clone();
            crate::terminal_theme::apply_tui_theme(
                &mut auto_buffer,
                crate::terminal_theme::TuiTheme::Auto,
            );
            assert_eq!(
                auto_buffer[(padding + 1, (start + 1) as u16)].fg,
                Color::White,
                "text on the dark user background must keep its contrast in Auto theme"
            );
            assert!(
                end - start > 3,
                "long user text must wrap within the background"
            );
            assert_eq!(
                rows[start..end - 1]
                    .iter()
                    .map(|row| row.matches("hello").count())
                    .sum::<usize>(),
                24
            );
            assert!(
                rows[start..end - 1]
                    .iter()
                    .any(|row| row.contains("ข้อความของฉัน"))
            );
        }
        // Decorative boundaries must not enter keyboard copy or shift selection indices.
        state.selection_anchor = 0;
        state.selection_head = 1;
        assert_eq!(
            state.selected_text(),
            format!("You › {}\nข้อความของฉัน", "hello ".repeat(24))
        );
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
    fn slash_suggestion_finds_description_and_completes_executable_token() {
        let mut state = ChatViewState::default();
        state.set_draft("/statistics".to_owned());
        assert_eq!(
            state.suggestions().first().map(|item| item.0.as_str()),
            Some("/stats")
        );
        assert!(state.apply_selected_suggestion());
        assert_eq!(state.composer_text(), "/stats ");
        state.set_draft("/reasoning".to_owned());
        assert!(
            state
                .suggestions()
                .iter()
                .all(|(token, _)| token.starts_with('/'))
        );
        state.set_draft("/remember a durable fact".to_owned());
        assert!(state.suggestions().is_empty());
        state.set_draft("/st".to_owned());
        assert_eq!(
            state.suggestions().first().map(|item| item.0.as_str()),
            Some("/stats")
        );
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
    fn long_skill_diff_keeps_decisions_visible_and_can_be_reviewed_to_the_end() {
        let (reply, _) = mpsc::channel();
        let mut state = ChatViewState::default();
        state.dialog = Some(DialogState::new_choice(
            "Save skill?",
            format!(
                "Changes instructions used in future tasks.\nPath: /workspace/.agents/skills/example/SKILL.md\n\n{}",
                (1..=100)
                    .map(|i| format!("+ Skill instruction {i:03}: {}", "รายละเอียด ".repeat(8)))
                    .collect::<Vec<_>>()
                    .join("\n")
            ),
            vec!["Approve".into(), "Deny".into()],
            None,
            reply,
        ));
        for (width, height) in [(80, 24), (160, 42), (60, 16)] {
            state.dialog.as_mut().unwrap().selected = 0;
            let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            let visible = |buffer: &Buffer| {
                buffer
                    .content
                    .iter()
                    .map(|cell| cell.symbol())
                    .collect::<String>()
            };
            let rendered = visible(terminal.backend().buffer());
            assert!(
                rendered.contains("1. Approve"),
                "{width}x{height}: approve hidden"
            );
            assert!(
                rendered.contains("2. Deny"),
                "{width}x{height}: deny hidden"
            );
            assert!(
                rendered.contains("Enter select"),
                "{width}x{height}: hint hidden"
            );
            assert!(rendered.contains("PgUp/PgDn"));
            assert!(rendered.contains("Changes instructions"));
            let dialog = state.dialog.as_mut().unwrap();
            assert_eq!(
                dialog.handle_key(KeyEvent::new(KeyCode::End, KeyModifiers::NONE)),
                None
            );
            assert_eq!(dialog.selected, 0);
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            let rendered = visible(terminal.backend().buffer());
            assert!(
                rendered.contains("Skill instruction 100"),
                "{width}x{height}: last diff line hidden"
            );
            assert!(rendered.contains("2. Deny"));
            route_mouse_wheel(&mut state, event::MouseEventKind::ScrollDown);
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(visible(terminal.backend().buffer()).contains("Skill instruction 100"));
            route_mouse_wheel(&mut state, event::MouseEventKind::ScrollUp);
            assert_eq!(
                state.scroll_from_bottom, 0,
                "wheel must review the dialog, not chat"
            );
            let dialog = state.dialog.as_mut().unwrap();
            dialog.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(visible(terminal.backend().buffer()).contains("Changes instructions"));
            let dialog = state.dialog.as_mut().unwrap();
            dialog.handle_key(KeyEvent::new(KeyCode::PageDown, KeyModifiers::NONE));
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(!visible(terminal.backend().buffer()).contains("Changes instructions"));
            state
                .dialog
                .as_mut()
                .unwrap()
                .handle_key(KeyEvent::new(KeyCode::PageUp, KeyModifiers::NONE));
            terminal
                .draw(|frame| {
                    state.render(frame);
                })
                .unwrap();
            assert!(visible(terminal.backend().buffer()).contains("Changes instructions"));
            let dialog = state.dialog.as_mut().unwrap();
            dialog.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
            assert_eq!(
                dialog.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
                Some(DialogAnswer::Choice(1))
            );
            dialog.handle_key(KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        }
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
    fn background_count_updates_above_composer_and_uses_the_actual_workspace() {
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
        let _cleanup = Cleanup(vec![first.id.clone(), second.id.clone()], root.clone());
        let mut state = ChatViewState::default();
        state.current_dir = root.clone();
        state.workspace = format!("{} · branch: Rust", root.display());
        state.set_notice("A temporary notice");
        for width in [160, 40] {
            let expected = "2 Background Terminals running";
            let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
            let mut composer = None;
            terminal
                .draw(|frame| composer = state.render(frame).composer)
                .unwrap();
            let area = composer.unwrap();
            let rendered: String = (area.x..area.right())
                .map(|x| {
                    terminal
                        .backend()
                        .buffer()
                        .cell(Position::new(x, area.y))
                        .unwrap()
                        .symbol()
                })
                .collect();
            assert!(
                rendered.contains(expected),
                "missing count above composer: {rendered}"
            );
            assert!(
                rendered.trim_end_matches([' ', '─']).ends_with("running"),
                "count must align right: {rendered}"
            );
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
        }
        let wait_for_count = |expected| {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while mint_core::bg_shell::running_count_for(&root) != expected {
                assert!(std::time::Instant::now() < deadline);
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
        };
        mint_core::bg_shell::kill_job(&first.id).unwrap();
        wait_for_count(1);
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
        assert!(rendered.contains("1 Background Terminal running"));
        mint_core::bg_shell::kill_job(&second.id).unwrap();
        wait_for_count(0);
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
        assert!(!rendered.contains("Background Terminal"));
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
    fn transcript_selection_copies_across_render_chunks_without_losing_rows() {
        let text = format!("start\n{}end", "middle\n".repeat(64));
        let area = Rect::new(0, 3, 8, 3);
        let mut drag =
            TranscriptSelection::from_text(Text::from(text.clone()), area, 0, point(0, 3));
        drag.update(point(7, 5));
        let mut scroll = 63;
        for _ in 0..70 {
            drag.scroll(&mut scroll);
        }
        assert_eq!(drag.text(), text);
    }

    #[test]
    fn transcript_selection_can_copy_the_visible_tail_beyond_u16_rows() {
        let mut lines = vec![Line::raw("earlier"); 66_000];
        lines.extend([
            Line::raw("tail one"),
            Line::raw("tail two"),
            Line::raw("tail end"),
        ]);
        let area = Rect::new(0, 3, 8, 3);
        let mut drag = TranscriptSelection::from_text(Text::from(lines), area, 66_000, point(0, 3));
        drag.update(point(7, 5));
        assert_eq!(drag.text(), "tail one\ntail two\ntail end");
    }

    #[test]
    fn transcript_selection_keeps_the_back_button_and_scrollbar_visible() {
        let area = Rect::new(0, 3, 8, 3);
        let drag = TranscriptSelection::from_text(
            Text::from(
                (0..9)
                    .map(|row| Line::raw(format!("line{row}")))
                    .collect::<Vec<_>>(),
            ),
            area,
            0,
            point(0, 4),
        );
        let mut screen = Buffer::empty(Rect::new(0, 0, 8, 8));
        screen.set_string(2, 5, "Back", Style::default());
        screen.set_string(7, 3, "▲", Style::default());
        drag.paint(&mut screen, Some(Rect::new(2, 5, 4, 1)));
        assert_eq!(screen.cell(Position::new(7, 3)).unwrap().symbol(), "▲");
        assert_eq!(screen.cell(Position::new(2, 5)).unwrap().symbol(), "B");
    }

    #[test]
    fn transcript_drag_scrolls_both_edges_and_keeps_offscreen_text() {
        let area = Rect::new(0, 3, 8, 3);
        let mut drag = TranscriptSelection::from_text(
            Text::from(
                (0..9)
                    .map(|row| Line::raw(format!("line{row}")))
                    .collect::<Vec<_>>(),
            ),
            area,
            0,
            point(0, 4),
        );
        drag.update(point(4, 5));
        let mut scroll = 6;
        for _ in 0..10 {
            drag.scroll(&mut scroll);
        }
        assert_eq!(scroll, 0);
        assert_eq!(
            drag.text(),
            "line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8"
        );
        let mut screen = Buffer::empty(Rect::new(0, 0, 8, 8));
        screen.set_string(0, 3, "new text", Style::default());
        drag.paint(&mut screen, None);
        assert_eq!(screen.cell(Position::new(0, 3)).unwrap().symbol(), "l");
        assert_eq!(
            screen.cell(Position::new(0, 3)).unwrap().bg,
            crate::terminal_theme::BLUE
        );
        assert_eq!(screen.cell(Position::new(0, 2)).unwrap().symbol(), " ");
        drag.update(point(0, 3));
        for _ in 0..10 {
            drag.scroll(&mut scroll);
        }
        assert_eq!(scroll, 6);
        assert_eq!(drag.text(), "line0\nl");
        drag.dragging = false;
        drag.update(point(4, 5));
        assert!(!drag.scroll(&mut scroll));
        assert_eq!(scroll, 6);
    }

    #[test]
    fn transcript_drag_interior_does_not_scroll() {
        let area = Rect::new(0, 3, 8, 3);
        let mut drag = TranscriptSelection::from_text(
            Text::from(vec![Line::raw(""); 9]),
            area,
            2,
            point(0, 4),
        );
        drag.update(point(4, 4));
        let mut scroll = 4;
        assert!(!drag.scroll(&mut scroll));
        assert_eq!(scroll, 4);
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

    fn thought_screen(state: &ChatViewState, width: u16, height: u16) -> Vec<String> {
        let mut terminal = Terminal::new(TestBackend::new(width, height)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content
            .chunks(width as usize)
            .map(|cells| cells.iter().map(|cell| cell.symbol()).collect())
            .collect()
    }

    fn ctrl_t() -> KeyEvent {
        let mut key = make_key(KeyCode::Char('t'));
        key.modifiers = KeyModifiers::CONTROL;
        key
    }

    fn tool_event(value: serde_json::Value) -> mint_core::AgentProgress {
        serde_json::from_value(value).unwrap()
    }

    #[test]
    fn running_tool_status_stays_above_composer_with_command_and_spinner() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":"a","action":"run_shell","input":{"command":"sleep 5 # live-command-marker"}}})));
        {
            let mut state = state.lock().unwrap();
            for _ in 0..35 {
                state.push_notice("older transcript row");
            }
            state.scroll_from_bottom = 20;
            state.status = (0..12).map(|index| format!("Plan row {index}")).collect();
            let rows = thought_screen(&state, 100, 30);
            let pinned = rows[22..26].join("\n");
            assert!(
                pinned.contains("Running 1 shell command"),
                "missing active summary: {pinned}"
            );
            assert!(
                pinned.contains("live-command-marker"),
                "missing running command: {pinned}"
            );
            assert!(
                pinned.chars().any(|ch| "⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏".contains(ch)),
                "missing animated spinner: {pinned}"
            );
        }
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"a","action":"run_shell","input":{"command":"sleep 5 # live-command-marker"},"result":"exit: 0"}})));
        let state = state.lock().unwrap();
        assert!(
            !thought_screen(&state, 100, 30)[22..26]
                .join("\n")
                .contains("live-command-marker")
        );
    }

    #[test]
    fn compact_tools_details_cannot_replace_an_approval_dialog() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":"a","action":"run_shell","input":{"command":"echo hi"}}})));
        {
            let mut state = state.lock().unwrap();
            let mut key = make_key(KeyCode::Char('o'));
            key.modifiers = KeyModifiers::CONTROL;
            assert!(state.handle_tool_key(key));
            let (reply, _) = mpsc::channel();
            state.dialog = Some(DialogState::new_choice(
                "Local shell command",
                "Approval required",
                vec!["Yes".into(), "No".into()],
                None,
                reply,
            ));
        }
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"a","action":"run_shell","input":{"command":"echo hi"},"result":"exit: 0"}})));
        let state = state.lock().unwrap();
        assert_eq!(state.dialog.as_ref().unwrap().title, "Local shell command");
        assert!(state.tool_viewer.is_none());
    }

    #[test]
    fn identical_legacy_starts_keep_separate_running_cards() {
        let mut state = ChatViewState::default();
        let start = tool_event(
            serde_json::json!({"type":"ToolStart","data":{"action":"read_file","input":{"path":"same"}}}),
        );
        state.record_tool_progress(&start);
        state.record_tool_progress(&start);
        assert_eq!(state.tools.len(), 2);
        let end = tool_event(
            serde_json::json!({"type":"ToolEnd","data":{"action":"read_file","input":{"path":"same"},"result":"contents"}}),
        );
        state.record_tool_progress(&end);
        assert_eq!(state.tools.iter().filter(|tool| tool.running()).count(), 1);
        state.record_tool_progress(&end);
        assert_eq!(state.tools.len(), 2);
        assert!(state.tools.iter().all(|tool| !tool.running()));
    }

    #[test]
    fn compact_tools_count_only_running_calls_and_update_existing_card() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        for (id, action) in [("a", "run_shell"), ("b", "run_shell"), ("c", "read_file")] {
            tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":id,"action":action,"input":{"command":"echo hi"}}})));
        }
        assert!(
            thought_screen(&state.lock().unwrap(), 100, 30)
                .join("\n")
                .contains("Running 3 tasks · 2 shell commands · 1 tool")
        );
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"a","action":"run_shell","input":{"command":"echo hi"},"result":"exit: 0\nstdout:\nhi"}})));
        let state = state.lock().unwrap();
        assert_eq!(state.transcript.len(), 3);
        let screen = thought_screen(&state, 100, 30).join("\n");
        assert!(screen.contains("run_shell completed"));
        assert!(screen.contains("Running 2 tasks · 1 shell command · 1 tool"));
        assert!(!screen.contains("stdout:"));
    }

    #[test]
    fn compact_tools_hide_multiline_commands_and_keep_full_details_accessible() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        let command = format!(
            "python3 - <<'EOF'\n{}\nEOF",
            "print('secret-detail')\n".repeat(40)
        );
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":"a","action":"run_shell","input":{"command":command}}})));
        let mut state = state.lock().unwrap();
        assert!(
            !thought_screen(&state, 80, 24)
                .join("\n")
                .contains("secret-detail")
        );
        let mut key = make_key(KeyCode::Char('o'));
        key.modifiers = KeyModifiers::CONTROL;
        assert!(state.handle_tool_key(key));
        assert!(state.dialog.as_ref().unwrap().body.contains(&command));
        assert!(state.handle_tool_key(make_key(KeyCode::Esc)));
        assert!(state.dialog.is_none());
    }

    #[test]
    fn compact_tools_match_parallel_results_by_id_and_keep_failures_red() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let tui = ChatTui::handle(state.clone());
        for id in ["a", "b"] {
            tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolStart","data":{"callId":id,"action":"run_shell","input":{"command":"same-command"}}})));
        }
        tui.tool_progress(&tool_event(serde_json::json!({"type":"ToolEnd","data":{"callId":"b","action":"run_shell","input":{"command":"same-command"},"result":"exit: 2\nstderr:\nfull failure"}})));
        let state = state.lock().unwrap();
        assert_eq!(state.transcript.len(), 2);
        assert_eq!(
            state.transcript[1].rendered_lines[0].spans[0].style.fg,
            Some(Color::Red)
        );
        assert!(
            thought_screen(&state, 80, 24)
                .join("\n")
                .contains("Running 1 shell command")
        );
    }

    #[test]
    fn live_thought_timer_advances_without_deltas_and_preview_fits_small_terminals() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "checking code", std::time::Duration::ZERO);
        let mut state = state.lock().unwrap();
        state.thoughts.last_mut().unwrap().updated_at =
            Some(std::time::Instant::now() - std::time::Duration::from_secs(3));
        assert!(
            thought_screen(&state, 80, 24)
                .join("\n")
                .contains("Thinking · 3.0s")
        );
        for (width, height) in [(40, 16), (60, 20), (80, 24)] {
            let rows = thought_screen(&state, width, height);
            let top = rows
                .iter()
                .position(|line| line.trim_start().starts_with('┌'))
                .unwrap();
            let bottom = rows
                .iter()
                .enumerate()
                .skip(top + 1)
                .find(|(_, line)| line.trim_start().starts_with('└'))
                .unwrap()
                .0;
            assert!(bottom - top < 6);
            assert!(rows.join("\n").contains("checking code"));
        }
        assert!(state.handle_thought_key(ctrl_t()));
        for (width, height) in [(20, 8), (40, 12), (80, 24)] {
            thought_screen(&state, width, height);
        }
    }

    #[test]
    fn live_thought_history_keeps_steps_once_and_remains_accessible_next_turn() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "first partial", std::time::Duration::ZERO);
        handle.finish_thought(
            Some("one"),
            "first complete",
            std::time::Duration::from_secs(1),
        );
        handle.append_thought_delta("two", "second partial", std::time::Duration::ZERO);
        assert!(state.lock().unwrap().handle_thought_key(ctrl_t()));
        handle.finish_thought(
            Some("two"),
            "second complete",
            std::time::Duration::from_secs(2),
        );
        {
            let state = state.lock().unwrap();
            let text = state.thought_modal.as_ref().unwrap().lines.join("\n");
            assert_eq!(text.matches("first complete").count(), 1);
            assert_eq!(text.matches("second complete").count(), 1);
            assert!(!text.contains("partial"));
            let screen = thought_screen(&state, 100, 30).join("\n");
            assert!(screen.contains("Thought Process · 3.0s"), "{screen}");
            assert!(screen.contains("Thought 1 · 1.0s"), "{screen}");
            assert!(screen.contains("Thought 2 · 2.0s"), "{screen}");
            assert_eq!(state.transcript.len(), 2);
        }
        handle.reset_thoughts();
        let mut state = state.lock().unwrap();
        assert!(state.thought_modal.is_none());
        assert!(state.handle_thought_key(ctrl_t()));
        assert!(
            state
                .thought_modal
                .as_ref()
                .unwrap()
                .lines
                .join("\n")
                .contains("first complete")
        );
    }

    #[test]
    fn live_thought_wrapped_tail_preserves_latest_unicode_text() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta(
            "one",
            &format!("{}\nlatest ข้อความ 🙂", "older words ".repeat(30)),
            std::time::Duration::ZERO,
        );
        let state = state.lock().unwrap();
        let screen = thought_screen(&state, 40, 24).join("\n");
        assert!(screen.contains("latest ข้อความ 🙂"));
        assert!(!screen.contains("older words older words older words older words"));
    }

    #[test]
    fn live_thought_preview_keeps_only_latest_rows_without_appending_transcript() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        assert!(
            !thought_screen(&state.lock().unwrap(), 80, 24)
                .join("\n")
                .contains("Thinking ·")
        );
        for index in 1..=20 {
            handle.append_thought_delta(
                "one",
                &format!("reasoning row {index:02}\n"),
                std::time::Duration::from_millis(1200),
            );
        }
        let state = state.lock().unwrap();
        let rows = thought_screen(&state, 80, 24);
        let title = rows
            .iter()
            .position(|line| line.contains("Thinking · 1.2s"))
            .expect("live thinking header");
        let latest = rows
            .iter()
            .position(|line| line.contains("reasoning row 20"))
            .expect("latest streamed row");
        assert!(latest > title && latest - title < 6);
        assert!(!rows.join("\n").contains("reasoning row 01"));
        assert!(state.transcript.is_empty());
    }

    #[test]
    fn live_thought_finishes_as_one_collapsed_summary_and_preserves_full_text() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "partial", std::time::Duration::from_millis(1200));
        handle.finish_thought(
            Some("one"),
            "complete reasoning",
            std::time::Duration::from_millis(1900),
        );
        let mut state = state.lock().unwrap();
        let rendered = thought_screen(&state, 80, 24).join("\n");
        assert!(rendered.contains("Thought · 1.9s (Ctrl+T to view)"));
        assert!(!rendered.contains("Thinking ·"));
        assert!(!rendered.contains("complete reasoning"));
        assert_eq!(state.transcript.len(), 1);
        assert!(state.handle_thought_key(ctrl_t()));
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["complete reasoning"]
        );
    }

    #[test]
    fn live_thought_modal_refreshes_deltas_and_closes_without_cancelling_agent() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta("one", "first", std::time::Duration::ZERO);
        assert!(state.lock().unwrap().handle_thought_key(ctrl_t()));
        handle.append_thought_delta("one", " second", std::time::Duration::from_secs(1));
        let mut state = state.lock().unwrap();
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["first second"]
        );
        assert!(state.handle_thought_key(make_key(KeyCode::Esc)));
        assert!(state.thought_modal.is_none());
        assert!(!handle.take_interrupted());
    }

    #[test]
    fn live_thought_interruption_preserves_partial_text_and_collapses_preview() {
        let state = Arc::new(Mutex::new(ChatViewState::default()));
        let handle = ChatTui::handle(state.clone());
        handle.append_thought_delta(
            "one",
            "unfinished reasoning",
            std::time::Duration::from_millis(1900),
        );
        let mut state = state.lock().unwrap();
        state.finish_live_thought();
        assert!(
            !thought_screen(&state, 80, 24)
                .join("\n")
                .contains("Thinking ·")
        );
        assert!(state.handle_thought_key(ctrl_t()));
        assert_eq!(
            state.thought_modal.as_ref().unwrap().lines[1..],
            vec!["unfinished reasoning"]
        );
        assert_eq!(state.transcript.len(), 1);
    }

    #[test]
    fn update_dialog_requires_explicit_selection_before_installing() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_update("1.0.0", "2.0.0", tx);
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(1))
        );
        dialog.handle_key(make_key(KeyCode::Up));
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(0))
        );
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Esc)),
            Some(DialogAnswer::Cancel)
        );
    }

    #[test]
    fn update_dialog_can_skip_version_and_cannot_filter_away_skip() {
        let (tx, _rx) = mpsc::channel();
        let mut dialog = DialogState::new_update("1.0.0", "2.0.0", tx);
        dialog.handle_key(make_key(KeyCode::Char('x')));
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Esc)),
            Some(DialogAnswer::Cancel)
        );
        assert_eq!(dialog.handle_key(make_key(KeyCode::Char('1'))), None);
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(0))
        );
        dialog.handle_key(make_key(KeyCode::Down));
        dialog.handle_key(make_key(KeyCode::Down));
        assert_eq!(
            dialog.handle_key(make_key(KeyCode::Enter)),
            Some(DialogAnswer::Choice(2))
        );
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

#[cfg(test)]
mod collapsed_paste_tests {
    use super::*;

    #[test]
    fn f2_toggles_an_edited_paste_without_folding_surrounding_instructions() {
        let mut state = ChatViewState::default();
        state.set_draft("before ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.replace_composer(state.cursor..state.cursor, " after");
        let f2 = event::KeyEvent::new(event::KeyCode::F(2), event::KeyModifiers::NONE);
        state.edit_composer_key(f2);
        state.edit_composer_key(event::KeyEvent::new(
            event::KeyCode::Char('ก'),
            event::KeyModifiers::NONE,
        ));
        state.edit_composer_key(f2);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "before [Pasted Content 14 chars] after"
        );
        state.edit_composer_key(f2);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "before กa\nb\nc\nd\ne\nf\ng after"
        );
        state.edit_composer_key(f2);
        assert_eq!(state.take_prompt(), "before กa\nb\nc\nd\ne\nf\ng after");
        state.set_draft("fresh".into());
        state.edit_composer_key(f2);
        assert_eq!(state.take_prompt(), "fresh");
    }

    #[test]
    fn f2_preserves_nested_pastes_and_edits_before_the_open_block() {
        let mut state = ChatViewState::default();
        state.set_draft("before ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        let f2 = event::KeyEvent::new(event::KeyCode::F(2), event::KeyModifiers::NONE);
        state.edit_composer_key(f2);
        state.replace_composer(0..0, "prefix ");
        state.cursor = 15;
        state.insert_paste("1\n2\n3\n4\n5\n6\n7");
        state.edit_composer_key(f2);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "prefix before [Pasted Content 26 chars]"
        );
        assert_eq!(
            state.take_prompt(),
            "prefix before a1\n2\n3\n4\n5\n6\n7\nb\nc\nd\ne\nf\ng"
        );
    }

    #[test]
    fn deleting_an_open_paste_drops_its_toggle_state() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        let f2 = event::KeyEvent::new(event::KeyCode::F(2), event::KeyModifiers::NONE);
        state.edit_composer_key(f2);
        state.replace_composer(0..13, "");
        state.edit_composer_key(f2);
        assert!(state.composer.is_empty());
    }

    #[test]
    fn seven_pasted_lines_render_as_a_badge_instead_of_filling_the_composer() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(80, 32)).unwrap();
        let mut composer = None;
        terminal
            .draw(|frame| composer = state.render(frame).composer)
            .unwrap();
        let area = composer.unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = (area.y..area.bottom())
            .flat_map(|y| (area.x..area.right()).map(move |x| buffer[(x, y)].symbol()))
            .collect();
        assert!(rendered.contains("[Pasted Content 13 chars]"), "{rendered}");
        assert!(rendered.contains("F2"));
        assert_eq!(area.height, 3, "A collapsed paste should use one input row");
    }

    #[test]
    fn six_pasted_lines_stay_editable_inline() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\r\nb\rc\nd\ne\nf");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "a\nb\nc\nd\ne\nf"
        );
    }

    #[test]
    fn autocomplete_keeps_the_adjacent_collapsed_paste_as_separate_content() {
        let mut state = ChatViewState::default();
        state.set_draft("/hel".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.cursor = 4;
        state.slash_selected = state
            .suggestions()
            .iter()
            .position(|(token, _)| token == "/help")
            .unwrap();
        assert!(state.apply_selected_suggestion());
        assert_eq!(state.take_prompt(), "/help a\nb\nc\nd\ne\nf\ng");
    }

    #[test]
    fn clipboard_trailing_newlines_survive_both_inline_and_collapsed_submit() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\n");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "a\nb\nc\nd\ne\nf\n"
        );
        assert_eq!(state.take_prompt(), "a\nb\nc\nd\ne\nf\n");
        state.insert_paste("a\nb\nc\nd\ne\nf\ng\n\n");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "[Pasted Content 15 chars]"
        );
        assert_eq!(state.take_prompt(), "a\nb\nc\nd\ne\nf\ng\n\n");
    }

    #[test]
    fn submit_expands_each_paste_without_replacing_literal_badge_text() {
        let mut state = ChatViewState::default();
        state.set_draft("literal [Pasted Content 13 chars]: ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.replace_composer(state.cursor..state.cursor, " then ");
        state.insert_paste("1\n2\n3\n4\n5\n6\n7");
        assert_eq!(
            state.take_prompt(),
            "literal [Pasted Content 13 chars]: a\nb\nc\nd\ne\nf\ng then 1\n2\n3\n4\n5\n6\n7"
        );
        assert!(state.composer.is_empty());
        state.set_draft("fresh".into());
        assert_eq!(state.take_prompt(), "fresh");
    }

    #[test]
    fn opening_a_paste_reveals_its_first_line_and_edits_reach_submit() {
        let mut state = ChatViewState::default();
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        assert!(state.open_paste());
        assert_eq!(state.cursor, 0);
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "a\nb\nc\nd\ne\nf\ng"
        );
        state.replace_composer(1..1, " edited");
        assert_eq!(state.take_prompt(), "a edited\nb\nc\nd\ne\nf\ng");
    }

    #[test]
    fn deleting_part_of_a_badge_removes_only_that_paste() {
        let mut state = ChatViewState::default();
        state.set_draft("before ".into());
        state.insert_paste("a\nb\nc\nd\ne\nf\ng");
        state.replace_composer(state.cursor..state.cursor, " after");
        state.replace_composer(8..9, "");
        assert_eq!(state.take_prompt(), "before  after");
    }

    #[test]
    fn unicode_character_count_and_history_restore_keep_the_real_content() {
        let mut state = ChatViewState {
            history: vec!["older".into()],
            ..Default::default()
        };
        state.insert_paste("ก\rข\rค\rง\rจ\rฉ\rช");
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "[Pasted Content 13 chars]"
        );
        state.history_previous();
        state.history_next();
        assert_eq!(state.composer_text(), "ก\nข\nค\nง\nจ\nฉ\nช");
    }
}

#[cfg(test)]
mod pasted_prompt_render_tests {
    use super::*;
    use ratatui::backend::TestBackend;

    const PROMPT: &str = "Make me a Mint mod called token-weather.\r\rWhat it should show:\r- ☀ Clear (yellow), ☁ Cloudy (cyan), ☂ Showers (blue)\r- 134.4k / 200k\r- ▁▂▃▄▅▆▇█\r- ▲ +98.3k last turn\r\rIt should update after every turn.";

    #[test]
    fn pasted_prompt_draft_renders_line_breaks_without_carriage_return_cells() {
        let mut state = ChatViewState::default();
        state.set_draft(PROMPT.into());
        state.cursor = 0;
        let mut terminal = Terminal::new(TestBackend::new(100, 40)).unwrap();
        let mut composer = None;
        terminal
            .draw(|frame| composer = state.render(frame).composer)
            .unwrap();
        let area = composer.unwrap();
        let buffer = terminal.backend().buffer();
        let rows: Vec<String> = (area.y + 1..area.bottom() - 1)
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buffer[(x, y)].symbol())
                    .collect()
            })
            .collect();
        assert!(
            rows.iter().all(|row| !row.contains('\r')),
            "CR must become a line break, not a terminal cursor movement: {rows:?}"
        );
        assert!(rows[0].contains("Make me a Mint mod called token-weather."));
        assert!(rows[2].contains("What it should show:"), "{rows:?}");
        assert!(rows[3].contains("☀ Clear"));
    }

    #[test]
    fn sent_pasted_prompt_preserves_all_lines_and_weather_symbols() {
        let mut state = ChatViewState::default();
        state.push_user(PROMPT.into());
        state.push_assistant("Reading the authoring guide.");
        let mut terminal = Terminal::new(TestBackend::new(80, 40)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let rendered: String = buffer.content.iter().map(|cell| cell.symbol()).collect();
        assert!(
            !rendered.contains('\r'),
            "Sent user text contains terminal cursor movements"
        );
        for label in [
            "token-weather.",
            "What it should show:",
            "☀ Clear",
            "134.4k / 200k",
            "▁▂▃▄▅▆▇█",
            "▲ +98.3k last turn",
            "It should update after every turn.",
        ] {
            assert!(rendered.contains(label), "Missing sent text: {label}");
        }
        assert_eq!(state.plain_transcript_lines()[2], "What it should show:");
    }

    #[test]
    fn reused_prompt_history_normalizes_crlf_and_lone_cr() {
        let mut state = ChatViewState {
            history: vec!["first\r\nsecond\rthird".into()],
            ..Default::default()
        };
        state.history_previous();
        assert_eq!(
            state.composer.iter().collect::<String>(),
            "first\nsecond\nthird"
        );
        assert_eq!(state.cursor, 18);
    }

    #[test]
    fn end_returns_scrolled_transcript_to_bottom_before_moving_draft_cursor() {
        use event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ChatViewState::default();
        state.set_draft("first line\nsecond line".into());
        state.cursor = 2;
        state.scroll_from_bottom = 12;
        let end = KeyEvent::new(KeyCode::End, KeyModifiers::NONE);
        assert!(state.navigate_composer(end, 20));
        assert_eq!(state.scroll_from_bottom, 0);
        assert_eq!(state.cursor, 2);
        assert_eq!(state.composer_text(), "first line\nsecond line");
        assert!(state.navigate_composer(end, 20));
        assert_eq!(state.cursor, 10);
    }

    #[test]
    fn composer_navigation_uses_visual_rows_for_wrapped_and_wide_text() {
        use event::{KeyCode, KeyEvent, KeyModifiers};
        let key = |code| KeyEvent::new(code, KeyModifiers::NONE);
        let mut state = ChatViewState::default();
        state.set_draft("0123456789abcdefghijABCDEFGHIJKLMNOPQRSTUVWXYZ".into());
        assert!(state.navigate_composer(key(KeyCode::Up), 20));
        assert_eq!(state.cursor, 26);
        assert!(state.navigate_composer(key(KeyCode::Home), 20));
        assert_eq!(state.cursor, 20);
        assert!(state.navigate_composer(key(KeyCode::Down), 20));
        assert_eq!(state.cursor, 40);
        assert!(state.navigate_composer(key(KeyCode::End), 20));
        assert_eq!(state.cursor, 46);
        state.set_draft("界界\naaaa".into());
        assert!(state.navigate_composer(key(KeyCode::Up), 20));
        assert_eq!(state.cursor, 2);
        assert!(state.navigate_composer(key(KeyCode::Down), 20));
        assert_eq!(state.cursor, 7);
    }

    #[test]
    fn long_pasted_prompt_can_be_inspected_from_first_to_last_line() {
        use event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ChatViewState::default();
        state.insert_paste(
            &(1..=12)
                .map(|i| format!("Prompt line {i:02}"))
                .collect::<Vec<_>>()
                .join("\r"),
        );
        assert!(state.edit_composer_key(KeyEvent::new(KeyCode::F(2), KeyModifiers::NONE)));
        assert_eq!(state.cursor, 0);
        state.cursor = state.composer.len();
        let mut terminal = Terminal::new(TestBackend::new(80, 32)).unwrap();
        let mut composer = None;
        terminal
            .draw(|frame| composer = state.render(frame).composer)
            .unwrap();
        let area = composer.unwrap();
        let visible = |buffer: &Buffer| -> String {
            (area.y + 1..area.bottom() - 1)
                .flat_map(|y| (area.x..area.right()).map(move |x| buffer[(x, y)].symbol()))
                .collect()
        };
        assert!(visible(terminal.backend().buffer()).contains("Prompt line 12"));
        for _ in 0..11 {
            assert!(state.navigate_composer(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE), 72));
        }
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        assert!(visible(terminal.backend().buffer()).contains("Prompt line 01"));
        assert!(visible(terminal.backend().buffer()).contains("Prompt line 06"));
    }
}

#[cfg(test)]
mod mod_surface_tests {
    use super::*;
    #[test]
    fn focused_mod_input_displays_the_current_value_before_editing() {
        let mut state = ChatViewState::default();
        state.mods_open = true;
        state.mods = vec![
            serde_json::json!({"id":"draft","enabled":true,"snapshot":{"ui":{"side_panel":{"type":"input","label":"Draft","action":"save","value":"existing"}}}}),
        ];
        let backend = ratatui::backend::TestBackend::new(120, 35);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let screen = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(screen.contains("› Draft: existing"));
    }
    #[test]
    fn terminal_shows_reload_diagnostics_while_the_old_mod_stays_active() {
        let mut state = ChatViewState::default();
        state.mods_open = true;
        state.mods = vec![
            serde_json::json!({"id":"counter","name":"Counter","enabled":true,"reloadError":"Invalid update","snapshot":{"ui":{"side_panel":{"type":"text","text":"Working revision"}}}}),
        ];
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(120, 35)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>();
        assert!(text.contains("Reload: Invalid update"));
        assert!(text.contains("Working revision"));
    }
    #[test]
    fn terminal_viewport_tracks_docked_panel_width_and_overlay_fallback() {
        let mut state = ChatViewState::default();
        state.mods_open = true;
        let wide = state.mod_viewport(Rect::new(0, 0, 120, 35));
        assert_eq!(wide.columns, Some(78));
        assert_eq!(wide.surface.as_deref(), Some("terminal"));
        let narrow = state.mod_viewport(Rect::new(0, 0, 80, 35));
        assert_eq!(narrow.columns, Some(76));
        state.mods_open = false;
        assert_eq!(
            state.mod_viewport(Rect::new(0, 0, 120, 35)).columns,
            Some(116)
        );
    }
    #[test]
    fn terminal_weather_band_keeps_color_and_metrics_on_one_row() {
        let mut state = ChatViewState::default();
        state.mods = vec![
            serde_json::json!({"id":"weather","name":"Weather","enabled":true,"snapshot":{"ui":{"above_composer":{"type":"stack","direction":"row","children":[{"type":"text","text":"Storm","color":"magenta"},{"type":"text","text":"80% 160k / 200k"}]}}}}),
        ];
        let mut terminal = Terminal::new(ratatui::backend::TestBackend::new(120, 35)).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let mut rows = Vec::new();
        for y in 0..35 {
            rows.push(
                (0..120)
                    .map(|x| buffer[(x, y)].symbol())
                    .collect::<String>(),
            );
        }
        let row = rows
            .iter()
            .position(|r| r.contains("Storm 80% 160k / 200k"))
            .expect("Weather and counts share one row");
        let x = rows[row].find("Storm").unwrap();
        assert_eq!(
            buffer[(x as u16, row as u16)].fg,
            ratatui::style::Color::Magenta
        );
    }
    #[test]
    fn terminal_renders_mod_band_and_side_panel() {
        let mut state = ChatViewState::default();
        state.mods = vec![
            serde_json::json!({"id":"meter","name":"Meter","enabled":true,"snapshot":{"ui":{"above_composer":{"type":"text","text":"123 tokens"},"side_panel":{"type":"table","columns":["Task"],"rows":[["Write tests"]]}}}}),
        ];
        state.mods_open = true;
        let backend = ratatui::backend::TestBackend::new(120, 35);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let buffer = terminal.backend().buffer();
        let text = buffer
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("123 tokens"));
        assert!(text.contains("Write tests"));
    }
}

#[cfg(test)]
mod swarm_panel_tests {
    use super::*;

    #[test]
    fn swarm_panel_is_hidden_when_empty() {
        let state = ChatViewState::default();
        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(!text.contains("Subagents"));
    }

    #[test]
    fn swarm_panel_renders_running_and_completed_subagents() {
        let mut state = ChatViewState::default();
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching for payment webhook endpoints...".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });
        state.swarm_agents.push(SwarmAgentUiState {
            label: "config".to_string(),
            profile: "explorer".to_string(),
            task: "Checking config files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Success,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: Some(3000),
            summary_preview: Some("Checked stripe API keys in .env".to_string()),
        });

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Running Subagents"));
        assert!(text.contains("[recon]"));
        assert!(text.contains("Searching for payment webhook endpoints..."));
        assert!(text.contains("[config]"));
        assert!(text.contains("Checked stripe API keys in .env"));
        assert!(text.contains("done 3s"));
    }

    #[test]
    fn swarm_panel_handles_more_than_5_subagents_with_overflow() {
        let mut state = ChatViewState::default();
        for i in 1..=7 {
            state.swarm_agents.push(SwarmAgentUiState {
                label: format!("agent-{}", i),
                profile: "explorer".to_string(),
                task: format!("Task description for agent {}", i),
                current_action: None,
                status: SwarmUiStatus::Running,
                started_at: std::time::Instant::now(),
                finished_elapsed_ms: None,
                summary_preview: None,
            });
        }

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("+3 more subagents..."));
    }

    #[test]
    fn swarm_panel_compact_mode_on_short_screen() {
        let mut state = ChatViewState::default();
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });

        let backend = ratatui::backend::TestBackend::new(80, 20);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Running 1 subagent"));
    }

    #[test]
    fn swarm_agents_cleared_on_user_input_and_clear() {
        let mut state = ChatViewState::default();
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });
        assert_eq!(state.swarm_agents.len(), 1);

        state.push_user("hello".to_string());
        assert!(state.swarm_agents.is_empty());

        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Searching files".to_string(),
            current_action: None,
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });
        state.clear_transcript();
        assert!(state.swarm_agents.is_empty());
    }

    #[test]
    fn record_tool_progress_updates_swarm_state() {
        let mut state = ChatViewState::default();
        let start_event = mint_core::AgentProgress::SubagentStarted {
            subagent: "worker".to_string(),
            profile: "explorer".to_string(),
            task: "Investigate database queries".to_string(),
        };
        state.record_tool_progress(&start_event);
        assert_eq!(state.swarm_agents.len(), 1);
        assert_eq!(state.swarm_agents[0].label, "worker");
        assert_eq!(state.swarm_agents[0].status, SwarmUiStatus::Running);

        let tool_start = mint_core::AgentProgress::ToolStart {
            call_id: Some("call-1".to_string()),
            action: "run_shell".to_string(),
            input: serde_json::json!({"command": "cargo test"}),
            subagent: Some("worker".to_string()),
        };
        state.record_tool_progress(&tool_start);
        assert_eq!(
            state.swarm_agents[0].current_action.as_deref(),
            Some("Running `cargo test`")
        );
        // Subagent tools must NOT be pushed to the main transcript or tools list!
        assert!(
            state.transcript.is_empty(),
            "transcript must stay clean without subagent tool spam"
        );
        assert!(
            state.tools.is_empty(),
            "tools list must not include subagent micro-tools"
        );

        let tool_end = mint_core::AgentProgress::ToolEnd {
            call_id: Some("call-1".to_string()),
            action: "run_shell".to_string(),
            input: serde_json::json!({"command": "cargo test"}),
            result: "test result ok".to_string(),
            status: Some(mint_core::integrations::mcp_result::ToolStatus::Success),
            subagent: Some("worker".to_string()),
            elapsed_ms: Some(120),
        };
        state.record_tool_progress(&tool_end);
        assert!(state.transcript.is_empty());
        assert!(state.tools.is_empty());

        let finish_event = mint_core::AgentProgress::SubagentFinished {
            subagent: "worker".to_string(),
            status: "completed".to_string(),
            elapsed_ms: 4500,
            steps: 3,
            summary_preview: "All queries optimized".to_string(),
        };
        state.record_tool_progress(&finish_event);
        assert_eq!(state.swarm_agents[0].status, SwarmUiStatus::Success);
        assert_eq!(state.swarm_agents[0].finished_elapsed_ms, Some(4500));
        assert_eq!(
            state.swarm_agents[0].summary_preview.as_deref(),
            Some("All queries optimized")
        );
        assert!(state.transcript.is_empty());
    }

    #[test]
    fn swarm_panel_renders_smoothly_in_plan_mode() {
        let mut state = ChatViewState::default();
        state.plan_mode = true;
        state.swarm_agents.push(SwarmAgentUiState {
            label: "recon".to_string(),
            profile: "explorer".to_string(),
            task: "Investigating schema for migration".to_string(),
            current_action: Some("Reading `schema.rs`".to_string()),
            status: SwarmUiStatus::Running,
            started_at: std::time::Instant::now(),
            finished_elapsed_ms: None,
            summary_preview: None,
        });

        let backend = ratatui::backend::TestBackend::new(100, 30);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|frame| {
                state.render(frame);
            })
            .unwrap();
        let text = terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect::<String>();
        assert!(text.contains("Running Subagents"));
        assert!(text.contains("[recon]"));
        assert!(text.contains("Reading `schema.rs`"));
        assert!(text.contains("[Plan]"));
    }

    #[test]
    fn tool_viewer_smart_jumps_to_web_search_when_sources_present() {
        use crate::interactive::tool_activity::ToolActivity;
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let mut state = ChatViewState::default();
        // Tool 0: web_search
        state.tools.push(ToolActivity {
            entry: 0,
            call_id: Some("call_web".into()),
            action: "web_search".into(),
            input: serde_json::json!({"query": "cosplay 4khd"}),
            subagent: None,
            started: std::time::Instant::now(),
            elapsed: Some(std::time::Duration::from_millis(1200)),
            result: Some("1. 4KHD Cosplay\n   URL: https://4khd.com\n   Preview text".into()),
            status: Some(mint_core::integrations::mcp_result::ToolStatus::Success),
        });
        // Tool 1: run_shell (ran afterwards)
        state.tools.push(ToolActivity {
            entry: 1,
            call_id: Some("call_shell".into()),
            action: "run_shell".into(),
            input: serde_json::json!({"command": "echo done"}),
            subagent: None,
            started: std::time::Instant::now(),
            elapsed: Some(std::time::Duration::from_millis(500)),
            result: Some("exit: 0\nstdout:\ndone\nstderr:".into()),
            status: Some(mint_core::integrations::mcp_result::ToolStatus::Success),
        });

        let make_key = |code: KeyCode, modifiers: KeyModifiers| KeyEvent::new(code, modifiers);

        // Without Sources notice, Ctrl+O opens the latest tool (index 1: run_shell)
        let key = make_key(KeyCode::Char('o'), KeyModifiers::CONTROL);
        assert!(state.handle_tool_key(key));
        assert_eq!(state.tool_viewer, Some(1));
        state.handle_tool_key(make_key(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(state.tool_viewer, None);

        // When Sources notice is in recent transcript, Ctrl+O smart jumps to web_search (index 0)
        state.push_notice("Sources:\n1. 4KHD Cosplay\n   https://4khd.com\n  … [+10 more sources] · Ctrl+O view web_search");
        state.push_notice("─ Worked for 10s • test ──────────────────────────────");

        assert!(state.handle_tool_key(key));
        assert_eq!(state.tool_viewer, Some(0));
        assert_eq!(state.tools[state.tool_viewer.unwrap()].action, "web_search");
    }

    #[test]
    fn format_tui_web_sources_renders_with_muted_and_blue_spans_in_transcript_entry() {
        let sources: Vec<(String, String)> = (1..=10)
            .map(|i| (format!("Title {i}"), format!("https://example.com/{i}")))
            .collect();
        let formatted = crate::agent::format_tui_web_sources(&sources, 5);
        let entry = TranscriptEntry::new(TranscriptRole::Notice, formatted);

        let more_line = entry
            .rendered_lines
            .iter()
            .find(|line| {
                line.spans
                    .iter()
                    .any(|s| s.content.contains("more sources] · "))
            })
            .expect("more sources line should be present");

        let muted_span = more_line
            .spans
            .iter()
            .find(|s| s.content.contains("more sources] · "))
            .expect("muted span found");
        assert_eq!(muted_span.style.fg, Some(crate::terminal_theme::MUTED));

        let blue_span = more_line
            .spans
            .iter()
            .find(|s| s.content.contains("Ctrl+O view web_search"))
            .expect("blue span found");
        assert_eq!(blue_span.style.fg, Some(crate::terminal_theme::BLUE));
    }
}

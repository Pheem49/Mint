//! Width-reactive transcript entries, paragraph layout, and user message highlighting.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TranscriptRole {
    User,
    Assistant,
    Notice,
    Command,
    ReleaseNotes,
    Plan,
    #[allow(dead_code)]
    System,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TranscriptEntry {
    pub role: TranscriptRole,
    pub text: String,
    pub rendered_lines: Vec<Line<'static>>,
    pub plain_lines: Vec<String>,
    pub rendered_width: u16,
}

impl TranscriptEntry {
    pub fn new(role: TranscriptRole, text: impl Into<String>) -> Self {
        let (default_width, _) = crate::markdown::terminal_size_or_default();
        Self::new_with_width(role, text, default_width)
    }

    pub fn new_with_width(role: TranscriptRole, text: impl Into<String>, width: u16) -> Self {
        let text = text.into();
        let (rendered_lines, plain_lines) =
            Self::render_content_with_width(role, &text, width as usize);
        Self {
            role,
            text,
            rendered_lines,
            plain_lines,
            rendered_width: width,
        }
    }

    pub fn lines_for_width(&self, width: u16) -> std::borrow::Cow<'_, [Line<'static>]> {
        if (self.role != TranscriptRole::Assistant
            && self.role != TranscriptRole::ReleaseNotes
            && self.role != TranscriptRole::Plan)
            || width == 0
            || width == self.rendered_width
            || self.text.is_empty()
        {
            return std::borrow::Cow::Borrowed(&self.rendered_lines);
        }
        if self.role == TranscriptRole::ReleaseNotes || self.role == TranscriptRole::Plan {
            let (rendered, _) =
                Self::render_content_with_width(self.role, &self.text, width as usize);
            return std::borrow::Cow::Owned(rendered);
        }
        let needs_width_reflow = self.text.contains('|')
            || self.text.contains("```")
            || self.text.contains("---")
            || self.text.contains("***")
            || self.text.contains("___")
            || self.text.contains("ui-");
        if !needs_width_reflow {
            return std::borrow::Cow::Borrowed(&self.rendered_lines);
        }
        let (rendered, _) = Self::render_content_with_width(self.role, &self.text, width as usize);
        std::borrow::Cow::Owned(rendered)
    }

    fn render_content_with_width(
        role: TranscriptRole,
        text: &str,
        width: usize,
    ) -> (Vec<Line<'static>>, Vec<String>) {
        use ansi_to_tui::IntoText;
        if text.is_empty() {
            return (Vec::new(), Vec::new());
        }
        match role {
            TranscriptRole::Assistant => {
                let sanitized = crate::agent::sanitize_latex(text);
                crate::agent::render_markdown_to_lines(&sanitized, width)
            }
            TranscriptRole::User => {
                let normalized = normalize_prompt_newlines(text);
                let rendered_lines = normalized
                    .as_str()
                    .into_text()
                    .map(|parsed| parsed.lines)
                    .unwrap_or_else(|_| {
                        normalized
                            .lines()
                            .map(|line| Line::raw(line.to_owned()))
                            .collect()
                    });
                let plain_lines = rendered_lines
                    .iter()
                    .map(|line| {
                        line.spans
                            .iter()
                            .map(|span| span.content.as_ref())
                            .collect()
                    })
                    .collect();
                (rendered_lines, plain_lines)
            }
            TranscriptRole::Notice | TranscriptRole::Command | TranscriptRole::System => {
                let rendered_lines =
                    text.into_text()
                        .map(|parsed| parsed.lines)
                        .unwrap_or_else(|_| {
                            text.lines()
                                .map(|line| Line::raw(line.to_owned()))
                                .collect()
                        });
                let plain_lines = rendered_lines
                    .iter()
                    .map(|line| {
                        line.spans
                            .iter()
                            .map(|span| span.content.as_ref())
                            .collect()
                    })
                    .collect();
                (rendered_lines, plain_lines)
            }
            TranscriptRole::ReleaseNotes => render_release_notes_card_to_lines(text, width as u16),
            TranscriptRole::Plan => render_plan_card_to_lines(text, width as u16),
        }
    }
}

pub(super) fn render_release_notes_card_to_lines(
    text: &str,
    width: u16,
) -> (Vec<Line<'static>>, Vec<String>) {
    use ratatui::widgets::Widget;

    if text.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut version = String::new();
    let mut sections_count = 0;
    let mut inner_lines: Vec<Line<'static>> = Vec::new();
    let mut plain_lines: Vec<String> = Vec::new();

    let padding = if width >= 40 { 1u16 } else { 0u16 };
    let card_width = width.saturating_sub(padding * 2 + 2).max(20);

    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if !inner_lines.is_empty() {
                inner_lines.push(Line::default());
                plain_lines.push(String::new());
            }
            continue;
        }

        if trimmed.starts_with("# ") {
            if let Some(pos) = trimmed.rfind('v') {
                version = trimmed[pos..].trim().to_string();
            }
            continue;
        }

        if trimmed.starts_with("## ") {
            sections_count += 1;
            let section_title = trimmed[3..].trim();
            if !inner_lines.is_empty() && !inner_lines.last().map_or(false, |l| l.spans.is_empty())
            {
                inner_lines.push(Line::default());
                plain_lines.push(String::new());
            }
            inner_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "● ",
                    Style::default()
                        .fg(crate::terminal_theme::CARD_BORDER)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    section_title.to_string(),
                    Style::default()
                        .fg(crate::terminal_theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            plain_lines.push(format!("● {}", section_title));
            continue;
        }

        let indent = line.len() - line.trim_start().len();
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let bullet_content = trimmed[2..].trim();
            let (prefix_indent, bullet_color) = if indent >= 2 {
                ("      • ", crate::terminal_theme::MUTED)
            } else {
                ("    • ", crate::terminal_theme::ACCENT)
            };

            let spans = if bullet_content.starts_with("**") {
                if let Some(end_bold) = bullet_content[2..].find("**") {
                    let title = &bullet_content[2..2 + end_bold];
                    let rest = &bullet_content[2 + end_bold + 2..];
                    vec![
                        Span::styled(prefix_indent, Style::default().fg(bullet_color)),
                        Span::styled(
                            title.to_string(),
                            Style::default()
                                .fg(crate::terminal_theme::TEXT)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            rest.to_string(),
                            Style::default().fg(crate::terminal_theme::TEXT),
                        ),
                    ]
                } else {
                    vec![
                        Span::styled(prefix_indent, Style::default().fg(bullet_color)),
                        Span::styled(
                            bullet_content.to_string(),
                            Style::default().fg(crate::terminal_theme::TEXT),
                        ),
                    ]
                }
            } else {
                vec![
                    Span::styled(prefix_indent, Style::default().fg(bullet_color)),
                    Span::styled(
                        bullet_content.to_string(),
                        Style::default().fg(crate::terminal_theme::TEXT),
                    ),
                ]
            };
            let plain_str = format!("{}{}", prefix_indent, bullet_content.replace("**", ""));
            plain_lines.push(plain_str);
            inner_lines.push(Line::from(spans));
            continue;
        }

        inner_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::raw(trimmed.to_string()),
        ]));
        plain_lines.push(trimmed.to_string());
    }

    if version.is_empty() {
        version = format!("v{}", env!("CARGO_PKG_VERSION"));
    }

    plain_lines.insert(0, format!("Release Notes · {}", version));

    let bg_color = crate::terminal_theme::CARD_BACKGROUND;
    let bg_style = Style::default().bg(bg_color);

    let title_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "🚀 Release Notes ",
            Style::default()
                .fg(crate::terminal_theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("· {} ", version),
            Style::default()
                .fg(crate::terminal_theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let footer_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            format!("[ {} Highlights · Latest ]", sections_count),
            Style::default().fg(crate::terminal_theme::MUTED),
        ),
    ]);

    let mut card_lines = Vec::with_capacity(inner_lines.len() + 6);
    card_lines.push(Line::default()); // top padding
    card_lines.push(title_line);
    card_lines.push(Line::default());
    card_lines.extend(inner_lines);
    card_lines.push(Line::default());
    card_lines.push(footer_line);
    card_lines.push(Line::default()); // bottom padding

    let paragraph = Paragraph::new(card_lines)
        .style(bg_style)
        .wrap(Wrap { trim: false });
    let card_height = paragraph
        .line_count(card_width)
        .max(1)
        .min(u16::MAX as usize) as u16;

    let area = Rect::new(0, 0, card_width, card_height);
    let mut buffer = Buffer::empty(area);
    for cell in &mut buffer.content {
        cell.set_bg(bg_color);
    }
    let widget = paragraph.block(Block::default().style(bg_style));
    widget.render(area, &mut buffer);

    let rendered_lines: Vec<Line<'static>> = (0..card_height)
        .map(|y| {
            let mut spans = Vec::new();
            if padding > 0 {
                spans.push(Span::raw(" ".repeat(padding as usize)));
            }
            let mut x = 0;
            while x < card_width {
                let cell = &buffer[(x, y)];
                let mut style = cell.style();
                style.bg = Some(bg_color);
                let sym = if cell.symbol() == " " {
                    "\u{00A0}"
                } else {
                    cell.symbol()
                };
                spans.push(Span::styled(sym.to_owned(), style));
                x += UnicodeWidthStr::width(cell.symbol()).max(1) as u16;
            }
            Line::from(spans)
        })
        .collect();

    (rendered_lines, plain_lines)
}

pub(super) fn render_plan_card_to_lines(
    text: &str,
    width: u16,
) -> (Vec<Line<'static>>, Vec<String>) {
    use ratatui::widgets::Widget;

    if text.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let mut title = String::new();
    let mut inner_lines: Vec<Line<'static>> = Vec::new();
    let mut plain_lines: Vec<String> = Vec::new();

    let padding = if width >= 40 { 1u16 } else { 0u16 };
    let card_width = width.saturating_sub(padding * 2 + 2).max(20);

    let mut in_code_block = false;

    for line in text.lines() {
        let trimmed = line.trim();

        if trimmed.starts_with("```") {
            in_code_block = !in_code_block;
            inner_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    trimmed.to_string(),
                    Style::default().fg(crate::terminal_theme::MUTED),
                ),
            ]));
            plain_lines.push(trimmed.to_string());
            continue;
        }

        if in_code_block {
            inner_lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(
                    line.to_string(),
                    Style::default().fg(crate::terminal_theme::TEXT),
                ),
            ]));
            plain_lines.push(format!("    {}", line));
            continue;
        }

        if trimmed.is_empty() {
            if !inner_lines.is_empty() && !inner_lines.last().map_or(false, |l| l.spans.is_empty())
            {
                inner_lines.push(Line::default());
                plain_lines.push(String::new());
            }
            continue;
        }

        // Title at level 1: # Title
        if trimmed.starts_with("# ") && title.is_empty() {
            title = trimmed[2..].trim().to_string();
            continue;
        }

        // Section header: ## or ### or # (if title already set)
        if trimmed.starts_with("### ") || trimmed.starts_with("## ") || trimmed.starts_with("# ") {
            let hash_count = trimmed.chars().take_while(|&c| c == '#').count();
            let section_title = trimmed[hash_count..].trim();
            if !inner_lines.is_empty() && !inner_lines.last().map_or(false, |l| l.spans.is_empty())
            {
                inner_lines.push(Line::default());
                plain_lines.push(String::new());
            }
            inner_lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    "● ",
                    Style::default()
                        .fg(crate::terminal_theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    section_title.to_string(),
                    Style::default()
                        .fg(crate::terminal_theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            plain_lines.push(format!("● {}", section_title));
            continue;
        }

        // Checklists: - [ ] or - [x] or * [ ] or * [x]
        if (trimmed.starts_with("- [ ] ") || trimmed.starts_with("* [ ] "))
            || (trimmed.starts_with("- [x] ")
                || trimmed.starts_with("- [X] ")
                || trimmed.starts_with("* [x] ")
                || trimmed.starts_with("* [X] "))
        {
            let is_checked = trimmed.starts_with("- [x] ")
                || trimmed.starts_with("- [X] ")
                || trimmed.starts_with("* [x] ")
                || trimmed.starts_with("* [X] ");
            let item_text = trimmed[6..].trim();
            let (status_icon, status_color) = if is_checked {
                ("[✓] ", crate::terminal_theme::ACCENT)
            } else {
                ("[ ] ", crate::terminal_theme::SUBTLE)
            };

            let spans = vec![
                Span::raw("    "),
                Span::styled(
                    status_icon,
                    Style::default()
                        .fg(status_color)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    item_text.to_string(),
                    Style::default().fg(crate::terminal_theme::TEXT),
                ),
            ];
            plain_lines.push(format!("    {}{}", status_icon, item_text));
            inner_lines.push(Line::from(spans));
            continue;
        }

        let indent = line.len() - line.trim_start().len();

        // Numbered list: e.g. "1. ", "2. ", "10. "
        let is_numbered = if let Some(dot_idx) = trimmed.find(". ") {
            !trimmed[..dot_idx].is_empty() && trimmed[..dot_idx].chars().all(|c| c.is_ascii_digit())
        } else {
            false
        };

        if is_numbered {
            let dot_idx = trimmed.find(". ").unwrap();
            let num_prefix = &trimmed[..=dot_idx]; // e.g. "1."
            let item_text = trimmed[dot_idx + 2..].trim();
            let prefix_indent = if indent >= 2 { "      " } else { "    " };

            let spans = vec![
                Span::styled(
                    format!("{}{}", prefix_indent, num_prefix),
                    Style::default()
                        .fg(crate::terminal_theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(
                    item_text.to_string(),
                    Style::default().fg(crate::terminal_theme::TEXT),
                ),
            ];
            plain_lines.push(format!("{}{} {}", prefix_indent, num_prefix, item_text));
            inner_lines.push(Line::from(spans));
            continue;
        }

        // Bullet lists: - or *
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            let bullet_content = trimmed[2..].trim();
            let (prefix_indent, bullet_color) = if indent >= 2 {
                ("      • ", crate::terminal_theme::MUTED)
            } else {
                ("    • ", crate::terminal_theme::ACCENT)
            };

            let spans = if bullet_content.starts_with("**") {
                if let Some(end_bold) = bullet_content[2..].find("**") {
                    let b_title = &bullet_content[2..2 + end_bold];
                    let rest = &bullet_content[2 + end_bold + 2..];
                    vec![
                        Span::styled(prefix_indent, Style::default().fg(bullet_color)),
                        Span::styled(
                            b_title.to_string(),
                            Style::default()
                                .fg(crate::terminal_theme::TEXT)
                                .add_modifier(Modifier::BOLD),
                        ),
                        Span::styled(
                            rest.to_string(),
                            Style::default().fg(crate::terminal_theme::TEXT),
                        ),
                    ]
                } else {
                    vec![
                        Span::styled(prefix_indent, Style::default().fg(bullet_color)),
                        Span::styled(
                            bullet_content.to_string(),
                            Style::default().fg(crate::terminal_theme::TEXT),
                        ),
                    ]
                }
            } else {
                vec![
                    Span::styled(prefix_indent, Style::default().fg(bullet_color)),
                    Span::styled(
                        bullet_content.to_string(),
                        Style::default().fg(crate::terminal_theme::TEXT),
                    ),
                ]
            };
            let plain_str = format!("{}{}", prefix_indent, bullet_content.replace("**", ""));
            plain_lines.push(plain_str);
            inner_lines.push(Line::from(spans));
            continue;
        }

        // Normal paragraph text
        inner_lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(
                trimmed.to_string(),
                Style::default().fg(crate::terminal_theme::TEXT),
            ),
        ]));
        plain_lines.push(format!("  {}", trimmed));
    }

    let bg_color = crate::terminal_theme::CARD_BACKGROUND;
    let bg_style = Style::default().bg(bg_color);

    let title_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "📋 Proposed Plan ",
            Style::default()
                .fg(crate::terminal_theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            if title.is_empty() {
                "· Plan Review".to_string()
            } else {
                format!("· {}", title)
            },
            Style::default()
                .fg(crate::terminal_theme::TEXT)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let footer_line = Line::from(vec![
        Span::raw("  "),
        Span::styled(
            "[ Review plan above · Choose Approve to execute or Deny below ]",
            Style::default().fg(crate::terminal_theme::MUTED),
        ),
    ]);

    let mut card_lines = Vec::with_capacity(inner_lines.len() + 6);
    card_lines.push(Line::default()); // top padding
    card_lines.push(title_line);
    card_lines.push(Line::default());
    card_lines.extend(inner_lines);
    card_lines.push(Line::default());
    card_lines.push(footer_line);
    card_lines.push(Line::default()); // bottom padding

    let paragraph = Paragraph::new(card_lines)
        .style(bg_style)
        .wrap(Wrap { trim: false });
    let card_height = paragraph
        .line_count(card_width)
        .max(1)
        .min(u16::MAX as usize) as u16;

    let area = Rect::new(0, 0, card_width, card_height);
    let mut buffer = Buffer::empty(area);
    for cell in &mut buffer.content {
        cell.set_bg(bg_color);
    }
    let widget = paragraph.block(Block::default().style(bg_style));
    widget.render(area, &mut buffer);

    let rendered_lines: Vec<Line<'static>> = (0..card_height)
        .map(|y| {
            let mut spans = Vec::new();
            if padding > 0 {
                spans.push(Span::raw(" ".repeat(padding as usize)));
            }
            let mut x = 0;
            while x < card_width {
                let cell = &buffer[(x, y)];
                let mut style = cell.style();
                style.bg = Some(bg_color);
                let sym = if cell.symbol() == " " {
                    "\u{00A0}"
                } else {
                    cell.symbol()
                };
                spans.push(Span::styled(sym.to_owned(), style));
                x += UnicodeWidthStr::width(cell.symbol()).max(1) as u16;
            }
            Line::from(spans)
        })
        .collect();

    plain_lines.insert(
        0,
        if title.is_empty() {
            "Proposed Plan · Plan Review".to_string()
        } else {
            format!("Proposed Plan · {}", title)
        },
    );

    (rendered_lines, plain_lines)
}

// Wrap first, then pad each visual row: a line background alone would leave
// the unused columns after short text unpainted in the transcript paragraph.
pub(super) fn highlighted_user_content(line: Line<'static>, width: u16) -> Vec<Line<'static>> {
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

// Keep styled text and visual-row offsets, rather than cells for the whole history.
pub(super) struct TranscriptLayout {
    pub(super) text: Text<'static>,
    pub(super) row_starts: Vec<usize>,
    pub(super) height: usize,
    pub(super) width: u16,
}

impl TranscriptLayout {
    pub(super) fn new(text: Text<'static>, width: u16) -> Self {
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
    pub(super) fn paragraph_at(&self, row: usize) -> Paragraph<'static> {
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

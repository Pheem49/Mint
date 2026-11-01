//! Width-reactive transcript entries, paragraph layout, and user message highlighting.
use super::*;

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
        if self.role != TranscriptRole::Assistant
            || width == 0
            || width == self.rendered_width
            || self.text.is_empty()
        {
            return std::borrow::Cow::Borrowed(&self.rendered_lines);
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
        }
    }
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

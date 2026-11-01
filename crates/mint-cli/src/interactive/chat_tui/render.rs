//! Ratatui frame layout and rendering for ChatViewState.
use super::*;

pub(super) fn mod_layout(area: Rect, dock: bool) -> (Rect, Option<Rect>) {
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
pub(super) struct RenderAreas {
    pub(super) transcript: Option<Rect>,
    pub(super) back_to_bottom: Option<Rect>,
    pub(super) composer: Option<Rect>,
    pub(super) watermark: Option<Rect>,
}

impl ChatViewState {
    pub(super) fn render(&self, frame: &mut ratatui::Frame<'_>) -> RenderAreas {
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
            if self.queued_count > 0 {
                let preview = self
                    .queued_preview
                    .as_deref()
                    .unwrap_or("")
                    .chars()
                    .take(30)
                    .collect::<String>();
                let text = if preview.is_empty() {
                    format!(" Queued ({}) · Ctrl+D drop ", self.queued_count)
                } else {
                    format!(
                        " Queued ({}): \"{}\" · Ctrl+D drop ",
                        self.queued_count, preview
                    )
                };
                input_block = input_block.title_bottom(
                    Line::styled(
                        text,
                        Style::default()
                            .fg(crate::terminal_theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )
                    .alignment(ratatui::layout::Alignment::Left),
                );
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

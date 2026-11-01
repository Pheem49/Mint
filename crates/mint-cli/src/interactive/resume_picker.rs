use super::format::truncate_to_width;
use super::*;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::tty::IsTty;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use std::path::Path;
use unicode_width::UnicodeWidthStr;

pub(super) fn format_relative_time(timestamp_str: &str) -> String {
    let parse_res = chrono::DateTime::parse_from_rfc3339(timestamp_str)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .or_else(|_| {
            chrono::NaiveDateTime::parse_from_str(timestamp_str, "%Y-%m-%d %H:%M:%S")
                .map(|ndt| ndt.and_utc())
        });

    let Ok(utc_time) = parse_res else {
        return "recently".to_string();
    };

    let now = chrono::Utc::now();
    let diff = now.signed_duration_since(utc_time);

    let secs = diff.num_seconds();
    if secs < 60 {
        return "just now".to_string();
    }
    let mins = diff.num_minutes();
    if mins < 60 {
        return format!("{mins}m ago");
    }
    let hours = diff.num_hours();
    if hours < 24 {
        return format!("{hours}h ago");
    }
    let days = diff.num_days();
    if days == 1 {
        return "yesterday".to_string();
    }
    if days < 7 {
        return format!("{days} days ago");
    }
    let weeks = days / 7;
    if weeks == 1 {
        return "1 week ago".to_string();
    }
    if weeks < 4 {
        return format!("{weeks} weeks ago");
    }
    let months = days / 30;
    if months == 1 {
        return "1 month ago".to_string();
    }
    if months < 12 {
        return format!("{months} months ago");
    }
    let years = days / 365;
    if years == 1 {
        return "1 year ago".to_string();
    }
    format!("{years} years ago")
}

#[derive(Debug, Clone)]
pub(crate) struct ResumePickerCore {
    pub sessions: Vec<mint_core::ChatSession>,
    pub current_workspace: String,
    pub current_branch: Option<String>,
    pub active_chat_id: String,
    pub show_all_projects: bool,
    pub only_current_branch: bool,
    pub sort_by_created: bool,
    pub showing_preview: bool,
    pub renaming: bool,
    pub rename_query: String,
}

impl ResumePickerCore {
    pub fn new(
        sessions: Vec<mint_core::ChatSession>,
        current_workspace: String,
        current_branch: Option<String>,
        active_chat_id: String,
    ) -> Self {
        let show_all_projects = !sessions
            .iter()
            .any(|session| session.workspace_path.as_deref() == Some(&current_workspace));
        Self {
            sessions,
            current_workspace,
            current_branch,
            active_chat_id,
            show_all_projects,
            only_current_branch: false,
            sort_by_created: false,
            showing_preview: false,
            renaming: false,
            rename_query: String::new(),
        }
    }

    pub fn filtered_indices(&self, filter: &[char]) -> Vec<usize> {
        let query: String = filter.iter().collect::<String>().to_lowercase();
        let mut indices: Vec<usize> = self
            .sessions
            .iter()
            .enumerate()
            .filter(|(_, session)| {
                if !self.show_all_projects
                    && session.workspace_path.as_deref() != Some(&self.current_workspace)
                {
                    return false;
                }
                if self.only_current_branch {
                    match self.current_branch.as_deref() {
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
            let left = &self.sessions[*left];
            let right = &self.sessions[*right];
            let (left_time, right_time) = if self.sort_by_created {
                (&left.created_at, &right.created_at)
            } else {
                (&left.updated_at, &right.updated_at)
            };
            right_time.cmp(left_time)
        });
        indices
    }

    pub fn render_lines(
        &self,
        filter: &[char],
        selected: usize,
        scroll_offset: usize,
        width: u16,
        height: u16,
    ) -> Vec<Line<'static>> {
        let filtered = self.filtered_indices(filter);
        let count = filtered.len();
        let total = self.sessions.len();
        let muted = Style::default().fg(crate::terminal_theme::MUTED);
        let selected_chip = Style::default()
            .fg(crate::terminal_theme::SELECTION_TEXT)
            .bg(crate::terminal_theme::SELECTION_BACKGROUND)
            .add_modifier(Modifier::BOLD);

        if self.renaming {
            return vec![
                Line::styled(
                    "Rename session title:",
                    Style::default()
                        .fg(crate::terminal_theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                ),
                Line::from(vec![
                    Span::styled("> ", Style::default().fg(crate::terminal_theme::SUBTLE)),
                    Span::raw(self.rename_query.clone()),
                    Span::styled("█", Style::default().fg(crate::terminal_theme::TEXT)),
                ]),
                Line::styled("(Press Enter to save, Esc to cancel)", muted),
            ];
        }

        if self.showing_preview
            && let Some(&orig_idx) = filtered.get(selected)
            && let Some(target_session) = self.sessions.get(orig_idx)
        {
            let mut lines = vec![
                Line::from(vec![
                    Span::styled("Session: ", muted),
                    Span::styled(
                        target_session.title.clone(),
                        Style::default()
                            .fg(crate::terminal_theme::TEXT)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(format!(" ({})", target_session.id), muted),
                ]),
                Line::raw(""),
            ];
            if let Ok(memory) = mint_core::MemoryStore::open_default()
                && let Ok(recent) = memory.get_session_preview(&target_session.id, 2)
            {
                if recent.is_empty() {
                    lines.push(Line::styled(
                        "  (No messages recorded in this session)",
                        muted,
                    ));
                } else {
                    for item in recent.iter().rev() {
                        let u_snippet = crate::interactive::truncate_utf8(&item.user_text, 100);
                        let a_snippet = crate::interactive::truncate_utf8(&item.ai_text, 140);
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  User › ",
                                Style::default()
                                    .fg(crate::terminal_theme::SUBTLE)
                                    .add_modifier(Modifier::BOLD),
                            ),
                            Span::raw(u_snippet.replace('\n', " ")),
                        ]));
                        lines.push(Line::from(vec![
                            Span::styled(
                                "  Assistant › ",
                                Style::default().fg(crate::terminal_theme::SUBTLE),
                            ),
                            Span::styled(
                                a_snippet.replace('\n', " "),
                                Style::default().fg(crate::terminal_theme::TEXT),
                            ),
                        ]));
                        lines.push(Line::raw(""));
                    }
                }
            }
            lines.push(Line::styled(
                "Press Space or Esc to close preview · Enter to resume",
                muted,
            ));
            return lines;
        }

        let selected_position = if count == 0 {
            0
        } else {
            (selected + 1).min(count)
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
                if self.show_all_projects {
                    muted
                } else {
                    selected_chip
                },
            ),
            Span::styled(
                " All ",
                if self.show_all_projects {
                    selected_chip
                } else {
                    muted
                },
            ),
            Span::styled("  ·  Branch: ", muted),
            Span::styled(
                " All ",
                if self.only_current_branch {
                    muted
                } else {
                    selected_chip
                },
            ),
            Span::styled(
                " Current ",
                if self.only_current_branch {
                    selected_chip
                } else {
                    muted
                },
            ),
            Span::styled("  ·  Sort: ", muted),
            Span::styled(
                " Updated ",
                if self.sort_by_created {
                    muted
                } else {
                    selected_chip
                },
            ),
            Span::styled(
                " Created ",
                if self.sort_by_created {
                    selected_chip
                } else {
                    muted
                },
            ),
        ];
        if self.only_current_branch && self.current_branch.is_none() {
            filters.push(Span::styled(" (no branch)", muted));
        }
        lines.push(Line::from(filters));

        let query: String = filter.iter().collect();
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

        let project_name = std::path::Path::new(&self.current_workspace)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.current_workspace.clone());
        let project_label = if self.show_all_projects {
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
        let scroll_offset = if selected < scroll_offset {
            selected
        } else if selected >= scroll_offset + available_rows {
            selected + 1 - available_rows
        } else {
            scroll_offset
        }
        .min(count.saturating_sub(available_rows));
        let end = (scroll_offset + available_rows).min(count);

        if count == 0 {
            lines.push(Line::styled("No matching sessions", muted));
        } else {
            for position in scroll_offset..end {
                let session = &self.sessions[filtered[position]];
                let time = truncate_to_width(&format_relative_time(&session.updated_at), 10);
                let message_count = format!("{} msgs", session.message_count);
                let current_tag = if session.id == self.active_chat_id {
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
                let title = truncate_to_width(&session.title, title_width);
                let is_selected = position == selected;
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
            "Enter resume · Space preview · Ctrl+R rename · ↑/↓ navigate · Esc cancel",
            muted,
        ));
        lines.push(Line::styled(
            "Ctrl+A project · Ctrl+B branch · Tab sort · Type to search",
            muted,
        ));
        lines
    }
}

pub fn prompt_resume_session_picker_with_ui(
    ui: &mut dyn CommandUi,
    current_dir: &Path,
    active_chat_id: &str,
) -> Result<Option<String>> {
    let memory = match mint_core::MemoryStore::open_default() {
        Ok(m) => m,
        Err(err) => {
            ui.push_notice(format!("Failed to open memory store: {err}"));
            return Ok(None);
        }
    };

    let mut sessions = memory.list_chat_sessions().unwrap_or_default();
    sessions.retain(|s| {
        (s.kind == "cli" || mint_core::is_cli_chat_id(&s.id))
            && (s.message_count > 0 || s.id == active_chat_id)
    });

    if sessions.is_empty() {
        ui.push_notice("No past conversation sessions found to resume.");
        return Ok(None);
    }

    let current_branch = mint_core::git::get_current_branch(current_dir);
    ui.prompt_resume_picker(sessions, current_dir, current_branch, active_chat_id)
}

pub fn prompt_resume_session_picker(
    current_dir: &Path,
    active_chat_id: &str,
) -> Result<Option<String>> {
    if !io::stdout().is_tty() || !io::stdin().is_tty() {
        return Ok(None);
    }

    let memory = match mint_core::MemoryStore::open_default() {
        Ok(m) => m,
        Err(err) => {
            println!("{ERROR}Failed to open memory store:{RESET} {err}");
            return Ok(None);
        }
    };

    let mut sessions = memory.list_chat_sessions().unwrap_or_default();
    // Filter to only CLI sessions (legacy "cli", "cli::<uuid>", or kind == "cli"),
    // excluding general Web/Desktop conversations and cron jobs.
    sessions.retain(|s| {
        (s.kind == "cli" || mint_core::is_cli_chat_id(&s.id))
            && (s.message_count > 0 || s.id == active_chat_id)
    });

    if sessions.is_empty() {
        println!("\n{DIM}No past conversation sessions found to resume.{RESET}\n");
        return Ok(None);
    }

    let current_branch = mint_core::git::get_current_branch(current_dir);
    let current_workspace_str = current_dir.to_string_lossy().to_string();
    let project_name = current_dir
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| "Mint-CLI".to_string());

    let mut search_query = String::new();
    let mut show_all_projects = false;
    let mut only_current_branch = false;
    let mut sort_by_created = false;
    let mut selected_idx: usize = 0;
    let mut scroll_offset: usize = 0;
    let mut showing_preview = false;
    let mut renaming = false;
    let mut rename_query = String::new();

    // Check if there are sessions for current workspace
    let has_current_workspace_sessions = sessions
        .iter()
        .any(|s| s.workspace_path.as_deref() == Some(&current_workspace_str));
    if !has_current_workspace_sessions {
        // If current project has none, default show all projects
        show_all_projects = true;
    }

    let (_tw, th) = crossterm::terminal::size().unwrap_or((80, 24));
    // TUI viewport height: 16 lines fits search, headers, list, and bottom hints
    let height = (th.saturating_sub(2)).clamp(12, 20);

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

    let result = loop {
        // 1. Filter sessions
        let mut filtered: Vec<&mint_core::ChatSession> = sessions
            .iter()
            .filter(|s| {
                if !show_all_projects
                    && let Some(ref p) = s.workspace_path
                    && p != &current_workspace_str
                {
                    return false;
                }
                if only_current_branch {
                    match current_branch.as_deref() {
                        Some(branch) if s.git_branch.as_deref() == Some(branch) => {}
                        _ => return false,
                    }
                }
                if !search_query.is_empty() {
                    let q = search_query.to_lowercase();
                    let title_match = s.title.to_lowercase().contains(&q);
                    let id_match = s.id.to_lowercase().contains(&q);
                    let lang_match = s
                        .main_language
                        .as_ref()
                        .map(|l| l.to_lowercase().contains(&q))
                        .unwrap_or(false);
                    let branch_match = s
                        .git_branch
                        .as_ref()
                        .map(|b| b.to_lowercase().contains(&q))
                        .unwrap_or(false);
                    if !title_match && !id_match && !lang_match && !branch_match {
                        return false;
                    }
                }
                true
            })
            .collect();
        filtered.sort_by(|left, right| {
            let left_time = if sort_by_created {
                &left.created_at
            } else {
                &left.updated_at
            };
            let right_time = if sort_by_created {
                &right.created_at
            } else {
                &right.updated_at
            };
            right_time.cmp(left_time)
        });

        let total_count = sessions.len();
        let filtered_count = filtered.len();

        if selected_idx >= filtered_count {
            selected_idx = filtered_count.saturating_sub(1);
        }

        // Header, filters, search, count, divider, and two-line footer use seven rows.
        let visible_items = (height as usize).saturating_sub(7).max(1);
        if selected_idx < scroll_offset {
            scroll_offset = selected_idx;
        } else if selected_idx >= scroll_offset + visible_items {
            scroll_offset = selected_idx.saturating_sub(visible_items) + 1;
        }

        // Draw TUI frame
        let project_name_clone = project_name.clone();
        let search_query_clone = search_query.clone();
        let rename_query_clone = rename_query.clone();

        let _ = terminal.draw(|frame| {
            let area = frame.area();
            frame.render_widget(Clear, area);

            if renaming {
                // Render rename popup
                let mut lines = Vec::new();
                lines.push(Line::from(vec![Span::styled(
                    "Rename session title:",
                    Style::default()
                        .fg(crate::terminal_theme::TEXT)
                        .add_modifier(Modifier::BOLD),
                )]));
                lines.push(Line::from(vec![
                    Span::styled("> ", Style::default().fg(crate::terminal_theme::SUBTLE)),
                    Span::raw(&rename_query_clone),
                    Span::styled("█", Style::default().fg(crate::terminal_theme::TEXT)),
                ]));
                lines.push(Line::from(vec![Span::styled(
                    "(Press Enter to save, Esc to cancel)",
                    Style::default().fg(crate::terminal_theme::MUTED),
                )]));

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Rename Session ")
                    .border_style(Style::default().fg(crate::terminal_theme::SUBTLE));
                let p = Paragraph::new(lines).block(block);
                frame.render_widget(p, area);
                return;
            }

            if showing_preview && selected_idx < filtered_count {
                // Render preview drawer
                let target_session = filtered[selected_idx];
                let mut lines = Vec::new();
                lines.push(Line::from(vec![
                    Span::styled(
                        "Session: ",
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ),
                    Span::styled(
                        &target_session.title,
                        Style::default()
                            .fg(crate::terminal_theme::TEXT)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({})", target_session.id),
                        Style::default().fg(crate::terminal_theme::MUTED),
                    ),
                ]));
                lines.push(Line::from(""));

                if let Ok(recent) = memory.get_session_preview(&target_session.id, 2) {
                    if recent.is_empty() {
                        lines.push(Line::from(Span::styled(
                            "  (No messages recorded in this session)",
                            Style::default().fg(crate::terminal_theme::MUTED),
                        )));
                    } else {
                        for item in recent.iter().rev() {
                            let u_snippet = crate::interactive::truncate_utf8(&item.user_text, 100);
                            let a_snippet = crate::interactive::truncate_utf8(&item.ai_text, 140);
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "  User › ",
                                    Style::default()
                                        .fg(crate::terminal_theme::SUBTLE)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                Span::raw(u_snippet.replace('\n', " ")),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled(
                                    "  Assistant › ",
                                    Style::default().fg(crate::terminal_theme::SUBTLE),
                                ),
                                Span::styled(
                                    a_snippet.replace('\n', " "),
                                    Style::default().fg(crate::terminal_theme::TEXT),
                                ),
                            ]));
                            lines.push(Line::from(""));
                        }
                    }
                }

                lines.push(Line::from(Span::styled(
                    "Press Space or Esc to close preview · Enter to resume",
                    Style::default().fg(crate::terminal_theme::MUTED),
                )));

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Session Preview ")
                    .border_style(Style::default().fg(crate::terminal_theme::SUBTLE));
                let p = Paragraph::new(lines).block(block);
                frame.render_widget(p, area);
                return;
            }

            // Normal Picker View
            let mut lines = Vec::new();
            lines.push(Line::styled(
                "Resume a previous session",
                Style::default()
                    .fg(crate::terminal_theme::TEXT)
                    .add_modifier(Modifier::BOLD),
            ));

            let active_filter_style = Style::default()
                .fg(crate::terminal_theme::SELECTION_TEXT)
                .bg(crate::terminal_theme::SELECTION_BACKGROUND)
                .add_modifier(Modifier::BOLD);
            let muted_style = Style::default().fg(crate::terminal_theme::MUTED);
            let mut filter_line = vec![
                Span::styled("Project: ", muted_style),
                Span::styled(
                    " Cwd ",
                    if !show_all_projects {
                        active_filter_style
                    } else {
                        muted_style
                    },
                ),
                Span::styled(
                    " All ",
                    if show_all_projects {
                        active_filter_style
                    } else {
                        muted_style
                    },
                ),
                Span::styled("  ·  Branch: ", muted_style),
                Span::styled(
                    " All ",
                    if !only_current_branch {
                        active_filter_style
                    } else {
                        muted_style
                    },
                ),
                Span::styled(
                    " Current ",
                    if only_current_branch {
                        active_filter_style
                    } else {
                        muted_style
                    },
                ),
                Span::styled("  ·  Sort: ", muted_style),
                Span::styled(
                    " Updated ",
                    if !sort_by_created {
                        active_filter_style
                    } else {
                        muted_style
                    },
                ),
                Span::styled(
                    " Created ",
                    if sort_by_created {
                        active_filter_style
                    } else {
                        muted_style
                    },
                ),
            ];
            if only_current_branch && current_branch.is_none() {
                filter_line.push(Span::styled(" (no branch)", muted_style));
            }
            lines.push(Line::from(filter_line));

            let search_text = if search_query_clone.is_empty() {
                "Type to search".to_owned()
            } else {
                search_query_clone.clone()
            };
            lines.push(Line::from(vec![
                Span::styled("⌕  ", Style::default().fg(crate::terminal_theme::SUBTLE)),
                Span::styled(
                    search_text,
                    if search_query_clone.is_empty() {
                        muted_style
                    } else {
                        Style::default().fg(crate::terminal_theme::TEXT)
                    },
                ),
                if search_query_clone.is_empty() {
                    Span::raw("")
                } else {
                    Span::styled("█", Style::default().fg(crate::terminal_theme::TEXT))
                },
            ]));

            let project_label = if show_all_projects {
                "All projects".to_owned()
            } else {
                project_name_clone.clone()
            };
            let result_label = if filtered_count == 1 {
                "session"
            } else {
                "sessions"
            };
            lines.push(Line::styled(
                format!("{project_label} · {filtered_count} of {total_count} {result_label}"),
                muted_style,
            ));
            lines.push(Line::styled(
                "─".repeat(area.width as usize),
                Style::default().fg(crate::terminal_theme::MUTED),
            ));

            // One compact row per session keeps long titles from colliding with metadata.
            if filtered_count == 0 {
                lines.push(Line::styled("No matching sessions", muted_style));
            } else {
                let end_idx = (scroll_offset + visible_items).min(filtered_count);
                for i in scroll_offset..end_idx {
                    let s = filtered[i];
                    let is_sel = i == selected_idx;
                    let rel_time = format_relative_time(&s.updated_at);
                    let time = truncate_to_width(&rel_time, 10);
                    let count = format!("{} msgs", s.message_count);
                    let current_tag = if s.id == active_chat_id {
                        " (current)"
                    } else {
                        ""
                    };
                    let title_width = (area.width as usize)
                        .saturating_sub(2 + 10 + 2 + UnicodeWidthStr::width(count.as_str()) + 2)
                        .saturating_sub(UnicodeWidthStr::width(current_tag));
                    let title = truncate_to_width(&s.title, title_width);
                    let row = format!("  {time:>10}  {title}{current_tag}  {count}");
                    let row_width = UnicodeWidthStr::width(row.as_str());
                    let row = format!(
                        "{row}{}",
                        " ".repeat((area.width as usize).saturating_sub(row_width))
                    );
                    if is_sel {
                        lines.push(Line::styled(format!("›{}", &row[1..]), active_filter_style));
                    } else {
                        lines.push(Line::from(vec![
                            Span::styled("  ", Style::default().fg(crate::terminal_theme::SUBTLE)),
                            Span::styled(format!("{time:>10}  "), muted_style),
                            Span::styled(
                                format!("{title}{current_tag}"),
                                Style::default().fg(crate::terminal_theme::TEXT),
                            ),
                            Span::styled(format!("  {count}"), muted_style),
                            Span::raw(" ".repeat((area.width as usize).saturating_sub(row_width))),
                        ]));
                    }
                }
            }

            // Fill empty list rows so the footer stays anchored.
            while lines.len() + 2 < height as usize {
                lines.push(Line::from(""));
            }

            lines.push(Line::styled(
                "Enter resume · Space preview · Ctrl+R rename · Esc cancel",
                muted_style,
            ));
            lines.push(Line::styled(
                "Ctrl+A project · Ctrl+B branch · Tab sort · Type to search",
                muted_style,
            ));

            let p = Paragraph::new(lines);
            frame.render_widget(p, area);
        });

        // 2. Handle Key Events
        match event::poll(std::time::Duration::from_millis(80)) {
            Ok(true) => match event::read() {
                Ok(Event::Key(key_event)) => {
                    if key_event.kind != event::KeyEventKind::Press {
                        continue;
                    }

                    // Ctrl+C exits immediately
                    if key_event.code == KeyCode::Char('c')
                        && key_event.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        break None;
                    }

                    // Handle renaming popup keys
                    if renaming {
                        match key_event.code {
                            KeyCode::Esc => {
                                renaming = false;
                            }
                            KeyCode::Enter => {
                                let trimmed = rename_query.trim();
                                if !trimmed.is_empty() && selected_idx < filtered_count {
                                    let target_id = filtered[selected_idx].id.clone();
                                    let _ = memory.rename_chat_session(&target_id, trimmed);
                                    if let Some(item) =
                                        sessions.iter_mut().find(|s| s.id == target_id)
                                    {
                                        item.title = trimmed.to_string();
                                    }
                                }
                                renaming = false;
                            }
                            KeyCode::Backspace => {
                                rename_query.pop();
                            }
                            KeyCode::Char(c) => {
                                rename_query.push(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle preview popup keys
                    if showing_preview {
                        match key_event.code {
                            KeyCode::Char(' ') | KeyCode::Esc => {
                                showing_preview = false;
                            }
                            KeyCode::Enter if selected_idx < filtered_count => {
                                break Some(filtered[selected_idx].id.clone());
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Global hotkeys
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) {
                        match key_event.code {
                            KeyCode::Char('a') => {
                                show_all_projects = !show_all_projects;
                                selected_idx = 0;
                                scroll_offset = 0;
                                continue;
                            }
                            KeyCode::Char('b') => {
                                only_current_branch = !only_current_branch;
                                selected_idx = 0;
                                scroll_offset = 0;
                                continue;
                            }
                            KeyCode::Char('r') if selected_idx < filtered_count => {
                                renaming = true;
                                rename_query = filtered[selected_idx].title.clone();
                                continue;
                            }
                            _ => {}
                        }
                    }

                    match key_event.code {
                        KeyCode::Esc => {
                            break None;
                        }
                        KeyCode::Up => {
                            selected_idx = selected_idx.saturating_sub(1);
                        }
                        KeyCode::Down => {
                            if filtered_count > 0 {
                                selected_idx = (selected_idx + 1).min(filtered_count - 1);
                            }
                        }
                        KeyCode::PageUp => {
                            selected_idx = selected_idx.saturating_sub(visible_items);
                        }
                        KeyCode::PageDown => {
                            if filtered_count > 0 {
                                selected_idx =
                                    (selected_idx + visible_items).min(filtered_count - 1);
                            }
                        }
                        KeyCode::Tab => {
                            sort_by_created = !sort_by_created;
                            selected_idx = 0;
                            scroll_offset = 0;
                        }
                        KeyCode::Char(' ')
                            if search_query.is_empty() && selected_idx < filtered_count =>
                        {
                            showing_preview = true;
                        }
                        KeyCode::Enter => {
                            if selected_idx < filtered_count {
                                break Some(filtered[selected_idx].id.clone());
                            }
                        }
                        KeyCode::Backspace => {
                            search_query.pop();
                            selected_idx = 0;
                            scroll_offset = 0;
                        }
                        KeyCode::Char(c) => {
                            search_query.push(c);
                            selected_idx = 0;
                            scroll_offset = 0;
                        }
                        _ => {}
                    }
                }
                _ => {}
            },
            _ => {}
        }
    };

    let _ = crossterm::terminal::disable_raw_mode();
    let _ = terminal.clear();

    Ok(result)
}

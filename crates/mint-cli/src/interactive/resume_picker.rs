use super::*;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use crossterm::tty::IsTty;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use std::path::Path;

fn format_relative_time(timestamp_str: &str) -> String {
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

fn format_bytes(bytes: usize) -> String {
    if bytes < 1024 {
        format!("{bytes}B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1}KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1}MB", bytes as f64 / (1024.0 * 1024.0))
    }
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
    let mut selected_idx: usize = 0;
    let mut scroll_offset: usize = 0;
    let mut showing_preview = false;
    let mut renaming = false;
    let mut rename_query = String::new();

    // Check if there are sessions for current workspace
    let has_current_workspace_sessions = sessions.iter().any(|s| {
        s.workspace_path.as_deref() == Some(&current_workspace_str)
    });
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
        let filtered: Vec<&mint_core::ChatSession> = sessions
            .iter()
            .filter(|s| {
                if !show_all_projects
                    && let Some(ref p) = s.workspace_path
                    && p != &current_workspace_str
                {
                    return false;
                }
                if only_current_branch
                    && let Some(ref cb) = current_branch
                    && s.git_branch.as_deref() != Some(cb.as_str())
                {
                    return false;
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

        let total_count = sessions.len();
        let filtered_count = filtered.len();

        if selected_idx >= filtered_count {
            selected_idx = filtered_count.saturating_sub(1);
        }

        // Available items visible in the list:
        // Height 16: title (1), search (3), project header (1), list (~9), hints (1)
        let visible_items = ((height as usize).saturating_sub(7) / 2).max(2);
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
                lines.push(Line::from(vec![
                    Span::styled("Rename session title:", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("> ", Style::default().fg(Color::Green)),
                    Span::raw(&rename_query_clone),
                    Span::styled("█", Style::default().fg(Color::Cyan)),
                ]));
                lines.push(Line::from(vec![
                    Span::styled("(Press Enter to save, Esc to cancel)", Style::default().fg(Color::DarkGray)),
                ]));

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Rename Session ")
                    .border_style(Style::default().fg(Color::Cyan));
                let p = Paragraph::new(lines).block(block);
                frame.render_widget(p, area);
                return;
            }

            if showing_preview && selected_idx < filtered_count {
                // Render preview drawer
                let target_session = filtered[selected_idx];
                let mut lines = Vec::new();
                lines.push(Line::from(vec![
                    Span::styled("Session: ", Style::default().fg(Color::DarkGray)),
                    Span::styled(&target_session.title, Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
                    Span::styled(format!(" ({})", target_session.id), Style::default().fg(Color::DarkGray)),
                ]));
                lines.push(Line::from(""));

                if let Ok(recent) = memory.get_session_preview(&target_session.id, 2) {
                    if recent.is_empty() {
                        lines.push(Line::from(Span::styled("  (No messages recorded in this session)", Style::default().fg(Color::DarkGray))));
                    } else {
                        for item in recent.iter().rev() {
                            let u_snippet = crate::interactive::truncate_utf8(&item.user_text, 100);
                            let a_snippet = crate::interactive::truncate_utf8(&item.ai_text, 140);
                            lines.push(Line::from(vec![
                                Span::styled("  User › ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                                Span::raw(u_snippet.replace('\n', " ")),
                            ]));
                            lines.push(Line::from(vec![
                                Span::styled("  Assistant › ", Style::default().fg(Color::Green)),
                                Span::styled(a_snippet.replace('\n', " "), Style::default().fg(Color::Gray)),
                            ]));
                            lines.push(Line::from(""));
                        }
                    }
                }

                lines.push(Line::from(Span::styled(
                    "Press Space or Esc to close preview · Enter to resume",
                    Style::default().fg(Color::DarkGray),
                )));

                let block = Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .title(" Session Preview ")
                    .border_style(Style::default().fg(Color::Cyan));
                let p = Paragraph::new(lines).block(block);
                frame.render_widget(p, area);
                return;
            }

            // Normal Picker View
            let mut lines = Vec::new();

            // 1. Header: Resume session (14 of 49)
            let header_text = format!("Resume session ({} of {})", if filtered_count > 0 { selected_idx + 1 } else { 0 }, total_count);
            lines.push(Line::from(vec![
                Span::styled(header_text, Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]));

            // 2. Search box with border
            let search_display = if search_query_clone.is_empty() {
                Span::styled("⌕ Search...", Style::default().fg(Color::DarkGray))
            } else {
                Span::styled(format!("⌕ {search_query_clone}█"), Style::default().fg(Color::White))
            };

            let search_width = area.width.saturating_sub(4).max(10) as usize;
            let bar_top = format!("┌{}┐", "─".repeat(search_width));
            let bar_bot = format!("└{}┘", "─".repeat(search_width));
            lines.push(Line::from(Span::styled(bar_top, Style::default().fg(Color::DarkGray))));
            lines.push(Line::from(vec![
                Span::styled("│ ", Style::default().fg(Color::DarkGray)),
                search_display,
            ]));
            lines.push(Line::from(Span::styled(bar_bot, Style::default().fg(Color::DarkGray))));

            // 3. Project Header
            let project_label = if show_all_projects {
                "  All Projects".to_string()
            } else {
                format!("  {project_name_clone}")
            };
            lines.push(Line::from(Span::styled(project_label, Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD))));

            // 4. List Items
            if filtered_count == 0 {
                lines.push(Line::from(Span::styled("    No matching sessions found", Style::default().fg(Color::DarkGray))));
            } else {
                let end_idx = (scroll_offset + visible_items).min(filtered_count);
                for i in scroll_offset..end_idx {
                    let s = filtered[i];
                    let is_sel = i == selected_idx;
                    let is_active = s.id == active_chat_id;

                    let prefix = if is_sel { "> " } else { "  " };
                    let active_tag = if is_active { " (current)" } else { "" };
                    let title_span = if is_sel {
                        Span::styled(format!("{prefix}{}{active_tag}", s.title), Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
                    } else {
                        Span::styled(format!("{prefix}{}{active_tag}", s.title), Style::default().fg(Color::White))
                    };

                    let rel_time = format_relative_time(&s.updated_at);
                    let lang = s.main_language.clone().unwrap_or_else(|| "Project".to_string());
                    let size_str = format_bytes(s.total_bytes);
                    let branch_str = s.git_branch.as_deref().unwrap_or("");
                    let branch_part = if !branch_str.is_empty() {
                        format!(" · {branch_str}")
                    } else {
                        String::new()
                    };

                    let subtitle = format!("    {rel_time} · {lang}{branch_part} · {size_str}");
                    let sub_span = Span::styled(subtitle, Style::default().fg(Color::DarkGray));

                    lines.push(Line::from(title_span));
                    lines.push(Line::from(sub_span));
                }
            }

            // Fill remaining empty lines up to height - 1
            while lines.len() + 1 < height as usize {
                lines.push(Line::from(""));
            }

            // 5. Footer Shortcuts
            let all_proj_hint = if show_all_projects { "Ctrl+A current project" } else { "Ctrl+A all projects" };
            let branch_hint = if only_current_branch { "Ctrl+B all branches" } else { "Ctrl+B current branch" };
            let footer_str = format!("{all_proj_hint} · {branch_hint} · Space to preview · Ctrl+R to rename · Esc to cancel");
            lines.push(Line::from(Span::styled(footer_str, Style::default().fg(Color::DarkGray))));

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
                                    if let Some(item) = sessions.iter_mut().find(|s| s.id == target_id) {
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
                                selected_idx = (selected_idx + visible_items).min(filtered_count - 1);
                            }
                        }
                        KeyCode::Char(' ') if search_query.is_empty() && selected_idx < filtered_count => {
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

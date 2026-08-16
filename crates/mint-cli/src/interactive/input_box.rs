use super::*;
use ansi_to_tui::IntoText;

const AUTOCOMPLETE_COMMANDS: &[(&str, &str)] = &[
    (
        "/autoskill",
        "Toggle auto-writing a SKILL.md after hard tasks",
    ),
    ("/bg", "Run a query in the background, non-blocking"),
    ("/cd", "Change active workspace directory"),
    (
        "/cron",
        "Create/list/remove/enable/disable scheduled agent tasks",
    ),
    (
        "/cron add",
        "Create a scheduled task — walks through a wizard if you don't type more",
    ),
    ("/link", "Link a folder chat can auto-write notes into"),
    ("/clear", "Clear conversation history"),
    ("/code", "Run in code-agent mode"),
    ("/edit-image", "Edit attached image with prompt instruction"),
    ("/exit", "Exit Mint CLI"),
    ("/fast", "Toggle fast mode (hide thinking traces)"),
    (
        "/plan",
        "Toggle plan mode (read-only until you approve a plan)",
    ),
    ("/gen-image", "Generate image using AI model"),
    ("/generate-image", "Generate image using AI model"),
    ("/help", "Show help menu"),
    ("/image", "Attach image from disk"),
    (
        "/image-provider",
        "List image gen providers or switch default provider",
    ),
    ("/jobs", "List, inspect, or cancel background jobs"),
    ("/learn", "Import persistent skill/instruction"),
    ("/mcp", "List configured MCP servers"),
    ("/memory", "Manage long-term memory store"),
    ("/models", "List AI providers or switch active provider"),
    ("/multi-agent", "Toggle Multi-Agent Collaboration system"),
    ("/paste", "Attach image from clipboard"),
    ("/plugins", "List or generate plugins/skills"),
    (
        "/release-notes",
        "Show release notes for the current version",
    ),
    (
        "/search-provider",
        "List web search providers or switch default provider",
    ),
    (
        "/shells",
        "List, inspect, or kill background shell jobs run_shell started",
    ),
    ("/skill add", "Add or install global skill file or folder"),
    ("/stats", "Show session statistics"),
    ("/veo", "Generate video using Google Veo"),
    (
        "/video-provider",
        "List video gen providers or switch default provider",
    ),
];
/// Width, in raw characters, available for input text within the box —
/// shared by every function that needs to reason about row layout, so the
/// terminal-width query and margin/prefix math stay in one place.
fn input_content_width() -> usize {
    let (term_width, _) = crate::markdown::terminal_size_or_default();
    let width = term_width as usize;
    let prefix_len = "› ".chars().count();
    width.saturating_sub(2).saturating_sub(prefix_len).max(1)
}
/// Splits `input_chars` into visual rows — breaking both at `row_width`
/// characters (character-count, not visual-width, matching this module's
/// existing horizontal-scroll convention) and at any explicit `\n` the user
/// inserted (Alt+Enter) — and reports which row/column the cursor falls in.
/// A `\n` itself is consumed by the break and never appears in a row's text.
fn wrap_input_into_rows(
    input_chars: &[char],
    row_width: usize,
    cursor_pos: usize,
) -> (Vec<String>, usize, usize) {
    let row_width = row_width.max(1);
    if input_chars.is_empty() {
        return (vec![String::new()], 0, 0);
    }

    let mut rows: Vec<String> = Vec::new();
    let mut row_starts: Vec<usize> = Vec::new();
    let mut row_start = 0usize;
    let mut row_len = 0usize;

    for (i, &c) in input_chars.iter().enumerate() {
        if c == '\n' {
            rows.push(input_chars[row_start..i].iter().collect());
            row_starts.push(row_start);
            row_start = i + 1;
            row_len = 0;
            continue;
        }
        row_len += 1;
        if row_len == row_width {
            rows.push(input_chars[row_start..=i].iter().collect());
            row_starts.push(row_start);
            row_start = i + 1;
            row_len = 0;
        }
    }
    // Only a trailing `\n` should conjure up a fresh empty row after it —
    // reaching exactly `row_width` with no more input left should NOT (matches
    // this box's prior single-row behavior: the cursor just sits at the edge
    // of the full row until another character is actually typed).
    if row_start < input_chars.len() || input_chars[input_chars.len() - 1] == '\n' {
        rows.push(input_chars[row_start..].iter().collect());
        row_starts.push(row_start);
    }

    let cursor_pos = cursor_pos.min(input_chars.len());
    let mut cursor_row = rows.len() - 1;
    for (idx, &start) in row_starts.iter().enumerate() {
        let end = start + rows[idx].chars().count();
        if cursor_pos <= end {
            cursor_row = idx;
            break;
        }
    }
    let cursor_col = cursor_pos - row_starts[cursor_row];

    (rows, cursor_row, cursor_col)
}
/// Visual (1-indexed) terminal column for the cursor within its current row —
/// column 4 is the first content character (columns 1-3 are the leading
/// margin space plus the 2-visual-width `"› "`/`"  "` prefix).
fn cursor_visual_column(input_chars: &[char], cursor_pos: usize, content_width: usize) -> usize {
    let (_, _, col) = wrap_input_into_rows(input_chars, content_width, cursor_pos);
    let cursor_pos = cursor_pos.min(input_chars.len());
    let row_start = cursor_pos - col;
    let visual: usize = input_chars[row_start..cursor_pos]
        .iter()
        .copied()
        .map(char_visual_width)
        .sum();
    4 + visual
}
/// Builds the input box's full content as ANSI-formatted lines (composer
/// blank/input rows/blank/status/suggestions) plus where within those lines
/// the terminal's cursor should sit — everything one `ratatui` draw call
/// needs, computed without touching the screen so it's cheap to call on
/// every keystroke. Pure/testable by design: the old implementation
/// interleaved this math with `println!`s, which is why it used to need a
/// second pass (`position_input_cursor`) to re-derive the cursor's on-screen
/// position from "how many lines did I just print" — `ratatui`'s
/// `Frame::set_cursor_position` takes an (x, y) directly, so this can just
/// hand that over instead of recomputing it via up/down ANSI motion.
fn compose_input_box(
    input_chars: &[char],
    cursor_pos: usize,
    placeholder: &str,
    model: &str,
    path_str: &str,
    tab_base_input: Option<&str>,
    tab_index: Option<usize>,
    current_dir: &Path,
    plan_mode: bool,
) -> (Vec<String>, u16, u16) {
    let (term_width, _) = crate::markdown::terminal_size_or_default();
    let width = term_width as usize;
    let prefix = "› ";
    let cont_prefix = "  ";
    let input_width = width.saturating_sub(2);
    let content_max_len = input_content_width();
    let blank_line = " ".repeat(input_width);

    let mut lines: Vec<String> = Vec::new();
    lines.push(format!(" {COMPOSER_BG}{blank_line}{RESET}"));

    let (cursor_row_idx, cursor_col);
    if input_chars.is_empty() {
        cursor_row_idx = 0;
        cursor_col = 0;
        let pad_len = content_max_len.saturating_sub(placeholder.chars().count());
        let padding = " ".repeat(pad_len);
        lines.push(format!(
            " {COMPOSER_BG}{MINT}{prefix}{RESET}{COMPOSER_BG}{DIM}{}\x1b[39m{}{RESET}",
            placeholder, padding
        ));
    } else {
        let (rows, c_row, c_col) = wrap_input_into_rows(input_chars, content_max_len, cursor_pos);
        cursor_row_idx = c_row;
        cursor_col = c_col;
        for (i, row) in rows.iter().enumerate() {
            let row_prefix = if i == 0 { prefix } else { cont_prefix };
            let display_row = format_placeholders(row);
            let visible_len = string_visual_width(row);
            let pad_len = content_max_len.saturating_sub(visible_len);
            let padding = " ".repeat(pad_len);
            lines.push(format!(
                " {COMPOSER_BG}{MINT}{}{RESET}{COMPOSER_BG}{}{}{RESET}",
                row_prefix, display_row, padding
            ));
        }
    }
    let _ = cursor_col; // visual column is derived below via `cursor_visual_column`

    lines.push(format!(" {COMPOSER_BG}{blank_line}{RESET}"));

    // Plan mode is read-only until the user approves a plan, so the status
    // bar swaps [Agent] for [Plan] to keep that state visible at a glance.
    let mode_label = if plan_mode { "[Plan]" } else { "[Agent]" };
    let agent_str = format!(" {DIM}{mode_label}{RESET} {MINT}{}{RESET}", model);
    // Background shell jobs (run_shell(background: true)) still running —
    // shown so they're never invisible between the start and finish notice.
    let bg_running = mint_core::bg_shell::running_count();
    let jobs_prefix = if bg_running > 0 {
        format!(
            "{bg_running} bg shell{} · ",
            if bg_running == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };
    // A `\0`-prefixed path_str is a status override (e.g. history browsing),
    // rendered as-is instead of the usual "path: ..." label.
    let path_rest = match path_str.strip_prefix('\0') {
        Some(status) => status.to_string(),
        None => format!("path: {}", path_str),
    };
    let agent_visible_len = format!(" {mode_label} ").len() + model.chars().count();
    let path_visible_len = jobs_prefix.chars().count() + path_rest.chars().count();

    let status_pad_len = width
        .saturating_sub(1)
        .saturating_sub(agent_visible_len + path_visible_len);
    let status_padding = " ".repeat(status_pad_len);

    // The job-count prefix gets its own BLUE so it stands out from the DIM
    // path text next to it, instead of blending into the status line.
    let colored_jobs_prefix = if bg_running > 0 {
        format!("{BLUE}{jobs_prefix}{RESET}")
    } else {
        String::new()
    };

    lines.push(format!(
        "{}{}{}{}{}{}",
        agent_str, status_padding, colored_jobs_prefix, DIM, path_rest, RESET
    ));

    // Compute and append suggestions
    let raw_input: String = input_chars.iter().collect();
    let search_query = tab_base_input.unwrap_or(&raw_input);
    if search_query.starts_with('/') {
        let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
            .iter()
            .filter(|(cmd, _)| cmd.starts_with(search_query))
            .collect();

        if !matches.is_empty() {
            let total_pages = matches.len().div_ceil(5);
            let highlight_idx = tab_index.map(|idx| idx % matches.len());
            let selected_idx = highlight_idx.unwrap_or(0);
            let current_page = selected_idx / 5;
            let s_start_idx = current_page * 5;
            let s_end_idx = std::cmp::min(s_start_idx + 5, matches.len());

            lines.push(String::new());
            lines.push(format!(
                " {BLUE}Suggestions ({}/{}){RESET}",
                current_page + 1,
                total_pages
            ));
            for i in s_start_idx..s_end_idx {
                let (cmd, desc) = matches[i];
                if Some(i) == highlight_idx {
                    lines.push(format!("  {BLUE}▶ {:<16}{RESET} {DIM}- {}{RESET}", cmd, desc));
                } else {
                    lines.push(format!("    {DIM}{:<16} - {}{RESET}", cmd, desc));
                }
            }
        }
    } else if search_query.starts_with('$') {
        // Parse skill name (excluding arguments)
        let skill_query = search_query
            .split_whitespace()
            .next()
            .unwrap_or(search_query);
        let prefix = &skill_query[1..].to_lowercase();
        let skills = load_all_available_skills(current_dir);
        let matches: Vec<_> = skills
            .iter()
            .filter(|skill| skill.name.to_lowercase().starts_with(prefix))
            .collect();

        if !matches.is_empty() {
            let total_pages = matches.len().div_ceil(5);
            let highlight_idx = tab_index.map(|idx| idx % matches.len());
            let selected_idx = highlight_idx.unwrap_or(0);
            let current_page = selected_idx / 5;
            let s_start_idx = current_page * 5;
            let s_end_idx = std::cmp::min(s_start_idx + 5, matches.len());

            lines.push(String::new());
            lines.push(format!(
                " {BLUE}Suggestions ({}/{}){RESET}",
                current_page + 1,
                total_pages
            ));
            for i in s_start_idx..s_end_idx {
                let skill = matches[i];
                let desc = skill
                    .description
                    .as_deref()
                    .unwrap_or("No description provided");
                let max_desc_len = width.saturating_sub(35);
                let truncated_desc = if desc.chars().count() > max_desc_len {
                    let mut s: String = desc.chars().take(max_desc_len.saturating_sub(3)).collect();
                    s.push_str("...");
                    s
                } else {
                    desc.to_string()
                };

                if Some(i) == highlight_idx {
                    lines.push(format!(
                        "  {BLUE}▶ ${:<20}{RESET} {MINT}[Skill]{RESET} {DIM}{}{RESET}",
                        skill.name, truncated_desc
                    ));
                } else {
                    lines.push(format!(
                        "    {DIM}${:<20} [Skill] {}{RESET}",
                        skill.name, truncated_desc
                    ));
                }
            }
        }
    }

    // Row 0 is the leading composer-background blank line, so input rows
    // start at y=1; `cursor_visual_column` already returns a 1-indexed
    // absolute column (it's what the old `\x1b[{}G` code used directly), so
    // subtracting 1 gives `ratatui`'s 0-indexed x.
    let cursor_y = 1 + cursor_row_idx as u16;
    let cursor_x = (cursor_visual_column(input_chars, cursor_pos, content_max_len) as u16)
        .saturating_sub(1);

    (lines, cursor_x, cursor_y)
}
pub fn read_line_interactive(
    _provider: &str,
    model: &str,
    path_str: &str,
    current_dir: &Path,
    history: &[String],
    jobs: &BackgroundJobs,
    plan_mode: bool,
) -> Result<Option<InteractiveInput>> {
    use crossterm::event::{self, Event, KeyCode};
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};
    use crossterm::tty::IsTty;

    if !io::stdout().is_tty() {
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_owned();
        if trimmed.is_empty() {
            return Ok(None);
        }
        return Ok(Some(InteractiveInput {
            text: trimmed,
            pasted_image: None,
        }));
    }

    let mut input_chars: Vec<char> = Vec::new();
    let mut cursor_pos = 0;
    let placeholder = "Ask anything...";
    let mut ctrl_d_pressed = false;
    let mut pasted_image: Option<String> = None;
    let mut paste_contents: Vec<(String, String)> = Vec::new();
    let mut last_paste_time: Option<std::time::Instant> = None;

    // Track tab autocomplete state
    let mut tab_base_input: Option<String> = None;
    let mut tab_index: Option<usize> = None;

    // Track input-history browsing state (Up/Down over previously submitted lines).
    // `history_index` counts back from the most recent entry: Some(0) is the
    // newest, Some(len-1) the oldest. `tab_base_input` (above) doubles as the
    // saved in-progress draft to restore when navigating back past the newest entry.
    let mut history_index: Option<usize> = None;

    // The box's own `ratatui` inline terminal — lives only for this one call
    // (unlike `agent.rs`'s `InlineTui`, which is shared across a whole agent
    // turn), so it's just a local here rather than a struct field. Safe to
    // reconstruct on every height change (unlike the agent-turn code): this
    // never calls `insert_before`, which is what made repeated reconstruction
    // dangerous there — see the TUI migration plan's Phase 2 notes.
    //
    // Held in a `RefCell` (not plain locals) so both closures below can each
    // borrow it independently — two separate `FnMut` closures can't each
    // hold their own `&mut` to the same locals at once, but they can each
    // `borrow_mut()` the same `RefCell` at different times.
    type BoxTerminal = ratatui::Terminal<ratatui::backend::CrosstermBackend<io::Stdout>>;
    let tui_state: std::cell::RefCell<(Option<BoxTerminal>, u16)> =
        std::cell::RefCell::new((None, 0));

    // Shadows the free functions of the same name for the rest of this call,
    // so every existing `redraw_input_box(...)`/`clear_input_box(...)` call
    // site below keeps working unchanged — only what happens inside changed.
    let redraw_input_box = |input_chars: &[char],
                             cursor_pos: usize,
                             placeholder: &str,
                             model: &str,
                             path_str: &str,
                             tab_base_input: Option<&str>,
                             tab_index: Option<usize>,
                             current_dir: &Path,
                             _cursor_row: &mut usize| {
        let (lines, cursor_x, cursor_y) = compose_input_box(
            input_chars,
            cursor_pos,
            placeholder,
            model,
            path_str,
            tab_base_input,
            tab_index,
            current_dir,
            plan_mode,
        );
        let desired_height = lines.len() as u16;
        let mut state = tui_state.borrow_mut();
        let (terminal, viewport_height) = &mut *state;
        if terminal.is_none() || *viewport_height != desired_height {
            if let Some(mut old) = terminal.take() {
                let _ = old.clear();
            }
            let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
            if let Ok(new_terminal) = agent::with_raw_mode_for_cursor_query(move || {
                ratatui::Terminal::with_options(
                    backend,
                    ratatui::TerminalOptions {
                        viewport: ratatui::Viewport::Inline(desired_height),
                    },
                )
            }) {
                *terminal = Some(new_terminal);
                *viewport_height = desired_height;
            }
        }
        if let Some(t) = terminal.as_mut()
            && let Ok(text) = lines.join("\n").into_text()
        {
            let _ = agent::with_raw_mode_for_cursor_query(|| {
                t.draw(|frame| {
                    let area = frame.area();
                    frame.render_widget(ratatui::widgets::Paragraph::new(text), area);
                    // `set_cursor_position` takes an *absolute* screen
                    // coordinate (it's passed straight through to the
                    // backend, unlike `render_widget`'s `area` which is
                    // already the viewport's real on-screen `Rect`) — so
                    // `cursor_x`/`cursor_y` (relative to the box's own top
                    // row) need `area`'s own offset added, or this places
                    // the cursor wherever the viewport happened to be at
                    // (0, 0) instead of where the box actually is.
                    frame.set_cursor_position(ratatui::layout::Position::new(
                        area.x + cursor_x,
                        area.y + cursor_y,
                    ));
                })
            });
        }
    };
    let clear_input_box = |_cursor_row: usize| {
        let mut state = tui_state.borrow_mut();
        if let Some(mut t) = state.0.take() {
            let _ = t.clear();
        }
        state.1 = 0;
    };

    let mut cursor_row: usize = 0;
    redraw_input_box(
        &input_chars,
        cursor_pos,
        placeholder,
        model,
        path_str,
        None,
        None,
        current_dir,
        &mut cursor_row,
    );

    enable_raw_mode()?;
    let _ = crossterm::execute!(io::stdout(), crossterm::event::EnableBracketedPaste);

    let result = loop {
        if event::poll(std::time::Duration::from_millis(100))? {
            let ev = event::read()?;
            if let Event::Paste(text) = &ev {
                let clean_text = text.trim_end_matches(&['\r', '\n'][..]).to_string();
                let lines_count = clean_text.lines().count();
                if lines_count > 1 || clean_text.chars().count() > 100 {
                    let paste_id = paste_contents.len() + 1;
                    let placeholder_str = if lines_count > 1 {
                        format!("[Pasted text #{} +{} lines]", paste_id, lines_count - 1)
                    } else {
                        format!("[Pasted text #{}]", paste_id)
                    };
                    paste_contents.push((placeholder_str.clone(), clean_text));
                    for c in placeholder_str.chars() {
                        input_chars.insert(cursor_pos, c);
                        cursor_pos += 1;
                    }
                } else {
                    for c in clean_text.chars() {
                        input_chars.insert(cursor_pos, c);
                        cursor_pos += 1;
                    }
                }
                disable_raw_mode()?;
                redraw_input_box(
                    &input_chars,
                    cursor_pos,
                    placeholder,
                    model,
                    path_str,
                    None,
                    None,
                    current_dir,
                    &mut cursor_row,
                );
                enable_raw_mode()?;
                last_paste_time = Some(std::time::Instant::now());
                continue;
            }
            if let Event::Key(key_event) = ev
                && key_event.kind == event::KeyEventKind::Press
            {
                let is_ctrl_d = matches!(key_event.code, KeyCode::Char('d'))
                    && key_event
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL);

                if !is_ctrl_d {
                    ctrl_d_pressed = false;
                }

                // Reset tab autocomplete / history-browsing state if any key other
                // than Tab, Up, or Down is pressed
                if key_event.code != KeyCode::Tab
                    && key_event.code != KeyCode::Up
                    && key_event.code != KeyCode::Down
                {
                    tab_base_input = None;
                    tab_index = None;
                    history_index = None;
                }

                match key_event.code {
                    KeyCode::Char('c')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        input_chars.clear();
                        cursor_pos = 0;
                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Char('d')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if ctrl_d_pressed {
                            disable_raw_mode()?;
                            clear_input_box(cursor_row);
                            let _ = io::stdout().flush();
                            break Some(InteractiveInput {
                                text: "/exit".to_string(),
                                pasted_image: None,
                            });
                        } else {
                            ctrl_d_pressed = true;
                            disable_raw_mode()?;
                            let content_width = input_content_width();
                            let (rows, cursor_row_idx, _) =
                                wrap_input_into_rows(&input_chars, content_width, cursor_pos);
                            let down_lines = rows.len() - cursor_row_idx + 2;
                            print!(
                                "\r\x1b[{down_lines}B\r\x1b[2K{WARN}Press Ctrl+D again to exit{RESET}\x1b[{down_lines}A"
                            );
                            print!(
                                "\x1b[{}G",
                                cursor_visual_column(&input_chars, cursor_pos, content_width)
                            );
                            let _ = io::stdout().flush();
                            enable_raw_mode()?;
                        }
                    }
                    KeyCode::Char('v')
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::CONTROL) =>
                    {
                        if let Ok(Some(uri)) = image::read_clipboard_image() {
                            if let Some(ref mut current) = pasted_image {
                                current.push(' ');
                                current.push_str(&uri);
                            } else {
                                pasted_image = Some(uri);
                            }
                            insert_image_placeholder(&mut input_chars, &mut cursor_pos);

                            disable_raw_mode()?;
                            redraw_input_box(
                                &input_chars,
                                cursor_pos,
                                placeholder,
                                model,
                                path_str,
                                None,
                                None,
                                current_dir,
                                &mut cursor_row,
                            );
                            enable_raw_mode()?;
                        }
                    }
                    KeyCode::Char(c) if input_chars.len() < 10000 => {
                        input_chars.insert(cursor_pos, c);
                        cursor_pos += 1;

                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Backspace if cursor_pos > 0 => {
                        cursor_pos -= 1;
                        input_chars.remove(cursor_pos);

                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Tab => {
                        let base = match &tab_base_input {
                            Some(b) => b.clone(),
                            None => {
                                let current_str: String = input_chars.iter().collect();
                                tab_base_input = Some(current_str.clone());
                                current_str
                            }
                        };

                        if base.starts_with('/') {
                            let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
                                .iter()
                                .filter(|(cmd, _)| cmd.starts_with(&base))
                                .collect();

                            if !matches.is_empty() {
                                let idx = tab_index.unwrap_or(0) % matches.len();
                                let completed = format!("{} ", matches[idx].0);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

                                // Highlight currently completed item in suggestions
                                let current_highlight = Some(idx);
                                tab_index = Some(idx + 1);

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    Some(&base),
                                    current_highlight,
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if base.starts_with('$') {
                            let skill_query = base.split_whitespace().next().unwrap_or(&base);
                            let prefix = &skill_query[1..].to_lowercase();
                            let skills = load_all_available_skills(current_dir);
                            let matches: Vec<_> = skills
                                .iter()
                                .filter(|s| s.name.to_lowercase().starts_with(prefix))
                                .collect();

                            if !matches.is_empty() {
                                let idx = tab_index.unwrap_or(0) % matches.len();
                                let completed = format!("${} ", matches[idx].name);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

                                let current_highlight = Some(idx);
                                tab_index = Some(idx + 1);

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    Some(&base),
                                    current_highlight,
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        }
                    }
                    KeyCode::Down => {
                        let base = match &tab_base_input {
                            Some(b) => b.clone(),
                            None => {
                                let current_str: String = input_chars.iter().collect();
                                tab_base_input = Some(current_str.clone());
                                current_str
                            }
                        };

                        if base.starts_with('/') {
                            let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
                                .iter()
                                .filter(|(cmd, _)| cmd.starts_with(&base))
                                .collect();

                            if !matches.is_empty() {
                                let new_idx = match tab_index {
                                    Some(idx) => (idx + 1) % matches.len(),
                                    None => 0,
                                };
                                tab_index = Some(new_idx);
                                let completed = format!("{} ", matches[new_idx].0);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    Some(&base),
                                    Some(new_idx),
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if base.starts_with('$') {
                            let skill_query = base.split_whitespace().next().unwrap_or(&base);
                            let prefix = &skill_query[1..].to_lowercase();
                            let skills = load_all_available_skills(current_dir);
                            let matches: Vec<_> = skills
                                .iter()
                                .filter(|s| s.name.to_lowercase().starts_with(prefix))
                                .collect();

                            if !matches.is_empty() {
                                let new_idx = match tab_index {
                                    Some(idx) => (idx + 1) % matches.len(),
                                    None => 0,
                                };
                                tab_index = Some(new_idx);
                                let completed = format!("${} ", matches[new_idx].name);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    Some(&base),
                                    Some(new_idx),
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if let Some(idx) = history_index {
                            // Down moves toward more recent entries, then back to the draft.
                            let status_path;
                            if idx == 0 {
                                history_index = None;
                                input_chars = base.chars().collect();
                                status_path = path_str.to_string();
                            } else {
                                let new_idx = idx - 1;
                                history_index = Some(new_idx);
                                input_chars =
                                    history[history.len() - 1 - new_idx].chars().collect();
                                status_path =
                                    format!("\0History {}/{}", new_idx + 1, history.len());
                            }
                            cursor_pos = input_chars.len();

                            disable_raw_mode()?;
                            redraw_input_box(
                                &input_chars,
                                cursor_pos,
                                placeholder,
                                model,
                                &status_path,
                                None,
                                None,
                                current_dir,
                                &mut cursor_row,
                            );
                            enable_raw_mode()?;
                        }
                    }
                    KeyCode::Up => {
                        let base = match &tab_base_input {
                            Some(b) => b.clone(),
                            None => {
                                let current_str: String = input_chars.iter().collect();
                                tab_base_input = Some(current_str.clone());
                                current_str
                            }
                        };

                        if base.starts_with('/') {
                            let matches: Vec<_> = AUTOCOMPLETE_COMMANDS
                                .iter()
                                .filter(|(cmd, _)| cmd.starts_with(&base))
                                .collect();

                            if !matches.is_empty() {
                                let new_idx = match tab_index {
                                    Some(idx) => {
                                        if idx == 0 {
                                            matches.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    }
                                    None => matches.len() - 1,
                                };
                                tab_index = Some(new_idx);
                                let completed = format!("{} ", matches[new_idx].0);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    Some(&base),
                                    Some(new_idx),
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if base.starts_with('$') {
                            let skill_query = base.split_whitespace().next().unwrap_or(&base);
                            let prefix = &skill_query[1..].to_lowercase();
                            let skills = load_all_available_skills(current_dir);
                            let matches: Vec<_> = skills
                                .iter()
                                .filter(|s| s.name.to_lowercase().starts_with(prefix))
                                .collect();

                            if !matches.is_empty() {
                                let new_idx = match tab_index {
                                    Some(idx) => {
                                        if idx == 0 {
                                            matches.len() - 1
                                        } else {
                                            idx - 1
                                        }
                                    }
                                    None => matches.len() - 1,
                                };
                                tab_index = Some(new_idx);
                                let completed = format!("${} ", matches[new_idx].name);
                                input_chars = completed.chars().collect();
                                cursor_pos = input_chars.len();

                                disable_raw_mode()?;
                                redraw_input_box(
                                    &input_chars,
                                    cursor_pos,
                                    placeholder,
                                    model,
                                    path_str,
                                    Some(&base),
                                    Some(new_idx),
                                    current_dir,
                                    &mut cursor_row,
                                );
                                enable_raw_mode()?;
                            }
                        } else if !history.is_empty() {
                            // Up moves toward older entries, saving the in-progress
                            // draft (`base`) the first time history browsing starts.
                            let new_idx = match history_index {
                                Some(idx) if idx + 1 < history.len() => idx + 1,
                                Some(idx) => idx,
                                None => 0,
                            };
                            history_index = Some(new_idx);
                            input_chars = history[history.len() - 1 - new_idx].chars().collect();
                            cursor_pos = input_chars.len();
                            let status_path =
                                format!("\0History {}/{}", new_idx + 1, history.len());

                            disable_raw_mode()?;
                            redraw_input_box(
                                &input_chars,
                                cursor_pos,
                                placeholder,
                                model,
                                &status_path,
                                None,
                                None,
                                current_dir,
                                &mut cursor_row,
                            );
                            enable_raw_mode()?;
                        }
                    }
                    KeyCode::Left => {
                        while cursor_pos > 0 {
                            cursor_pos -= 1;
                            if cursor_pos == 0 || !is_thai_zero_width(input_chars[cursor_pos]) {
                                break;
                            }
                        }
                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Right => {
                        while cursor_pos < input_chars.len() {
                            cursor_pos += 1;
                            if cursor_pos == input_chars.len()
                                || !is_thai_zero_width(input_chars[cursor_pos])
                            {
                                break;
                            }
                        }
                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Enter
                        if key_event
                            .modifiers
                            .contains(crossterm::event::KeyModifiers::ALT) =>
                    {
                        // Alt+Enter inserts a literal newline without submitting —
                        // Enter alone still submits. Backspace/wrap/echo all treat
                        // '\n' as just another character already, so no other
                        // handler needs to change to support this.
                        input_chars.insert(cursor_pos, '\n');
                        cursor_pos += 1;

                        disable_raw_mode()?;
                        redraw_input_box(
                            &input_chars,
                            cursor_pos,
                            placeholder,
                            model,
                            path_str,
                            None,
                            None,
                            current_dir,
                            &mut cursor_row,
                        );
                        enable_raw_mode()?;
                    }
                    KeyCode::Enter => {
                        if let Some(time) = last_paste_time
                            && time.elapsed() < std::time::Duration::from_millis(100)
                        {
                            continue;
                        }
                        disable_raw_mode()?;
                        clear_input_box(cursor_row);
                        let input_str: String = input_chars.iter().collect();

                        let mut expanded_str = input_str.clone();
                        for (placeholder_str, content) in &paste_contents {
                            expanded_str = expanded_str.replace(placeholder_str, content);
                        }

                        let lines: Vec<&str> = expanded_str.lines().collect();
                        if lines.len() <= 1 {
                            println!("  {BLUE}You ›{RESET} {}", expanded_str);
                        } else {
                            for (idx, line) in lines.iter().enumerate() {
                                if idx == 0 {
                                    println!("  {BLUE}You ›{RESET} {}", line);
                                } else {
                                    println!("        {}", line);
                                }
                            }
                        }
                        let _ = io::stdout().flush();

                        break Some(InteractiveInput {
                            text: expanded_str,
                            pasted_image,
                        });
                    }
                    KeyCode::Esc => {
                        disable_raw_mode()?;
                        clear_input_box(cursor_row);
                        let _ = io::stdout().flush();
                        break None;
                    }
                    _ => {}
                }
            }
        } else {
            // Idle tick (no key event within the poll window) — a good place
            // to surface any /bg job, a background shell job (started via
            // run_shell(background: true)), or a linked-folder note write,
            // that finished while the user was typing elsewhere, without
            // disturbing whatever they're mid-edit on.
            let mut notices = jobs.take_notices();
            notices.extend(mint_core::bg_shell::take_finished_notices());
            notices.extend(mint_core::take_linked_folder_notices());
            if !notices.is_empty() {
                disable_raw_mode()?;
                clear_input_box(cursor_row);
                for notice in &notices {
                    println!(" {DIM}{}{RESET}", notice);
                }
                redraw_input_box(
                    &input_chars,
                    cursor_pos,
                    placeholder,
                    model,
                    path_str,
                    tab_base_input.as_deref(),
                    tab_index,
                    current_dir,
                    &mut cursor_row,
                );
                enable_raw_mode()?;
            }
        }
    };

    let _ = crossterm::execute!(io::stdout(), crossterm::event::DisableBracketedPaste);
    Ok(result)
}
pub fn insert_image_placeholder(input_chars: &mut Vec<char>, cursor_pos: &mut usize) {
    let input: String = input_chars.iter().collect();
    let mut idx = 1;
    while input.contains(&format!("[Image #{}]", idx)) {
        idx += 1;
    }
    let placeholder = format!("[Image #{}]", idx);
    let placeholder_chars = placeholder.chars().collect::<Vec<_>>();
    input_chars.splice(*cursor_pos..*cursor_pos, placeholder_chars.iter().copied());
    *cursor_pos += placeholder_chars.len();
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inserts_one_image_placeholder_at_cursor() {
        let mut chars = "ask ".chars().collect::<Vec<_>>();
        let mut cursor = chars.len();

        insert_image_placeholder(&mut chars, &mut cursor);

        assert_eq!(chars.iter().collect::<String>(), "ask [Image #1]");
        assert_eq!(cursor, "ask [Image #1]".chars().count());
    }

    #[test]
    fn empty_input_wraps_to_a_single_empty_row() {
        let (rows, row, col) = wrap_input_into_rows(&[], 10, 0);
        assert_eq!(rows, vec![String::new()]);
        assert_eq!((row, col), (0, 0));
    }

    #[test]
    fn short_input_stays_on_one_row() {
        let chars: Vec<char> = "hello".chars().collect();
        let (rows, row, col) = wrap_input_into_rows(&chars, 10, 3);
        assert_eq!(rows, vec!["hello".to_string()]);
        assert_eq!((row, col), (0, 3));
    }

    #[test]
    fn input_longer_than_row_width_splits_into_multiple_rows() {
        let chars: Vec<char> = "0123456789ABCDE".chars().collect(); // 15 chars
        let (rows, _, _) = wrap_input_into_rows(&chars, 10, 0);
        assert_eq!(rows, vec!["0123456789".to_string(), "ABCDE".to_string()]);
    }

    #[test]
    fn cursor_in_second_row_reports_correct_row_and_column() {
        let chars: Vec<char> = "0123456789ABCDE".chars().collect();
        let (_, row, col) = wrap_input_into_rows(&chars, 10, 12);
        assert_eq!((row, col), (1, 2));
    }

    #[test]
    fn cursor_at_exact_row_boundary_clamps_to_the_last_existing_row() {
        // 10 chars with row_width 10 makes exactly one full row; a cursor at
        // the end (pos 10) must not index a nonexistent second row.
        let chars: Vec<char> = "0123456789".chars().collect();
        let (rows, row, col) = wrap_input_into_rows(&chars, 10, 10);
        assert_eq!(rows.len(), 1);
        assert_eq!((row, col), (0, 10));
    }

    #[test]
    fn up_lines_for_a_single_row_matches_the_original_hardcoded_constants() {
        // Before wrapping existed this was hard-coded to 2 (no suggestions)
        // or 4+match_count (with suggestions); the generalized formula in
        // `position_input_cursor` must reduce to exactly those values when
        // there's only one input row.
        let chars: Vec<char> = "hi".chars().collect();
        let content_width = 50;
        let (rows, row_idx, _) = wrap_input_into_rows(&chars, content_width, 2);
        let row = row_idx + 1;
        let row_count = rows.len();

        let up_lines_no_suggestions = 0 + 2 + (row_count - row);
        assert_eq!(up_lines_no_suggestions, 2);

        let match_count = 3;
        let base = 2 + match_count;
        let up_lines_with_suggestions = base + 2 + (row_count - row);
        assert_eq!(up_lines_with_suggestions, 4 + match_count);
    }

    #[test]
    fn explicit_newline_forces_a_row_break_even_under_the_width_limit() {
        let chars: Vec<char> = "ab\ncd".chars().collect();
        let (rows, _, _) = wrap_input_into_rows(&chars, 10, 0);
        assert_eq!(rows, vec!["ab".to_string(), "cd".to_string()]);
    }

    #[test]
    fn cursor_right_before_a_newline_stays_on_the_preceding_row() {
        let chars: Vec<char> = "ab\ncd".chars().collect(); // indices: a=0 b=1 \n=2 c=3 d=4
        let (_, row, col) = wrap_input_into_rows(&chars, 10, 2);
        assert_eq!((row, col), (0, 2));
    }

    #[test]
    fn cursor_right_after_a_newline_starts_the_next_row() {
        let chars: Vec<char> = "ab\ncd".chars().collect();
        let (_, row, col) = wrap_input_into_rows(&chars, 10, 3);
        assert_eq!((row, col), (1, 0));
    }

    #[test]
    fn a_long_logical_line_still_wraps_by_width_between_newlines() {
        let chars: Vec<char> = "0123456789ABCDE\nxyz".chars().collect();
        let (rows, _, _) = wrap_input_into_rows(&chars, 10, 0);
        assert_eq!(
            rows,
            vec![
                "0123456789".to_string(),
                "ABCDE".to_string(),
                "xyz".to_string()
            ]
        );
    }

    #[test]
    fn trailing_newline_produces_an_extra_empty_row() {
        let chars: Vec<char> = "hi\n".chars().collect();
        let (rows, row, col) = wrap_input_into_rows(&chars, 10, 3);
        assert_eq!(rows, vec!["hi".to_string(), String::new()]);
        assert_eq!((row, col), (1, 0));
    }

    #[test]
    fn cursor_visual_column_accounts_for_row_start_after_a_newline() {
        let chars: Vec<char> = "hello\nworld".chars().collect();
        let content_width = 20;
        // cursor after "wor" on the second row (index 5 for '\n' + 1 + 3 = 9)
        let col = cursor_visual_column(&chars, 9, content_width);
        assert_eq!(col, 4 + 3);
    }
}

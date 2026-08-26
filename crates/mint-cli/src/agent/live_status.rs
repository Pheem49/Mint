use super::*;

pub(super) fn clear_working_status() {
    print!("\r\x1b[2K");
    let _ = io::stdout().flush();
}

pub(super) fn extract_domain(url: &str) -> String {
    let clean = url.trim();
    let without_scheme = clean
        .strip_prefix("https://")
        .or_else(|| clean.strip_prefix("http://"))
        .unwrap_or(clean);
    let hostname = without_scheme.split('/').next().unwrap_or(without_scheme);
    hostname
        .strip_prefix("www.")
        .unwrap_or(hostname)
        .to_lowercase()
}

/// Parse (title, url) pairs from the formatted `web_search` ToolEnd result text.
/// The format produced by orchestration.rs is:
///   1. Title\n   URL: https://...\n   Snippet\n
pub(super) fn parse_web_search_sources(result: &str) -> Vec<(String, String)> {
    let mut sources = Vec::new();
    let mut current_title: Option<String> = None;
    for line in result.lines() {
        let trimmed = line.trim();
        // Match numbered title lines: "1. Title text"
        if let Some(rest) = trimmed.split_once(". ")
            && rest.0.parse::<usize>().is_ok()
        {
            current_title = Some(rest.1.trim().to_owned());
            continue;
        }
        // Match URL lines: "URL: https://..."
        if let Some(url) = trimmed.strip_prefix("URL: ")
            && let Some(title) = current_title.take()
        {
            let url = url.trim().to_owned();
            if !url.is_empty() {
                sources.push((title, url));
            }
        }
    }
    sources
}

/// Playful gerunds shown in place of "Thinking" while the model is working,
/// picked once per turn so the label doesn't change mid-flight.
const THINKING_VERBS: &[&str] = &[
    "Thinking",
    "Pondering",
    "Percolating",
    "Ruminating",
    "Noodling",
    "Marinating",
    "Simmering",
    "Mulling",
    "Cogitating",
    "Deliberating",
    "Puzzling",
    "Burrowing",
    "Excavating",
    "Foraging",
    "Spelunking",
    "Divining",
    "Conjuring",
    "Brewing",
    "Synthesizing",
    "Weaving",
    "Wrangling",
    "Herding",
    "Contemplating",
    "Musing",
    "Untangling",
    "Unpacking",
];

pub(super) fn random_thinking_verb() -> &'static str {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    THINKING_VERBS[nanos as usize % THINKING_VERBS.len()]
}

pub(super) fn format_elapsed(duration: Duration) -> String {
    let total_seconds = duration.as_secs();
    let minutes = total_seconds / 60;
    let seconds = total_seconds % 60;
    if minutes == 0 {
        format!("{seconds}s")
    } else {
        format!("{minutes}m {seconds:02}s")
    }
}

pub(super) fn render_live_summary(summary: &str) {
    let (tw, _) = markdown::terminal_size_or_default();
    let width = tw as usize;

    let mut is_first = true;
    let mut table_buffer: Vec<String> = Vec::new();

    for line in summary.split('\n') {
        if markdown::is_table_line(line) {
            table_buffer.push(line.to_string());
            continue;
        }

        if !table_buffer.is_empty() {
            print_table_block(&table_buffer, &mut is_first);
            table_buffer.clear();
        }

        let indent = if is_first { "" } else { "  " };
        let options = textwrap::Options::new(width)
            .initial_indent(indent)
            .subsequent_indent("  ")
            .break_words(true);
        let wrapped = textwrap::fill(line, &options);

        if is_first {
            print!("{wrapped}");
            is_first = false;
        } else {
            print!("\n{wrapped}");
        }
    }

    if !table_buffer.is_empty() {
        print_table_block(&table_buffer, &mut is_first);
    }

    let _ = io::stdout().flush();
}

pub(super) fn print_table_block(table_lines: &[String], is_first: &mut bool) {
    let rendered = markdown::render_markdown_table(table_lines);
    for line in rendered.split('\n') {
        if *is_first {
            print!("{line}");
            *is_first = false;
        } else {
            print!("\n  {line}");
        }
    }
}

pub(super) fn should_show_verification(verification: &str) -> bool {
    let normalized = verification.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        return false;
    }
    if normalized.starts_with("information retrieved from web search")
        || normalized.starts_with("successfully ran background command")
        || normalized.starts_with("opened ")
        || normalized.contains("background command to open")
        || normalized.contains("web search results")
    {
        return false;
    }
    !matches!(
        normalized.as_str(),
        "not run"
            | "not run."
            | "no checks run"
            | "no checks run."
            | "no technical task requested"
            | "no technical task requested."
            | "no technical task requested, just a greeting."
            | "not required"
            | "not required."
            | "none"
            | "n/a"
    )
}

#[derive(Debug, Default)]
pub(super) struct LiveStatus {
    pub(super) thinking: Option<String>,
    /// Context-window usage (0-100) as of the last completed step —
    /// mirrored here (not just built into `thinking`'s label text) so the
    /// 150ms elapsed-time ticker in `run_code_agent_with_options` can keep
    /// including it too when it rebuilds `thinking` between the (much less
    /// frequent) `AgentProgress::Thinking` events, instead of silently
    /// dropping it on every tick.
    pub(super) context_pct: Option<u8>,
    /// `Some((attempt, max_attempts))` while retrying after every provider
    /// came back unreachable — same reasoning as `context_pct`: the 150ms
    /// ticker rebuilds `thinking` far more often than `AgentProgress` events
    /// arrive, so it needs its own copy to keep rendering "retrying (N/4)"
    /// instead of silently reverting to the generic "Thinking (Xs)…" label
    /// on its very next tick.
    pub(super) waiting_for_network: Option<(usize, usize)>,
    pub(super) explored: Vec<ExploredAction>,
    pub(super) activities: Vec<String>,
    pub(super) tasks: Vec<TaskEntry>,
    pub(super) plan_steps: Vec<String>,
    pub(super) committed_explored: usize,
    pub(super) committed_activities: usize,
    pub(super) committed_tasks: usize,
    pub(super) spinner_tick: usize,
    /// Sources collected from web_search ToolEnd results (title, url)
    pub(super) web_sources: Vec<(String, String)>,
    pub(super) inline_tui: InlineTui,
    /// Whether this turn keeps a typeable follow-up box pinned under the
    /// live status region (see [`AgentOptions::queueing`]).
    pub(super) queue_enabled: bool,
    /// Flips to `false` once the turn is wrapping up (final chunk printing,
    /// or the turn ending), so a stray keystroke can't resurrect the box
    /// after [`clear_live_status`] has already torn it down.
    pub(super) accepting_input: bool,
    /// In-progress text typed into the follow-up box, not yet submitted.
    pub(super) draft: Vec<char>,
    /// Follow-up messages submitted (Enter) while this turn was still
    /// running. Copied out to the caller's `queued_out` once the turn ends.
    pub(super) queued: Vec<String>,
    pub(super) model_label: String,
    pub(super) path_label: String,
    pub(super) plan_mode: bool,
}

/// A `ratatui` inline-viewport terminal for the live status region — the
/// only part of the interactive chat migrated to `ratatui` so far (see the
/// TUI migration plan). Lazily constructed on first use and torn down by
/// [`clear_live_status`] before any other code prints raw lines, since a
/// live `Terminal` desyncs (and corrupts the next redraw) if the screen
/// changes underneath it without going through the terminal's own API.
///
/// Deliberately constructed **once** per live stretch (not resized/rebuilt
/// as content grows) and held for the rest of the turn, including across
/// [`commit_activity_snapshot`]/[`print_timeline_note`]'s `insert_before`
/// calls. An earlier version reconstructed the whole `Terminal` (a fresh
/// cursor-position query + fresh `viewport_area`/`last_known_area`) every
/// time live content grew even slightly, and combined with `insert_before`
/// that measurably corrupted already-printed conversation history — the
/// repeated resets desynced `insert_before`'s internal scroll bookkeeping
/// from where content actually was on screen. A single long-lived instance
/// per turn is the pattern ratatui's own inline example uses (many
/// `insert_before` calls against one `Terminal`, never rebuilt mid-run),
/// so this follows that instead of inventing a resize dance the library
/// wasn't built around. The tradeoff: the live viewport's height is fixed
/// generously up front rather than tracking content exactly, so unusually
/// long in-flight content (e.g. a very long plan) can get visually clipped
/// in the *live* view — but nothing is lost, since it still prints in full
/// once committed (`insert_before`'s own per-call height isn't capped by
/// this).
#[derive(Debug, Default)]
pub(super) struct InlineTui {
    terminal: Option<ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>>,
}

/// Set for the duration of a `ratatui`/`crossterm` cursor-position query
/// (see [`with_raw_mode_for_cursor_query`]) so [`wait_for_escape_interrupt`]
/// — which polls the *same* stdin for an Esc key every 80ms for the entire
/// agent turn — knows to back off instead of racing to read the terminal's
/// `\x1b[row;colR` reply first. Without this, `event::read()` over there
/// can consume the reply before the query's own reader sees it (it doesn't
/// look like a recognized key event so it's silently swallowed), leaving
/// the query to time out or read whatever garbled remainder is left —
/// which is what the literal `^[[49;9R` text some users saw came from.
pub(super) static CURSOR_QUERY_ACTIVE: AtomicBool = AtomicBool::new(false);

/// Runs `f` with raw mode temporarily forced on, restoring whatever state
/// was in effect before. Any `ratatui`/`crossterm` call that queries the
/// terminal's cursor position sends `\x1b[6n` and reads the reply off
/// stdin — a reply that only reaches that read if raw mode is on; in cooked
/// mode the terminal instead local-echoes it as literal `^[[row;colR` text
/// into whatever's currently on screen. The rest of the interactive loop
/// only enables raw mode while reading a key, not during agent-turn output
/// (where the inline-viewport code runs), so every such call has to
/// bracket it like this itself. Also claims [`CURSOR_QUERY_ACTIVE`] for the
/// duration, so the Esc-watcher doesn't steal the reply off stdin.
pub(crate) fn with_raw_mode_for_cursor_query<T>(f: impl FnOnce() -> T) -> T {
    CURSOR_QUERY_ACTIVE.store(true, Ordering::Relaxed);
    let was_raw = crossterm::terminal::is_raw_mode_enabled().unwrap_or(false);
    if !was_raw {
        let _ = crossterm::terminal::enable_raw_mode();
    }
    let result = f();
    if !was_raw {
        let _ = crossterm::terminal::disable_raw_mode();
    }
    CURSOR_QUERY_ACTIVE.store(false, Ordering::Relaxed);
    result
}

impl InlineTui {
    /// Returns the live terminal, constructing it once (sized generously
    /// from the current window height, not from content) if this is the
    /// first call since the last [`teardown`](Self::teardown). Never
    /// reconstructs for an already-live terminal — see the struct docs for
    /// why that matters for `insert_before`'s correctness.
    fn ensure(
        &mut self,
    ) -> io::Result<&mut ratatui::Terminal<ratatui::backend::CrosstermBackend<std::io::Stdout>>>
    {
        if self.terminal.is_none() {
            let (_, term_rows) = markdown::terminal_size_or_default();
            // Generous enough for realistic in-flight content between two
            // commits, capped so it can't dominate a short terminal.
            let height = term_rows.saturating_sub(6).clamp(3, 20);
            let backend = ratatui::backend::CrosstermBackend::new(io::stdout());
            let terminal = with_raw_mode_for_cursor_query(move || {
                ratatui::Terminal::with_options(
                    backend,
                    ratatui::TerminalOptions {
                        viewport: ratatui::Viewport::Inline(height),
                    },
                )
            })?;
            self.terminal = Some(terminal);
        }
        Ok(self.terminal.as_mut().expect("just set above"))
    }

    fn teardown(&mut self) {
        self.drop_and_erase();
    }

    /// Erases the current terminal's drawn rows *before* dropping it — a
    /// bare `self.terminal = None` leaves last frame's content sitting on
    /// screen, so a freshly-constructed `Terminal`'s cursor-position query
    /// lands below it instead of on top of it, stacking a new copy under
    /// the old one every time. `Terminal::clear()` moves the cursor back to
    /// the top of its own viewport and clears from there, so the *next*
    /// construction's cursor query sees a clean slate in the right place.
    fn drop_and_erase(&mut self) {
        if let Some(mut terminal) = self.terminal.take() {
            let _ = terminal.clear();
        }
    }
}

#[derive(Debug, Clone, Default)]
pub(super) struct TaskEntry {
    pub(super) label: String,
    /// Truncated preview of the command's raw output, shown indented under the label.
    pub(super) output: Vec<String>,
}

impl From<String> for TaskEntry {
    fn from(label: String) -> Self {
        TaskEntry {
            label,
            output: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct ExploredAction {
    kind: &'static str,
    target: String,
}

impl ExploredAction {
    /// Flat `"{kind} {target}"` rendering, for contexts (a subagent's own
    /// nested tool calls) that show one label per call rather than grouping
    /// same-kind calls together the way [`explored_lines`] does.
    pub(super) fn as_label(&self) -> String {
        format!("{} {}", self.kind, self.target)
    }
}

pub(super) fn explored_action_label(
    action: &str,
    input: &serde_json::Value,
) -> Option<ExploredAction> {
    match action {
        "list_files" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|path| ExploredAction {
                kind: "[list_files] List",
                target: display_tool_target(path),
            }),
        "read_file" => {
            let path = input.get("path").and_then(|v| v.as_str())?;
            let start = input.get("startLine").and_then(|v| v.as_u64());
            let end = input.get("endLine").and_then(|v| v.as_u64());
            let file_name = display_tool_target(path);
            let target = match (start, end) {
                (Some(s), Some(e)) => format!("{} #L{}-{}", file_name, s, e),
                (Some(s), None) => format!("{} #L{}", file_name, s),
                _ => file_name,
            };
            Some(ExploredAction {
                kind: "[read_file] Read",
                target,
            })
        }
        "search_code" => {
            let query = input.get("query").and_then(|v| v.as_str())?;
            let path = input.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            let target = if path.trim().is_empty() || path == "." {
                query.to_owned()
            } else {
                format!("{} in {}", query, display_tool_target(path))
            };
            Some(ExploredAction {
                kind: "[search_code] Search",
                target,
            })
        }
        "symbols" => input
            .get("path")
            .and_then(|v| v.as_str())
            .map(|path| ExploredAction {
                kind: "[symbols] Index symbols",
                target: display_tool_target(path),
            }),
        _ => None,
    }
}

pub(super) fn display_tool_target(path: &str) -> String {
    if path.trim().is_empty() {
        ".".into()
    } else {
        Path::new(path)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(path)
            .into()
    }
}

pub(super) fn strip_ansi_escapes(s: &str) -> String {
    let mut result = String::new();
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c.is_ascii_alphabetic() {
                in_escape = false;
            }
        } else {
            result.push(c);
        }
    }
    result
}

pub(super) fn is_thai_combining(c: char) -> bool {
    matches!(c,
        '\u{0e31}' | '\u{0e34}'..='\u{0e37}' | '\u{0e38}'..='\u{0e39}' |
        '\u{0e47}'..='\u{0e4e}'
    )
}

pub(super) fn apply_wave_effect(text: &str, tick: usize) -> String {
    let (label, metadata) = if let Some(idx) = text.rfind(" • Esc to interrupt)") {
        if let Some(open_paren_idx) = text[..idx].rfind('(') {
            (&text[..open_paren_idx], &text[open_paren_idx..])
        } else {
            (text, "")
        }
    } else {
        (text, "")
    };

    let trimmed_label = label.trim_end();
    let spaces_count = label.len() - trimmed_label.len();

    let chars: Vec<char> = trimmed_label.chars().collect();
    let n = chars.len();
    if n == 0 {
        return text.to_string();
    }

    let mut animated_label = String::new();

    // Smooth wave movement using a sine wave function.
    // phase shifts with tick (time), index i acts as spatial shift.
    // 0.3 speed controls animation rate, 0.4 controls width of the wave crest.
    let speed = 0.3;
    let phase = tick as f32 * speed;

    for (i, &c) in chars.iter().enumerate() {
        if c == ' ' {
            animated_label.push(c);
            continue;
        }

        let x = (i as f32 * 0.4) - phase;
        let t = (x.sin() + 1.0) / 2.0; // Oscillates in [0.0, 1.0]

        // Stop colors: Dim Gray (70, 70, 70) -> Mint Green (105, 230, 166) -> Cyan (78, 201, 216)
        let (r, g, b) = if t < 0.3 {
            let local_t = t / 0.3;
            let r = 70.0 + (105.0 - 70.0) * local_t;
            let g = 70.0 + (230.0 - 70.0) * local_t;
            let b = 70.0 + (166.0 - 70.0) * local_t;
            (r, g, b)
        } else if t < 0.7 {
            let local_t = (t - 0.3) / 0.4;
            let r = 105.0 + (78.0 - 105.0) * local_t;
            let g = 230.0 + (201.0 - 230.0) * local_t;
            let b = 166.0 + (216.0 - 166.0) * local_t;
            (r, g, b)
        } else {
            let local_t = (t - 0.7) / 0.3;
            let r = 78.0 + (70.0 - 78.0) * local_t;
            let g = 201.0 + (70.0 - 201.0) * local_t;
            let b = 216.0 + (70.0 - 216.0) * local_t;
            (r, g, b)
        };

        animated_label.push_str(&format!(
            "\x1b[1m\x1b[38;2;{};{};{}m{}\x1b[0m",
            r.round() as u8,
            g.round() as u8,
            b.round() as u8,
            c
        ));
    }

    animated_label.push_str(&" ".repeat(spaces_count));
    if !metadata.is_empty() {
        animated_label.push_str(&format!("{BRIGHT}{metadata}{RESET}"));
    }
    animated_label
}

/// Builds the queueing follow-up box `render_live_status` pins under the
/// live region while an agent turn is running — a thin divider line plus a
/// plain (no filled background) input row, styled after Claude Code's own
/// mid-turn box rather than `compose_input_box`'s solid composer bar (see
/// the call site's comment for why). Shares `compose_input_box`'s exact
/// leading-margin-plus-prefix layout (one leading space, then `"› "`/`"  "`)
/// so it can reuse `wrap_input_into_rows`/`cursor_visual_column` unchanged —
/// those hardcode that layout's column offsets.
///
/// Returns the lines plus the cursor's (x, y) relative to line 0, same
/// contract as `compose_input_box`.
pub(super) fn compose_queue_box(
    input_chars: &[char],
    cursor_pos: usize,
    model: &str,
    path_str: &str,
    plan_mode: bool,
    thinking_display: Option<&str>,
) -> (Vec<String>, u16, u16) {
    let (term_width, _) = markdown::terminal_size_or_default();
    let width = term_width as usize;
    let content_max_len = crate::interactive::input_content_width();

    let mut lines: Vec<String> = Vec::new();
    // Pinned as its own row directly above the box's top divider — part of
    // this fixed-size box rather than the scrolling activity log above it
    // (see the caller in `render_live_status`), so it stays put right here
    // instead of drifting down every time a new activity/tool line gets
    // added above it.
    if let Some(display) = thinking_display {
        lines.push(format!("  {display}"));
    }
    lines.push(format!(
        "{DIM}{}{RESET}",
        "─".repeat(width.saturating_sub(2))
    ));

    let cursor_row_idx;
    if input_chars.is_empty() {
        cursor_row_idx = 0;
        lines.push(format!(
            " \x1b[1m{MINT}› {RESET}{DIM}Ask anything...{RESET}"
        ));
    } else {
        let (rows, c_row, _) =
            crate::interactive::wrap_input_into_rows(input_chars, content_max_len, cursor_pos);
        cursor_row_idx = c_row;
        for (i, row) in rows.iter().enumerate() {
            let row_prefix = if i == 0 { "› " } else { "  " };
            let display_row = crate::interactive::format_placeholders(row);
            lines.push(format!(" \x1b[1m{MINT}{row_prefix}{RESET}{display_row}"));
        }
    }

    lines.push(format!(
        "{DIM}{}{RESET}",
        "─".repeat(width.saturating_sub(2))
    ));

    let mode_label = if plan_mode { "[Plan]" } else { "[Agent]" };
    lines.push(format!(
        " {DIM}{mode_label}{RESET} {MINT}{model}{RESET}    {DIM}path: {path_str}{RESET}"
    ));

    let top_offset = if thinking_display.is_some() { 2 } else { 1 };
    let cursor_y = top_offset + cursor_row_idx as u16;
    let cursor_x =
        (crate::interactive::cursor_visual_column(input_chars, cursor_pos, content_max_len) as u16)
            .saturating_sub(1);
    (lines, cursor_x, cursor_y)
}

pub(super) fn render_live_status(status: &mut LiveStatus) {
    let mut lines = Vec::new();
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let tick = status.spinner_tick;
    // Advances every render call (not just while `status.thinking` is set) so
    // the plan/activity bullets below keep pulsing through a running tool —
    // a shell command in flight is still "live", even though the "Thinking
    // (Xs)…" text specifically only makes sense while waiting on the model.
    status.spinner_tick += 1;

    lines.extend(plan_lines(&status.plan_steps, true, tick));
    lines.extend(activity_block_lines(
        &status.tasks[tasks_start..],
        &status.activities[activities_start..],
        &status.explored[explored_start..],
        true,
        tick,
    ));
    // Built here (not inline below) so both destinations for it — the old
    // trailing-line spot in `lines`, and the queue box's own pinned row —
    // share one animation. When the queue box is about to be drawn, it's
    // handed to `compose_queue_box` instead of pushed into `lines`: as the
    // last line of the scrolling activity log, it used to drift down every
    // time a new activity/tool line landed above it, rather than staying
    // put right above the box it's actually about (see the box's "queueing
    // follow-up" comment below). Pinning it as the box's own top row fixes
    // that — same information, anchored to the thing it describes instead
    // of to whatever happened to print last.
    let thinking_display = status.thinking.as_ref().map(|thinking| {
        let frames = &[
            "🌑\u{FE0E}",
            "🌒\u{FE0E}",
            "🌓\u{FE0E}",
            "🌔\u{FE0E}",
            "🌕\u{FE0E}",
            "🌖\u{FE0E}",
            "🌗\u{FE0E}",
            "🌘\u{FE0E}",
        ];
        let frame = frames[status.spinner_tick % frames.len()];

        let dots_frames = &["", ".", "..", "..."];
        let dots = dots_frames[(status.spinner_tick / 2) % dots_frames.len()];

        // The verb is whatever precedes the trailing "(elapsed • Esc to interrupt)"
        // segment — find that split point rather than matching a literal word, since
        // the verb is now randomized per turn (see THINKING_VERBS).
        let custom_thinking = if let Some(idx) = thinking.rfind(" (") {
            format!("{}{:<3}{}", &thinking[..idx], dots, &thinking[idx..])
        } else {
            thinking.clone()
        };

        let waved_thinking = apply_wave_effect(&custom_thinking, status.spinner_tick);

        format!("{MINT}{frame}{RESET} {waved_thinking}")
    });
    let queue_box_will_show = status.queue_enabled && status.accepting_input;
    if !queue_box_will_show && let Some(display) = &thinking_display {
        lines.push(format!("  {display}"));
    }
    // The queueing follow-up box (see `AgentOptions::queueing`) is appended
    // after everything above, so it's always the bottom-most thing in the
    // live region. Deliberately a *different* style from
    // `compose_input_box` (the solid composer-background bar the primary
    // prompt uses between turns): a thin divider line instead, closer to
    // Claude Code's own mid-turn box — the solid bar reads as the main UI,
    // which fights for attention against the tool/activity log scrolling
    // above it, while a thin line reads as a temporary overlay.
    //
    // Rendered as its own `Paragraph`, in its own `Rect` below the status
    // lines, deliberately *without* `.wrap(...)` — matching how
    // `read_line_interactive`'s own box is drawn. Combining tightly-padded
    // box content with `Wrap` in the same `Paragraph` as the status lines
    // above it turned out to insert a phantom near-empty row after several
    // of them — visible as the box's cursor landing a row above its actual
    // text. Status lines (which can genuinely be longer than the terminal,
    // e.g. a long command or file path) still need `.wrap(...)`, so the fix
    // is two `Paragraph`s in two `Rect`s rather than dropping wrap
    // everywhere.
    let mut box_lines: Vec<String> = Vec::new();
    let mut box_cursor: Option<(u16, u16)> = None;
    if queue_box_will_show {
        let cursor_pos = status.draft.len();
        let (composed, cursor_x, cursor_y) = compose_queue_box(
            &status.draft,
            cursor_pos,
            &status.model_label,
            &status.path_label,
            status.plan_mode,
            thinking_display.as_deref(),
        );
        box_cursor = Some((cursor_x, cursor_y));
        box_lines = composed;
        for queued in &status.queued {
            let preview: String = queued.chars().take(80).collect();
            let ellipsis = if queued.chars().count() > preview.chars().count() {
                "…"
            } else {
                ""
            };
            box_lines.push(format!("  {DIM}Queued · {preview}{ellipsis}{RESET}"));
        }
    }

    if lines.is_empty() && box_lines.is_empty() {
        status.inline_tui.teardown();
        return;
    }

    let Ok(terminal) = status.inline_tui.ensure() else {
        return;
    };
    // `draw()` itself only queries the cursor position on the rare path
    // where it detects the terminal window was actually resized since the
    // last frame — bracket it too, defensively, for that case.
    let _ = with_raw_mode_for_cursor_query(|| {
        terminal.draw(|frame| {
            let area = frame.area();
            // The box (the "Ask anything..." input) claims its own rows
            // first, since it's a fixed handful of lines that must always
            // stay visible; the status/activity log — which can grow
            // unboundedly (e.g. several tool calls each with output
            // previews) — gets whatever's left over and clips/wraps instead.
            // Giving the log first claim (as this used to) let a long log
            // push the box's height down to zero, making it disappear
            // entirely whenever multiple commands ran at once.
            let box_height = (box_lines.len() as u16).min(area.height);
            let available_for_status = area.height.saturating_sub(box_height);

            let mut status_line_count: u16 = 0;
            let mut status_paragraph = None;
            if !lines.is_empty()
                && let Ok(status_text) = lines.join("\n").into_text()
            {
                // `lines.len()` counts logical entries, not the terminal rows
                // they occupy once wrapped — a single long shell command or
                // output preview line can expand to many rows. Measuring each
                // parsed `Line`'s display width (post-ANSI-stripping, via
                // `into_text()` above) against the real terminal width and
                // rounding up approximates the same wrapping `Wrap{trim:
                // false}` performs at render time, so the reserved height
                // stays in sync with what's actually drawn instead of
                // under-reserving and clipping trailing lines (e.g. the
                // "Deliberating" spinner, always last in `lines`) off the
                // bottom of the frame. (`Paragraph::line_count` would do this
                // exactly, but it's gated behind an unstable ratatui feature.)
                let wrap_width = area.width.max(1);
                status_line_count = status_text
                    .lines
                    .iter()
                    .map(|line| (line.width().max(1) as u16).div_ceil(wrap_width))
                    .sum();
                status_paragraph = Some(
                    ratatui::widgets::Paragraph::new(status_text)
                        .wrap(ratatui::widgets::Wrap { trim: false }),
                );
            }
            let status_height = status_line_count.min(available_for_status);
            let status_area = ratatui::layout::Rect {
                height: status_height,
                ..area
            };
            let box_area = ratatui::layout::Rect {
                y: area.y + status_height,
                height: area.height.saturating_sub(status_height),
                ..area
            };
            if let Some(paragraph) = status_paragraph {
                frame.render_widget(paragraph, status_area);
            }
            if !box_lines.is_empty()
                && let Ok(box_text) = box_lines.join("\n").into_text()
            {
                frame.render_widget(ratatui::widgets::Paragraph::new(box_text), box_area);
            }
            if let Some((cursor_x, cursor_y)) = box_cursor {
                frame.set_cursor_position(ratatui::layout::Position::new(
                    box_area.x + cursor_x,
                    box_area.y + cursor_y,
                ));
            }
        })
    });
}

pub(super) fn commit_activity_snapshot(status: &mut LiveStatus) {
    let explored_start = status.committed_explored.min(status.explored.len());
    let activities_start = status.committed_activities.min(status.activities.len());
    let tasks_start = status.committed_tasks.min(status.tasks.len());

    let mut lines = activity_block_lines(
        &status.tasks[tasks_start..],
        &status.activities[activities_start..],
        &status.explored[explored_start..],
        false,
        0,
    );
    if lines.is_empty() {
        return;
    }

    let (tw, _) = markdown::terminal_size_or_default();
    let width = tw as usize;
    lines.push(String::new());
    lines.push(format!("{DIM}{}{RESET}", "─".repeat(width)));
    lines.push(String::new());
    insert_permanent_lines(status, &lines);

    status.committed_explored = status.explored.len();
    status.committed_activities = status.activities.len();
    status.committed_tasks = status.tasks.len();
}

pub(super) fn print_timeline_note(status: &mut LiveStatus, thought: &str) {
    let thought = thought.trim();
    if thought.is_empty() {
        return;
    }
    let (tw, _) = markdown::terminal_size_or_default();
    let width = tw as usize;
    let options = textwrap::Options::new(width)
        .initial_indent("  • ")
        .subsequent_indent("    ")
        .break_words(true);
    let wrapped = textwrap::fill(thought, &options);
    insert_permanent_lines(status, &[wrapped]);
}

/// Inserts already ANSI-formatted `lines` as permanent content above the
/// live inline region — used by [`commit_activity_snapshot`] and
/// [`print_timeline_note`] to "freeze" in-flight status into real
/// scrollback, via `Terminal::insert_before`.
///
/// A first attempt at this kept `insert_before`'s target `Terminal` alive
/// correctly, but `InlineTui::ensure` at the time *also* reconstructed that
/// same `Terminal` from scratch every time live-status content grew even
/// slightly — a fresh cursor-position query and fresh `viewport_area`/
/// `last_known_area` on every reconstruction, discarding what
/// `insert_before` had been tracking. Interleaved with enough
/// reconstructions, that measurably corrupted already-printed conversation
/// history, not just live-status content. `ensure` no longer reconstructs
/// once a terminal is live (see its docs), so this is safe to use again:
/// exactly one `Terminal` instance persists for the whole live stretch, and
/// only `draw`/`insert_before` — never a full rebuild — touch it in between.
/// Falls back to plain `println!` if no inline terminal is currently live
/// (e.g. committing before anything has rendered yet this turn).
pub(super) fn insert_permanent_lines(status: &mut LiveStatus, lines: &[String]) {
    let Some(terminal) = status.inline_tui.terminal.as_mut() else {
        // Defensive, same reasoning as `approve_cb`/`on_chunk`: this is a
        // plain `println!`, so it needs cooked mode regardless of whether
        // `wait_for_escape_interrupt` has noticed yet.
        let _ = crossterm::terminal::disable_raw_mode();
        for line in lines {
            println!("{line}");
        }
        let _ = io::stdout().flush();
        return;
    };

    let (tw, _) = markdown::terminal_size_or_default();
    let width = tw as usize;
    let mut height: u16 = 0;
    for line in lines {
        let stripped = strip_ansi_escapes(line);
        let line_len = stripped.chars().filter(|&c| !is_thai_combining(c)).count();
        let physical_lines = if width > 0 {
            line_len.div_ceil(width)
        } else {
            1
        }
        .max(1);
        height = height.saturating_add(physical_lines as u16);
    }

    let Ok(text) = lines.join("\n").into_text() else {
        return;
    };
    let _ = terminal.insert_before(height, |buf| {
        use ratatui::widgets::Widget as _;
        // `height` above is computed assuming lines longer than the
        // terminal width wrap onto extra rows; without `.wrap(...)` here
        // the `Paragraph` instead clips each source line to a single row,
        // so a long committed line (e.g. a full shell command) reserved
        // more rows than it painted — leaving stray blank rows behind in
        // the scrollback. Wrapping keeps what's actually drawn in sync
        // with what was reserved.
        ratatui::widgets::Paragraph::new(text)
            .wrap(ratatui::widgets::Wrap { trim: false })
            .render(buf.area, buf);
    });
}

/// Tears down the shared inline `ratatui` terminal (if one is currently
/// live), so whatever prints next — an approval card, a committed activity
/// snapshot, the final AI answer — starts from a clean, un-managed cursor
/// position instead of colliding with content the inline viewport still
/// thinks it owns. Dropping the `Terminal` doesn't itself touch the screen
/// (its `Drop` impl only restores cursor visibility); the next
/// `render_live_status` call reconstructs it fresh, re-querying the
/// terminal for where the cursor actually is now.
pub(super) fn clear_live_status(status: &mut LiveStatus) {
    if status.inline_tui.terminal.is_none() {
        clear_working_status();
        return;
    }
    status.inline_tui.teardown();
}

/// `animate` distinguishes a still-live status region (pulse the bullet every
/// tick — true for the whole turn, not just while waiting on the model, so a
/// running shell command pulses too) from a snapshot already being committed
/// to permanent scrollback (`commit_activity_snapshot` passes `false` for a
/// single frozen `●`, since re-animating text that's already been printed
/// makes no sense).
pub(super) fn bullet_char(animate: bool, tick: usize) -> &'static str {
    if animate {
        if (tick / 4).is_multiple_of(2) {
            "●"
        } else {
            "○"
        }
    } else {
        "●"
    }
}

pub(super) fn get_bullet(name: &str, animate: bool, tick: usize) -> String {
    let char_str = bullet_char(animate, tick);
    match name {
        "plan" => format!("{BLUE}{char_str}{RESET} plan"),
        _ => char_str.to_string(),
    }
}

/// One-line rollup of everything counted in `activity_summary_line`, e.g.
/// "Searching for 8 patterns, reading 5 files, listing 1 directory, running 2
/// shell commands…" — shown as the header for the combined tasks/activities/
/// explored block instead of a bare word, so the user gets a sense of scope
/// at a glance instead of only a growing, undifferentiated list.
pub(super) fn activity_summary_line(
    tasks: &[TaskEntry],
    activities: &[String],
    explored: &[ExploredAction],
) -> Option<String> {
    let mut pattern_count = 0usize;
    let mut file_count = 0usize;
    let mut dir_count = 0usize;
    let mut symbol_count = 0usize;
    for action in explored {
        match action.kind {
            "[search_code] Search" => pattern_count += 1,
            "[read_file] Read" => file_count += 1,
            "[list_files] List" => dir_count += 1,
            "[symbols] Index symbols" => symbol_count += 1,
            _ => {}
        }
    }
    let web_count = activities.len();
    let shell_count = tasks
        .iter()
        .filter(|t| t.label.starts_with("[run_shell]"))
        .count();

    let mut parts: Vec<String> = Vec::new();
    if pattern_count > 0 {
        parts.push(format!(
            "searching for {pattern_count} pattern{}",
            if pattern_count == 1 { "" } else { "s" }
        ));
    }
    if file_count > 0 {
        parts.push(format!(
            "reading {file_count} file{}",
            if file_count == 1 { "" } else { "s" }
        ));
    }
    if dir_count > 0 {
        parts.push(format!(
            "listing {dir_count} director{}",
            if dir_count == 1 { "y" } else { "ies" }
        ));
    }
    if symbol_count > 0 {
        parts.push(format!(
            "indexing {symbol_count} symbol file{}",
            if symbol_count == 1 { "" } else { "s" }
        ));
    }
    if web_count > 0 {
        parts.push(format!(
            "searching the web {web_count} time{}",
            if web_count == 1 { "" } else { "s" }
        ));
    }
    if shell_count > 0 {
        parts.push(format!(
            "running {shell_count} shell command{}",
            if shell_count == 1 { "" } else { "s" }
        ));
    }

    if parts.is_empty() {
        return None;
    }
    let sentence = parts.join(", ");
    let mut chars = sentence.chars();
    let capitalized = match chars.next() {
        Some(c) => c.to_uppercase().collect::<String>() + chars.as_str(),
        None => sentence,
    };
    Some(format!("{capitalized}…"))
}

/// Combined header + body for the tasks/activities/explored groups: a single
/// summary line (via `activity_summary_line`) instead of three separate,
/// unlabeled bullet groups.
pub(super) fn activity_block_lines(
    tasks: &[TaskEntry],
    activities: &[String],
    explored: &[ExploredAction],
    animate: bool,
    tick: usize,
) -> Vec<String> {
    if tasks.is_empty() && activities.is_empty() && explored.is_empty() {
        return Vec::new();
    }
    let char_str = bullet_char(animate, tick);
    let header_text =
        activity_summary_line(tasks, activities, explored).unwrap_or_else(|| "activity".into());
    let mut lines = vec![format!("  {BLUE}{char_str}{RESET} {header_text}")];
    lines.extend(tasks_lines(tasks));
    lines.extend(activities_lines(activities));
    lines.extend(explored_lines(explored));
    lines
}

pub(super) fn explored_lines(actions: &[ExploredAction]) -> Vec<String> {
    if actions.is_empty() {
        return Vec::new();
    }
    let grouped = grouped_explored_actions(actions);
    let mut lines: Vec<String> = grouped
        .iter()
        .take(24)
        .enumerate()
        .map(|(index, action)| {
            let prefix = if index == 0 { "    └" } else { "     " };
            format!("{DIM}{prefix} {action}{RESET}")
        })
        .collect();
    if grouped.len() > 24 {
        lines.push(format!("{DIM}     ... {} more{RESET}", grouped.len() - 24));
    }
    lines
}

pub(super) fn ran_command_labels(action: &str, input: &serde_json::Value) -> Option<Vec<String>> {
    match action {
        "run_shell" => input
            .get("command")
            .and_then(|v| v.as_str())
            .filter(|command| !command.trim().is_empty())
            .map(|command| vec![command.trim().to_owned()]),
        "shell_output" | "kill_shell" => input
            .get("job_id")
            .and_then(|v| v.as_str())
            .filter(|job_id| !job_id.trim().is_empty())
            .map(|job_id| vec![job_id.trim().to_owned()]),
        "verify" => input
            .get("commands")
            .and_then(|v| v.as_array())
            .map(|commands| {
                commands
                    .iter()
                    .filter_map(|command| command.as_str())
                    .map(str::trim)
                    .filter(|command| !command.is_empty())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            }),
        _ => None,
    }
}

/// If `path` looks like a skill file — `<skills-dir>/<name>/SKILL.md` (or
/// SKILL.txt / lowercase variants), or a flat `<skills-dir>/<name>.md` in the
/// global skills folder — returns the skill's name. Mirrors the file shapes
/// `mint_core::skills::load_skills_from_dir` recognizes. Used to tell a
/// skill file read apart from an ordinary `read_file` call so it renders as
/// a `Skill(name)` card instead of a generic "explored files" line.
pub(super) fn skill_name_for_read_path(path: &str) -> Option<String> {
    let path = Path::new(path);
    let file_name = path.file_name()?.to_str()?;
    let parent = path.parent()?;
    let parent_name = parent.file_name()?.to_str()?;

    if matches!(
        file_name,
        "SKILL.md" | "SKILL.txt" | "skill.md" | "skill.txt"
    ) {
        let grandparent_name = parent
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str());
        if matches!(grandparent_name, Some("skills") | Some("mint-skills")) {
            return Some(parent_name.to_string());
        }
    }

    if matches!(parent_name, "skills" | "mint-skills") {
        let ext = path.extension().and_then(|e| e.to_str())?;
        if matches!(ext.to_ascii_lowercase().as_str(), "md" | "txt") {
            return path.file_stem().and_then(|n| n.to_str()).map(str::to_owned);
        }
    }

    None
}

/// Skill names the `memory_recall` tool matched, parsed out of its
/// `"[Skill: {name}]\n{content}"` result blocks (see the `"memory_recall"`
/// arm in `orchestration.rs`) — lets ToolEnd render a `Skill(name)` card
/// when the agent found and is about to use a learned skill this way.
pub(super) fn skill_names_from_memory_recall(result: &str) -> Vec<String> {
    result
        .lines()
        .filter_map(|line| {
            line.strip_prefix("[Skill: ")
                .and_then(|rest| rest.strip_suffix(']'))
                .map(str::to_owned)
        })
        .collect()
}

pub(super) fn skill_card(skill_name: &str) -> TaskEntry {
    TaskEntry {
        label: format!("Skill({skill_name})"),
        output: vec!["Successfully loaded skill".to_string()],
    }
}

pub(super) fn command_was_run(result: &str) -> bool {
    result.lines().any(|line| line.starts_with("exit: "))
}

/// Truncated preview of a command's raw stdout/stderr, shown indented under its
/// "Finished command" label. Drops internal bookkeeping lines (`mode:`, `sandboxed:`)
/// and caps the output so a noisy command can't flood the terminal.
pub(super) fn command_output_preview(result: &str) -> Vec<String> {
    const MAX_LINES: usize = 7;
    // A single very long line (a full `pgrep -af` invocation, a minified
    // JSON blob, ...) still wraps across many terminal rows even though it
    // only counts as one line against MAX_LINES above — cut it down too.
    const MAX_LINE_WIDTH: usize = 120;

    let filtered: Vec<&str> = result
        .lines()
        .filter(|line| {
            let trimmed = line.trim_start();
            !(trimmed.starts_with("mode: ") || trimmed.starts_with("sandboxed: "))
        })
        .collect();

    let start = filtered.iter().position(|l| !l.trim().is_empty());
    let Some(start) = start else {
        return Vec::new();
    };
    let end = filtered
        .iter()
        .rposition(|l| !l.trim().is_empty())
        .map_or(start, |i| i + 1);
    let trimmed = &filtered[start..end];

    let mut preview: Vec<String> = trimmed
        .iter()
        .take(MAX_LINES)
        .map(|line| truncate_line(line, MAX_LINE_WIDTH))
        .collect();
    if trimmed.len() > MAX_LINES {
        preview.push(format!("... {} more lines", trimmed.len() - MAX_LINES));
    }
    preview
}

pub(super) fn truncate_line(line: &str, max_chars: usize) -> String {
    if line.chars().count() <= max_chars {
        return line.to_string();
    }
    let head: String = line.chars().take(max_chars).collect();
    format!("{head}…")
}

pub(super) fn activities_lines(activities: &[String]) -> Vec<String> {
    if activities.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> = activities
        .iter()
        .take(24)
        .enumerate()
        .map(|(index, act)| {
            let prefix = if index == 0 { "    └" } else { "     " };
            format!("{DIM}{prefix} {act}{RESET}")
        })
        .collect();
    if activities.len() > 24 {
        lines.push(format!(
            "{DIM}     ... {} more{RESET}",
            activities.len() - 24
        ));
    }
    lines
}

pub(super) fn extract_plan_steps(input: &serde_json::Value) -> Option<Vec<String>> {
    let steps_val = input.get("steps")?;
    let arr = steps_val.as_array()?;
    let mut steps = Vec::new();
    for v in arr {
        if let Some(s) = v.as_str() {
            steps.push(s.to_string());
        }
    }
    Some(steps)
}

pub(super) fn plan_lines(steps: &[String], animate: bool, tick: usize) -> Vec<String> {
    if steps.is_empty() {
        return Vec::new();
    }
    let mut lines = vec![format!("  {}", get_bullet("plan", animate, tick))];
    for (index, step) in steps.iter().enumerate() {
        let prefix = if index == steps.len() - 1 {
            "    └"
        } else {
            "    ├"
        };
        let (checked, text) = if step.to_lowercase().starts_with("done:") {
            (format!("{MINT}[x]{RESET}"), step["done:".len()..].trim())
        } else if step.to_lowercase().starts_with("done: ") {
            (format!("{MINT}[x]{RESET}"), step["done: ".len()..].trim())
        } else if step.to_lowercase().starts_with("in_progress:") {
            (
                format!("{BLUE}[~]{RESET}"),
                step["in_progress:".len()..].trim(),
            )
        } else if step.to_lowercase().starts_with("in_progress: ") {
            (
                format!("{BLUE}[~]{RESET}"),
                step["in_progress: ".len()..].trim(),
            )
        } else if step.to_lowercase().starts_with("todo:") {
            (format!("{DIM}[ ]{RESET}"), step["todo:".len()..].trim())
        } else if step.to_lowercase().starts_with("todo: ") {
            (format!("{DIM}[ ]{RESET}"), step["todo: ".len()..].trim())
        } else {
            (format!("{DIM}[ ]{RESET}"), step.trim())
        };
        lines.push(format!("{DIM}{} {}{RESET} {}", prefix, checked, text));
    }
    lines
}

pub(super) fn tasks_lines(tasks: &[TaskEntry]) -> Vec<String> {
    if tasks.is_empty() {
        return Vec::new();
    }
    let mut lines = Vec::new();
    for (index, task) in tasks.iter().take(24).enumerate() {
        let prefix = if index == 0 { "    └" } else { "     " };
        lines.push(format!("{DIM}{prefix} {}{RESET}", task.label));
        for out_line in &task.output {
            lines.push(format!("{DIM}       │ {}{RESET}", out_line));
        }
    }
    if tasks.len() > 24 {
        lines.push(format!("{DIM}     ... {} more{RESET}", tasks.len() - 24));
    }
    lines
}

pub(super) fn grouped_explored_actions(actions: &[ExploredAction]) -> Vec<String> {
    let mut groups: Vec<(&str, Vec<&str>)> = Vec::new();
    for action in actions {
        if let Some((_, targets)) = groups.iter_mut().find(|(kind, _)| *kind == action.kind) {
            if !targets.iter().any(|target| *target == action.target) {
                targets.push(action.target.as_str());
            }
        } else {
            groups.push((action.kind, vec![action.target.as_str()]));
        }
    }
    groups
        .into_iter()
        .map(|(kind, targets)| format!("{} {}", kind, targets.join(", ")))
        .collect()
}
